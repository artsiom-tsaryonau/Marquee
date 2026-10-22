//! Durable BitTorrent transfers. Private device shares and public discovery
//! use separate engines. Downloads remain in owned per-transfer folders; an
//! engine/package download never establishes installation or play readiness.
use crate::error::{AppError, Result};
use crate::model::TorrentSettings;
use crate::Core;
use marquee_torrent::{Config, Engine, Source, State};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

const PORT: u16 = 51413;
const MAX_METADATA: u64 = 4 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    kind: String,
    info_hash: String,
    name: String,
    edition_id: Option<String>,
    system: Option<String>,
    dir: PathBuf,
    #[serde(default)]
    destination: Option<PathBuf>,
    peers: Vec<SocketAddr>,
    #[serde(default)]
    scanned: bool,
    #[serde(default)]
    complete: bool,
    #[serde(default)]
    total: u64,
    #[serde(default)]
    downloaded: u64,
    #[serde(default)]
    uploaded: u64,
    #[serde(default)]
    owned: bool,
    #[serde(default)]
    public: bool,
    #[serde(default)]
    paused: bool,
    #[serde(default)]
    source: Option<String>,
}
struct Job {
    edition_id: String,
    name: String,
    error: Option<String>,
}
struct Inner {
    dir: PathBuf,
    engines: Mutex<BTreeMap<bool, Arc<Engine>>>,
    active: Mutex<BTreeSet<String>>,
    errors: Mutex<BTreeMap<String, String>>,
    jobs: Mutex<Vec<Job>>,
    file: Mutex<()>,
    scans: Mutex<BTreeSet<String>>,
    scan_errors: Mutex<BTreeMap<String, String>>,
    epoch: AtomicU64,
}
pub struct Transfers {
    inner: Arc<Inner>,
}
#[derive(Debug, Clone, Serialize)]
pub struct TransferItem {
    pub info_hash: String,
    pub name: String,
    pub kind: String,
    pub edition_id: Option<String>,
    pub state: String,
    pub downloaded: u64,
    pub total: u64,
    pub uploaded: u64,
    pub link: Option<String>,
    pub error: Option<String>,
    pub directory: PathBuf,
    pub public: bool,
    pub files_present: Option<bool>,
}
#[derive(Debug, Clone, Serialize)]
pub struct TransferStatus {
    pub enabled: bool,
    pub seeding: bool,
    pub items: Vec<TransferItem>,
}
#[derive(Debug, Clone, Serialize)]
pub struct TorrentReview {
    #[serde(flatten)]
    pub manifest: marquee_torrent::Manifest,
    pub review_token: String,
    pub directory: PathBuf,
}

fn valid_hash(hash: &str) -> bool {
    hash.len() == 40 && hash.bytes().all(|b| b.is_ascii_hexdigit())
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::new("TORRENT_STATE", "Missing state directory"))?;
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| AppError::from(e.error))?;
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}
fn metadata(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_METADATA + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_METADATA {
        return Err(AppError::new(
            "TORRENT_METADATA",
            "Torrent metadata exceeds 4 MiB",
        ));
    }
    Ok(bytes)
}
fn off() -> AppError {
    AppError::new(
        "TORRENT_OFF",
        "Library transfer is off. Turn it on in Settings first.",
    )
}

fn fail(e: anyhow::Error) -> AppError {
    AppError::new("TORRENT", format!("{e:#}"))
}

fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn decode(s: &str) -> Result<String> {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    let bad = || AppError::new("TORRENT_LINK", "Invalid percent encoding in magnet link");
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let high = bytes
                    .get(i + 1)
                    .and_then(|b| (*b as char).to_digit(16))
                    .ok_or_else(bad)?;
                let low = bytes
                    .get(i + 2)
                    .and_then(|b| (*b as char).to_digit(16))
                    .ok_or_else(bad)?;
                out.push((high * 16 + low) as u8);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| bad())
}

fn local_host() -> String {
    if let Ok(h) = std::env::var("MARQUEE_TORRENT_HOST") {
        if !h.trim().is_empty() {
            return h.trim().to_string();
        }
    }
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("192.0.2.1:9")?;
            s.local_addr()
        })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".into())
}

fn make_link(hash: &str, name: &str, system: Option<&str>, peer: Option<&str>) -> String {
    let mut l = format!("magnet:?xt=urn:btih:{hash}&dn={}", encode(name));
    if let Some(p) = peer {
        l.push_str(&format!("&x.pe={p}"));
    }
    if let Some(s) = system {
        l.push_str(&format!("&x.mq={}", encode(s)));
    }
    l
}

#[derive(Debug, PartialEq, Eq)]
struct Link {
    hash: String,
    name: String,
    system: Option<String>,
    peers: Vec<SocketAddr>,
}

fn parse_link(link: &str) -> Result<Link> {
    let bad = || {
        AppError::new(
            "TORRENT_LINK",
            "That is not a Marquee game link. Copy it from the other device's Share button.",
        )
    };
    if link.len() > 16 * 1024 {
        return Err(bad());
    }
    let q = link.trim().strip_prefix("magnet:?").ok_or_else(bad)?;
    let (mut hash, mut name, mut system, mut peers) = (None, String::new(), None, Vec::new());
    for part in q.split('&') {
        let Some((k, v)) = part.split_once('=') else {
            continue;
        };
        let v = decode(v)?;
        match k {
            "xt" => {
                if let Some(value) = v.to_ascii_lowercase().strip_prefix("urn:btih:") {
                    hash = normalize_hash(value);
                }
            }
            "dn" => name = v,
            "x.mq" => system = Some(v),
            "x.pe" => {
                let peer: SocketAddr = v.parse().map_err(|_| bad())?;
                if peer.port() == 0 || peer.ip().is_unspecified() || peers.len() >= 50 {
                    return Err(bad());
                }
                if !peers.contains(&peer) {
                    peers.push(peer);
                }
            }
            _ => {}
        }
    }
    let hash = hash.ok_or_else(bad)?;
    Ok(Link {
        hash,
        name,
        system,
        peers,
    })
}

fn normalize_hash(value: &str) -> Option<String> {
    if valid_hash(value) {
        return Some(value.to_ascii_lowercase());
    }
    if value.len() != 32 {
        return None;
    }
    let mut accumulator = 0u32;
    let mut bits = 0;
    let mut out = Vec::new();
    for byte in value.bytes().map(|b| b.to_ascii_uppercase()) {
        let digit = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'2'..=b'7' => byte - b'2' + 26,
            _ => return None,
        };
        accumulator = (accumulator << 5) | u32::from(digit);
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((accumulator >> bits) as u8);
        }
    }
    Some(out.iter().map(|b| format!("{b:02x}")).collect())
}
pub fn magnet_hash(link: &str) -> Result<String> {
    Ok(parse_link(link)?.hash)
}

impl Transfers {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            inner: Arc::new(Inner {
                dir: data_dir.join("torrents"),
                engines: Mutex::new(BTreeMap::new()),
                active: Mutex::new(BTreeSet::new()),
                errors: Mutex::new(BTreeMap::new()),
                jobs: Mutex::new(Vec::new()),
                file: Mutex::new(()),
                scans: Mutex::new(BTreeSet::new()),
                scan_errors: Mutex::new(BTreeMap::new()),
                epoch: AtomicU64::new(0),
            }),
        }
    }
    pub fn autostart(&self, settings: &TorrentSettings) {
        if !settings.enabled {
            return;
        }
        if let Ok(entries) = self.inner.load() {
            for entry in entries.into_iter().filter(|e| !e.paused) {
                self.inner.activate(entry, settings.clone());
            }
        }
    }
    pub fn reset(&self, settings: &TorrentSettings) {
        self.inner.epoch.fetch_add(1, Ordering::SeqCst);
        let old =
            std::mem::take(&mut *self.inner.engines.lock().unwrap_or_else(|e| e.into_inner()));
        for engine in old.values() {
            engine.cancel();
        }
        self.inner
            .active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.inner
            .errors
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        std::thread::spawn(move || drop(old));
        self.autostart(settings);
    }
}
impl Inner {
    fn entries_path(&self) -> PathBuf {
        self.dir.join("entries.json")
    }
    fn load_unlocked(&self) -> Result<Vec<Entry>> {
        let file = match std::fs::File::open(self.entries_path()) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut bytes = Vec::new();
        file.take(8 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err(AppError::new(
                "TORRENT_STATE",
                "Saved transfer state is oversized; file was preserved",
            ));
        }
        let entries: Vec<Entry> = serde_json::from_slice(&bytes).map_err(|e| {
            AppError::new(
                "TORRENT_STATE",
                format!("Could not read saved transfers; state was preserved: {e}"),
            )
        })?;
        let mut hashes = BTreeSet::new();
        for e in &entries {
            if !valid_hash(&e.info_hash)
                || !hashes.insert(&e.info_hash)
                || !matches!(e.kind.as_str(), "share" | "get")
                || (e.owned
                    && e.dir
                        != e.destination
                            .as_ref()
                            .map(|base| base.join(&e.info_hash))
                            .unwrap_or_else(|| self.download_dir(&e.info_hash)))
            {
                return Err(AppError::new(
                    "TORRENT_STATE",
                    "Invalid saved transfer record; file was preserved",
                ));
            }
        }
        Ok(entries)
    }
    fn load(&self) -> Result<Vec<Entry>> {
        let _guard = self.file.lock().unwrap_or_else(|e| e.into_inner());
        self.load_unlocked()
    }
    fn update<R>(&self, change: impl FnOnce(&mut Vec<Entry>) -> R) -> Result<R> {
        let _guard = self.file.lock().unwrap_or_else(|e| e.into_inner());
        let mut entries = self.load_unlocked()?;
        let result = change(&mut entries);
        let bytes = serde_json::to_vec_pretty(&entries)?;
        if bytes.len() > 8 * 1024 * 1024 || entries.len() > 1000 {
            return Err(AppError::new(
                "TORRENT_LIMIT",
                "Transfer history limit reached; no state was changed",
            ));
        }
        atomic_write(&self.entries_path(), &bytes)?;
        Ok(result)
    }
    fn torrent_file(&self, hash: &str) -> PathBuf {
        self.dir.join(format!("{hash}.torrent"))
    }
    fn download_dir(&self, hash: &str) -> PathBuf {
        self.dir.join("downloads").join(hash)
    }
    fn owned_dir_at(&self, hash: &str, dir: &Path) -> Result<PathBuf> {
        let dir = dir.to_path_buf();
        std::fs::create_dir_all(dir.parent().unwrap())?;
        let marker = dir
            .parent()
            .unwrap()
            .join(format!(".marquee-transfer-{hash}"));
        match std::fs::create_dir(&dir) {
            Ok(()) => {
                match std::fs::File::options()
                    .write(true)
                    .create_new(true)
                    .open(&marker)
                {
                    Ok(mut file) => {
                        file.write_all(hash.as_bytes())?;
                        file.sync_all()?;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(e) => return Err(e.into()),
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
        if std::fs::symlink_metadata(&dir)?.file_type().is_symlink()
            || !std::fs::symlink_metadata(&marker)?.is_file()
            || std::fs::read(&marker)? != hash.as_bytes()
        {
            return Err(AppError::new(
                "TORRENT_CONFLICT",
                "Transfer folder is not owned by this job; existing files were kept",
            ));
        }
        // A resumed download must never follow a symlink planted in its tree.
        let mut stack = vec![dir.clone()];
        let mut count = 0;
        while let Some(path) = stack.pop() {
            let meta = std::fs::symlink_metadata(&path)?;
            count += 1;
            if count > 40_000
                || meta.file_type().is_symlink()
                || (!meta.is_dir() && !meta.is_file())
            {
                return Err(AppError::new(
                    "TORRENT_CONFLICT",
                    "Unsafe file in transfer folder; resume refused",
                ));
            }
            if meta.is_dir() {
                for item in std::fs::read_dir(path)? {
                    stack.push(item?.path());
                }
            }
        }
        Ok(dir)
    }
    fn files_present(&self, entry: &Entry) -> bool {
        metadata(&self.torrent_file(&entry.info_hash))
            .ok()
            .and_then(|bytes| marquee_torrent::inspect(&bytes).ok())
            .is_some_and(|manifest| {
                manifest.files.iter().all(|file| {
                    std::fs::symlink_metadata(entry.dir.join(&file.path))
                        .is_ok_and(|m| m.is_file() && m.len() == file.size)
                })
            })
    }
    fn config(&self, settings: &TorrentSettings, public: bool) -> Config {
        let mut cfg = Config::private(self.dir.join("incoming"));
        cfg.listen = SocketAddr::from(([0, 0, 0, 0], if public { 0 } else { PORT }));
        cfg.seeding = settings.seeding;
        cfg.trackers = public && settings.trackers;
        cfg.dht = public && settings.dht;
        cfg.upload_bps =
            (settings.upload_kbps > 0).then(|| settings.upload_kbps.saturating_mul(1024));
        cfg.download_bps =
            (settings.download_kbps > 0).then(|| settings.download_kbps.saturating_mul(1024));
        cfg
    }
    fn engine(&self, settings: &TorrentSettings, public: bool, epoch: u64) -> Result<Arc<Engine>> {
        let mut engines = self.engines.lock().unwrap_or_else(|e| e.into_inner());
        if !settings.enabled || self.epoch.load(Ordering::SeqCst) != epoch {
            return Err(off());
        }
        if let Some(engine) = engines.get(&public) {
            return Ok(engine.clone());
        }
        let mut cfg = self.config(settings, public);
        let engine = match Engine::start(&cfg) {
            Ok(engine) => engine,
            Err(_) => {
                cfg.listen.set_port(0);
                Engine::start(&cfg).map_err(fail)?
            }
        };
        let engine = Arc::new(engine);
        engines.insert(public, engine.clone());
        Ok(engine)
    }
    fn activate(self: &Arc<Self>, entry: Entry, settings: TorrentSettings) {
        if !self
            .active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(entry.info_hash.clone())
        {
            return;
        }
        let inner = self.clone();
        let epoch = self.epoch.load(Ordering::SeqCst);
        std::thread::spawn(move || {
            let result = (|| {
                if entry.kind == "share" && !settings.seeding {
                    return Err(AppError::new(
                        "TORRENT_SEEDING_OFF",
                        "Sharing is disabled; enable uploads to resume this share",
                    ));
                }
                if entry.kind == "get" && entry.complete && !inner.files_present(&entry) {
                    return Err(AppError::new("TORRENT_SOURCE_MISSING", "Downloaded source files are missing. Record retained; Resume to download them again"));
                }
                let engine = inner.engine(&settings, entry.public, epoch)?;
                let id = if entry.kind == "share" {
                    engine
                        .seed(metadata(&inner.torrent_file(&entry.info_hash))?, &entry.dir)
                        .map_err(fail)?
                } else {
                    if !entry.owned {
                        return Err(AppError::new("TORRENT_LEGACY", "Legacy download points into a library folder. Stop and re-add it to resume safely; existing files are kept"));
                    }
                    let dir = inner.owned_dir_at(&entry.info_hash, &entry.dir)?;
                    let source =
                        if inner.torrent_file(&entry.info_hash).is_file() {
                            Source::TorrentBytes(metadata(&inner.torrent_file(&entry.info_hash))?)
                        } else {
                            Source::Link(entry.source.clone().unwrap_or_else(|| {
                                format!("magnet:?xt=urn:btih:{}", entry.info_hash)
                            }))
                        };
                    engine
                        .download_owned(source, &dir, entry.peers.clone())
                        .map_err(fail)?
                };
                if entry.kind == "get" {
                    let bytes = engine.metadata_bytes(id).map_err(fail)?;
                    let manifest = marquee_torrent::inspect(&bytes).map_err(fail)?;
                    atomic_write(&inner.torrent_file(&entry.info_hash), &bytes)?;
                    inner.update(|entries| {
                        if let Some(e) = entries.iter_mut().find(|e| e.info_hash == entry.info_hash)
                        {
                            e.total = manifest.total;
                        }
                    })?;
                }
                let latest = inner
                    .load()?
                    .into_iter()
                    .find(|e| e.info_hash == entry.info_hash);
                if inner.epoch.load(Ordering::SeqCst) != epoch || latest.is_none() {
                    let _ = engine.remove(id, false);
                } else if latest.is_some_and(|e| e.paused) {
                    engine.pause(id).map_err(fail)?;
                } else if matches!(engine.progress(id).map_err(fail)?.state, State::Paused) {
                    engine.resume(id).map_err(fail)?;
                }
                Ok(())
            })();
            if inner.epoch.load(Ordering::SeqCst) == epoch {
                if let Err(e) = result {
                    inner
                        .errors
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(entry.info_hash, e.message);
                }
            }
        });
    }
}
fn peer_string(engine: &Engine) -> Option<String> {
    engine
        .listen_addr()
        .map(|a| format!("{}:{}", local_host(), a.port()))
}

pub fn status(core: &Arc<Core>) -> Result<TransferStatus> {
    let settings = core.torrent_settings()?;
    let inner = &core.transfers.inner;
    let entries = inner.load()?;
    core.transfers.autostart(&settings);
    let engines = inner
        .engines
        .try_lock()
        .map(|e| e.clone())
        .unwrap_or_default();
    let progress: BTreeMap<_, _> = engines
        .iter()
        .map(|(public, engine)| (*public, engine.list()))
        .collect();
    let changed = entries.iter().any(|entry| {
        progress
            .get(&entry.public)
            .and_then(|list| list.iter().find(|p| p.info_hash == entry.info_hash))
            .is_some_and(|p| {
                p.downloaded != entry.downloaded
                    || p.uploaded != entry.uploaded
                    || p.total != entry.total
                    || (p.finished && !entry.complete)
            })
    });
    if changed {
        inner.update(|entries| {
            for entry in entries {
                if let Some(p) = progress
                    .get(&entry.public)
                    .and_then(|list| list.iter().find(|p| p.info_hash == entry.info_hash))
                {
                    entry.downloaded = p.downloaded;
                    entry.uploaded = p.uploaded;
                    entry.total = p.total;
                    entry.complete |= p.finished;
                }
            }
        })?;
    }
    let mut items = Vec::new();
    for entry in entries {
        let p = progress
            .get(&entry.public)
            .and_then(|list| list.iter().find(|p| p.info_hash == entry.info_hash));
        let mut error = inner
            .errors
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&entry.info_hash)
            .cloned();
        let files_present =
            (entry.kind == "get" && entry.complete).then(|| inner.files_present(&entry));
        let state = if files_present == Some(false) {
            "source_missing"
        } else if !settings.enabled || entry.paused {
            "paused"
        } else if error.is_some() {
            "error"
        } else {
            match p.map(|p| &p.state) {
                None => "starting",
                Some(State::Checking) => "checking",
                Some(State::Downloading) => "downloading",
                Some(State::Seeding) if !settings.seeding => "complete",
                Some(State::Seeding) => {
                    if entry.public {
                        "seeding"
                    } else {
                        "sharing"
                    }
                }
                Some(State::Paused) => "paused",
                Some(State::Error(message)) => {
                    error = Some(message.clone());
                    "error"
                }
            }
        };
        if entry.kind == "get"
            && !entry.scanned
            && p.is_some_and(|p| p.finished)
            && inner
                .scans
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(entry.info_hash.clone())
        {
            let core = core.clone();
            let entry = entry.clone();
            std::thread::spawn(move || {
                let inner = &core.transfers.inner;
                let result = (|| {
                    if let Some(system) = &entry.system {
                        // Register only the verified job directory, never partial files.
                        core.add_root(system, entry.dir.clone(), None)?;
                        core.scan(None, None)?;
                    }
                    inner.update(|entries| {
                        entries
                            .iter_mut()
                            .filter(|e| e.info_hash == entry.info_hash)
                            .for_each(|e| e.scanned = true)
                    })
                })();
                let mut errors = inner.scan_errors.lock().unwrap_or_else(|e| e.into_inner());
                match result {
                    Ok(()) => {
                        errors.remove(&entry.info_hash);
                    }
                    Err(e) => {
                        errors.insert(
                            entry.info_hash.clone(),
                            format!("Downloaded; library scan failed: {}", e.message),
                        );
                    }
                }
                inner
                    .scans
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&entry.info_hash);
            });
        }
        error = error.or_else(|| {
            inner
                .scan_errors
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&entry.info_hash)
                .cloned()
        });
        let link =
            if !entry.public && (entry.kind == "share" || entry.complete) && state == "sharing" {
                engines
                    .get(&false)
                    .and_then(|engine| peer_string(engine))
                    .map(|peer| {
                        make_link(
                            &entry.info_hash,
                            &entry.name,
                            entry.system.as_deref(),
                            Some(&peer),
                        )
                    })
            } else {
                None
            };
        items.push(TransferItem {
            info_hash: entry.info_hash,
            name: entry.name,
            kind: entry.kind,
            edition_id: entry.edition_id,
            state: state.into(),
            downloaded: p.map_or(
                if entry.complete && files_present != Some(false) {
                    entry.total
                } else {
                    entry.downloaded
                },
                |p| p.downloaded,
            ),
            total: p.map_or(entry.total, |p| p.total),
            uploaded: p.map_or(entry.uploaded, |p| p.uploaded),
            link,
            error,
            directory: entry.dir,
            public: entry.public,
            files_present,
        });
    }
    for job in inner.jobs.lock().unwrap_or_else(|e| e.into_inner()).iter() {
        items.push(TransferItem {
            info_hash: String::new(),
            name: job.name.clone(),
            kind: "share".into(),
            edition_id: Some(job.edition_id.clone()),
            state: if job.error.is_some() {
                "error"
            } else {
                "preparing"
            }
            .into(),
            downloaded: 0,
            total: 0,
            uploaded: 0,
            link: None,
            error: job.error.clone(),
            directory: PathBuf::new(),
            public: false,
            files_present: None,
        });
    }
    Ok(TransferStatus {
        enabled: settings.enabled,
        seeding: settings.seeding,
        items,
    })
}

pub fn share(core: &Arc<Core>, edition_id: &str) -> Result<()> {
    let settings = core.torrent_settings()?;
    if !settings.enabled {
        return Err(off());
    }
    if !settings.seeding {
        return Err(AppError::new(
            "TORRENT_SEEDING_OFF",
            "Sharing is disabled in Settings",
        ));
    }
    let inner = core.transfers.inner.clone();
    if inner
        .load()?
        .iter()
        .any(|e| e.edition_id.as_deref() == Some(edition_id))
    {
        return Ok(());
    }
    let snapshot = core.snapshot()?;
    let edition = snapshot
        .editions
        .iter()
        .find(|e| e.id == edition_id)
        .ok_or_else(|| AppError::new("NOT_FOUND", "Game not found"))?;
    let media = snapshot
        .media
        .iter()
        .find(|m| m.edition_id == edition_id && m.available)
        .ok_or_else(|| AppError::new("MEDIA_MISSING", "Game has no available file"))?;
    let root = snapshot
        .roots
        .iter()
        .filter(|r| media.path.starts_with(&r.path))
        .max_by_key(|r| r.path.as_os_str().len());
    let target = match root.and_then(|r| media.path.strip_prefix(&r.path).ok().map(|rel| (r, rel)))
    {
        Some((r, rel)) if rel.components().count() > 1 => {
            r.path.join(rel.components().next().unwrap())
        }
        _ => media.path.clone(),
    };
    let dir = target.parent().unwrap_or(Path::new(".")).to_path_buf();
    let name = target
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let system = edition.system.clone();
    let edition_id = edition_id.to_string();
    let epoch = inner.epoch.load(Ordering::SeqCst);
    inner
        .jobs
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(Job {
            edition_id: edition_id.clone(),
            name: name.clone(),
            error: None,
        });
    std::thread::spawn(move || {
        let result: Result<()> = (|| {
            let engine = inner.engine(&settings, false, epoch)?;
            let made = engine.create(&target, vec![]).map_err(fail)?;
            if inner.epoch.load(Ordering::SeqCst) != epoch {
                return Err(off());
            }
            atomic_write(&inner.torrent_file(&made.info_hash), &made.bytes)?;
            let mut entry = Entry {
                kind: "share".into(),
                info_hash: made.info_hash,
                name,
                edition_id: Some(edition_id.clone()),
                system: Some(system),
                dir,
                destination: None,
                peers: vec![],
                scanned: true,
                complete: false,
                total: 0,
                downloaded: 0,
                uploaded: 0,
                owned: false,
                public: false,
                paused: false,
                source: None,
            };
            inner.update(|entries| {
                if let Some(existing) = entries.iter_mut().find(|e| e.info_hash == entry.info_hash)
                {
                    existing.edition_id = entry.edition_id.clone();
                    existing.system = entry.system.clone();
                    existing.paused = false;
                    entry = existing.clone();
                } else {
                    entries.push(entry.clone());
                }
            })?;
            inner
                .active
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&entry.info_hash);
            inner.activate(entry, settings);
            Ok(())
        })();
        let mut jobs = inner.jobs.lock().unwrap_or_else(|e| e.into_inner());
        match result {
            Ok(()) => jobs.retain(|j| j.edition_id != edition_id),
            Err(e) => {
                if let Some(job) = jobs.iter_mut().find(|j| j.edition_id == edition_id) {
                    job.error = Some(e.message);
                }
            }
        }
    });
    Ok(())
}

fn enqueue(core: &Arc<Core>, entry: Entry) -> Result<()> {
    let settings = core.torrent_settings()?;
    if !settings.enabled {
        return Err(off());
    }
    let inner = &core.transfers.inner;
    if inner.load()?.iter().any(|e| e.info_hash == entry.info_hash) {
        return Err(AppError::new(
            "TORRENT_EXISTS",
            "This torrent is already listed; use Resume or Retry",
        ));
    }
    inner.owned_dir_at(&entry.info_hash, &entry.dir)?;
    inner.update(|entries| -> Result<()> {
        if entries.iter().any(|e| e.info_hash == entry.info_hash) {
            return Err(AppError::new(
                "TORRENT_EXISTS",
                "This torrent is already listed; use Resume or Retry",
            ));
        }
        entries.push(entry.clone());
        Ok(())
    })??;
    inner.activate(entry, settings);
    Ok(())
}

pub fn get(core: &Arc<Core>, link: &str) -> Result<()> {
    get_at(core, link, None)
}

pub fn get_at(core: &Arc<Core>, link: &str, destination: Option<PathBuf>) -> Result<()> {
    let settings = core.torrent_settings()?;
    if !settings.enabled {
        return Err(off());
    }
    let parsed = parse_link(link)?;
    let public = parsed.peers.is_empty();
    if public && !settings.trackers && !settings.dht {
        return Err(AppError::new(
            "TORRENT_DISCOVERY",
            "This magnet has no direct peer. Enable trackers or DHT in Torrent settings",
        ));
    }
    if let Some(system) = &parsed.system {
        crate::catalog::by_id(system)?;
    }
    enqueue(
        core,
        Entry {
            kind: "get".into(),
            info_hash: parsed.hash.clone(),
            name: if parsed.name.is_empty() {
                parsed.hash.clone()
            } else {
                parsed.name
            },
            edition_id: None,
            system: parsed.system,
            dir: destination
                .as_ref()
                .map(|base| base.join(&parsed.hash))
                .unwrap_or_else(|| core.transfers.inner.download_dir(&parsed.hash)),
            destination,
            peers: parsed.peers,
            scanned: false,
            complete: false,
            total: 0,
            downloaded: 0,
            uploaded: 0,
            owned: true,
            public,
            paused: false,
            source: Some(if public {
                link.trim().to_string()
            } else {
                format!("magnet:?xt=urn:btih:{}", parsed.hash)
            }),
        },
    )
}

pub fn inspect_file(path: &Path) -> Result<(marquee_torrent::Manifest, String)> {
    let bytes = metadata(path)?;
    let manifest = marquee_torrent::inspect(&bytes).map_err(fail)?;
    let review_token = format!("{:x}", Sha256::digest(&bytes));
    Ok((manifest, review_token))
}

pub fn review(core: &Core, path: &Path) -> Result<TorrentReview> {
    let (manifest, review_token) = inspect_file(path)?;
    let directory = core.transfers.inner.download_dir(&manifest.info_hash);
    Ok(TorrentReview {
        manifest,
        review_token,
        directory,
    })
}

pub fn add_file(
    core: &Arc<Core>,
    path: &Path,
    review_token: &str,
    peers: Vec<SocketAddr>,
) -> Result<()> {
    add_file_at(core, path, review_token, peers, None)
}

pub fn add_file_at(
    core: &Arc<Core>,
    path: &Path,
    review_token: &str,
    peers: Vec<SocketAddr>,
    destination: Option<PathBuf>,
) -> Result<()> {
    let settings = core.torrent_settings()?;
    if !settings.enabled {
        return Err(off());
    }
    let bytes = metadata(path)?;
    if format!("{:x}", Sha256::digest(&bytes)) != review_token {
        return Err(AppError::new(
            "TORRENT_REVIEW_CHANGED",
            "Torrent metadata changed after review. Inspect it again before downloading",
        ));
    }
    let manifest = marquee_torrent::inspect(&bytes).map_err(fail)?;
    if core
        .transfers
        .inner
        .load()?
        .iter()
        .any(|e| e.info_hash == manifest.info_hash)
    {
        return Err(AppError::new(
            "TORRENT_EXISTS",
            "This torrent is already listed; use Resume or Retry",
        ));
    }
    if peers.is_empty() && !settings.trackers && (!settings.dht || manifest.private) {
        return Err(AppError::new(
            "TORRENT_DISCOVERY",
            "Enable tracker discovery (or DHT for a non-private torrent), or provide a direct peer",
        ));
    }
    if peers.is_empty()
        && settings.trackers
        && manifest.tracker_count == 0
        && (!settings.dht || manifest.private)
    {
        return Err(AppError::new(
            "TORRENT_DISCOVERY",
            "This torrent has no trackers; provide a peer or enable DHT for non-private torrents",
        ));
    }
    atomic_write(
        &core.transfers.inner.torrent_file(&manifest.info_hash),
        &bytes,
    )?;
    enqueue(
        core,
        Entry {
            kind: "get".into(),
            info_hash: manifest.info_hash.clone(),
            name: manifest.name,
            edition_id: None,
            system: None,
            dir: destination
                .as_ref()
                .map(|base| base.join(&manifest.info_hash))
                .unwrap_or_else(|| core.transfers.inner.download_dir(&manifest.info_hash)),
            destination,
            peers,
            scanned: false,
            complete: false,
            total: 0,
            downloaded: 0,
            uploaded: 0,
            owned: true,
            public: true,
            paused: false,
            source: None,
        },
    )
}

pub fn pause(core: &Arc<Core>, hash: &str, paused: bool) -> Result<()> {
    if !paused && !core.torrent_settings()?.enabled {
        return Err(off());
    }
    let inner = &core.transfers.inner;
    let mut recheck = false;
    let found = inner
        .update(|entries| {
            entries.iter_mut().find(|e| e.info_hash == hash).map(|e| {
                e.paused = paused;
                if !paused && e.complete && !inner.files_present(e) {
                    e.complete = false;
                    e.scanned = false;
                    recheck = true;
                }
                e.clone()
            })
        })?
        .ok_or_else(|| AppError::new("NOT_FOUND", "Transfer not found"))?;
    let engine = inner
        .engines
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&found.public)
        .cloned();
    if let Some(engine) = engine {
        if let Some(progress) = engine.list().into_iter().find(|p| p.info_hash == hash) {
            if paused {
                engine.pause(progress.id).map_err(fail)?;
            } else if recheck || matches!(progress.state, State::Error(_)) {
                engine.remove(progress.id, false).map_err(fail)?;
                inner
                    .active
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(hash);
            } else {
                engine.resume(progress.id).map_err(fail)?;
            }
        } else if !paused {
            inner
                .active
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(hash);
        }
    } else {
        inner
            .active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(hash);
    }
    inner
        .errors
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(hash);
    if !paused {
        inner.activate(found, core.torrent_settings()?);
    }
    Ok(())
}

pub fn stop(core: &Arc<Core>, hash: &str) -> Result<()> {
    let inner = &core.transfers.inner;
    let engines = inner
        .engines
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    for engine in engines.values() {
        if let Some(progress) = engine.list().into_iter().find(|p| p.info_hash == hash) {
            engine.remove(progress.id, false).map_err(fail)?;
        }
    }
    inner.update(|entries| {
        if let Some(e) = entries.iter_mut().find(|e| e.info_hash == hash) {
            e.paused = true;
        }
    })?;
    inner
        .active
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(hash);
    inner
        .errors
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(hash);
    // Retain the receipt, metadata and payload; explicit Resume can continue.
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_round_trips() {
        let h = "4927b4f8762d3727d8be7d48e754485c7f091436";
        let l = make_link(
            h,
            "Grand Theft Auto (USA) [v4].iso",
            Some("ps2"),
            Some("192.168.1.20:51413"),
        );
        let p = parse_link(&l).unwrap();
        assert_eq!(p.hash, h);
        assert_eq!(p.name, "Grand Theft Auto (USA) [v4].iso");
        assert_eq!(p.system.as_deref(), Some("ps2"));
        assert_eq!(p.peers, vec!["192.168.1.20:51413".parse().unwrap()]);
    }

    fn open(dir: &Path) -> Arc<Core> {
        let c = Arc::new(Core::open_isolated(Some(dir.to_path_buf()), true).unwrap());
        let mut t = c.torrent_settings().unwrap();
        t.enabled = true;
        c.set_torrent_settings(&t).unwrap();
        c
    }

    fn wait(core: &Arc<Core>, f: impl Fn(&TransferStatus) -> bool) -> TransferStatus {
        for _ in 0..200 {
            let s = status(core).unwrap();
            if f(&s) {
                return s;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        panic!("timed out: {:?}", status(core).unwrap());
    }

    #[test]
    fn share_on_one_core_and_get_on_another() {
        std::env::set_var("MARQUEE_TORRENT_HOST", "127.0.0.1");
        let tmp = tempfile::tempdir().unwrap();
        let a = open(&tmp.path().join("a"));
        let b = open(&tmp.path().join("b"));

        // A has a DS game in its library.
        let lib = tmp.path().join("a-roms/ds");
        std::fs::create_dir_all(&lib).unwrap();
        let bytes: Vec<u8> = (0..3_000_000u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
            .collect();
        std::fs::write(lib.join("Test Game (Spain).nds"), &bytes).unwrap();
        a.add_root("ds", lib.clone(), None).unwrap();
        a.scan(None, None).unwrap();
        let edition = a.snapshot().unwrap().editions[0].id.clone();

        share(&a, &edition).unwrap();
        let s = wait(&a, |s| {
            s.items
                .iter()
                .any(|i| i.state == "sharing" && i.link.is_some())
        });
        let link = s.items[0].link.clone().unwrap();
        assert!(link.contains("x.mq=ds") && link.contains("x.pe=127.0.0.1:"));

        get(&b, &link).unwrap();
        b.transfers
            .inner
            .update(|entries| entries[0].system = Some("not-a-system".into()))
            .unwrap();
        wait(&b, |s| {
            s.items.iter().any(|item| {
                item.error
                    .as_deref()
                    .is_some_and(|e| e.contains("library scan failed"))
            })
        });
        assert!(!b.transfers.inner.load().unwrap()[0].scanned);
        b.transfers
            .inner
            .update(|entries| entries[0].system = Some("ds".into()))
            .unwrap();
        for _ in 0..100 {
            status(&b).unwrap();
            if b.transfers.inner.load().unwrap()[0].scanned {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(b.transfers.inner.load().unwrap()[0].scanned);
        assert_eq!(b.snapshot().unwrap().editions.len(), 1);
        wait(&b, |s| {
            s.items
                .iter()
                .any(|i| i.kind == "get" && i.state == "sharing")
        });
        let hash = parse_link(&link).unwrap().hash;
        let got = b
            .transfers
            .inner
            .download_dir(&hash)
            .join("Test Game (Spain).nds");
        assert_eq!(std::fs::read(got).unwrap(), bytes);
        assert!(wait(&a, |s| s.items[0].uploaded > 0).items[0].uploaded > 0);

        stop(&a, &s.items[0].info_hash).unwrap();
        assert_eq!(status(&a).unwrap().items[0].state, "paused");
        assert!(
            lib.join("Test Game (Spain).nds").exists(),
            "stopping must keep the game"
        );
    }

    #[test]
    fn transfer_is_refused_while_off() {
        let tmp = tempfile::tempdir().unwrap();
        let c = Arc::new(Core::open_isolated(Some(tmp.path().to_path_buf()), true).unwrap());
        assert_eq!(share(&c, "x").unwrap_err().code, "TORRENT_OFF");
        assert_eq!(
            get(
                &c,
                "magnet:?xt=urn:btih:4927b4f8762d3727d8be7d48e754485c7f091436&x.pe=127.0.0.1:1"
            )
            .unwrap_err()
            .code,
            "TORRENT_OFF"
        );
        assert!(!status(&c).unwrap().enabled);
    }

    #[test]
    fn rejects_things_that_are_not_game_links() {
        assert!(parse_link("https://example.com").is_err());
        assert!(parse_link("magnet:?xt=urn:btih:short").is_err());
        assert!(parse_link("magnet:?dn=x").is_err());
    }

    #[test]
    fn malformed_unicode_percent_escapes_never_panic() {
        for value in ["%€", "%aé", "%", "%GG", "%FF"] {
            let link =
                format!("magnet:?xt=urn:btih:4927b4f8762d3727d8be7d48e754485c7f091436&dn={value}");
            assert!(parse_link(&link).is_err());
        }
        assert_eq!(decode("caf%C3%A9+🎮").unwrap(), "café 🎮");
        assert_eq!(
            magnet_hash("magnet:?xt=urn:btih:JET3J6DWFU3SPWF6PVEOOVCILR7QSFBW").unwrap(),
            "4927b4f8762d3727d8be7d48e754485c7f091436"
        );
        assert!(parse_link(
            "magnet:?xt=urn:btih:4927b4f8762d3727d8be7d48e754485c7f091436&x.pe=invalid"
        )
        .is_err());
    }

    #[test]
    fn corrupt_saved_transfers_are_reported_and_never_replaced() {
        let temp = tempfile::tempdir().unwrap();
        let transfers = Transfers::new(temp.path());
        let inner = &transfers.inner;
        std::fs::create_dir_all(&inner.dir).unwrap();
        std::fs::write(inner.entries_path(), b"broken state").unwrap();
        assert_eq!(inner.load().err().unwrap().code, "TORRENT_STATE");
        assert!(inner.update(|v| v.clear()).is_err());
        assert_eq!(
            std::fs::read(inner.entries_path()).unwrap(),
            b"broken state"
        );
    }

    #[test]
    fn state_write_errors_are_not_successful_mutations() {
        let temp = tempfile::tempdir().unwrap();
        let transfers = Transfers::new(temp.path());
        std::fs::write(&transfers.inner.dir, b"not a directory").unwrap();
        assert!(transfers.inner.update(|v| v.clear()).is_err());
    }

    #[test]
    fn failed_settings_save_does_not_partially_enable_network_discovery() {
        let temp = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(temp.path().to_path_buf()), true).unwrap();
        let original = core.torrent_settings().unwrap();
        core.with_db(|db| {
            db.conn.execute_batch("CREATE TEMP TRIGGER reject_torrent_save BEFORE INSERT ON settings WHEN NEW.key='torrent.download_kbps' BEGIN SELECT RAISE(ABORT,'simulated save failure'); END;")?;
            Ok(())
        }).unwrap();
        let changed = TorrentSettings {
            enabled: true,
            trackers: true,
            dht: true,
            ..original.clone()
        };
        assert!(core.set_torrent_settings(&changed).is_err());
        assert_eq!(core.torrent_settings().unwrap(), original);
        assert!(core.transfers.inner.engines.lock().unwrap().is_empty());
    }

    #[test]
    fn reviewed_file_download_uses_tracker_and_retains_history_after_source_deletion() {
        use std::sync::atomic::{AtomicBool, AtomicUsize};
        let temp = tempfile::tempdir().unwrap();
        let source_dir = temp.path().join("source");
        std::fs::create_dir_all(&source_dir).unwrap();
        let payload: Vec<u8> = (0..8 * 1024 * 1024u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
            .collect();
        let source = source_dir.join("Synthetic package.pkg");
        std::fs::write(&source, &payload).unwrap();
        let mut cfg = Config::private(&source_dir);
        cfg.listen = "127.0.0.1:0".parse().unwrap();
        let seed = Engine::start(&cfg).unwrap();
        let tracker = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}/announce", tracker.server_addr());
        let made = seed.create(&source, vec![url]).unwrap();
        let seed_id = seed.seed(made.bytes.clone(), &source_dir).unwrap();
        seed.wait_finished(seed_id, std::time::Duration::from_secs(5))
            .unwrap();
        let peer = seed.listen_addr().unwrap();
        let requests = Arc::new(AtomicUsize::new(0));
        let done = Arc::new(AtomicBool::new(false));
        let requests2 = requests.clone();
        let done2 = done.clone();
        let tracker_thread = std::thread::spawn(move || {
            while !done2.load(Ordering::SeqCst) {
                if let Ok(Some(request)) =
                    tracker.recv_timeout(std::time::Duration::from_millis(50))
                {
                    requests2.fetch_add(1, Ordering::SeqCst);
                    let mut response = b"d8:intervali60e5:peers6:".to_vec();
                    response.extend_from_slice(&[127, 0, 0, 1]);
                    response.extend_from_slice(&peer.port().to_be_bytes());
                    response.push(b'e');
                    let _ = request.respond(tiny_http::Response::from_data(response));
                }
            }
        });
        let torrent = temp.path().join("package.torrent");
        std::fs::write(&torrent, &made.bytes).unwrap();
        let data = temp.path().join("receiver");
        let receiver = open(&data);
        let reviewed = review(&receiver, &torrent).unwrap();
        assert_eq!(reviewed.manifest.files.len(), 1);
        assert_eq!(reviewed.manifest.total, payload.len() as u64);
        assert!(
            receiver.transfers.inner.engines.lock().unwrap().is_empty(),
            "review must not start networking"
        );
        assert_eq!(
            add_file(&receiver, &torrent, &reviewed.review_token, vec![])
                .unwrap_err()
                .code,
            "TORRENT_DISCOVERY"
        );
        assert_eq!(
            requests.load(Ordering::SeqCst),
            0,
            "offline review must not announce to a tracker"
        );
        let mut settings = receiver.torrent_settings().unwrap();
        settings.trackers = true;
        settings.seeding = false;
        settings.download_kbps = 1024;
        receiver.set_torrent_settings(&settings).unwrap();
        let mut changed = made.bytes.clone();
        changed.push(b' ');
        std::fs::write(&torrent, changed).unwrap();
        assert_eq!(
            add_file(&receiver, &torrent, &reviewed.review_token, vec![])
                .unwrap_err()
                .code,
            "TORRENT_REVIEW_CHANGED"
        );
        std::fs::write(&torrent, &made.bytes).unwrap();
        add_file(&receiver, &torrent, &reviewed.review_token, vec![]).unwrap();
        let partial = wait(&receiver, |s| {
            s.items
                .iter()
                .any(|i| i.state == "downloading" && i.downloaded > 0 && i.downloaded < i.total)
        });
        assert!(partial.items[0].downloaded < payload.len() as u64);
        pause(&receiver, &made.info_hash, true).unwrap();
        drop(receiver);
        let receiver = Arc::new(Core::open_isolated(Some(data.clone()), true).unwrap());
        assert_eq!(status(&receiver).unwrap().items[0].state, "paused");
        assert!(
            status(&receiver).unwrap().items[0].downloaded > 0,
            "paused progress must survive restart"
        );
        settings.download_kbps = 0;
        receiver.set_torrent_settings(&settings).unwrap();
        pause(&receiver, &made.info_hash, false).unwrap();
        wait(&receiver, |s| s.items.iter().any(|i| i.state == "complete"));
        assert!(requests.load(Ordering::SeqCst) > 0);
        assert_eq!(
            std::fs::read(reviewed.directory.join("Synthetic package.pkg")).unwrap(),
            payload
        );
        assert!(
            receiver.snapshot().unwrap().editions.is_empty(),
            "a package download is not an installation"
        );
        stop(&receiver, &made.info_hash).unwrap();
        assert_eq!(status(&receiver).unwrap().items[0].state, "paused");
        std::fs::remove_file(reviewed.directory.join("Synthetic package.pkg")).unwrap();
        assert_eq!(status(&receiver).unwrap().items[0].state, "source_missing");
        // Wait for the completion receipt update before releasing the instance.
        for _ in 0..100 {
            if receiver.transfers.inner.load().unwrap()[0].scanned {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        drop(receiver);
        let receiver = Arc::new(Core::open_isolated(Some(data), true).unwrap());
        assert_eq!(status(&receiver).unwrap().items[0].state, "source_missing");
        pause(&receiver, &made.info_hash, false).unwrap();
        wait(&receiver, |s| s.items.iter().any(|i| i.state == "complete"));
        assert_eq!(
            std::fs::read(reviewed.directory.join("Synthetic package.pkg")).unwrap(),
            payload
        );
        assert!(!receiver.transfers.inner.config(&settings, false).trackers);
        assert!(!receiver.transfers.inner.config(&settings, false).dht);
        done.store(true, Ordering::SeqCst);
        tracker_thread.join().unwrap();
        seed.shutdown();
    }

    #[test]
    fn unowned_download_folder_and_symlink_are_refused_without_overwriting() {
        let temp = tempfile::tempdir().unwrap();
        let transfers = Transfers::new(temp.path());
        let hash = "4927b4f8762d3727d8be7d48e754485c7f091436";
        let dir = transfers.inner.download_dir(hash);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("keep"), b"user data").unwrap();
        assert!(transfers.inner.owned_dir_at(hash, &dir).is_err());
        assert_eq!(std::fs::read(dir.join("keep")).unwrap(), b"user data");
        std::fs::remove_dir_all(&dir).unwrap();
        transfers.inner.owned_dir_at(hash, &dir).unwrap();
        std::os::unix::fs::symlink(temp.path(), dir.join("escape")).unwrap();
        assert!(transfers.inner.owned_dir_at(hash, &dir).is_err());
    }
}
