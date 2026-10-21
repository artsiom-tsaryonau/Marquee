use crate::error::{AppError, Result};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

const USER_AGENT: &str = "Marquee/0.0 (https://github.com/artsiom-tsaryonau/Marquee)";
pub const COVER_MAX_BYTES: u64 = 8 * 1024 * 1024;
pub const JSON_MAX_BYTES: u64 = 2 * 1024 * 1024;
pub const APPIMAGE_MAX_BYTES: u64 = 512 * 1024 * 1024;
pub const PORT_MAX_BYTES: u64 = 1024 * 1024 * 1024;

pub fn get_bytes(url: &str) -> Result<Vec<u8>> {
    read_limited(url, COVER_MAX_BYTES, Duration::from_secs(45))
}

pub fn get_json(url: &str) -> Result<serde_json::Value> {
    let bytes = read_limited(url, JSON_MAX_BYTES, Duration::from_secs(45))?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// Stream `url` to `dest`, hashing as we go. Returns lowercase sha256 hex.
pub fn download_file(
    url: &str,
    dest: &Path,
    max_bytes: u64,
    timeout: Duration,
    mut on_progress: impl FnMut(u64),
) -> Result<String> {
    let result = stream_to_file(url, dest, max_bytes, timeout, &mut on_progress);
    if result.is_err() {
        let _ = std::fs::remove_file(dest);
    }
    result
}

fn stream_to_file(
    url: &str,
    dest: &Path,
    max_bytes: u64,
    timeout: Duration,
    on_progress: &mut impl FnMut(u64),
) -> Result<String> {
    let resp = call(url, timeout)?;
    let mut reader = resp.into_reader();
    let mut file = File::create(dest)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > max_bytes {
            let _ = std::fs::remove_file(dest);
            return Err(AppError::new(
                "NETWORK",
                format!("download exceeded {} MiB", max_bytes / (1024 * 1024)),
            ));
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n])?;
        on_progress(total);
    }
    file.sync_all()?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn read_limited(url: &str, max_bytes: u64, timeout: Duration) -> Result<Vec<u8>> {
    let resp = call(url, timeout)?;
    let mut buf = Vec::new();
    resp.into_reader()
        .take(max_bytes + 1)
        .read_to_end(&mut buf)?;
    if buf.len() as u64 > max_bytes {
        return Err(AppError::new(
            "NETWORK",
            format!("download exceeded {} MiB", max_bytes / (1024 * 1024)),
        ));
    }
    Ok(buf)
}

fn call(url: &str, timeout: Duration) -> Result<ureq::Response> {
    let mut req = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .timeout(timeout);
    if url.contains("api.github.com") {
        req = req
            .set("Accept", "application/vnd.github+json")
            .set("X-GitHub-Api-Version", "2022-11-28");
    }
    let resp = req.call().map_err(|err| match err {
        ureq::Error::Status(404, _) => AppError::new("NOT_FOUND", format!("not found: {url}")),
        other => AppError::new("NETWORK", other.to_string()),
    })?;
    let status = resp.status();
    if status == 404 {
        return Err(AppError::new("NOT_FOUND", format!("not found: {url}")));
    }
    if !(200..300).contains(&status) {
        return Err(AppError::new("NETWORK", format!("HTTP {status} for {url}")));
    }
    Ok(resp)
}

/// GET JSON with a bearer token (used for provider APIs that need a key).
pub fn get_json_bearer(url: &str, token: &str) -> Result<serde_json::Value> {
    let resp = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .set("Authorization", &format!("Bearer {token}"))
        .timeout(Duration::from_secs(30))
        .call()
        .map_err(|err| match err {
            ureq::Error::Status(401 | 403, _) => {
                AppError::new("NETWORK", "The provider rejected the API key")
            }
            ureq::Error::Status(404, _) => AppError::new("NOT_FOUND", format!("not found: {url}")),
            other => AppError::new("NETWORK", other.to_string()),
        })?;
    let mut buf = Vec::new();
    resp.into_reader()
        .take(JSON_MAX_BYTES + 1)
        .read_to_end(&mut buf)?;
    if buf.len() as u64 > JSON_MAX_BYTES {
        return Err(AppError::new("NETWORK", "response too large"));
    }
    Ok(serde_json::from_slice(&buf)?)
}

/// Download a text file (for example a database list) up to `max_bytes`.
pub fn get_text_limited(url: &str, max_bytes: u64) -> Result<String> {
    let bytes = read_limited(url, max_bytes, Duration::from_secs(120))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
