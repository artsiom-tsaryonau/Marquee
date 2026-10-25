//! Bounded read-only XDVDFS / XEX2 identity inspection. No decryption or execution.
//! Layout checked against Xenia Canary 451f915's disc_image_device.cc / xex2_info.h.
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const SECTOR: u64 = 2048;
const MAGIC: &[u8] = b"MICROSOFT*XBOX*MEDIA";
const OFFSETS: [u64; 5] = [0, 0xfb20, 0x20600, 0x2080000, 0xfd90000];
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Disc {
    pub title_id: String,
    pub media_id: String,
    pub version: u32,
    pub disc_number: u8,
    pub disc_count: u8,
    pub partition_offset: u64,
    pub boot_bytes: u64,
}
fn invalid(message: &str) -> AppError {
    AppError::new("XBOX360_FORMAT", message)
}
fn range(at: u64, bytes: u64, length: u64) -> Result<()> {
    if at.checked_add(bytes).is_none_or(|end| end > length) {
        return Err(invalid(
            "Disc metadata or executable range exceeds the file",
        ));
    }
    Ok(())
}
fn read(file: &mut File, at: u64, bytes: usize, length: u64) -> Result<Vec<u8>> {
    range(at, bytes as u64, length)?;
    file.seek(SeekFrom::Start(at))?;
    let mut b = vec![0; bytes];
    file.read_exact(&mut b)?;
    Ok(b)
}
fn le32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(b[at..at + 4].try_into().unwrap())
}
/// Detect the shared original-Xbox / Xbox 360 volume marker without guessing
/// which console it belongs to. Keeps legacy ISO intake out of PlayStation paths.
pub fn has_partition(path: &Path) -> Result<bool> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    for partition in OFFSETS {
        let at = partition + 32 * SECTOR;
        if range(at, 20, length).is_ok() && read(&mut file, at, 20, length)? == MAGIC {
            return Ok(true);
        }
    }
    Ok(false)
}
pub fn inspect(path: &Path) -> Result<Disc> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(invalid("Choose a regular Xbox 360 ISO"));
    }
    let length = meta.len();
    let mut file = File::open(path)?;
    for partition in OFFSETS {
        let at = partition + 32 * SECTOR;
        if range(at, SECTOR, length).is_err() {
            continue;
        }
        let header = read(&mut file, at, SECTOR as usize, length)?;
        if &header[..20] != MAGIC {
            continue;
        }
        if &header[0x7ec..] != MAGIC {
            return Err(invalid("Incomplete XDVDFS volume descriptor"));
        }
        let root_at = partition + u64::from(le32(&header, 20)) * SECTOR;
        let root_size = le32(&header, 24) as usize;
        if !(14..=32 * 1024 * 1024).contains(&root_size) {
            return Err(invalid("Invalid XDVDFS root directory size"));
        }
        let root = read(&mut file, root_at, root_size, length)?;
        let mut pending = vec![0usize];
        let mut seen = BTreeSet::new();
        let mut boot = None;
        while let Some(offset) = pending.pop() {
            if seen.len() >= 65_536 || !seen.insert(offset) {
                return Err(invalid("Cyclic or excessive XDVDFS directory links"));
            }
            range(offset as u64, 14, root.len() as u64)?;
            let name_size = root[offset + 13] as usize;
            range(offset as u64 + 14, name_size as u64, root.len() as u64)?;
            if name_size == 0 {
                return Err(invalid("Empty XDVDFS directory entry"));
            }
            let entry = &root[offset..offset + 14 + name_size];
            for child in [0, 2] {
                let ordinal = u16::from_le_bytes(entry[child..child + 2].try_into().unwrap());
                if ordinal != 0 {
                    pending.push(usize::from(ordinal) * 4);
                }
            }
            let name = &entry[14..];
            if name.iter().any(|b| matches!(b, 0 | b'/' | b'\\')) {
                return Err(invalid("Invalid XDVDFS entry name"));
            }
            let data_at = partition + u64::from(le32(entry, 4)) * SECTOR;
            let size = u64::from(le32(entry, 8));
            range(data_at, size, length)?;
            if name.eq_ignore_ascii_case(b"default.xex")
                && (entry[12] & 0x10 != 0 || boot.replace((data_at, size)).is_some())
            {
                return Err(invalid("Ambiguous or invalid default.xex"));
            }
        }
        let (boot_at, boot_bytes) = boot.ok_or_else(|| invalid("No Xbox 360 default.xex in the disc root; original Xbox XBE discs are not supported by Xenia"))?;
        let header = read(&mut file, boot_at, 24, length)?;
        if boot_bytes < 24 || &header[..4] != b"XEX2" {
            return Err(invalid("Disc boot file is not an XEX2 executable"));
        }
        let count = u64::from(be32(&header, 20));
        let pe_at = u64::from(be32(&header, 8));
        let table_size = count * 8;
        if count == 0 || count > 4096 || pe_at > boot_bytes {
            return Err(invalid("Invalid XEX2 optional header bounds"));
        }
        range(24, table_size, pe_at)?;
        let table = read(&mut file, boot_at + 24, table_size as usize, length)?;
        let mut execution = None;
        for item in table.chunks_exact(8) {
            if be32(item, 0) == 0x0004_0006 {
                let at = u64::from(be32(item, 4));
                if execution.replace(at).is_some() || at < 24 + table_size {
                    return Err(invalid("Ambiguous XEX2 execution information"));
                }
                range(at, 24, pe_at)?;
            }
        }
        let at = execution.ok_or_else(|| invalid("XEX2 execution information is missing"))?;
        let info = read(&mut file, boot_at + at, 24, length)?;
        let title_id = be32(&info, 12);
        if matches!(title_id, 0 | u32::MAX) {
            return Err(invalid("Invalid Xbox 360 title ID"));
        }
        return Ok(Disc {
            title_id: format!("{title_id:08X}"),
            media_id: format!("{:08X}", be32(&info, 0)),
            version: be32(&info, 4),
            disc_number: info[18],
            disc_count: info[19],
            partition_offset: partition,
            boot_bytes,
        });
    }
    Err(invalid("No supported XDVDFS game partition found"))
}

/// Synthetic metadata only; contains no executable program or retail game bytes.
#[cfg(any(test, feature = "test-fixtures"))]
pub fn test_disc() -> Vec<u8> {
    let mut b = vec![0; 100 * 2048];
    let volume = 32 * 2048;
    b[volume..volume + 20].copy_from_slice(MAGIC);
    b[volume + 0x7ec..volume + 2048].copy_from_slice(MAGIC);
    b[volume + 20..volume + 24].copy_from_slice(&40u32.to_le_bytes());
    b[volume + 24..volume + 28].copy_from_slice(&128u32.to_le_bytes());
    let root = 40 * 2048;
    b[root + 4..root + 8].copy_from_slice(&48u32.to_le_bytes());
    b[root + 8..root + 12].copy_from_slice(&512u32.to_le_bytes());
    b[root + 13] = 11;
    b[root + 14..root + 25].copy_from_slice(b"default.xex");
    let boot = 48 * 2048;
    b[boot..boot + 4].copy_from_slice(b"XEX2");
    b[boot + 8..boot + 12].copy_from_slice(&256u32.to_be_bytes());
    b[boot + 20..boot + 24].copy_from_slice(&1u32.to_be_bytes());
    b[boot + 24..boot + 28].copy_from_slice(&0x0004_0006u32.to_be_bytes());
    b[boot + 28..boot + 32].copy_from_slice(&64u32.to_be_bytes());
    b[boot + 64..boot + 68].copy_from_slice(&0x12345678u32.to_be_bytes());
    b[boot + 68..boot + 72].copy_from_slice(&1u32.to_be_bytes());
    b[boot + 76..boot + 80].copy_from_slice(&0x53450841u32.to_be_bytes());
    b[boot + 82] = 1;
    b[boot + 83] = 1;
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn reads_plain_and_partitioned_discs_without_game_execution() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("untrusted name.iso");
        let b = test_disc();
        for partition in [0, 0xfd90000] {
            let mut file = File::create(&path).unwrap();
            file.set_len(partition + b.len() as u64).unwrap();
            file.seek(SeekFrom::Start(partition)).unwrap();
            file.write_all(&b).unwrap();
            let d = inspect(&path).unwrap();
            assert_eq!(d.title_id, "53450841");
            assert_eq!(d.media_id, "12345678");
            assert_eq!(d.partition_offset, partition);
            assert_eq!(d.disc_number, 1);
            assert!(has_partition(&path).unwrap());
            assert!(
                crate::iso::identify_disc_iso(&path).is_err(),
                "must not be classified as PlayStation"
            );
        }
        std::os::unix::fs::symlink(&path, temp.path().join("link.iso")).unwrap();
        assert!(inspect(&temp.path().join("link.iso")).is_err());
    }
    #[test]
    fn rejects_wrong_console_and_damaged_directory_or_executable_bounds() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("Vanquish.iso");
        let root = 40 * 2048;
        let boot = 48 * 2048;
        for (at, bytes) in [
            (32 * 2048, b"NOT A DISC".to_vec()),
            (32 * 2048 + 0x7ec, vec![0; 20]),
            (32 * 2048 + 20, u32::MAX.to_le_bytes().to_vec()),
            (root + 2, 40u16.to_le_bytes().to_vec()), // link to malformed node
            (root + 4, u32::MAX.to_le_bytes().to_vec()),
            (root + 13, vec![255]),
            (root + 14, b"default.xbe".to_vec()),
            (boot, b"XBEH".to_vec()),
            (boot + 20, 4097u32.to_be_bytes().to_vec()),
            (boot + 28, 255u32.to_be_bytes().to_vec()),
            (boot + 76, 0u32.to_be_bytes().to_vec()),
        ] {
            let mut b = test_disc();
            b[at..at + bytes.len()].copy_from_slice(&bytes);
            fs::write(&path, b).unwrap();
            assert!(inspect(&path).is_err(), "bad field at {at}");
        }
        fs::write(&path, b"XEX2").unwrap();
        assert!(inspect(&path).is_err());
    }
    #[test]
    fn directory_cycles_and_duplicate_execution_headers_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("game.iso");
        let mut b = test_disc();
        let root = 40 * 2048;
        b[root + 2..root + 4].copy_from_slice(&16u16.to_le_bytes());
        let node = b[root..root + 25].to_vec();
        b[root + 64..root + 89].copy_from_slice(&node);
        fs::write(&path, b).unwrap();
        assert!(inspect(&path).is_err());
        let mut b = test_disc();
        let boot = 48 * 2048;
        b[boot + 20..boot + 24].copy_from_slice(&2u32.to_be_bytes());
        let header = b[boot + 24..boot + 32].to_vec();
        b[boot + 32..boot + 40].copy_from_slice(&header);
        fs::write(&path, b).unwrap();
        assert!(inspect(&path).is_err());
    }
}
