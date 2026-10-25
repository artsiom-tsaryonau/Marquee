//! Bounded archive plans shared by the worker and legacy importer.
use crate::catalog;
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use sevenz_rust2::{ArchiveReader, Password};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MAX_ENTRIES: usize = 20_000;
const MAX_TOTAL: u64 = 128 * 1024 * 1024 * 1024;
const FREE_MARGIN: u64 = 256 * 1024 * 1024;
/// A bare `.iso` at least this large is a DVD (PS2); smaller is a CD (PS1).
const PS2_MIN_ISO: u64 = 750 * 1024 * 1024;

const JUNK_EXT: &[&str] = &[
    "txt", "nfo", "url", "jpg", "jpeg", "png", "gif", "html", "htm", "diz", "md5", "sha1", "sfv",
    "db", "ini", "lnk", "pdf",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Zip,
    SevenZ,
    Rar,
}

#[derive(Debug, Clone)]
pub struct Entry {
    /// Name as stored in the archive, used to find the entry again.
    pub raw: String,
    /// Sanitised relative path.
    pub rel: PathBuf,
    pub size: u64,
    pub index: usize,
}

pub fn kind_of(path: &Path) -> Result<Kind> {
    let mut magic = [0u8; 6];
    let n = File::open(path)?.read(&mut magic)?;
    if n >= 4 && (&magic[..4] == b"PK\x03\x04" || &magic[..4] == b"PK\x05\x06") {
        Ok(Kind::Zip)
    } else if n >= 6 && magic == [b'7', b'z', 0xBC, 0xAF, 0x27, 0x1C] {
        Ok(Kind::SevenZ)
    } else if n >= 6 && &magic[..4] == b"Rar!" && magic[4] == 0x1A && magic[5] == 0x07 {
        Ok(Kind::Rar)
    } else {
        Err(AppError::new(
            "INTAKE_FORMAT",
            "Only .zip, .7z and .rar archives are supported.",
        ))
    }
}

/// A safe relative path, or None if the name could escape the destination.
pub fn safe_rel(name: &str) -> Option<PathBuf> {
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

pub fn unsafe_entry(name: &str) -> AppError {
    AppError::new(
        "INTAKE_UNSAFE",
        format!("Archive entry has an unsafe path and was refused: {name}"),
    )
}

pub fn list_entries(path: &Path, kind: Kind) -> Result<Vec<Entry>> {
    list_entries_bounded(path, kind, MAX_ENTRIES)
}

fn list_entries_bounded(path: &Path, kind: Kind, max_entries: usize) -> Result<Vec<Entry>> {
    let mut out = Vec::new();
    match kind {
        Kind::Zip => {
            let mut zip = zip::ZipArchive::new(File::open(path)?)
                .map_err(|e| AppError::new("INTAKE_FORMAT", format!("Invalid ZIP: {e}")))?;
            if zip.len() > max_entries {
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
        Kind::Rar => {
            let mut archive = crate::rar::Archive::open(path)?;
            let mut index = 0usize;
            while let Some(h) = archive.next()? {
                if index >= max_entries {
                    return Err(AppError::new(
                        "INTAKE_LIMIT",
                        "Archive has too many entries",
                    ));
                }
                let rel = safe_rel(&h.name).ok_or_else(|| unsafe_entry(&h.name))?;
                if !h.directory {
                    out.push(Entry {
                        raw: h.name,
                        rel,
                        size: h.size,
                        index,
                    });
                }
                archive.skip()?;
                index += 1;
            }
        }
        Kind::SevenZ => {
            let reader = ArchiveReader::new(File::open(path)?, Password::empty())
                .map_err(|e| AppError::new("INTAKE_FORMAT", format!("Invalid 7z: {e}")))?;
            let files = &reader.archive().files;
            if files.len() > max_entries {
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
    let total = out
        .iter()
        .try_fold(0u64, |total, e| total.checked_add(e.size));
    if total.is_none_or(|size| size > MAX_TOTAL) {
        return Err(AppError::new(
            "INTAKE_LIMIT",
            "Archive would unpack to more than 128 GiB",
        ));
    }
    Ok(out)
}

pub fn ext_of(rel: &Path) -> String {
    rel.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

pub fn is_junk(rel: &Path) -> bool {
    let name = rel
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    name.starts_with('.')
        || rel.components().any(|c| c.as_os_str() == "__MACOSX")
        || JUNK_EXT.contains(&ext_of(rel).as_str())
}

/// System id for one entry, or None when its extension says nothing.
pub fn system_for(rel: &Path, size: u64, has_cue: bool) -> Option<String> {
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

pub fn looks_like_neogeo_set(stem: &str, entries: &[Entry]) -> bool {
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

pub fn free_bytes(path: &Path) -> Option<u64> {
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
pub fn copy_limited(reader: &mut dyn Read, out: &mut File, size: u64) -> Result<u64> {
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

pub fn extract(path: &Path, kind: Kind, wanted: &[(Entry, PathBuf)], staging: &Path) -> Result<()> {
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
        Kind::Rar => {
            let mut archive = crate::rar::Archive::open(path)?;
            let mut index = 0usize;
            let mut done = 0usize;
            while let Some(h) = archive.next()? {
                if let Some((entry, rel_out)) = wanted.iter().find(|(w, _)| w.index == index) {
                    if h.directory || h.name != entry.raw || h.size != entry.size {
                        return Err(AppError::new(
                            "INTAKE_FORMAT",
                            "RAR entry changed after inspection",
                        ));
                    }
                    let mut file = File::options()
                        .write(true)
                        .create_new(true)
                        .open(target(rel_out)?)?;
                    archive.write(&mut file, entry.size)?;
                    done += 1;
                } else {
                    archive.skip()?;
                }
                index += 1;
            }
            if done != wanted.len() {
                return Err(AppError::new(
                    "INTAKE_FORMAT",
                    "RAR archive ended before all entries were read",
                ));
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
pub fn possible_duplicates(dir: &Path, wanted: &[(Entry, PathBuf)]) -> Vec<String> {
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

pub fn require_preparation_space(available: Option<u64>, required: u64) -> Result<()> {
    if available.is_none_or(|free| free < required) {
        return Err(AppError::new(
            "INTAKE_DISK",
            "Insufficient free space, or space could not be checked; source retained",
        ));
    }
    Ok(())
}

/// Read-only, explicit-platform preparation plan. Unlike legacy intake this
/// never guesses a console from ISO size or selects/creates a library root.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchivePlan {
    pub system: String,
    pub files: Vec<PlannedFile>,
    pub keep_archive: bool,
    pub required_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlannedFile {
    pub path: PathBuf,
    pub size: u64,
}

impl ArchivePlan {
    pub fn inspect(source: &Path, system: &str) -> Result<Self> {
        if system == "ps3" {
            return Err(AppError::new("PREPARATION_PLATFORM","PS3 archive preparation is not implemented. Import a verified unpacked ISO through RPCS3 content discovery."));
        }
        let descriptor = catalog::by_id(system)?;
        let kind = kind_of(source)?;
        let entries = list_entries(source, kind)?;
        let has_cue = entries.iter().any(|e| ext_of(&e.rel) == "cue");
        let stem = source.file_stem().unwrap_or_default().to_string_lossy();
        let keep_archive =
            system == "neogeo" && kind == Kind::Zip && looks_like_neogeo_set(&stem, &entries);
        if keep_archive {
            return Ok(Self {
                system: system.into(),
                keep_archive,
                files: vec![PlannedFile {
                    path: source
                        .file_name()
                        .ok_or_else(|| unsafe_entry("archive"))?
                        .into(),
                    size: fs::metadata(source)?.len(),
                }],
                required_bytes: fs::metadata(source)?.len().saturating_add(FREE_MARGIN),
            });
        }
        let mut files = Vec::new();
        for entry in entries {
            let ext = ext_of(&entry.rel);
            let supported =
                descriptor.supports_extension(&ext) || (system == "ps1" && has_cue && ext == "bin");
            if !supported && is_junk(&entry.rel) {
                continue;
            }
            if !supported {
                return Err(AppError::new("PREPARATION_UNSUPPORTED",
                    format!("{} is not supported {system} media. Choose the correct platform or a platform-specific installer; no files were changed.", entry.rel.display())));
            }
            if files.iter().any(|f: &PlannedFile| f.path == entry.rel) {
                return Err(AppError::new(
                    "INTAKE_UNSAFE",
                    "Duplicate archive output path",
                ));
            }
            files.push(PlannedFile {
                path: entry.rel,
                size: entry.size,
            });
        }
        if files.is_empty() {
            return Err(AppError::new("PREPARATION_UNSUPPORTED", "No supported game media found. Firmware and packages need their own setup workflow."));
        }
        let required_bytes = files
            .iter()
            .map(|f| f.size)
            .sum::<u64>()
            .saturating_add(FREE_MARGIN);
        Ok(Self {
            system: system.into(),
            files,
            keep_archive,
            required_bytes,
        })
    }

    /// Writes only a new job-owned staging directory. Source and final library
    /// files are never modified. The caller owns durable checkpoints/promotion.
    pub fn extract_into(&self, source: &Path, staging: &Path) -> Result<()> {
        if Self::inspect(source, &self.system)? != *self {
            return Err(AppError::new(
                "PREPARATION_CHANGED",
                "Archive layout changed; inspect and review again",
            ));
        }
        let parent = staging.parent().ok_or_else(|| unsafe_entry("staging"))?;
        require_preparation_space(free_bytes(parent), self.required_bytes)?;
        fs::create_dir(staging)?;
        if self.keep_archive {
            fs::copy(source, staging.join(&self.files[0].path))?;
        } else {
            let kind = kind_of(source)?;
            let entries = list_entries(source, kind)?;
            let wanted = self
                .files
                .iter()
                .map(|f| {
                    let entry = entries
                        .iter()
                        .find(|e| e.rel == f.path && e.size == f.size)
                        .ok_or_else(|| {
                            AppError::new("PREPARATION_CHANGED", "Archive entry changed")
                        })?;
                    Ok((entry.clone(), f.path.clone()))
                })
                .collect::<Result<Vec<_>>>()?;
            extract(source, kind, &wanted, staging)?;
            for file in &self.files {
                let path = staging.join(&file.path);
                if ext_of(&path) == "cue" {
                    if let Some(bytes) = crate::cue::normalized_import(&path)? {
                        fs::write(&path, bytes)?;
                    }
                    crate::cue::manifest(&path)?;
                }
            }
        }
        for file in &self.files {
            let path = staging.join(&file.path);
            let meta = fs::symlink_metadata(&path)?;
            if !meta.is_file() || meta.len() != file.size {
                return Err(AppError::new(
                    "PREPARATION_INVALID",
                    "Extracted output size/type did not match the plan",
                ));
            }
            File::open(path)?.sync_all()?;
        }
        let mut directories = BTreeSet::new();
        for file in &self.files {
            let path = staging.join(&file.path);
            for directory in path.parent().into_iter().flat_map(Path::ancestors) {
                if !directory.starts_with(staging) {
                    break;
                }
                directories.insert(directory.to_path_buf());
            }
        }
        for directory in directories.into_iter().rev() {
            File::open(directory)?.sync_all()?;
        }
        File::open(staging)?.sync_all()?;
        Ok(())
    }
}

/// Bounded, streaming header inventory for large console RARs. Never extracts data.
#[derive(Debug, Serialize)]
pub struct ConsoleInventory {
    pub entries: usize,
    pub files: usize,
    pub unpacked_bytes: u64,
    pub candidates: Vec<ConsoleMember>,
    pub sample: Vec<PathBuf>,
    pub truncated: bool,
}
#[derive(Debug, Serialize)]
pub struct ConsoleMember {
    pub path: PathBuf,
    pub bytes: u64,
    pub role: String,
}
pub fn console_rar_inventory(path: &Path) -> Result<ConsoleInventory> {
    let mut archive = crate::rar::Archive::open(path)?;
    let mut inventory = ConsoleInventory {
        entries: 0,
        files: 0,
        unpacked_bytes: 0,
        candidates: vec![],
        sample: vec![],
        truncated: false,
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while let Some(header) = archive.next()? {
        if inventory.entries >= 500_000 || std::time::Instant::now() > deadline {
            inventory.truncated = true;
            break;
        }
        let rel = safe_rel(&header.name).ok_or_else(|| unsafe_entry(&header.name))?;
        inventory.entries += 1;
        if !header.directory {
            inventory.files += 1;
            inventory.unpacked_bytes = inventory
                .unpacked_bytes
                .checked_add(header.size)
                .filter(|n| *n <= MAX_TOTAL)
                .ok_or_else(|| AppError::new("INTAKE_LIMIT", "Console archive exceeds 128 GiB"))?;
            if inventory.sample.len() < 12 {
                inventory.sample.push(rel.clone());
            }
            let filename = rel
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let role = match filename.as_str() {
                "eboot.bin" => Some("boot_candidate"),
                "param.json" => Some("ps5_metadata_candidate"),
                "param.sfo" => Some("ps4_metadata_candidate"),
                _ if ext_of(&rel) == "pkg" => Some("package_candidate"),
                _ => None,
            };
            if let Some(role) = role {
                if inventory.candidates.len() < 256 {
                    inventory.candidates.push(ConsoleMember {
                        path: rel,
                        bytes: header.size,
                        role: role.into(),
                    });
                } else {
                    inventory.truncated = true;
                }
            }
        }
        archive.skip()?;
    }
    Ok(inventory)
}
