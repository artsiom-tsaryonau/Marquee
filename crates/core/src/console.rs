//! Read-only console package / archive inspection. No installation or launch authority.
use crate::{
    error::{AppError, Result},
    ps3,
};
use marquee_media::archive::{self, ConsoleInventory};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize)]
pub struct Inspection {
    pub source: PathBuf,
    pub system: String,
    pub kind: String,
    pub bytes: u64,
    pub content_id: Option<String>,
    pub content_type: Option<u32>,
    pub content_flags: Option<u32>,
    pub metadata: Option<ps3::Metadata>,
    pub role: String,
    pub archive: Option<ConsoleInventory>,
    pub diagnostics: Vec<String>,
    pub playable: bool,
}
fn invalid(message: &str) -> AppError {
    AppError::new("CONSOLE_FORMAT", message)
}
fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(b[at..at + 4].try_into().unwrap())
}
fn be64(b: &[u8], at: usize) -> u64 {
    u64::from_be_bytes(b[at..at + 8].try_into().unwrap())
}
fn range(offset: u64, bytes: u64, length: u64) -> Result<()> {
    if offset.checked_add(bytes).is_none_or(|end| end > length) {
        return Err(invalid("Package table or payload range exceeds the file"));
    }
    Ok(())
}
/// Layout cross-checked against pinned LibOrbisPkg PkgReader and ShadPKG PKGHeader.
/// Only public header/PARAM.SFO identity is read. No key derivation or decryption.
fn inspect_pkg(path: &Path, mut file: File, bytes: u64) -> Result<Inspection> {
    let mut header = [0; 0x440];
    file.read_exact(&mut header)?;
    if &header[..4] != b"\x7fCNT" {
        return Err(invalid("Not a PS4 CNT package"));
    }
    if be64(&header, 0x430) != 0 && be64(&header, 0x430) != bytes {
        return Err(invalid("Package length does not match its header"));
    }
    let count = be32(&header, 0x10) as usize;
    let table = be32(&header, 0x18) as u64;
    if count == 0 || count > 4096 || table < 0x440 {
        return Err(invalid("Invalid package metadata table"));
    }
    range(table, (count as u64) * 32, bytes)?;
    range(be64(&header, 0x20), be64(&header, 0x28), bytes)?;
    range(be64(&header, 0x410), be64(&header, 0x418), bytes)?;
    let raw = &header[0x40..0x64];
    let raw = raw.split(|c| *c == 0).next().unwrap_or(raw);
    if raw.len() != 36
        || raw
            .iter()
            .any(|c| !c.is_ascii_alphanumeric() && !b"-_".contains(c))
        || &raw[7..11] != b"CUSA"
        || !raw[11..16].iter().all(u8::is_ascii_digit)
    {
        return Err(invalid("Invalid PS4 package content ID"));
    }
    let content_id = std::str::from_utf8(raw)
        .map_err(|_| invalid("Invalid content ID"))?
        .to_string();
    file.seek(SeekFrom::Start(table))?;
    let mut entries = vec![0; count * 32];
    file.read_exact(&mut entries)?;
    let mut sfo = None;
    let mut ids = BTreeSet::new();
    for entry in entries.chunks_exact(32) {
        let id = be32(entry, 0);
        if !ids.insert(id) {
            return Err(invalid("Duplicate package metadata entry"));
        }
        let offset = be32(entry, 16) as u64;
        let size = be32(entry, 20) as u64;
        range(offset, size, bytes)?;
        if id == 0x1000 {
            if size == 0 || size > 1024 * 1024 {
                return Err(invalid("Package PARAM.SFO exceeds its limit"));
            }
            file.seek(SeekFrom::Start(offset))?;
            let mut raw = vec![0; size as usize];
            file.read_exact(&mut raw)?;
            if raw.starts_with(b"\0PSF") {
                sfo = Some(ps3::parse_sfo(&raw).map_err(|e| invalid(&e.message))?);
            }
        }
    }
    if sfo
        .as_ref()
        .is_some_and(|m| m.title_id != content_id[7..16])
    {
        return Err(invalid("Package and PARAM.SFO title IDs disagree"));
    }
    let role = match sfo.as_ref().map(|m| m.category.as_str()) {
        Some("gd") => "base",
        Some("gp") => "update",
        Some("ac") => "dlc",
        _ => "unknown",
    };
    let mut diagnostics=vec!["Metadata inspection only. Package decryption/extraction, prepared outputs and title compatibility have not been validated.".into()];
    if be64(&header, 0x430) == 0 {
        diagnostics.push("Package header does not declare a complete package size; this can occur for metadata-only content. No installation readiness is implied.".into());
    }
    if sfo.is_none() {
        diagnostics.push("No readable PARAM.SFO identity. Encrypted or absent metadata is not guessed from the filename.".into());
    }
    Ok(Inspection {
        source: path.into(),
        system: "ps4".into(),
        kind: "package".into(),
        bytes,
        content_id: Some(content_id),
        content_type: Some(be32(&header, 0x74)),
        content_flags: Some(be32(&header, 0x78)),
        metadata: sfo,
        role: role.into(),
        archive: None,
        diagnostics,
        playable: false,
    })
}
pub fn inspect(path: &Path, system: &str) -> Result<Inspection> {
    if !matches!(system, "ps4" | "ps5" | "xbox" | "xbox360") {
        return Err(invalid("Choose PS4, PS5, Xbox or Xbox 360 explicitly"));
    }
    if !path.is_absolute() {
        return Err(invalid("Choose an absolute path on the Marquee computer"));
    }
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(invalid("Choose a regular package or RAR file"));
    }
    let file = File::open(path)?;
    match (
        system,
        path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
    ) {
        ("ps4", "pkg") => inspect_pkg(path, file, meta.len()),
        (_, "rar") => {
            let inventory = archive::console_rar_inventory(path)?;
            let diagnostic = if inventory.truncated {
                "Archive inventory reached a time, entry or candidate limit. Partial evidence cannot be used as an installation plan."
            } else {
                "Header inventory only. Internal metadata, executable format and compatibility remain unchecked; no files were extracted."
            };
            Ok(Inspection {
                source: path.into(),
                system: system.into(),
                kind: "archive".into(),
                bytes: meta.len(),
                content_id: None,
                content_type: None,
                content_flags: None,
                metadata: None,
                role: "unknown".into(),
                archive: Some(inventory),
                diagnostics: vec![diagnostic.into()],
                playable: false,
            })
        }
        _ => Err(invalid(
            "This inspector supports PS4 PKG and PS4/PS5/Xbox/Xbox 360 RAR inputs",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sfo(category: &str) -> Vec<u8> {
        let fields = [
            ("TITLE", "Synthetic game"),
            ("TITLE_ID", "CUSA08809"),
            ("CATEGORY", category),
            ("APP_VER", "01.00"),
        ];
        let mut keys = vec![];
        let mut data = vec![];
        let mut entries = vec![];
        for (key, value) in fields {
            entries.extend_from_slice(&(keys.len() as u16).to_le_bytes());
            entries.extend_from_slice(&0x204u16.to_le_bytes());
            let len = value.len() as u32 + 1;
            entries.extend_from_slice(&len.to_le_bytes());
            entries.extend_from_slice(&len.to_le_bytes());
            entries.extend_from_slice(&(data.len() as u32).to_le_bytes());
            keys.extend_from_slice(key.as_bytes());
            keys.push(0);
            data.extend_from_slice(value.as_bytes());
            data.push(0);
        }
        let mut bytes = b"\0PSF".to_vec();
        bytes.extend_from_slice(&0x101u32.to_le_bytes());
        bytes.extend_from_slice(&(20u32 + entries.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(20u32 + entries.len() as u32 + keys.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend(entries);
        bytes.extend(keys);
        bytes.extend(data);
        bytes
    }
    fn package(category: &str) -> Vec<u8> {
        let mut b = vec![0; 0x4000];
        b[..4].copy_from_slice(b"\x7fCNT");
        b[0x10..0x14].copy_from_slice(&1u32.to_be_bytes());
        b[0x18..0x1c].copy_from_slice(&0x1000u32.to_be_bytes());
        b[0x40..0x64].copy_from_slice(b"EP9000-CUSA08809_00-SOTC0000000000EU");
        b[0x430..0x438].copy_from_slice(&0x4000u64.to_be_bytes());
        let meta = sfo(category);
        b[0x1000..0x1004].copy_from_slice(&0x1000u32.to_be_bytes());
        b[0x1010..0x1014].copy_from_slice(&0x2000u32.to_be_bytes());
        b[0x1014..0x1018].copy_from_slice(&(meta.len() as u32).to_be_bytes());
        b[0x2000..0x2000 + meta.len()].copy_from_slice(&meta);
        b
    }
    #[test]
    fn roles_come_from_metadata_and_no_inspection_claims_playability() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("misleading-update-filename.pkg");
        for (category, role) in [("gd", "base"), ("gp", "update"), ("ac", "dlc")] {
            fs::write(&path, package(category)).unwrap();
            let report = inspect(&path, "ps4").unwrap();
            assert_eq!(report.role, role);
            assert_eq!(report.metadata.unwrap().title_id, "CUSA08809");
            assert!(!report.playable);
        }
        assert!(inspect(&path, "ps5").is_err());
        assert!(inspect(Path::new("relative.pkg"), "ps4").is_err());
        // Xbox selection must not reinterpret a PlayStation package as installable.
        assert!(inspect(&path, "xbox").is_err());
        assert!(inspect(&path, "xbox360").is_err());
    }
    #[test]
    fn malformed_package_tables_and_disagreeing_identity_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("game.pkg");
        let base = package("gd");
        for (offset, changed) in [
            (0x430, vec![0; 8]),
            (0x10, u32::MAX.to_be_bytes().to_vec()),
            (0x18, u32::MAX.to_be_bytes().to_vec()),
            (0x1010, u32::MAX.to_be_bytes().to_vec()),
            (0x1014, u32::MAX.to_be_bytes().to_vec()),
            (0x47, b"CUSA99999".to_vec()),
        ] {
            let mut b = base.clone();
            b[offset..offset + changed.len()].copy_from_slice(&changed);
            if offset == 0x430 {
                b[0x430..0x438].copy_from_slice(&0x4001u64.to_be_bytes());
            }
            fs::write(&path, b).unwrap();
            assert!(inspect(&path, "ps4").is_err(), "offset {offset:x}");
        }
        let mut duplicate = base.clone();
        duplicate[0x10..0x14].copy_from_slice(&2u32.to_be_bytes());
        duplicate.copy_within(0x1000..0x1020, 0x1020);
        fs::write(&path, duplicate).unwrap();
        assert!(inspect(&path, "ps4").is_err());
        std::os::unix::fs::symlink(&path, dir.path().join("link.pkg")).unwrap();
        assert!(inspect(&dir.path().join("link.pkg"), "ps4").is_err());
    }
    #[test]
    fn missing_size_or_encrypted_metadata_remain_unverified_instead_of_filename_guesses() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("base.pkg");
        let mut b = package("ac");
        b[0x430..0x438].fill(0);
        fs::write(&path, &b).unwrap();
        let report = inspect(&path, "ps4").unwrap();
        assert_eq!(report.role, "dlc");
        assert!(!report.playable);
        b[0x2000..0x2004].fill(0xff);
        fs::write(&path, b).unwrap();
        let report = inspect(&path, "ps4").unwrap();
        assert_eq!(report.role, "unknown");
        assert!(report.metadata.is_none());
    }
    #[test]
    fn archive_inventory_uses_headers_and_refuses_redirections_and_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("game.rar");
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rar");
        fs::copy(fixture.join("regular5.rar"), &path).unwrap();
        for system in ["ps5", "xbox", "xbox360"] {
            let report = inspect(&path, system).unwrap();
            assert_eq!(report.system, system);
            assert!(!report.playable);
            let archive = report.archive.unwrap();
            assert_eq!(archive.files, 1);
            assert!(!archive.truncated);
        }
        assert!(!dir.path().join("Game.nds").exists());
        for name in ["symlink5.rar", "traversal5.rar", "hardlink5.rar"] {
            fs::copy(fixture.join(name), &path).unwrap();
            assert!(inspect(&path, "ps5").is_err());
        }
    }
}
