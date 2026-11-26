//! On-demand copies of emulator profiles and port saves.
//! A snapshot never follows a symlink, so a link cannot pull in files outside the scope.

use crate::db::now_rfc3339;
use crate::error::{AppError, Result};
use crate::paths::Paths;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const KEEP: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Kind {
    File,
    Symlink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    path: String,
    kind: Kind,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    sha256: String,
    mode: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Manifest {
    scope: String,
    created_at: String,
    entries: Vec<Entry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Info {
    pub id: String,
    pub scope: String,
    pub created_at: String,
    pub bytes: u64,
    pub files: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScopeKind {
    Emulator,
    Port,
}

struct Scope {
    kind: ScopeKind,
    id: String,
    label: String,
}

pub fn list(paths: &Paths) -> Result<Vec<Info>> {
    let root = paths.data_dir.join("backups");
    let mut found = Vec::new();
    let Ok(scopes) = fs::read_dir(&root) else {
        return Ok(found);
    };
    for scope in scopes.flatten() {
        let Ok(archives) = fs::read_dir(scope.path()) else {
            continue;
        };
        for archive in archives.flatten() {
            let name = archive.file_name();
            let Some(id) = name
                .to_str()
                .and_then(|name| name.strip_suffix(".tar.gz"))
                .map(str::to_string)
            else {
                continue;
            };
            let Some(label) = scope.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if let Ok(info) = read_info(&root, &label, &id) {
                found.push(info);
            }
        }
    }
    found.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(found)
}

pub fn create(paths: &Paths, scope: &str, exclude: &[String]) -> Result<Option<Info>> {
    let scope = parse(scope)?;
    let entries = collect(paths, &scope, exclude)?;
    if entries.is_empty() {
        return Ok(None);
    }
    let mut millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_millis())
        .unwrap_or(0);
    let dir = scope_dir(&paths.data_dir, &scope.label);
    fs::create_dir_all(&dir)?;
    while dir.join(format!("{millis}.tar.gz")).exists() {
        millis += 1;
    }
    let id = millis.to_string();
    let manifest = Manifest {
        scope: scope.label.clone(),
        created_at: now_rfc3339(),
        entries,
    };
    write_archive(&paths.data_dir, &dir, &id, &manifest)?;
    prune(&dir)?;
    read_info(&paths.data_dir.join("backups"), &scope.label, &id).map(Some)
}

pub fn restore(paths: &Paths, scope: &str, id: &str, exclude: &[String]) -> Result<Info> {
    let scope = parse(scope)?;
    validate_id(id)?;
    let dir = scope_dir(&paths.data_dir, &scope.label);
    let archive = dir.join(format!("{id}.tar.gz"));
    if !archive.is_file() {
        return Err(AppError::new("SNAPSHOT_MISSING", "That snapshot is gone"));
    }
    let manifest = read_manifest(&dir, id)?;
    let saved = tempfile::NamedTempFile::new_in(&paths.data_dir)?;
    fs::copy(&archive, saved.path())?;
    let pre = create(paths, &scope.label, exclude)?;
    if manifest.scope != scope.label {
        return Err(AppError::new(
            "SNAPSHOT",
            "Snapshot scope does not match this restore",
        ));
    }
    let staging = tempfile::tempdir_in(&paths.data_dir)?;
    extract(saved.path(), staging.path(), &manifest)?;
    swap(paths, &scope, staging.path(), &manifest, exclude)?;
    Ok(pre.unwrap_or(read_info(
        &paths.data_dir.join("backups"),
        &scope.label,
        id,
    )?))
}

pub fn delete(paths: &Paths, scope: &str, id: &str) -> Result<()> {
    let scope = parse(scope)?;
    validate_id(id)?;
    let dir = scope_dir(&paths.data_dir, &scope.label);
    for name in [format!("{id}.tar.gz"), format!("{id}.json")] {
        let path = dir.join(name);
        if path.exists() {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn parse(scope: &str) -> Result<Scope> {
    let (kind, id) = scope.split_once(':').ok_or_else(|| {
        AppError::new(
            "BAD_REQUEST",
            "Snapshot scope is emulator:<id> or port:<id>",
        )
    })?;
    let kind = match kind {
        "emulator" => ScopeKind::Emulator,
        "port" => ScopeKind::Port,
        _ => return Err(AppError::new("BAD_REQUEST", "Unknown snapshot scope")),
    };
    if id.is_empty()
        || id.len() > 40
        || !id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
        })
    {
        return Err(AppError::new("BAD_REQUEST", "Invalid snapshot id"));
    }
    Ok(Scope {
        kind,
        id: id.to_string(),
        label: scope.to_string(),
    })
}

fn validate_id(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > 20 || !id.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(AppError::new("BAD_REQUEST", "Invalid snapshot id"));
    }
    Ok(())
}

fn scope_dir(data: &Path, label: &str) -> PathBuf {
    data.join("backups").join(label.replace(':', "-"))
}

fn collect(paths: &Paths, scope: &Scope, exclude: &[String]) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    match scope.kind {
        ScopeKind::Emulator => {
            for name in ["xdg-data", "xdg-config"] {
                walk(
                    &paths.data_dir.join(&scope.id).join(name),
                    name,
                    exclude,
                    &mut entries,
                )?;
            }
        }
        ScopeKind::Port => walk(
            &paths.data_dir.join("ports").join(&scope.id),
            "",
            exclude,
            &mut entries,
        )?,
    }
    Ok(entries)
}

fn walk(root: &Path, prefix: &str, exclude: &[String], out: &mut Vec<Entry>) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }
    let mut pending = vec![(root.to_path_buf(), prefix.to_string())];
    while let Some((dir, rel_dir)) = pending.pop() {
        for item in fs::read_dir(&dir)? {
            let item = item?;
            let path = item.path();
            let name = item.file_name();
            let name = name.to_string_lossy();
            let rel = if rel_dir.is_empty() {
                name.to_string()
            } else {
                format!("{rel_dir}/{name}")
            };
            if exclude.iter().any(|skipped| skipped == &rel) || name.starts_with(".marquee-") {
                continue;
            }
            let meta = fs::symlink_metadata(&path)?;
            let mode = meta.permissions().mode() & 0o777;
            if meta.file_type().is_symlink() {
                let target = fs::read_link(&path)?;
                out.push(Entry {
                    path: rel,
                    kind: Kind::Symlink,
                    size: 0,
                    sha256: String::new(),
                    mode,
                    target: Some(target.to_string_lossy().into_owned()),
                });
                continue;
            }
            if meta.is_dir() {
                pending.push((path, rel));
                continue;
            }
            if !meta.is_file() {
                continue;
            }
            let (sha256, size) = hash_file(&path)?;
            out.push(Entry {
                path: rel,
                kind: Kind::File,
                size,
                sha256,
                mode,
                target: None,
            });
        }
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<(String, u64)> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        size += read as u64;
        if size > 2 * 1024 * 1024 * 1024 {
            return Err(AppError::new(
                "SNAPSHOT",
                "A save file is larger than 2 GiB",
            ));
        }
        hasher.update(&buffer[..read]);
    }
    Ok((format!("{:x}", hasher.finalize()), size))
}

fn write_archive(data: &Path, dir: &Path, id: &str, manifest: &Manifest) -> Result<()> {
    let json = dir.join(format!("{id}.json"));
    let archive = dir.join(format!("{id}.tar.gz"));
    fs::write(&json, serde_json::to_vec_pretty(manifest)?)?;
    let file = File::create(&archive)?;
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    for entry in &manifest.entries {
        let mut header = tar::Header::new_gnu();
        header.set_mode(entry.mode);
        match entry.kind {
            Kind::Symlink => {
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_size(0);
                header.set_cksum();
                builder.append_link(
                    &mut header,
                    &entry.path,
                    entry.target.as_deref().unwrap_or_default(),
                )?;
            }
            Kind::File => {
                let source = live_root(data, manifest)?.join(&entry.path);
                let mut input = File::open(&source)?;
                header.set_entry_type(tar::EntryType::Regular);
                header.set_size(entry.size);
                header.set_cksum();
                builder.append_data(&mut header, &entry.path, &mut input)?;
            }
        }
    }
    let encoder = builder.into_inner()?;
    encoder.finish()?;
    Ok(())
}

fn live_root(data: &Path, manifest: &Manifest) -> Result<PathBuf> {
    let scope = parse(&manifest.scope)?;
    Ok(match scope.kind {
        ScopeKind::Emulator => data.join(&scope.id),
        ScopeKind::Port => data.join("ports").join(&scope.id),
    })
}

fn read_info(root: &Path, label: &str, id: &str) -> Result<Info> {
    let manifest = read_manifest(&root.join(label.replace(':', "-")), id)?;
    let bytes = fs::metadata(
        root.join(label.replace(':', "-"))
            .join(format!("{id}.tar.gz")),
    )?
    .len();
    Ok(Info {
        id: id.to_string(),
        scope: manifest.scope,
        created_at: manifest.created_at,
        bytes,
        files: manifest.entries.len() as u32,
    })
}

fn read_manifest(dir: &Path, id: &str) -> Result<Manifest> {
    let bytes = fs::read(dir.join(format!("{id}.json")))?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn extract(archive: &Path, staging: &Path, manifest: &Manifest) -> Result<()> {
    let decoder = flate2::read::GzDecoder::new(File::open(archive)?);
    let mut tar = tar::Archive::new(decoder);
    for item in tar.entries()? {
        let mut item = item?;
        let path = item.path()?.into_owned();
        if !path_is_safe(&path) {
            return Err(AppError::new(
                "SNAPSHOT",
                "Snapshot contains an unsafe path",
            ));
        }
        if !item.unpack_in(staging)? {
            return Err(AppError::new(
                "SNAPSHOT",
                "Snapshot path escapes its folder",
            ));
        }
    }
    for entry in &manifest.entries {
        let path = staging.join(&entry.path);
        match entry.kind {
            Kind::File => {
                let (sha256, size) = hash_file(&path)?;
                if sha256 != entry.sha256 || size != entry.size {
                    return Err(AppError::new(
                        "SNAPSHOT",
                        format!(
                            "{} does not match its snapshot (got {size} {sha256}, want {} {})",
                            entry.path, entry.size, entry.sha256
                        ),
                    ));
                }
                fs::set_permissions(&path, fs::Permissions::from_mode(entry.mode))?;
            }
            Kind::Symlink => {
                let target = fs::read_link(&path)?;
                if target.to_string_lossy() != entry.target.as_deref().unwrap_or_default() {
                    return Err(AppError::new(
                        "SNAPSHOT",
                        format!("{} does not match its snapshot link", entry.path),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn path_is_safe(path: &Path) -> bool {
    path.is_relative()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn swap(
    paths: &Paths,
    scope: &Scope,
    staging: &Path,
    manifest: &Manifest,
    exclude: &[String],
) -> Result<()> {
    match scope.kind {
        ScopeKind::Emulator => {
            for name in ["xdg-data", "xdg-config"] {
                if !manifest
                    .entries
                    .iter()
                    .any(|entry| entry.path.starts_with(name))
                {
                    continue;
                }
                replace_tree(
                    &paths.data_dir.join(&scope.id).join(name),
                    &staging.join(name),
                )?;
            }
        }
        ScopeKind::Port => {
            let live = paths.data_dir.join("ports").join(&scope.id);
            for skipped in exclude {
                let source = live.join(skipped);
                if source.is_file() {
                    let dest = staging.join(skipped);
                    if let Some(parent) = dest.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(&source, dest)?;
                }
            }
            replace_tree(&live, staging)?;
        }
    }
    Ok(())
}

fn replace_tree(live: &Path, staged: &Path) -> Result<()> {
    if let Some(parent) = live.parent() {
        fs::create_dir_all(parent)?;
    }
    let aside = live.with_file_name(format!(
        ".{}.snapshot-aside",
        live.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("scope")
    ));
    let _ = fs::remove_dir_all(&aside);
    let had_live = live.exists();
    if had_live {
        fs::rename(live, &aside)?;
    }
    if let Err(error) = fs::rename(staged, live) {
        if had_live {
            let _ = fs::rename(&aside, live);
        }
        return Err(error.into());
    }
    let _ = fs::remove_dir_all(aside);
    Ok(())
}

fn prune(dir: &Path) -> Result<()> {
    let mut archives = Vec::new();
    for item in fs::read_dir(dir)? {
        let item = item?;
        let Some(name) = item
            .file_name()
            .to_str()
            .and_then(|name| name.strip_suffix(".tar.gz"))
            .map(str::to_string)
        else {
            continue;
        };
        archives.push(name);
    }
    archives.sort();
    archives.reverse();
    for old in archives.into_iter().skip(KEEP) {
        let _ = fs::remove_file(dir.join(format!("{old}.tar.gz")));
        let _ = fs::remove_file(dir.join(format!("{old}.json")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn sample(paths: &Paths) {
        let config = paths.data_dir.join("pcsx2/xdg-config");
        fs::create_dir_all(&config).unwrap();
        let file = config.join("settings.ini");
        fs::write(&file, b"volume=4").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();
        fs::write(paths.data_dir.join("outside.txt"), b"secret").unwrap();
        symlink("../outside.txt", config.join("link")).unwrap();
    }

    #[test]
    fn a_snapshot_round_trips_bytes_modes_and_does_not_follow_links() {
        let temp = tempfile::tempdir().unwrap();
        let paths = Paths::resolve(Some(temp.path().join("data"))).unwrap();
        fs::create_dir_all(&paths.data_dir).unwrap();
        sample(&paths);
        let info = create(&paths, "emulator:pcsx2", &[]).unwrap().unwrap();
        fs::write(
            paths.data_dir.join("pcsx2/xdg-config/settings.ini"),
            b"changed",
        )
        .unwrap();
        fs::write(paths.data_dir.join("outside.txt"), b"changed-outside").unwrap();
        restore(&paths, "emulator:pcsx2", &info.id, &[]).unwrap();
        assert_eq!(
            fs::read(paths.data_dir.join("pcsx2/xdg-config/settings.ini")).unwrap(),
            b"volume=4"
        );
        let mode = fs::metadata(paths.data_dir.join("pcsx2/xdg-config/settings.ini"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o640);
        assert_eq!(
            fs::read(paths.data_dir.join("outside.txt")).unwrap(),
            b"changed-outside"
        );
        assert_eq!(
            fs::read_link(paths.data_dir.join("pcsx2/xdg-config/link")).unwrap(),
            Path::new("../outside.txt")
        );
    }

    #[test]
    fn retention_keeps_the_newest_five_and_restore_keeps_a_pre_restore_copy() {
        let temp = tempfile::tempdir().unwrap();
        let paths = Paths::resolve(Some(temp.path().join("data"))).unwrap();
        fs::create_dir_all(&paths.data_dir).unwrap();
        sample(&paths);
        let mut ids = Vec::new();
        for n in 0..6 {
            fs::write(
                paths.data_dir.join("pcsx2/xdg-config/settings.ini"),
                format!("n={n}"),
            )
            .unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2));
            ids.push(create(&paths, "emulator:pcsx2", &[]).unwrap().unwrap().id);
        }
        let listed = list(&paths).unwrap();
        assert_eq!(listed.len(), 5);
        assert!(!listed.iter().any(|info| info.id == ids[0]));
        let newest = ids.last().unwrap();
        fs::write(paths.data_dir.join("pcsx2/xdg-config/settings.ini"), b"now").unwrap();
        let pre = restore(&paths, "emulator:pcsx2", newest, &[]).unwrap();
        assert!(list(&paths).unwrap().iter().any(|info| info.id == pre.id));
        assert_eq!(
            fs::read(paths.data_dir.join("pcsx2/xdg-config/settings.ini")).unwrap(),
            b"n=5"
        );
    }

    #[test]
    fn a_port_snapshot_keeps_saves_and_leaves_the_engine_in_place() {
        let temp = tempfile::tempdir().unwrap();
        let paths = Paths::resolve(Some(temp.path().join("data"))).unwrap();
        let port = paths.data_dir.join("ports/shipwright");
        fs::create_dir_all(&port).unwrap();
        fs::write(port.join("SoH.AppImage"), b"engine-v1").unwrap();
        fs::write(port.join("save.dat"), b"save-v1").unwrap();
        let info = create(&paths, "port:shipwright", &["SoH.AppImage".into()])
            .unwrap()
            .unwrap();
        fs::write(port.join("SoH.AppImage"), b"engine-v2").unwrap();
        fs::write(port.join("save.dat"), b"save-v2").unwrap();
        restore(
            &paths,
            "port:shipwright",
            &info.id,
            &["SoH.AppImage".into()],
        )
        .unwrap();
        assert_eq!(fs::read(port.join("save.dat")).unwrap(), b"save-v1");
        assert_eq!(fs::read(port.join("SoH.AppImage")).unwrap(), b"engine-v2");
    }

    #[test]
    fn restore_is_refused_while_that_emulator_is_running() {
        let temp = tempfile::tempdir().unwrap();
        let core = crate::Core::open_isolated(Some(temp.path().join("data")), true).unwrap();
        let config = core.paths.data_dir.join("pcsx2/xdg-config");
        fs::create_dir_all(&config).unwrap();
        fs::write(config.join("settings.ini"), b"volume=4").unwrap();
        let info = core
            .create_save_snapshot("emulator:pcsx2")
            .unwrap()
            .unwrap();
        core.with_db(|db| {
            db.conn.execute(
                "INSERT INTO editions(id, system, scan_title, imported_at) VALUES('e', 'ps2', 'Game', '2026-01-01')",
                [],
            )?;
            db.conn.execute(
                "INSERT OR IGNORE INTO runners(id, kind, display_name, command_json, bios_state, source) VALUES('runner.pcsx2.marquee', 'emulator', 'PCSX2', '[]', 'missing', 'marquee')",
                [],
            )?;
            db.insert_session(
                "s",
                "e",
                "runner.pcsx2.marquee",
                "2026-01-01T00:00:00Z",
                "running",
            )
        })
        .unwrap();
        assert_eq!(
            core.restore_save_snapshot("emulator:pcsx2", &info.id)
                .unwrap_err()
                .code,
            "LAUNCH_BUSY"
        );
    }
}
