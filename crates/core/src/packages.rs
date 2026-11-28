//! `.marquee` game packages: export a prepared game to one file, import it into
//! another Marquee library.
//!
//! A package is a ZIP64 with stored members: `marquee.json` first, then the game
//! (`game/`) and, for PS4, its update (`update/`) and add-ons (`addcont/`). Every
//! file is hashed. Saves, BIOS, firmware, keys and licences are never included.
//! Per-title fixes are not packaged either: the receiving Marquee seeds them from
//! its own catalogue. Nothing in a package is executed during import.

use crate::db::Db;
use crate::paths::Paths;
use crate::{AppError, Core, Result};
use marquee_manager::portable::{self, Manifest, PayloadFile, RuntimeRef, MANIFEST_NAME};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const MAX_MANIFEST: u64 = 128 * 1024 * 1024;
/// Left free on the destination disk beyond the payload itself.
const SPARE: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Serialize)]
pub struct Exported {
    pub package: PathBuf,
    pub files: usize,
    pub bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct ImportReport {
    pub title: String,
    pub platform: String,
    pub files: usize,
    pub bytes: u64,
    /// Folders or files this import creates.
    pub creates: Vec<PathBuf>,
    pub warnings: Vec<String>,
    pub imported: bool,
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::new("PORTABLE", message.into())
}

fn runtime_id(system: &str) -> Result<String> {
    let descriptor = crate::catalog::by_id(system)?;
    Ok(descriptor.emulator.unwrap_or(descriptor.runner_kind))
}

fn installed_version(db: &Db, runtime: &str) -> Result<Option<String>> {
    Ok(crate::emulators::installed(db, runtime)?.map(|i| i.version))
}

fn addcont_dir(paths: &Paths, title_id: &str) -> PathBuf {
    paths
        .data_dir
        .join("shadps4/xdg-data/shadPS4/addcont")
        .join(title_id)
}

/// Every regular file below `root`, as `<prefix>/<relative path>`. A link inside a
/// game would copy something outside it, so links are refused.
fn collect(root: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(root)?.collect::<io::Result<_>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let kind = entry.file_type()?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid("a file name is not valid UTF-8"))?;
        let relative = format!("{prefix}/{name}");
        if kind.is_dir() {
            collect(&entry.path(), &relative, out)?;
        } else if kind.is_file() {
            out.push((relative, entry.path()));
        } else {
            return Err(invalid(format!(
                "{} is a link or special file; packages hold plain files only",
                entry.path().display()
            )));
        }
    }
    Ok(())
}

/// Copy at most `limit` bytes and return how many were copied and their SHA-256.
fn copy_hashed(from: &mut impl Read, to: &mut impl Write, limit: u64) -> Result<(u64, String)> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut total = 0u64;
    let mut limited = from.take(limit);
    loop {
        let read = limited.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        to.write_all(&buffer[..read])?;
        total += read as u64;
    }
    Ok((total, format!("{:x}", hasher.finalize())))
}

struct Plan {
    manifest: Manifest,
    sources: Vec<(String, PathBuf)>,
}

fn plan_export(db: &Db, paths: &Paths, edition_id: &str) -> Result<Plan> {
    let edition = db
        .edition_by_id(edition_id)?
        .ok_or_else(|| invalid("that game is not in the library"))?;
    let media = crate::library::find_media_for_edition(db, edition_id)?
        .filter(|m| m.available)
        .ok_or_else(|| invalid("the game's files are not available"))?;
    let system = edition.system.clone();
    let file_name = |path: &Path| -> Result<String> {
        path.file_name()
            .and_then(|n| n.to_str())
            .map(str::to_owned)
            .ok_or_else(|| invalid("a file name is not valid UTF-8"))
    };
    let title_id = media.serial.clone().unwrap_or_default();
    let entry = file_name(&media.path)?;
    let parent = media
        .path
        .parent()
        .ok_or_else(|| invalid("the game file has no folder"))?;
    let mut sources = Vec::new();
    let folder = if system == "ps4" {
        if !entry.eq_ignore_ascii_case("eboot.bin") {
            return Err(invalid("expected a prepared PS4 game folder"));
        }
        collect(parent, "game", &mut sources)?;
        let name = file_name(parent)?;
        for suffix in ["-UPDATE", "-patch"] {
            let overlay = parent.with_file_name(format!("{name}{suffix}"));
            if overlay.is_dir() {
                collect(&overlay, "update", &mut sources)?;
                break; // shadPS4 reads the first of these it finds.
            }
        }
        let addons = addcont_dir(paths, &title_id);
        if !title_id.is_empty() && addons.is_dir() {
            collect(&addons, "addcont", &mut sources)?;
        }
        Some(name)
    } else {
        // Games spread over several files need rules of their own; refusing them
        // is better than exporting half a game.
        let extension = media
            .path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let companions = matches!(extension.as_str(), "cue" | "gdi" | "m3u" | "ccd" | "mds");
        let installed_ps3 = system == "ps3" && entry.eq_ignore_ascii_case("EBOOT.BIN");
        let wiiu_folder = system == "wiiu" && extension == "rpx";
        if companions || installed_ps3 || wiiu_folder {
            return Err(invalid(
                "this game is made of several files; packaging it is not supported yet",
            ));
        }
        if !fs::symlink_metadata(&media.path)?.is_file() {
            return Err(invalid("the game file is a link; package the real file"));
        }
        sources.push((format!("game/{entry}"), media.path.clone()));
        None
    };
    let runtime = runtime_id(&system)?;
    let manifest = Manifest {
        schema: portable::SCHEMA,
        platform: system,
        title_id,
        title: edition.display_title.unwrap_or(edition.scan_title),
        folder,
        entry,
        files: Vec::new(),
        runtime: RuntimeRef {
            version: installed_version(db, &runtime)?,
            id: runtime,
            os: std::env::consts::OS.into(),
            cpu: std::env::consts::ARCH.into(),
        },
    };
    Ok(Plan { manifest, sources })
}

fn package_name(manifest: &Manifest) -> String {
    let title: String = manifest
        .title
        .chars()
        .map(|c| if "/\\:*?\"<>|\0".contains(c) { ' ' } else { c })
        .collect();
    let title: String = title
        .trim()
        .trim_start_matches('.')
        .chars()
        .take(120)
        .collect();
    let id = if manifest.title_id.is_empty() || title.contains(&manifest.title_id) {
        String::new()
    } else {
        format!(" [{}]", manifest.title_id)
    };
    format!("{title}{id}.{}.marquee", manifest.platform)
}

fn zip_error(error: zip::result::ZipError) -> AppError {
    invalid(format!("package archive: {error}"))
}

fn write_package(mut plan: Plan, out_dir: &Path) -> Result<Exported> {
    for (name, source) in &plan.sources {
        let size = fs::metadata(source)?.len();
        let (read, sha256) = copy_hashed(&mut File::open(source)?, &mut io::sink(), u64::MAX)?;
        if read != size {
            return Err(invalid(format!(
                "{} changed during export",
                source.display()
            )));
        }
        plan.manifest.files.push(PayloadFile {
            path: name.clone(),
            size,
            sha256,
        });
    }
    portable::check(&plan.manifest)?;
    fs::create_dir_all(out_dir)?;
    let target = out_dir.join(package_name(&plan.manifest));
    if target.exists() {
        return Err(invalid(format!("{} already exists", target.display())));
    }
    let part = tempfile::Builder::new()
        .prefix(".export-")
        .suffix(".part")
        .tempfile_in(out_dir)?;
    let mut zip = ZipWriter::new(part.reopen()?);
    // Game data is mostly compressed already, so members are stored.
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file(MANIFEST_NAME, stored).map_err(zip_error)?;
    zip.write_all(serde_json::to_string_pretty(&plan.manifest)?.as_bytes())?;
    for ((name, source), listed) in plan.sources.iter().zip(&plan.manifest.files) {
        let options = stored.large_file(listed.size >= u32::MAX as u64);
        zip.start_file(name.as_str(), options).map_err(zip_error)?;
        let (written, sha256) = copy_hashed(&mut File::open(source)?, &mut zip, u64::MAX)?;
        if written != listed.size || sha256 != listed.sha256 {
            return Err(invalid(format!(
                "{} changed during export",
                source.display()
            )));
        }
    }
    zip.finish().map_err(zip_error)?.sync_all()?;
    // Never replace a package that appeared meanwhile.
    part.persist_noclobber(&target)
        .map_err(|e| AppError::from(e.error))?;
    Ok(Exported {
        files: plan.manifest.files.len(),
        bytes: plan.manifest.total_bytes(),
        package: target,
    })
}

fn open_package(path: &Path) -> Result<(ZipArchive<File>, Manifest)> {
    let mut zip = ZipArchive::new(File::open(path)?).map_err(zip_error)?;
    let manifest = {
        let entry = zip
            .by_name(MANIFEST_NAME)
            .map_err(|_| invalid("not a .marquee package: marquee.json is missing"))?;
        if entry.size() > MAX_MANIFEST {
            return Err(invalid("the manifest is too large"));
        }
        let mut raw = String::new();
        entry.take(MAX_MANIFEST).read_to_string(&mut raw)?;
        portable::parse(&raw)?
    };
    // The archive holds exactly what the manifest lists.
    let listed: HashSet<&str> = manifest.files.iter().map(|f| f.path.as_str()).collect();
    if zip.len() != listed.len() + 1
        || zip
            .file_names()
            .any(|name| name != MANIFEST_NAME && !listed.contains(name))
    {
        return Err(invalid("the archive and its manifest list different files"));
    }
    Ok((zip, manifest))
}

fn free_bytes(dir: &Path) -> Result<u64> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(dir.as_os_str().as_bytes())
        .map_err(|_| invalid("bad destination path"))?;
    let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(path.as_ptr(), &mut stats) } != 0 {
        return Err(io::Error::last_os_error().into());
    }
    Ok((stats.f_bavail as u64).saturating_mul(stats.f_frsize as u64))
}

/// Where each part of a package goes, and the add-on folders it brings.
struct Layout {
    game: PathBuf,
    update: Option<PathBuf>,
    addons: Vec<(String, PathBuf)>,
}

fn layout(paths: &Paths, manifest: &Manifest, root: &Path) -> Layout {
    let game = match &manifest.folder {
        Some(folder) => root.join(folder),
        None => root.join(&manifest.entry),
    };
    let update = manifest
        .folder
        .as_ref()
        .filter(|_| manifest.files.iter().any(|f| f.path.starts_with("update/")))
        .map(|folder| root.join(format!("{folder}-UPDATE")));
    let mut labels: Vec<String> = manifest
        .files
        .iter()
        .filter_map(|f| f.path.strip_prefix("addcont/")?.split('/').next())
        .map(str::to_owned)
        .collect();
    labels.sort();
    labels.dedup();
    let base = addcont_dir(paths, &manifest.title_id);
    let addons = labels
        .into_iter()
        .map(|label| (label.clone(), base.join(label)))
        .collect();
    Layout {
        game,
        update,
        addons,
    }
}

/// Extract into hidden staging folders, check every hash, then move into place
/// without replacing anything. A failure leaves the library as it was.
fn extract(
    zip: &mut ZipArchive<File>,
    manifest: &Manifest,
    plan: &Layout,
    root: &Path,
) -> Result<()> {
    // Beside the library root, so a scan never sees a half-written game.
    let staging_parent = root.parent().unwrap_or(root);
    let staging = tempfile::Builder::new()
        .prefix(".marquee-import-")
        .tempdir_in(staging_parent)?;
    let addon_staging = match plan.addons.first() {
        Some((_, first)) => {
            let parent = first
                .parent()
                .and_then(Path::parent)
                .ok_or_else(|| invalid("bad add-on location"))?;
            fs::create_dir_all(parent)?;
            Some(
                tempfile::Builder::new()
                    .prefix(".marquee-import-")
                    .tempdir_in(parent)?,
            )
        }
        None => None,
    };
    for file in &manifest.files {
        let target = match (file.path.strip_prefix("addcont/"), &addon_staging) {
            (Some(rest), Some(dir)) => dir.path().join(rest),
            _ => staging.path().join(&file.path),
        };
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut entry = zip.by_name(&file.path).map_err(zip_error)?;
        let mut out = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)?;
        let (written, sha256) = copy_hashed(&mut entry, &mut out, file.size.saturating_add(1))
            .map_err(|e| invalid(format!("{} is damaged: {}", file.path, e.message)))?;
        if written != file.size || !sha256.eq_ignore_ascii_case(&file.sha256) {
            return Err(invalid(format!(
                "{} does not match the manifest; the package is damaged or altered",
                file.path
            )));
        }
    }
    let mut moves: Vec<(PathBuf, PathBuf)> = Vec::new();
    match &manifest.folder {
        Some(_) => moves.push((staging.path().join("game"), plan.game.clone())),
        None => moves.push((
            staging.path().join("game").join(&manifest.entry),
            plan.game.clone(),
        )),
    }
    if let Some(update) = &plan.update {
        moves.push((staging.path().join("update"), update.clone()));
    }
    if let Some(dir) = &addon_staging {
        for (label, target) in &plan.addons {
            moves.push((dir.path().join(label), target.clone()));
        }
    }
    let mut done: Vec<(&PathBuf, &PathBuf)> = Vec::new();
    for (from, to) in &moves {
        let moved = (|| -> io::Result<()> {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            if to.symlink_metadata().is_ok() {
                return Err(io::Error::from(io::ErrorKind::AlreadyExists));
            }
            fs::rename(from, to)
        })();
        if let Err(error) = moved {
            for (from, to) in done.into_iter().rev() {
                let _ = fs::rename(to, from);
            }
            return Err(invalid(format!(
                "could not place {}: {error}",
                to.display()
            )));
        }
        done.push((from, to));
    }
    Ok(())
}

fn import_into(
    db: &Db,
    paths: &Paths,
    package: &Path,
    root: &Path,
    dry_run: bool,
) -> Result<ImportReport> {
    let (mut zip, manifest) = open_package(package)?;
    let runtime = runtime_id(&manifest.platform)?;
    let plan = layout(paths, &manifest, root);
    let mut creates = vec![plan.game.clone()];
    creates.extend(plan.update.clone());
    creates.extend(plan.addons.iter().map(|(_, path)| path.clone()));
    if let Some(taken) = creates.iter().find(|p| p.symlink_metadata().is_ok()) {
        return Err(invalid(format!(
            "{} already exists; nothing was changed",
            taken.display()
        )));
    }
    let free = free_bytes(root)?;
    if free < manifest.total_bytes().saturating_add(SPARE) {
        return Err(invalid(format!(
            "not enough space: {} bytes needed, {free} free",
            manifest.total_bytes()
        )));
    }
    let mut warnings = Vec::new();
    match (installed_version(db, &runtime)?, &manifest.runtime.version) {
        (None, _) => warnings.push(format!("{runtime} is not installed here")),
        (Some(here), Some(there)) if &here != there => warnings.push(format!(
            "exported with {runtime} {there}; this library has {here}"
        )),
        _ => {}
    }
    if !dry_run {
        extract(&mut zip, &manifest, &plan, root)?;
    }
    Ok(ImportReport {
        title: manifest.title.clone(),
        platform: manifest.platform.clone(),
        files: manifest.files.len(),
        bytes: manifest.total_bytes(),
        creates,
        warnings,
        imported: !dry_run,
    })
}

impl Core {
    /// Write `edition_id`'s game to a new `.marquee` file in `out_dir`.
    pub fn export_package(&self, edition_id: &str, out_dir: &Path) -> Result<Exported> {
        // Plan under the database lock; hash and copy without it.
        let plan = self.with_db(|db| plan_export(db, &self.paths, edition_id))?;
        write_package(plan, out_dir)
    }

    /// Add a package's game to the library folder for its system, or to `into`
    /// (which must be one of that system's library folders). With `dry_run`,
    /// report what would happen and change nothing.
    pub fn import_package(
        &self,
        package: &Path,
        into: Option<&Path>,
        dry_run: bool,
    ) -> Result<ImportReport> {
        let (_, manifest) = open_package(package)?;
        let system = manifest.platform.clone();
        let into = into.map(fs::canonicalize).transpose()?;
        let roots: Vec<_> = self
            .with_db(|db| db.list_roots())?
            .into_iter()
            .filter(|r| r.system == system && r.path.is_dir())
            .filter(|r| into.as_ref().is_none_or(|wanted| &r.path == wanted))
            .collect();
        let root = match roots.as_slice() {
            [only] => only,
            [] => {
                return Err(invalid(format!(
                    "no library folder for {system}; add one first (library add-root)"
                )))
            }
            many => {
                let paths: Vec<_> = many.iter().map(|r| r.path.display().to_string()).collect();
                return Err(invalid(format!(
                    "several {system} library folders; choose one with --into: {}",
                    paths.join(", ")
                )));
            }
        };
        let root_id = root.id.clone();
        let root_path = root.path.clone();
        let report =
            self.with_db(|db| import_into(db, &self.paths, package, &root_path, dry_run))?;
        if report.imported {
            self.scan(Some(&root_id), None)?;
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(path: &Path, contents: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    struct Library {
        dir: tempfile::TempDir,
        paths: Paths,
        db: Db,
    }

    fn library() -> Library {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::resolve(Some(dir.path().join("data"))).unwrap();
        fs::create_dir_all(dir.path().join("roms")).unwrap();
        Library {
            paths,
            db: Db::open_in_memory().unwrap(),
            dir,
        }
    }

    /// A PS4 game with an update and one add-on, as the scanner would list it.
    fn exported_ps4(source: &Library, out: &Path) -> PathBuf {
        let root = source.dir.path().join("roms");
        put(&root.join("Game [CUSA00000]/eboot.bin"), b"base program");
        put(&root.join("Game [CUSA00000]/data/level"), b"level data");
        put(
            &root.join("Game [CUSA00000]-UPDATE/eboot.bin"),
            b"new program",
        );
        let addons = addcont_dir(&source.paths, "CUSA00000");
        put(&addons.join("DLC000001/sce_sys/param.sfo"), b"addon");
        let mut sources = Vec::new();
        collect(&root.join("Game [CUSA00000]"), "game", &mut sources).unwrap();
        collect(
            &root.join("Game [CUSA00000]-UPDATE"),
            "update",
            &mut sources,
        )
        .unwrap();
        collect(&addons, "addcont", &mut sources).unwrap();
        let manifest = Manifest {
            schema: portable::SCHEMA,
            platform: "ps4".into(),
            title_id: "CUSA00000".into(),
            title: "Game: The Test".into(),
            folder: Some("Game [CUSA00000]".into()),
            entry: "eboot.bin".into(),
            files: Vec::new(),
            runtime: RuntimeRef {
                id: "shadps4".into(),
                version: Some("v.0.21.0".into()),
                os: "linux".into(),
                cpu: "x86_64".into(),
            },
        };
        write_package(Plan { manifest, sources }, out)
            .unwrap()
            .package
    }

    #[test]
    fn a_ps4_game_with_update_and_addon_moves_between_libraries() {
        let (a, b) = (library(), library());
        let package = exported_ps4(&a, a.dir.path());
        assert!(package.ends_with("Game  The Test [CUSA00000].ps4.marquee"));
        let root = b.dir.path().join("roms");

        let preview = import_into(&b.db, &b.paths, &package, &root, true).unwrap();
        assert!(!preview.imported && preview.creates.len() == 3);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        assert!(preview.warnings[0].contains("not installed"));

        let report = import_into(&b.db, &b.paths, &package, &root, false).unwrap();
        assert!(report.imported && report.files == 4);
        let read = |p: PathBuf| fs::read_to_string(p).unwrap();
        assert_eq!(read(root.join("Game [CUSA00000]/data/level")), "level data");
        assert_eq!(
            read(root.join("Game [CUSA00000]-UPDATE/eboot.bin")),
            "new program"
        );
        assert_eq!(
            read(addcont_dir(&b.paths, "CUSA00000").join("DLC000001/sce_sys/param.sfo")),
            "addon"
        );
        // A second import replaces nothing.
        assert!(import_into(&b.db, &b.paths, &package, &root, false).is_err());
    }

    #[test]
    fn a_damaged_package_leaves_the_library_untouched() {
        let (a, b) = (library(), library());
        let package = exported_ps4(&a, a.dir.path());
        let mut bytes = fs::read(&package).unwrap();
        let at = bytes.windows(10).position(|w| w == b"level data").unwrap();
        bytes[at] ^= 1;
        fs::write(&package, bytes).unwrap();
        let root = b.dir.path().join("roms");
        let error = import_into(&b.db, &b.paths, &package, &root, false).unwrap_err();
        assert_eq!(error.code, "PORTABLE");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        assert!(!addcont_dir(&b.paths, "CUSA00000").exists());
        let leftovers = fs::read_dir(b.dir.path())
            .unwrap()
            .filter(|e| {
                let name = e.as_ref().unwrap().file_name();
                name.to_string_lossy().starts_with(".marquee-import-")
            })
            .count();
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn links_are_not_packaged() {
        let dir = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("/etc/hostname", dir.path().join("link")).unwrap();
        assert!(collect(dir.path(), "game", &mut Vec::new()).is_err());
    }
}
