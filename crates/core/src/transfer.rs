//! Library transfer between the user's own devices, over BitTorrent.
//!
//! The UI exposes three things: **share** a game (make a torrent, start
//! seeding, get a link), **get** a game from a link, and a status list. Nothing
//! runs unless the user turned it on in settings. The engine starts lazily,
//! restores what was being shared or fetched, and stays private: no DHT, no
//! trackers; peers come from the address inside the link.
//!
//! Link: `magnet:?xt=urn:btih:<hash>&dn=<name>&x.pe=<host:port>&x.mq=<system>`.
//! `x.pe` is the standard magnet "peer address" field; `x.mq` is Marquee's own
//! hint for which library folder a received game belongs in.

use crate::error::{AppError, Result};
use crate::model::TorrentSettings;
use crate::Core;
use marquee_torrent::{Config, Engine, Source, State};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const PORT: u16 = 51413;

#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    /// `share` (we have the files) or `get` (we are fetching them).
    kind: String,
    info_hash: String,
    name: String,
    edition_id: Option<String>,
    system: Option<String>,
    /// Folder that holds `name`.
    dir: PathBuf,
    peers: Vec<SocketAddr>,
    #[serde(default)]
    scanned: bool,
}

struct Job {
    edition_id: String,
    name: String,
    error: Option<String>,
}

struct Inner {
    dir: PathBuf,
    engine: Mutex<Option<Arc<Engine>>>,
    jobs: Mutex<Vec<Job>>,
    file: Mutex<()>,
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
    /// preparing, checking, downloading, sharing, paused, error, starting
    pub state: String,
    pub downloaded: u64,
    pub total: u64,
    pub uploaded: u64,
    pub link: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferStatus {
    pub enabled: bool,
    pub seeding: bool,
    pub items: Vec<TransferItem>,
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

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
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
    let q = link.trim().strip_prefix("magnet:?").ok_or_else(bad)?;
    let (mut hash, mut name, mut system, mut peers) = (None, String::new(), None, Vec::new());
    for part in q.split('&') {
        let Some((k, v)) = part.split_once('=') else { continue };
        let v = decode(v);
        match k {
            "xt" => hash = v.strip_prefix("urn:btih:").map(str::to_lowercase),
            "dn" => name = v,
            "x.mq" => system = Some(v),
            "x.pe" => {
                if let Ok(a) = v.parse() {
                    peers.push(a)
                }
            }
            _ => {}
        }
    }
    let hash = hash
        .filter(|h| h.len() == 40 && h.bytes().all(|c| c.is_ascii_hexdigit()))
        .ok_or_else(bad)?;
    Ok(Link { hash, name, system, peers })
}

impl Transfers {
    pub fn new(data_dir: &Path) -> Self {
        Transfers {
            inner: Arc::new(Inner {
                dir: data_dir.join("torrents"),
                engine: Mutex::new(None),
                jobs: Mutex::new(Vec::new()),
                file: Mutex::new(()),
            }),
        }
    }

    /// Start in the background when something was being shared or fetched.
    pub fn autostart(&self, t: &TorrentSettings) {
        if t.enabled && !self.inner.load().is_empty() {
            let inner = Arc::clone(&self.inner);
            let t = t.clone();
            std::thread::spawn(move || {
                let _ = inner.engine(&t);
            });
        }
    }

    /// Stop the engine; it restarts on next use with the new settings.
    pub fn reset(&self, t: &TorrentSettings) {
        let old = self.inner.engine.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(engine) = old {
            engine.cancel();
            std::thread::spawn(move || drop(engine));
        }
        self.autostart(t);
    }
}

impl Inner {
    fn entries_path(&self) -> PathBuf {
        self.dir.join("entries.json")
    }

    fn load(&self) -> Vec<Entry> {
        let _g = self.file.lock().unwrap_or_else(|e| e.into_inner());
        std::fs::read(self.entries_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn update<R>(&self, f: impl FnOnce(&mut Vec<Entry>) -> R) -> R {
        let _g = self.file.lock().unwrap_or_else(|e| e.into_inner());
        let mut v: Vec<Entry> = std::fs::read(self.entries_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let r = f(&mut v);
        let _ = std::fs::create_dir_all(&self.dir);
        let _ = std::fs::write(self.entries_path(), serde_json::to_vec_pretty(&v).unwrap_or_default());
        r
    }

    fn torrent_file(&self, hash: &str) -> PathBuf {
        self.dir.join(format!("{hash}.torrent"))
    }

    fn config(&self, t: &TorrentSettings, port: u16) -> Config {
        let mut cfg = Config::private(self.dir.join("incoming"));
        cfg.listen = SocketAddr::from(([0, 0, 0, 0], port));
        cfg.seeding = t.seeding;
        cfg.upload_bps = (t.upload_kbps > 0).then(|| t.upload_kbps.saturating_mul(1024));
        cfg.download_bps = (t.download_kbps > 0).then(|| t.download_kbps.saturating_mul(1024));
        cfg
    }

    /// The running engine, starting it (and restoring saved transfers) if needed.
    fn engine(&self, t: &TorrentSettings) -> Result<Arc<Engine>> {
        if !t.enabled {
            return Err(off());
        }
        let mut slot = self.engine.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(e) = slot.as_ref() {
            return Ok(Arc::clone(e));
        }
        let engine = match Engine::start(&self.config(t, PORT)) {
            Ok(e) => e,
            Err(_) => Engine::start(&self.config(t, 0)).map_err(fail)?,
        };
        let engine = Arc::new(engine);
        for e in self.load() {
            let res = if e.kind == "share" {
                std::fs::read(self.torrent_file(&e.info_hash))
                    .map_err(|x| anyhow::anyhow!("{x}"))
                    .and_then(|b| engine.seed(b, &e.dir))
            } else {
                engine.download(
                    Source::Link(format!("magnet:?xt=urn:btih:{}", e.info_hash)),
                    &e.dir,
                    e.peers.clone(),
                )
            };
            if let Err(err) = res {
                eprintln!("marquee: could not restore transfer {}: {err:#}", e.name);
            }
        }
        *slot = Some(Arc::clone(&engine));
        Ok(engine)
    }
}

fn peer_string(engine: &Engine) -> Option<String> {
    engine
        .listen_addr()
        .map(|a| format!("{}:{}", local_host(), a.port()))
}

/// Status for the UI. Also kicks off one library rescan per finished download.
pub fn status(core: &Arc<Core>) -> Result<TransferStatus> {
    let t = core.torrent_settings()?;
    let inner = &core.transfers.inner;
    let entries = inner.load();
    let mut items = Vec::new();
    let live = if t.enabled {
        match inner.engine.try_lock() {
            Ok(g) => g.clone(),
            Err(_) => None,
        }
    } else {
        None
    };
    if t.enabled && live.is_none() && !entries.is_empty() {
        core.transfers.autostart(&t);
    }
    let progress = live.as_ref().map(|e| e.list()).unwrap_or_default();
    let peer = live.as_deref().and_then(peer_string);
    for e in &entries {
        let p = progress.iter().find(|p| p.info_hash == e.info_hash);
        let (state, error) = match p.map(|p| &p.state) {
            None => ("starting".to_string(), None),
            Some(State::Checking) => ("checking".into(), None),
            Some(State::Downloading) => ("downloading".into(), None),
            Some(State::Seeding) => ("sharing".into(), None),
            Some(State::Paused) => ("paused".into(), None),
            Some(State::Error(m)) => ("error".into(), Some(m.clone())),
        };
        if e.kind == "get" && !e.scanned && p.is_some_and(|p| p.finished) {
            let hash = e.info_hash.clone();
            inner.update(|v| v.iter_mut().filter(|x| x.info_hash == hash).for_each(|x| x.scanned = true));
            let core = Arc::clone(core);
            std::thread::spawn(move || {
                let _ = core.scan(None, None);
            });
        }
        items.push(TransferItem {
            info_hash: e.info_hash.clone(),
            name: e.name.clone(),
            kind: e.kind.clone(),
            edition_id: e.edition_id.clone(),
            state,
            downloaded: p.map_or(0, |p| p.downloaded),
            total: p.map_or(0, |p| p.total),
            uploaded: p.map_or(0, |p| p.uploaded),
            link: peer
                .as_deref()
                .map(|peer| make_link(&e.info_hash, &e.name, e.system.as_deref(), Some(peer))),
            error,
        });
    }
    for j in inner.jobs.lock().unwrap_or_else(|e| e.into_inner()).iter() {
        items.push(TransferItem {
            info_hash: String::new(),
            name: j.name.clone(),
            kind: "share".into(),
            edition_id: Some(j.edition_id.clone()),
            state: if j.error.is_some() { "error" } else { "preparing" }.into(),
            downloaded: 0,
            total: 0,
            uploaded: 0,
            link: None,
            error: j.error.clone(),
        });
    }
    Ok(TransferStatus { enabled: t.enabled, seeding: t.seeding, items })
}

/// Make a torrent for a game in the library and start sharing it. Returns at
/// once; hashing a large image happens in the background (state `preparing`).
pub fn share(core: &Arc<Core>, edition_id: &str) -> Result<()> {
    let t = core.torrent_settings()?;
    if !t.enabled {
        return Err(off());
    }
    if !t.seeding {
        return Err(AppError::new(
            "TORRENT_SEEDING_OFF",
            "Sharing is turned off in Settings.",
        ));
    }
    let inner = Arc::clone(&core.transfers.inner);
    if inner.load().iter().any(|e| e.edition_id.as_deref() == Some(edition_id)) {
        return Ok(());
    }
    let snap = core.snapshot()?;
    let edition = snap
        .editions
        .iter()
        .find(|e| e.id == edition_id)
        .ok_or_else(|| AppError::new("NOT_FOUND", "Game not found"))?;
    let media = snap
        .media
        .iter()
        .find(|m| m.edition_id == edition_id && m.available)
        .ok_or_else(|| AppError::new("MEDIA_MISSING", "This game has no file to share"))?;
    let root = snap
        .roots
        .iter()
        .filter(|r| media.path.starts_with(&r.path))
        .max_by_key(|r| r.path.as_os_str().len());
    // A game that lives in its own folder (cue + bin) is shared as the folder.
    let target = match root.and_then(|r| media.path.strip_prefix(&r.path).ok().map(|rel| (r, rel))) {
        Some((r, rel)) if rel.components().count() > 1 => {
            r.path.join(rel.components().next().unwrap())
        }
        _ => media.path.clone(),
    };
    let dir = target.parent().map(Path::to_path_buf).unwrap_or_default();
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let system = edition.system.clone();
    inner.jobs.lock().unwrap_or_else(|e| e.into_inner()).push(Job {
        edition_id: edition_id.to_string(),
        name: name.clone(),
        error: None,
    });
    let edition_id = edition_id.to_string();
    std::thread::spawn(move || {
        let result: Result<()> = (|| {
            let engine = inner.engine(&t)?;
            let made = engine.create(&target, vec![]).map_err(fail)?;
            engine.seed(made.bytes.clone(), &dir).map_err(fail)?;
            std::fs::create_dir_all(&inner.dir)?;
            std::fs::write(inner.torrent_file(&made.info_hash), &made.bytes)?;
            inner.update(|v| {
                v.retain(|e| e.info_hash != made.info_hash);
                v.push(Entry {
                    kind: "share".into(),
                    info_hash: made.info_hash.clone(),
                    name: name.clone(),
                    edition_id: Some(edition_id.clone()),
                    system: Some(system.clone()),
                    dir: dir.clone(),
                    peers: vec![],
                    scanned: true,
                })
            });
            Ok(())
        })();
        let mut jobs = inner.jobs.lock().unwrap_or_else(|e| e.into_inner());
        match result {
            Ok(()) => jobs.retain(|j| j.edition_id != edition_id),
            Err(e) => {
                if let Some(j) = jobs.iter_mut().find(|j| j.edition_id == edition_id) {
                    j.error = Some(e.message);
                }
            }
        }
    });
    Ok(())
}

/// Fetch a game from a link copied from another device.
pub fn get(core: &Arc<Core>, link: &str) -> Result<()> {
    let t = core.torrent_settings()?;
    let l = parse_link(link)?;
    let engine = core.transfers.inner.engine(&t)?;
    if l.peers.is_empty() {
        return Err(AppError::new(
            "TORRENT_LINK",
            "This link has no address for the other device, so there is nobody to fetch from.",
        ));
    }
    let dir = match l.system.as_deref() {
        Some(s) => core.intake_root(s)?,
        None => core.transfers.inner.dir.join("incoming"),
    };
    engine
        .download(
            Source::Link(format!("magnet:?xt=urn:btih:{}", l.hash)),
            &dir,
            l.peers.clone(),
        )
        .map_err(fail)?;
    let (hash, name) = (l.hash.clone(), if l.name.is_empty() { l.hash.clone() } else { l.name.clone() });
    core.transfers.inner.update(|v| {
        v.retain(|e| e.info_hash != hash);
        v.push(Entry {
            kind: "get".into(),
            info_hash: hash,
            name,
            edition_id: None,
            system: l.system.clone(),
            dir,
            peers: l.peers,
            scanned: false,
        })
    });
    Ok(())
}

/// Stop sharing or fetching. Never deletes game files.
pub fn stop(core: &Arc<Core>, info_hash: &str) -> Result<()> {
    let inner = &core.transfers.inner;
    let live = inner.engine.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(engine) = live {
        if let Some(p) = engine.list().into_iter().find(|p| p.info_hash == info_hash) {
            engine.remove(p.id, false).map_err(fail)?;
        }
    }
    inner.update(|v| v.retain(|e| e.info_hash != info_hash));
    let _ = std::fs::remove_file(inner.torrent_file(info_hash));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_round_trips() {
        let h = "4927b4f8762d3727d8be7d48e754485c7f091436";
        let l = make_link(h, "Grand Theft Auto (USA) [v4].iso", Some("ps2"), Some("192.168.1.20:51413"));
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
        let bytes: Vec<u8> = (0..3_000_000u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
        std::fs::write(lib.join("Test Game (Spain).nds"), &bytes).unwrap();
        a.add_root("ds", lib.clone(), None).unwrap();
        a.scan(None, None).unwrap();
        let edition = a.snapshot().unwrap().editions[0].id.clone();

        share(&a, &edition).unwrap();
        let s = wait(&a, |s| s.items.iter().any(|i| i.state == "sharing" && i.link.is_some()));
        let link = s.items[0].link.clone().unwrap();
        assert!(link.contains("x.mq=ds") && link.contains("x.pe=127.0.0.1:"));

        get(&b, &link).unwrap();
        wait(&b, |s| s.items.iter().any(|i| i.kind == "get" && i.state == "sharing"));
        let got = b.paths.data_dir.join("roms/ds/Test Game (Spain).nds");
        assert_eq!(std::fs::read(got).unwrap(), bytes);
        assert!(wait(&a, |s| s.items[0].uploaded > 0).items[0].uploaded > 0);

        stop(&a, &s.items[0].info_hash).unwrap();
        assert!(status(&a).unwrap().items.is_empty());
        assert!(lib.join("Test Game (Spain).nds").exists(), "stopping must keep the game");
    }

    #[test]
    fn transfer_is_refused_while_off() {
        let tmp = tempfile::tempdir().unwrap();
        let c = Arc::new(Core::open_isolated(Some(tmp.path().to_path_buf()), true).unwrap());
        assert_eq!(share(&c, "x").unwrap_err().code, "TORRENT_OFF");
        assert_eq!(get(&c, "magnet:?xt=urn:btih:4927b4f8762d3727d8be7d48e754485c7f091436&x.pe=127.0.0.1:1").unwrap_err().code, "TORRENT_OFF");
        assert!(!status(&c).unwrap().enabled);
    }

    #[test]
    fn rejects_things_that_are_not_game_links() {
        assert!(parse_link("https://example.com").is_err());
        assert!(parse_link("magnet:?xt=urn:btih:short").is_err());
        assert!(parse_link("magnet:?dn=x").is_err());
    }
}
