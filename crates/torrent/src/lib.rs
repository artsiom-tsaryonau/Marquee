//! Optional BitTorrent transport for moving a personal game library between
//! the user's own devices. Embeds `librqbit` behind a small synchronous API so
//! `marquee-core` stays free of async code.
//!
//! Defaults are conservative: no DHT, no trackers, no local-network discovery
//! and no UPnP. Peers are reached only through addresses the caller supplies
//! (or a tracker the caller explicitly configures when creating a torrent).
//! Nothing here fetches games from third parties.

use std::net::SocketAddr;
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
    /// Public DHT. Off by default; the library is private.
    pub dht: bool,
    /// Tracker announces. Off by default.
    pub trackers: bool,
    /// Serve pieces to others. Turn off for download-only.
    pub seeding: bool,
    /// Bytes per second; None is unlimited.
    pub upload_bps: Option<u32>,
    pub download_bps: Option<u32>,
}

impl Config {
    pub fn private(download_dir: impl Into<PathBuf>) -> Self {
        Config {
            download_dir: download_dir.into(),
            listen: "0.0.0.0:0".parse().unwrap(),
            dht: false,
            trackers: false,
            seeding: true,
            upload_bps: None,
            download_bps: None,
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
}

/// A created torrent: the `.torrent` bytes plus its identity.
#[derive(Clone, Debug)]
pub struct Created {
    pub bytes: Vec<u8>,
    pub info_hash: String,
    pub magnet: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct FileSummary {
    pub path: String,
    pub size: u64,
}

#[derive(Clone, Debug, serde::Serialize)]
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
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("marquee-torrent")
            .build()
            .context("starting the torrent runtime")?;
        std::fs::create_dir_all(&cfg.download_dir)?;
        let mut opts = SessionOptions {
            dht: if cfg.dht {
                Some(Default::default())
            } else {
                None
            },
            disable_trackers: !cfg.trackers,
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
        self.add(
            AddTorrent::TorrentFileBytes(torrent.into()),
            data_root,
            vec![],
            true,
        )
    }

    /// Download from a `.torrent` file's bytes, a magnet link or an http(s) URL,
    /// into `into`, connecting to `peers` directly.
    pub fn download(&self, source: Source, into: &Path, peers: Vec<SocketAddr>) -> Result<usize> {
        let add = match source {
            Source::TorrentBytes(b) => AddTorrent::TorrentFileBytes(b.into()),
            Source::Link(l) => AddTorrent::Url(l.into()),
        };
        self.add(add, into, peers, false)
    }

    /// Resume only inside a caller-validated, exclusively owned transfer folder.
    pub fn download_owned(
        &self,
        source: Source,
        into: &Path,
        peers: Vec<SocketAddr>,
    ) -> Result<usize> {
        let add = match source {
            Source::TorrentBytes(bytes) => {
                inspect(&bytes)?;
                AddTorrent::TorrentFileBytes(bytes.into())
            }
            Source::Link(link) => AddTorrent::Url(link.into()),
        };
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
    /// A `magnet:` link or an http(s) URL to a `.torrent`.
    Link(String),
}
