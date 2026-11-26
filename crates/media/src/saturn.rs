//! Sega Saturn disc recognition. Every Saturn disc starts its data track with a
//! system ID whose first 16 bytes are `SEGA SEGASATURN `. `.cue`, `.iso` and
//! `.chd` are shared with PlayStation, so the header, not the extension, says
//! which console a disc belongs to. Compressed `.chd` cannot be read here.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const MAGIC: &[u8; 16] = b"SEGA SEGASATURN ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaturnDisc {
    /// Product number, e.g. `MK-81088`.
    pub product: String,
    /// Area symbols, e.g. `JTUE`.
    pub areas: String,
    pub title: String,
}

/// The Saturn system ID of a `.cue` (its data track) or `.iso`, if it has one.
pub fn identify(path: &Path) -> Option<SaturnDisc> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)?;
    match ext.as_str() {
        "cue" => {
            let sheet = crate::cue::manifest(path).ok()?;
            let track = crate::cue::data_track(&sheet)?;
            let file = crate::cue::track_path(path, &track.file)?;
            let sector = track.sector_bytes()?;
            let start = track.start_frame?.checked_mul(sector)?;
            let header = match (track.mode.to_ascii_uppercase().as_str(), sector) {
                (_, 2048) => 0,
                ("MODE2/2336", _) => 8,
                ("MODE2/2352", _) => 24,
                _ => 16,
            };
            read_id(&file, start + header)
        }
        // Cooked 2048-byte images start with the ID; raw dumps named .iso do not.
        "iso" => [0, 16, 24]
            .into_iter()
            .find_map(|offset| read_id(path, offset)),
        _ => None,
    }
}

fn read_id(path: &Path, offset: u64) -> Option<SaturnDisc> {
    let mut file = File::open(path).ok()?;
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut id = [0u8; 0xD0];
    file.read_exact(&mut id).ok()?;
    if &id[..16] != MAGIC {
        return None;
    }
    let text = |range: std::ops::Range<usize>| {
        String::from_utf8_lossy(&id[range])
            .chars()
            .filter(|c| !c.is_control())
            .collect::<String>()
            .trim()
            .to_string()
    };
    Some(SaturnDisc {
        product: text(0x20..0x2A),
        areas: text(0x40..0x50),
        title: text(0x60..0xD0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn system_id(product: &str, title: &str) -> Vec<u8> {
        let mut id = vec![b' '; 0x100];
        id[..16].copy_from_slice(MAGIC);
        id[0x20..0x20 + product.len()].copy_from_slice(product.as_bytes());
        id[0x40..0x44].copy_from_slice(b"JTUE");
        id[0x60..0x60 + title.len()].copy_from_slice(title.as_bytes());
        id
    }

    #[test]
    fn cooked_iso_and_raw_cue_tracks_are_recognised_and_playstation_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let iso = dir.path().join("game.iso");
        let mut cooked = system_id("MK-81088", "NIGHTS");
        cooked.resize(4096, 0);
        std::fs::write(&iso, &cooked).unwrap();
        let disc = identify(&iso).unwrap();
        assert_eq!(disc.product, "MK-81088");
        assert_eq!(disc.areas, "JTUE");
        assert_eq!(disc.title, "NIGHTS");

        // MODE1/2352: 12 sync bytes and a 4-byte header before the user data.
        let mut raw = vec![0u8; 16];
        raw.extend(system_id("T-12345H", "RACER"));
        raw.resize(2352 * 2, 0);
        std::fs::write(dir.path().join("game (Track 1).bin"), &raw).unwrap();
        let cue = dir.path().join("game.cue");
        std::fs::write(
            &cue,
            "FILE \"game (Track 1).bin\" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n",
        )
        .unwrap();
        assert_eq!(identify(&cue).unwrap().product, "T-12345H");

        let ps1 = dir.path().join("ps1.iso");
        std::fs::write(&ps1, vec![0u8; 4096]).unwrap();
        assert_eq!(identify(&ps1), None);
    }
}
