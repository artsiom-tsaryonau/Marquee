//! Archive intake: turn a downloaded `.zip` or `.7z` into library files.
//!
//! The archive is inspected first (nothing is written), the system is chosen
//! from the contents, and files are unpacked into that system's library root
//! through a staging folder, so a failed or hostile archive never leaves
//! half-written games behind. Arcade ZIP sets are kept intact because their
//! emulators read the ZIP itself.
//!
//! Limits: entries must stay inside the destination, symlinks are refused,
//! entry count and total size are capped, actual bytes written may not exceed
//! the size an entry declares, and free disk space is checked up front.
//! `.rar` is not supported.

use crate::catalog;
use crate::error::{AppError, Result};
use crate::Core;
use serde::Serialize;
use sevenz_rust2::{ArchiveReader, Password};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_ENTRIES: usize = 20_000;
const MAX_TOTAL: u64 = 128 * 1024 * 1024 * 1024;
const FREE_MARGIN: u64 = 256 * 1024 * 1024;
/// A bare `.iso` at least this large is a DVD (PS2); smaller is a CD (PS1).
const PS2_MIN_ISO: u64 = 750 * 1024 * 1024;

const JUNK_EXT: &[&str] = &[
    "txt", "nfo", "url", "jpg", "jpeg", "png", "gif", "html", "htm", "diz", "md5", "sha1", "sfv",
    "db", "ini", "lnk", "pdf",
];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Files were unpacked into the system's library folder.
    Imported,
    /// An arcade ZIP set was placed unchanged.
    KeptArchive,
    /// Files were stored, but Marquee has no emulator for this system yet.
    StoredNoEmulator,
    /// Everything was already in the library.
    AlreadyPresent,
    /// Nothing was changed.
    Skipped,
}

#[derive(Debug, Clone, Serialize)]
pub struct IntakeReport {
    pub archive: PathBuf,
    pub outcome: Outcome,
    pub system: Option<String>,
    pub dest: Option<PathBuf>,
    pub files: Vec<String>,
    pub message: String,
    pub source_removed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Zip,
    SevenZ,
}

#[derive(Debug, Clone)]
struct Entry {
    /// Name as stored in the archive, used to find the entry again.
    raw: String,
    /// Sanitised relative path.
    rel: PathBuf,
    size: u64,
    index: usize,
}

fn kind_of(path: &Path) -> Result<Kind> {
    let mut magic = [0u8; 6];
    let n = File::open(path)?.read(&mut magic)?;
    if n >= 4 && (&magic[..4] == b"PK\x03\x04" || &magic[..4] == b"PK\x05\x06") {
        Ok(Kind::Zip)
    } else if n >= 6 && magic == [b'7', b'z', 0xBC, 0xAF, 0x27, 0x1C] {
        Ok(Kind::SevenZ)
    } else {
        Err(AppError::new(
            "INTAKE_FORMAT",
            "Only .zip and .7z archives are supported (not .rar).",
        ))
    }
}

/// A safe relative path, or None if the name could escape the destination.
fn safe_rel(name: &str) -> Option<PathBuf> {
    let n = name.replace('\\', "/");
    if n.is_empty() || n.starts_with('/') || n.contains('\0') {
        return None;
    }
    let mut out = PathBuf::new();
    for part in n.split('/') {
        match part {
            "" | "." => continue,
            ".." => return None,
            p if p.len() > 255 => return None,
            p => out.push(p),
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

fn unsafe_entry(name: &str) -> AppError {
    AppError::new(
        "INTAKE_UNSAFE",
        format!("Archive entry has an unsafe path and was refused: {name}"),
    )
}

fn list_entries(path: &Path, kind: Kind) -> Result<Vec<Entry>> {
    let mut out = Vec::new();
    match kind {
        Kind::Zip => {
            let mut zip = zip::ZipArchive::new(File::open(path)?)
                .map_err(|e| AppError::new("INTAKE_FORMAT", format!("Invalid ZIP: {e}")))?;
            if zip.len() > MAX_ENTRIES {
                return Err(AppError::new(
                    "INTAKE_LIMIT",
                    "Archive has too many entries",
                ));
            }
            for i in 0..zip.len() {
                let f = zip
                    .by_index_raw(i)
                    .map_err(|e| AppError::new("INTAKE_FORMAT", format!("Invalid ZIP: {e}")))?;
                if f.is_dir() {
                    continue;
                }
                if f.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
                    return Err(unsafe_entry(f.name()));
                }
                let rel = safe_rel(f.name()).ok_or_else(|| unsafe_entry(f.name()))?;
                out.push(Entry {
                    raw: f.name().to_string(),
                    rel,
                    size: f.size(),
                    index: i,
                });
            }
        }
        Kind::SevenZ => {
            let reader = ArchiveReader::new(File::open(path)?, Password::empty())
                .map_err(|e| AppError::new("INTAKE_FORMAT", format!("Invalid 7z: {e}")))?;
            let files = &reader.archive().files;
            if files.len() > MAX_ENTRIES {
                return Err(AppError::new(
                    "INTAKE_LIMIT",
                    "Archive has too many entries",
                ));
            }
            for (i, f) in files.iter().enumerate() {
                if f.is_directory() || !f.has_stream() && f.size() == 0 && f.name().ends_with('/') {
                    continue;
                }
                let rel = safe_rel(f.name()).ok_or_else(|| unsafe_entry(f.name()))?;
                out.push(Entry {
                    raw: f.name().to_string(),
                    rel,
                    size: f.size(),
                    index: i,
                });
            }
        }
    }
    let total: u64 = out.iter().map(|e| e.size).sum();
    if total > MAX_TOTAL {
        return Err(AppError::new(
            "INTAKE_LIMIT",
            "Archive would unpack to more than 128 GiB",
        ));
    }
    Ok(out)
}

fn ext_of(rel: &Path) -> String {
    rel.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn is_junk(rel: &Path) -> bool {
    let name = rel
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    name.starts_with('.')
        || rel.components().any(|c| c.as_os_str() == "__MACOSX")
        || JUNK_EXT.contains(&ext_of(rel).as_str())
}

/// System id for one entry, or None when its extension says nothing.
fn system_for(rel: &Path, size: u64, has_cue: bool) -> Option<String> {
    let ext = ext_of(rel);
    match ext.as_str() {
        "cue" | "chd" | "pbp" => return Some("ps1".into()),
        "iso" => {
            return Some(if size >= PS2_MIN_ISO { "ps2" } else { "ps1" }.into());
        }
        "bin" if has_cue => return Some("ps1".into()),
        "md" | "gen" | "smd" => return Some("genesis".into()),
        "bin" => return None,
        _ => {}
    }
    catalog::all().ok()?.into_iter().find_map(|d| {
        let only_zip = d.supported_extensions.iter().all(|e| e == "zip");
        (!only_zip && d.supports_extension(&ext)).then_some(d.id)
    })
}

fn looks_like_neogeo_set(stem: &str, entries: &[Entry]) -> bool {
    if stem.eq_ignore_ascii_case("neogeo") {
        return true;
    }
    entries.iter().any(|e| {
        let name = e
            .rel
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        ext_of(&e.rel) == "rom"
            || (name.len() > 6
                && name.as_bytes()[..3].iter().all(u8::is_ascii_digit)
                && name.as_bytes()[3] == b'-')
    })
}

fn free_bytes(path: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: `c` is a valid NUL-terminated path and `s` is a plain C struct.
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    Some((s.f_bavail as u64).saturating_mul(s.f_frsize as u64))
}

/// Copy at most `size` bytes: an entry may not write more than it declared.
fn copy_limited(reader: &mut dyn Read, out: &mut File, size: u64) -> Result<u64> {
    let mut limited = reader.take(size.saturating_add(1));
    let n = std::io::copy(&mut limited, out)?;
    if n > size {
        return Err(AppError::new(
            "INTAKE_LIMIT",
            "An entry unpacked to more than its declared size",
        ));
    }
    out.flush()?;
    Ok(n)
}

fn extract(path: &Path, kind: Kind, wanted: &[(Entry, PathBuf)], staging: &Path) -> Result<()> {
    let target = |rel_out: &Path| -> Result<PathBuf> {
        let p = staging.join(rel_out);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(p)
    };
    match kind {
        Kind::Zip => {
            let mut zip = zip::ZipArchive::new(File::open(path)?)
                .map_err(|e| AppError::new("INTAKE_FORMAT", format!("Invalid ZIP: {e}")))?;
            for (entry, rel_out) in wanted {
                let mut f = zip
                    .by_index(entry.index)
                    .map_err(|e| AppError::new("INTAKE_FORMAT", format!("Bad ZIP entry: {e}")))?;
                let mut out = File::create(target(rel_out)?)?;
                copy_limited(&mut f, &mut out, entry.size)?;
            }
        }
        Kind::SevenZ => {
            let mut reader = ArchiveReader::new(File::open(path)?, Password::empty())
                .map_err(|e| AppError::new("INTAKE_FORMAT", format!("Invalid 7z: {e}")))?;
            let mut failure: Option<AppError> = None;
            reader
                .for_each_entries(|e, data| {
                    let Some((entry, rel_out)) = wanted.iter().find(|(w, _)| w.raw == e.name())
                    else {
                        return Ok(true);
                    };
                    let step = target(rel_out)
                        .and_then(|p| Ok(File::create(p)?))
                        .and_then(|mut out| copy_limited(data, &mut out, entry.size));
                    match step {
                        Ok(_) => Ok(true),
                        Err(err) => {
                            failure = Some(err);
                            Ok(false)
                        }
                    }
                })
                .map_err(|e| AppError::new("INTAKE_FORMAT", format!("7z error: {e}")))?;
            if let Some(err) = failure {
                return Err(err);
            }
        }
    }
    Ok(())
}

/// Files already in `dir` (two levels deep) with the same size and extension
/// as something just unpacked. Size alone is a hint, not proof, so this only
/// warns; it never skips or deletes.
fn possible_duplicates(dir: &Path, wanted: &[(Entry, PathBuf)]) -> Vec<String> {
    let fresh: Vec<PathBuf> = wanted.iter().map(|(_, r)| dir.join(r)).collect();
    let mut found = BTreeSet::new();
    let mut stack = vec![(dir.to_path_buf(), 0usize)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                if depth < 2 && !e.file_name().to_string_lossy().starts_with('.') {
                    stack.push((p, depth + 1));
                }
            } else if meta.len() >= 64 * 1024 * 1024 && !fresh.contains(&p) {
                let hit = wanted
                    .iter()
                    .any(|(en, rel)| en.size == meta.len() && ext_of(rel) == ext_of(&p));
                if hit {
                    found.insert(p.strip_prefix(dir).unwrap_or(&p).display().to_string());
                }
            }
        }
    }
    found.into_iter().collect()
}

impl Core {
    /// Folder for a system's library files, creating and registering one when
    /// no root exists yet. Returns the path and whether the system is known.
    pub(crate) fn intake_root(&self, system: &str) -> Result<PathBuf> {
        let snap = self.snapshot()?;
        if let Some(root) = snap
            .roots
            .iter()
            .find(|r| r.system == system && r.path.is_dir())
        {
            return Ok(root.path.clone());
        }
        let base = snap
            .roots
            .iter()
            .find_map(|r| r.path.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| self.paths.data_dir.join("roms"));
        let dir = base.join(system);
        fs::create_dir_all(&dir)?;
        if catalog::by_id(system).is_ok() {
            self.add_root(system, dir.clone(), None)?;
        }
        Ok(dir)
    }

    /// Unpack one archive into the library. Does not rescan; call `scan` after.
    pub fn intake_archive(&self, archive: &Path, delete_source: bool) -> Result<IntakeReport> {
        let report = |outcome, system: Option<String>, dest, files, message: String| IntakeReport {
            archive: archive.to_path_buf(),
            outcome,
            system,
            dest,
            files,
            message,
            source_removed: false,
        };
        let kind = kind_of(archive)?;
        let entries = list_entries(archive, kind)?;
        let stem = archive
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "game".into());
        let playable: Vec<Entry> = entries
            .iter()
            .filter(|e| !is_junk(&e.rel))
            .cloned()
            .collect();
        if playable.is_empty() {
            return Ok(report(
                Outcome::Skipped,
                None,
                None,
                vec![],
                "Nothing playable in this archive (only documents or images).".into(),
            ));
        }
        let has_cue = playable.iter().any(|e| ext_of(&e.rel) == "cue");
        let systems: BTreeSet<String> = playable
            .iter()
            .filter_map(|e| system_for(&e.rel, e.size, has_cue))
            .collect();

        // Arcade sets stay zipped.
        if systems.is_empty() && kind == Kind::Zip && looks_like_neogeo_set(&stem, &playable) {
            let dest_dir = self.intake_root("neogeo")?;
            let name = archive
                .file_name()
                .map(|n| n.to_os_string())
                .unwrap_or_else(|| "set.zip".into());
            let dest = dest_dir.join(&name);
            if dest.exists() {
                return Ok(report(
                    Outcome::AlreadyPresent,
                    Some("neogeo".into()),
                    Some(dest_dir),
                    vec![name.to_string_lossy().into_owned()],
                    "This set is already in the library.".into(),
                ));
            }
            fs::copy(archive, &dest)?;
            let mut rep = report(
                Outcome::KeptArchive,
                Some("neogeo".into()),
                Some(dest_dir),
                vec![name.to_string_lossy().into_owned()],
                "Arcade set kept as a ZIP. Its BIOS and parent sets must sit beside it.".into(),
            );
            if delete_source {
                fs::remove_file(archive)?;
                rep.source_removed = true;
            }
            return Ok(rep);
        }

        let system = match systems.len() {
            1 => systems.into_iter().next().unwrap(),
            0 => {
                return Ok(report(
                    Outcome::Skipped,
                    None,
                    None,
                    vec![],
                    "Could not tell which system this belongs to. If it is a BIOS pack use `marquee bios import`.".into(),
                ))
            }
            _ => {
                return Ok(report(
                    Outcome::Skipped,
                    None,
                    None,
                    vec![],
                    format!("Archive mixes systems ({}); unpack it by hand.", systems.into_iter().collect::<Vec<_>>().join(", ")),
                ))
            }
        };

        let supported = catalog::by_id(&system).is_ok();
        let dest_dir = if supported {
            self.intake_root(&system)?
        } else {
            let snap = self.snapshot()?;
            let base = snap
                .roots
                .iter()
                .find_map(|r| r.path.parent().map(Path::to_path_buf))
                .unwrap_or_else(|| self.paths.data_dir.join("roms"));
            let dir = base.join(&system);
            fs::create_dir_all(&dir)?;
            dir
        };

        // One file goes straight in; several keep a folder named after the archive.
        let folder = playable.len() > 1;
        let common_top = {
            let first = playable[0].rel.components().next();
            first.filter(|f| {
                playable.len() > 1
                    && playable.iter().all(|e| {
                        e.rel.components().count() > 1 && e.rel.components().next() == Some(*f)
                    })
            })
        };
        let wanted: Vec<(Entry, PathBuf)> = playable
            .iter()
            .map(|e| {
                let rel: PathBuf = match (folder, common_top) {
                    (false, _) => e
                        .rel
                        .file_name()
                        .map(PathBuf::from)
                        .unwrap_or_else(|| e.rel.clone()),
                    (true, Some(_)) => e.rel.clone(),
                    (true, None) => Path::new(&stem).join(&e.rel),
                };
                (e.clone(), rel)
            })
            .collect();

        let need: u64 = wanted.iter().map(|(e, _)| e.size).sum();
        if let Some(free) = free_bytes(&dest_dir) {
            if free < need.saturating_add(FREE_MARGIN) {
                return Err(AppError::new(
                    "INTAKE_DISK",
                    format!(
                        "Not enough free space in {}: need about {} MiB, have {} MiB",
                        dest_dir.display(),
                        need / 1048576,
                        free / 1048576
                    ),
                ));
            }
        }

        let pending: Vec<&(Entry, PathBuf)> = wanted
            .iter()
            .filter(|(e, rel)| fs::metadata(dest_dir.join(rel)).map_or(true, |m| m.len() != e.size))
            .collect();
        if pending.is_empty() {
            return Ok(report(
                Outcome::AlreadyPresent,
                Some(system),
                Some(dest_dir),
                wanted
                    .iter()
                    .map(|(_, r)| r.display().to_string())
                    .collect(),
                "Everything in this archive is already in the library.".into(),
            ));
        }

        let staging = dest_dir.join(format!(".marquee-intake-{}", Uuid::new_v4()));
        fs::create_dir_all(&staging)?;
        let result = (|| -> Result<()> {
            let todo: Vec<(Entry, PathBuf)> = pending.iter().map(|p| (*p).clone()).collect();
            extract(archive, kind, &todo, &staging)?;
            for (_, rel) in &todo {
                let to = dest_dir.join(rel);
                if let Some(parent) = to.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::rename(staging.join(rel), &to)?;
            }
            Ok(())
        })();
        let _ = fs::remove_dir_all(&staging);
        result?;

        let files: Vec<String> = wanted
            .iter()
            .map(|(_, r)| r.display().to_string())
            .collect();
        let (outcome, message) = if supported {
            (
                Outcome::Imported,
                format!("Unpacked {} file(s).", files.len()),
            )
        } else {
            (
                Outcome::StoredNoEmulator,
                format!(
                    "Stored {} file(s) under {system}, but Marquee has no emulator for it yet.",
                    files.len()
                ),
            )
        };
        let message = match possible_duplicates(&dest_dir, &wanted) {
            d if d.is_empty() => message,
            d => format!("{message} Possible duplicate of: {}.", d.join(", ")),
        };
        let mut rep = report(
            outcome,
            Some(system),
            Some(dest_dir.clone()),
            files,
            message,
        );
        if delete_source {
            let verified = wanted
                .iter()
                .all(|(e, r)| fs::metadata(dest_dir.join(r)).is_ok_and(|m| m.len() == e.size));
            if verified {
                fs::remove_file(archive)?;
                rep.source_removed = true;
            }
        }
        Ok(rep)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_zip(path: &Path, files: &[(&str, &[u8])]) {
        let mut w = zip::ZipWriter::new(File::create(path).unwrap());
        for (name, data) in files {
            w.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap();
    }

    fn core(dir: &Path) -> Core {
        Core::open(Some(dir.to_path_buf()), true).unwrap()
    }

    #[test]
    fn rejects_paths_that_escape() {
        assert!(safe_rel("../evil").is_none());
        assert!(safe_rel("a/../../evil").is_none());
        assert!(safe_rel("/etc/passwd").is_none());
        assert_eq!(safe_rel("a\\b.nds").unwrap(), PathBuf::from("a/b.nds"));
    }

    #[test]
    fn single_cartridge_lands_in_its_system_root_and_source_can_go() {
        let tmp = tempfile::tempdir().unwrap();
        let c = core(&tmp.path().join("data"));
        let zip = tmp.path().join("Game (Spain).zip");
        make_zip(
            &zip,
            &[("Game (Spain).nds", b"nds-bytes"), ("readme.txt", b"hi")],
        );
        let r = c.intake_archive(&zip, true).unwrap();
        assert_eq!(r.outcome, Outcome::Imported);
        assert_eq!(r.system.as_deref(), Some("ds"));
        assert_eq!(r.files, vec!["Game (Spain).nds"]);
        let dest = r.dest.unwrap().join("Game (Spain).nds");
        assert_eq!(fs::read(dest).unwrap(), b"nds-bytes");
        assert!(r.source_removed && !zip.exists());
        // Second time: nothing to do.
        make_zip(&zip, &[("Game (Spain).nds", b"nds-bytes")]);
        let again = c.intake_archive(&zip, false).unwrap();
        assert_eq!(again.outcome, Outcome::AlreadyPresent);
    }

    #[test]
    fn multi_file_disc_gets_a_folder_and_leaves_no_staging() {
        let tmp = tempfile::tempdir().unwrap();
        let c = core(&tmp.path().join("data"));
        let zip = tmp.path().join("Disc (USA).zip");
        make_zip(
            &zip,
            &[
                ("Disc (USA).cue", b"FILE \"Disc (USA).bin\" BINARY"),
                ("Disc (USA).bin", b"xx"),
            ],
        );
        let r = c.intake_archive(&zip, false).unwrap();
        assert_eq!(r.system.as_deref(), Some("ps1"));
        let dest = r.dest.unwrap();
        assert!(dest.join("Disc (USA)/Disc (USA).cue").is_file());
        assert!(fs::read_dir(&dest).unwrap().flatten().all(|e| !e
            .file_name()
            .to_string_lossy()
            .starts_with(".marquee-intake")));
        assert!(zip.exists());
    }

    #[test]
    fn arcade_set_stays_zipped() {
        let tmp = tempfile::tempdir().unwrap();
        let c = core(&tmp.path().join("data"));
        let zip = tmp.path().join("kof98.zip");
        make_zip(&zip, &[("242-p1.p1", b"a"), ("242-c1.rom", b"b")]);
        let r = c.intake_archive(&zip, false).unwrap();
        assert_eq!(r.outcome, Outcome::KeptArchive);
        assert!(r.dest.unwrap().join("kof98.zip").is_file());
    }

    #[test]
    fn hostile_archive_is_refused_and_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let c = core(&tmp.path().join("data"));
        let zip = tmp.path().join("evil.zip");
        make_zip(&zip, &[("../escape.nds", b"x")]);
        let err = c.intake_archive(&zip, false).unwrap_err();
        assert_eq!(err.code, "INTAKE_UNSAFE");
        assert!(!tmp.path().join("escape.nds").exists());
    }

    #[test]
    fn bios_pack_and_documents_are_skipped_not_guessed() {
        let tmp = tempfile::tempdir().unwrap();
        let c = core(&tmp.path().join("data"));
        let bios = tmp.path().join("bios.zip");
        make_zip(&bios, &[("SCPH-70012.bin", b"b")]);
        assert_eq!(
            c.intake_archive(&bios, false).unwrap().outcome,
            Outcome::Skipped
        );
        let docs = tmp.path().join("docs.zip");
        make_zip(&docs, &[("readme.txt", b"b")]);
        assert_eq!(
            c.intake_archive(&docs, false).unwrap().outcome,
            Outcome::Skipped
        );
    }

    #[test]
    fn rar_is_refused_by_content_not_name() {
        let tmp = tempfile::tempdir().unwrap();
        let c = core(&tmp.path().join("data"));
        let f = tmp.path().join("x.zip");
        fs::write(&f, b"Rar!\x1a\x07\x00 not a zip").unwrap();
        assert_eq!(
            c.intake_archive(&f, false).unwrap_err().code,
            "INTAKE_FORMAT"
        );
    }
}
