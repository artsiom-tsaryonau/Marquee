//! Native ports and decomps (Ship of Harkinian, Zelda64Recomp, ...).
//!
//! Each port is one JSON file in `catalog/ports/`. Install downloads the latest
//! GitHub release asset (no key needed), stages and validates it before replacing
//! the installation, preserves user files, and records the release tag.
//! Port engine binaries only. Shipwright game data is copied by a setup job;
//! other ports still use the managed folder until their NP4 jobs land.

use crate::{AppError, Result};
use marquee_media::http;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const CATALOGUE: &[&str] = &[
    include_str!("../../../catalog/ports/shipwright.json"),
    include_str!("../../../catalog/ports/2ship2harkinian.json"),
    include_str!("../../../catalog/ports/zelda64recomp.json"),
    include_str!("../../../catalog/ports/sm64coopdx.json"),
    include_str!("../../../catalog/ports/fallout2-ce.json"),
    include_str!("../../../catalog/ports/uzdoom.json"),
    include_str!("../../../catalog/ports/driveclub-recomp.json"),
];

const MAX_EXTRACTED_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortDef {
    pub id: String,
    pub name: String,
    pub game: String,
    pub family: String,
    pub platform: String,
    pub github: String,
    #[serde(default)]
    pub repository_aliases: Vec<String>,
    pub asset_suffix: String,
    #[serde(default)]
    pub asset_excludes: Vec<String>,
    /// File to run inside an extracted archive; `None` means the asset itself is the executable (AppImage).
    pub binary: Option<String>,
    pub requirement: String,
    #[serde(default)]
    pub pinned_release: Option<PinnedRelease>,
    /// Known game files. Absent only for tests that isolate engine-tag logic.
    #[serde(default)]
    pub data: Option<PortData>,
}

/// Files a port needs before it can be called ready. Hashes are the ones the
/// port project publishes; an empty hash list means the file only has to exist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortData {
    /// `rom` or `data`.
    pub kind: String,
    pub source: String,
    #[serde(default)]
    pub files: Vec<PortDataFile>,
    /// Any one of these sets is enough. When this is non-empty, `files` is ignored.
    #[serde(default)]
    pub any_of: Vec<Vec<PortDataFile>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PortDataFile {
    /// Path relative to the port folder. A trailing slash is a directory.
    /// Empty means any regular file directly in the folder.
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub sha256: Vec<String>,
    #[serde(default)]
    pub sha1: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinnedRelease {
    pub tag: String,
    pub asset: String,
    pub sha256: String,
    pub binary_sha256: String,
}

/// What Marquee remembers about an installed port.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPort {
    pub tag: String,
    /// Executable path relative to the port folder.
    pub bin: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PortInfo {
    pub id: String,
    pub name: String,
    pub game: String,
    pub family: String,
    pub platform: String,
    pub upstream: String,
    pub requirement: String,
    pub installed_version: Option<String>,
    pub latest_version: Option<String>,
    /// `not_installed`, `ready`, `needs-rom`, `needs-data`, `review` or `update`.
    pub status: String,
    pub folder: Option<PathBuf>,
    pub job: Option<crate::native_ports::Job>,
    #[serde(default)]
    pub playtime_seconds: i64,
    #[serde(default)]
    pub play_count: i64,
    #[serde(default)]
    pub last_played_at: Option<String>,
}

pub fn catalogue() -> Result<Vec<PortDef>> {
    CATALOGUE
        .iter()
        .map(|raw| Ok(serde_json::from_str::<PortDef>(raw)?))
        .collect()
}

pub fn find(id: &str) -> Result<PortDef> {
    catalogue()?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| AppError::new("PORT_UNKNOWN", format!("unknown port: {id}")))
}

pub fn setting_key(id: &str) -> String {
    format!("port.{id}")
}

pub fn latest_key(id: &str) -> String {
    format!("port.{id}.latest")
}

pub fn port_dir(data: &Path, id: &str) -> PathBuf {
    data.join("ports").join(id)
}

pub fn info(
    data: &Path,
    def: &PortDef,
    installed: Option<&InstalledPort>,
    latest: Option<&str>,
) -> PortInfo {
    let engine = match (installed, latest) {
        (None, _) => "not_installed",
        (Some(i), Some(l)) if i.tag != l && def.pinned_release.is_some() => "review",
        (Some(i), Some(l)) if i.tag != l => "update",
        (Some(_), _) => "ready",
    };
    // Missing game files outrank a newer upstream tag: there is nothing to review until data is present.
    let status = if installed.is_none() {
        engine
    } else {
        let data_state = game_data_status(&port_dir(data, &def.id), def);
        if data_state == "ready" {
            engine
        } else {
            data_state
        }
    };
    PortInfo {
        job: None,
        id: def.id.clone(),
        name: def.name.clone(),
        game: def.game.clone(),
        family: def.family.clone(),
        platform: def.platform.clone(),
        upstream: def.github.clone(),
        requirement: def.requirement.clone(),
        installed_version: installed.map(|i| i.tag.clone()),
        latest_version: latest.map(str::to_string),
        status: status.into(),
        folder: installed.map(|_| port_dir(data, &def.id)),
        playtime_seconds: 0,
        play_count: 0,
        last_played_at: None,
    }
}

fn game_data_status(dir: &Path, def: &PortDef) -> &'static str {
    let Some(spec) = &def.data else {
        return "ready";
    };
    let missing = if spec.kind == "data" {
        "needs-data"
    } else {
        "needs-rom"
    };
    let sets: Vec<&[PortDataFile]> = if spec.any_of.is_empty() {
        vec![spec.files.as_slice()]
    } else {
        spec.any_of.iter().map(Vec::as_slice).collect()
    };
    if sets.iter().all(|set| set.is_empty()) {
        return missing;
    }
    let mut cache = HashCache::load(dir);
    let ready = sets.iter().any(|set| {
        set.iter()
            .all(|file| file_matches(dir, def, file, &mut cache))
    });
    cache.store(dir);
    if ready {
        "ready"
    } else {
        missing
    }
}

fn file_matches(dir: &Path, def: &PortDef, file: &PortDataFile, cache: &mut HashCache) -> bool {
    if file.path.ends_with('/') {
        let rel = file.path.trim_end_matches('/');
        return relative_file(dir, rel).is_some_and(|path| path.is_dir() && inside(dir, &path));
    }
    if file.path.is_empty() {
        let Ok(entries) = fs::read_dir(dir) else {
            return false;
        };
        return entries.flatten().any(|entry| {
            let path = entry.path();
            scan_candidate(def, &path)
                && path.is_file()
                && inside(dir, &path)
                && hashes_match(&path, file, cache)
        });
    }
    let Some(path) = relative_file(dir, &file.path) else {
        return false;
    };
    path.is_file() && inside(dir, &path) && hashes_match(&path, file, cache)
}

fn scan_candidate(def: &PortDef, path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if name.starts_with('.') {
        return false;
    }
    if def.binary.as_deref() == Some(name) {
        return false;
    }
    def.pinned_release.as_ref().map(|pin| pin.asset.as_str()) != Some(name)
}

fn relative_file(dir: &Path, rel: &str) -> Option<PathBuf> {
    let path = Path::new(rel);
    if rel.is_empty()
        || !path.is_relative()
        || path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return None;
    }
    Some(dir.join(path))
}

fn inside(root: &Path, path: &Path) -> bool {
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let Ok(path) = path.canonicalize() else {
        return false;
    };
    path.starts_with(root)
}

fn hashes_match(path: &Path, file: &PortDataFile, cache: &mut HashCache) -> bool {
    if file.sha1.is_empty() && file.sha256.is_empty() {
        return true;
    }
    let Some((sha1, sha256)) = cache.hashes(path, !file.sha1.is_empty(), !file.sha256.is_empty())
    else {
        return false;
    };
    let sha1_ok = sha1.is_some_and(|hash| {
        file.sha1
            .iter()
            .any(|expected| expected.eq_ignore_ascii_case(&hash))
    });
    let sha256_ok = sha256.is_some_and(|hash| {
        file.sha256
            .iter()
            .any(|expected| expected.eq_ignore_ascii_case(&hash))
    });
    sha1_ok || sha256_ok
}

#[derive(Default, Serialize, Deserialize)]
struct HashCache {
    files: Vec<HashEntry>,
    #[serde(skip)]
    dirty: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct HashEntry {
    name: String,
    len: u64,
    mtime: u64,
    sha1: Option<String>,
    sha256: Option<String>,
}

impl HashCache {
    fn load(dir: &Path) -> Self {
        fs::read(dir.join(".marquee-data-cache.json"))
            .ok()
            .and_then(|raw| serde_json::from_slice(&raw).ok())
            .unwrap_or_default()
    }

    fn store(&self, dir: &Path) {
        if !self.dirty {
            return;
        }
        let Ok(raw) = serde_json::to_vec(self) else {
            return;
        };
        let path = dir.join(".marquee-data-cache.json");
        let tmp = dir.join(".marquee-data-cache.json.tmp");
        if fs::write(&tmp, raw).is_ok() {
            let _ = fs::rename(tmp, path);
        }
    }

    fn hashes(
        &mut self,
        path: &Path,
        want_sha1: bool,
        want_sha256: bool,
    ) -> Option<(Option<String>, Option<String>)> {
        let meta = fs::symlink_metadata(path).ok()?;
        if !meta.file_type().is_file() || meta.len() > 1024 * 1024 * 1024 {
            return None;
        }
        let name = path.file_name()?.to_str()?.to_string();
        let mtime = meta
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs();
        if let Some(hit) = self.files.iter().find(|entry| {
            entry.name == name
                && entry.len == meta.len()
                && entry.mtime == mtime
                && (!want_sha1 || entry.sha1.is_some())
                && (!want_sha256 || entry.sha256.is_some())
        }) {
            return Some((hit.sha1.clone(), hit.sha256.clone()));
        }
        let mut sha1 = Sha1::new();
        let mut sha256 = Sha256::new();
        let mut input = File::open(path).ok()?;
        let mut buf = [0u8; 1024 * 1024];
        loop {
            let n = input.read(&mut buf).ok()?;
            if n == 0 {
                break;
            }
            if want_sha1 {
                sha1.update(&buf[..n]);
            }
            if want_sha256 {
                sha256.update(&buf[..n]);
            }
        }
        let entry = HashEntry {
            name,
            len: meta.len(),
            mtime,
            sha1: want_sha1.then(|| format!("{:x}", sha1.finalize())),
            sha256: want_sha256.then(|| format!("{:x}", sha256.finalize())),
        };
        let result = (entry.sha1.clone(), entry.sha256.clone());
        self.files.retain(|old| old.name != entry.name);
        self.files.push(entry);
        self.dirty = true;
        Some(result)
    }
}

const ROM_MAX_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupReceipt {
    pub recipe: String,
    pub rom_sha1: Option<String>,
    pub rom_sha256: String,
    pub outputs: Vec<SetupOutput>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupOutput {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

pub fn setup_available(id: &str) -> bool {
    matches!(id, "shipwright" | "2ship2harkinian" | "sm64coopdx")
}

/// Copy a verified ROM into the port folder. Never moves the original. Shipwright
/// has no headless extractor; first launch generates assets.
pub fn place_rom(
    data: &Path,
    def: &PortDef,
    source: &Path,
    checkpoint: &mut dyn FnMut(&str) -> Result<()>,
) -> Result<SetupReceipt> {
    checkpoint("Checking ROM…")?;
    if !setup_available(&def.id) {
        return Err(AppError::new(
            "PORT_SETUP",
            "This port's game-data setup is not available yet",
        ));
    }
    let meta = fs::symlink_metadata(source).map_err(|_| {
        AppError::new(
            "PORT_ROM",
            "Choose a ROM file that still exists on this computer",
        )
    })?;
    if meta.file_type().is_symlink() || !meta.file_type().is_file() {
        return Err(AppError::new(
            "PORT_ROM",
            "Choose a regular ROM file, not a link or folder",
        ));
    }
    if meta.len() > ROM_MAX_BYTES {
        return Err(AppError::new(
            "PORT_ROM",
            "ROM is larger than the setup limit",
        ));
    }
    let (sha1, sha256, bytes) = digest_file(source, meta.len())?;
    let Some(accepted) = accepted_file(def, &sha1, &sha256) else {
        return Err(AppError::new(
            "PORT_ROM_UNKNOWN",
            format!(
                "This dump is not on the port's published list ({} accepted hashes)",
                accepted_hash_count(def)
            ),
        ));
    };
    let dest_dir = port_dir(data, &def.id);
    fs::create_dir_all(&dest_dir)?;
    let (dest, present) = match fixed_rom_name(&accepted) {
        // The port reads one exact name (sm64coopdx: baserom.us.z64).
        Some(name) => fixed_destination(&dest_dir, name, bytes, &sha256)?,
        None => rom_destination(&dest_dir, &safe_rom_name(source, def), bytes, &sha256)?,
    };
    if !present {
        let mut partial = tempfile::NamedTempFile::new_in(&dest_dir)?;
        copy_rom(source, partial.as_file_mut(), bytes, checkpoint)?;
        let (_, copied_sha256, _) = digest_file(partial.path(), bytes)?;
        if copied_sha256 != sha256 {
            return Err(AppError::new(
                "PORT_ROM",
                "Copied ROM did not match the source",
            ));
        }
        checkpoint("Publishing ROM…")?;
        partial
            .persist_noclobber(&dest)
            .map_err(|error| error.error)?;
    } else {
        checkpoint("Publishing ROM…")?;
    }
    let relative = dest.file_name().unwrap().to_string_lossy().into_owned();
    let receipt = SetupReceipt {
        recipe: def.id.clone(),
        rom_sha1: Some(sha1),
        rom_sha256: sha256.clone(),
        outputs: vec![SetupOutput {
            path: relative,
            sha256,
            bytes,
        }],
        note: if def.id == "sm64coopdx" {
            "ROM placed; the game reads it from this folder at launch".into()
        } else {
            "ROM placed, first launch will generate assets".into()
        },
    };
    let path = dest_dir.join(".marquee-setup.json");
    let mut receipt_file = tempfile::NamedTempFile::new_in(&dest_dir)?;
    receipt_file.write_all(&serde_json::to_vec_pretty(&receipt)?)?;
    receipt_file.as_file().sync_all()?;
    receipt_file.persist(path).map_err(|error| error.error)?;
    File::open(&dest_dir)?.sync_all()?;
    Ok(receipt)
}

fn digest_file(path: &Path, expected: u64) -> Result<(String, String, u64)> {
    let mut sha1 = Sha1::new();
    let mut sha256 = Sha256::new();
    let mut input = File::open(path)?;
    let mut buf = [0u8; 1024 * 1024];
    let mut total = 0u64;
    loop {
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > expected {
            return Err(AppError::new("PORT_ROM", "ROM changed while reading it"));
        }
        sha1.update(&buf[..n]);
        sha256.update(&buf[..n]);
    }
    if total != expected {
        return Err(AppError::new("PORT_ROM", "ROM changed while reading it"));
    }
    Ok((
        format!("{:x}", sha1.finalize()),
        format!("{:x}", sha256.finalize()),
        total,
    ))
}

/// The published data entry this dump matches, if any.
fn accepted_file(def: &PortDef, sha1: &str, sha256: &str) -> Option<PortDataFile> {
    let spec = def.data.as_ref()?;
    let files: Vec<&PortDataFile> = if spec.any_of.is_empty() {
        spec.files.iter().collect()
    } else {
        spec.any_of.iter().flatten().collect()
    };
    files
        .into_iter()
        .find(|file| {
            file.sha1.iter().any(|hash| hash.eq_ignore_ascii_case(sha1))
                || file
                    .sha256
                    .iter()
                    .any(|hash| hash.eq_ignore_ascii_case(sha256))
        })
        .cloned()
}

/// A plain file name the catalogue requires, or None when any name will do.
fn fixed_rom_name(file: &PortDataFile) -> Option<&str> {
    let name = file.path.as_str();
    (!name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\\', '\0']))
        .then_some(name)
}

/// The required name is used as is. A different file already there is the
/// owner's, so it is reported rather than replaced.
fn fixed_destination(dir: &Path, name: &str, bytes: u64, digest: &str) -> Result<(PathBuf, bool)> {
    let path = dir.join(name);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok((path, false)),
        Err(error) => Err(error.into()),
        Ok(meta)
            if meta.is_file() && meta.len() == bytes && digest_file(&path, bytes)?.1 == digest =>
        {
            Ok((path, true))
        }
        Ok(_) => Err(AppError::new(
            "PORT_ROM",
            format!("{name} in the port folder is a different file; move it out and try again"),
        )),
    }
}

fn accepted_hash_count(def: &PortDef) -> usize {
    let Some(spec) = &def.data else {
        return 0;
    };
    let files: Vec<&PortDataFile> = if spec.any_of.is_empty() {
        spec.files.iter().collect()
    } else {
        spec.any_of.iter().flatten().collect()
    };
    files
        .iter()
        .flat_map(|file| file.sha1.iter().chain(file.sha256.iter()))
        .count()
}

fn safe_rom_name(source: &Path, def: &PortDef) -> String {
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("game.z64");
    if name.starts_with('.')
        || name.contains(['/', '\\', '\0'])
        || def.binary.as_deref() == Some(name)
        || def.pinned_release.as_ref().map(|pin| pin.asset.as_str()) == Some(name)
    {
        return "game.z64".into();
    }
    name.into()
}

fn rom_destination(dir: &Path, name: &str, bytes: u64, digest: &str) -> Result<(PathBuf, bool)> {
    let path = Path::new(name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("game");
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| format!(".{s}"))
        .unwrap_or_default();
    for index in 1..=100 {
        let candidate = match index {
            1 => dir.join(name),
            100 => dir.join(format!("{stem}-copy{ext}")),
            _ => dir.join(format!("{stem}-{index}{ext}")),
        };
        match fs::symlink_metadata(&candidate) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok((candidate, false)),
            Err(error) => return Err(error.into()),
            Ok(meta)
                if meta.is_file()
                    && meta.len() == bytes
                    && digest_file(&candidate, bytes)?.1 == digest =>
            {
                return Ok((candidate, true))
            }
            Ok(_) => {}
        }
    }
    Err(AppError::new(
        "PORT_ROM",
        "No free ROM filename in the port folder",
    ))
}

fn copy_rom(
    source: &Path,
    output: &mut File,
    expected: u64,
    checkpoint: &mut dyn FnMut(&str) -> Result<()>,
) -> Result<()> {
    let mut input = File::open(source)?;
    let mut buf = [0u8; 1024 * 1024];
    let mut copied = 0u64;
    loop {
        checkpoint("Copying game file…")?;
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        output.write_all(&buf[..n])?;
        copied += n as u64;
        if copied > expected {
            return Err(AppError::new("PORT_ROM", "ROM grew while copying"));
        }
    }
    if copied != expected {
        return Err(AppError::new("PORT_ROM", "ROM changed while copying"));
    }
    output.sync_all()?;
    Ok(())
}

pub struct Asset {
    pub name: String,
    pub url: String,
    pub sha256: Option<String>,
}

pub struct Release {
    pub tag: String,
    pub asset: Asset,
}

pub fn pick_asset(def: &PortDef, release: &serde_json::Value) -> Result<Release> {
    let tag = release
        .get("tag_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::new("NETWORK", "GitHub release has no tag"))?
        .to_string();
    if let Some(pin) = &def.pinned_release {
        if pin.tag != tag
            || pin.sha256.len() != 64
            || !pin.sha256.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err(AppError::new(
                "PORT_RELEASE",
                "Release does not match the reviewed port recipe",
            ));
        }
        let matching: Vec<_> = release["assets"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|a| a["name"].as_str() == Some(pin.asset.as_str()))
            .collect();
        if matching.len() != 1 {
            return Err(AppError::new(
                "PORT_RELEASE",
                "Pinned release asset is missing or ambiguous",
            ));
        }
        let asset = matching[0];
        let url = asset["browser_download_url"]
            .as_str()
            .ok_or_else(|| AppError::new("PORT_RELEASE", "Pinned asset has no URL"))?;
        let expected_url = format!(
            "https://github.com/{}/releases/download/{}/{}",
            def.github, pin.tag, pin.asset
        );
        if !url.eq_ignore_ascii_case(&expected_url) {
            return Err(AppError::new(
                "PORT_RELEASE",
                "Pinned asset URL does not match the reviewed repository",
            ));
        }
        if let Some(digest) = asset["digest"].as_str() {
            if !digest.eq_ignore_ascii_case(&format!("sha256:{}", pin.sha256)) {
                return Err(AppError::new(
                    "PORT_RELEASE",
                    "Upstream digest changed; the port recipe needs review",
                ));
            }
        }
        return Ok(Release {
            tag,
            asset: Asset {
                name: pin.asset.clone(),
                url: url.into(),
                sha256: Some(pin.sha256.to_ascii_lowercase()),
            },
        });
    }
    let assets = release
        .get("assets")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::new("NETWORK", "GitHub release has no assets"))?;
    let suffix = def.asset_suffix.to_ascii_lowercase();
    let found = assets.iter().find_map(|a| {
        let name = a.get("name")?.as_str()?;
        let lower = name.to_ascii_lowercase();
        if !lower.ends_with(&suffix) || def.asset_excludes.iter().any(|x| lower.contains(x)) {
            return None;
        }
        let url = a.get("browser_download_url")?.as_str()?.to_string();
        if !http::github_release_asset(&url, &def.github) {
            return None;
        }
        let sha256 = a
            .get("digest")
            .and_then(|d| d.as_str())
            .and_then(|d| d.strip_prefix("sha256:"))
            .map(|d| d.to_ascii_lowercase())?;
        Some(Asset {
            name: name.to_string(),
            url,
            sha256: Some(sha256),
        })
    });
    let asset = found.ok_or_else(|| {
        AppError::new(
            "NETWORK",
            format!(
                "{} {tag} has no checksum-verified Linux download matching *{}",
                def.name, def.asset_suffix
            ),
        )
    })?;
    Ok(Release { tag, asset })
}

/// The tag GitHub currently calls latest, without selecting or installing an asset.
pub fn upstream_tag(def: &PortDef) -> Result<String> {
    let release = http::get_json(&format!(
        "https://api.github.com/repos/{}/releases/latest",
        def.github
    ))?;
    release
        .get("tag_name")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .ok_or_else(|| AppError::new("NETWORK", "GitHub release has no tag"))
}

pub fn latest_release(def: &PortDef) -> Result<serde_json::Value> {
    let selector = def
        .pinned_release
        .as_ref()
        .map(|pin| {
            format!(
                "tags/{}",
                url::form_urlencoded::byte_serialize(pin.tag.as_bytes()).collect::<String>()
            )
        })
        .unwrap_or_else(|| "latest".into());
    http::get_json(&format!(
        "https://api.github.com/repos/{}/releases/{selector}",
        def.github
    ))
}

/// Download and extract the latest release. Returns the tag and executable to remember.
pub fn install_latest(
    data: &Path,
    def: &PortDef,
    current_tag: Option<&str>,
    progress: &mut impl FnMut(&str) -> Result<()>,
) -> Result<InstalledPort> {
    recover_installation(data, def)?;
    progress(&format!("Looking up {}…", def.name))?;
    let release = pick_asset(def, &latest_release(def)?)?;
    let dir = port_dir(data, &def.id);
    if current_tag == Some(release.tag.as_str()) {
        if let Some(bin) =
            locate_binary(&dir, def).filter(|bin| validate_pinned_binary(def, bin).is_ok())
        {
            return Ok(InstalledPort {
                tag: release.tag,
                bin: rel(&dir, &bin),
            });
        }
    }
    let parent = dir
        .parent()
        .ok_or_else(|| AppError::new("IO", "invalid port directory"))?;
    fs::create_dir_all(parent)?;
    let stage = tempfile::Builder::new()
        .prefix(&format!(".port-stage-{}-", def.id))
        .tempdir_in(parent)?;
    let release_dir = stage.path().join("release");
    fs::create_dir(&release_dir)?;
    let part = stage.path().join("download");
    progress(&format!("Downloading {} {}…", def.name, release.tag))?;
    let mut last = 0u64;
    let expected = release.asset.sha256.clone().ok_or_else(|| {
        AppError::new(
            "PORT_RELEASE",
            "This release publishes no checksum; the recipe needs a reviewed pin",
        )
    })?;
    http::download_verified(
        &release.asset.url,
        &expected,
        &http::UrlRule::GithubRelease(&def.github),
        &part,
        http::PORT_MAX_BYTES,
        Duration::from_secs(900),
        |n| {
            if n.saturating_sub(last) >= 8 * 1024 * 1024 {
                last = n;
                progress(&format!("Downloaded {} MiB…", n / 1_048_576))?;
            }
            progress("")
        },
    )
    .map_err(|error| {
        if error.code == "CHECKSUM" {
            AppError::new(
                "NETWORK",
                "download checksum did not match the GitHub release digest",
            )
            .with_context(json!({ "expected": expected }))
        } else {
            error
        }
    })?;

    let name = release.asset.name.to_ascii_lowercase();
    let result = if name.ends_with(".zip") {
        progress("Unpacking…")?;
        extract_zip(&part, &release_dir)
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        progress("Unpacking…")?;
        extract_tar_gz(&part, &release_dir)
    } else {
        // AppImage or another single executable.
        let name = Path::new(&release.asset.name);
        if name.components().count() != 1
            || !matches!(
                name.components().next(),
                Some(std::path::Component::Normal(_))
            )
        {
            return Err(AppError::new(
                "PORT_BROKEN",
                "invalid release asset filename",
            ));
        }
        let target = release_dir.join(name);
        fs::rename(&part, &target).map_err(AppError::from)
    };
    let _ = fs::remove_file(&part);
    result?;

    // Some releases wrap a tarball in a zip (Zelda64Recomp): unpack it when the executable is absent.
    if def.binary.is_some() && locate_binary(&release_dir, def).is_none() {
        unpack_nested_tarballs(&release_dir)?;
    }

    let bin = match &def.binary {
        None => release_dir.join(&release.asset.name),
        Some(_) => locate_binary(&release_dir, def).ok_or_else(|| {
            AppError::new(
                "PORT_BROKEN",
                format!("{} was unpacked but its executable was not found", def.name),
            )
        })?,
    };
    validate_pinned_binary(def, &bin)?;
    make_executable(&bin)?;
    let installed = InstalledPort {
        tag: release.tag,
        bin: rel(&release_dir, &bin),
    };
    progress("Publishing engine…")?;
    promote_installation(&release_dir, &dir)?;
    Ok(installed)
}

fn validate_pinned_binary(def: &PortDef, binary: &Path) -> Result<()> {
    let Some(pin) = &def.pinned_release else {
        return Ok(());
    };
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Err(AppError::new(
            "PORT_PLATFORM",
            "This pinned recipe requires Linux x86-64",
        ));
    }
    let mut file = File::open(binary)?;
    if file.metadata()?.len() > http::PORT_MAX_BYTES {
        return Err(AppError::new(
            "PORT_PLATFORM",
            "Port executable exceeds its size limit",
        ));
    }
    let mut header = [0; 20];
    file.read_exact(&mut header)?;
    if &header[..4] != b"\x7fELF"
        || header[4] != 2
        || header[5] != 1
        || u16::from_le_bytes([header[18], header[19]]) != 62
    {
        return Err(AppError::new(
            "PORT_PLATFORM",
            "Pinned recipe did not contain a Linux x86-64 executable",
        ));
    }
    let mut hash = Sha256::new();
    hash.update(header);
    std::io::copy(&mut file.take(http::PORT_MAX_BYTES), &mut hash)?;
    if format!("{:x}", hash.finalize()) != pin.binary_sha256.to_ascii_lowercase() {
        return Err(AppError::new(
            "PORT_CHECKSUM",
            "Pinned port executable checksum did not match",
        ));
    }
    Ok(())
}

/// Only promote validated release files. A failed copy/extraction leaves the
/// previous installation untouched, including files symlinked into another library.
fn sync_tree(dir: &Path) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            sync_tree(&entry.path())?;
        } else if kind.is_file() {
            File::open(entry.path())?.sync_all()?;
        }
    }
    File::open(dir)?.sync_all()?;
    Ok(())
}

/// A deterministic backup closes the crash window between the two renames.
fn recover_installation(data: &Path, def: &PortDef) -> Result<()> {
    let parent = data.join("ports");
    crate::service::private_dir(&parent)?;
    let dest = port_dir(data, &def.id);
    let backup = dest.with_extension("engine-backup");
    for path in [&dest, &backup] {
        match fs::symlink_metadata(path) {
            Ok(meta) if !meta.is_dir() || meta.file_type().is_symlink() => {
                return Err(AppError::new(
                    "PORT_PATH",
                    "Engine storage must be a regular directory",
                ))
            }
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
    }
    if backup.exists() {
        if dest.exists() {
            fs::remove_dir_all(&backup)?;
        } else {
            fs::rename(&backup, &dest)?;
        }
        File::open(&parent)?.sync_all()?;
    }
    // Only this recipe's abandoned staging directories; never user game folders.
    let prefix = format!(".port-stage-{}-", def.id);
    for entry in fs::read_dir(&parent)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with(&prefix) && entry.file_type()?.is_dir() {
            fs::remove_dir_all(entry.path())?;
        }
    }
    Ok(())
}

fn promote_installation(stage: &Path, dest: &Path) -> Result<()> {
    if let Ok(meta) = fs::symlink_metadata(dest) {
        if meta.file_type().is_symlink() || !meta.is_dir() {
            return Err(AppError::new(
                "IO",
                "port destination is not a regular directory",
            ));
        }
        preserve_user_files(dest, stage)?;
    }
    sync_tree(stage)?;
    let parent = dest
        .parent()
        .ok_or_else(|| AppError::new("PORT_PATH", "Invalid engine destination"))?;
    let backup = dest.with_extension("engine-backup");
    if backup.exists() {
        return Err(AppError::new(
            "PORT_RECOVERY",
            "Recover the previous installation before publication",
        ));
    }
    let had_previous = dest.exists();
    if had_previous {
        fs::rename(dest, &backup)?;
        File::open(parent)?.sync_all()?;
    }
    if let Err(err) = fs::rename(stage, dest) {
        if had_previous {
            fs::rename(&backup, dest)?;
            File::open(parent)?.sync_all()?;
        }
        return Err(err.into());
    }
    File::open(parent)?.sync_all()?;
    if had_previous {
        fs::remove_dir_all(backup)?;
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

fn preserve_user_files(old: &Path, stage: &Path) -> Result<()> {
    for entry in fs::read_dir(old)? {
        let entry = entry?;
        let from = entry.path();
        let to = stage.join(entry.file_name());
        let kind = entry.file_type()?;
        if fs::symlink_metadata(&to).is_ok() {
            if kind.is_symlink() {
                return Err(AppError::new(
                    "IO",
                    format!("release conflicts with user symlink: {}", from.display()),
                ));
            }
            if kind.is_dir() != to.is_dir() {
                return Err(AppError::new(
                    "IO",
                    "release conflicts with a user directory",
                ));
            }
            if kind.is_dir() && to.is_dir() {
                preserve_user_files(&from, &to)?;
            }
            continue;
        }
        if kind.is_symlink() {
            std::os::unix::fs::symlink(fs::read_link(&from)?, &to)?;
        } else if kind.is_dir() {
            fs::create_dir(&to)?;
            preserve_user_files(&from, &to)?;
        } else if kind.is_file() {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

fn reject_symlink_path(dest: &Path, relative: &Path) -> Result<()> {
    let mut path = dest.to_path_buf();
    for component in std::iter::once(None).chain(relative.components().map(Some)) {
        if let Some(component) = component {
            if !matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            ) {
                return Err(AppError::new("IO", "archive path escapes destination"));
            }
            path.push(component);
        }
        if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(AppError::new(
                "IO",
                "archive path traverses an existing symlink",
            ));
        }
    }
    Ok(())
}

fn unpack_nested_tarballs(dir: &Path) -> Result<()> {
    for entry in fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if path.is_file() && (name.ends_with(".tar.gz") || name.ends_with(".tgz")) {
            extract_tar_gz(&path, dir)?;
            let _ = fs::remove_file(&path);
        }
    }
    Ok(())
}

fn rel(dir: &Path, path: &Path) -> String {
    path.strip_prefix(dir)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn locate_binary(dir: &Path, def: &PortDef) -> Option<PathBuf> {
    let wanted = def.binary.as_deref()?;
    find_file(dir, wanted, 4)
}

fn find_file(dir: &Path, name: &str, depth: u32) -> Option<PathBuf> {
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_file() && entry.file_name() == name {
            return Some(path);
        }
        if kind.is_dir() && depth > 0 {
            if let Some(found) = find_file(&path, name, depth - 1) {
                return Some(found);
            }
        }
    }
    None
}

fn make_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(perms.mode() | 0o755);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

pub fn extract_zip(archive: &Path, dest: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(File::open(archive)?)
        .map_err(|e| AppError::new("IO", format!("not a valid zip: {e}")))?;
    let mut total = 0u64;
    for i in 0..zip.len() {
        total = total
            .checked_add(
                zip.by_index(i)
                    .map_err(|e| AppError::new("IO", e.to_string()))?
                    .size(),
            )
            .ok_or_else(|| AppError::new("IO", "archive size overflow"))?;
    }
    if total > MAX_EXTRACTED_BYTES {
        return Err(AppError::new("IO", "archive is unreasonably large"));
    }
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| AppError::new("IO", e.to_string()))?;
        // Entries that would escape the destination (`../`, absolute paths) are skipped.
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        reject_symlink_path(dest, &rel)?;
        let out = dest.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(&out)?;
            continue;
        }
        if entry.is_symlink() {
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        let _ = fs::remove_file(&out);
        let mut file = File::create(&out)?;
        io::copy(&mut entry, &mut file)?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&out, fs::Permissions::from_mode(mode & 0o777))?;
        }
    }
    Ok(())
}

pub fn extract_tar_gz(archive: &Path, dest: &Path) -> Result<()> {
    extract_tar(flate2::read::GzDecoder::new(File::open(archive)?), dest)
}

/// Same rules as `.tar.gz`: regular files and folders only, contained, size-capped.
pub fn extract_tar_xz(archive: &Path, dest: &Path) -> Result<()> {
    let file = io::BufReader::new(File::open(archive)?);
    extract_tar(lzma_rust2::XzReader::new(file, true), dest)
}

fn extract_tar(stream: impl io::Read, dest: &Path) -> Result<()> {
    let mut tar = tar::Archive::new(stream);
    let mut total = 0u64;
    for entry in tar.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        reject_symlink_path(dest, &path)?;
        let kind = entry.header().entry_type();
        if !kind.is_file() && !kind.is_dir() {
            return Err(AppError::new(
                "IO",
                "archive links and special files are not supported",
            ));
        }
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| AppError::new("IO", "archive size overflow"))?;
        if total > MAX_EXTRACTED_BYTES {
            return Err(AppError::new("IO", "archive is unreasonably large"));
        }
        if !entry.unpack_in(dest)? {
            return Err(AppError::new("IO", "archive path escapes destination"));
        }
    }
    Ok(())
}

/// Start the port from its own folder (many ports read data beside the binary).
/// The caller owns the child so the session can be recorded.
pub fn launch(
    data: &Path,
    def: &PortDef,
    installed: &InstalledPort,
) -> Result<std::process::Child> {
    let dir = port_dir(data, &def.id);
    let bin = dir.join(&installed.bin);
    if !bin.is_file() {
        return Err(AppError::new(
            "PORT_BROKEN",
            format!("{} is missing; install it again", def.name),
        )
        .with_context(json!({ "path": bin })));
    }
    let cwd = bin.parent().unwrap_or(&dir).to_path_buf();
    let mut command = Command::new(&bin);
    configure_launch(&mut command, &dir, def);
    // Its own process group, so Marquee's Stop reaches everything the port starts.
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    command
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| {
            AppError::new("SPAWN_FAILED", err.to_string()).with_context(json!({ "path": bin }))
        })
}

/// Shipwright otherwise writes into the host's shared ~/.local/share/soh profile.
/// Use its supported override and AppImage runtime fallback on hosts without FUSE 2.
/// sm64coopdx otherwise uses ~/.local/share/sm64coopdx and copies the ROM there;
/// `--savepath` keeps its saves, config and ROM lookup in the port folder.
fn configure_launch(command: &mut Command, dir: &Path, def: &PortDef) {
    if def.id == "shipwright" {
        command
            .env("SHIP_HOME", dir)
            .env("APPIMAGE_EXTRACT_AND_RUN", "1");
    }
    if def.id == "sm64coopdx" {
        command.arg("--savepath").arg(dir);
    }
    // DriveClub Recomp: `--play` skips its own launcher window. Naming the data and
    // game folders keeps it off ~/.local/share/driveclub-recomp and any saved choice.
    if def.id == "driveclub-recomp" {
        command
            .arg("--play")
            .env("DRIVECLUB_DATA_DIR", dir.join("data"))
            .env("DRIVECLUB_UPDATE_DIR", dir.join("update"))
            .env("DRIVECLUB_BASE_DIR", dir.join("base"))
            .env("APPIMAGE_EXTRACT_AND_RUN", "1");
    }
}

/// Open the port folder in the file manager so the user can drop in their own files.
pub fn open_folder(data: &Path, id: &str) -> Result<()> {
    let dir = port_dir(data, id);
    fs::create_dir_all(&dir)?;
    Command::new("xdg-open")
        .arg(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| AppError::new("SPAWN_FAILED", format!("could not open folder: {err}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn interrupted_promotion_restores_user_files_and_discards_only_recipe_staging() {
        let data = tempfile::tempdir().unwrap();
        let def = find("2ship2harkinian").unwrap();
        let dest = port_dir(data.path(), &def.id);
        let backup = dest.with_extension("engine-backup");
        fs::create_dir_all(&backup).unwrap();
        fs::write(backup.join("save.dat"), b"precious save").unwrap();
        let stage = data
            .path()
            .join("ports/.port-stage-2ship2harkinian-abandoned");
        fs::create_dir(&stage).unwrap();
        fs::write(stage.join("partial"), b"broken").unwrap();
        let other = data.path().join("ports/.port-stage-shipwright-unrelated");
        fs::create_dir(&other).unwrap();
        recover_installation(data.path(), &def).unwrap();
        assert_eq!(fs::read(dest.join("save.dat")).unwrap(), b"precious save");
        assert!(!backup.exists());
        assert!(!stage.exists());
        assert!(other.exists());
        // Crash after the new tree was renamed, before deleting the old backup.
        fs::create_dir(&backup).unwrap();
        fs::write(backup.join("save.dat"), b"old").unwrap();
        recover_installation(data.path(), &def).unwrap();
        assert_eq!(fs::read(dest.join("save.dat")).unwrap(), b"precious save");
        assert!(!backup.exists());
    }

    #[test]
    fn shipwright_launch_keeps_saves_and_generated_assets_inside_its_library() {
        let mut command = Command::new("engine");
        let dir = Path::new("/isolated/library/ports/shipwright");
        configure_launch(&mut command, dir, &find("shipwright").unwrap());
        let env: std::collections::BTreeMap<_, _> = command
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.unwrap().to_string_lossy().into_owned(),
                )
            })
            .collect();
        assert_eq!(env["SHIP_HOME"], dir.to_str().unwrap());
        assert_eq!(env["APPIMAGE_EXTRACT_AND_RUN"], "1");
        assert!(!env.contains_key("HOME"));
    }

    #[test]
    fn sm64coopdx_launch_keeps_saves_and_its_rom_lookup_inside_its_library() {
        let mut command = Command::new("engine");
        let dir = Path::new("/isolated/library/ports/sm64coopdx");
        configure_launch(&mut command, dir, &find("sm64coopdx").unwrap());
        let args: Vec<_> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, ["--savepath", dir.to_str().unwrap()]);
    }

    #[test]
    fn driveclub_plays_directly_with_its_data_and_game_folders_in_its_library() {
        let mut command = Command::new("engine");
        let dir = Path::new("/isolated/library/ports/driveclub-recomp");
        configure_launch(&mut command, dir, &find("driveclub-recomp").unwrap());
        let args: Vec<_> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, ["--play"]);
        let env: std::collections::HashMap<_, _> = command
            .get_envs()
            .filter_map(|(k, v)| {
                Some((
                    k.to_string_lossy().into_owned(),
                    v?.to_string_lossy().into_owned(),
                ))
            })
            .collect();
        assert_eq!(
            env["DRIVECLUB_DATA_DIR"],
            dir.join("data").to_str().unwrap()
        );
        assert_eq!(
            env["DRIVECLUB_UPDATE_DIR"],
            dir.join("update").to_str().unwrap()
        );
        assert_eq!(
            env["DRIVECLUB_BASE_DIR"],
            dir.join("base").to_str().unwrap()
        );
    }

    #[test]
    fn driveclub_is_ready_only_with_both_extracted_folders() {
        let t = tempfile::tempdir().unwrap();
        let def = find("driveclub-recomp").unwrap();
        let dir = t.path();
        assert_eq!(game_data_status(dir, &def), "needs-data");
        fs::create_dir_all(dir.join("update/sce_sys")).unwrap();
        fs::write(dir.join("update/eboot.bin"), b"x").unwrap();
        fs::write(dir.join("update/sce_sys/param.sfo"), b"x").unwrap();
        assert_eq!(game_data_status(dir, &def), "needs-data");
        fs::create_dir_all(dir.join("base")).unwrap();
        fs::write(dir.join("base/game000.dat"), b"x").unwrap();
        assert_eq!(game_data_status(dir, &def), "ready");
    }

    fn release() -> serde_json::Value {
        json!({
            "tag_name": "v1.2.2",
            "assets": [
                {"name": "Zelda64Recompiled-v1.2.2-Linux-Flatpak-X64.zip", "browser_download_url": "https://github.com/Zelda64Recomp/Zelda64Recomp/releases/download/v1.2.2/flatpak.zip", "digest": "sha256:11"},
                {"name": "Zelda64Recompiled-v1.2.2-Linux-ARM64.zip", "browser_download_url": "https://github.com/Zelda64Recomp/Zelda64Recomp/releases/download/v1.2.2/arm.zip", "digest": "sha256:22"},
                {"name": "Zelda64Recompiled-v1.2.2-Linux-X64.zip", "browser_download_url": "https://github.com/Zelda64Recomp/Zelda64Recomp/releases/download/v1.2.2/Zelda64Recompiled-v1.2.2-Linux-X64.zip", "digest": "sha256:ABCD"}
            ]
        })
    }
    fn unpinned(id: &str) -> PortDef {
        let mut def = find(id).unwrap();
        def.pinned_release = None;
        def
    }

    #[test]
    fn catalogue_parses_and_ids_are_unique() {
        let all = catalogue().unwrap();
        assert_eq!(all.len(), 7);
        let mut ids: Vec<_> = all.iter().map(|p| p.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 7);
    }

    #[test]
    fn picks_the_plain_x64_linux_asset() {
        let def = unpinned("zelda64recomp");
        let picked = pick_asset(&def, &release()).unwrap();
        assert_eq!(picked.tag, "v1.2.2");
        assert!(picked.asset.url.ends_with("Linux-X64.zip"));
        assert_eq!(picked.asset.sha256.as_deref(), Some("abcd"));
    }

    #[test]
    fn every_port_declares_where_its_game_data_comes_from() {
        for def in catalogue().unwrap() {
            let data = def.data.expect(&def.id);
            assert!(data.kind == "rom" || data.kind == "data", "{}", def.id);
            assert!(data.source.starts_with("https://"), "{}", def.id);
            assert!(
                !data.files.is_empty() || data.any_of.iter().any(|set| !set.is_empty()),
                "{}",
                def.id
            );
        }
    }

    #[test]
    fn installed_port_without_matching_game_data_is_not_ready() {
        let dir = tempfile::tempdir().unwrap();
        let mut def = find("sm64coopdx").unwrap();
        def.data.as_mut().unwrap().files[0].sha1 =
            vec![format!("{:x}", Sha1::digest(b"owned-rom"))];
        def.data.as_mut().unwrap().files[0].sha256.clear();
        let installed = InstalledPort {
            tag: def.pinned_release.as_ref().unwrap().tag.clone(),
            bin: "sm64coopdx".into(),
        };
        let folder = port_dir(dir.path(), &def.id);
        fs::create_dir_all(&folder).unwrap();
        assert_eq!(
            info(dir.path(), &def, Some(&installed), Some(&installed.tag)).status,
            "needs-rom"
        );
        fs::write(folder.join("baserom.us.z64"), b"wrong").unwrap();
        assert_eq!(
            info(dir.path(), &def, Some(&installed), Some("9.9.9")).status,
            "needs-rom"
        );
        fs::write(folder.join("baserom.us.z64"), b"owned-rom").unwrap();
        assert_eq!(
            info(dir.path(), &def, Some(&installed), Some(&installed.tag)).status,
            "ready"
        );
        assert_eq!(
            info(dir.path(), &def, Some(&installed), Some("9.9.9")).status,
            "review"
        );
    }

    #[test]
    fn presence_only_data_and_scanned_roms_match_without_leaving_the_port_folder() {
        let dir = tempfile::tempdir().unwrap();
        let mut def = find("fallout2-ce").unwrap();
        def.data = Some(PortData {
            kind: "data".into(),
            source: "https://example.invalid/data".into(),
            files: vec![
                PortDataFile {
                    path: "master.dat".into(),
                    ..PortDataFile::default()
                },
                PortDataFile {
                    path: "data/".into(),
                    ..PortDataFile::default()
                },
            ],
            any_of: vec![],
        });
        let installed = InstalledPort {
            tag: "v1".into(),
            bin: "fallout2-ce".into(),
        };
        let folder = port_dir(dir.path(), &def.id);
        fs::create_dir_all(folder.join("data")).unwrap();
        assert_eq!(
            info(dir.path(), &def, Some(&installed), None).status,
            "needs-data"
        );
        fs::write(folder.join("master.dat"), b"game").unwrap();
        assert_eq!(
            info(dir.path(), &def, Some(&installed), None).status,
            "ready"
        );

        let mut scanned = find("shipwright").unwrap();
        let digest = format!("{:x}", Sha1::digest(b"ocarina"));
        scanned.data = Some(PortData {
            kind: "rom".into(),
            source: "https://example.invalid/rom".into(),
            files: vec![],
            any_of: vec![vec![PortDataFile {
                path: String::new(),
                sha1: vec![digest],
                sha256: vec![],
            }]],
        });
        let ship = port_dir(dir.path(), &scanned.id);
        fs::create_dir_all(&ship).unwrap();
        let outside = dir.path().join("outside.z64");
        fs::write(&outside, b"ocarina").unwrap();
        std::os::unix::fs::symlink(&outside, ship.join("linked.z64")).unwrap();
        let installed = InstalledPort {
            tag: "9.3.1".into(),
            bin: "SoH.appimage".into(),
        };
        assert_eq!(
            info(dir.path(), &scanned, Some(&installed), None).status,
            "needs-rom",
            "a symlink to a file outside the port folder is not game data"
        );
        fs::write(ship.join("oot.z64"), b"ocarina").unwrap();
        assert_eq!(
            info(dir.path(), &scanned, Some(&installed), None).status,
            "ready"
        );
    }

    #[test]
    fn every_port_recipe_pins_a_reviewed_release() {
        for def in catalogue().unwrap() {
            let pin = def.pinned_release.expect(&def.id);
            assert_eq!(pin.sha256.len(), 64, "{}", def.id);
            assert!(pin.sha256.bytes().all(|c| c.is_ascii_hexdigit()));
            assert_eq!(pin.binary_sha256.len(), 64, "{}", def.id);
            assert!(http::github_release_asset(
                &format!(
                    "https://github.com/{}/releases/download/{}/{}",
                    def.github, pin.tag, pin.asset
                ),
                &def.github
            ));
        }
    }

    #[test]
    fn an_unpinned_release_without_a_digest_is_refused() {
        let def = unpinned("zelda64recomp");
        let mut release = release();
        release["assets"]
            .as_array_mut()
            .unwrap()
            .retain(|asset| asset["name"].as_str().unwrap().contains("Linux-X64.zip"));
        release["assets"][0]
            .as_object_mut()
            .unwrap()
            .remove("digest");
        match pick_asset(&def, &release) {
            Err(error) => assert_eq!(error.code, "NETWORK"),
            Ok(_) => panic!("an unpinned release without a digest was accepted"),
        }
    }

    #[test]
    fn pinned_port_rejects_release_asset_url_and_digest_changes() {
        let def = find("2ship2harkinian").unwrap();
        let pin = def.pinned_release.as_ref().unwrap();
        let mut release = json!({"tag_name":pin.tag,"assets":[{"name":pin.asset,"browser_download_url":format!("https://github.com/{}/releases/download/{}/{}",def.github,pin.tag,pin.asset),"digest":format!("sha256:{}",pin.sha256)}]});
        assert_eq!(
            pick_asset(&def, &release).unwrap().asset.sha256.as_deref(),
            Some(pin.sha256.as_str())
        );
        release["assets"][0]
            .as_object_mut()
            .unwrap()
            .remove("digest");
        assert_eq!(
            pick_asset(&def, &release).unwrap().asset.sha256.as_deref(),
            Some(pin.sha256.as_str())
        );
        for (key, value) in [
            ("digest", "sha256:changed"),
            ("name", "2Ship-Windows.zip"),
            ("browser_download_url", "https://other.invalid/asset"),
        ] {
            let mut changed = release.clone();
            changed["assets"][0][key] = serde_json::Value::String(value.into());
            assert!(pick_asset(&def, &changed).is_err());
        }
        release["tag_name"] = serde_json::Value::String("different".into());
        assert!(pick_asset(&def, &release).is_err());
    }

    #[test]
    fn pinned_binary_validation_checks_machine_and_content() {
        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("appimage");
        let mut def = find("2ship2harkinian").unwrap();
        let mut bytes = vec![0; 64];
        bytes[..4].copy_from_slice(b"\x7fELF");
        bytes[4] = 2;
        bytes[5] = 1;
        bytes[18] = 62;
        def.pinned_release.as_mut().unwrap().binary_sha256 =
            format!("{:x}", Sha256::digest(&bytes));
        fs::write(&binary, &bytes).unwrap();
        assert!(validate_pinned_binary(&def, &binary).is_ok());
        bytes[63] = 1;
        fs::write(&binary, &bytes).unwrap();
        assert_eq!(
            validate_pinned_binary(&def, &binary).unwrap_err().code,
            "PORT_CHECKSUM"
        );
        bytes[18] = 183;
        fs::write(&binary, &bytes).unwrap();
        assert_eq!(
            validate_pinned_binary(&def, &binary).unwrap_err().code,
            "PORT_PLATFORM"
        );
    }

    #[test]
    fn missing_asset_is_a_typed_error() {
        let def = unpinned("uzdoom");
        let err = pick_asset(&def, &release()).err().unwrap();
        assert_eq!(err.code, "NETWORK");
    }

    #[test]
    fn status_reflects_installed_and_latest() {
        let dir = tempfile::tempdir().unwrap();
        let paths = dir.path();
        let mut def = find("shipwright").unwrap();
        def.data = None;
        let inst = InstalledPort {
            tag: "1".into(),
            bin: "x".into(),
        };
        assert_eq!(info(paths, &def, None, Some("2")).status, "not_installed");
        assert_eq!(info(paths, &def, Some(&inst), None).status, "ready");
        assert_eq!(info(paths, &def, Some(&inst), Some("1")).status, "ready");
        assert_eq!(info(paths, &def, Some(&inst), Some("2")).status, "review");
        let mut rolling = def.clone();
        rolling.pinned_release = None;
        assert_eq!(
            info(paths, &rolling, Some(&inst), Some("2")).status,
            "update"
        );
    }

    #[test]
    fn zip_extracts_with_modes_and_blocks_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("a.zip");
        {
            let mut w = zip::ZipWriter::new(File::create(&archive).unwrap());
            let opts = zip::write::SimpleFileOptions::default().unix_permissions(0o755);
            w.start_file("bin/game", opts).unwrap();
            w.write_all(b"#!/bin/sh\n").unwrap();
            w.start_file("../escape.txt", opts).unwrap();
            w.write_all(b"nope").unwrap();
            w.finish().unwrap();
        }
        let dest = dir.path().join("out");
        fs::create_dir_all(&dest).unwrap();
        extract_zip(&archive, &dest).unwrap();
        assert!(dest.join("bin/game").is_file());
        assert!(!dir.path().join("escape.txt").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(dest.join("bin/game"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111);
        }
    }

    #[test]
    fn user_files_survive_reextraction() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("a.zip");
        {
            let mut w = zip::ZipWriter::new(File::create(&archive).unwrap());
            w.start_file("game", zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(b"v2").unwrap();
            w.finish().unwrap();
        }
        let dest = dir.path().join("out");
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("baserom.us.z64"), b"mine").unwrap();
        fs::write(dest.join("game"), b"v1").unwrap();
        extract_zip(&archive, &dest).unwrap();
        assert_eq!(fs::read(dest.join("game")).unwrap(), b"v2");
        assert_eq!(fs::read(dest.join("baserom.us.z64")).unwrap(), b"mine");
    }
}

#[cfg(test)]
mod correction_tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn archive_cannot_follow_an_existing_directory_symlink() {
        let t = tempfile::tempdir().unwrap();
        let outside = t.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("save"), b"mine").unwrap();
        let dest = t.path().join("dest");
        fs::create_dir(&dest).unwrap();
        std::os::unix::fs::symlink(&outside, dest.join("data")).unwrap();
        let archive = t.path().join("a.zip");
        let mut zip = zip::ZipWriter::new(File::create(&archive).unwrap());
        zip.start_file("data/save", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"replacement").unwrap();
        zip.finish().unwrap();
        assert!(extract_zip(&archive, &dest).is_err());
        assert_eq!(fs::read(outside.join("save")).unwrap(), b"mine");
    }
    #[test]
    fn staged_release_preserves_data_and_refuses_symlink_conflicts() {
        let t = tempfile::tempdir().unwrap();
        let dest = t.path().join("installed");
        let stage = t.path().join("stage");
        fs::create_dir(&dest).unwrap();
        fs::create_dir(&stage).unwrap();
        fs::write(dest.join("game"), b"v1").unwrap();
        fs::write(dest.join("save"), b"mine").unwrap();
        fs::write(stage.join("game"), b"v2").unwrap();
        promote_installation(&stage, &dest).unwrap();
        assert_eq!(fs::read(dest.join("game")).unwrap(), b"v2");
        assert_eq!(fs::read(dest.join("save")).unwrap(), b"mine");
        fs::create_dir(&stage).unwrap();
        fs::create_dir(stage.join("data")).unwrap();
        std::os::unix::fs::symlink(t.path(), dest.join("data")).unwrap();
        assert!(promote_installation(&stage, &dest).is_err());
        assert_eq!(fs::read(dest.join("game")).unwrap(), b"v2");
    }
    #[test]
    fn failed_extraction_never_changes_the_installed_binary() {
        let t = tempfile::tempdir().unwrap();
        let dest = t.path().join("installed");
        let stage = t.path().join("stage");
        fs::create_dir(&dest).unwrap();
        fs::create_dir(&stage).unwrap();
        fs::write(dest.join("game"), b"v1").unwrap();
        let archive = t.path().join("a.zip");
        let mut zip = zip::ZipWriter::new(File::create(&archive).unwrap());
        zip.start_file(
            "game",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
        zip.write_all(b"unique-v2-content").unwrap();
        zip.finish().unwrap();
        let mut bytes = fs::read(&archive).unwrap();
        let i = bytes
            .windows(17)
            .position(|b| b == b"unique-v2-content")
            .unwrap();
        bytes[i] ^= 1;
        fs::write(&archive, bytes).unwrap();
        assert!(extract_zip(&archive, &stage).is_err());
        assert_eq!(fs::read(dest.join("game")).unwrap(), b"v1");
    }
    #[test]
    fn nested_tar_update_uses_new_binary_in_fresh_stage() {
        let t = tempfile::tempdir().unwrap();
        let old = t.path().join("old");
        let stage = t.path().join("stage");
        fs::create_dir(&old).unwrap();
        fs::create_dir(&stage).unwrap();
        let def = find("zelda64recomp").unwrap();
        let name = def.binary.as_deref().unwrap();
        fs::write(old.join(name), b"v1").unwrap();
        let gz = flate2::write::GzEncoder::new(
            File::create(stage.join("release.tar.gz")).unwrap(),
            flate2::Compression::default(),
        );
        let mut tar = tar::Builder::new(gz);
        let mut header = tar::Header::new_gnu();
        header.set_size(2);
        header.set_mode(0o755);
        header.set_cksum();
        tar.append_data(&mut header, name, &b"v2"[..]).unwrap();
        tar.into_inner().unwrap().finish().unwrap();
        assert!(locate_binary(&stage, &def).is_none());
        unpack_nested_tarballs(&stage).unwrap();
        promote_installation(&stage, &old).unwrap();
        assert_eq!(fs::read(old.join(name)).unwrap(), b"v2");
    }

    fn hashed_rom(bytes: &[u8]) -> (PortDef, String) {
        let mut def = find("shipwright").unwrap();
        let sha1 = format!("{:x}", Sha1::digest(bytes));
        def.data = Some(PortData {
            kind: "rom".into(),
            source: "test".into(),
            files: vec![],
            any_of: vec![vec![PortDataFile {
                path: String::new(),
                sha1: vec![sha1.clone()],
                sha256: vec![],
            }]],
        });
        (def, sha1)
    }

    #[test]
    fn setup_copies_a_matching_rom_and_leaves_the_original() {
        let data = tempfile::tempdir().unwrap();
        let src_dir = tempfile::tempdir().unwrap();
        let bytes = b"synthetic oot dump";
        let source = src_dir.path().join("ZELOOTMA.z64");
        fs::write(&source, bytes).unwrap();
        let (def, sha1) = hashed_rom(bytes);
        let cancel = place_rom(data.path(), &def, &source, &mut |message| {
            if message == "Publishing ROM…" {
                return Err(AppError::new("PORT_CANCELLED", "cancelled"));
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(cancel.code, "PORT_CANCELLED");
        assert_eq!(
            fs::read_dir(port_dir(data.path(), "shipwright"))
                .unwrap()
                .count(),
            0
        );
        let receipt = place_rom(data.path(), &def, &source, &mut |_| Ok(())).unwrap();
        assert_eq!(receipt.recipe, "shipwright");
        assert_eq!(receipt.rom_sha1.as_deref(), Some(sha1.as_str()));
        assert_eq!(receipt.outputs[0].path, "ZELOOTMA.z64");
        assert_eq!(
            receipt.note,
            "ROM placed, first launch will generate assets"
        );
        let retried = place_rom(data.path(), &def, &source, &mut |_| Ok(())).unwrap();
        assert_eq!(retried.outputs[0].path, receipt.outputs[0].path);
        let dest = port_dir(data.path(), "shipwright").join("ZELOOTMA.z64");
        assert_eq!(fs::read(&dest).unwrap(), bytes);
        assert_eq!(fs::read(&source).unwrap(), bytes);
        assert!(port_dir(data.path(), "shipwright")
            .join(".marquee-setup.json")
            .is_file());
        assert_eq!(
            game_data_status(&port_dir(data.path(), "shipwright"), &def),
            "ready"
        );
    }

    #[test]
    fn setup_copies_a_majora_rom_for_2ship() {
        let data = tempfile::tempdir().unwrap();
        let src_dir = tempfile::tempdir().unwrap();
        let bytes = b"synthetic mm dump";
        let source = src_dir.path().join("ZELOOTMM.z64");
        fs::write(&source, bytes).unwrap();
        let mut def = find("2ship2harkinian").unwrap();
        def.data = Some(PortData {
            kind: "rom".into(),
            source: "test".into(),
            files: vec![],
            any_of: vec![vec![PortDataFile {
                path: String::new(),
                sha1: vec![format!("{:x}", Sha1::digest(bytes))],
                sha256: vec![],
            }]],
        });
        let receipt = place_rom(data.path(), &def, &source, &mut |_| Ok(())).unwrap();
        assert_eq!(receipt.recipe, "2ship2harkinian");
        assert_eq!(fs::read(&source).unwrap(), bytes);
        assert_eq!(
            fs::read(port_dir(data.path(), "2ship2harkinian").join("ZELOOTMM.z64")).unwrap(),
            bytes
        );
    }

    #[test]
    fn setup_names_the_rom_baserom_for_sm64coopdx_and_never_replaces_another_file() {
        let data = tempfile::tempdir().unwrap();
        let src_dir = tempfile::tempdir().unwrap();
        let bytes = b"synthetic sm64 dump";
        let source = src_dir.path().join("Super Mario 64 (USA).z64");
        fs::write(&source, bytes).unwrap();
        let mut def = find("sm64coopdx").unwrap();
        def.data = Some(PortData {
            kind: "rom".into(),
            source: "test".into(),
            files: vec![PortDataFile {
                path: "baserom.us.z64".into(),
                sha1: vec![format!("{:x}", Sha1::digest(bytes))],
                sha256: vec![],
            }],
            any_of: vec![],
        });
        let dir = port_dir(data.path(), "sm64coopdx");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("baserom.us.z64"), b"someone else's file").unwrap();
        let refused = place_rom(data.path(), &def, &source, &mut |_| Ok(())).unwrap_err();
        assert_eq!(refused.code, "PORT_ROM");
        assert_eq!(
            fs::read(dir.join("baserom.us.z64")).unwrap(),
            b"someone else's file"
        );
        fs::remove_file(dir.join("baserom.us.z64")).unwrap();
        let receipt = place_rom(data.path(), &def, &source, &mut |_| Ok(())).unwrap();
        assert_eq!(receipt.outputs[0].path, "baserom.us.z64");
        assert_eq!(fs::read(dir.join("baserom.us.z64")).unwrap(), bytes);
        assert_eq!(fs::read(&source).unwrap(), bytes);
        let retried = place_rom(data.path(), &def, &source, &mut |_| Ok(())).unwrap();
        assert_eq!(retried.outputs[0].path, "baserom.us.z64");
        assert_eq!(game_data_status(&dir, &def), "ready");
    }

    #[test]
    fn setup_refuses_an_unknown_dump() {
        let data = tempfile::tempdir().unwrap();
        let src = data.path().join("unknown.z64");
        fs::write(&src, b"not a listed dump").unwrap();
        let def = find("shipwright").unwrap();
        assert_eq!(
            place_rom(data.path(), &def, &src, &mut |_| Ok(()))
                .unwrap_err()
                .code,
            "PORT_ROM_UNKNOWN"
        );
        assert!(!port_dir(data.path(), "shipwright")
            .join("unknown.z64")
            .exists());
    }

    #[test]
    fn setup_cancel_during_copy_leaves_no_partial() {
        let data = tempfile::tempdir().unwrap();
        let src_dir = tempfile::tempdir().unwrap();
        let bytes = vec![7u8; 2 * 1024 * 1024 + 10];
        let source = src_dir.path().join("large.z64");
        fs::write(&source, &bytes).unwrap();
        let (def, _) = hashed_rom(&bytes);
        let mut steps = 0;
        let err = place_rom(data.path(), &def, &source, &mut |_| {
            steps += 1;
            if steps > 2 {
                Err(AppError::new(
                    "PORT_CANCELLED",
                    "Engine installation cancelled; existing game files are retained",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(err.code, "PORT_CANCELLED");
        let dest = port_dir(data.path(), "shipwright");
        assert!(!dest.join("large.z64").exists());
        assert_eq!(fs::read_dir(dest).unwrap().count(), 0);
        assert_eq!(fs::read(&source).unwrap(), bytes);
    }
}
