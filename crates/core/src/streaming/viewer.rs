//! Own the optional reference receiver, independently of the game's lifetime.
//! Control uses a private local socket, never a persisted PID or Wolf stop API.
use crate::error::{AppError, Result};
use crate::instance::InstanceLock;
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::{
    fs::PermissionsExt,
    net::{UnixListener, UnixStream},
    process::CommandExt,
};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const MAX_LOG: u64 = 16 * 1024 * 1024;

fn root(data: &Path) -> PathBuf {
    data.join("streaming/moonlight/viewer")
}

fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    if !fs::symlink_metadata(path)?.is_dir() {
        return Err(AppError::new(
            "VIEWER_PATH",
            "A viewer directory is not a regular directory",
        ));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

pub(crate) fn configuration_lease(data: &Path) -> Result<InstanceLock> {
    private_dir(&root(data))?;
    InstanceLock::acquire(&root(data).join("owner.lock"))
}

/// Foreground owner. The caller can disconnect it from another CLI/app process.
/// A live child is not proof of an authenticated stream or successful decoding.
pub fn view(data: &Path, socket: &Path, app_id: &str) -> Result<Value> {
    let snapshot = super::wolf::WolfClient::new(socket)?.inspect()?;
    let apps: Vec<_> = snapshot.apps.iter().filter(|a| a.id == app_id).collect();
    if apps.len() != 1 {
        return Err(AppError::new(
            "VIEWER_APP",
            "Select one app advertised by the running Wolf host",
        ));
    }
    let title = &apps[0].title;
    // Moonlight's CLI selects by case-insensitive name, not the Wolf app ID.
    if title.is_empty()
        || title.len() > 256
        || title.starts_with('-')
        || title.chars().any(char::is_control)
        || snapshot
            .apps
            .iter()
            .filter(|a| a.title.to_lowercase() == title.to_lowercase())
            .count()
            != 1
    {
        return Err(AppError::new(
            "VIEWER_APP",
            "The reference receiver needs a unique, unambiguous app name",
        ));
    }
    let mut command = reference_command(data)?;
    command.args([
        "stream",
        "127.0.0.1",
        title,
        "--720",
        "--fps",
        "60",
        "--bitrate",
        "10000",
        "--display-mode",
        "windowed",
        "--audio-config",
        "stereo",
        "--video-codec",
        "H.264",
        "--video-decoder",
        "auto",
        "--capture-system-keys",
        "never",
        "--no-quit-after",
        "--no-audio-on-host",
        "--no-background-gamepad",
    ]);
    supervise(&root(data), &mut command, app_id)
}

fn reference_command(data: &Path) -> Result<Command> {
    let installed = super::receiver::status(data)?.ok_or_else(|| {
        AppError::new(
            "VIEWER_NOT_INSTALLED",
            "Install the reference receiver first",
        )
    })?;
    let preferences = data.join("streaming/moonlight/preferences");
    private_dir(&preferences)?;
    let mut command = Command::new(installed.executable);
    for (name, dir) in [
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_CACHE_HOME", "cache"),
    ] {
        let path = preferences.join(dir);
        private_dir(&path)?;
        command.env(name, path);
    }
    command.env("QT_QPA_PLATFORM", &installed.reference.qt_platform);
    Ok(command)
}

/// Pair using the same isolated preferences and ownership as the viewer.
pub fn pair(data: &Path) -> Result<Value> {
    let mut command = reference_command(data)?;
    command.args(["pair", "127.0.0.1"]);
    supervise(&root(data), &mut command, "pairing")
}

struct OwnedChild(Child);
impl OwnedChild {
    fn stop(&mut self) -> Result<()> {
        if self.0.try_wait()?.is_some() {
            return Ok(());
        }
        // The group belongs to this unreaped child, created below. No PID from
        // disk, API input or a process-name search is ever used as a kill target.
        unsafe {
            libc::kill(-(self.0.id() as i32), libc::SIGTERM);
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if self.0.try_wait()?.is_some() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        unsafe {
            libc::kill(-(self.0.id() as i32), libc::SIGKILL);
        }
        self.0.wait()?;
        Ok(())
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

struct ControlSocket(PathBuf);
impl Drop for ControlSocket {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn supervise(dir: &Path, command: &mut Command, app_id: &str) -> Result<Value> {
    private_dir(dir)?;
    let _lease = InstanceLock::acquire(&dir.join("owner.lock"))?;
    let socket = dir.join("control.sock");
    match fs::symlink_metadata(&socket) {
        Ok(meta) => {
            use std::os::unix::fs::FileTypeExt;
            if !meta.file_type().is_socket() || UnixStream::connect(&socket).is_ok() {
                return Err(AppError::new(
                    "VIEWER_SOCKET",
                    "An existing viewer control path was preserved",
                ));
            }
            fs::remove_file(&socket)?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.into()),
    }
    let listener = UnixListener::bind(&socket)?;
    let _socket = ControlSocket(socket);
    fs::set_permissions(&_socket.0, fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    let log_path = dir.join("last-viewer.log");
    use std::os::unix::fs::OpenOptionsExt;
    let log = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .custom_flags(libc::O_NOFOLLOW)
        .mode(0o600)
        .open(&log_path)?;
    fs::set_permissions(&log_path, fs::Permissions::from_mode(0o600))?;
    #[cfg(target_os = "linux")]
    {
        let owner_pid = std::process::id() as libc::pid_t;
        // Only async-signal-safe syscalls between fork and exec. If the owner
        // dies, the receiver must not remain as an unowned window. Closing it
        // still never invokes the host's game-stop endpoint.
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::getppid() != owner_pid {
                    return Err(std::io::Error::from_raw_os_error(libc::ECHILD));
                }
                Ok(())
            });
        }
    }
    let mut child = OwnedChild(
        command
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log)
            .process_group(0)
            .spawn()?,
    );
    loop {
        if let Some(status) = child.0.try_wait()? {
            return Ok(
                json!({"reason":"receiver-exited", "exit_code":status.code(), "game_stop_requested":false}),
            );
        }
        if fs::metadata(&log_path)?.len() > MAX_LOG {
            child.stop()?;
            return Err(AppError::new(
                "VIEWER_LOG_LIMIT",
                "Viewer diagnostics exceeded the limit; only the receiver was stopped",
            ));
        }
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_read_timeout(Some(Duration::from_millis(150)))?;
                stream.set_write_timeout(Some(Duration::from_millis(150)))?;
                let mut reader = BufReader::new(stream);
                let mut request = Vec::new();
                let read = reader.by_ref().take(33).read_until(b'\n', &mut request);
                let disconnect = read.is_ok() && request == b"disconnect\n";
                let value = if disconnect {
                    child.stop()?;
                    json!({"running":false,"reason":"viewer-disconnected","game_stop_requested":false})
                } else if read.is_ok() && request == b"status\n" {
                    json!({"running":true,"receiver_pid":child.0.id(),"app_id":app_id,
                        "connection_verified":false,"game_stop_requested":false})
                } else {
                    json!({"error":"Invalid viewer control request"})
                };
                let _ = writeln!(reader.get_mut(), "{value}");
                if disconnect {
                    return Ok(value);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20))
            }
            Err(e) => return Err(e.into()),
        }
    }
}

fn request(data: &Path, action: &[u8]) -> Result<Value> {
    let mut stream = UnixStream::connect(root(data).join("control.sock"))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;
    stream.write_all(action)?;
    let mut bytes = Vec::new();
    BufReader::new(stream)
        .take(4097)
        .read_until(b'\n', &mut bytes)?;
    if bytes.len() > 4096 || bytes.last() != Some(&b'\n') {
        return Err(AppError::new(
            "VIEWER_PROTOCOL",
            "Invalid viewer control response",
        ));
    }
    let value: Value = serde_json::from_slice(&bytes)?;
    if value.get("error").is_some() {
        return Err(AppError::new(
            "VIEWER_PROTOCOL",
            "Viewer control request failed",
        ));
    }
    let expected_running = action == b"status\n";
    if value["running"] != expected_running || value["game_stop_requested"] != false {
        return Err(AppError::new(
            "VIEWER_PROTOCOL",
            "Viewer control returned an invalid acknowledgement",
        ));
    }
    Ok(value)
}

/// Does not inspect PID files, launch a process, or contact the game host.
pub fn status(data: &Path) -> Result<Value> {
    match request(data, b"status\n") {
        Ok(v) => Ok(v),
        Err(_) if !root(data).join("control.sock").exists() => Ok(json!({"running":false})),
        Err(e) => Err(e),
    }
}

/// Closes only the receiver owned by this supervisor; never calls Wolf stop.
pub fn disconnect(data: &Path) -> Result<Value> {
    if !root(data).join("control.sock").exists() {
        return Err(AppError::new(
            "VIEWER_NOT_RUNNING",
            "There is no Marquee-owned viewer to disconnect",
        ));
    }
    request(data, b"disconnect\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait_running(data: &Path) -> Value {
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            if let Ok(value) = status(data) {
                if value["running"] == true {
                    return value;
                }
            }
            assert!(Instant::now() < deadline, "viewer owner did not start");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn disconnect_targets_only_owned_receiver_and_rejects_other_commands() {
        let temp = tempfile::tempdir().unwrap();
        let data = temp.path().to_path_buf();
        let owned = data.clone();
        let server = std::thread::spawn(move || {
            supervise(
                &root(&owned),
                Command::new("/bin/sh")
                    .args(["-c", "trap 'exit 0' TERM; while :; do sleep 0.1; done"]),
                "test-app",
            )
        });
        let value = wait_running(&data);
        assert_eq!(value["app_id"], "test-app");
        assert_eq!(value["connection_verified"], false);
        let mut unrelated = OwnedChild(
            Command::new("sleep")
                .arg("20")
                .process_group(0)
                .spawn()
                .unwrap(),
        );
        assert_eq!(
            request(&data, b"disconnect-extra\n").unwrap_err().code,
            "VIEWER_PROTOCOL"
        );
        assert_eq!(status(&data).unwrap()["running"], true);
        assert_eq!(
            supervise(&root(&data), &mut Command::new("/bin/false"), "other")
                .unwrap_err()
                .code,
            "INSTANCE_BUSY"
        );
        let ack = disconnect(&data).unwrap();
        assert_eq!(ack["running"], false);
        assert_eq!(ack["game_stop_requested"], false);
        assert_eq!(server.join().unwrap().unwrap(), ack);
        assert!(unrelated.0.try_wait().unwrap().is_none());
        assert_eq!(status(&data).unwrap()["running"], false);
        assert_eq!(disconnect(&data).unwrap_err().code, "VIEWER_NOT_RUNNING");
        assert!(!root(&data).join("control.sock").exists());
    }

    #[test]
    fn natural_exit_and_stale_socket_recovery_do_not_use_saved_pids() {
        let temp = tempfile::tempdir().unwrap();
        let dir = root(temp.path());
        private_dir(&dir).unwrap();
        let socket = dir.join("control.sock");
        drop(UnixListener::bind(&socket).unwrap());
        fs::write(dir.join("old-pid.json"), std::process::id().to_string()).unwrap();
        let result = supervise(
            &dir,
            Command::new("/bin/sh").args(["-c", "exit 7"]),
            "test-app",
        )
        .unwrap();
        assert_eq!(result["exit_code"], 7);
        assert_eq!(result["reason"], "receiver-exited");
        assert!(!socket.exists());
        assert_eq!(
            fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }

    #[test]
    fn unexpected_socket_path_is_preserved_without_launching_a_receiver() {
        let temp = tempfile::tempdir().unwrap();
        let dir = root(temp.path());
        private_dir(&dir).unwrap();
        let socket = dir.join("control.sock");
        fs::write(&socket, b"preserve me").unwrap();
        let err = supervise(&dir, &mut Command::new("/bin/false"), "test-app").unwrap_err();
        assert_eq!(err.code, "VIEWER_SOCKET");
        assert_eq!(fs::read(socket).unwrap(), b"preserve me");
    }

    #[test]
    fn linked_log_cannot_truncate_an_unrelated_file() {
        let temp = tempfile::tempdir().unwrap();
        let dir = root(temp.path());
        private_dir(&dir).unwrap();
        let other = temp.path().join("unrelated.txt");
        fs::write(&other, b"preserve this content").unwrap();
        std::os::unix::fs::symlink(&other, dir.join("last-viewer.log")).unwrap();
        assert!(supervise(&dir, &mut Command::new("/bin/false"), "test-app").is_err());
        assert_eq!(fs::read(other).unwrap(), b"preserve this content");
        assert!(!dir.join("control.sock").exists());
    }

    #[test]
    fn missing_or_false_control_acknowledgements_are_not_success() {
        for response in [
            "{}\n",
            "{\"running\":true,\"game_stop_requested\":false}\n",
            "{\"running\":false,\"game_stop_requested\":true}\n",
            "{\"running\":false}",
        ] {
            let temp = tempfile::tempdir().unwrap();
            let dir = root(temp.path());
            private_dir(&dir).unwrap();
            let listener = UnixListener::bind(dir.join("control.sock")).unwrap();
            let response = response.to_string();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut action = [0; 11];
                stream.read_exact(&mut action).unwrap();
                assert_eq!(&action, b"disconnect\n");
                stream.write_all(response.as_bytes()).unwrap();
            });
            assert_eq!(disconnect(temp.path()).unwrap_err().code, "VIEWER_PROTOCOL");
            server.join().unwrap();
        }
    }
}
