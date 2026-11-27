use crate::db::{now_rfc3339, Db, SessionRow};
use crate::error::{path_display, AppError, Result};
use crate::library;
use crate::model::{RunnerKind, Session, SessionState};
use crate::runners;
use serde_json::json;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

pub trait EventSink: Send + Sync {
    fn emit(&self, event: crate::model::CoreEvent);
}

pub fn list(db: &Db) -> Result<Vec<Session>> {
    db.list_sessions()?
        .into_iter()
        .map(row_to_session)
        .collect()
}

pub(crate) fn row_to_session(row: SessionRow) -> Result<Session> {
    Ok(Session {
        id: row.id,
        edition_id: row.edition_id,
        runner_id: row.runner_id,
        started_at: row.started_at,
        ended_at: row.ended_at,
        duration_seconds: row.duration_seconds,
        exit_code: row.exit_code,
        state: match row.state.as_str() {
            "running" => SessionState::Running,
            "failed" => SessionState::Failed,
            _ => SessionState::Ended,
        },
        error: row.error,
        port_id: None,
        mode: if row.mode.is_empty() {
            "local".into()
        } else {
            row.mode
        },
    })
}

pub fn launch(
    db: Arc<Mutex<Db>>,
    edition_id: &str,
    extra_args: &[String],
    sink: Arc<dyn EventSink>,
    xdg_config: Option<std::path::PathBuf>,
    lease: Arc<crate::instance::InstanceLock>,
) -> Result<Session> {
    let (session, argv, runner_kind, isolate) = {
        let db = db.lock().unwrap_or_else(|e| e.into_inner());
        if db.running_session()?.is_some() || crate::port_sessions::has_running(&db.conn)? {
            return Err(AppError::new("LAUNCH_BUSY", "a game is already running"));
        }
        let edition = db
            .edition_by_id(edition_id)?
            .ok_or_else(|| AppError::new("MEDIA_MISSING", "title is not in the library"))?;
        let system = edition.system.clone();
        let media = library::find_media_for_edition(&db, edition_id)?
            .ok_or_else(|| AppError::new("MEDIA_MISSING", "no disc is attached to this title"))?;
        if !media.available {
            return Err(AppError::new(
                "MEDIA_MISSING",
                media.skip_reason.clone().unwrap_or_else(|| {
                    format!("disc file is missing: {}", path_display(&media.path))
                }),
            )
            .with_context(json!({ "path": path_display(&media.path) })));
        }
        // A `.bin` track is never ingested on its own — it belongs to a `.cue`
        // pair, and the `.cue` is the entry that imports and launches. So any row
        // carrying a skip reason here is genuinely unsupported.
        if media.skip_reason.is_some() {
            return Err(AppError::new(
                "MEDIA_MISSING",
                media
                    .skip_reason
                    .clone()
                    .unwrap_or_else(|| "This media cannot be launched".into()),
            ));
        }
        if system == "xbox360" {
            let identity = marquee_media::xbox360::inspect(&media.path)?;
            if media.serial.as_deref() != Some(identity.title_id.as_str()) {
                return Err(AppError::new(
                    "MEDIA_CHANGED",
                    "Xbox 360 disc identity changed. Rescan before launching.",
                ));
            }
        }
        let runners = runners::snapshot(&db)?;
        // The stored selection is a hint for one emulator family; only honour it
        // when it matches this title's system, so a PS2 pin cannot hijack a PS1
        // launch (and vice versa).
        let legacy = db.get_setting("selected_runner_id")?;
        let selected = db
            .get_setting(&format!("selected_runner.{system}"))?
            .or_else(|| {
                runners::selection_for(&runners, legacy.as_deref(), &system)
                    .filter(|id| runners.iter().any(|r| r.id == *id && r.available))
                    .map(str::to_owned)
            });
        let profile = db.profile_runner(edition_id)?;
        let runner =
            runners::resolve(&runners, &system, profile.as_deref(), selected.as_deref())?.clone();
        runners::require_launchable(&runner)?;
        let fullscreen = db.get_setting("display_mode")?.as_deref() == Some("fullscreen");
        let argv = runners::launch_argv(&runner, &media.path, extra_args, fullscreen)?;
        let session_id = Uuid::new_v4().to_string();
        let started = now_rfc3339();
        db.insert_session(&session_id, edition_id, &runner.id, &started, "running")?;
        let session = Session {
            id: session_id,
            edition_id: edition_id.to_string(),
            runner_id: runner.id.clone(),
            started_at: started,
            ended_at: None,
            duration_seconds: None,
            exit_code: None,
            state: SessionState::Running,
            error: None,
            port_id: None,
            mode: "local".into(),
        };
        let isolate = runners::uses_marquee_prefix(&runner);
        // Pair the game with a BIOS of its own region. Best effort: no match,
        // no settings file yet, or any error simply leaves PCSX2's own choice.
        if isolate && runner.kind == RunnerKind::Pcsx2 {
            if let Some(home) = xdg_config.as_deref() {
                let _ = crate::bios::select_for_pcsx2(home, media.region.as_deref());
            }
        }
        // Marquee's DuckStation carries its own XDG_CONFIG_HOME in the command
        // (`env XDG_CONFIG_HOME=… <program>`); skip its first-run wizard there.
        if runner.kind == RunnerKind::Duckstation && runner.source == "marquee" {
            if let Some(home) = runner
                .command
                .iter()
                .find_map(|a| a.strip_prefix("XDG_CONFIG_HOME="))
            {
                let _ = crate::profiles::prepare(Path::new(home));
            }
        }
        // Generic catalogue emulators: write the entry's seed files (first-run
        // wizard suppression) into the config folder the command points at.
        if runner.kind == RunnerKind::Emulator && runner.source == "marquee" {
            let home = runner
                .command
                .iter()
                .find_map(|a| a.strip_prefix("XDG_CONFIG_HOME="));
            if let (Some(home), Some(def)) = (home, runners::emulator_for_runner(&runner.id)) {
                crate::emulators::seed_config(&def, Path::new(home));
            }
        }
        (session, argv, runner.kind, isolate)
    };

    sink.emit(crate::model::CoreEvent::SessionStarted {
        session: session.clone(),
    });

    let child = spawn(&argv, if isolate { xdg_config.as_deref() } else { None });
    match child {
        Ok(mut child) => {
            let identity = ProcessIdentity::read(child.id());
            if let Some(ref identity) = identity {
                let bind = db
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .bind_process(&session.id, &serde_json::to_string(identity)?);
                if let Err(err) = bind {
                    let _ = child.kill();
                    let _ = child.wait();
                    let guard = db.lock().unwrap_or_else(|e| e.into_inner());
                    guard.complete_session(
                        &session.id,
                        0,
                        None,
                        "failed",
                        Some(&err.to_string()),
                        false,
                    )?;
                    if let Some(row) = guard.session_by_id(&session.id)? {
                        sink.emit(crate::model::CoreEvent::SessionFailed {
                            session: row_to_session(row)?,
                        });
                    }
                    return Err(err);
                }
            }

            let db = Arc::clone(&db);
            let sink = Arc::clone(&sink);
            let session_id = session.id.clone();
            let edition_id = session.edition_id.clone();
            let runner_id = session.runner_id.clone();
            thread::spawn(move || {
                wait_and_finish(
                    db,
                    sink,
                    session_id,
                    edition_id,
                    runner_id,
                    runner_kind,
                    child,
                    identity,
                    lease,
                );
            });
            Ok(session)
        }
        Err(err) => {
            let dbg = db.lock().unwrap_or_else(|e| e.into_inner());
            let ended = now_rfc3339();
            dbg.finish_session(
                &session.id,
                &ended,
                0,
                None,
                "failed",
                Some(&err.to_string()),
            )?;
            let mut failed = session.clone();
            failed.ended_at = Some(ended);
            failed.duration_seconds = Some(0);
            failed.state = SessionState::Failed;
            failed.error = Some(err.to_string());
            sink.emit(crate::model::CoreEvent::SessionFailed {
                session: failed.clone(),
            });
            Err(AppError::new("SPAWN_FAILED", err.to_string())
                .with_context(json!({ "argv": argv })))
        }
    }
}

fn spawn(argv: &[String], xdg_config: Option<&Path>) -> std::io::Result<std::process::Child> {
    let (bin, args) = argv
        .split_first()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "empty argv"))?;
    let mut cmd = Command::new(bin);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // Its own process group, so Stop also reaches wrapper children (Wine, flatpak).
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    if let Some(home) = xdg_config {
        cmd.env("XDG_CONFIG_HOME", home);
    }
    cmd.spawn()
}

// Process bookkeeping (identity + lease) legitimately needs this many inputs.
#[allow(clippy::too_many_arguments)]
fn wait_and_finish(
    db: Arc<Mutex<Db>>,
    sink: Arc<dyn EventSink>,
    session_id: String,
    _edition_id: String,
    _runner_id: String,
    runner_kind: RunnerKind,
    child: std::process::Child,
    identity: Option<ProcessIdentity>,
    _lease: Arc<crate::instance::InstanceLock>,
) {
    let watched = watch_child(
        child,
        identity.as_ref(),
        runner_kind != RunnerKind::Fake,
        |seconds| {
            let _ = db
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .checkpoint_session(&session_id, seconds);
        },
    );
    let duration = watched.duration;
    let exit_code = watched.exit_code;
    let state = watched.state;
    let error = watched.error;

    {
        let guard = db.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(err) = guard.complete_session(
            &session_id,
            duration,
            exit_code,
            state,
            error.as_deref(),
            state == "ended",
        ) {
            eprintln!("could not finalize session {session_id}: {err}");
            return;
        }
        // Process exit alone does not establish that a BIOS/game booted.
        if let Ok(Some(row)) = guard.session_by_id(&session_id) {
            if let Ok(session) = row_to_session(row) {
                if state == "ended" {
                    sink.emit(crate::model::CoreEvent::SessionEnded { session });
                } else {
                    sink.emit(crate::model::CoreEvent::SessionFailed { session });
                }
            }
        }
    }
}

/// A normal exit sooner than this after launch is a failed start, not a played session.
pub(crate) const EARLY_EXIT_SECS: i64 = 10;

pub(crate) struct WatchedExit {
    pub duration: i64,
    pub exit_code: Option<i32>,
    pub state: &'static str,
    pub error: Option<String>,
}

/// Wait for a game process, checkpointing its runtime. A clean exit before
/// `EARLY_EXIT_SECS` is a failed start when `early_failure` is set.
pub(crate) fn watch_child(
    mut child: std::process::Child,
    identity: Option<&ProcessIdentity>,
    early_failure: bool,
    mut checkpoint: impl FnMut(i64),
) -> WatchedExit {
    let pid = child.id();
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Err(err) => break Err(err),
            Ok(None) => {
                let seconds = identity
                    .map(ProcessIdentity::elapsed)
                    .unwrap_or(start.elapsed().as_secs() as i64);
                checkpoint(seconds);
                thread::sleep(Duration::from_millis(250));
            }
        }
    };
    let duration = identity
        .map(ProcessIdentity::elapsed)
        .unwrap_or(start.elapsed().as_secs() as i64);
    let (exit_code, state, error) = match status {
        Ok(st) => {
            let code = st.code();
            if st.success() && early_failure && duration < EARLY_EXIT_SECS {
                (
                    code,
                    "failed",
                    Some(format!(
                        "the program closed itself after {duration}s without a game being played"
                    )),
                )
            } else if st.success() {
                (code, "ended", None)
            } else {
                (
                    code,
                    "failed",
                    Some(format!("process exited with {code:?}")),
                )
            }
        }
        Err(err) => (None, "failed", Some(err.to_string())),
    };
    // A game the owner stopped was played, not a failed start: credit its time.
    let (state, error) = match take_stop(pid) {
        Some(Stop::Closed) => ("ended", Some("Stopped from Marquee".to_string())),
        Some(Stop::Forced) => (
            "ended",
            Some("Force-stopped from Marquee: it did not close in time".to_string()),
        ),
        None => (state, error),
    };
    WatchedExit {
        duration,
        exit_code,
        state,
        error,
    }
}

// Linux process identity includes the boot and kernel start time, so a reused PID
// can never be mistaken for the emulator. Uptime excludes suspend just like Instant.
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct ProcessIdentity {
    pid: u32,
    boot: String,
    ticks: String,
    began: f64,
}
fn uptime() -> Option<f64> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: a valid, writable timespec is supplied to the system clock API.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) } != 0 {
        return None;
    }
    Some(time.tv_sec as f64 + time.tv_nsec as f64 / 1_000_000_000.0)
}
impl ProcessIdentity {
    pub(crate) fn read(pid: u32) -> Option<Self> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let fields: Vec<_> = stat.rsplit_once(')')?.1.split_whitespace().collect();
        if fields.first() == Some(&"Z") {
            return None;
        }
        Some(Self {
            pid,
            boot: std::fs::read_to_string("/proc/sys/kernel/random/boot_id").ok()?,
            ticks: fields.get(19)?.to_string(),
            began: uptime()?,
        })
    }
    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }
    pub(crate) fn alive(&self) -> bool {
        Self::read(self.pid).is_some_and(|p| p.boot == self.boot && p.ticks == self.ticks)
    }
    pub(crate) fn elapsed(&self) -> i64 {
        uptime()
            .map(|now| (now - self.began).max(0.0) as i64)
            .unwrap_or(0)
    }
}

/// STOP1: how long a game gets to close after SIGTERM before it is killed.
pub(crate) const STOP_GRACE: Duration = Duration::from_secs(8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stop {
    Closed,
    Forced,
}

/// Stops requested for live game processes, keyed by PID, read by `watch_child`.
static STOPS: Mutex<std::collections::BTreeMap<u32, Stop>> =
    Mutex::new(std::collections::BTreeMap::new());

fn mark_stop(pid: u32, stop: Stop) {
    STOPS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(pid, stop);
}

pub(crate) fn take_stop(pid: u32) -> Option<Stop> {
    STOPS.lock().unwrap_or_else(|e| e.into_inner()).remove(&pid)
}

/// Signal the game and, when it leads its own process group, everything it started.
/// A process that is not a group leader shares Marquee's group, so only it is signalled.
fn signal(identity: &ProcessIdentity, sig: libc::c_int) {
    if !identity.alive() {
        return;
    }
    let pid = identity.pid as libc::pid_t;
    // SAFETY: plain syscalls on a PID whose boot and start time were just verified.
    unsafe {
        if libc::getpgid(pid) == pid {
            libc::kill(-pid, sig);
        } else {
            libc::kill(pid, sig);
        }
    }
}

/// Ask a running game to close (SIGTERM), then kill it if it is still running after
/// `grace`. `force` kills at once. Returns without waiting; the session watcher
/// records the exit.
pub(crate) fn stop_process(identity: ProcessIdentity, force: bool, grace: Duration) -> bool {
    if !identity.alive() {
        return false;
    }
    if force {
        mark_stop(identity.pid, Stop::Forced);
        signal(&identity, libc::SIGKILL);
        return true;
    }
    mark_stop(identity.pid, Stop::Closed);
    signal(&identity, libc::SIGTERM);
    thread::spawn(move || {
        let deadline = Instant::now() + grace;
        while identity.alive() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(100));
        }
        if identity.alive() {
            mark_stop(identity.pid, Stop::Forced);
            signal(&identity, libc::SIGKILL);
        }
    });
    true
}

pub fn recover(
    db: Arc<Mutex<Db>>,
    sink: Arc<dyn EventSink>,
    lease: Arc<crate::instance::InstanceLock>,
) -> Result<()> {
    let rows = db
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .list_sessions()?;
    for row in rows
        .into_iter()
        .filter(|r| r.state == "running" && r.mode != "stream")
    {
        let identity = db
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .session_process(&row.id)?
            .and_then(|s| serde_json::from_str::<ProcessIdentity>(&s).ok());
        if let Some(identity) = identity.filter(|p| p.alive()) {
            let db = db.clone();
            let sink = sink.clone();
            let lease = lease.clone();
            thread::spawn(move || {
                let _lease = lease;
                while identity.alive() {
                    let _ = db
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .checkpoint_session(&row.id, identity.elapsed());
                    thread::sleep(Duration::from_millis(250));
                }
                let note = match take_stop(identity.pid) {
                    Some(Stop::Closed) => "Stopped from Marquee",
                    Some(Stop::Forced) => "Force-stopped from Marquee: it did not close in time",
                    None => "Recovered process ended; exit status unavailable",
                };
                let guard = db.lock().unwrap_or_else(|e| e.into_inner());
                if let Err(err) = guard.complete_session(
                    &row.id,
                    identity.elapsed(),
                    None,
                    "ended",
                    Some(note),
                    true,
                ) {
                    eprintln!("could not finalize recovered session {}: {err}", row.id);
                    return;
                }
                if let Ok(Some(row)) = guard.session_by_id(&row.id) {
                    if let Ok(session) = row_to_session(row) {
                        sink.emit(crate::model::CoreEvent::SessionEnded { session });
                    }
                }
            });
        } else {
            db.lock()
                .unwrap_or_else(|e| e.into_inner())
                .complete_session(
                &row.id,
                row.duration_seconds.unwrap_or(0),
                None,
                "failed",
                Some(
                    "Marquee exited while this session was running; last saved duration recovered",
                ),
                row.duration_seconds.is_some(),
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    use crate::{Core, NoopSink};
    fn seeded() -> (tempfile::TempDir, Core) {
        let t = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(t.path().join("data")), true).unwrap();
        {
            let db = core.db_arc();
            let db = db.lock().unwrap();
            db.conn.execute("INSERT INTO editions(id,system,scan_title,imported_at) VALUES('edition','ps2','Game','2026-01-01')", []).unwrap();
            db.insert_session("session", "edition", "runner.fake", "2026-01-01", "running")
                .unwrap();
        }
        (t, core)
    }
    #[test]
    fn reopen_recovers_live_process_and_credits_it_once() {
        let (_t, core) = seeded();
        let data = core.paths.data_dir.clone();
        let mut child = Command::new("/bin/sleep").arg("2").spawn().unwrap();
        let identity = ProcessIdentity::read(child.id()).unwrap();
        core.db_arc()
            .lock()
            .unwrap()
            .bind_process("session", &serde_json::to_string(&identity).unwrap())
            .unwrap();
        drop(core);
        let core = Core::open_isolated(Some(data.clone()), true).unwrap();
        assert_eq!(
            core.snapshot().unwrap().sessions[0].state,
            SessionState::Running
        );
        assert_eq!(
            core.launch("edition", &[], Arc::new(NoopSink))
                .unwrap_err()
                .code,
            "LAUNCH_BUSY"
        );
        child.wait().unwrap();
        for _ in 0..30 {
            if core.snapshot().unwrap().sessions[0].state != SessionState::Running {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        let snap = core.snapshot().unwrap();
        assert_eq!(snap.sessions[0].state, SessionState::Ended);
        assert!(snap.editions[0].playtime_seconds >= 1);
        assert_eq!(snap.editions[0].play_count, 1);
        drop(core);
        // The recovery monitor has released its instance lease by this point.
        thread::sleep(Duration::from_millis(25));
        let core = Core::open_isolated(Some(data), true).unwrap();
        assert_eq!(core.snapshot().unwrap().editions[0].play_count, 1);
    }
    #[test]
    fn reused_pid_is_rejected_and_only_checkpointed_time_is_credited() {
        let (_t, core) = seeded();
        let data = core.paths.data_dir.clone();
        let mut identity = ProcessIdentity::read(std::process::id()).unwrap();
        identity.ticks = "wrong-start-time".into();
        {
            let db = core.db_arc();
            let db = db.lock().unwrap();
            db.bind_process("session", &serde_json::to_string(&identity).unwrap())
                .unwrap();
            db.checkpoint_session("session", 17).unwrap();
        }
        drop(core);
        let core = Core::open_isolated(Some(data), true).unwrap();
        let snap = core.snapshot().unwrap();
        assert_eq!(snap.sessions[0].state, SessionState::Failed);
        assert_eq!(snap.editions[0].playtime_seconds, 17);
        assert_eq!(snap.editions[0].play_count, 1);
    }
}

#[cfg(test)]
mod stop_tests {
    use super::*;
    use std::io::BufRead;
    use std::os::unix::process::CommandExt;

    fn group(script: &str) -> std::process::Child {
        let mut cmd = Command::new("/bin/sh");
        cmd.args(["-c", script])
            .stdout(Stdio::piped())
            .process_group(0);
        cmd.spawn().unwrap()
    }

    fn gone(pid: u32) -> bool {
        std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .map(|stat| {
                stat.rsplit_once(')')
                    .is_some_and(|(_, rest)| rest.trim_start().starts_with('Z'))
            })
            .unwrap_or(true)
    }

    #[test]
    fn stop_closes_the_whole_group_and_credits_the_session() {
        let mut child = group("sleep 30 & echo $!; wait");
        let mut line = String::new();
        std::io::BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let helper: u32 = line.trim().parse().unwrap();
        let identity = ProcessIdentity::read(child.id()).unwrap();
        let watched_identity = ProcessIdentity::read(child.id()).unwrap();
        assert!(stop_process(identity, false, Duration::from_secs(5)));
        let started = Instant::now();
        let watched = watch_child(child, Some(&watched_identity), true, |_| {});
        assert!(started.elapsed() < Duration::from_secs(3));
        assert_eq!(watched.state, "ended");
        assert_eq!(watched.error.as_deref(), Some("Stopped from Marquee"));
        for _ in 0..40 {
            if gone(helper) {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        assert!(gone(helper), "a process the game started survived Stop");
    }

    #[test]
    fn a_game_that_ignores_close_is_killed_after_the_grace_period() {
        let child = group("trap '' TERM; while :; do sleep 1; done");
        thread::sleep(Duration::from_millis(100));
        let identity = ProcessIdentity::read(child.id()).unwrap();
        let watched_identity = ProcessIdentity::read(child.id()).unwrap();
        let started = Instant::now();
        assert!(stop_process(identity, false, Duration::from_millis(300)));
        let watched = watch_child(child, Some(&watched_identity), true, |_| {});
        assert!(started.elapsed() >= Duration::from_millis(300));
        assert!(started.elapsed() < Duration::from_secs(3));
        assert_eq!(watched.state, "ended");
        assert!(watched.error.unwrap().starts_with("Force-stopped"));
    }

    #[test]
    fn stopping_with_no_game_running_says_so() {
        let t = tempfile::tempdir().unwrap();
        let core = crate::Core::open_isolated(Some(t.path().join("data")), true).unwrap();
        assert_eq!(core.stop_game(false).unwrap_err().code, "NOT_RUNNING");
    }
}
