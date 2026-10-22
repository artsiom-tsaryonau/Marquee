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
