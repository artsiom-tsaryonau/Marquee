//! Nintendo 3DS cartridge dumps (NCSD/CCI). Azahar only boots decrypted dumps:
//! the first partition's NCCH header carries a NoCrypto flag when it is.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// `Some(true)` for a decrypted dump, `Some(false)` for an encrypted one, `None`
/// when the file is not a readable NCSD image.
pub fn decrypted(path: &Path) -> Option<bool> {
    let mut file = File::open(path).ok()?;
    let mut ncsd = [0u8; 0x200];
    file.read_exact(&mut ncsd).ok()?;
    if &ncsd[0x100..0x104] != b"NCSD" {
        return None;
    }
    let units = u32::from_le_bytes(ncsd[0x120..0x124].try_into().ok()?) as u64;
    file.seek(SeekFrom::Start(units.checked_mul(0x200)?)).ok()?;
    let mut ncch = [0u8; 0x200];
    file.read_exact(&mut ncch).ok()?;
    if &ncch[0x100..0x104] != b"NCCH" {
        return None;
    }
    // flags[7] bit 2: NoCrypto.
    Some(ncch[0x188 + 7] & 0x04 != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(flags7: u8) -> Vec<u8> {
        let mut bytes = vec![0u8; 0x800];
        bytes[0x100..0x104].copy_from_slice(b"NCSD");
        bytes[0x120..0x124].copy_from_slice(&2u32.to_le_bytes());
        bytes[0x400 + 0x100..0x400 + 0x104].copy_from_slice(b"NCCH");
        bytes[0x400 + 0x18F] = flags7;
        bytes
    }

    #[test]
    fn the_nocrypto_flag_decides_and_other_files_are_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("plain.3ds");
        std::fs::write(&plain, image(0x04)).unwrap();
        let locked = dir.path().join("locked.3ds");
        std::fs::write(&locked, image(0x00)).unwrap();
        let other = dir.path().join("other.3ds");
        std::fs::write(&other, vec![0u8; 0x800]).unwrap();
        assert_eq!(decrypted(&plain), Some(true));
        assert_eq!(decrypted(&locked), Some(false));
        assert_eq!(decrypted(&other), None);
    }
}
