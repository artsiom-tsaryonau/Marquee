use crate::error::{AppError, Result};
use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;
use std::path::Path;

pub struct InstanceLock {
    _file: File,
}

impl InstanceLock {
    pub fn acquire(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        // SAFETY: `file` is an open fd we own for the lifetime of InstanceLock.
        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if rc != 0 {
            return Err(AppError::new(
                "INSTANCE_BUSY",
                "Marquee is already running with this data directory",
            ));
        }
        Ok(Self { _file: file })
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        // Explicitly unlock: a concurrent fork may briefly inherit this open file
        // description before exec closes it. That child must not extend our lease.
        // SAFETY: this fd remains valid until the File field is dropped next.
        unsafe {
            libc::flock(self._file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}
