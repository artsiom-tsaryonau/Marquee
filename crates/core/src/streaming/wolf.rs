//! Wolf API v1 adapter, based on upstream API structs and session reflectors.
//! Source revision and MIT attribution: third_party/wolf/README.md and LICENSE.
//! Keep the privileged control API on an explicitly selected local Unix socket.
use crate::error::{AppError, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const REVIEWED_REVISION: &str = "2d2ae0021ad812f79a966dd10e8289e0fae98ecb";
const MAX_BODY: usize = 2 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Serialize, Deserialize)]
pub struct App {
    pub id: String,
    pub title: String,
    pub support_hdr: bool,
    pub start_virtual_compositor: bool,
    pub start_audio_server: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Client {
    pub client_id: String,
}

/// Upstream's session reflector calls the session identifier `client_id`.
/// It is NOT the paired-client identity. Encryption material is never retained.
#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    #[serde(rename(deserialize = "client_id"))]
    pub session_id: String,
    pub client_ip: String,
    pub app_id: Option<String>,
    pub video_width: u32,
    pub video_height: u32,
    pub video_refresh_rate: u32,
    pub audio_channel_count: u32,
}

#[derive(Debug, Serialize)]
pub struct Snapshot {
    pub backend: &'static str,
    /// Contract revision reviewed by Marquee; not a claim about the live binary.
    pub reviewed_source_revision: &'static str,
    pub apps: Vec<App>,
    pub clients: Vec<Client>,
    pub sessions: Vec<Session>,
}

#[derive(Debug, Serialize)]
pub struct PendingClient {
    pub client_ip: String,
}

// Never derive Debug or Serialize here: the pairing secret stays in the adapter.
#[derive(Deserialize)]
struct PairRequest {
    client_ip: String,
    pair_secret: String,
}

#[derive(Deserialize)]
struct Apps {
    apps: Vec<App>,
}
#[derive(Deserialize)]
struct Clients {
    clients: Vec<Client>,
}
#[derive(Deserialize)]
struct Sessions {
    sessions: Vec<Session>,
}
#[derive(Deserialize)]
struct Pending {
    requests: Vec<PairRequest>,
}

#[derive(Debug, Clone)]
pub struct WolfClient {
    socket: PathBuf,
    timeout: Duration,
}

impl WolfClient {
    pub fn new(socket: impl AsRef<Path>) -> Result<Self> {
        if !socket.as_ref().is_absolute() {
            return Err(AppError::new(
                "STREAMING_SOCKET",
                "Choose an absolute path to Wolf's local API socket",
            ));
        }
        Ok(Self {
            socket: socket.as_ref().to_path_buf(),
            timeout: TIMEOUT,
        })
    }

    /// Three independent reads, not an atomic snapshot or a readiness verdict.
    pub fn inspect(&self) -> Result<Snapshot> {
        Ok(Snapshot {
            backend: "wolf",
            reviewed_source_revision: REVIEWED_REVISION,
            apps: self.request::<Apps>("GET", "/api/v1/apps", None)?.apps,
            clients: self
                .request::<Clients>("GET", "/api/v1/clients", None)?
                .clients,
            sessions: self
                .request::<Sessions>("GET", "/api/v1/sessions", None)?
                .sessions,
        })
    }

    pub fn pending(&self) -> Result<Vec<PendingClient>> {
        Ok(self
            .pending_requests()?
            .into_iter()
            .map(|r| PendingClient {
                client_ip: r.client_ip,
            })
            .collect())
    }

    /// Resolve exactly one pending request selected by its displayed address.
    /// Success means Wolf accepted the PIN, not that the client finished pairing.
    pub fn pair(&self, client_ip: &str, pin: &str) -> Result<()> {
        if pin.len() != 4 || !pin.bytes().all(|b| b.is_ascii_digit()) {
            return Err(AppError::new(
                "STREAMING_PIN",
                "Enter the four-digit PIN shown by Moonlight",
            ));
        }
        let requests = self.pending_requests()?;
        let matches: Vec<_> = requests
            .iter()
            .filter(|r| r.client_ip == client_ip)
            .collect();
        if matches.len() != 1 {
            return Err(AppError::new(
                "STREAMING_PAIR_SELECTION",
                "Choose an address with exactly one pending pairing request",
            ));
        }
        let _: serde_json::Value = self.request(
            "POST",
            "/api/v1/pair/client",
            Some(json!({
                "pair_secret": matches[0].pair_secret,
                "pin": pin,
            })),
        )?;
        Ok(())
    }

    /// This can end the game as well as the stream. Never use as a disconnect action.
    pub fn stop_session(&self, session_id: &str) -> Result<()> {
        validate_id(session_id)?;
        let _: serde_json::Value = self.request(
            "POST",
            "/api/v1/sessions/stop",
            Some(json!({"session_id": session_id})),
        )?;
        Ok(())
    }

    pub fn unpair(&self, client_id: &str) -> Result<()> {
        validate_id(client_id)?;
        let _: serde_json::Value = self.request(
            "POST",
            "/api/v1/unpair/client",
            Some(json!({"client_id": client_id})),
        )?;
        Ok(())
    }

    fn pending_requests(&self) -> Result<Vec<PairRequest>> {
        Ok(self
            .request::<Pending>("GET", "/api/v1/pair/pending", None)?
            .requests)
    }

    fn request<T: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        let body = body
            .map(|v| serde_json::to_vec(&v))
            .transpose()?
            .unwrap_or_default();
        let (status, bytes) = exchange(&self.socket, self.timeout, method, path, &body)?;
        if status == 404 && method == "GET" {
            return Err(AppError::new(
                "STREAMING_API_UNSUPPORTED",
                "Wolf's API v1 endpoint is unavailable; check the backend version",
            ));
        }
        if status != 200 {
            // Never echo backend bodies: errors may contain PINs, keys or commands.
            return Err(AppError::new(
                "STREAMING_REJECTED",
                format!("Wolf rejected the request (HTTP {status})"),
            ));
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| protocol())?;
        if value.get("success").and_then(|v| v.as_bool()) != Some(true) {
            return Err(AppError::new(
                "STREAMING_REJECTED",
                "Wolf did not acknowledge the request",
            ));
        }
        serde_json::from_value(value).map_err(|_| protocol())
    }
}

fn validate_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 20
        || !id.bytes().all(|b| b.is_ascii_digit())
        || id.parse::<u64>().is_err()
    {
        return Err(AppError::new(
            "STREAMING_ID",
            "Choose a numeric identifier returned by Wolf",
        ));
    }
    Ok(())
}

fn protocol() -> AppError {
    AppError::new(
        "STREAMING_PROTOCOL",
        "Wolf returned an invalid or unsupported API response",
    )
}

#[cfg(unix)]
fn exchange(
    socket: &Path,
    timeout: Duration,
    method: &str,
    path: &str,
    body: &[u8],
) -> Result<(u16, Vec<u8>)> {
    use std::io::{BufReader, Read, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Instant;

    let deadline = Instant::now() + timeout;
    let remaining = || {
        deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| AppError::new("STREAMING_TIMEOUT", "Wolf's API request timed out"))
    };
    let io_error = |err: std::io::Error| {
        if matches!(
            err.kind(),
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
        ) {
            AppError::new("STREAMING_TIMEOUT", "Wolf's API request timed out")
        } else {
            AppError::new("STREAMING_IO", "Wolf's API connection ended unexpectedly")
        }
    };
    let address = socket2::SockAddr::unix(socket).map_err(|_| {
        AppError::new(
            "STREAMING_SOCKET",
            "Wolf's API socket path is invalid or too long",
        )
    })?;
    let native = socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::STREAM, None)
        .map_err(&io_error)?;
    native.connect_timeout(&address, remaining()?).map_err(|error| {
        if error.kind() == std::io::ErrorKind::TimedOut {
            AppError::new("STREAMING_TIMEOUT", "Wolf's API connection timed out")
        } else {
            AppError::new("STREAMING_UNAVAILABLE", "Cannot connect to Wolf's local API socket; check that Wolf is running and the socket is accessible")
        }
    })?;
    let stream: UnixStream = native.into();
    stream
        .set_write_timeout(Some(remaining()?))
        .map_err(&io_error)?;
    let mut stream = stream;
    // Wolf uses Content-Length HTTP/1.0 responses and may keep the socket open.
    // Read exactly the frame, never wait for EOF or follow a redirect to TCP.
    write!(stream, "{method} {path} HTTP/1.0\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).map_err(&io_error)?;
    stream
        .set_write_timeout(Some(remaining()?))
        .map_err(&io_error)?;
    stream.write_all(body).map_err(&io_error)?;
    let mut reader = BufReader::new(stream);
    let mut headers = Vec::new();
    while !headers.ends_with(b"\r\n\r\n") {
        if headers.len() >= 16 * 1024 {
            return Err(protocol());
        }
        reader
            .get_ref()
            .set_read_timeout(Some(remaining()?))
            .map_err(&io_error)?;
        let mut byte = [0];
        reader.read_exact(&mut byte).map_err(&io_error)?;
        headers.push(byte[0]);
    }
    let headers = std::str::from_utf8(&headers).map_err(|_| protocol())?;
    let mut lines = headers.split("\r\n");
    let mut status = lines.next().ok_or_else(protocol)?.split_whitespace();
    if !matches!(status.next(), Some("HTTP/1.0" | "HTTP/1.1")) {
        return Err(protocol());
    }
    let status: u16 = status
        .next()
        .ok_or_else(protocol)?
        .parse()
        .map_err(|_| protocol())?;
    let mut length = None;
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':').ok_or_else(protocol)?;
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(protocol());
        }
        if name.eq_ignore_ascii_case("content-length") {
            if length.is_some() {
                return Err(protocol());
            }
            length = Some(value.trim().parse::<usize>().map_err(|_| protocol())?);
        }
    }
    let length = length.ok_or_else(protocol)?;
    if length > MAX_BODY {
        return Err(protocol());
    }
    let mut bytes = vec![0; length];
    let mut read = 0;
    while read < length {
        reader
            .get_ref()
            .set_read_timeout(Some(remaining()?))
            .map_err(&io_error)?;
        let count = reader.read(&mut bytes[read..]).map_err(&io_error)?;
        if count == 0 {
            return Err(protocol());
        }
        read += count;
    }
    Ok((status, bytes))
}

#[cfg(not(unix))]
fn exchange(_: &Path, _: Duration, _: &str, _: &str, _: &[u8]) -> Result<(u16, Vec<u8>)> {
    Err(AppError::new(
        "STREAMING_UNSUPPORTED",
        "Wolf control currently requires Unix sockets",
    ))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;
    use std::thread;

    #[test]
    fn response_timeout_is_bounded_and_truncated_bodies_fail() {
        for partial in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let socket = temp.path().join("wolf.sock");
            let listener = UnixListener::bind(&socket).unwrap();
            let server = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut headers = Vec::new();
                while !headers.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    headers.push(byte[0]);
                }
                if partial {
                    stream
                        .write_all(b"HTTP/1.0 200 OK\r\nContent-Length: 40\r\n\r\n{}")
                        .unwrap();
                } else {
                    thread::sleep(Duration::from_millis(150));
                }
            });
            let error = exchange(
                &socket,
                Duration::from_millis(40),
                "GET",
                "/api/v1/apps",
                &[],
            )
            .unwrap_err();
            assert_eq!(
                error.code,
                if partial {
                    "STREAMING_PROTOCOL"
                } else {
                    "STREAMING_TIMEOUT"
                }
            );
            server.join().unwrap();
        }
    }
}
