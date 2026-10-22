use crate::endpoint::{self, Lock};
use crate::protocol::{self, Operation, Reply, Request, Status};
use crate::service::Manager;
use crate::{AppError, Result};
use serde::Serialize;
use serde_json::Value;
use std::fs::{self, File};
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

fn json(value: impl Serialize) -> Result<Value> {
    Ok(serde_json::to_value(value)?)
}

struct Server {
    manager: Arc<Manager>,
    stopping: AtomicBool,
    connections: AtomicUsize,
    stopped_marker: std::path::PathBuf,
}
impl Server {
    fn status(&self) -> Status {
        Status {
            running: true,
            pid: Some(std::process::id()),
            stopping: self.stopping.load(Ordering::Acquire),
            protocol_version: protocol::VERSION,
            snapshot_polling: true,
            event_replay: false,
            active_cancel: false,
        }
    }
    fn dispatch(&self, operation: Operation) -> Result<Value> {
        if self.stopping.load(Ordering::Acquire)
            && !matches!(
                operation,
                Operation::Hello | Operation::Get { .. } | Operation::List | Operation::Stop
            )
        {
            return Err(AppError::new(
                "MANAGER_STOPPING",
                "The manager is finishing accepted work before stopping",
            ));
        }
        match operation {
            Operation::Hello => json(self.status()),
            Operation::Browser { client_id, request } => {
                self.manager.browser_request(&client_id, request)
            }
            Operation::BrowserList => json(self.manager.browser_jobs()?),
            Operation::BrowserCancel { id } => json(self.manager.request_browser_cancel(&id)?),
            Operation::InspectBrowser { id, system } => {
                json(self.manager.inspect_browser(&id, &system)?)
            }
            Operation::List => json(self.manager.list()?),
            Operation::Get { id } => json(self.manager.get(&id)?),
            Operation::Inspect { source, system } => json(self.manager.inspect(&source, &system)?),
            Operation::Start { id, review_token } => json(self.manager.start(&id, &review_token)?),
            Operation::Cancel { id } => json(self.manager.cancel(&id)?),
            Operation::Acknowledge {
                id,
                receipt_fingerprint,
                edition_ids,
            } => json(
                self.manager
                    .acknowledge(&id, &receipt_fingerprint, edition_ids)?,
            ),
            Operation::PublicationFailed {
                id,
                receipt_fingerprint,
                code,
                message,
            } => {
                json(
                    self.manager
                        .publication_failed(&id, &receipt_fingerprint, &code, &message)?,
                )
            }
            Operation::Stop => {
                File::options()
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .mode(0o600)
                    .custom_flags(libc::O_NOFOLLOW)
                    .open(&self.stopped_marker)?
                    .sync_all()?;
                File::open(self.stopped_marker.parent().expect("manager directory"))?.sync_all()?;
                self.stopping.store(true, Ordering::Release);
                json(self.status())
            }
        }
    }
    fn serve(&self, stream: &mut UnixStream) -> Result<()> {
        endpoint::same_user(stream)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(30)))?;
        let request: Request = protocol::read_frame(stream)?;
        let result = if request.protocol_version != protocol::VERSION {
            Err(AppError::new(
                "MANAGER_VERSION",
                "Unsupported manager protocol version; stop/restart with compatible binaries",
            ))
        } else if request.request_id.is_empty() || request.request_id.len() > 128 {
            Err(AppError::new(
                "MANAGER_REQUEST",
                "Invalid manager request ID",
            ))
        } else {
            self.dispatch(request.operation)
        };
        let reply = Reply {
            protocol_version: protocol::VERSION,
            request_id: request.request_id,
            result,
        };
        if let Err(error) = protocol::write_frame(stream, &reply) {
            if error.code == "MANAGER_FRAME" {
                let fallback = Reply {
                    protocol_version: reply.protocol_version,
                    request_id: reply.request_id,
                    result: Err(error),
                };
                return protocol::write_frame(stream, &fallback);
            }
            return Err(error);
        }
        Ok(())
    }
}

pub fn run(data: &Path) -> Result<()> {
    let data = fs::canonicalize(data)?;
    let _lock = Lock::acquire(&data)?;
    let stopped_marker = data.join("manager/stopped");
    if stopped_marker.exists() {
        fs::remove_file(&stopped_marker)?;
    }
    let socket = endpoint::socket(&data)?;
    // Only the process holding the data-directory lease can remove a stale socket.
    if let Ok(meta) = fs::symlink_metadata(&socket) {
        if !meta.file_type().is_socket() {
            return Err(AppError::new(
                "MANAGER_PRIVATE_PATH",
                "Manager socket path was replaced",
            ));
        }
        fs::remove_file(&socket)?;
    }
    let listener = UnixListener::bind(&socket)?;
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    let server = Arc::new(Server {
        manager: Arc::new(Manager::new(&data)),
        stopping: AtomicBool::new(false),
        connections: AtomicUsize::new(0),
        stopped_marker: data.join("manager/stopped"),
    });
    // Reconcile M1/abandoned checkpoints only after acquiring exclusive ownership.
    server.manager.list()?;
    loop {
        if server.stopping.load(Ordering::Acquire)
            && !server.manager.active()
            && server.connections.load(Ordering::Acquire) == 0
        {
            break;
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                if server.connections.fetch_add(1, Ordering::AcqRel) >= 32 {
                    server.connections.fetch_sub(1, Ordering::Release);
                    continue;
                }
                let connection = Arc::clone(&server);
                if std::thread::Builder::new()
                    .name("manager-ipc".into())
                    .spawn(move || {
                        let _ = connection.serve(&mut stream);
                        connection.connections.fetch_sub(1, Ordering::Release);
                    })
                    .is_err()
                {
                    server.connections.fetch_sub(1, Ordering::Release);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20))
            }
            Err(e) => return Err(e.into()),
        }
    }
    fs::remove_file(&socket)?;
    Ok(())
}
