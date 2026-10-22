use crate::{AppError, Result};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

pub fn private_directory(path: &Path) -> Result<()> {
    if !path.exists() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)?;
    }
    let meta = fs::symlink_metadata(path)?;
    // SAFETY: geteuid has no preconditions.
    let uid = unsafe { libc::geteuid() };
    if !meta.is_dir() || meta.uid() != uid || meta.mode() & 0o077 != 0 {
        return Err(AppError::new(
            "MANAGER_PRIVATE_PATH",
            "Manager directory must be owned by this user with mode 700",
        ));
    }
    Ok(())
}

pub fn socket(data: &Path) -> Result<PathBuf> {
    let data = fs::canonicalize(data)?;
    // Short paths also work with deeply nested isolated library directories.
    // SAFETY: geteuid has no preconditions.
    let uid = unsafe { libc::geteuid() };
    let parent = std::env::temp_dir().join(format!("marquee-manager-{uid}"));
    private_directory(&parent)?;
    let key = format!("{:x}", Sha256::digest(data.as_os_str().as_encoded_bytes()));
    let dir = parent.join(&key[..24]);
    private_directory(&dir)?;
    Ok(dir.join("ipc.sock"))
}

pub struct Lock(File);
impl Lock {
    pub fn acquire(data: &Path) -> Result<Self> {
        let dir = data.join("manager");
        private_directory(&dir)?;
        let file = File::options()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(dir.join("instance.lock"))?;
        use std::os::fd::AsRawFd;
        // SAFETY: the descriptor belongs to `file`, held for the whole service.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(AppError::new(
                "MANAGER_BUSY",
                "A manager already owns this data directory",
            ));
        }
        Ok(Self(file))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        // SAFETY: the descriptor stays open until this destructor returns.
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

pub fn same_user(stream: &std::os::unix::net::UnixStream) -> Result<()> {
    use std::os::fd::AsRawFd;
    let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: valid socket descriptor and writable ucred/length buffers.
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut credentials as *mut _ as *mut libc::c_void,
            &mut len,
        )
    };
    if result != 0
        || len as usize != std::mem::size_of::<libc::ucred>()
        || credentials.uid != unsafe { libc::geteuid() }
    {
        return Err(AppError::new(
            "MANAGER_PEER",
            "Manager connection requires the same local user",
        ));
    }
    Ok(())
}
