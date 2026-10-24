//! Explicit soundtrack attachments. Owned audio never enters the game scanner.
use crate::error::{AppError, Result};
use crate::Core;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_FILE: u64 = 512 * 1024 * 1024;
const MAX_BATCH: u64 = 1024 * 1024 * 1024;

pub fn ensure_tables(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS music_albums (id TEXT PRIMARY KEY, title TEXT NOT NULL, artist TEXT NOT NULL, source_url TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS music_links (game_key TEXT NOT NULL, album_id TEXT NOT NULL REFERENCES music_albums(id), PRIMARY KEY(game_key,album_id));
        CREATE TABLE IF NOT EXISTS music_tracks (id TEXT PRIMARY KEY, album_id TEXT NOT NULL REFERENCES music_albums(id), title TEXT NOT NULL, media TEXT NOT NULL, position INTEGER NOT NULL, UNIQUE(album_id,media));")?;
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub available: bool,
}

#[derive(Debug, Serialize)]
pub struct Album {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub source_url: String,
    pub tracks: Vec<Track>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlbumDraft {
    pub game_key: String,
    pub album_id: Option<String>,
    pub title: String,
    pub artist: String,
    pub source_url: String,
}

fn text(value: &str, max: usize, required: bool) -> Result<()> {
    if value.len() > max
        || (required && value.trim().is_empty())
        || value.chars().any(char::is_control)
    {
        return Err(AppError::new(
            "MUSIC_INPUT",
            "Soundtrack details are empty, too long or contain control characters",
        ));
    }
    Ok(())
}

fn validate_url(value: &str) -> Result<()> {
    if value.is_empty() {
        return Ok(());
    }
    let parsed = ureq::get(value)
        .request_url()
        .map_err(|_| AppError::new("MUSIC_URL", "Enter an HTTPS source page URL"))?;
    if parsed.scheme() != "https"
        || !parsed.as_url().username().is_empty()
        || parsed.as_url().password().is_some()
    {
        return Err(AppError::new(
            "MUSIC_URL",
            "Use an HTTPS source page without embedded credentials",
        ));
    }
    Ok(())
}

fn search_url(title: &str) -> Result<String> {
    text(title, 300, true)?;
    let query = format!("{title} official soundtrack OST");
    Ok(ureq::get("https://www.youtube.com/results")
        .query("search_query", &query)
        .request_url()
        .map_err(|_| AppError::new("MUSIC_URL", "Cannot build soundtrack search"))?
        .as_url()
        .to_string())
}

fn open_browser(url: &str) -> Result<()> {
    validate_url(url)?;
    let binary = crate::runners::which("xdg-open")
        .ok_or_else(|| AppError::new("MUSIC_BROWSER", "No browser opener found"))?;
    let mut child = std::process::Command::new(binary)
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

fn media_name(value: &str) -> Result<&'static str> {
    let (hash, ext) = value
        .split_once('.')
        .ok_or_else(|| AppError::new("MUSIC_MEDIA", "Invalid music identity"))?;
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(AppError::new("MUSIC_MEDIA", "Invalid music identity"));
    }
    match ext {
        "mp3" => Ok("audio/mpeg"),
        "flac" => Ok("audio/flac"),
        "ogg" => Ok("audio/ogg"),
        "wav" => Ok("audio/wav"),
        _ => Err(AppError::new("MUSIC_MEDIA", "Unsupported audio format")),
    }
}

fn storage(root: &Path) -> Result<PathBuf> {
    let mut path = root.to_path_buf();
    for part in ["music", "media"] {
        path.push(part);
        if !path.exists() {
            fs::create_dir(&path)?;
        }
        if !fs::symlink_metadata(&path)?.is_dir() {
            return Err(AppError::new(
                "MUSIC_PATH",
                "The music folder was redirected",
            ));
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(path)
}

fn open_regular(path: &Path) -> Result<File> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(AppError::new("MUSIC_FILE", "Choose a regular audio file"));
    }
    Ok(file)
}

fn copy_audio(source: &Path, destination: &Path, total: &mut u64) -> Result<(String, String)> {
    if !source.is_absolute() {
        return Err(AppError::new(
            "MUSIC_FILE",
            "Choose an absolute local audio path",
        ));
    }
    let ext = source
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    media_name(&format!("{}.{}", "0".repeat(64), ext))?;
    let mut input = open_regular(source)?;
    let size = input.metadata()?.len();
    if size == 0 || size > MAX_FILE || total.saturating_add(size) > MAX_BATCH {
        return Err(AppError::new(
            "MUSIC_SIZE",
            "Limit audio to 512 MiB per file and 1 GiB per import",
        ));
    }
    let mut head = [0; 12];
    let count = input.read(&mut head)?;
    let valid = match ext.as_str() {
        "mp3" => {
            head.starts_with(b"ID3") || (count >= 2 && head[0] == 0xff && head[1] & 0xe0 == 0xe0)
        }
        "flac" => head.starts_with(b"fLaC"),
        "ogg" => head.starts_with(b"OggS"),
        "wav" => head.starts_with(b"RIFF") && &head[8..12] == b"WAVE",
        _ => false,
    };
    if !valid {
        return Err(AppError::new(
            "MUSIC_FORMAT",
            "The file header does not match its audio extension",
        ));
    }
    input.seek(SeekFrom::Start(0))?;
    let mut temp = tempfile::NamedTempFile::new_in(destination)?;
    let mut hash = Sha256::new();
    let mut copied = 0;
    let mut buffer = [0; 65536];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        copied += count as u64;
        if copied > MAX_FILE || total.saturating_add(copied) > MAX_BATCH {
            return Err(AppError::new(
                "MUSIC_SIZE",
                "Audio grew beyond the import limit",
            ));
        }
        hash.update(&buffer[..count]);
        temp.write_all(&buffer[..count])?;
    }
    *total += copied;
    let name = format!("{:x}.{ext}", hash.finalize());
    let target = destination.join(&name);
    match temp.persist_noclobber(&target) {
        Ok(_) => (),
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
            let mut existing = open_regular(&target)?;
            if existing.metadata()?.len() != copied {
                return Err(AppError::new(
                    "MUSIC_INTEGRITY",
                    "Stored music has an unexpected size",
                ));
            }
            let mut hash = Sha256::new();
            loop {
                let n = existing.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hash.update(&buffer[..n]);
            }
            if format!("{:x}.{ext}", hash.finalize()) != name {
                return Err(AppError::new(
                    "MUSIC_INTEGRITY",
                    "Stored music does not match its identity",
                ));
            }
        }
        Err(e) => return Err(e.error.into()),
    }
    Ok((
        name,
        source
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .chars()
            .take(300)
            .filter(|c| !c.is_control())
            .collect(),
    ))
}

impl Core {
    pub fn music_find(&self, title: &str) -> Result<()> {
        open_browser(&search_url(title)?)
    }

    pub fn music_open_source(&self, album: &str) -> Result<()> {
        let url = self.with_db(|db| {
            Ok(db.conn.query_row(
                "SELECT source_url FROM music_albums WHERE id=?1",
                [album],
                |r| r.get::<_, String>(0),
            )?)
        })?;
        if url.is_empty() {
            return Err(AppError::new("MUSIC_URL", "This album has no source page"));
        }
        open_browser(&url)
    }
    pub fn music_albums(&self, game_key: Option<&str>) -> Result<Vec<Album>> {
        self.with_db(|db| {
            let mut stmt = db.conn.prepare("SELECT id,title,artist,source_url FROM music_albums WHERE ?1 IS NULL OR id IN (SELECT album_id FROM music_links WHERE game_key=?1) ORDER BY title,id")?;
            let rows = stmt.query_map([game_key], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)))?;
            let mut albums = vec![];
            for row in rows {
                let (id,title,artist,source_url) = row?;
                let mut tracks = db.conn.prepare("SELECT id,title,media FROM music_tracks WHERE album_id=?1 ORDER BY position,id")?;
                let tracks = tracks.query_map([&id], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?)))?.collect::<std::result::Result<Vec<_>,_>>()?;
                albums.push(Album { id,title,artist,source_url,tracks:tracks.into_iter().map(|(id,title,media)| Track { id,title,available:self.music_open_name(&media).is_ok() }).collect() });
            }
            Ok(albums)
        })
    }

    pub fn music_save_album(&self, draft: AlbumDraft) -> Result<String> {
        text(&draft.game_key, 512, true)?;
        text(&draft.title, 300, true)?;
        text(&draft.artist, 300, false)?;
        text(&draft.source_url, 2048, false)?;
        validate_url(&draft.source_url)?;
        let id = draft.album_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        Uuid::parse_str(&id).map_err(|_| AppError::new("MUSIC_INPUT", "Invalid album identity"))?;
        self.with_db(|db| {
            let tx = db.conn.unchecked_transaction()?;
            tx.execute("INSERT INTO music_albums(id,title,artist,source_url) VALUES(?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET title=excluded.title,artist=excluded.artist,source_url=excluded.source_url",params![id,draft.title.trim(),draft.artist.trim(),draft.source_url])?;
            tx.execute("INSERT OR IGNORE INTO music_links(game_key,album_id) VALUES(?1,?2)",params![draft.game_key,id])?;
            tx.commit()?; Ok(id)
        })
    }

    pub fn music_link(&self, game_key: &str, album: &str, linked: bool) -> Result<()> {
        text(game_key, 512, true)?;
        self.with_db(|db| {
            if linked {
                db.conn.execute(
                    "INSERT OR IGNORE INTO music_links(game_key,album_id) VALUES(?1,?2)",
                    params![game_key, album],
                )?;
            } else {
                db.conn.execute(
                    "DELETE FROM music_links WHERE game_key=?1 AND album_id=?2",
                    params![game_key, album],
                )?;
            }
            Ok(())
        })
    }

    pub fn music_import(&self, album: &str, paths: Vec<PathBuf>) -> Result<usize> {
        if paths.is_empty() || paths.len() > 200 {
            return Err(AppError::new(
                "MUSIC_INPUT",
                "Choose between 1 and 200 audio files",
            ));
        }
        self.with_db(|db| {
            let exists: bool = db.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM music_albums WHERE id=?1)",
                [album],
                |r| r.get(0),
            )?;
            if !exists {
                return Err(AppError::new(
                    "MUSIC_ALBUM",
                    "Select an existing soundtrack",
                ));
            }
            Ok(())
        })?;
        let destination = storage(&self.paths.data_dir)?;
        let mut total = 0;
        let mut copied = vec![];
        for source in paths {
            copied.push(copy_audio(&source, &destination, &mut total)?);
        }
        self.with_db(|db| {
            let tx=db.conn.unchecked_transaction()?;
            let start: i64=tx.query_row("SELECT COALESCE(MAX(position)+1,0) FROM music_tracks WHERE album_id=?1",[album],|r|r.get(0))?;
            let mut added=0;
            for (i,(media,title)) in copied.iter().enumerate() { added+=tx.execute("INSERT OR IGNORE INTO music_tracks(id,album_id,title,media,position) VALUES(?1,?2,?3,?4,?5)",params![Uuid::new_v4().to_string(),album,title,media,start+i as i64])?; }
            tx.commit()?; Ok(added)
        })
    }

    pub fn music_remove_track(&self, album: &str, track: &str) -> Result<()> {
        self.with_db(|db| {
            db.conn.execute(
                "DELETE FROM music_tracks WHERE album_id=?1 AND id=?2",
                params![album, track],
            )?;
            Ok(())
        })
    }

    fn music_open_name(&self, name: &str) -> Result<(File, &'static str)> {
        let mime = media_name(name)?;
        let directory = storage(&self.paths.data_dir)?;
        Ok((open_regular(&directory.join(name))?, mime))
    }

    pub fn music_open(&self, track: &str) -> Result<(File, &'static str)> {
        let name = self
            .with_db(|db| {
                Ok(db
                    .conn
                    .query_row("SELECT media FROM music_tracks WHERE id=?1", [track], |r| {
                        r.get::<_, String>(0)
                    })
                    .optional()?)
            })?
            .ok_or_else(|| AppError::new("MUSIC_MISSING", "Track is no longer attached"))?;
        self.music_open_name(&name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn wav(path: &Path) {
        let mut content = b"RIFF\x24\x00\x00\x00WAVEfmt ".to_vec();
        content.extend_from_slice(&[0; 32]);
        fs::write(path, content).unwrap();
    }
    fn album(core: &Core) -> String {
        core.music_save_album(AlbumDraft {
            game_key: "game-one".into(),
            album_id: None,
            title: "Original soundtrack".into(),
            artist: "Composer".into(),
            source_url: "https://example.com/album".into(),
        })
        .unwrap()
    }
    #[test]
    fn copied_tracks_deduplicate_and_links_survive_restart_without_scanning_audio_as_games() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("library");
        let core = Core::open_isolated(Some(root.clone()), true).unwrap();
        let id = album(&core);
        let source = dir.path().join("01 Opening.wav");
        wav(&source);
        assert_eq!(
            core.music_import(&id, vec![source.clone(), source.clone()])
                .unwrap(),
            1
        );
        core.music_link("game-two", &id, true).unwrap();
        core.music_link("game-one", &id, false).unwrap();
        assert!(core.music_albums(Some("game-one")).unwrap().is_empty());
        let track = core.music_albums(Some("game-two")).unwrap()[0].tracks[0]
            .id
            .clone();
        fs::remove_file(source).unwrap();
        drop(core);
        let core = Core::open_isolated(Some(root), true).unwrap();
        let albums = core.music_albums(Some("game-two")).unwrap();
        assert!(core.snapshot().unwrap().editions.is_empty());
        assert_eq!(albums[0].tracks.len(), 1);
        assert!(albums[0].tracks[0].available);
        assert_eq!(albums[0].tracks[0].title, "01 Opening");
        assert!(core.music_open("../../config").is_err());
        core.music_remove_track(&id, &track).unwrap();
        assert!(core.music_open(&track).is_err());
        assert_eq!(
            fs::read_dir(core.paths.data_dir.join("music/media"))
                .unwrap()
                .count(),
            1
        );
    }
    #[test]
    fn invalid_batches_do_not_publish_partial_tracks_and_symlinks_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(dir.path().join("library")), true).unwrap();
        let id = album(&core);
        let good = dir.path().join("good.wav");
        wav(&good);
        let bad = dir.path().join("fake.mp3");
        fs::write(&bad, b"<html>not audio</html>").unwrap();
        assert!(core.music_import(&id, vec![good.clone(), bad]).is_err());
        assert!(core.music_albums(None).unwrap()[0].tracks.is_empty());
        let link = dir.path().join("link.wav");
        std::os::unix::fs::symlink(&good, &link).unwrap();
        assert!(core.music_import(&id, vec![link]).is_err());
        let oversized = dir.path().join("huge.wav");
        File::create(&oversized)
            .unwrap()
            .set_len(MAX_FILE + 1)
            .unwrap();
        assert!(core.music_import(&id, vec![oversized]).is_err());
        assert!(core
            .music_import(&id, vec![PathBuf::from("relative.wav")])
            .is_err());
    }
    #[test]
    fn source_links_reject_active_schemes_and_credentials_and_missing_media_keeps_album() {
        let dir = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(dir.path().join("library")), true).unwrap();
        for source in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "http://example.com",
            "https://user:secret@example.com/album",
        ] {
            assert!(validate_url(source).is_err());
        }
        let url = search_url("Sonic & Knuckles #1").unwrap();
        assert!(url.starts_with("https://www.youtube.com/results?search_query="));
        assert!(url.contains("%26"));
        assert!(url.contains("%23"));
        let id = album(&core);
        let source = dir.path().join("a.wav");
        wav(&source);
        core.music_import(&id, vec![source]).unwrap();
        let file = fs::read_dir(core.paths.data_dir.join("music/media"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        fs::remove_file(file).unwrap();
        let albums = core.music_albums(None).unwrap();
        assert_eq!(albums.len(), 1);
        assert!(!albums[0].tracks[0].available);
    }
}
