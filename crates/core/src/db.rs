use crate::error::Result;
use crate::model::{BiosState, Settings, SCHEMA_VERSION};
use crate::paths::{path_bytes, path_from_bytes, Paths};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;

pub struct Db {
    pub conn: Connection,
}

impl Db {
    pub fn open(paths: &Paths) -> Result<Self> {
        let conn = Connection::open(paths.db_path())?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        let version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version == 0 {
            self.conn.execute_batch(
                "
                CREATE TABLE library_roots (
                    id TEXT PRIMARY KEY,
                    path BLOB NOT NULL UNIQUE,
                    kind TEXT NOT NULL,
                    label TEXT,
                    last_scan_at TEXT
                );
                CREATE TABLE editions (
                    id TEXT PRIMARY KEY,
                    system TEXT NOT NULL,
                    scan_title TEXT NOT NULL,
                    display_title TEXT,
                    favorite INTEGER NOT NULL DEFAULT 0,
                    media_id TEXT,
                    last_played_at TEXT,
                    playtime_seconds INTEGER NOT NULL DEFAULT 0,
                    play_count INTEGER NOT NULL DEFAULT 0,
                    imported_at TEXT NOT NULL
                );
                CREATE TABLE media (
                    id TEXT PRIMARY KEY,
                    edition_id TEXT NOT NULL REFERENCES editions(id),
                    kind TEXT NOT NULL,
                    path BLOB NOT NULL UNIQUE,
                    size_bytes INTEGER NOT NULL,
                    mtime INTEGER NOT NULL,
                    serial TEXT,
                    serial_raw TEXT,
                    region TEXT,
                    skip_reason TEXT
                );
                CREATE TABLE runners (
                    id TEXT PRIMARY KEY,
                    kind TEXT NOT NULL,
                    display_name TEXT NOT NULL,
                    command_json TEXT NOT NULL,
                    bios_state TEXT NOT NULL,
                    bios_hint TEXT,
                    source TEXT NOT NULL
                );
                CREATE TABLE launch_profiles (
                    edition_id TEXT PRIMARY KEY REFERENCES editions(id),
                    runner_id TEXT NOT NULL REFERENCES runners(id),
                    extra_args_json TEXT NOT NULL DEFAULT '[]'
                );
                CREATE TABLE sessions (
                    id TEXT PRIMARY KEY,
                    edition_id TEXT NOT NULL REFERENCES editions(id),
                    runner_id TEXT NOT NULL REFERENCES runners(id),
                    started_at TEXT NOT NULL,
                    ended_at TEXT,
                    duration_seconds INTEGER,
                    exit_code INTEGER,
                    state TEXT NOT NULL,
                    error TEXT
                );
                CREATE TABLE settings (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );
                CREATE TABLE artwork (
                    edition_id TEXT NOT NULL,
                    kind TEXT NOT NULL,
                    path TEXT NOT NULL,
                    source TEXT NOT NULL,
                    fetched_at TEXT NOT NULL,
                    PRIMARY KEY (edition_id, kind)
                );
                ",
            )?;
            self.conn.pragma_update(None, "user_version", 2)?;
        } else if version == 1 {
            self.conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS artwork (
                    edition_id TEXT NOT NULL,
                    kind TEXT NOT NULL,
                    path TEXT NOT NULL,
                    source TEXT NOT NULL,
                    fetched_at TEXT NOT NULL,
                    PRIMARY KEY (edition_id, kind)
                );
                ",
            )?;
            self.conn.pragma_update(None, "user_version", 2)?;
        }
        let version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version == 2 && !column_exists(&self.conn, "sessions", "process_identity")? {
            self.conn.execute_batch(
                "BEGIN IMMEDIATE;
                ALTER TABLE sessions ADD COLUMN process_identity TEXT;
                DELETE FROM launch_profiles;
                PRAGMA user_version = 3;
                COMMIT;",
            )?;
        }
        let version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version == 3 && !column_exists(&self.conn, "library_roots", "system")? {
            self.conn.execute_batch(
                "BEGIN IMMEDIATE;
                ALTER TABLE library_roots ADD COLUMN system TEXT NOT NULL DEFAULT 'ps2';
                PRAGMA user_version = 4;
                COMMIT;",
            )?;
        }
        let version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version == 4 {
            self.conn.execute_batch(
                "BEGIN IMMEDIATE;
                CREATE TABLE catalogue_records (id TEXT PRIMARY KEY, payload TEXT NOT NULL);
                CREATE TABLE catalogue_systems (id TEXT PRIMARY KEY, payload TEXT NOT NULL);
                CREATE TABLE catalogue_links (
                    edition_id TEXT PRIMARY KEY REFERENCES editions(id),
                    record_id TEXT NOT NULL REFERENCES catalogue_records(id),
                    media_id TEXT NOT NULL,
                    local_media_id TEXT NOT NULL,
                    size_bytes INTEGER NOT NULL,
                    mtime INTEGER NOT NULL,
                    method TEXT NOT NULL
                    ,source_hash TEXT NOT NULL DEFAULT ''
                );
                PRAGMA user_version = 5;
                COMMIT;",
            )?;
        }
        let version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version == 5 {
            self.conn.execute_batch(
                "BEGIN IMMEDIATE;
                CREATE TABLE IF NOT EXISTS preparation_receipts (
                    id TEXT PRIMARY KEY, payload TEXT NOT NULL, edition_ids TEXT NOT NULL
                );
                PRAGMA user_version=6; COMMIT;",
            )?;
        }
        crate::romfacts::ensure_table(&self.conn)?;
        crate::wanted::ensure_table(&self.conn)?;
        if version > SCHEMA_VERSION {
            return Err(crate::error::AppError::new(
                "DB",
                format!("library schema {version} is newer than this Marquee ({SCHEMA_VERSION})"),
            ));
        }
        Ok(())
    }

    pub fn session_process(&self, id: &str) -> Result<Option<String>> {
        Ok(self.conn.query_row(
            "SELECT process_identity FROM sessions WHERE id=?1",
            [id],
            |r| r.get(0),
        )?)
    }

    pub fn bind_process(&self, id: &str, identity: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET process_identity=?2 WHERE id=?1 AND state='running'",
            params![id, identity],
        )?;
        Ok(())
    }

    pub fn checkpoint_session(&self, id: &str, seconds: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET duration_seconds=?2 WHERE id=?1 AND state='running'",
            params![id, seconds],
        )?;
        Ok(())
    }

    /// End and credit a session atomically, once, including time spent before a crash.
    pub fn complete_session(
        &self,
        id: &str,
        seconds: i64,
        code: Option<i32>,
        state: &str,
        error: Option<&str>,
        credit: bool,
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let ended = now_rfc3339();
        let changed = tx.execute("UPDATE sessions SET ended_at=?2, duration_seconds=?3, exit_code=?4, state=?5, error=?6 WHERE id=?1 AND state='running'", params![id, ended, seconds.max(0), code, state, error])?;
        if changed > 0 && credit {
            tx.execute("UPDATE editions SET playtime_seconds=playtime_seconds+?2, play_count=play_count+1, last_played_at=?3 WHERE id=(SELECT edition_id FROM sessions WHERE id=?1)", params![id, seconds.max(0), ended])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn load_settings(&self) -> Result<Settings> {
        Ok(Settings {
            last_root: self.get_setting("last_root")?.map(PathBuf::from),
            last_collection: self.get_setting("last_collection")?,
            last_focus: self.get_setting("last_focus")?,
            selected_runner_id: self.get_setting("selected_runner_id")?,
            display_mode: match self.get_setting("display_mode")?.as_deref() {
                Some("fullscreen") => "fullscreen".into(),
                _ => "window".into(),
            },
            torrents: crate::model::TorrentSettings {
                trackers: self.get_setting("torrent.trackers")?.as_deref() == Some("1"),
                dht: self.get_setting("torrent.dht")?.as_deref() == Some("1"),
                enabled: self.get_setting("torrent.enabled")?.as_deref() == Some("1"),
                seeding: self.get_setting("torrent.seeding")?.as_deref() != Some("0"),
                upload_kbps: self
                    .get_setting("torrent.upload_kbps")?
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0),
                download_kbps: self
                    .get_setting("torrent.download_kbps")?
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0),
                // Manager-owned; the legacy library database never stored a proxy.
                proxy: None,
            },
            selected_runner_ids: ["ps1", "ps2"]
                .into_iter()
                .filter_map(
                    |system| match self.get_setting(&format!("selected_runner.{system}")) {
                        Ok(Some(id)) => Some(Ok((system.to_string(), id))),
                        Ok(None) => None,
                        Err(e) => Some(Err(e)),
                    },
                )
                .collect::<Result<_>>()?,
        })
    }

    pub fn upsert_root(
        &self,
        id: &str,
        path: &std::path::Path,
        kind: &str,
        system: &str,
        label: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO library_roots(id, path, kind, system, label)
             VALUES(?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(path) DO UPDATE SET
                label = COALESCE(excluded.label, library_roots.label),
                system = excluded.system",
            params![id, path_bytes(path), kind, system, label],
        )?;
        Ok(())
    }

    pub fn root_by_path(&self, path: &std::path::Path) -> Result<Option<(String, Vec<u8>)>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, path FROM library_roots WHERE path = ?1",
                [path_bytes(path)],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    pub fn update_scan_time(&self, root_id: &str, when: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE library_roots SET last_scan_at = ?2 WHERE id = ?1",
            params![root_id, when],
        )?;
        Ok(())
    }

    pub fn list_roots(&self) -> Result<Vec<RootRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, kind, label, last_scan_at, system FROM library_roots ORDER BY label, id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(RootRow {
                id: r.get(0)?,
                path: path_from_bytes(&r.get::<_, Vec<u8>>(1)?),
                kind: r.get(2)?,
                label: r.get(3)?,
                last_scan_at: r.get(4)?,
                system: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn upsert_edition(
        &self,
        id: &str,
        system: &str,
        scan_title: &str,
        media_id: Option<&str>,
        imported_at: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO editions(id, system, scan_title, media_id, imported_at)
             VALUES(?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                scan_title = excluded.scan_title,
                media_id = excluded.media_id",
            params![id, system, scan_title, media_id, imported_at],
        )?;
        Ok(())
    }

    pub fn set_display_title(&self, id: &str, title: Option<&str>) -> Result<usize> {
        Ok(self.conn.execute(
            "UPDATE editions SET display_title = ?2 WHERE id = ?1",
            params![id, title],
        )?)
    }

    pub fn set_favourite(&self, id: &str, favorite: bool) -> Result<usize> {
        Ok(self.conn.execute(
            "UPDATE editions SET favorite = ?2 WHERE id = ?1",
            params![id, favorite as i64],
        )?)
    }

    pub fn edition_by_id(&self, id: &str) -> Result<Option<EditionRow>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, system, scan_title, display_title, favorite, media_id, last_played_at, playtime_seconds, play_count, imported_at
                 FROM editions WHERE id = ?1",
                [id],
                edition_from_row,
            )
            .optional()?)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn upsert_media(
        &self,
        id: &str,
        edition_id: &str,
        kind: &str,
        path: &std::path::Path,
        size_bytes: i64,
        mtime: i64,
        serial: Option<&str>,
        serial_raw: Option<&str>,
        region: Option<&str>,
        skip_reason: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO media(id, edition_id, kind, path, size_bytes, mtime, serial, serial_raw, region, skip_reason)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(path) DO UPDATE SET
                kind = excluded.kind,
                size_bytes = excluded.size_bytes,
                mtime = excluded.mtime,
                serial = excluded.serial,
                serial_raw = excluded.serial_raw,
                region = excluded.region,
                skip_reason = excluded.skip_reason,
                edition_id = excluded.edition_id",
            params![
                id,
                edition_id,
                kind,
                path_bytes(path),
                size_bytes,
                mtime,
                serial,
                serial_raw,
                region,
                skip_reason
            ],
        )?;
        Ok(())
    }

    pub fn media_by_path(&self, path: &std::path::Path) -> Result<Option<MediaKey>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, edition_id, kind, size_bytes, mtime, serial FROM media WHERE path = ?1",
                [path_bytes(path)],
                |r| {
                    Ok(MediaKey {
                        id: r.get(0)?,
                        edition_id: r.get(1)?,
                        kind: r.get(2)?,
                        size_bytes: r.get(3)?,
                        mtime: r.get(4)?,
                        serial: r.get(5)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn list_media(&self) -> Result<Vec<MediaRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, edition_id, kind, path, size_bytes, mtime, serial, serial_raw, region, skip_reason FROM media",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(MediaRow {
                id: r.get(0)?,
                edition_id: r.get(1)?,
                kind: r.get(2)?,
                path: path_from_bytes(&r.get::<_, Vec<u8>>(3)?),
                size_bytes: r.get(4)?,
                mtime: r.get(5)?,
                serial: r.get(6)?,
                serial_raw: r.get(7)?,
                region: r.get(8)?,
                skip_reason: r.get(9)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn list_editions(&self) -> Result<Vec<EditionRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, system, scan_title, display_title, favorite, media_id, last_played_at, playtime_seconds, play_count, imported_at
             FROM editions ORDER BY COALESCE(display_title, scan_title)",
        )?;
        let rows = stmt.query_map([], edition_from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn upsert_runner(
        &self,
        id: &str,
        kind: &str,
        display_name: &str,
        command: &[String],
        bios_state: BiosState,
        bios_hint: Option<&str>,
        source: &str,
    ) -> Result<()> {
        let command_json = serde_json::to_string(command)?;
        self.conn.execute(
            "INSERT INTO runners(id, kind, display_name, command_json, bios_state, bios_hint, source)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                display_name = excluded.display_name,
                command_json = excluded.command_json,
                bios_state = CASE
                    WHEN runners.bios_state = 'launch_tested' THEN runners.bios_state
                    ELSE excluded.bios_state
                END,
                bios_hint = excluded.bios_hint,
                source = excluded.source",
            params![
                id,
                kind,
                display_name,
                command_json,
                bios_state.as_str(),
                bios_hint,
                source
            ],
        )?;
        Ok(())
    }

    /// Remove an edition and its rows, but only if nothing about it is worth keeping:
    /// never played, not a favourite, no custom title. Returns whether it was removed.
    pub fn delete_unplayed_edition(&self, id: &str) -> Result<bool> {
        let keep: i64 = self.conn.query_row(
            "SELECT (SELECT COUNT(*) FROM sessions WHERE edition_id = ?1)
                  + (SELECT COUNT(*) FROM editions WHERE id = ?1
                       AND (favorite != 0 OR display_title IS NOT NULL OR play_count > 0))",
            [id],
            |r| r.get(0),
        )?;
        if keep > 0 {
            return Ok(false);
        }
        for sql in [
            "DELETE FROM artwork WHERE edition_id = ?1",
            "DELETE FROM catalogue_links WHERE edition_id = ?1",
            "DELETE FROM launch_profiles WHERE edition_id = ?1",
            "UPDATE editions SET media_id = NULL WHERE id = ?1",
            "DELETE FROM media WHERE edition_id = ?1",
            "DELETE FROM editions WHERE id = ?1",
        ] {
            self.conn.execute(sql, [id])?;
        }
        Ok(true)
    }

    pub fn delete_runner(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM runners WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn list_runners(&self) -> Result<Vec<RunnerRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, display_name, command_json, bios_state, bios_hint, source FROM runners",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(RunnerRow {
                id: r.get(0)?,
                kind: r.get(1)?,
                display_name: r.get(2)?,
                command_json: r.get(3)?,
                bios_state: r.get(4)?,
                bios_hint: r.get(5)?,
                source: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn set_bios_state(&self, runner_id: &str, state: BiosState) -> Result<()> {
        self.conn.execute(
            "UPDATE runners SET bios_state = ?2 WHERE id = ?1",
            params![runner_id, state.as_str()],
        )?;
        Ok(())
    }

    pub fn insert_session(
        &self,
        id: &str,
        edition_id: &str,
        runner_id: &str,
        started_at: &str,
        state: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions(id, edition_id, runner_id, started_at, state)
             VALUES(?1, ?2, ?3, ?4, ?5)",
            params![id, edition_id, runner_id, started_at, state],
        )?;
        Ok(())
    }

    pub fn finish_session(
        &self,
        id: &str,
        ended_at: &str,
        duration_seconds: i64,
        exit_code: Option<i32>,
        state: &str,
        error: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET ended_at = ?2, duration_seconds = ?3, exit_code = ?4, state = ?5, error = ?6 WHERE id = ?1",
            params![id, ended_at, duration_seconds, exit_code, state, error],
        )?;
        Ok(())
    }

    pub fn running_session(&self) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id FROM sessions WHERE state = 'running' LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn list_sessions(&self) -> Result<Vec<SessionRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, edition_id, runner_id, started_at, ended_at, duration_seconds, exit_code, state, error
             FROM sessions ORDER BY started_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(SessionRow {
                id: r.get(0)?,
                edition_id: r.get(1)?,
                runner_id: r.get(2)?,
                started_at: r.get(3)?,
                ended_at: r.get(4)?,
                duration_seconds: r.get(5)?,
                exit_code: r.get(6)?,
                state: r.get(7)?,
                error: r.get(8)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn session_by_id(&self, id: &str) -> Result<Option<SessionRow>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, edition_id, runner_id, started_at, ended_at, duration_seconds, exit_code, state, error
                 FROM sessions WHERE id = ?1",
                [id],
                |r| {
                    Ok(SessionRow {
                        id: r.get(0)?,
                        edition_id: r.get(1)?,
                        runner_id: r.get(2)?,
                        started_at: r.get(3)?,
                        ended_at: r.get(4)?,
                        duration_seconds: r.get(5)?,
                        exit_code: r.get(6)?,
                        state: r.get(7)?,
                        error: r.get(8)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn set_profile(&self, edition_id: &str, runner_id: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO launch_profiles(edition_id, runner_id) VALUES(?1, ?2)
             ON CONFLICT(edition_id) DO UPDATE SET runner_id = excluded.runner_id",
            params![edition_id, runner_id],
        )?;
        Ok(())
    }

    pub fn profile_runner(&self, edition_id: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT runner_id FROM launch_profiles WHERE edition_id = ?1",
                [edition_id],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn upsert_artwork(
        &self,
        edition_id: &str,
        kind: &str,
        path: &str,
        source: &str,
        fetched_at: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO artwork(edition_id, kind, path, source, fetched_at)
             VALUES(?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(edition_id, kind) DO UPDATE SET
                path = excluded.path,
                source = excluded.source,
                fetched_at = excluded.fetched_at",
            params![edition_id, kind, path, source, fetched_at],
        )?;
        Ok(())
    }

    pub fn artwork_path(&self, edition_id: &str, kind: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT path FROM artwork WHERE edition_id = ?1 AND kind = ?2",
                params![edition_id, kind],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn list_cover_paths(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT edition_id, path FROM artwork WHERE kind = 'cover'")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

/// True when `table` already has `column`. Used to keep migrations idempotent:
/// the v0 schema is created in full, so a database stamped v2 already carries
/// columns that a later step would otherwise try to add again.
fn column_exists(conn: &rusqlite::Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        if row.get::<_, String>(1)? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

pub struct RootRow {
    pub id: String,
    pub path: PathBuf,
    pub kind: String,
    pub label: Option<String>,
    pub last_scan_at: Option<String>,
    pub system: String,
}

pub struct MediaKey {
    pub id: String,
    pub edition_id: String,
    pub kind: String,
    pub size_bytes: i64,
    pub mtime: i64,
    pub serial: Option<String>,
}

pub struct MediaRow {
    pub id: String,
    pub edition_id: String,
    pub kind: String,
    pub path: PathBuf,
    pub size_bytes: i64,
    pub mtime: i64,
    pub serial: Option<String>,
    pub serial_raw: Option<String>,
    pub region: Option<String>,
    pub skip_reason: Option<String>,
}

pub struct EditionRow {
    pub id: String,
    pub system: String,
    pub scan_title: String,
    pub display_title: Option<String>,
    pub favorite: bool,
    pub media_id: Option<String>,
    pub last_played_at: Option<String>,
    pub playtime_seconds: i64,
    pub play_count: i64,
    pub imported_at: String,
}

fn edition_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<EditionRow> {
    Ok(EditionRow {
        id: r.get(0)?,
        system: r.get(1)?,
        scan_title: r.get(2)?,
        display_title: r.get(3)?,
        favorite: r.get::<_, i64>(4)? != 0,
        media_id: r.get(5)?,
        last_played_at: r.get(6)?,
        playtime_seconds: r.get(7)?,
        play_count: r.get(8)?,
        imported_at: r.get(9)?,
    })
}

pub struct RunnerRow {
    pub id: String,
    pub kind: String,
    pub display_name: String,
    pub command_json: String,
    pub bios_state: String,
    pub bios_hint: Option<String>,
    pub source: String,
}

pub struct SessionRow {
    pub id: String,
    pub edition_id: String,
    pub runner_id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_seconds: Option<i64>,
    pub exit_code: Option<i32>,
    pub state: String,
    pub error: Option<String>,
}

pub use marquee_media::time::{format_unix, now_rfc3339};
