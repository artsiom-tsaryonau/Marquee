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
use std::time::Instant;
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
) -> Result<Session> {
    let (session, argv, runner_kind, isolate) = {
        let db = db.lock().unwrap_or_else(|e| e.into_inner());
        if db.running_session()?.is_some() {
            return Err(AppError::new("LAUNCH_BUSY", "a game is already running"));
        }
        db.edition_by_id(edition_id)?
            .ok_or_else(|| AppError::new("MEDIA_MISSING", "title is not in the library"))?;
        let media = library::find_media_for_edition(&db, edition_id)?
            .ok_or_else(|| AppError::new("MEDIA_MISSING", "no disc is attached to this title"))?;
        if !media.available {
            return Err(AppError::new(
                "MEDIA_MISSING",
                format!("disc file is missing: {}", path_display(&media.path)),
            )
            .with_context(json!({ "path": path_display(&media.path) })));
        }
        if media.skip_reason.is_some() || !media.kind.is_supported() {
            return Err(AppError::new(
                "MEDIA_MISSING",
                "this disc format is not launched in this milestone (.iso only)",
            ));
        }
        let runners = runners::snapshot(&db)?;
        let selected = db.get_setting("selected_runner_id")?;
        let runner_id = db.profile_runner(edition_id)?.or(selected);
        let runner = match runner_id {
            Some(id) => runners
                .iter()
                .find(|r| r.id == id)
                .cloned()
                .or_else(|| runners::pick_default(&runners).cloned()),
            None => runners::pick_default(&runners).cloned(),
        }
        .ok_or_else(|| {
            AppError::new("RUNNER_MISSING", "PCSX2 is not installed. See docs/PS2.md.")
        })?;
        runners::require_launchable(&runner)?;
        let argv = runners::launch_argv(&runner, &media.path, extra_args)?;
        let session_id = Uuid::new_v4().to_string();
        let started = now_rfc3339();
        db.insert_session(&session_id, edition_id, &runner.id, &started, "running")?;
        db.set_profile(edition_id, &runner.id)?;
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
        (session, argv, runner.kind, isolate)
    };

    sink.emit(crate::model::CoreEvent::SessionStarted {
        session: session.clone(),
    });

    let child = spawn(&argv, if isolate { xdg_config.as_deref() } else { None });
    match child {
        Ok(child) => {
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

fn wait_and_finish(
    db: Arc<Mutex<Db>>,
    sink: Arc<dyn EventSink>,
    session_id: String,
    edition_id: String,
    runner_id: String,
    runner_kind: RunnerKind,
    mut child: std::process::Child,
) {
    let start = Instant::now();
    let status = child.wait();
    let duration = start.elapsed().as_secs() as i64;
    let ended = now_rfc3339();
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
        let _ = guard.finish_session(
            &session_id,
            &ended,
            duration,
            exit_code,
            state,
            error.as_deref(),
        );
        if state == "ended" {
            let _ = guard.add_playtime(&edition_id, duration.max(0), &ended);
            if runner_kind == RunnerKind::Pcsx2 {
                let _ = runners::mark_launch_tested(&guard, &runner_id);
            }
        }
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
