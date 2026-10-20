//! Native ports and decomps (Ship of Harkinian, Zelda64Recomp, ...).
//!
//! Each port is one JSON file in `catalog/ports/`. Install downloads the latest
//! GitHub release asset (no key needed), stages and validates it before replacing
//! the installation, preserves user files, and records the release tag.
//! Marquee never downloads game data.

use crate::error::{AppError, Result};
use crate::http;
use crate::model::CoreEvent;
use crate::paths::Paths;
use crate::sessions::EventSink;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

const CATALOGUE: &[&str] = &[
    include_str!("../../../catalog/ports/shipwright.json"),
    include_str!("../../../catalog/ports/zelda64recomp.json"),
    include_str!("../../../catalog/ports/sm64coopdx.json"),
    include_str!("../../../catalog/ports/fallout2-ce.json"),
    include_str!("../../../catalog/ports/uzdoom.json"),
];

const MAX_EXTRACTED_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
pub struct PortDef {
    pub id: String,
    pub name: String,
    pub game: String,
    pub family: String,
    pub platform: String,
    pub github: String,
    pub asset_suffix: String,
    #[serde(default)]
    pub asset_excludes: Vec<String>,
    /// File to run inside an extracted archive; `None` means the asset itself is the executable (AppImage).
    pub binary: Option<String>,
    pub requirement: String,
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
    /// `not_installed`, `ready` or `update`.
    pub status: String,
    pub folder: Option<PathBuf>,
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

pub fn port_dir(paths: &Paths, id: &str) -> PathBuf {
    paths.data_dir.join("ports").join(id)
}

pub fn info(
    paths: &Paths,
    def: &PortDef,
    installed: Option<&InstalledPort>,
    latest: Option<&str>,
) -> PortInfo {
    let status = match (installed, latest) {
        (None, _) => "not_installed",
        (Some(i), Some(l)) if i.tag != l => "update",
        (Some(_), _) => "ready",
    };
    PortInfo {
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
        folder: installed.map(|_| port_dir(paths, &def.id)),
    }
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
        let sha256 = a
            .get("digest")
            .and_then(|d| d.as_str())
            .and_then(|d| d.strip_prefix("sha256:"))
            .map(|d| d.to_ascii_lowercase());
        Some(Asset {
            name: name.to_string(),
            url,
            sha256,
        })
    });
    let asset = found.ok_or_else(|| {
        AppError::new(
            "NETWORK",
            format!(
                "{} {tag} has no Linux download matching *{}",
                def.name, def.asset_suffix
            ),
        )
    })?;
    Ok(Release { tag, asset })
}

pub fn latest_release(def: &PortDef) -> Result<serde_json::Value> {
    http::get_json(&format!(
        "https://api.github.com/repos/{}/releases/latest",
        def.github
    ))
}

fn progress(sink: &Arc<dyn EventSink>, id: &str, message: impl Into<String>) {
    sink.emit(CoreEvent::PortProgress {
        port_id: id.into(),
        message: message.into(),
    });
}

/// Download and extract the latest release. Returns the tag and executable to remember.
pub fn install_latest(
    paths: &Paths,
    def: &PortDef,
    current_tag: Option<&str>,
    sink: &Arc<dyn EventSink>,
) -> Result<InstalledPort> {
    progress(sink, &def.id, format!("Looking up {}…", def.name));
    let release = pick_asset(def, &latest_release(def)?)?;
    let dir = port_dir(paths, &def.id);
    if current_tag == Some(release.tag.as_str()) {
        if let Some(bin) = locate_binary(&dir, def) {
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
        .prefix(".port-stage-")
        .tempdir_in(parent)?;
    let release_dir = stage.path().join("release");
    fs::create_dir(&release_dir)?;
    let part = stage.path().join("download");
    progress(
        sink,
        &def.id,
        format!("Downloading {} {}…", def.name, release.tag),
    );
    let mut last = 0u64;
    let got = http::download_file(
        &release.asset.url,
        &part,
        http::PORT_MAX_BYTES,
        Duration::from_secs(900),
        |n| {
            if n.saturating_sub(last) >= 8 * 1024 * 1024 {
                last = n;
                progress(sink, &def.id, format!("Downloaded {} MiB…", n / 1_048_576));
            }
        },
    )
    .inspect_err(|_| {
        let _ = fs::remove_file(&part);
    })?;
    match &release.asset.sha256 {
        Some(expected) if *expected != got => {
            let _ = fs::remove_file(&part);
            return Err(AppError::new(
                "NETWORK",
                "download checksum did not match the GitHub release digest",
            )
            .with_context(json!({ "expected": expected, "got": got })));
        }
        Some(_) => {}
        None => progress(
            sink,
            &def.id,
            "This release publishes no checksum; continuing over HTTPS.",
        ),
    }

    let name = release.asset.name.to_ascii_lowercase();
    let result = if name.ends_with(".zip") {
        progress(sink, &def.id, "Unpacking…");
        extract_zip(&part, &release_dir)
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        progress(sink, &def.id, "Unpacking…");
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
    make_executable(&bin)?;
    let installed = InstalledPort {
        tag: release.tag,
        bin: rel(&release_dir, &bin),
    };
    promote_installation(&release_dir, &dir)?;
    Ok(installed)
}

/// Only promote validated release files. A failed copy/extraction leaves the
/// previous installation untouched, including files symlinked into another library.
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
    let backup = dest.with_extension(format!("backup-{}", uuid::Uuid::new_v4()));
    let had_previous = dest.exists();
    if had_previous {
        fs::rename(dest, &backup)?;
    }
    if let Err(err) = fs::rename(stage, dest) {
        if had_previous {
            fs::rename(&backup, dest)?;
        }
        return Err(err.into());
    }
    if had_previous {
        let _ = fs::remove_dir_all(backup);
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

pub(crate) fn extract_zip(archive: &Path, dest: &Path) -> Result<()> {
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

pub(crate) fn extract_tar_gz(archive: &Path, dest: &Path) -> Result<()> {
    let gz = flate2::read::GzDecoder::new(File::open(archive)?);
    let mut tar = tar::Archive::new(gz);
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
pub fn launch(paths: &Paths, def: &PortDef, installed: &InstalledPort) -> Result<()> {
    let dir = port_dir(paths, &def.id);
    let bin = dir.join(&installed.bin);
    if !bin.is_file() {
        return Err(AppError::new(
            "PORT_BROKEN",
            format!("{} is missing; install it again", def.name),
        )
        .with_context(json!({ "path": bin })));
    }
    let cwd = bin.parent().unwrap_or(&dir).to_path_buf();
    let mut child = Command::new(&bin)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| {
            AppError::new("SPAWN_FAILED", err.to_string()).with_context(json!({ "path": bin }))
        })?;
    // Reap the child so it never lingers as a zombie.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Open the port folder in the file manager so the user can drop in their own files.
pub fn open_folder(paths: &Paths, id: &str) -> Result<()> {
    let dir = port_dir(paths, id);
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

    fn release() -> serde_json::Value {
        json!({
            "tag_name": "v1.2.2",
            "assets": [
                {"name": "Zelda64Recompiled-v1.2.2-Linux-Flatpak-X64.zip", "browser_download_url": "https://x/flatpak"},
                {"name": "Zelda64Recompiled-v1.2.2-Linux-ARM64.zip", "browser_download_url": "https://x/arm"},
                {"name": "Zelda64Recompiled-v1.2.2-Linux-X64.zip", "browser_download_url": "https://x/x64", "digest": "sha256:ABCD"}
            ]
        })
    }

    #[test]
    fn catalogue_parses_and_ids_are_unique() {
        let all = catalogue().unwrap();
        assert_eq!(all.len(), 5);
        let mut ids: Vec<_> = all.iter().map(|p| p.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 5);
    }

    #[test]
    fn picks_the_plain_x64_linux_asset() {
        let def = find("zelda64recomp").unwrap();
        let picked = pick_asset(&def, &release()).unwrap();
        assert_eq!(picked.tag, "v1.2.2");
        assert_eq!(picked.asset.url, "https://x/x64");
        assert_eq!(picked.asset.sha256.as_deref(), Some("abcd"));
    }

    #[test]
    fn missing_asset_is_a_typed_error() {
        let def = find("uzdoom").unwrap();
        let err = pick_asset(&def, &release()).err().unwrap();
        assert_eq!(err.code, "NETWORK");
    }

    #[test]
    fn status_reflects_installed_and_latest() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::resolve(Some(dir.path().to_path_buf())).unwrap();
        let def = find("shipwright").unwrap();
        let inst = InstalledPort {
            tag: "1".into(),
            bin: "x".into(),
        };
        assert_eq!(info(&paths, &def, None, Some("2")).status, "not_installed");
        assert_eq!(info(&paths, &def, Some(&inst), None).status, "ready");
        assert_eq!(info(&paths, &def, Some(&inst), Some("1")).status, "ready");
        assert_eq!(info(&paths, &def, Some(&inst), Some("2")).status, "update");
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
}
