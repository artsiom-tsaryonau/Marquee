//! Storage visibility and future-input settings. Never rewrites receipts or moves data.
use crate::error::{AppError, Result};
use crate::Core;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const COLLECTION: &str = "storage.collection_root";
const COLLECTION_IDENTITY: &str = "storage.collection_identity";
const MAX_ENTRIES: u64 = 250_000;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct DirectoryIdentity {
    device: u64,
    inode: u64,
}
fn directory_identity(path: &Path) -> Result<DirectoryIdentity> {
    let meta = fs::metadata(path)?;
    Ok(DirectoryIdentity {
        device: meta.dev(),
        inode: meta.ino(),
    })
}

#[derive(Clone, Default, Serialize)]
pub struct Usage {
    pub bytes: u64,
    pub files: u64,
    pub skipped_links: u64,
    pub skipped_special: u64,
    pub incomplete: bool,
}

#[derive(Default)]
pub struct ScanState {
    pub running: bool,
    pub measured_at: Option<String>,
    pub usage: BTreeMap<PathBuf, Usage>,
}

#[derive(Serialize)]
pub struct Location {
    pub id: String,
    pub category: String,
    pub label: String,
    pub path: PathBuf,
    pub resolved: Option<PathBuf>,
    pub exists: bool,
    pub free_bytes: Option<u64>,
    pub usage: Option<Usage>,
    pub note: String,
}

#[derive(Serialize)]
pub struct Snapshot {
    pub collection_root: Option<String>,
    pub bios_inbox: Option<String>,
    pub locations: Vec<Location>,
    pub scanning: bool,
    pub measured_at: Option<String>,
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.is_empty())
}

fn nearest_existing(path: &Path) -> Option<&Path> {
    let mut current = path;
    while !current.exists() {
        current = current.parent()?;
    }
    Some(current)
}

fn location(id: &str, category: &str, label: &str, path: PathBuf, note: &str) -> Location {
    let exists = path.exists();
    Location {
        id: id.into(),
        category: category.into(),
        label: label.into(),
        resolved: fs::canonicalize(&path).ok(),
        exists,
        free_bytes: nearest_existing(&path).and_then(crate::intake::free_bytes),
        path,
        usage: None,
        note: note.into(),
    }
}

// A root may itself be a compatibility link; nested links are never traversed.
fn measure(path: &Path) -> Usage {
    let mut result = Usage::default();
    let Ok(root) = fs::canonicalize(path) else {
        result.incomplete = true;
        return result;
    };
    let mut pending = vec![root];
    let mut entries = 0;
    while let Some(path) = pending.pop() {
        if entries >= MAX_ENTRIES {
            result.incomplete = true;
            break;
        }
        entries += 1;
        let Ok(meta) = fs::symlink_metadata(&path) else {
            result.incomplete = true;
            continue;
        };
        if meta.file_type().is_symlink() {
            result.skipped_links += 1;
        } else if meta.is_file() {
            result.bytes = result.bytes.saturating_add(meta.len());
            result.files += 1;
        } else if meta.is_dir() {
            match fs::read_dir(&path) {
                Ok(children) => {
                    for child in children {
                        match child {
                            Ok(child) if pending.len() < MAX_ENTRIES as usize => {
                                pending.push(child.path())
                            }
                            _ => result.incomplete = true,
                        }
                    }
                }
                Err(_) => result.incomplete = true,
            }
        } else {
            result.skipped_special += 1;
        }
    }
    result
}

impl Core {
    pub(crate) fn collection_root(&self) -> Result<Option<PathBuf>> {
        let (root, identity) = self.with_db(|db| {
            Ok((
                nonempty(db.get_setting(COLLECTION)?),
                nonempty(db.get_setting(COLLECTION_IDENTITY)?),
            ))
        })?;
        let Some(root) = root.map(PathBuf::from) else {
            return Ok(None);
        };
        let expected =
            identity.and_then(|value| serde_json::from_str::<DirectoryIdentity>(&value).ok());
        if !root.is_dir()
            || directory_identity(&root).ok().as_ref() != expected.as_ref()
            || expected.is_none()
        {
            return Err(AppError::new("STORAGE_OFFLINE", "The collection folder is unavailable or has changed. Reconnect its disk or choose the folder again in Storage settings."));
        }
        Ok(Some(root))
    }

    pub fn storage_snapshot(&self) -> Result<Snapshot> {
        let (collection_root, bios_inbox, roots, connection) = self.with_db(|db| {
            Ok((
                nonempty(db.get_setting(COLLECTION)?),
                nonempty(db.get_setting("bios_inbox")?),
                db.list_roots()?,
                db.get_setting("streaming.local_connection")?,
            ))
        })?;
        let data = &self.paths.data_dir;
        let mut locations = vec![
            location("database", "library", "Library database", self.paths.db_path(), "Game IDs, metadata, favorites and play history. Back up consistently with SQLite."),
            location("configuration", "library", "Configuration", self.paths.config_dir.clone(), "Contains private provider keys. Keep out of shared packages."),
            location("installations", "library", "Managed installations", data.join("manager/installations"), "Receipt-owned game payloads. Keep this location until a coordinated manager migration is available."),
            location("programs", "programs", "Emulator programs", data.join("runners"), "Keep each installed build with its libraries, resources and receipt."),
            location("bios-inbox", "firmware", "BIOS import inbox", data.join("bios-inbox"), "Inputs are copied into the appropriate emulator profile; this is not installed firmware."),
            location("ps1-bios", "firmware", "PlayStation BIOS", self.paths.duckstation_bios_dir(), "Working DuckStation BIOS files."),
            location("ps2-bios", "firmware", "PlayStation 2 BIOS", self.paths.pcsx2_bios_dir(), "Working PCSX2 BIOS files and sidecars."),
            location("ps3-firmware", "firmware", "PlayStation 3 firmware", data.join("rpcs3/xdg-config/rpcs3/dev_flash"), "Installed RPCS3 firmware. An original PUP is a separate input."),
            location("dreamcast-bios", "firmware", "Dreamcast boot ROM", data.join("flycast/dc_boot.bin"), "Optional Flycast boot ROM. The current adapter also supports its HLE boot path."),
            location("xemu-firmware", "firmware", "Original Xbox dependencies", data.join("xemu/xemu"), "MCPX, flash BIOS and working HDD; a mutable HDD can also hold user data. Setup is separate."),
            location("music", "media", "Music", data.join("music"), "Imported audio. Unlinking an album from a game does not remove these files."),
            location("media", "media", "Images and video", data.join("game-media"), "Local media collections and prepared videos."),
            location("artwork", "media", "Artwork cache", self.paths.cache_dir.join("art"), "Cached covers referenced by the library; preserve for offline artwork."),
            location("manager", "sources", "Manager jobs and acquisition", data.join("manager"), "Includes receipts, firmware setup, downloads and installations; overlaps other rows. Not disposable staging."),
            location("torrents", "sources", "Library torrent state", data.join("torrents"), "Transfer state is separate from the exact source payload needed to seed."),
            location("backups", "user-data", "Database backups", data.join("backups"), "Keep recovery copies; a database backup does not contain game payloads or saves."),
        ];
        if let Some(root) = &collection_root {
            locations.push(location("sources", "sources", "Retained original inputs", Path::new(root).join("sources"), "Original archives, firmware inputs and provenance. Do not infer that every input is installed."));
            locations.push(location("collection-library", "library", "Collection ROM folders", Path::new(root).join("library/imported-roms"), "Default for immediate archive intake when a platform has no existing folder. Existing roots take precedence."));
        }
        if let Some(inbox) = &bios_inbox {
            locations.push(location(
                "extra-bios-inbox",
                "firmware",
                "Additional BIOS import folder",
                PathBuf::from(inbox),
                "Additional input folder; existing firmware and saves stay in their profiles.",
            ));
        }
        for root in roots {
            locations.push(location(&format!("root:{}", root.id), "library", &format!("{} games", crate::catalog::by_id(&root.system)?.display_name), root.path.clone(), "Registered library path. A resolved path below it may be a storage compatibility link."));
            if root.system == "neogeo" {
                locations.push(location(&format!("neogeo:{}", root.id), "firmware", "Neo Geo BIOS set", root.path.join("neogeo.zip"), "This adapter requires a matching neogeo.zip beside its game ZIPs. The BIOS is excluded from Games."));
            }
        }
        for id in [
            "duckstation",
            "pcsx2",
            "rpcs3",
            "xenia",
            "mesen",
            "mgba",
            "blastem",
            "melonds",
            "rmg",
            "flycast",
            "fbneo",
            "xemu",
        ] {
            let path = data.join(id);
            if path.is_dir() {
                locations.push(location(&format!("profile:{id}"), "user-data", &format!("{id} profile"), path, "Settings, saves, states and working dependencies. Firmware rows may overlap this profile."));
            }
        }
        if self.paths.cache_dir != data.join("cache") {
            locations.push(location(
                "instance-artwork",
                "media",
                "Instance-local artwork",
                data.join("cache/art"),
                "Artwork imported by explicit --data-dir commands can be referenced here.",
            ));
        }
        if let Some(root) = connection
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v["data_dir"].as_str().map(PathBuf::from))
        {
            locations.push(location("streaming", "streaming", "Configured streaming runtime", root.join("streaming"), "Includes paired host, receiver, image receipts and streamed save profiles. A pilot directory can be a live dependency."));
        }
        let scan = self.storage.lock().unwrap_or_else(|e| e.into_inner());
        for row in &mut locations {
            row.usage = scan.usage.get(&row.path).cloned();
        }
        Ok(Snapshot {
            collection_root,
            bios_inbox,
            locations,
            scanning: scan.running,
            measured_at: scan.measured_at.clone(),
        })
    }

    pub fn storage_scan(self: &Arc<Self>) -> Result<()> {
        let locations = self.storage_snapshot()?.locations;
        {
            let mut scan = self.storage.lock().unwrap_or_else(|e| e.into_inner());
            if scan.running {
                return Err(AppError::new(
                    "STORAGE_BUSY",
                    "A storage usage scan is already running",
                ));
            }
            scan.running = true;
            scan.usage.clear();
            scan.measured_at = None;
        }
        let core = Arc::clone(self);
        std::thread::spawn(move || {
            let mut measured = BTreeMap::new();
            for row in locations {
                if !row.exists {
                    continue;
                }
                // Aliases and repeated profile rows do not need another walk.
                let canonical = row.resolved.unwrap_or_else(|| row.path.clone());
                let usage = measured
                    .entry(canonical)
                    .or_insert_with(|| measure(&row.path))
                    .clone();
                core.storage
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .usage
                    .insert(row.path, usage);
            }
            let mut scan = core.storage.lock().unwrap_or_else(|e| e.into_inner());
            scan.running = false;
            scan.measured_at = Some(crate::db::now_rfc3339());
        });
        Ok(())
    }

    pub fn storage_set_folder(&self, kind: &str, path: Option<&str>) -> Result<()> {
        let key = match kind {
            "collection" => COLLECTION,
            "bios_inbox" => "bios_inbox",
            _ => {
                return Err(AppError::new(
                    "STORAGE_KIND",
                    "This location is managed by its owning service",
                ))
            }
        };
        let mut scan = self.storage.lock().unwrap_or_else(|e| e.into_inner());
        if scan.running {
            return Err(AppError::new(
                "STORAGE_BUSY",
                "Wait for the storage usage scan before changing folders",
            ));
        }
        let value = if let Some(path) = path.filter(|s| !s.is_empty()) {
            let input = Path::new(path);
            if !input.is_absolute() || path.len() > 8192 {
                return Err(AppError::new(
                    "STORAGE_PATH",
                    "Choose an existing absolute folder on the Marquee host",
                ));
            }
            let canonical = fs::canonicalize(input)?;
            if !canonical.is_dir() {
                return Err(AppError::new("STORAGE_PATH", "Choose a folder, not a file"));
            }
            if kind == "collection"
                && [
                    &self.paths.data_dir,
                    &self.paths.config_dir,
                    &self.paths.cache_dir,
                ]
                .iter()
                .any(|p| canonical.starts_with(p) || p.starts_with(&canonical))
            {
                return Err(AppError::new("STORAGE_PATH", "Keep the collection separate from Marquee's data, configuration and cache folders"));
            }
            canonical
                .to_str()
                .ok_or_else(|| AppError::new("STORAGE_PATH", "Folder name must be valid UTF-8"))?
                .to_string()
        } else {
            String::new()
        };
        let identity = if kind == "collection" && !value.is_empty() {
            serde_json::to_string(&directory_identity(Path::new(&value))?)?
        } else {
            String::new()
        };
        self.with_db(|db| {
            let transaction = db.conn.unchecked_transaction()?;
            db.set_setting(key, &value)?;
            if kind == "collection" {
                db.set_setting(COLLECTION_IDENTITY, &identity)?;
            }
            transaction.commit()?;
            Ok(())
        })?;
        scan.usage.clear();
        scan.measured_at = None;
        Ok(())
    }

    pub fn storage_open(&self, id: &str) -> Result<()> {
        let row = self
            .storage_snapshot()?
            .locations
            .into_iter()
            .find(|r| r.id == id)
            .ok_or_else(|| {
                AppError::new("STORAGE_LOCATION", "Storage location is no longer listed")
            })?;
        let path = fs::canonicalize(row.path)?;
        let folder = if path.is_file() {
            path.parent().unwrap_or(&path)
        } else {
            &path
        };
        std::process::Command::new("xdg-open").arg(folder).spawn()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn usage_resolves_compatibility_root_but_never_walks_nested_links() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("game"), b"data").unwrap();
        std::os::unix::fs::symlink(temp.path(), source.join("loop")).unwrap();
        let alias = temp.path().join("alias");
        std::os::unix::fs::symlink(&source, &alias).unwrap();
        let usage = measure(&alias);
        assert_eq!((usage.bytes, usage.files, usage.skipped_links), (4, 1, 1));
        assert!(!usage.incomplete);
    }
    #[test]
    fn folder_configuration_preserves_roots_and_rejects_managed_overlaps() {
        let temp = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(temp.path().join("data")), false).unwrap();
        let folder = temp.path().join("collection");
        fs::create_dir(&folder).unwrap();
        core.storage_set_folder("collection", folder.to_str())
            .unwrap();
        assert_eq!(core.collection_root().unwrap(), Some(folder));
        assert!(core
            .storage_set_folder("collection", temp.path().to_str())
            .is_err());
        assert!(core
            .storage_set_folder("programs", temp.path().to_str())
            .is_err());
        assert!(core
            .storage_snapshot()
            .unwrap()
            .locations
            .iter()
            .any(|r| r.id == "ps3-firmware"));
        assert!(core.snapshot().unwrap().roots.is_empty());
        assert!(core.storage_open("../../private").is_err());
        core.storage_set_folder("collection", None).unwrap();
        assert_eq!(core.collection_root().unwrap(), None);
    }

    #[test]
    fn intake_uses_collection_for_new_platforms_and_keeps_existing_paths() {
        let temp = tempfile::tempdir().unwrap();
        let data = temp.path().join("data");
        let core = Core::open_isolated(Some(data.clone()), false).unwrap();
        let original = temp.path().join("existing");
        fs::create_dir(&original).unwrap();
        let existing = core.add_root("nes", original.clone(), None).unwrap();
        let collection = temp.path().join("collection");
        fs::create_dir(&collection).unwrap();
        core.storage_set_folder("collection", collection.to_str())
            .unwrap();
        assert_eq!(core.intake_root("nes").unwrap(), original);
        assert_eq!(
            core.intake_root("snes").unwrap(),
            collection.join("library/imported-roms/snes")
        );
        assert!(core
            .snapshot()
            .unwrap()
            .roots
            .iter()
            .any(|r| r.id == existing.id && r.path == original));
        drop(core);
        let core = Core::open_isolated(Some(data), false).unwrap();
        assert_eq!(core.collection_root().unwrap(), Some(collection));
    }

    #[test]
    fn unavailable_collection_does_not_fall_back_to_another_disk() {
        let temp = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(temp.path().join("data")), false).unwrap();
        let collection = temp.path().join("collection");
        fs::create_dir(&collection).unwrap();
        core.storage_set_folder("collection", collection.to_str())
            .unwrap();
        fs::remove_dir(&collection).unwrap();
        assert_eq!(core.intake_root("nes").unwrap_err().code, "STORAGE_OFFLINE");
        assert!(!collection.exists());
        assert!(core.snapshot().unwrap().roots.is_empty());
    }

    #[test]
    fn replacement_at_the_same_collection_path_requires_reconfirmation() {
        let temp = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(temp.path().join("data")), false).unwrap();
        let collection = temp.path().join("collection");
        fs::create_dir(&collection).unwrap();
        core.storage_set_folder("collection", collection.to_str())
            .unwrap();
        fs::rename(&collection, temp.path().join("original")).unwrap();
        fs::create_dir(&collection).unwrap();
        assert_eq!(core.intake_root("nes").unwrap_err().code, "STORAGE_OFFLINE");
        assert!(!collection.join("library").exists());
        core.storage_set_folder("collection", collection.to_str())
            .unwrap();
        assert!(core.intake_root("nes").is_ok());
    }
}
