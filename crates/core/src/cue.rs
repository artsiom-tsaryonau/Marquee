//! CUE sheet handling for disc images split into a `.cue` sheet plus a `.bin`
//! track.
//!
//! The emulator is always handed the `.cue` and reads the track itself. Marquee
//! only needs the sheet to find out which track holds the data, so it can read
//! the serial. Parsing here is deliberately minimal: `FILE`, `TRACK`, `INDEX`,
//! and a few no-op directives. It never executes anything and never trusts the
//! filename to be a path outside the sheet's own folder.

use crate::error::Result;
use std::path::{Path, PathBuf};

/// A parsed CUE sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CueSheet {
    /// Tracks in sheet order.
    pub tracks: Vec<CueTrack>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CueTrack {
    pub number: u32,
    /// `MODE1/2352`, `MODE2/2352`, `AUDIO`, …
    pub mode: String,
    /// The `FILE` this track lives in, as written in the sheet.
    pub file: String,
    /// True when `INDEX 01` is present, i.e. the track has a real start.
    pub has_index01: bool,
}

impl CueTrack {
    /// Data tracks carry the ISO9660 filesystem; audio tracks do not.
    pub fn is_data(&self) -> bool {
        let mode = self.mode.to_ascii_uppercase();
        !mode.starts_with("AUDIO") && self.has_index01
    }

    /// Real CD sector size implied by the mode suffix (`MODE2/2352` → 2352).
    pub fn sector_bytes(&self) -> Option<u64> {
        self.mode
            .rsplit('/')
            .next()
            .and_then(|s| s.trim().parse::<u64>().ok())
    }
}

/// Parse the subset of CUE syntax that identifies data tracks.
pub fn parse(text: &str) -> CueSheet {
    let mut tracks: Vec<CueTrack> = Vec::new();
    let mut current_file = String::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        let (keyword, rest) = match line.split_once(char::is_whitespace) {
            Some((k, r)) => (k.to_ascii_uppercase(), r.trim()),
            None => (line.to_ascii_uppercase(), ""),
        };
        match keyword.as_str() {
            "FILE" => {
                // FILE "name.bin" BINARY — only the name matters, not the encoding.
                current_file = file_field(rest);
            }
            "TRACK" => {
                let mut parts = rest.split_whitespace();
                let number = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0);
                let mode = parts.next().unwrap_or_default().to_string();
                tracks.push(CueTrack {
                    number,
                    mode,
                    file: current_file.clone(),
                    has_index01: false,
                });
            }
            "INDEX" => {
                // INDEX 01 mm:ss:ff — only 01 marks the track start.
                let mut parts = rest.split_whitespace();
                let idx = parts.next().unwrap_or_default();
                if idx == "01" {
                    if let Some(track) = tracks.last_mut() {
                        track.has_index01 = true;
                    }
                }
            }
            // REM / PERFORMER / TITLE / SONGWRITER / PREGAP / POSTGAP / CATALOG / ISRC / FLAGS
            _ => {}
        }
    }

    CueSheet { tracks }
}

/// The filename field of a `FILE` line, dropping the trailing encoding.
///
/// `FILE "game.bin" BINARY` → `game.bin`; `FILE game.bin BINARY` → `game.bin`.
/// A quoted name may contain spaces, so the closing quote is what ends it.
fn file_field(rest: &str) -> String {
    let v = rest.trim();
    if let Some(inner) = v.strip_prefix('"') {
        return inner
            .split_once('"')
            .map(|(name, _)| name.to_string())
            .unwrap_or_else(|| inner.to_string());
    }
    v.split_whitespace().next().unwrap_or_default().to_string()
}

/// The first data track, which is the one that boots.
pub fn data_track(sheet: &CueSheet) -> Option<&CueTrack> {
    sheet.tracks.iter().find(|t| t.is_data())
}

/// The data track's `.bin` resolved beside `cue_path`.
///
/// Only an ordinary filename is accepted: the returned path is always the
/// sheet's own directory joined with a plain file name, so a crafted `FILE`
/// line cannot point the reader at somewhere else on disk.
pub fn track_path(cue_path: &Path, file: &str) -> Option<PathBuf> {
    let name = file.replace('\\', "/");
    let name = name.rsplit('/').next().unwrap_or(&name);
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    let dir = cue_path.parent()?;
    Some(dir.join(name))
}

/// Resolve the `.bin` a sheet boots from, if it exists on disk.
pub fn resolve_track(cue_path: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(cue_path).ok()?;
    let sheet = parse(&text);
    let track = data_track(&sheet)?;
    let candidate = track_path(cue_path, &track.file)?;
    candidate.is_file().then_some(candidate)
}

/// Read a disc serial from a `.cue` by locating its data track.
///
/// Returns the disc identity of the track, or an unresolved identity when the
/// sheet has no readable track.
pub fn identify_cue(cue_path: &Path) -> Result<crate::iso::DiscIdentity> {
    let Some(track) = resolve_track(cue_path) else {
        return Ok(crate::iso::unresolved());
    };
    crate::iso::identify_disc_iso(&track)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MGS_DISC_1: &str = "FILE \"Metal Gear Solid (USA) (Disc 1) (Rev 1).bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n";

    #[test]
    fn parses_the_real_metal_gear_sheet() {
        let sheet = parse(MGS_DISC_1);
        assert_eq!(sheet.tracks.len(), 1);
        let track = data_track(&sheet).unwrap();
        assert_eq!(track.number, 1);
        assert_eq!(track.mode, "MODE2/2352");
        assert_eq!(track.file, "Metal Gear Solid (USA) (Disc 1) (Rev 1).bin");
        assert_eq!(track.sector_bytes(), Some(2352));
        assert!(track.is_data());
    }

    #[test]
    fn picks_the_data_track_when_audio_comes_first() {
        let sheet = parse(
            "FILE \"game.bin\" BINARY\n  TRACK 01 AUDIO\n    INDEX 01 00:00:00\n  TRACK 02 MODE1/2352\n    INDEX 01 03:00:00\n",
        );
        let track = data_track(&sheet).unwrap();
        assert_eq!(track.number, 2);
        assert_eq!(track.mode, "MODE1/2352");
    }

    #[test]
    fn ignores_comments_gaps_and_unrelated_directives() {
        let sheet = parse(
            "; a comment\nREM COMMENT \"hi\"\nFILE \"x.bin\" BINARY\n  TRACK 01 MODE2/2352\n  PREGAP 00:02:00\n  INDEX 00 00:00:00\n  INDEX 01 00:02:00\n",
        );
        let track = data_track(&sheet).unwrap();
        assert!(track.has_index01);
    }

    #[test]
    fn index00_alone_is_not_a_start() {
        let sheet = parse("FILE \"x.bin\" BINARY\n  TRACK 01 MODE2/2352\n  INDEX 00 00:00:00\n");
        assert!(data_track(&sheet).is_none());
    }

    #[test]
    fn file_names_cannot_escape_the_sheet_directory() {
        let base = Path::new("/games/ps1/game.cue");
        assert_eq!(
            track_path(base, "disc.bin"),
            Some(PathBuf::from("/games/ps1/disc.bin"))
        );
        // A path in the sheet is reduced to its final component.
        assert_eq!(
            track_path(base, "../../../../etc/passwd"),
            Some(PathBuf::from("/games/ps1/passwd"))
        );
        assert_eq!(
            track_path(base, "/etc/passwd"),
            Some(PathBuf::from("/games/ps1/passwd"))
        );
        assert_eq!(
            track_path(base, r"..\..\windows\system32\evil.bin"),
            Some(PathBuf::from("/games/ps1/evil.bin"))
        );
        assert_eq!(track_path(base, ".."), None);
        assert_eq!(track_path(base, ""), None);
    }

    #[test]
    fn bare_unquoted_file_names_are_accepted() {
        let sheet = parse("FILE game.bin BINARY\nTRACK 01 MODE1/2352\nINDEX 01 00:00:00\n");
        assert_eq!(data_track(&sheet).unwrap().file, "game.bin");
    }
}
