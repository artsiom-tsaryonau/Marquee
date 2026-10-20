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

fn row_to_session(row: SessionRow) -> Result<Session> {
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
        if db.running_session()?.is_some() {
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
                "this disc format is not launched in this milestone",
            ));
        }
        let runners = runners::snapshot(&db)?;
        // The stored selection is a hint for one emulator family; only honour it
        // when it matches this title's system, so a PS2 pin cannot hijack a PS1
        // launch (and vice versa).
        let selected = db
            .get_setting(&format!("selected_runner.{system}"))?
            .or(db.get_setting("selected_runner_id")?);
        let selected =
            runners::selection_for(&runners, selected.as_deref(), &system).map(str::to_string);
        let profile = db.profile_runner(edition_id)?;
        let profile =
            runners::selection_for(&runners, profile.as_deref(), &system).map(str::to_string);
        let runner_id = profile.or(selected);
        let runner = match runner_id {
            Some(id) => runners
                .iter()
                .find(|r| r.id == id && r.available)
                .cloned()
                .or_else(|| runners::pick_for_system(&runners, &system).cloned()),
            None => runners::pick_for_system(&runners, &system).cloned(),
        }
        .ok_or_else(|| {
            AppError::new(
                "RUNNER_MISSING",
                format!(
                    "no emulator is installed for {system}. See docs/{}.md.",
                    system.to_ascii_uppercase()
                ),
            )
        })?;
        runners::require_launchable(&runner)?;
        let argv = runners::launch_argv(&runner, &media.path, extra_args)?;
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
        };
        let isolate = runners::uses_marquee_prefix(&runner);
        // Pair the game with a BIOS of its own region. Best effort: no match,
        // no settings file yet, or any error simply leaves PCSX2's own choice.
        if isolate && runner.kind == RunnerKind::Pcsx2 {
            if let Some(home) = xdg_config.as_deref() {
                let _ = crate::bios::select_for_pcsx2(home, media.region.as_deref());
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
    _runner_kind: RunnerKind,
    mut child: std::process::Child,
    identity: Option<ProcessIdentity>,
    _lease: Arc<crate::instance::InstanceLock>,
) {
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Err(err) => break Err(err),
            Ok(None) => {
                let seconds = identity
                    .as_ref()
                    .map(|p| p.elapsed())
                    .unwrap_or(start.elapsed().as_secs() as i64);
                let _ = db
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .checkpoint_session(&session_id, seconds);
                thread::sleep(Duration::from_millis(250));
            }
        }
    };
    let duration = identity
        .as_ref()
        .map(|p| p.elapsed())
        .unwrap_or(start.elapsed().as_secs() as i64);
    let (exit_code, state, error) = match status {
        Ok(st) => {
            let code = st.code();
            if st.success() {
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

pub fn require_iso_exists(path: &Path) -> Result<()> {
    if path.is_file() {
        Ok(())
    } else {
        Err(AppError::new(
            "MEDIA_MISSING",
            format!("disc file is missing: {}", path_display(path)),
        ))
    }
}

// Linux process identity includes the boot and kernel start time, so a reused PID
// can never be mistaken for the emulator. Uptime excludes suspend just like Instant.
#[derive(serde::Serialize, serde::Deserialize)]
struct ProcessIdentity {
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
    fn read(pid: u32) -> Option<Self> {
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
    fn alive(&self) -> bool {
        Self::read(self.pid).is_some_and(|p| p.boot == self.boot && p.ticks == self.ticks)
    }
    fn elapsed(&self) -> i64 {
        uptime()
            .map(|now| (now - self.began).max(0.0) as i64)
            .unwrap_or(0)
    }
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
    for row in rows.into_iter().filter(|r| r.state == "running") {
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
                let guard = db.lock().unwrap_or_else(|e| e.into_inner());
                if let Err(err) = guard.complete_session(
                    &row.id,
                    identity.elapsed(),
                    None,
                    "ended",
                    Some("Recovered process ended; exit status unavailable"),
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
