use crate::endpoint;
use crate::protocol::{self, Operation, Reply, Request, Status};
use crate::{AppError, Job, Result};
use serde::de::DeserializeOwned;
use std::fs;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn transport_error(error: AppError) -> AppError {
    if error.code == "IO" {
        AppError::new(
            "MANAGER_OFFLINE",
            format!("Manager connection interrupted: {}", error.message),
        )
    } else {
        error
    }
}

#[derive(Clone)]
pub struct Client {
    data: PathBuf,
    library_owned: bool,
}
impl Client {
    pub fn new(data: &Path) -> Self {
        Self {
            data: data.to_path_buf(),
            library_owned: false,
        }
    }
    /// The caller must hold the library's exclusive instance lease. Used only
    /// for the one-time M1→M2 ownership cutover, never by the daemon itself.
    pub fn for_library_owner(data: &Path) -> Self {
        Self {
            data: data.to_path_buf(),
            library_owned: true,
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data
    }

    pub fn socket_path(&self) -> Result<PathBuf> {
        endpoint::socket(&self.data)
    }

    fn exchange<T: DeserializeOwned>(&self, stream: UnixStream, operation: Operation) -> Result<T> {
        self.exchange_version(stream, operation, protocol::VERSION)
    }
    fn exchange_version<T: DeserializeOwned>(
        &self,
        mut stream: UnixStream,
        operation: Operation,
        version: u32,
    ) -> Result<T> {
        endpoint::same_user(&stream)?;
        stream.set_read_timeout(Some(Duration::from_secs(
            if matches!(operation, Operation::Hello) {
                2
            } else {
                1800
            },
        )))?;
        stream.set_write_timeout(Some(Duration::from_secs(30)))?;
        let request_id = uuid::Uuid::new_v4().to_string();
        protocol::write_frame(
            &mut stream,
            &Request {
                protocol_version: version,
                request_id: request_id.clone(),
                operation,
            },
        )
        .map_err(transport_error)?;
        let reply: Reply = protocol::read_frame(&mut stream).map_err(transport_error)?;
        if reply.protocol_version != version || reply.request_id != request_id {
            return Err(AppError::new(
                "MANAGER_VERSION",
                "Manager reply version/request does not match",
            ));
        }
        Ok(serde_json::from_value(reply.result?)?)
    }
    fn connect(&self) -> Result<UnixStream> {
        let socket = endpoint::socket(&self.data)?;
        UnixStream::connect(socket).map_err(|e| {
            AppError::new(
                "MANAGER_OFFLINE",
                format!("Manager unavailable: {e}. Start the manager to reconnect."),
            )
        })
    }
    pub fn status(&self) -> Result<Status> {
        match self
            .connect()
            .and_then(|stream| self.exchange(stream, Operation::Hello))
        {
            Ok(status) => Ok(status),
            Err(e) if e.code == "MANAGER_OFFLINE" => Ok(Status {
                running: false,
                pid: None,
                stopping: false,
                protocol_version: protocol::VERSION,
                snapshot_polling: true,
                event_replay: false,
                active_cancel: false,
            }),
            Err(e) => Err(e),
        }
    }
    fn binary(&self) -> Result<PathBuf> {
        if let Some(path) = std::env::var_os("MARQUEE_MANAGER_BIN") {
            let path = PathBuf::from(path);
            if path.is_absolute() && path.is_file() {
                return Ok(path);
            }
            return Err(AppError::new(
                "MANAGER_BINARY",
                "MARQUEE_MANAGER_BIN must name an absolute manager executable",
            ));
        }
        let current = std::env::current_exe()?;
        let parent = current
            .parent()
            .ok_or_else(|| AppError::new("MANAGER_BINARY", "No application directory"))?;
        for dir in [
            Some(parent),
            parent
                .file_name()
                .filter(|n| *n == "deps")
                .and_then(|_| parent.parent()),
        ]
        .into_iter()
        .flatten()
        {
            let binary = dir.join("marquee-manager");
            if binary.is_file() {
                return Ok(binary);
            }
        }
        Err(AppError::new("MANAGER_BINARY", "The marquee-manager companion is missing. Build it with cargo build -p marquee-manager, or reinstall Marquee with its companion."))
    }
    pub fn start_manager(&self) -> Result<Status> {
        endpoint::private_directory(&self.data.join("manager"))?;
        let status = match self.status() {
            Err(e) if e.code == "MANAGER_VERSION" => {
                self.stop()?;
                let deadline = Instant::now() + Duration::from_secs(10);
                loop {
                    if self.connect().is_err() {
                        break;
                    }
                    if Instant::now() >= deadline {
                        return Err(AppError::new(
                            "MANAGER_STOPPING",
                            "Older manager is draining accepted work; retry when it has stopped",
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                self.status()?
            }
            result => result?,
        };
        if status.running {
            return Ok(status);
        }
        let stopped = self.data.join("manager/stopped");
        if stopped.exists() {
            fs::remove_file(stopped)?;
        }
        let binary = self.binary()?;
        crate::migration::adopt_m1(&self.data, self.library_owned)?;
        let data = fs::canonicalize(&self.data)?;
        let log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(data.join("manager/worker.log"))?;
        let mut command = Command::new(binary);
        command
            .arg("--data-dir")
            .arg(&data)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log);
        // SAFETY: setsid is async-signal-safe; the child does not capture or
        // allocate anything before exec. Closing the launcher cannot signal it.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = command.spawn()?;
        let until = Instant::now() + Duration::from_secs(10);
        while Instant::now() < until {
            if let Ok(stream) = self.connect() {
                let result = self.exchange(stream, Operation::Hello);
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                return result;
            }
            if let Some(status) = child.try_wait()? {
                // A competing spawn may have won the exclusive manager lease.
                if let Ok(stream) = self.connect() {
                    return self.exchange(stream, Operation::Hello);
                }
                return Err(AppError::new(
                    "MANAGER_START",
                    format!("Manager exited during startup ({status}); see manager/worker.log"),
                ));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Err(AppError::new(
            "MANAGER_START",
            "Manager startup timed out; existing games remain available",
        ))
    }
    fn call<T: DeserializeOwned>(&self, operation: Operation, explicit: bool) -> Result<T> {
        if explicit {
            self.start_manager()?;
        }
        let stream = match self.connect() {
            Ok(stream) => stream,
            Err(e)
                if e.code == "MANAGER_OFFLINE" && !self.data.join("manager/stopped").exists() =>
            {
                self.start_manager()?;
                self.connect()?
            }
            Err(e) => return Err(e),
        };
        self.exchange(stream, operation)
    }
    pub fn list(&self) -> Result<Vec<Job>> {
        if !self.data.join("manager/jobs.db").exists() {
            return Ok(vec![]);
        }
        self.call(Operation::List, false)
    }
    pub fn browser_request(
        &self,
        client_id: &str,
        request: crate::browser::Request,
    ) -> Result<serde_json::Value> {
        let explicit = matches!(request, crate::browser::Request::Start { .. });
        self.call(
            Operation::Browser {
                client_id: client_id.into(),
                request,
            },
            explicit,
        )
    }
    pub fn browser_jobs(&self) -> Result<Vec<crate::browser::Job>> {
        if !self.data.join("manager/jobs.db").exists() {
            return Ok(vec![]);
        }
        self.call(Operation::BrowserList, false)
    }
    pub fn inspect_browser(&self, id: &str, system: &str) -> Result<Job> {
        self.call(
            Operation::InspectBrowser {
                id: id.into(),
                system: system.into(),
            },
            true,
        )
    }
    pub fn request_browser_cancel(&self, id: &str) -> Result<crate::browser::Job> {
        self.call(Operation::BrowserCancel { id: id.into() }, false)
    }
    pub fn get(&self, id: &str) -> Result<Job> {
        self.call(Operation::Get { id: id.into() }, false)
    }
    pub fn inspect(&self, source: &Path, system: &str) -> Result<Job> {
        self.call(
            Operation::Inspect {
                source: source.into(),
                system: system.into(),
            },
            true,
        )
    }
    pub fn start_job(&self, id: &str, token: &str) -> Result<Job> {
        self.call(
            Operation::Start {
                id: id.into(),
                review_token: token.into(),
            },
            true,
        )
    }
    pub fn cancel(&self, id: &str) -> Result<Job> {
        self.call(Operation::Cancel { id: id.into() }, true)
    }
    pub fn acknowledge(&self, id: &str, fingerprint: &str, ids: Vec<String>) -> Result<Job> {
        self.call(
            Operation::Acknowledge {
                id: id.into(),
                receipt_fingerprint: fingerprint.into(),
                edition_ids: ids,
            },
            false,
        )
    }
    pub fn publication_failed(
        &self,
        id: &str,
        fingerprint: &str,
        code: &str,
        message: &str,
    ) -> Result<Job> {
        self.call(
            Operation::PublicationFailed {
                id: id.into(),
                receipt_fingerprint: fingerprint.into(),
                code: code.into(),
                message: message.into(),
            },
            false,
        )
    }
    pub fn torrent<T: DeserializeOwned>(&self, request: crate::torrent::Request) -> Result<T> {
        crate::migration::adopt_torrents(&self.data, self.library_owned)?;
        // Also upgrades an older daemon through a drain-only stop before sending
        // transport commands. No preparation/receipt operation is downgraded.
        let passive = matches!(
            request,
            crate::torrent::Request::Status
                | crate::torrent::Request::Settings
                | crate::torrent::Request::Acknowledge { .. }
        );
        if !passive || !self.data.join("manager/stopped").exists() {
            self.start_manager()?;
        }
        self.call(Operation::Torrent { request }, false)
    }
    pub fn stop(&self) -> Result<Status> {
        for version in [protocol::VERSION, 2, 1] {
            match self.exchange_version(self.connect()?, Operation::Stop, version) {
                Err(e) if e.code == "MANAGER_VERSION" => continue,
                result => return result,
            }
        }
        Err(AppError::new(
            "MANAGER_VERSION",
            "No compatible shutdown protocol",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    #[test]
    fn explicit_stop_can_drain_a_v1_manager_without_downgrading_job_operations() {
        let temp = tempfile::tempdir().unwrap();
        let client = Client::new(temp.path());
        let socket = client.socket_path().unwrap();
        let listener = UnixListener::bind(&socket).unwrap();
        let server = std::thread::spawn(move || {
            for version in [protocol::VERSION, 2, 1] {
                let (mut stream, _) = listener.accept().unwrap();
                let request: Request = protocol::read_frame(&mut stream).unwrap();
                assert_eq!(request.protocol_version, version);
                assert!(matches!(request.operation, Operation::Stop));
                let result = if version == 1 {
                    Ok(
                        serde_json::json!({"running":true,"pid":null,"stopping":true,"protocol_version":1,"snapshot_polling":true,"event_replay":false,"active_cancel":false}),
                    )
                } else {
                    Err(AppError::new("MANAGER_VERSION", "v1 only"))
                };
                protocol::write_frame(
                    &mut stream,
                    &Reply {
                        protocol_version: 1,
                        request_id: request.request_id,
                        result,
                    },
                )
                .unwrap();
            }
        });
        assert!(client.stop().unwrap().stopping);
        server.join().unwrap();
    }
}
