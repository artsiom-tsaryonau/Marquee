//! One-time M1 ownership cutover, before the daemon starts. At runtime only the
//! daemon writes jobs.db. Version 2 makes old in-process M1 writers refuse it.
use crate::endpoint::Lock;
use crate::{AppError, Result};
use rusqlite::{Connection, OpenFlags};
use std::fs::{self, File};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

pub fn adopt_m1(data: &Path, library_owned: bool) -> Result<()> {
    let path = data.join("manager/jobs.db");
    if !path.exists() {
        return Ok(());
    }
    if !fs::symlink_metadata(&path)?.is_file() {
        return Err(AppError::new(
            "PREPARATION_UNSAFE",
            "Manager database is not a regular file",
        ));
    }
    let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    drop(conn);
    if version != 1 {
        return Ok(());
    }
    let _manager = Lock::acquire(data)?;
    let _library = if library_owned {
        None
    } else {
        let file = File::options()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(data.join("instance.lock"))?;
        // SAFETY: this descriptor is held until cutover finishes.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(AppError::new("MANAGER_MIGRATION_BUSY", "Close the older Marquee before adopting its M1 jobs, or open the new launcher first"));
        }
        Some(file)
    };
    let conn = Connection::open(&path)?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version != 1 {
        return Ok(());
    }
    let backup = data
        .join("manager")
        .join(format!("jobs-m1-backup-{}.db", uuid::Uuid::new_v4()));
    conn.execute("VACUUM INTO ?1", [backup.to_string_lossy().as_ref()])?;
    fs::set_permissions(backup, fs::Permissions::from_mode(0o600))?;
    conn.execute_batch("BEGIN IMMEDIATE; PRAGMA user_version=2; COMMIT;")?;
    Ok(())
}

/// Torrent cutover runs in the client while it holds (or acquires) the library
/// lease. The daemon never opens library.db, and cannot start an engine until
/// the legacy writer has been retired by the final tombstone.
pub fn adopt_torrents(data: &Path, library_owned: bool) -> Result<()> {
    let dir = data.join("manager/torrents");
    crate::endpoint::private_directory(&dir)?;
    let lock = File::options()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(dir.join("adoption.lock"))?;
    // SAFETY: this descriptor remains open throughout the cutover.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        // SAFETY: this descriptor is held throughout the bounded wait/cutover.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            break;
        }
        if std::time::Instant::now() >= deadline {
            return Err(AppError::new(
                "MANAGER_MIGRATION_BUSY",
                "Another torrent cutover is in progress; retry shortly",
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    if torrent_cutover_complete(data)? {
        return Ok(());
    }
    let _library = if library_owned {
        None
    } else {
        let file = File::options()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(data.join("instance.lock"))?;
        // SAFETY: the file owns this descriptor until migration finishes.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(AppError::new("MANAGER_MIGRATION_BUSY", "Close the older Marquee before adopting its torrents, or open the new launcher first"));
        }
        Some(file)
    };
    let legacy = data.join("torrents/entries.json");
    let bytes = bounded_file(&legacy, 8 * 1024 * 1024)?.unwrap_or_else(|| b"[]".to_vec());
    crate::torrent::validate_legacy(data, &bytes)?;
    let settings = legacy_torrent_settings(data)?;
    let journal = dir.join("legacy-backup.json");
    let backup = serde_json::json!({"version":1,"entries":String::from_utf8(bytes.clone()).map_err(|_| AppError::new("TORRENT_STATE", "Transfer history is not UTF-8"))?,"settings":settings});
    if let Some(saved) = bounded_file(&journal, 20 * 1024 * 1024)? {
        if serde_json::from_slice::<serde_json::Value>(&saved)? != backup {
            return Err(AppError::new("TORRENT_MIGRATION_CHANGED", "Legacy torrents changed after an interrupted cutover. Backup and original files were preserved; reconcile before retrying"));
        }
    } else {
        crate::torrent::write_state(&journal, &serde_json::to_vec_pretty(&backup)?)?;
    }
    for (name, content) in [
        ("entries.json", bytes),
        ("settings.json", serde_json::to_vec_pretty(&settings)?),
    ] {
        let path = dir.join(name);
        if let Some(staged) = bounded_file(&path, 8 * 1024 * 1024)? {
            if staged != content {
                return Err(AppError::new(
                    "TORRENT_MIGRATION_CHANGED",
                    "Staged torrent state differs from its backup; files were preserved",
                ));
            }
        } else {
            crate::torrent::write_state(&path, &content)?;
        }
    }
    let old_dir = data.join("torrents");
    fs::create_dir_all(&old_dir)?;
    if !fs::symlink_metadata(&old_dir)?.is_dir() {
        return Err(AppError::new(
            "TORRENT_STATE",
            "Legacy torrent directory is unsafe",
        ));
    }
    // This is deliberately not a Vec<Entry>: every older embedded writer fails
    // closed on load, including attempts to add shares or resume downloads.
    crate::torrent::write_state(&legacy, TORRENT_TOMBSTONE)?;
    Ok(())
}

const TORRENT_TOMBSTONE: &[u8] = b"{\"authority\":\"marquee-manager\",\"version\":1}";

pub(crate) fn torrent_cutover_complete(data: &Path) -> Result<bool> {
    if !torrents_owned(data)? {
        return Ok(false);
    }
    let entries = bounded_file(&data.join("manager/torrents/entries.json"), 8 * 1024 * 1024)?
        .ok_or_else(|| AppError::new("TORRENT_STATE", "Adopted torrent history is missing"))?;
    crate::torrent::validate_legacy(data, &entries)?;
    let settings = bounded_file(&data.join("manager/torrents/settings.json"), 64 * 1024)?
        .ok_or_else(|| AppError::new("TORRENT_STATE", "Adopted torrent settings are missing"))?;
    serde_json::from_slice::<crate::torrent::TorrentSettings>(&settings)?;
    Ok(true)
}

/// Cheap ownership check for Core settings reads; history corruption must not
/// prevent using an already installed library while its transport is offline.
pub(crate) fn torrents_owned(data: &Path) -> Result<bool> {
    use std::io::Read;
    let file = match File::options()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(data.join("torrents/entries.json"))
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e.into()),
    };
    let meta = file.metadata()?;
    if !meta.is_file() {
        return Err(AppError::new(
            "TORRENT_STATE",
            "Legacy state must be a regular file",
        ));
    }
    if meta.len() != TORRENT_TOMBSTONE.len() as u64 {
        return Ok(false);
    }
    let mut bytes = Vec::new();
    file.take(TORRENT_TOMBSTONE.len() as u64 + 1)
        .read_to_end(&mut bytes)?;
    Ok(bytes == TORRENT_TOMBSTONE)
}

fn bounded_file(path: &Path, max: usize) -> Result<Option<Vec<u8>>> {
    use std::io::Read;
    let file = match File::options()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if !file.metadata()?.is_file() {
        return Err(AppError::new(
            "TORRENT_STATE",
            "Torrent state must be a regular file",
        ));
    }
    let mut bytes = Vec::new();
    file.take(max as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > max {
        return Err(AppError::new(
            "TORRENT_STATE",
            "Torrent state exceeds its size limit",
        ));
    }
    Ok(Some(bytes))
}

fn legacy_torrent_settings(data: &Path) -> Result<crate::torrent::TorrentSettings> {
    let mut settings = crate::torrent::TorrentSettings::default();
    let path = data.join("library.db");
    if !path.exists() {
        return Ok(settings);
    }
    if !fs::symlink_metadata(&path)?.is_file() {
        return Err(AppError::new("TORRENT_STATE", "Library database is unsafe"));
    }
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut stmt = conn.prepare("SELECT key,value FROM settings WHERE key LIKE 'torrent.%'")?;
    for row in stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        let (key, value) = row?;
        match key.as_str() {
            "torrent.enabled" => settings.enabled = value == "1",
            "torrent.trackers" => settings.trackers = value == "1",
            "torrent.dht" => settings.dht = value == "1",
            "torrent.seeding" => settings.seeding = value == "1",
            "torrent.upload_kbps" | "torrent.download_kbps" => {
                let number = value.parse().map_err(|_| {
                    AppError::new("TORRENT_STATE", "Invalid saved torrent speed limit")
                })?;
                if key == "torrent.upload_kbps" {
                    settings.upload_kbps = number;
                } else {
                    settings.download_kbps = number;
                }
            }
            _ => {}
        }
    }
    Ok(settings)
}

#[cfg(test)]
mod torrent_tests {
    use super::*;
    fn data() -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("torrents")).unwrap();
        temp
    }
    #[test]
    fn cutover_keeps_exact_backup_settings_paths_and_retires_old_writers() {
        let temp = data();
        let data = temp.path();
        let db = Connection::open(data.join("library.db")).unwrap();
        db.execute_batch("CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT); INSERT INTO settings VALUES('torrent.enabled','1'),('torrent.trackers','1'),('torrent.dht','0'),('torrent.seeding','0'),('torrent.upload_kbps','432'),('torrent.download_kbps','123');").unwrap();
        let hash = "4927b4f8762d3727d8be7d48e754485c7f091436";
        let custom = data.join("custom");
        let destination = custom.join(hash);
        let entries = serde_json::json!([{"kind":"get","info_hash":hash,"name":"partial.zip","edition_id":null,"system":null,"dir":destination,"destination":custom,"peers":["127.0.0.1:1"],"paused":true,"owned":true,"downloaded":123,"uploaded":456,"total":789,"public":true}]);
        let bytes = serde_json::to_vec_pretty(&entries).unwrap();
        fs::write(data.join("torrents/entries.json"), &bytes).unwrap();
        adopt_torrents(data, false).unwrap();
        assert!(torrent_cutover_complete(data).unwrap());
        let backup: serde_json::Value = serde_json::from_slice(
            &fs::read(data.join("manager/torrents/legacy-backup.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(backup["entries"].as_str().unwrap().as_bytes(), bytes);
        assert_eq!(
            fs::read(data.join("manager/torrents/entries.json")).unwrap(),
            bytes
        );
        assert!(serde_json::from_slice::<Vec<serde_json::Value>>(
            &fs::read(data.join("torrents/entries.json")).unwrap()
        )
        .is_err());
        let settings = crate::torrent::read_settings(data).unwrap();
        assert!(settings.enabled && settings.trackers && !settings.dht && !settings.seeding);
        assert_eq!((settings.upload_kbps, settings.download_kbps), (432, 123));
        // Subsequent starts never overwrite manager authority with stale SQLite.
        db.execute(
            "UPDATE settings SET value='0' WHERE key='torrent.enabled'",
            [],
        )
        .unwrap();
        adopt_torrents(data, false).unwrap();
        assert_eq!(crate::torrent::read_settings(data).unwrap(), settings);
        assert_eq!(
            fs::metadata(data.join("manager/torrents/legacy-backup.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    #[test]
    fn live_legacy_owner_blocks_adoption_and_corruption_is_preserved() {
        let temp = data();
        let data = temp.path();
        let file = File::create(data.join("instance.lock")).unwrap();
        // SAFETY: held descriptor is valid until test completion.
        assert_eq!(
            unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );
        assert_eq!(
            adopt_torrents(data, false).unwrap_err().code,
            "MANAGER_MIGRATION_BUSY"
        );
        assert!(!data.join("manager/torrents/legacy-backup.json").exists());
        drop(file);
        fs::write(data.join("torrents/entries.json"), b"corrupt").unwrap();
        assert_eq!(
            adopt_torrents(data, false).unwrap_err().code,
            "TORRENT_STATE"
        );
        assert_eq!(
            fs::read(data.join("torrents/entries.json")).unwrap(),
            b"corrupt"
        );
        assert!(!data.join("manager/torrents/entries.json").exists());
    }
    #[test]
    fn interrupted_cutover_recovers_only_unchanged_inputs() {
        let temp = data();
        let data = temp.path();
        fs::write(data.join("torrents/entries.json"), b"[]").unwrap();
        adopt_torrents(data, false).unwrap();
        // Simulate a crash before the final tombstone was persisted.
        fs::write(data.join("torrents/entries.json"), b"[]").unwrap();
        adopt_torrents(data, false).unwrap();
        assert!(torrent_cutover_complete(data).unwrap());
        fs::write(data.join("torrents/entries.json"), b"[ ]").unwrap();
        assert_eq!(
            adopt_torrents(data, false).unwrap_err().code,
            "TORRENT_MIGRATION_CHANGED"
        );
        assert_eq!(
            fs::read(data.join("torrents/entries.json")).unwrap(),
            b"[ ]"
        );
    }
}
