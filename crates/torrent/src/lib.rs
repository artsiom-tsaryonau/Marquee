//! Optional BitTorrent transport for moving a personal game library between
//! the user's own devices. Embeds `librqbit` behind a small synchronous API so
//! `marquee-core` stays free of async code.
//!
//! Transfers are confined to private, link-local and loopback addresses. DHT,
//! trackers, automatic discovery and UPnP are disabled. Both incoming and
//! outgoing peer connections pass the mandatory local address filter.
//! Nothing here fetches games from third parties.

use std::io::Write;
use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use librqbit::limits::LimitsConfig;
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, CreateTorrentOptions, ListenerMode,
    ListenerOptions, ManagedTorrent, Session, SessionOptions, TorrentStatsState,
};
use tokio::runtime::Runtime;

pub use librqbit::spawn_utils::BlockingSpawner;

/// How the engine is configured. `Default` is the private, quiet profile.
#[derive(Clone, Debug)]
pub struct Config {
    /// Folder where finished downloads land unless a call overrides it.
    pub download_dir: PathBuf,
    /// Address to accept peers on. Port 0 picks a free one.
    pub listen: SocketAddr,
    /// Legacy field: enabling public DHT is rejected.
    pub dht: bool,
    /// Legacy field: enabling tracker announces is rejected.
    pub trackers: bool,
    /// Serve pieces to others. Turn off for download-only.
    pub seeding: bool,
    /// Bytes per second; None is unlimited.
    pub upload_bps: Option<u32>,
    pub download_bps: Option<u32>,
    /// Legacy field: proxies are rejected for LAN-only transfers.
    pub proxy: Option<String>,
}

impl Config {
    pub fn private(download_dir: impl Into<PathBuf>) -> Self {
        Config {
            download_dir: download_dir.into(),
            listen: "127.0.0.1:0".parse().unwrap(),
            dht: false,
            trackers: false,
            seeding: true,
            upload_bps: None,
            download_bps: None,
            proxy: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Checking,
    Downloading,
    Seeding,
    Paused,
    Error(String),
}

#[derive(Clone, Debug)]
pub struct Progress {
    pub id: usize,
    pub name: String,
    pub info_hash: String,
    pub state: State,
    pub downloaded: u64,
    pub total: u64,
    pub uploaded: u64,
    pub finished: bool,
    pub download_bps: u64,
    pub upload_bps: u64,
    pub peers: usize,
}

/// Only addresses confined to this computer or a private/link-local network.
pub fn is_local_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_private() || ip.is_loopback() || ip.is_link_local(),
        IpAddr::V6(ip) => ip.to_ipv4_mapped().map_or_else(
            || ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local(),
            |ip| is_local_ip(IpAddr::V4(ip)),
        ),
    }
}

pub fn check_local_peer(peer: SocketAddr) -> Result<()> {
    anyhow::ensure!(peer.port() != 0 && is_local_ip(peer.ip()),
        "Transfers are local-network only; use a private or link-local device IP and a nonzero port");
    Ok(())
}

// librqbit applies this list to both inbound and outbound peers, including PEX.
// Load from a local file so constructing a session never fetches network policy.
const LOCAL_PEERS: &str = "loopback:127.0.0.0-127.255.255.255\nprivate:10.0.0.0-10.255.255.255\nprivate:172.16.0.0-172.31.255.255\nprivate:192.168.0.0-192.168.255.255\nlinklocal:169.254.0.0-169.254.255.255\nloopback6:::1-::1\nprivate6:fc00::-fdff:ffff:ffff:ffff:ffff:ffff:ffff:ffff\nlinklocal6:fe80::-febf:ffff:ffff:ffff:ffff:ffff:ffff:ffff\nmapped:::ffff:127.0.0.0-::ffff:127.255.255.255\nmapped:::ffff:10.0.0.0-::ffff:10.255.255.255\nmapped:::ffff:172.16.0.0-::ffff:172.31.255.255\nmapped:::ffff:192.168.0.0-::ffff:192.168.255.255\nmapped:::ffff:169.254.0.0-::ffff:169.254.255.255\n";

/// A created torrent: the `.torrent` bytes plus its identity.
#[derive(Clone, Debug)]
pub struct Created {
    pub bytes: Vec<u8>,
    pub info_hash: String,
    pub magnet: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FileSummary {
    pub path: String,
    pub size: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    pub info_hash: String,
    pub name: String,
    pub total: u64,
    pub files: Vec<FileSummary>,
    pub tracker_count: usize,
    pub private: bool,
}

/// Offline metadata validation: no session, sockets, tracker requests or writes.
pub fn inspect(bytes: &[u8]) -> Result<Manifest> {
    anyhow::ensure!(
        bytes.len() <= 4 * 1024 * 1024,
        "Torrent metadata exceeds 4 MiB"
    );
    let meta = librqbit::torrent_from_bytes(bytes).context("invalid torrent metadata")?;
    let info_hash = meta.info_hash.as_string();
    let tracker_count = meta.iter_announce().count();
    let info = meta
        .info
        .data
        .validate()
        .context("invalid torrent file list")?;
    let name = info
        .name()
        .unwrap_or_else(|| "torrent-content".into())
        .into_owned();
    let mut files = Vec::new();
    let mut total = 0u64;
    for file in info.iter_file_details() {
        anyhow::ensure!(files.len() < 20_000, "Torrent has too many files");
        anyhow::ensure!(
            !file.attrs().symlink && file.symlink_path.is_none(),
            "Torrent symlinks are unsupported"
        );
        let parts = file.filename.to_vec();
        anyhow::ensure!(
            parts.iter().all(|p| !p.is_empty()
                && p != "."
                && p != ".."
                && !p.contains(['/', '\\', ':', '\0'])
                && p.len() <= 255),
            "Unsafe torrent filename"
        );
        total = total
            .checked_add(file.len)
            .ok_or_else(|| anyhow!("Torrent size overflow"))?;
        anyhow::ensure!(total <= 16 * 1024u64.pow(4), "Torrent exceeds 16 TiB");
        files.push(FileSummary {
            path: parts.join("/"),
            size: file.len,
        });
    }
    Ok(Manifest {
        info_hash,
        name,
        total,
        files,
        tracker_count,
        private: info.info().private,
    })
}

pub struct Engine {
    rt: Runtime,
    session: Arc<Session>,
}

fn nz(v: Option<u32>) -> Option<NonZeroU32> {
    v.and_then(NonZeroU32::new)
}

impl Engine {
    pub fn start(cfg: &Config) -> Result<Engine> {
        anyhow::ensure!(
            !cfg.dht && !cfg.trackers && cfg.proxy.is_none(),
            "Marquee transfers are local-network only; DHT, trackers and proxies are unsupported"
        );
        anyhow::ensure!(
            is_local_ip(cfg.listen.ip()),
            "The transfer listener must use a local-network address"
        );
        let mut allowlist = tempfile::NamedTempFile::new()?;
        allowlist.write_all(LOCAL_PEERS.as_bytes())?;
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("marquee-torrent")
            .build()
            .context("starting the torrent runtime")?;
        std::fs::create_dir_all(&cfg.download_dir)?;
        let mut opts = SessionOptions {
            dht: None,
            disable_trackers: true,
            allowlist_url: Some(
                url::Url::from_file_path(allowlist.path())
                    .map_err(|_| anyhow!("Invalid local peer policy path"))?
                    .into(),
            ),
            disable_local_service_discovery: true,
            disable_upload: !cfg.seeding,
            ipv4_only: cfg.listen.is_ipv4(),
            ratelimits: LimitsConfig {
                upload_bps: nz(cfg.upload_bps),
                download_bps: nz(cfg.download_bps),
            },
            ..Default::default()
        };
        opts.listen = Some(ListenerOptions {
            mode: ListenerMode::TcpOnly,
            listen_addr: cfg.listen,
            enable_upnp_port_forwarding: false,
            ipv4_only: cfg.listen.is_ipv4(),
            ..Default::default()
        });
        let dir = cfg.download_dir.clone();
        let session = rt
            .block_on(Session::new_with_opts(dir, opts))
            .context("starting the torrent session")?;
        Ok(Engine { rt, session })
    }

    /// The address peers can reach this engine on.
    pub fn listen_addr(&self) -> Option<SocketAddr> {
        self.session.listen_addr()
    }

    /// Hash a file or folder into a torrent. Slow for very large images; the
    /// work runs on the engine's runtime.
    pub fn create(&self, path: &Path, trackers: Vec<String>) -> Result<Created> {
        let res = self.rt.block_on(async {
            let spawner = BlockingSpawner::new(2);
            librqbit::create_torrent(
                path,
                CreateTorrentOptions {
                    name: None,
                    trackers,
                    piece_length: None,
                },
                &spawner,
            )
            .await
        })?;
        Ok(Created {
            bytes: res.as_bytes()?.to_vec(),
            info_hash: res.info_hash().as_string(),
            magnet: res.as_magnet().to_string(),
        })
    }

    /// Start seeding `torrent` from files that already exist under `data_root`
    /// (the folder that contains the torrent's top-level file or folder).
    /// Existing data is verified, not re-downloaded.
    pub fn seed(&self, torrent: Vec<u8>, data_root: &Path) -> Result<usize> {
        let manifest = inspect(&torrent)?;
        let meta = librqbit::torrent_from_bytes(&torrent)?;
        let root = if meta.info.data.files.is_some() {
            anyhow::ensure!(
                !manifest.name.is_empty()
                    && manifest.name != "."
                    && manifest.name != ".."
                    && !manifest.name.contains(['/', '\\', ':', '\0']),
                "Unsafe torrent root name"
            );
            data_root.join(&manifest.name)
        } else {
            data_root.to_path_buf()
        };
        // An explicit output_folder suppresses librqbit's automatic top folder.
        // Do not point a folder seed at its parent or create missing source files.
        anyhow::ensure!(root.is_dir(), "Seed source directory is missing");
        for file in &manifest.files {
            let meta = std::fs::symlink_metadata(root.join(&file.path))?;
            anyhow::ensure!(
                meta.is_file() && meta.len() == file.size,
                "Seed source file is missing or changed size"
            );
        }
        self.add(
            AddTorrent::TorrentFileBytes(torrent.into()),
            &root,
            vec![],
            true,
        )
    }

    /// Download from local `.torrent` bytes or a device magnet link,
    /// into `into`, connecting to `peers` directly.
    pub fn download(&self, source: Source, into: &Path, peers: Vec<SocketAddr>) -> Result<usize> {
        let (add, peers) = local_source(source, peers)?;
        self.add(add, into, peers, false)
    }

    /// Resume only inside a caller-validated, exclusively owned transfer folder.
    pub fn download_owned(
        &self,
        source: Source,
        into: &Path,
        peers: Vec<SocketAddr>,
    ) -> Result<usize> {
        let (add, peers) = local_source(source, peers)?;
        self.add(add, into, peers, true)
    }

    fn add(
        &self,
        add: AddTorrent<'_>,
        into: &Path,
        peers: Vec<SocketAddr>,
        overwrite: bool,
    ) -> Result<usize> {
        std::fs::create_dir_all(into)?;
        let opts = AddTorrentOptions {
            overwrite,
            output_folder: Some(into.to_string_lossy().into_owned()),
            initial_peers: if peers.is_empty() { None } else { Some(peers) },
            ..Default::default()
        };
        let resp = self
            .rt
            .block_on(async {
                tokio::time::timeout(
                    Duration::from_secs(60),
                    self.session.add_torrent(add, Some(opts)),
                )
                .await
                .context("Timed out retrieving torrent metadata; retry when peers are reachable")?
            })
            .context("adding the torrent")?;
        match resp {
            AddTorrentResponse::Added(id, _) | AddTorrentResponse::AlreadyManaged(id, _) => Ok(id),
            AddTorrentResponse::ListOnly(_) => Err(anyhow!("unexpected list-only response")),
        }
    }

    fn handle(&self, id: usize) -> Result<Arc<ManagedTorrent>> {
        self.session
            .with_torrents(|it| {
                for (tid, h) in it {
                    if tid == id {
                        return Some(h.clone());
                    }
                }
                None
            })
            .ok_or_else(|| anyhow!("no torrent {id}"))
    }

    pub fn progress(&self, id: usize) -> Result<Progress> {
        let h = self.handle(id)?;
        let s = h.stats();
        let state = match (&s.state, s.error.as_deref()) {
            (_, Some(e)) => State::Error(e.to_string()),
            (TorrentStatsState::Initializing { .. }, _) => State::Checking,
            (TorrentStatsState::Paused, _) => State::Paused,
            (TorrentStatsState::Error, _) => State::Error("torrent error".into()),
            (TorrentStatsState::Live, _) if s.finished => State::Seeding,
            (TorrentStatsState::Live, _) => State::Downloading,
        };
        Ok(Progress {
            id,
            name: h.name().unwrap_or_default(),
            info_hash: h.info_hash().as_string(),
            state,
            downloaded: s.progress_bytes,
            total: s.total_bytes,
            uploaded: s.uploaded_bytes,
            finished: s.finished,
            download_bps: s
                .live
                .as_ref()
                .map_or(0, |live| live.download_speed.as_bytes()),
            upload_bps: s
                .live
                .as_ref()
                .map_or(0, |live| live.upload_speed.as_bytes()),
            peers: s
                .live
                .as_ref()
                .map_or(0, |live| live.snapshot.peer_stats.live as usize),
        })
    }

    pub fn list(&self) -> Vec<Progress> {
        let ids: Vec<usize> = self
            .session
            .with_torrents(|it| it.map(|(id, _)| id).collect());
        ids.into_iter()
            .filter_map(|id| self.progress(id).ok())
            .collect()
    }

    pub fn metadata_bytes(&self, id: usize) -> Result<Vec<u8>> {
        self.handle(id)?
            .with_metadata(|meta| meta.torrent_bytes.to_vec())
    }

    /// Block until the download has finished and verified, or `timeout` passes.
    pub fn wait_finished(&self, id: usize, timeout: Duration) -> Result<()> {
        let h = self.handle(id)?;
        self.rt.block_on(async {
            tokio::time::timeout(timeout, h.wait_until_completed())
                .await
                .map_err(|_| anyhow!("timed out"))?
        })
    }

    pub fn pause(&self, id: usize) -> Result<()> {
        let h = self.handle(id)?;
        self.rt.block_on(self.session.pause(&h))
    }

    pub fn resume(&self, id: usize) -> Result<()> {
        let h = self.handle(id)?;
        self.rt.block_on(self.session.unpause(&h))
    }

    /// Stop and forget a torrent. Files stay on disk unless `delete_files`.
    pub fn remove(&self, id: usize, delete_files: bool) -> Result<()> {
        self.rt
            .block_on(self.session.delete(id.into(), delete_files))
    }

    /// Ask every task to stop. The engine can then be dropped on any thread.
    pub fn cancel(&self) {
        self.session.cancellation_token().cancel();
    }

    pub fn shutdown(self) {
        self.session.cancellation_token().cancel();
        self.rt.shutdown_timeout(Duration::from_secs(3));
    }
}

pub enum Source {
    TorrentBytes(Vec<u8>),
    /// A device `magnet:` link with local peers; HTTP URLs are rejected.
    Link(String),
}

fn local_source(
    source: Source,
    mut peers: Vec<SocketAddr>,
) -> Result<(AddTorrent<'static>, Vec<SocketAddr>)> {
    let add = match source {
        Source::TorrentBytes(bytes) => {
            inspect(&bytes)?;
            AddTorrent::TorrentFileBytes(bytes.into())
        }
        Source::Link(link) => {
            let source = url::Url::parse(&link)?;
            anyhow::ensure!(
                source.scheme() == "magnet",
                "Use a local torrent file or device magnet link"
            );
            let mut local = url::Url::parse("magnet:?")?;
            for (key, value) in source.query_pairs() {
                match key.as_ref() {
                    "xt" | "dn" => {
                        local.query_pairs_mut().append_pair(&key, &value);
                    }
                    "x.pe" => peers.push(value.parse()?),
                    _ => {} // No tracker, web-source or metadata URL is forwarded.
                }
            }
            AddTorrent::Url(local.to_string().into())
        }
    };
    anyhow::ensure!(
        !peers.is_empty(),
        "A local device peer is required; public discovery is disabled"
    );
    for &peer in &peers {
        check_local_peer(peer)?;
    }
    Ok((add, peers))
}

/// Recheck the pieces intersecting one file, including bytes in neighboring
/// files at piece boundaries. This is offline integrity, not publisher identity.
/// Returns the SHA-256 of the selected bytes in those verified pieces.
/// The caller supplies a safe file opener rooted in its owned download folder.
pub fn verify_file(
    bytes: &[u8],
    selected: &str,
    mut open: impl FnMut(&str, u64) -> Result<std::fs::File>,
) -> Result<String> {
    use sha1::{Digest, Sha1};
    use std::io::{Read, Seek, SeekFrom};
    let manifest = inspect(bytes)?;
    let meta = librqbit::torrent_from_bytes(bytes)?;
    let info = meta.info.data.validate()?;
    let piece_length = u64::from(info.info().piece_length);
    let mut offset = 0u64;
    let mut selected_range = None;
    for file in &manifest.files {
        if file.path == selected {
            selected_range = Some((offset, offset + file.size));
        }
        offset += file.size;
    }
    let (start, end) =
        selected_range.ok_or_else(|| anyhow!("Selected file is not in this torrent"))?;
    anyhow::ensure!(end > start, "Empty files cannot be prepared");
    let first_piece = start / piece_length;
    let range_start = first_piece * piece_length;
    let range_end = (end.div_ceil(piece_length) * piece_length).min(manifest.total);
    let hashes: &[u8] = info.info().pieces.as_ref();
    let mut position = range_start;
    let mut piece = first_piece;
    let mut digest = Sha1::new();
    let mut selected_digest = sha2::Sha256::new();
    let mut buffer = [0u8; 128 * 1024];
    offset = 0;
    for file in &manifest.files {
        let file_start = offset;
        offset += file.size;
        let from = range_start.max(file_start);
        let to = range_end.min(offset);
        if from >= to {
            continue;
        }
        let mut input = open(&file.path, file.size)?;
        let before = input.metadata()?;
        anyhow::ensure!(
            before.is_file() && before.len() == file.size,
            "Torrent source size changed"
        );
        input.seek(SeekFrom::Start(from - file_start))?;
        let mut remaining = to - from;
        while remaining > 0 {
            let piece_end = ((piece + 1) * piece_length).min(manifest.total);
            let take = remaining.min(piece_end - position).min(buffer.len() as u64) as usize;
            input.read_exact(&mut buffer[..take])?;
            digest.update(&buffer[..take]);
            let selected_start = position.max(start);
            let selected_end = (position + take as u64).min(end);
            if selected_start < selected_end {
                selected_digest.update(
                    &buffer
                        [(selected_start - position) as usize..(selected_end - position) as usize],
                );
            }
            position += take as u64;
            remaining -= take as u64;
            if position == piece_end {
                let expected = hashes
                    .get(piece as usize * 20..piece as usize * 20 + 20)
                    .ok_or_else(|| anyhow!("Missing torrent piece hash"))?;
                anyhow::ensure!(
                    digest.finalize_reset().as_slice() == expected,
                    "Torrent piece {piece} changed; recheck the download before preparation"
                );
                piece += 1;
            }
        }
        let after = input.metadata()?;
        anyhow::ensure!(
            after.len() == before.len() && after.modified()? == before.modified()?,
            "Torrent source changed during piece verification"
        );
    }
    anyhow::ensure!(
        position == range_end,
        "Incomplete torrent piece verification"
    );
    Ok(format!("{:x}", selected_digest.finalize()))
}

#[cfg(test)]
mod network_policy_tests {
    use super::*;

    #[test]
    fn session_filters_incoming_and_discovered_outgoing_peer_addresses() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = Config::private(temp.path());
        config.listen = "127.0.0.1:0".parse().unwrap();
        let engine = Engine::start(&config).unwrap();
        let list = engine
            .session
            .allowlist
            .as_ref()
            .expect("mandatory local peer filter");
        for address in [
            "127.0.0.1",
            "10.255.255.255",
            "172.31.255.255",
            "192.168.1.1",
            "169.254.1.1",
            "::1",
            "fd12::1",
            "fe80::1",
            "::ffff:192.168.1.1",
            "8.8.8.8",
            "172.32.0.1",
            "2001:4860::1",
            "::ffff:8.8.8.8",
        ] {
            let ip = address.parse().unwrap();
            assert_eq!(list.has(ip), is_local_ip(ip), "{address}");
        }
        engine.shutdown();
    }
}
