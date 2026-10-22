//! Torrent UI/library adapter. Only marquee-manager owns engines and history.
use crate::error::{AppError, Result};
use crate::Core;
pub use marquee_manager::torrent::{
    inspect_file, magnet_hash, TorrentReview, TransferItem, TransferStatus,
};
use marquee_manager::torrent::{Request, ShareRequest};
use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct Transfers {
    scans: Mutex<BTreeSet<String>>,
    errors: Mutex<BTreeMap<String, String>>,
}
impl Transfers {
    pub fn new(_data: &Path) -> Self {
        Self::default()
    }
}

pub fn status(core: &Arc<Core>) -> Result<TransferStatus> {
    let mut status: TransferStatus = core.preparation.torrent(Request::Status)?;
    for item in &mut status.items {
        if let Some(publication) = item.publication.clone() {
            if core
                .transfers
                .scans
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(item.info_hash.clone())
            {
                let core = core.clone();
                let hash = item.info_hash.clone();
                let public = item.public;
                std::thread::spawn(move || {
                    let result: Result<()> = (|| {
                        // A package completion is not an installation. Only an
                        // existing private-device system hint uses library scan.
                        if !public {
                            if let Some(system) = &publication.system {
                                crate::catalog::by_id(system)?;
                                core.add_root(system, publication.directory.clone(), None)?;
                                core.scan(None, None)?;
                            }
                        }
                        core.preparation.torrent::<()>(Request::Acknowledge {
                            hash: hash.clone(),
                            directory: publication.directory,
                        })
                    })();
                    let mut errors = core
                        .transfers
                        .errors
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    match result {
                        Ok(()) => {
                            errors.remove(&hash);
                        }
                        Err(e) => {
                            errors.insert(
                                hash.clone(),
                                format!("Downloaded; library scan failed: {}", e.message),
                            );
                        }
                    }
                    core.transfers
                        .scans
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .remove(&hash);
                });
            }
        }
        if item.error.is_none() {
            item.error = core
                .transfers
                .errors
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&item.info_hash)
                .cloned();
        }
    }
    Ok(status)
}

pub fn share(core: &Arc<Core>, edition_id: &str) -> Result<()> {
    let settings = core.torrent_settings()?;
    if !settings.enabled {
        return Err(AppError::new(
            "TORRENT_OFF",
            "BitTorrent transfers are disabled",
        ));
    }
    if !settings.seeding {
        return Err(AppError::new(
            "TORRENT_SEEDING_OFF",
            "Sharing is disabled in Settings",
        ));
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
        Some((root, rel)) if rel.components().count() > 1 => {
            root.path.join(rel.components().next().unwrap())
        }
        _ => media.path.clone(),
    };
    core.preparation.torrent(Request::Share {
        request: ShareRequest {
            target,
            edition_id: edition_id.into(),
            system: edition.system.clone(),
        },
    })
}
pub fn get(core: &Arc<Core>, link: &str) -> Result<()> {
    get_at(core, link, None)
}
pub fn get_at(core: &Arc<Core>, link: &str, destination: Option<PathBuf>) -> Result<()> {
    if let Some(system) = marquee_manager::torrent::magnet_system(link)? {
        crate::catalog::by_id(&system)?;
    }
    core.preparation.torrent(Request::Get {
        link: link.into(),
        destination,
    })
}
pub fn review(core: &Core, path: &Path) -> Result<TorrentReview> {
    let (manifest, review_token) = inspect_file(path)?;
    let directory = core
        .paths
        .data_dir
        .join("torrents/downloads")
        .join(&manifest.info_hash);
    Ok(TorrentReview {
        preparation_files: marquee_manager::torrent::preparation_files(&manifest),
        manifest,
        review_token,
        directory,
    })
}
pub fn add_file(core: &Arc<Core>, path: &Path, token: &str, peers: Vec<SocketAddr>) -> Result<()> {
    add_file_at(core, path, token, peers, None)
}
pub fn add_file_at(
    core: &Arc<Core>,
    path: &Path,
    token: &str,
    peers: Vec<SocketAddr>,
    destination: Option<PathBuf>,
) -> Result<()> {
    core.preparation.torrent(Request::AddFile {
        path: path.into(),
        review_token: token.into(),
        peers,
        destination,
    })
}
pub fn pause(core: &Arc<Core>, hash: &str, paused: bool) -> Result<()> {
    core.preparation.torrent(Request::Pause {
        hash: hash.into(),
        paused,
    })
}
pub fn stop(core: &Arc<Core>, hash: &str) -> Result<()> {
    core.preparation
        .torrent(Request::Stop { hash: hash.into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use marquee_manager::torrent::TorrentSettings;
    use std::time::{Duration, Instant};
    struct Managers(Vec<marquee_manager::Client>);
    impl Drop for Managers {
        fn drop(&mut self) {
            for client in &self.0 {
                let _ = client.stop();
                let deadline = Instant::now() + Duration::from_secs(5);
                while client.status().is_ok_and(|s| s.running) && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }
    fn wait(core: &Arc<Core>, predicate: impl Fn(&TransferStatus) -> bool) -> TransferStatus {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let value = status(core).unwrap();
            if predicate(&value) {
                return value;
            }
            assert!(Instant::now() < deadline, "transfer timed out: {value:?}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    #[test]
    fn detached_private_share_publishes_through_library_owner_and_retries_failed_scan() {
        std::env::set_var("MARQUEE_TORRENT_HOST", "127.0.0.1");
        let temp = tempfile::tempdir().unwrap();
        let a = Arc::new(Core::open_isolated(Some(temp.path().join("a")), true).unwrap());
        let b = Arc::new(Core::open_isolated(Some(temp.path().join("b")), true).unwrap());
        let _managers = Managers(vec![a.preparation.clone(), b.preparation.clone()]);
        let settings = TorrentSettings {
            enabled: true,
            ..Default::default()
        };
        a.set_torrent_settings(&settings).unwrap();
        b.set_torrent_settings(&settings).unwrap();
        let root = temp.path().join("ds");
        std::fs::create_dir(&root).unwrap();
        let bytes = vec![0x5au8; 1024 * 1024];
        std::fs::write(root.join("Test.nds"), &bytes).unwrap();
        a.add_root("ds", root.clone(), None).unwrap();
        a.scan(None, None).unwrap();
        let edition = a.snapshot().unwrap().editions[0].id.clone();
        share(&a, &edition).unwrap();
        let shared = wait(&a, |s| s.items.iter().any(|i| i.link.is_some()));
        let link = shared.items[0].link.clone().unwrap();
        b.with_db(|db| {db.conn.execute_batch("CREATE TEMP TRIGGER reject_received_root BEFORE INSERT ON library_roots BEGIN SELECT RAISE(ABORT,'simulated publication failure'); END;")?;Ok(())}).unwrap();
        get(&b, &link).unwrap();
        wait(&b, |s| {
            s.items.iter().any(|i| {
                i.error
                    .as_deref()
                    .is_some_and(|e| e.contains("library scan failed"))
            })
        });
        assert!(b.snapshot().unwrap().editions.is_empty());
        b.with_db(|db| {
            db.conn
                .execute_batch("DROP TRIGGER reject_received_root;")?;
            Ok(())
        })
        .unwrap();
        let received = wait(&b, |s| {
            s.items.iter().any(|i| {
                i.publication.is_none() && i.error.is_none() && i.files_present == Some(true)
            })
        });
        assert_eq!(
            std::fs::read(received.items[0].directory.join("Test.nds")).unwrap(),
            bytes
        );
        assert_eq!(b.snapshot().unwrap().editions.len(), 1);
        // Torrent CLI uses IPC while this Core still owns the library lease.
        let cli = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("marquee");
        let output = std::process::Command::new(cli)
            .arg("--data-dir")
            .arg(&b.paths.data_dir)
            .args(["torrent", "list"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let list: TransferStatus = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(list.items.len(), 1);
        b.preparation.stop().unwrap();
        assert_eq!(
            b.snapshot().unwrap().editions.len(),
            1,
            "library reads must not require the manager socket"
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while b.preparation.status().unwrap().running {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        std::fs::write(
            b.paths.data_dir.join("manager/torrents/entries.json"),
            b"corrupt history",
        )
        .unwrap();
        assert_eq!(
            b.snapshot().unwrap().editions.len(),
            1,
            "corrupt offline transport history must not block library reads"
        );
        assert!(root.join("Test.nds").exists());
    }
}
