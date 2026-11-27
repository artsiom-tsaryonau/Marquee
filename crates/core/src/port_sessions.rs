//! Play sessions for native ports. They cannot use `sessions`, whose edition
//! foreign key every library launch has to satisfy.

use crate::db::{now_rfc3339, Db};
use crate::error::{AppError, Result};
use crate::history::{self, GameHistory};
use crate::model::{Session, SessionState};
use crate::sessions::{self, EventSink};
use crate::Core;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::{Arc, Mutex};
use std::thread;
use uuid::Uuid;

pub fn ensure_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS port_sessions (
            id TEXT PRIMARY KEY,
            port_id TEXT NOT NULL,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            duration_seconds INTEGER,
            exit_code INTEGER,
            state TEXT NOT NULL,
            process_identity TEXT,
            error TEXT
        );
        CREATE INDEX IF NOT EXISTS port_sessions_by_port
            ON port_sessions(port_id, started_at);",
    )?;
    Ok(())
}

pub fn is_running(conn: &Connection, port_id: &str) -> Result<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM port_sessions WHERE state = 'running' AND port_id = ?1 LIMIT 1",
            [port_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

pub fn has_running(conn: &Connection) -> Result<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM port_sessions WHERE state = 'running' LIMIT 1",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// The live process of the running port session, if any.
pub(crate) fn running_process(conn: &Connection) -> Result<Option<sessions::ProcessIdentity>> {
    Ok(running_rows(conn)?
        .into_iter()
        .filter_map(|row| row.process_identity)
        .find_map(|raw| serde_json::from_str(&raw).ok()))
}

pub fn totals(conn: &Connection, port_id: &str) -> Result<(i64, i64, Option<String>)> {
    conn.query_row(
        "SELECT COALESCE(SUM(duration_seconds), 0), COUNT(*), MAX(ended_at)
         FROM port_sessions WHERE port_id = ?1 AND state = 'ended'",
        [port_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
    .map_err(Into::into)
}

pub fn credited_sessions(conn: &Connection) -> Result<Vec<Session>> {
    Ok(list_conn(conn, None)?.into_iter().map(to_session).collect())
}

pub fn history_for(core: &Core, port_id: &str) -> Result<GameHistory> {
    let rows = core.with_db(|db| list_conn(&db.conn, Some(port_id)))?;
    let now = crate::unix_now();
    let offset = history::local_offset_secs(now);
    let week_seconds = rows
        .iter()
        .filter(|row| row.state == "ended")
        .filter(|row| history::counts_this_week(&row.started_at, now, offset))
        .map(|row| row.duration_seconds.unwrap_or(0).max(0))
        .sum();
    let sessions = rows.into_iter().take(50).map(to_session).collect();
    Ok(GameHistory {
        sessions,
        week_seconds,
        stream_seconds: 0,
    })
}

pub fn launch(core: &Core, id: &str) -> Result<()> {
    core.storage_move_ready()?;
    let _gate = core
        .headless_gate
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    core.storage_move_ready()?;
    if core.headless_active()? {
        return Err(AppError::new(
            "LAUNCH_BUSY",
            "End the streamed game before starting a local game",
        ));
    }
    if core.with_db(|db| db.running_session())?.is_some()
        || core.with_db(|db| has_running(&db.conn))?
    {
        return Err(AppError::new("LAUNCH_BUSY", "a game is already running"));
    }
    if core.list_ports()?.iter().any(|port| {
        port.id == id
            && port
                .job
                .as_ref()
                .is_some_and(|job| job.state == "installing")
    }) {
        return Err(AppError::new(
            "INSTALL_BUSY",
            "Wait for engine installation to finish",
        ));
    }
    let def = crate::ports::find(id)?;
    let installed = core.installed_port(id)?.ok_or_else(|| {
        AppError::new(
            "PORT_NOT_INSTALLED",
            format!("{} is not installed", def.name),
        )
    })?;
    let mut child = crate::ports::launch(&core.paths, &def, &installed)?;
    let session_id = Uuid::new_v4().to_string();
    let started = now_rfc3339();
    core.with_db(|db| insert(&db.conn, &session_id, id, &started))?;
    let identity = sessions::ProcessIdentity::read(child.id());
    if let Some(identity) = identity.as_ref() {
        let raw = serde_json::to_string(identity)?;
        if let Err(error) = core.with_db(|db| bind(&db.conn, &session_id, &raw)) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = core.with_db(|db| {
                finish(
                    &db.conn,
                    &session_id,
                    0,
                    None,
                    "failed",
                    Some(&error.to_string()),
                )
            });
            return Err(error);
        }
    }
    core.events.emit(crate::model::CoreEvent::SessionStarted {
        session: session_event(&session_id, id, &started, None, SessionState::Running, None),
    });
    let db = Arc::clone(&core.db);
    let sink: Arc<dyn EventSink> = core.events.clone();
    let port_id = id.to_string();
    thread::spawn(move || {
        let watched = sessions::watch_child(child, identity.as_ref(), true, |seconds| {
            let guard = db.lock().unwrap_or_else(|error| error.into_inner());
            let _ = checkpoint(&guard.conn, &session_id, seconds);
        });
        let guard = db.lock().unwrap_or_else(|error| error.into_inner());
        if finish(
            &guard.conn,
            &session_id,
            watched.duration,
            watched.exit_code,
            watched.state,
            watched.error.as_deref(),
        )
        .is_err()
        {
            return;
        }
        let state = if watched.state == "ended" {
            SessionState::Ended
        } else {
            SessionState::Failed
        };
        let session = session_event(
            &session_id,
            &port_id,
            &started,
            Some(watched.duration),
            state,
            watched.error,
        );
        if watched.state == "ended" {
            sink.emit(crate::model::CoreEvent::SessionEnded { session });
        } else {
            sink.emit(crate::model::CoreEvent::SessionFailed { session });
        }
    });
    Ok(())
}

pub fn recover(db: &Arc<Mutex<Db>>, sink: Arc<dyn EventSink>) -> Result<()> {
    let rows = {
        let guard = db.lock().unwrap_or_else(|error| error.into_inner());
        running_rows(&guard.conn)?
    };
    for row in rows {
        let identity = row.process_identity.as_deref().and_then(|raw| {
            serde_json::from_str::<sessions::ProcessIdentity>(raw)
                .ok()
                .filter(|identity| identity.alive())
        });
        if let Some(identity) = identity {
            let db = Arc::clone(db);
            let sink = sink.clone();
            let id = row.id.clone();
            let port_id = row.port_id.clone();
            let started = row.started_at.clone();
            thread::spawn(move || {
                while identity.alive() {
                    let guard = db.lock().unwrap_or_else(|error| error.into_inner());
                    let _ = checkpoint(&guard.conn, &id, identity.elapsed());
                    drop(guard);
                    thread::sleep(std::time::Duration::from_millis(250));
                }
                sessions::take_stop(identity.pid());
                let guard = db.lock().unwrap_or_else(|error| error.into_inner());
                let duration = identity.elapsed();
                if finish(&guard.conn, &id, duration, None, "ended", None).is_err() {
                    return;
                }
                sink.emit(crate::model::CoreEvent::SessionEnded {
                    session: session_event(
                        &id,
                        &port_id,
                        &started,
                        Some(duration),
                        SessionState::Ended,
                        None,
                    ),
                });
            });
        } else {
            let guard = db.lock().unwrap_or_else(|error| error.into_inner());
            finish(
                &guard.conn,
                &row.id,
                row.duration_seconds.unwrap_or(0),
                None,
                "interrupted",
                Some("Marquee exited while this port was running"),
            )?;
        }
    }
    Ok(())
}

struct Row {
    id: String,
    port_id: String,
    started_at: String,
    ended_at: Option<String>,
    exit_code: Option<i32>,
    duration_seconds: Option<i64>,
    state: String,
    error: Option<String>,
    process_identity: Option<String>,
}

fn insert(conn: &Connection, id: &str, port_id: &str, started_at: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO port_sessions(id, port_id, started_at, state) VALUES(?1, ?2, ?3, 'running')",
        params![id, port_id, started_at],
    )?;
    Ok(())
}

fn bind(conn: &Connection, id: &str, identity: &str) -> Result<()> {
    conn.execute(
        "UPDATE port_sessions SET process_identity = ?2 WHERE id = ?1 AND state = 'running'",
        params![id, identity],
    )?;
    Ok(())
}

fn checkpoint(conn: &Connection, id: &str, seconds: i64) -> Result<()> {
    conn.execute(
        "UPDATE port_sessions SET duration_seconds = ?2 WHERE id = ?1 AND state = 'running'",
        params![id, seconds.max(0)],
    )?;
    Ok(())
}

fn finish(
    conn: &Connection,
    id: &str,
    seconds: i64,
    exit_code: Option<i32>,
    state: &str,
    error: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE port_sessions
         SET ended_at = ?2, duration_seconds = ?3, exit_code = ?4, state = ?5, error = ?6
         WHERE id = ?1 AND state = 'running'",
        params![id, now_rfc3339(), seconds.max(0), exit_code, state, error],
    )?;
    Ok(())
}

fn running_rows(conn: &Connection) -> Result<Vec<Row>> {
    let mut statement = conn.prepare(
        "SELECT id, port_id, started_at, duration_seconds, state, error, process_identity, ended_at, exit_code
         FROM port_sessions WHERE state = 'running'",
    )?;
    let rows = statement.query_map([], row_from)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn list_conn(conn: &Connection, port_id: Option<&str>) -> Result<Vec<Row>> {
    let mut statement = conn.prepare(
        "SELECT id, port_id, started_at, duration_seconds, state, error, process_identity, ended_at, exit_code
         FROM port_sessions
         WHERE (?1 IS NULL OR port_id = ?1)
         ORDER BY started_at DESC",
    )?;
    let rows = statement.query_map(params![port_id], row_from)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn row_from(row: &rusqlite::Row<'_>) -> rusqlite::Result<Row> {
    Ok(Row {
        id: row.get(0)?,
        port_id: row.get(1)?,
        started_at: row.get(2)?,
        duration_seconds: row.get(3)?,
        state: row.get(4)?,
        error: row.get(5)?,
        process_identity: row.get(6)?,
        ended_at: row.get(7)?,
        exit_code: row.get(8)?,
    })
}

fn to_session(row: Row) -> Session {
    let (ended_at, exit_code) = (row.ended_at.clone(), row.exit_code);
    let mut session = session_event(
        &row.id,
        &row.port_id,
        &row.started_at,
        row.duration_seconds,
        match row.state.as_str() {
            "running" => SessionState::Running,
            "ended" => SessionState::Ended,
            _ => SessionState::Failed,
        },
        row.error,
    );
    session.ended_at = ended_at;
    session.exit_code = exit_code;
    session
}

fn session_event(
    id: &str,
    port_id: &str,
    started_at: &str,
    duration_seconds: Option<i64>,
    state: SessionState,
    error: Option<String>,
) -> Session {
    Session {
        id: id.to_string(),
        edition_id: String::new(),
        runner_id: format!("port.{port_id}"),
        started_at: started_at.to_string(),
        ended_at: None,
        duration_seconds,
        exit_code: None,
        state,
        error,
        port_id: Some(port_id.to_string()),
        mode: "local".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::InstalledPort;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn install_script(core: &Core, name: &str, body: &str) {
        let dir = core.paths.data_dir.join("ports/shipwright");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        fs::write(&path, body).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        let installed = InstalledPort {
            tag: "9.3.1".into(),
            bin: name.into(),
        };
        core.with_db(|db| {
            db.set_setting(
                &crate::ports::setting_key("shipwright"),
                &serde_json::to_string(&installed).unwrap(),
            )
        })
        .unwrap();
    }

    #[test]
    fn a_running_port_blocks_another_launch_and_a_short_exit_is_not_playtime() {
        let temp = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(temp.path().join("data")), true).unwrap();
        install_script(
            &core,
            "hold.sh",
            "#!/bin/sh\nwhile [ ! -f stop ]; do sleep 0.05; done\nexit 0\n",
        );
        launch(&core, "shipwright").unwrap();
        assert_eq!(launch(&core, "shipwright").unwrap_err().code, "LAUNCH_BUSY");
        assert_eq!(
            core.launch("missing", &[], Arc::new(crate::NoopSink))
                .unwrap_err()
                .code,
            "LAUNCH_BUSY"
        );
        fs::write(core.paths.data_dir.join("ports/shipwright/stop"), b"").unwrap();
        for _ in 0..40 {
            let history = history_for(&core, "shipwright").unwrap();
            if history
                .sessions
                .first()
                .is_some_and(|session| session.state != SessionState::Running)
            {
                assert_eq!(history.sessions[0].state, SessionState::Failed);
                assert_eq!(played(&core), (0, 0));
                return;
            }
            thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("held port session did not finish");
    }

    #[test]
    fn a_restart_interrupts_a_session_whose_process_is_gone() {
        let temp = tempfile::tempdir().unwrap();
        let data = temp.path().join("data");
        let core = Core::open_isolated(Some(data.clone()), true).unwrap();
        core.with_db(|db| insert(&db.conn, "gone", "shipwright", "2026-11-26T08:00:00Z"))
            .unwrap();
        drop(core);
        let core = Core::open_isolated(Some(data), true).unwrap();
        let history = history_for(&core, "shipwright").unwrap();
        assert_eq!(history.sessions[0].state, SessionState::Failed);
        assert_eq!(played(&core), (0, 0));
    }

    #[test]
    fn eleven_seconds_of_play_is_credited_once() {
        let temp = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(temp.path().join("data")), true).unwrap();
        install_script(&core, "long.sh", "#!/bin/sh\nsleep 11\n");
        launch(&core, "shipwright").unwrap();
        for _ in 0..80 {
            let (seconds, count) = played(&core);
            if count == 1 {
                assert!(seconds >= 10, "{seconds}");
                let history = history_for(&core, "shipwright").unwrap();
                assert!(history.week_seconds >= 10);
                assert!(history.sessions[0].ended_at.is_some());
                assert_eq!(history.sessions[0].exit_code, Some(0));
                return;
            }
            thread::sleep(std::time::Duration::from_millis(250));
        }
        panic!("port session was not credited");
    }

    fn played(core: &Core) -> (i64, i64) {
        let (seconds, count, _) = core.with_db(|db| totals(&db.conn, "shipwright")).unwrap();
        (seconds, count)
    }

    #[test]
    fn home_week_total_includes_ended_port_play_and_install_waits_until_the_port_exits() {
        let temp = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(temp.path().join("data")), true).unwrap();
        let started = now_rfc3339();
        core.with_db(|db| {
            insert(&db.conn, "played", "shipwright", &started)?;
            finish(&db.conn, "played", 90, Some(0), "ended", None)
        })
        .unwrap();
        assert_eq!(core.play_summary().unwrap().week_seconds, 90);
        core.with_db(|db| insert(&db.conn, "run", "shipwright", &started))
            .unwrap();
        assert_eq!(
            core.start_port_install("shipwright").unwrap_err().code,
            "LAUNCH_BUSY"
        );
    }
}
