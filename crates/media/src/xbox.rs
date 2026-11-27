//! Original Xbox discs. xemu boots XISO images (the game partition alone); a
//! redump image carries a 0x18300000-byte video partition in front of it.
use crate::error::{AppError, Result};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

const SECTOR: u64 = 2048;
const MAGIC: &[u8] = b"MICROSOFT*XBOX*MEDIA";
/// Where the game partition starts in a redump image.
pub const REDUMP_GAME_PARTITION: u64 = 0x1830_0000;

fn read_at(file: &mut File, at: u64, bytes: usize, length: u64) -> Option<Vec<u8>> {
    if at.checked_add(bytes as u64)? > length {
        return None;
    }
    file.seek(SeekFrom::Start(at)).ok()?;
    let mut buf = vec![0; bytes];
    file.read_exact(&mut buf).ok()?;
    Some(buf)
}

/// The game partition offset of an original Xbox disc image (0 for XISO,
/// `REDUMP_GAME_PARTITION` for a redump image), or `None` when the image has no
/// XDVDFS volume with `default.xbe` in its root (Xbox 360 discs boot `default.xex`).
pub fn game_partition(path: &Path) -> Result<Option<u64>> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    for partition in [0, REDUMP_GAME_PARTITION] {
        let Some(header) = read_at(&mut file, partition + 32 * SECTOR, SECTOR as usize, length)
        else {
            continue;
        };
        if &header[..20] != MAGIC {
            continue;
        }
        let root_at =
            partition + u64::from(u32::from_le_bytes(header[20..24].try_into().unwrap())) * SECTOR;
        let root_size = u32::from_le_bytes(header[24..28].try_into().unwrap()) as usize;
        if !(14..=4 * 1024 * 1024).contains(&root_size) {
            continue;
        }
        let Some(root) = read_at(&mut file, root_at, root_size, length) else {
            continue;
        };
        if root_has(&root, b"default.xbe") {
            return Ok(Some(partition));
        }
    }
    Ok(None)
}

/// Walk an XDVDFS directory tree (bounded) for a file name, case-insensitively.
fn root_has(root: &[u8], wanted: &[u8]) -> bool {
    let mut pending = vec![0usize];
    let mut seen = BTreeSet::new();
    while let Some(offset) = pending.pop() {
        if seen.len() >= 65_536 || !seen.insert(offset) || offset + 14 > root.len() {
            return false;
        }
        let name_size = root[offset + 13] as usize;
        if name_size == 0 || offset + 14 + name_size > root.len() {
            return false;
        }
        if root[offset + 14..offset + 14 + name_size].eq_ignore_ascii_case(wanted) {
            return true;
        }
        for child in [0, 2] {
            let ordinal =
                u16::from_le_bytes(root[offset + child..offset + child + 2].try_into().unwrap());
            if ordinal != 0 && ordinal != 0xffff {
                pending.push(usize::from(ordinal) * 4);
            }
        }
    }
    false
}

/// Write the game partition of `src`, from `offset` to the end, as a new XISO.
pub fn write_xiso(src: &Path, offset: u64, dst: &Path) -> Result<()> {
    let mut input = File::open(src)?;
    let length = input.metadata()?.len();
    if offset >= length {
        return Err(AppError::new(
            "INTAKE_FORMAT",
            "Xbox image is shorter than its partition offset",
        ));
    }
    input.seek(SeekFrom::Start(offset))?;
    let mut output = File::options().write(true).create_new(true).open(dst)?;
    let copied = std::io::copy(&mut input.take(length - offset), &mut output)?;
    if copied != length - offset {
        return Err(AppError::new(
            "INTAKE_FORMAT",
            "Xbox image changed while it was trimmed",
        ));
    }
    output.flush()?;
    output.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal XDVDFS volume: descriptor at sector 32, root table at sector 33.
    fn disc(prefix: u64, name: &[u8]) -> Vec<u8> {
        let mut image = vec![0u8; (prefix + 34 * SECTOR) as usize];
        let at = (prefix + 32 * SECTOR) as usize;
        image[at..at + 20].copy_from_slice(MAGIC);
        image[at + 20..at + 24].copy_from_slice(&33u32.to_le_bytes());
        image[at + 24..at + 28].copy_from_slice(&(SECTOR as u32).to_le_bytes());
        let entry = (prefix + 33 * SECTOR) as usize;
        image[entry + 13] = name.len() as u8;
        image[entry + 14..entry + 14 + name.len()].copy_from_slice(name);
        image
    }

    #[test]
    fn finds_xiso_and_redump_layouts_and_trims_to_xiso() {
        let t = tempfile::tempdir().unwrap();
        let xiso = t.path().join("x.iso");
        std::fs::write(&xiso, disc(0, b"default.xbe")).unwrap();
        assert_eq!(game_partition(&xiso).unwrap(), Some(0));
        let x360 = t.path().join("360.iso");
        std::fs::write(&x360, disc(0, b"default.xex")).unwrap();
        assert_eq!(game_partition(&x360).unwrap(), None);
        // A redump image is the video partition followed by the same volume
        // (sparse here, so the test does not write 387 MiB).
        let redump = t.path().join("r.iso");
        let volume = disc(0, b"DEFAULT.XBE");
        let file = File::create(&redump).unwrap();
        file.set_len(REDUMP_GAME_PARTITION + volume.len() as u64)
            .unwrap();
        {
            use std::os::unix::fs::FileExt;
            file.write_all_at(&volume, REDUMP_GAME_PARTITION).unwrap();
        }
        assert_eq!(
            game_partition(&redump).unwrap(),
            Some(REDUMP_GAME_PARTITION)
        );
        let out = t.path().join("out.iso");
        write_xiso(&redump, REDUMP_GAME_PARTITION, &out).unwrap();
        assert_eq!(game_partition(&out).unwrap(), Some(0));
        assert_eq!(std::fs::read(&out).unwrap(), disc(0, b"DEFAULT.XBE"));
    }
}
