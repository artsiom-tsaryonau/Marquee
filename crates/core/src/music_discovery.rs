//! Bounded, cookie-free YouTube discovery. No media download or browser access.
use crate::error::{AppError, Result};
use crate::Core;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const VERSION: &str = "2026.10.21";
const SHA256: &str = "d8bd33fca9d5b23bd5b9952b3fca36db5c46331933fda9f598a3da14ce2c243f";
const MAX_HELPER: u64 = 64 * 1024 * 1024;
const MAX_JSON: u64 = 2 * 1024 * 1024;
static DISCOVERY: Mutex<()> = Mutex::new(());

fn error(message: &str) -> AppError {
    AppError::new("MUSIC_DISCOVERY", message)
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Playlist,
    Video,
}
#[derive(Debug, Clone, Serialize)]
pub struct Source {
    pub id: String,
    pub title: String,
    pub uploader: String,
    pub url: String,
    pub kind: Kind,
    pub duration: Option<f64>,
}
#[derive(Debug, Serialize)]
pub struct Playlist {
    pub title: String,
    pub uploader: String,
    pub url: String,
    pub tracks: Vec<Source>,
    pub truncated: bool,
}
fn text(value: &str, max: usize) -> Result<&str> {
    let value = value.trim();
    if value.len() < 2 || value.len() > max || value.chars().any(char::is_control) {
        return Err(error(
            "Enter between 2 and 200 characters without control characters",
        ));
    }
    Ok(value)
}
fn valid_id(id: &str, kind: Kind) -> bool {
    let length = match kind {
        Kind::Playlist => (10..=128).contains(&id.len()),
        Kind::Video => id.len() == 11,
    };
    length
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
}
fn source_url(id: &str, kind: Kind) -> String {
    match kind {
        Kind::Playlist => format!("https://www.youtube.com/playlist?list={id}"),
        Kind::Video => format!("https://www.youtube.com/watch?v={id}"),
    }
}
fn playlist_id(url: &str) -> Result<&str> {
    let id = url
        .strip_prefix("https://www.youtube.com/playlist?list=")
        .filter(|id| valid_id(id, Kind::Playlist))
        .ok_or_else(|| error("Choose a playlist returned by YouTube search"))?;
    Ok(id)
}
fn canonical_source(url: &str) -> Result<String> {
    if let Ok(id) = playlist_id(url) {
        return Ok(source_url(id, Kind::Playlist));
    }
    let id = url
        .strip_prefix("https://www.youtube.com/watch?v=")
        .filter(|id| valid_id(id, Kind::Video))
        .ok_or_else(|| error("Choose a source returned by YouTube search"))?;
    Ok(source_url(id, Kind::Video))
}
fn clean(value: &Value, max: usize) -> String {
    value
        .as_str()
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .take(max)
        .collect::<String>()
        .trim()
        .to_string()
}
fn sources(json: &Value, kind: Kind, max: usize) -> Vec<Source> {
    json["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .take(max)
        .filter_map(|entry| {
            let id = entry["id"].as_str()?;
            if !valid_id(id, kind) {
                return None;
            }
            let title = clean(&entry["title"], 300);
            if title.is_empty() {
                return None;
            }
            Some(Source {
                id: id.into(),
                title,
                uploader: clean(&entry["uploader"], 300),
                url: source_url(id, kind),
                kind,
                duration: entry["duration"]
                    .as_f64()
                    .filter(|n| n.is_finite() && *n > 0.0 && *n <= 604800.0),
            })
        })
        .collect()
}
fn verify(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_HELPER {
        return Err(error("YouTube search helper is not a regular bounded file"));
    }
    let mut hash = Sha256::new();
    std::io::copy(&mut File::open(path)?.take(MAX_HELPER + 1), &mut hash)?;
    if format!("{:x}", hash.finalize()) != SHA256 {
        return Err(error("YouTube search helper checksum failed"));
    }
    Ok(())
}
fn helper(data: &Path) -> Result<PathBuf> {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Err(error("Managed YouTube search currently supports Linux x86-64. Use the browser search link on this host."));
    }
    let dir = data.join("music-tools/yt-dlp").join(VERSION);
    let path = dir.join("yt-dlp");
    if fs::symlink_metadata(&path).is_ok() {
        verify(&path)?;
        return Ok(path);
    }
    fs::create_dir_all(&dir)?;
    let staging = tempfile::NamedTempFile::new_in(&dir)?;
    let hash = crate::http::download_file(
        &format!("https://github.com/yt-dlp/yt-dlp/releases/download/{VERSION}/yt-dlp_linux"),
        staging.path(), MAX_HELPER, Duration::from_secs(180), |_| {},
    ).map_err(|_| error("Could not install the verified YouTube search helper. Check your connection or use browser search."))?;
    if hash != SHA256 {
        return Err(error("Downloaded YouTube search helper checksum failed"));
    }
    fs::set_permissions(staging.path(), fs::Permissions::from_mode(0o700))?;
    staging
        .persist_noclobber(&path)
        .map_err(|_| error("Could not publish the YouTube search helper"))?;
    Ok(path)
}
struct Child(std::process::Child);
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn inspect(helper: &Path, source: &str, limit: usize) -> Result<Value> {
    let mut output = tempfile::tempfile()?;
    let stderr = tempfile::tempfile()?;
    let mut child = Child(
        Command::new(helper)
            .args([
                "--ignore-config",
                "--no-cookies",
                "--no-cache-dir",
                "--no-js-runtimes",
                "--no-plugin-dirs",
                "--no-remote-components",
                "--flat-playlist",
                "--skip-download",
                "--dump-single-json",
                "--socket-timeout",
                "12",
                "--retries",
                "0",
                "--extractor-retries",
                "0",
                "--playlist-end",
                &limit.to_string(),
                "--",
                source,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::from(output.try_clone()?))
            .stderr(Stdio::from(stderr.try_clone()?))
            .spawn()?,
    );
    let started = Instant::now();
    loop {
        if started.elapsed() > Duration::from_secs(45)
            || output.metadata()?.len() > MAX_JSON
            || stderr.metadata()?.len() > 64 * 1024
        {
            return Err(error(
                "YouTube discovery exceeded its time or response limit. Try a narrower query.",
            ));
        }
        if let Some(status) = child.0.try_wait()? {
            if !status.success() {
                return Err(error("YouTube discovery failed. Try browser search; sign-in or source restrictions may prevent inspection."));
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    output.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    output.take(MAX_JSON + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_JSON {
        return Err(error("YouTube response exceeded its size limit"));
    }
    serde_json::from_slice(&bytes).map_err(|_| error("YouTube returned unreadable metadata"))
}
impl Core {
    pub fn music_open_youtube(&self, url: &str) -> Result<()> {
        crate::music::open_browser(&canonical_source(url)?)
    }
    pub fn music_link_youtube(&self, game_key: &str, url: &str, title: &str) -> Result<String> {
        if game_key.len() > 512 || game_key.chars().any(char::is_control) {
            return Err(error("Invalid game identity"));
        }
        let title = text(title, 300)?;
        let url = canonical_source(url)?;
        self.with_db(|db| {
            let tx = db.conn.unchecked_transaction()?;
            let existing: Option<String> = tx.query_row("SELECT id FROM music_albums WHERE source_url=?1 ORDER BY id LIMIT 1", [&url], |r| r.get(0)).optional()?;
            let id = existing.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            tx.execute("INSERT OR IGNORE INTO music_albums(id,title,artist,source_url) VALUES(?1,?2,'',?3)", params![id,title,url])?;
            if !game_key.is_empty() { tx.execute("INSERT OR IGNORE INTO music_links(game_key,album_id) VALUES(?1,?2)", params![game_key,id])?; }
            tx.commit()?;
            Ok(id)
        })
    }
    pub fn music_search_youtube(&self, query: &str, kind: Kind) -> Result<Vec<Source>> {
        let query = text(query, 200)?;
        let _guard = DISCOVERY
            .try_lock()
            .map_err(|_| error("Another YouTube discovery request is running"))?;
        let source = match kind {
            Kind::Playlist => format!(
                "https://www.youtube.com/results?search_query={}&sp=EgIQAw%3D%3D",
                crate::romart::encode(query)
            ),
            Kind::Video => format!("ytsearch8:{query}"),
        };
        let json = inspect(&helper(&self.paths.data_dir)?, &source, 8)?;
        Ok(sources(&json, kind, 8))
    }
    pub fn music_inspect_youtube(&self, url: &str) -> Result<Playlist> {
        let id = playlist_id(url)?;
        let _guard = DISCOVERY
            .try_lock()
            .map_err(|_| error("Another YouTube discovery request is running"))?;
        let url = source_url(id, Kind::Playlist);
        let json = inspect(&helper(&self.paths.data_dir)?, &url, 51)?;
        let title = clean(&json["title"], 300);
        if title.is_empty() {
            return Err(error("The playlist has no readable title"));
        }
        Ok(Playlist {
            title,
            uploader: clean(&json["uploader"], 300),
            url,
            tracks: sources(&json, Kind::Video, 50),
            truncated: json["entries"].as_array().is_some_and(|e| e.len() > 50),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn youtube_sources_never_use_extractor_urls_and_reject_noncanonical_input() {
        for bad in [
            "http://www.youtube.com/playlist?list=PL1234567890",
            "https://www.youtube.com/playlist?list=PL1234567890&next=evil",
            "https://www.youtube.com@evil/playlist?list=PL1234567890",
            "file:///etc/passwd",
            "--exec=bad",
        ] {
            assert!(playlist_id(bad).is_err());
        }
        assert!(text("bad\nquery", 200).is_err());
        let data = serde_json::json!({"entries":[{"id":"PL1234567890","title":"Game OST","uploader":"An uploader","url":"https://evil.test"},{"id":"bad/id","title":"Invalid"}]});
        let results = sources(&data, Kind::Playlist, 8);
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].url,
            "https://www.youtube.com/playlist?list=PL1234567890"
        );
        let root = tempfile::tempdir().unwrap();
        let binary = root.path().join("helper");
        fs::write(&binary, "wrong binary").unwrap();
        assert!(verify(&binary).is_err());
        std::os::unix::fs::symlink(&binary, root.path().join("linked")).unwrap();
        assert!(verify(&root.path().join("linked")).is_err());
    }
    #[test]
    fn linked_sources_are_idempotent_and_keep_existing_album_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(temp.path().join("data")), false).unwrap();
        let url = "https://www.youtube.com/playlist?list=PL1234567890";
        let id = core
            .music_link_youtube("game", url, "Game soundtrack")
            .unwrap();
        core.music_save_album(crate::music::AlbumDraft {
            game_key: "game".into(),
            album_id: Some(id.clone()),
            title: "My album title".into(),
            artist: "My composer".into(),
            source_url: url.into(),
        })
        .unwrap();
        assert_eq!(
            core.music_link_youtube("game", url, "Provider title")
                .unwrap(),
            id
        );
        assert_eq!(
            core.music_link_youtube("other", url, "Provider title")
                .unwrap(),
            id
        );
        let albums = core.music_albums(None).unwrap();
        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].title, "My album title");
        assert_eq!(albums[0].artist, "My composer");
        assert_eq!(albums[0].game_keys.len(), 2);
        assert!(albums[0].tracks.is_empty());
        assert!(core
            .music_link_youtube("game", "file:///tmp/file", "Bad source")
            .is_err());
    }
    #[test]
    fn helper_has_a_bounded_lifecycle_and_never_downloads_media() {
        let root = tempfile::tempdir().unwrap();
        let binary = root.path().join("fixture");
        fs::write(&binary, "#!/bin/sh\nfor arg do case \"$arg\" in --skip-download) skip=1;; --no-cookies) cookies=1;; --flat-playlist) flat=1;; esac; done\n[ \"$skip$cookies$flat\" = 111 ] || exit 1\nprintf '{\"entries\":[]}'\n").unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(inspect(&binary, "ytsearch8:test", 8).unwrap()["entries"]
            .as_array()
            .unwrap()
            .is_empty());
        fs::write(&binary, "#!/bin/sh\nhead -c 2200000 /dev/zero\n").unwrap();
        assert!(inspect(&binary, "test", 8).is_err());
    }
}
