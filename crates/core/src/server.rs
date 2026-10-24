//! Local HTTP surface over the same core, so a browser tab shows the same
//! library, launches the same games and receives the same events as the native
//! window. Loopback only; every API and cover request needs the token.

use crate::api;
use crate::error::{AppError, Result};
use crate::Core;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};
use uuid::Uuid;

/// Looks up a built UI file by relative path (`index.html`, `assets/x.js`).
pub type Assets = Arc<dyn Fn(&str) -> Option<(Vec<u8>, String)> + Send + Sync>;

pub const DEFAULT_PORT: u16 = 47820;
const MAX_BODY: u64 = 1024 * 1024;
const LONG_POLL: Duration = Duration::from_secs(25);
const CSP: &str = "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; font-src 'self' data:; connect-src 'self'; frame-ancestors 'none'";

pub struct ServerHandle {
    pub addr: SocketAddr,
    pub token: String,
}

impl ServerHandle {
    /// Open this once; the UI keeps the token and removes it from the address bar.
    pub fn url(&self) -> String {
        format!("http://{}/#token={}", self.addr, self.token)
    }

    /// Read-only track capability, never the general API bearer token.
    pub fn music_url(&self, id: &str) -> String {
        format!(
            "http://{}/music/{}?key={}",
            self.addr,
            id,
            music_key(&self.token, id)
        )
    }
}

fn music_key(token: &str, id: &str) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "{:x}",
        Sha256::digest(format!("marquee-music:{token}:{id}"))
    )
}

fn audio_range(value: Option<&str>, size: u64) -> Option<(u64, u64, bool)> {
    if size == 0 {
        return None;
    }
    let Some(value) = value else {
        return Some((0, size - 1, false));
    };
    let (start, end) = value.strip_prefix("bytes=")?.split_once('-')?;
    if start.is_empty() {
        let count = end.parse::<u64>().ok()?;
        return (count > 0).then_some((size.saturating_sub(count), size - 1, true));
    }
    let start = start.parse::<u64>().ok()?;
    let end = if end.is_empty() {
        size - 1
    } else {
        end.parse::<u64>().ok()?.min(size - 1)
    };
    (start < size && start <= end).then_some((start, end, true))
}

fn serve_music(core: &Core, req: Request, id: &str) {
    let Ok((mut file, mime)) = core.music_open(id) else {
        return respond(req, 404, "text/plain", b"Track unavailable".to_vec(), true);
    };
    let Ok(size) = file.metadata().map(|m| m.len()) else {
        return respond(req, 404, "text/plain", b"Track unavailable".to_vec(), true);
    };
    let range = req
        .headers()
        .iter()
        .find(|h| h.field.equiv("Range"))
        .map(|h| h.value.as_str());
    let Some((start, end, partial)) = audio_range(range, size) else {
        let mut response = Response::empty(StatusCode(416));
        response.add_header(header("Content-Range", &format!("bytes */{size}")));
        let _ = req.respond(response);
        return;
    };
    if file.seek(SeekFrom::Start(start)).is_err() {
        return respond(req, 404, "text/plain", b"Track unavailable".to_vec(), true);
    }
    let length = end - start + 1;
    let mut headers = vec![
        header("Content-Type", mime),
        header("Accept-Ranges", "bytes"),
        header("Cache-Control", "no-store"),
        header("Referrer-Policy", "no-referrer"),
        header("X-Content-Type-Options", "nosniff"),
    ];
    if partial {
        headers.push(header(
            "Content-Range",
            &format!("bytes {start}-{end}/{size}"),
        ));
    }
    let _ = req.respond(Response::new(
        StatusCode(if partial { 206 } else { 200 }),
        headers,
        file.take(length),
        Some(length as usize),
        None,
    ));
}

pub fn load_or_create_token(path: &Path) -> Result<String> {
    if let Ok(existing) = fs::read_to_string(path) {
        let existing = existing.trim().to_string();
        if existing.len() >= 32 {
            return Ok(existing);
        }
    }
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(token.as_bytes())?;
    }
    #[cfg(not(unix))]
    fs::write(path, &token)?;
    Ok(token)
}

/// Serve the built UI from a directory (`apps/desktop/dist`).
pub fn dir_assets(dir: PathBuf) -> Assets {
    Arc::new(move |rel: &str| {
        let rel = Path::new(rel);
        if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
            return None;
        }
        let bytes = fs::read(dir.join(rel)).ok()?;
        Some((bytes, mime_for(rel).to_string()))
    })
}

fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("woff2") => "font/woff2",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
}

pub fn start(core: Arc<Core>, assets: Option<Assets>, port: Option<u16>) -> Result<ServerHandle> {
    let token = load_or_create_token(&core.paths.data_dir.join("server.token"))?;
    let base = port.unwrap_or(DEFAULT_PORT);
    let mut bound = None;
    for candidate in base..base.saturating_add(10) {
        if let Ok(server) = Server::http(("127.0.0.1", candidate)) {
            bound = Some((server, candidate));
            break;
        }
    }
    let (server, port) = bound.ok_or_else(|| {
        AppError::new(
            "SERVER_BIND",
            format!("no free local port from {base} to {}", base + 9),
        )
    })?;
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let shared_token = token.clone();
    thread::spawn(move || {
        for request in server.incoming_requests() {
            let core = Arc::clone(&core);
            let token = shared_token.clone();
            let assets = assets.clone();
            thread::spawn(move || handle(&core, &token, port, assets.as_ref(), request));
        }
    });
    Ok(ServerHandle { addr, token })
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("static header is valid")
}

fn respond(req: Request, status: u16, content_type: &str, body: Vec<u8>, api: bool) {
    let mut response = Response::from_data(body).with_status_code(StatusCode(status));
    response.add_header(header("Content-Type", content_type));
    response.add_header(header("X-Content-Type-Options", "nosniff"));
    response.add_header(header("Referrer-Policy", "no-referrer"));
    if api {
        response.add_header(header("Cache-Control", "no-store"));
    } else {
        response.add_header(header("Content-Security-Policy", CSP));
        response.add_header(header("Cache-Control", "no-cache"));
    }
    let _ = req.respond(response);
}

fn respond_json(req: Request, status: u16, value: &impl serde::Serialize) {
    let body = serde_json::to_vec(value).unwrap_or_else(|_| b"null".to_vec());
    respond(req, status, "application/json", body, true);
}

fn respond_error(req: Request, status: u16, err: AppError) {
    respond_json(req, status, &err);
}

fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

fn header_value(req: &Request, name: &str) -> Option<String> {
    req.headers()
        .iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str().to_string())
}

fn query_value<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v)
}

fn host_ok(req: &Request, port: u16) -> bool {
    // Rejects DNS-rebinding: only the loopback names we bound are accepted.
    match header_value(req, "Host") {
        Some(host) => host == format!("127.0.0.1:{port}") || host == format!("localhost:{port}"),
        None => false,
    }
}

fn authorized(req: &Request, query: &str, token: &str) -> bool {
    let bearer = header_value(req, "Authorization")
        .and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
    let supplied = bearer.or_else(|| query_value(query, "token").map(str::to_string));
    supplied.is_some_and(|t| same(&t, token))
}

fn handle(core: &Arc<Core>, token: &str, port: u16, assets: Option<&Assets>, mut req: Request) {
    let url = req.url().to_string();
    let (path, query) = url.split_once('?').unwrap_or((url.as_str(), ""));
    let (path, query) = (path.to_string(), query.to_string());
    let method = req.method().clone();

    if !host_ok(&req, port) {
        return respond(req, 403, "text/plain", b"forbidden".to_vec(), true);
    }

    if let Some(id) = path.strip_prefix("/music/") {
        if !matches!(method, Method::Get | Method::Head) {
            return respond(req, 405, "text/plain", b"use GET or HEAD".to_vec(), true);
        }
        let supplied = query_value(&query, "key").unwrap_or_default();
        if uuid::Uuid::parse_str(id).is_err() || !same(supplied, &music_key(token, id)) {
            return respond(req, 403, "text/plain", b"forbidden".to_vec(), true);
        }
        return serve_music(core, req, id);
    }

    if let Some(cmd) = path.strip_prefix("/api/") {
        if !authorized(&req, &query, token) {
            return respond_error(
                req,
                401,
                AppError::new("UNAUTHORIZED", "missing or wrong token"),
            );
        }
        if cmd == "events" && method == Method::Get {
            let after = query_value(&query, "after").and_then(|v| v.parse::<u64>().ok());
            let hub = core.hub();
            let (seq, events) = match after {
                // First call: report where the stream is, do not replay history.
                None => (hub.seq(), Vec::new()),
                Some(after) => hub.wait_after(after, LONG_POLL),
            };
            return respond_json(
                req,
                200,
                &serde_json::json!({ "seq": seq, "events": events }),
            );
        }
        if method != Method::Post {
            return respond_error(req, 405, AppError::new("BAD_REQUEST", "use POST"));
        }
        let mut body = String::new();
        if req
            .as_reader()
            .take(MAX_BODY)
            .read_to_string(&mut body)
            .is_err()
        {
            return respond_error(req, 400, AppError::new("BAD_REQUEST", "unreadable body"));
        }
        let args = if body.trim().is_empty() {
            serde_json::Value::Null
        } else {
            match serde_json::from_str(&body) {
                Ok(v) => v,
                Err(err) => {
                    return respond_error(req, 400, AppError::new("BAD_REQUEST", err.to_string()))
                }
            }
        };
        if cmd == "music_url" {
            let id = args
                .get("track_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            return match core.music_open(id) {
                Ok(_) => respond_json(
                    req,
                    200,
                    &format!("/music/{id}?key={}", music_key(token, id)),
                ),
                Err(e) => respond_error(req, 404, e),
            };
        }
        return match api::dispatch(core, cmd, args) {
            Ok(value) => respond_json(req, 200, &value),
            Err(err) => respond_error(req, 400, err),
        };
    }

    if method != Method::Get {
        return respond(req, 405, "text/plain", b"method not allowed".to_vec(), true);
    }

    if let Some(name) = path.strip_prefix("/art/") {
        if !authorized(&req, &query, token) {
            return respond_error(
                req,
                401,
                AppError::new("UNAUTHORIZED", "missing or wrong token"),
            );
        }
        let clean = !name.is_empty()
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_')
            && !name.starts_with('.');
        if !clean {
            return respond(req, 404, "text/plain", b"not found".to_vec(), true);
        }
        let file = core.paths.cache_dir.join("art").join(name);
        return match fs::read(&file) {
            Ok(bytes) => {
                let mime = mime_for(&file);
                respond(req, 200, mime, bytes, true)
            }
            Err(_) => respond(req, 404, "text/plain", b"not found".to_vec(), true),
        };
    }

    let Some(assets) = assets else {
        return respond(
            req,
            404,
            "text/plain",
            b"This Marquee instance serves no UI. Use the Vite dev server or build the UI first."
                .to_vec(),
            false,
        );
    };
    let rel = path.trim_start_matches('/');
    let rel = if rel.is_empty() { "index.html" } else { rel };
    let found = assets(rel).or_else(|| {
        // Single-page app: unknown routes without a file extension get the shell.
        if Path::new(rel).extension().is_none() {
            assets("index.html")
        } else {
            None
        }
    });
    match found {
        Some((bytes, mime)) => respond(req, 200, &mime, bytes, false),
        None => respond(
            req,
            404,
            "text/plain",
            b"UI files not found. Run `pnpm build` in apps/desktop.".to_vec(),
            false,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    fn request(addr: SocketAddr, raw: &str) -> String {
        let mut stream = TcpStream::connect(addr).unwrap();
        stream.write_all(raw.as_bytes()).unwrap();
        let mut out = String::new();
        let _ = stream.read_to_string(&mut out);
        out
    }

    fn start_test_server(dir: &Path) -> (Arc<Core>, ServerHandle) {
        let core = Arc::new(Core::open_isolated(Some(dir.to_path_buf()), false).unwrap());
        let handle = start(Arc::clone(&core), None, Some(48900)).unwrap();
        (core, handle)
    }

    #[test]
    fn music_capabilities_are_track_scoped_and_support_seek_ranges() {
        let dir = tempfile::tempdir().unwrap();
        let (core, h) = start_test_server(dir.path());
        let source = dir.path().join("audio.wav");
        fs::write(&source, b"RIFF1234WAVEabcdefghijklmnop").unwrap();
        let album = core
            .music_save_album(crate::music::AlbumDraft {
                game_key: "game".into(),
                album_id: None,
                title: "OST".into(),
                artist: String::new(),
                source_url: String::new(),
            })
            .unwrap();
        core.music_import(&album, vec![source]).unwrap();
        let id = core.music_albums(None).unwrap()[0].tracks[0].id.clone();
        let url = h.music_url(&id);
        let path = url.strip_prefix(&format!("http://{}", h.addr)).unwrap();
        let get = |path: &str, range: &str| {
            request(
                h.addr,
                &format!(
                    "GET {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n{range}\r\n",
                    h.addr
                ),
            )
        };
        let partial = get(path, "Range: bytes=12-15\r\n");
        assert!(partial.starts_with("HTTP/1.1 206"));
        assert!(partial.contains("bytes 12-15/28"));
        assert!(partial.ends_with("abcd"));
        assert!(get(path, "Range: bytes=99-\r\n").starts_with("HTTP/1.1 416"));
        assert!(get(path, "Range: bytes=-4\r\n").ends_with("mnop"));
        assert!(get(&format!("/music/{id}?key=bad"), "").starts_with("HTTP/1.1 403"));
        assert!(
            get(&path.replace(&id, &Uuid::new_v4().to_string()), "").starts_with("HTTP/1.1 403")
        );
        assert!(!url.contains(&h.token));
        let unauthorized=request(h.addr,&format!("POST /api/music_albums HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",h.addr,music_key(&h.token,&id)));
        assert!(unauthorized.starts_with("HTTP/1.1 401"));
        assert_eq!(audio_range(Some("bytes=1-2,4-5"), 28), None);
    }

    #[test]
    fn api_requires_the_token_and_the_right_host() {
        let dir = tempfile::tempdir().unwrap();
        let (_core, h) = start_test_server(dir.path());
        let host = h.addr;
        let post = |extra: &str, host_header: &str, path: &str| {
            request(
                host,
                &format!(
                    "POST {path} HTTP/1.1\r\nHost: {host_header}\r\n{extra}Content-Length: 0\r\nConnection: close\r\n\r\n"
                ),
            )
        };
        let own = format!("127.0.0.1:{}", host.port());
        assert!(post("", &own, "/api/health").starts_with("HTTP/1.1 401"));
        assert!(
            post("Authorization: Bearer nope\r\n", &own, "/api/health").starts_with("HTTP/1.1 401")
        );
        assert!(post(
            "Authorization: Bearer x\r\n",
            "evil.example:80",
            "/api/health"
        )
        .starts_with("HTTP/1.1 403"));
        let ok = post(
            &format!("Authorization: Bearer {}\r\n", h.token),
            &own,
            "/api/health",
        );
        assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
        assert!(ok.contains("ok:0:"));
    }

    #[test]
    fn unknown_commands_and_art_traversal_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let (_core, h) = start_test_server(dir.path());
        let own = format!("127.0.0.1:{}", h.addr.port());
        let unknown = request(
            h.addr,
            &format!(
                "POST /api/rm_rf HTTP/1.1\r\nHost: {own}\r\nAuthorization: Bearer {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                h.token
            ),
        );
        assert!(unknown.starts_with("HTTP/1.1 400") && unknown.contains("BAD_REQUEST"));
        let traversal = request(
            h.addr,
            &format!(
                "GET /art/..%2f..%2flibrary.db?token={} HTTP/1.1\r\nHost: {own}\r\nConnection: close\r\n\r\n",
                h.token
            ),
        );
        assert!(traversal.starts_with("HTTP/1.1 404"));
    }

    #[test]
    fn token_is_stable_across_runs_and_private() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("server.token");
        let first = load_or_create_token(&path).unwrap();
        assert_eq!(first, load_or_create_token(&path).unwrap());
        assert!(first.len() >= 32);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o077, 0);
        }
    }
}
