//! Read-only PS3 discovery. Never extracts, installs, decrypts or registers games.
use crate::error::{AppError, Result};
use marquee_media::archive::{ext_of, kind_of, list_entries};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_SFO: usize = 1024 * 1024;
const MAX_ENTRIES: usize = 20_000;
const MAX_ITEMS: usize = 200;
const MAX_DEPTH: usize = 8;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Metadata {
    pub title: String,
    pub title_id: String,
    pub category: String,
    pub app_version: Option<String>,
    pub system_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Candidate {
    pub path: PathBuf,
    pub kind: String,
    pub bytes: Option<u64>,
    pub metadata: Option<Metadata>,
    pub boot_file_present: bool,
    pub artwork_present: bool,
    pub status: String,
    pub next_action: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Inspection {
    pub source: PathBuf,
    pub source_kind: String,
    pub source_bytes: Option<u64>,
    pub archive_unpacked_bytes: Option<u64>,
    pub inspected_entries: usize,
    pub truncated: bool,
    pub items: Vec<Candidate>,
    pub diagnostics: Vec<String>,
    /// Discovery cannot confer readiness, even for a complete-looking folder.
    pub playable: bool,
}

impl Inspection {
    fn note(&mut self, message: String) {
        if self.diagnostics.len() < 100 {
            self.diagnostics.push(message);
        } else {
            self.truncated = true;
        }
    }
}

fn invalid(message: &str) -> AppError {
    AppError::new("PS3_METADATA", message)
}
fn u32_at(b: &[u8], offset: usize) -> Result<usize> {
    let bytes = b
        .get(offset..offset + 4)
        .ok_or_else(|| invalid("Truncated PARAM.SFO"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()) as usize)
}

/// Original bounded PSF reader, based on format fields checked against RPCS3's
/// Loader/PSF.cpp. Only text identity fields are retained; no upstream code copied.
pub fn parse_sfo(b: &[u8]) -> Result<Metadata> {
    if b.len() > MAX_SFO || b.get(..4) != Some(b"\0PSF") || u32_at(b, 4)? != 0x101 {
        return Err(invalid("Invalid or oversized PARAM.SFO"));
    }
    let keys = u32_at(b, 8)?;
    let data = u32_at(b, 12)?;
    let count = u32_at(b, 16)?;
    if count > 512 || keys < 20 + count * 16 || keys > data || data > b.len() {
        return Err(invalid("Invalid PARAM.SFO table bounds"));
    }
    let mut fields = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for i in 0..count {
        let entry = 20 + i * 16;
        let key_offset = u16::from_le_bytes(b[entry..entry + 2].try_into().unwrap()) as usize;
        let format = u16::from_le_bytes(b[entry + 2..entry + 4].try_into().unwrap());
        let len = u32_at(b, entry + 4)?;
        let max = u32_at(b, entry + 8)?;
        let offset = u32_at(b, entry + 12)?;
        let key_bytes = b
            .get(keys + key_offset..data)
            .ok_or_else(|| invalid("Invalid PARAM.SFO key offset"))?;
        let end = key_bytes
            .iter()
            .position(|v| *v == 0)
            .ok_or_else(|| invalid("Unterminated PARAM.SFO key"))?;
        if end == 0
            || end > 128
            || !key_bytes[..end]
                .iter()
                .all(|v| v.is_ascii_alphanumeric() || *v == b'_')
        {
            return Err(invalid("Invalid PARAM.SFO key"));
        }
        let key = std::str::from_utf8(&key_bytes[..end])
            .map_err(|_| invalid("Invalid PARAM.SFO key encoding"))?;
        if !seen.insert(key)
            || len > max
            || max > MAX_SFO
            || offset > b.len() - data
            || max > b.len() - data - offset
        {
            return Err(invalid("Invalid or duplicate PARAM.SFO value"));
        }
        let value = &b[data + offset..data + offset + len];
        match format {
            0x0204 => {
                if value.last() != Some(&0) {
                    return Err(invalid("Unterminated PARAM.SFO string"));
                }
                if matches!(
                    key,
                    "TITLE" | "TITLE_ID" | "CATEGORY" | "APP_VER" | "PS3_SYSTEM_VER"
                ) {
                    let text = std::str::from_utf8(&value[..value.len() - 1])
                        .map_err(|_| invalid("Invalid PARAM.SFO text encoding"))?;
                    if text.len() > 1024 || text.chars().any(char::is_control) {
                        return Err(invalid("Invalid PARAM.SFO identity text"));
                    }
                    fields.insert(key, text.to_string());
                }
            }
            0x0404 if len == 4 => {}
            0x0004 => {}
            _ => return Err(invalid("Unsupported PARAM.SFO value format")),
        }
    }
    let required = |key| {
        fields
            .get(key)
            .cloned()
            .filter(|v| !v.is_empty())
            .ok_or_else(|| invalid("PARAM.SFO lacks title, title ID or category"))
    };
    let title_id = required("TITLE_ID")?;
    if title_id.len() != 9
        || !title_id.as_bytes()[..4]
            .iter()
            .all(|v| v.is_ascii_uppercase())
        || !title_id.as_bytes()[4..].iter().all(|v| v.is_ascii_digit())
    {
        return Err(invalid("Invalid title ID in PARAM.SFO"));
    }
    let category = required("CATEGORY")?;
    if category.len() != 2 || !category.bytes().all(|v| v.is_ascii_alphanumeric()) {
        return Err(invalid("Invalid PARAM.SFO category"));
    }
    Ok(Metadata {
        title: required("TITLE")?,
        title_id,
        category,
        app_version: fields.get("APP_VER").filter(|v| !v.is_empty()).cloned(),
        system_version: fields
            .get("PS3_SYSTEM_VER")
            .filter(|v| !v.is_empty())
            .cloned(),
    })
}

fn plain_file(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let mut current = root.to_path_buf();
    for part in relative.components() {
        current.push(part);
        let Ok(meta) = fs::symlink_metadata(&current) else {
            return false;
        };
        if meta.file_type().is_symlink() {
            return false;
        }
    }
    fs::symlink_metadata(path).is_ok_and(|m| m.is_file())
}

fn folder(root: &Path, sfo: &Path) -> Result<Candidate> {
    if !plain_file(root, sfo) {
        return Err(invalid("PARAM.SFO is missing or uses a symbolic link"));
    }
    let mut file = File::open(sfo)?;
    if file.metadata()?.len() > MAX_SFO as u64 {
        return Err(invalid("PARAM.SFO exceeds the 1 MiB limit"));
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_SFO as u64 + 1)
        .read_to_end(&mut bytes)?;
    let metadata = parse_sfo(&bytes)?;
    let content = sfo.parent().unwrap();
    let disc = metadata.category == "DG";
    let kind = match metadata.category.as_str() {
        "DG" => "disc_folder",
        "HG" => "digital_folder",
        "GD" => "game_data",
        _ => "other_content",
    };
    let boot_file_present = plain_file(root, &content.join("USRDIR/EBOOT.BIN"));
    let has_sfb = !disc
        || content.file_name().is_some_and(|n| n == "PS3_GAME")
            && plain_file(root, &content.parent().unwrap().join("PS3_DISC.SFB"));
    let status = if !matches!(kind, "disc_folder" | "digital_folder") {
        "Not identified as a base game"
    } else if metadata.system_version.is_none() {
        "PS3 platform evidence incomplete"
    } else if !boot_file_present || !has_sfb {
        "Game layout incomplete"
    } else {
        "Needs RPCS3 setup and launch validation"
    };
    Ok(Candidate {
        path: content.to_path_buf(),
        kind: kind.into(),
        bytes: None,
        metadata: Some(metadata),
        boot_file_present,
        artwork_present: plain_file(root, &content.join("ICON0.PNG")),
        status: status.into(),
        next_action: if matches!(kind, "disc_folder" | "digital_folder") {
            "Review firmware, boot target and save paths before adding to Games"
        } else {
            "Keep separate from the base game; review content role before installation"
        }
        .into(),
    })
}

fn file_candidate(path: &Path, bytes: u64) -> Option<Candidate> {
    let (kind, status, next) = match ext_of(path).as_str() {
        "zip" | "7z" | "rar" => (
            "archive",
            "Archive contents not inspected in this folder scan",
            "Select this archive with Inspect a file to read its headers",
        ),
        "iso" => (
            "disc_image",
            "Disc identity and encryption are unverified",
            "Inspect the disc filesystem and identify the title before preparation",
        ),
        "pkg" => (
            "package",
            "Package platform and content role are unverified",
            "Validate package metadata; distinguish base game, update and extra content",
        ),
        "pup" => (
            "firmware",
            "Firmware candidate; contents unverified",
            "Validate the firmware source and install through the managed RPCS3 workflow",
        ),
        _ => return None,
    };
    Some(Candidate {
        path: path.to_path_buf(),
        kind: kind.into(),
        bytes: Some(bytes),
        metadata: None,
        boot_file_present: false,
        artwork_present: false,
        status: status.into(),
        next_action: next.into(),
    })
}

pub fn inspect(path: &Path) -> Result<Inspection> {
    if !path.is_absolute() {
        return Err(AppError::new(
            "PATH",
            "Choose an absolute path on the Marquee computer",
        ));
    }
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() || !meta.is_dir() && !meta.is_file() {
        return Err(AppError::new(
            "PATH",
            "Choose a regular file or folder, not a symbolic link or device",
        ));
    }
    let mut report = Inspection {
        source: path.to_path_buf(),
        source_kind: if meta.is_dir() { "folder" } else { "file" }.into(),
        source_bytes: meta.is_file().then_some(meta.len()),
        archive_unpacked_bytes: None,
        inspected_entries: 0,
        truncated: false,
        items: vec![],
        diagnostics: vec![],
        playable: false,
    };
    if meta.is_file() && matches!(ext_of(path).as_str(), "zip" | "7z" | "rar") {
        report.source_kind = "archive".into();
        let entries = list_entries(path, kind_of(path)?)?;
        report.inspected_entries = entries.len();
        report.archive_unpacked_bytes = Some(entries.iter().map(|e| e.size).sum());
        for entry in &entries {
            if report.items.len() == MAX_ITEMS {
                report.truncated = true;
                break;
            }
            if let Some(item) = file_candidate(&entry.rel, entry.size) {
                report.items.push(item);
            }
        }
        for entry in entries.iter().filter(|e| {
            e.rel
                .file_name()
                .is_some_and(|n| n.eq_ignore_ascii_case("PARAM.SFO"))
        }) {
            if report.items.len() == MAX_ITEMS {
                report.truncated = true;
                break;
            }
            report.items.push(Candidate {
                path: entry.rel.clone(),
                kind: "archived_metadata".into(),
                bytes: Some(entry.size),
                metadata: None,
                boot_file_present: false,
                artwork_present: false,
                status: "Archived metadata has not been validated".into(),
                next_action:
                    "Review archive preparation before reading title identity or registering a game"
                        .into(),
            });
        }
        report.note("Archive headers only. Inner filenames are hints; no payload was extracted, hashed or decrypted".into());
    } else if meta.is_file() {
        if path.file_name().is_some_and(|n| n == "PARAM.SFO") {
            report.items.push(folder(path.parent().unwrap(), path)?);
        } else if let Some(item) = file_candidate(path, meta.len()) {
            report.items.push(item);
        } else {
            return Err(AppError::new(
                "PS3_FORMAT",
                "Choose a PS3 game folder, PARAM.SFO, archive, ISO, PKG or firmware file",
            ));
        }
        report.inspected_entries = 1;
    } else {
        let mut stack = vec![(path.to_path_buf(), 0)];
        while let Some((dir, depth)) = stack.pop() {
            if report.inspected_entries >= MAX_ENTRIES || report.items.len() >= MAX_ITEMS {
                report.truncated = true;
                break;
            }
            let mut recognized = false;
            for sfo in [dir.join("PS3_GAME/PARAM.SFO"), dir.join("PARAM.SFO")] {
                if plain_file(path, &sfo) {
                    report.inspected_entries += 1;
                    match folder(path, &sfo) {
                        Ok(item) => report.items.push(item),
                        Err(e) => report.note(format!("{}: {}", sfo.display(), e.message)),
                    }
                    recognized = true;
                    break;
                }
            }
            if recognized {
                continue;
            }
            let entries = fs::read_dir(&dir)?;
            let mut children = vec![];
            for entry in entries {
                report.inspected_entries += 1;
                if report.inspected_entries > MAX_ENTRIES {
                    report.truncated = true;
                    break;
                }
                let entry = entry?;
                let ty = entry.file_type()?;
                if ty.is_symlink() {
                    report.note(format!("Skipped symbolic link: {}", entry.path().display()));
                    continue;
                }
                if ty.is_dir() {
                    if depth < MAX_DEPTH {
                        children.push((entry.path(), depth + 1));
                    } else {
                        report.truncated = true;
                    }
                } else if ty.is_file() {
                    if let Some(item) = file_candidate(&entry.path(), entry.metadata()?.len()) {
                        report.items.push(item);
                        if report.items.len() >= MAX_ITEMS {
                            report.truncated = true;
                            break;
                        }
                    }
                }
            }
            children.sort();
            stack.extend(children.into_iter().rev());
        }
    }
    if report.items.is_empty() {
        report.note("No supported content candidate found. Nothing was added to Games".into());
    }
    if report.truncated {
        report.note("Inspection limit reached; select a smaller folder or inspect remaining inputs separately".into());
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::io::Write;

    fn sfo(fields: &[(&str, &str)]) -> Vec<u8> {
        let mut keys = vec![];
        let mut data = vec![];
        let mut table = vec![];
        for (key, value) in fields {
            table.extend_from_slice(&(keys.len() as u16).to_le_bytes());
            table.extend_from_slice(&0x0204_u16.to_le_bytes());
            let len = value.len() as u32 + 1;
            table.extend_from_slice(&len.to_le_bytes());
            table.extend_from_slice(&len.to_le_bytes());
            table.extend_from_slice(&(data.len() as u32).to_le_bytes());
            keys.extend_from_slice(key.as_bytes());
            keys.push(0);
            data.extend_from_slice(value.as_bytes());
            data.push(0);
        }
        let mut out = b"\0PSF".to_vec();
        out.extend_from_slice(&0x101_u32.to_le_bytes());
        out.extend_from_slice(&(20_u32 + table.len() as u32).to_le_bytes());
        out.extend_from_slice(&(20_u32 + table.len() as u32 + keys.len() as u32).to_le_bytes());
        out.extend_from_slice(&(fields.len() as u32).to_le_bytes());
        out.extend(table);
        out.extend(keys);
        out.extend(data);
        out
    }
    fn metadata(category: &str) -> Vec<u8> {
        sfo(&[
            ("TITLE", "Example PS3 title"),
            ("TITLE_ID", "BCUS98111"),
            ("CATEGORY", category),
            ("APP_VER", "02.00"),
            ("PS3_SYSTEM_VER", "03.5500"),
        ])
    }
    fn tree(root: &Path, category: &str) {
        let content = if category == "DG" {
            root.join("PS3_GAME")
        } else {
            root.to_path_buf()
        };
        fs::create_dir_all(content.join("USRDIR")).unwrap();
        fs::write(content.join("PARAM.SFO"), metadata(category)).unwrap();
        fs::write(content.join("USRDIR/EBOOT.BIN"), "fixture; not executable").unwrap();
        fs::write(content.join("ICON0.PNG"), "fixture; not decoded").unwrap();
        if category == "DG" {
            fs::write(root.join("PS3_DISC.SFB"), "fixture; not parsed").unwrap();
        }
    }

    #[test]
    fn bounded_psf_identity_parser_rejects_bad_offsets_duplicates_and_unicode_ids() {
        let bytes = metadata("DG");
        let parsed = parse_sfo(&bytes).unwrap();
        assert_eq!(parsed.title_id, "BCUS98111");
        assert_eq!(parsed.app_version.as_deref(), Some("02.00"));
        for len in 0..bytes.len() {
            assert!(parse_sfo(&bytes[..len]).is_err(), "truncation at {len}");
        }
        let mut corrupt = bytes.clone();
        corrupt[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse_sfo(&corrupt).is_err());
        let mut corrupt = bytes.clone();
        corrupt[32..36].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse_sfo(&corrupt).is_err());
        assert!(parse_sfo(&vec![0; MAX_SFO + 1]).is_err());
        assert!(parse_sfo(&sfo(&[("TITLE", "a"), ("TITLE", "b")])).is_err());
        for id in ["ééé123", "AB€1234", "ABCD1234", "ABCD123456", "abcd12345"] {
            assert!(parse_sfo(&sfo(&[
                ("TITLE", "Test"),
                ("TITLE_ID", id),
                ("CATEGORY", "DG")
            ]))
            .is_err());
        }
    }

    #[test]
    fn folder_inspection_preserves_sources_and_separates_base_candidates_from_game_data() {
        let t = tempfile::tempdir().unwrap();
        for (name, cat) in [
            ("disc", "DG"),
            ("digital", "HG"),
            ("update", "GD"),
            ("other", "AC"),
        ] {
            tree(&t.path().join(name), cat);
        }
        let file = t.path().join("disc/PS3_GAME/PARAM.SFO");
        let before = Sha256::digest(fs::read(&file).unwrap());
        let report = inspect(t.path()).unwrap();
        assert_eq!(report.items.len(), 4);
        assert!(!report.playable);
        assert_eq!(
            report
                .items
                .iter()
                .filter(|i| i.kind == "game_data")
                .count(),
            1
        );
        let disc = report
            .items
            .iter()
            .find(|i| i.kind == "disc_folder")
            .unwrap();
        assert!(disc.boot_file_present && disc.artwork_present);
        assert_eq!(disc.status, "Needs RPCS3 setup and launch validation");
        assert_eq!(
            report
                .items
                .iter()
                .find(|i| i.kind == "game_data")
                .unwrap()
                .status,
            "Not identified as a base game"
        );
        assert_eq!(Sha256::digest(fs::read(file).unwrap()), before);
        fs::remove_file(t.path().join("disc/PS3_GAME/USRDIR/EBOOT.BIN")).unwrap();
        assert_eq!(
            inspect(&t.path().join("disc")).unwrap().items[0].status,
            "Game layout incomplete"
        );
        assert!(!t.path().join("library.db").exists());
    }

    #[test]
    fn archive_headers_never_infer_identity_or_extract_payloads_and_unsafe_paths_fail() {
        let t = tempfile::tempdir().unwrap();
        let archive = t.path().join("Alleged title.dec.zip");
        let mut zip = zip::ZipWriter::new(File::create(&archive).unwrap());
        zip.start_file(
            "Alleged title.dec.iso",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(b"not an ISO").unwrap();
        zip.start_file("Vimm's Lair.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"not read by discovery").unwrap();
        zip.finish().unwrap();
        let before = Sha256::digest(fs::read(&archive).unwrap());
        let report = inspect(&archive).unwrap();
        assert_eq!(report.inspected_entries, 2);
        assert_eq!(report.items.len(), 1);
        assert_eq!(report.items[0].kind, "disc_image");
        assert!(report.items[0].metadata.is_none());
        assert_eq!(report.archive_unpacked_bytes, Some(31));
        assert_eq!(Sha256::digest(fs::read(&archive).unwrap()), before);
        assert_eq!(fs::read_dir(t.path()).unwrap().count(), 1);
        let collection = inspect(t.path()).unwrap();
        assert_eq!(collection.items[0].kind, "archive");
        let mut zip = zip::ZipWriter::new(File::create(t.path().join("unsafe.zip")).unwrap());
        zip.start_file("../outside.iso", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"x").unwrap();
        zip.finish().unwrap();
        assert!(inspect(&t.path().join("unsafe.zip")).is_err());
    }

    #[test]
    fn linked_metadata_and_devices_are_not_read_and_scan_depth_is_bounded() {
        let t = tempfile::tempdir().unwrap();
        let outside = t.path().join("outside");
        tree(&outside, "HG");
        let root = t.path().join("root");
        fs::create_dir(&root).unwrap();
        std::os::unix::fs::symlink(outside.join("PARAM.SFO"), root.join("PARAM.SFO")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("linked")).unwrap();
        assert!(inspect(&root.join("PARAM.SFO")).is_err());
        assert!(inspect(&root).unwrap().items.is_empty());
        assert!(inspect(Path::new("/dev/null")).is_err());
        assert!(inspect(Path::new("relative.iso")).is_err());
        let deep = root.join("a/b/c/d/e/f/g/h/i/j");
        fs::create_dir_all(&deep).unwrap();
        tree(&deep, "HG");
        let report = inspect(&root).unwrap();
        assert!(report.truncated);
        assert!(report.items.is_empty());
    }
}
