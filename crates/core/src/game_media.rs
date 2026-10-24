//! Explicit local game extras. Disc inspection reads directories only; importing
//! copies just the selected member. Media never becomes a playable game.
use crate::{
    error::{AppError, Result},
    Core,
};
use rusqlite::params;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};
use uuid::Uuid;
const MAX_VIDEO: u64 = 2 * 1024 * 1024 * 1024;
fn err(s: &str) -> AppError {
    AppError::new("GAME_MEDIA", s)
}
fn regular(p: &Path) -> Result<File> {
    if !p.is_absolute() {
        return Err(err("Choose an absolute path on the Marquee host"));
    }
    let f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(p)?;
    if !f.metadata()?.is_file() {
        return Err(err("Choose a regular file, not a link or device"));
    }
    Ok(f)
}
fn format(path: &str) -> Option<(&'static str, &'static str, bool)> {
    match path.rsplit('.').next()?.to_ascii_lowercase().as_str() {
        "png" => Some(("image", "image/png", true)),
        "jpg" | "jpeg" => Some(("image", "image/jpeg", true)),
        "mp4" => Some(("video", "video/mp4", true)),
        "webm" => Some(("video", "video/webm", true)),
        "m2v" => Some(("video", "video/mpeg", false)),
        _ => None,
    }
}
fn title(s: &str) -> String {
    Path::new(s)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .replace(['_', '-'], " ")
}
fn validate(s: &str) -> Result<()> {
    if s.trim().is_empty() || s.len() > 300 || s.chars().any(char::is_control) {
        Err(err(
            "Enter a title or game identity of at most 300 characters",
        ))
    } else {
        Ok(())
    }
}
pub fn ensure_tables(c: &rusqlite::Connection) -> Result<()> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS game_media_collections(id TEXT PRIMARY KEY,title TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS game_media_links(collection_id TEXT NOT NULL REFERENCES game_media_collections(id),game_key TEXT NOT NULL,PRIMARY KEY(collection_id,game_key));
    CREATE TABLE IF NOT EXISTS game_media_assets(id TEXT PRIMARY KEY,collection_id TEXT NOT NULL REFERENCES game_media_collections(id),title TEXT NOT NULL,kind TEXT NOT NULL,mime TEXT NOT NULL,media TEXT NOT NULL,bytes INTEGER NOT NULL,source TEXT NOT NULL,spoiler INTEGER NOT NULL,position REAL NOT NULL DEFAULT 0,UNIQUE(collection_id,media));")?;
    Ok(())
}
#[derive(Serialize)]
pub struct Asset {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub bytes: u64,
    pub source: String,
    pub spoiler: bool,
    pub position: f64,
    pub available: bool,
}
#[derive(Serialize)]
pub struct Collection {
    pub id: String,
    pub title: String,
    pub game_keys: Vec<String>,
    pub assets: Vec<Asset>,
}
#[derive(Serialize, Clone)]
pub struct DiscItem {
    pub path: String,
    pub bytes: u64,
    pub kind: String,
    pub supported: bool,
    #[serde(skip)]
    extents: Vec<(u64, u64)>,
}
#[derive(Serialize)]
pub struct Disc {
    pub path: PathBuf,
    pub source_bytes: u64,
    pub signature: String,
    pub items: Vec<DiscItem>,
}
fn block(f: &mut File, offset: u64, len: usize, size: u64) -> Result<Vec<u8>> {
    if offset.checked_add(len as u64).is_none_or(|n| n > size) {
        return Err(err("Disc extent exceeds the image"));
    }
    f.seek(SeekFrom::Start(offset))?;
    let mut bytes = vec![0; len];
    f.read_exact(&mut bytes)?;
    Ok(bytes)
}
fn both(b: &[u8]) -> Result<u64> {
    let a = u32::from_le_bytes(b[..4].try_into().unwrap());
    let z = u32::from_be_bytes(b[4..8].try_into().unwrap());
    if a != z {
        return Err(err("Inconsistent ISO directory fields"));
    }
    Ok(a as u64)
}
/// Bounded ISO9660 primary directory reader, including level-3 multi-extents.
/// No mounting, shell execution, license access or game payload parsing.
pub fn inspect(path: &Path) -> Result<Disc> {
    let mut f = regular(path)?;
    let meta = f.metadata()?;
    let size = meta.len();
    let p = block(&mut f, 16 * 2048, 2048, size)?;
    if p[..7] != [1, b'C', b'D', b'0', b'0', b'1', 1] || p[128..132] != [0, 8, 8, 0] {
        return Err(err("Expected an ISO9660 image with 2048-byte sectors"));
    }
    let declared = both(&p[80..88])?
        .checked_mul(2048)
        .ok_or_else(|| err("Invalid ISO size"))?;
    if declared > size {
        return Err(err("Disc image is incomplete"));
    }
    let root = &p[156..190];
    let mut queue = vec![(
        String::new(),
        both(&root[2..10])? * 2048,
        both(&root[10..18])?,
        0,
    )];
    let mut seen = BTreeSet::new();
    let mut files: BTreeMap<String, DiscItem> = BTreeMap::new();
    let mut total = 0u64;
    let mut records = 0;
    while let Some((parent, offset, len, depth)) = queue.pop() {
        if !seen.insert(offset) {
            return Err(err("Repeated ISO directory extent"));
        }
        total += len;
        if depth > 8 || seen.len() > 512 || len > 1024 * 1024 || total > 16 * 1024 * 1024 {
            return Err(err("Disc directory inspection limit reached"));
        }
        let bytes = block(&mut f, offset, len as usize, size)?;
        let mut at = 0;
        let mut unfinished: Option<String> = None;
        while at < bytes.len() {
            let n = bytes[at] as usize;
            if n == 0 {
                at = ((at / 2048) + 1) * 2048;
                continue;
            }
            records += 1;
            if records > 20000 || n < 34 || at + n > bytes.len() || at % 2048 + n > 2048 {
                return Err(err("Invalid or oversized ISO directory"));
            }
            let r = &bytes[at..at + n];
            at += n;
            let namelen = r[32] as usize;
            if 33 + namelen > r.len() {
                return Err(err("Invalid ISO filename"));
            }
            let raw = &r[33..33 + namelen];
            if raw == [0] || raw == [1] {
                continue;
            }
            let name = std::str::from_utf8(raw)
                .map_err(|_| err("Unsupported ISO filename encoding"))?
                .trim_end_matches(";1");
            if name.is_empty()
                || name.contains(['/', '\\'])
                || name == ".."
                || name.chars().any(char::is_control)
            {
                return Err(err("Unsafe ISO filename"));
            }
            let path = format!("{parent}/{name}");
            let start = both(&r[2..10])? * 2048;
            let count = both(&r[10..18])?;
            if start.checked_add(count).is_none_or(|end| end > size)
                || r[1] != 0
                || r[26] != 0
                || r[27] != 0
            {
                return Err(err("Unsupported or out-of-bounds ISO extent"));
            }
            if r[25] & 2 != 0 {
                queue.push((path, start, count, depth + 1));
                continue;
            }
            let Some((kind, _, supported)) = format(&path) else {
                continue;
            };
            if files.len() >= 2000 && !files.contains_key(&path) {
                return Err(err("Disc media candidate limit reached"));
            }
            if unfinished.as_ref().is_some_and(|p| p != &path)
                || files.contains_key(&path) && unfinished.as_deref() != Some(&path)
            {
                return Err(err("Invalid media extent sequence"));
            }
            unfinished = if r[25] & 128 != 0 {
                Some(path.clone())
            } else {
                None
            };
            let item = files.entry(path.clone()).or_insert(DiscItem {
                path,
                bytes: 0,
                kind: kind.into(),
                supported,
                extents: vec![],
            });
            if item.extents.len() > 512 {
                return Err(err("Too many media extents"));
            }
            item.bytes = item
                .bytes
                .checked_add(count)
                .ok_or_else(|| err("Media size overflow"))?;
            item.extents.push((start, count));
        }
        if unfinished.is_some() {
            return Err(err("Incomplete media extent sequence"));
        }
    }
    let modified = meta
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut hash = Sha256::new();
    hash.update(format!("{size}:{modified}:{}", path.display()));
    for i in files.values() {
        hash.update(format!("{}:{}:{:?}", i.path, i.bytes, i.extents));
    }
    Ok(Disc {
        path: path.into(),
        source_bytes: size,
        signature: format!("{:x}", hash.finalize()),
        items: files.into_values().collect(),
    })
}
fn storage(root: &Path) -> Result<PathBuf> {
    let p = root.join("game-media");
    match fs::symlink_metadata(&p) {
        Ok(m) if m.is_dir() && !m.file_type().is_symlink() => {}
        Ok(_) => return Err(err("Media storage was redirected")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&p)?,
        Err(e) => return Err(e.into()),
    }
    fs::set_permissions(&p, fs::Permissions::from_mode(0o700))?;
    Ok(p)
}
struct DiscReader {
    file: File,
    extents: Vec<(u64, u64)>,
    index: usize,
    remaining: u64,
}
impl Read for DiscReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        while self.remaining == 0 {
            let Some((offset, len)) = self.extents.get(self.index) else {
                return Ok(0);
            };
            self.file.seek(SeekFrom::Start(*offset))?;
            self.remaining = *len;
            self.index += 1;
        }
        let len = buf.len().min(self.remaining as usize);
        let n = self.file.read(&mut buf[..len])?;
        self.remaining -= n as u64;
        Ok(n)
    }
}

impl Core {
    pub fn media_collections(&self, game: Option<&str>) -> Result<Vec<Collection>> {
        self.with_db(|db| {
            let mut stmt=db.conn.prepare("SELECT id,title FROM game_media_collections WHERE ?1 IS NULL OR id IN (SELECT collection_id FROM game_media_links WHERE game_key=?1) ORDER BY title,id")?;
            let rows=stmt.query_map([game],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<std::result::Result<Vec<_>,_>>()?;
            let mut out=vec![];
            for (id,title) in rows {
                let mut s=db.conn.prepare("SELECT game_key FROM game_media_links WHERE collection_id=?1 ORDER BY game_key")?;
                let keys=s.query_map([&id],|r|r.get(0))?.collect::<std::result::Result<Vec<String>,_>>()?;
                let mut s=db.conn.prepare("SELECT id,title,kind,media,bytes,source,spoiler,position FROM game_media_assets WHERE collection_id=?1 ORDER BY title,id")?;
                let assets=s.query_map([&id],|r|{let media:String=r.get(3)?;Ok(Asset{id:r.get(0)?,title:r.get(1)?,kind:r.get(2)?,bytes:r.get(4)?,source:r.get(5)?,spoiler:r.get(6)?,position:r.get(7)?,available:regular(&self.paths.data_dir.join("game-media").join(media)).is_ok()})})?.collect::<std::result::Result<Vec<_>,_>>()?;
                out.push(Collection{id,title,game_keys:keys,assets});
            } Ok(out)
        })
    }
    pub fn media_create(&self, title: &str, game: Option<&str>) -> Result<String> {
        validate(title)?;
        if let Some(k) = game {
            validate(k)?;
        }
        let id = Uuid::new_v4().to_string();
        self.with_db(|db| {
            db.conn.execute(
                "INSERT INTO game_media_collections(id,title) VALUES(?1,?2)",
                params![id, title.trim()],
            )?;
            if let Some(k) = game {
                db.conn
                    .execute("INSERT INTO game_media_links VALUES(?1,?2)", params![id, k])?;
            }
            Ok(())
        })?;
        Ok(id)
    }
    pub fn media_link(&self, id: &str, game: &str, linked: bool) -> Result<()> {
        validate(game)?;
        self.with_db(|db| {
            if linked {
                db.conn.execute(
                    "INSERT OR IGNORE INTO game_media_links VALUES(?1,?2)",
                    params![id, game],
                )?;
            } else {
                db.conn.execute(
                    "DELETE FROM game_media_links WHERE collection_id=?1 AND game_key=?2",
                    params![id, game],
                )?;
            }
            Ok(())
        })
    }
    fn media_store(
        &self,
        collection: &str,
        name: &str,
        source: &str,
        mut reader: impl Read,
        bytes: u64,
    ) -> Result<()> {
        let (kind, mime, supported) =
            format(name).ok_or_else(|| err("Choose PNG, JPEG, MP4 or WebM media"))?;
        if !supported {
            return Err(err(
                "M2V cinematics need conversion before playback. Choose an MP4/WebM clip for now",
            ));
        }
        if bytes == 0
            || bytes
                > if kind == "image" {
                    16 * 1024 * 1024
                } else {
                    MAX_VIDEO
                }
        {
            return Err(err("Media exceeds the 16 MiB image or 2 GiB video limit"));
        }
        self.with_db(|db| {
            let _: String = db.conn.query_row(
                "SELECT id FROM game_media_collections WHERE id=?1",
                [collection],
                |r| r.get(0),
            )?;
            Ok(())
        })?;
        let dir = storage(&self.paths.data_dir)?;
        let mut tmp = tempfile::NamedTempFile::new_in(&dir)?;
        let n = std::io::copy(&mut (&mut reader).take(bytes + 1), &mut tmp)?;
        if n != bytes {
            return Err(err("Media source changed or was truncated"));
        }
        tmp.as_file_mut().seek(SeekFrom::Start(0))?;
        let mut magic = [0; 12];
        tmp.as_file_mut().read_exact(&mut magic)?;
        if kind == "image" {
            let mut r = image::ImageReader::new(std::io::BufReader::new(File::open(tmp.path())?))
                .with_guessed_format()?;
            if !matches!(
                r.format(),
                Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
            ) {
                return Err(err("Unsupported image contents"));
            }
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(8192);
            limits.max_image_height = Some(8192);
            limits.max_alloc = Some(256 * 1024 * 1024);
            r.limits(limits);
            let image = r
                .decode()
                .map_err(|_| err("Image cannot be decoded within the gallery limits"))?;
            tmp.as_file_mut().set_len(0)?;
            tmp.as_file_mut().seek(SeekFrom::Start(0))?;
            image
                .thumbnail(image.width().min(2560), image.height().min(2560))
                .write_to(tmp.as_file_mut(), image::ImageFormat::Png)
                .map_err(|_| err("Cannot prepare gallery image"))?;
        } else if !(mime == "video/mp4" && &magic[4..8] == b"ftyp"
            || mime == "video/webm" && magic[..4] == [0x1a, 0x45, 0xdf, 0xa3])
        {
            return Err(err("Video header does not match its format"));
        }
        tmp.as_file_mut().seek(SeekFrom::Start(0))?;
        let mut hash = Sha256::new();
        let mut buf = [0; 65536];
        loop {
            let n = tmp.as_file_mut().read(&mut buf)?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n]);
        }
        let ext = if kind == "image" {
            "png"
        } else if mime == "video/mp4" {
            "mp4"
        } else {
            "webm"
        };
        let file = format!("{:x}.{ext}", hash.finalize());
        let size = tmp.as_file().metadata()?.len();
        tmp.as_file().sync_all()?;
        match tmp.persist_noclobber(dir.join(&file)) {
            Ok(_) => {}
            Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.error.into()),
        }
        let id = Uuid::new_v4().to_string();
        let spoiler = kind == "video";
        self.with_db(|db| {db.conn.execute("INSERT OR IGNORE INTO game_media_assets(id,collection_id,title,kind,mime,media,bytes,source,spoiler) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![id,collection,title(name),kind,if kind=="image"{"image/png"}else{mime},file,size,source,spoiler])?;Ok(())})
    }
    pub fn media_import(&self, id: &str, paths: Vec<PathBuf>) -> Result<()> {
        if paths.is_empty() || paths.len() > 20 {
            return Err(err("Choose 1–20 media files"));
        }
        let mut total = 0;
        for p in &paths {
            let f = regular(p)?;
            total += f.metadata()?.len();
            if total > MAX_VIDEO {
                return Err(err("Import at most 2 GiB per batch"));
            }
        }
        for p in paths {
            let f = regular(&p)?;
            let n = f.metadata()?.len();
            self.media_store(
                id,
                p.file_name().unwrap_or_default().to_string_lossy().as_ref(),
                p.to_string_lossy().as_ref(),
                f,
                n,
            )?;
        }
        Ok(())
    }
    pub fn media_import_disc(
        &self,
        id: &str,
        path: &Path,
        signature: &str,
        member: &str,
    ) -> Result<()> {
        let disc = inspect(path)?;
        if disc.signature != signature {
            return Err(err("Disc changed; inspect it again"));
        }
        let item = disc
            .items
            .iter()
            .find(|i| i.path == member)
            .ok_or_else(|| err("Media member no longer exists"))?;
        if !item.supported || item.bytes > MAX_VIDEO {
            return Err(err(
                "This disc member needs conversion or exceeds the 2 GiB import limit",
            ));
        }
        let stream = DiscReader {
            file: regular(path)?,
            extents: item.extents.clone(),
            index: 0,
            remaining: 0,
        };
        self.media_store(
            id,
            member,
            &format!("{}:{member}", path.display()),
            stream,
            item.bytes,
        )
    }
    pub fn media_open(&self, id: &str) -> Result<(File, String)> {
        if Uuid::parse_str(id).is_err() {
            return Err(err("Invalid media identity"));
        }
        let (name, mime): (String, String) = self.with_db(|db| {
            Ok(db.conn.query_row(
                "SELECT media,mime FROM game_media_assets WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?)
        })?;
        if name.contains(['/', '\\']) || name.len() > 80 {
            return Err(err("Invalid stored media identity"));
        }
        let dir = storage(&self.paths.data_dir)?;
        Ok((regular(&dir.join(name))?, mime))
    }
    pub fn media_position(&self, id: &str, position: f64) -> Result<()> {
        if !position.is_finite() || !(0.0..=604800.0).contains(&position) {
            return Err(err("Invalid playback position"));
        }
        self.with_db(|db| {
            db.conn.execute(
                "UPDATE game_media_assets SET position=?2 WHERE id=?1 AND kind='video'",
                params![id, position],
            )?;
            Ok(())
        })
    }
    pub fn media_update(&self, id: &str, title: &str, spoiler: bool) -> Result<()> {
        validate(title)?;
        self.with_db(|db| {
            db.conn.execute(
                "UPDATE game_media_assets SET title=?2,spoiler=?3 WHERE id=?1",
                params![id, title.trim(), spoiler],
            )?;
            Ok(())
        })
    }
    pub fn media_remove(&self, id: &str) -> Result<()> {
        self.with_db(|db| {
            db.conn
                .execute("DELETE FROM game_media_assets WHERE id=?1", [id])?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn dual(n: u32) -> [u8; 8] {
        let mut v = [0; 8];
        v[..4].copy_from_slice(&n.to_le_bytes());
        v[4..].copy_from_slice(&n.to_be_bytes());
        v
    }
    fn record(name: &[u8], sector: u32, len: u32, flags: u8) -> Vec<u8> {
        let n = 33 + name.len() + name.len().is_multiple_of(2) as usize;
        let mut r = vec![0; n];
        r[0] = n as u8;
        r[2..10].copy_from_slice(&dual(sector));
        r[10..18].copy_from_slice(&dual(len));
        r[25] = flags;
        r[32] = name.len() as u8;
        r[33..33 + name.len()].copy_from_slice(name);
        r
    }
    fn iso(path: &Path) -> Vec<u8> {
        let mut png = std::io::Cursor::new(vec![]);
        image::RgbImage::new(8, 6)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let png = png.into_inner();
        let cut = png.len() / 2;
        let mut b = vec![0; 64 * 2048];
        let p = &mut b[16 * 2048..17 * 2048];
        p[..7].copy_from_slice(b"\x01CD001\x01");
        p[80..88].copy_from_slice(&dual(64));
        p[128..132].copy_from_slice(&[0, 8, 8, 0]);
        p[156..190].copy_from_slice(&record(&[0], 20, 2048, 2));
        let mut entries = vec![];
        for r in [
            record(&[0], 20, 2048, 2),
            record(&[1], 20, 2048, 2),
            record(b"ART.PNG;1", 30, cut as u32, 128),
            record(b"ART.PNG;1", 32, (png.len() - cut) as u32, 0),
            record(b"MOVIE.MP4;1", 34, 32, 0),
            record(b"RAW.M2V;1", 35, 16, 0),
        ] {
            entries.extend(r);
        }
        b[20 * 2048..20 * 2048 + entries.len()].copy_from_slice(&entries);
        b[30 * 2048..30 * 2048 + cut].copy_from_slice(&png[..cut]);
        b[32 * 2048..32 * 2048 + png.len() - cut].copy_from_slice(&png[cut..]);
        b[34 * 2048 + 4..34 * 2048 + 8].copy_from_slice(b"ftyp");
        fs::write(path, b).unwrap();
        png
    }
    #[test]
    fn iso_selective_import_multi_extent_identity_resume_and_unlink_preserve_sources() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("disc.iso");
        let png = iso(&p);
        let core = Core::open_isolated(Some(t.path().join("data")), true).unwrap();
        let report = inspect(&p).unwrap();
        assert_eq!(report.items.len(), 3);
        let art = report.items.iter().find(|i| i.path == "/ART.PNG").unwrap();
        assert_eq!(art.bytes, png.len() as u64);
        assert_eq!(art.extents.len(), 2);
        assert!(
            !report
                .items
                .iter()
                .find(|i| i.path == "/RAW.M2V")
                .unwrap()
                .supported
        );
        let id = core.media_create("Game extras", Some("game-one")).unwrap();
        assert!(core
            .media_import_disc(&id, &p, "changed", "/ART.PNG")
            .is_err());
        assert!(core
            .media_import_disc(&id, &p, &report.signature, "/RAW.M2V")
            .is_err());
        core.media_import_disc(&id, &p, &report.signature, "/ART.PNG")
            .unwrap();
        core.media_import_disc(&id, &p, &report.signature, "/ART.PNG")
            .unwrap();
        core.media_import_disc(&id, &p, &report.signature, "/MOVIE.MP4")
            .unwrap();
        let c = core.media_collections(Some("game-one")).unwrap();
        assert_eq!(c[0].assets.len(), 2);
        let image = c[0].assets.iter().find(|a| a.kind == "image").unwrap();
        assert!(!image.spoiler);
        core.media_update(&image.id, "Box art", true).unwrap();
        assert!(
            core.media_collections(None).unwrap()[0]
                .assets
                .iter()
                .find(|a| a.kind == "image")
                .unwrap()
                .spoiler
        );
        let mut f = core.media_open(&image.id).unwrap().0;
        let mut bytes = vec![];
        f.read_to_end(&mut bytes).unwrap();
        assert_eq!(image::load_from_memory(&bytes).unwrap().width(), 8);
        let movie = c[0].assets.iter().find(|a| a.kind == "video").unwrap();
        core.media_position(&movie.id, 42.5).unwrap();
        assert!(core.media_position(&movie.id, f64::NAN).is_err());
        assert!(core.media_position(&movie.id, -1.0).is_err());
        core.media_link(&id, "game-one", false).unwrap();
        assert!(core.media_collections(Some("game-one")).unwrap().is_empty());
        assert_eq!(
            core.media_collections(None).unwrap()[0]
                .assets
                .iter()
                .find(|a| a.kind == "video")
                .unwrap()
                .position,
            42.5
        );
        core.media_remove(&movie.id).unwrap();
        assert!(core.media_open(&movie.id).is_err());
        assert_eq!(p.metadata().unwrap().len(), 64 * 2048);
        drop(core);
        let core = Core::open_isolated(Some(t.path().join("data")), true).unwrap();
        assert_eq!(core.media_collections(None).unwrap()[0].assets.len(), 1);
    }
    #[test]
    fn invalid_images_links_devices_and_malformed_disc_extents_are_refused() {
        let t = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(t.path().join("data")), true).unwrap();
        let id = core.media_create("Extras", None).unwrap();
        let p = t.path().join("bad.png");
        fs::write(&p, b"<svg>not a PNG</svg>").unwrap();
        assert!(core.media_import(&id, vec![p.clone()]).is_err());
        let link = t.path().join("link.png");
        std::os::unix::fs::symlink(&p, &link).unwrap();
        assert!(core.media_import(&id, vec![link]).is_err());
        assert!(regular(Path::new("/dev/zero")).is_err());
        assert!(core.media_open("../../private").is_err());
        let p = t.path().join("disc.iso");
        iso(&p);
        let mut f = OpenOptions::new().write(true).open(&p).unwrap();
        f.seek(SeekFrom::Start(20 * 2048 + 34 * 2 + 2)).unwrap();
        f.write_all(&dual(1000)).unwrap();
        assert!(inspect(&p).is_err());
    }
}
