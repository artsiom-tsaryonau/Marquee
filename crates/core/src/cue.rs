//! CUE sheet handling for disc images split into a `.cue` sheet plus a `.bin`
//! track.
//!
//! The emulator is always handed the `.cue` and reads the track itself. Marquee
//! only needs the sheet to find out which track holds the data, so it can read
//! the serial. Parsing here is deliberately minimal: `FILE`, `TRACK`, `INDEX`,
//! and a few no-op directives. It never executes anything and never trusts the
//! filename to be a path outside the sheet's own folder.

use crate::error::{AppError, Result};
use std::path::{Path, PathBuf};

const CUE_MAX_BYTES: u64 = 1024 * 1024;

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
    pub start_frame: Option<u64>,
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
                    start_frame: None,
                });
            }
            "INDEX" => {
                // INDEX 01 mm:ss:ff — only 01 marks the track start.
                let mut parts = rest.split_whitespace();
                let idx = parts.next().unwrap_or_default();
                if idx == "01" {
                    if let Some(track) = tracks.last_mut() {
                        track.start_frame = parts.next().and_then(parse_frames);
                        track.has_index01 = track.start_frame.is_some();
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

/// Resolve a contained relative reference without silently changing its meaning.
/// Parent/absolute paths and Windows drive paths are unsupported.
pub fn track_path(cue_path: &Path, file: &str) -> Option<PathBuf> {
    let name = file.replace('\\', "/");
    let relative = Path::new(&name);
    if name.is_empty()
        || name.contains(':')
        || relative.is_absolute()
        || relative.components().any(|c| {
            !matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
    {
        return None;
    }
    Some(cue_path.parent()?.join(relative))
}

fn parse_frames(text: &str) -> Option<u64> {
    let mut parts = text.split(':');
    let minutes: u64 = parts.next()?.parse().ok()?;
    let seconds: u64 = parts.next()?.parse().ok()?;
    let frames: u64 = parts.next()?.parse().ok()?;
    if seconds >= 60 || frames >= 75 || parts.next().is_some() {
        return None;
    }
    minutes
        .checked_mul(60)?
        .checked_add(seconds)?
        .checked_mul(75)?
        .checked_add(frames)
}

/// Parse and validate every required track, including audio companions.
pub fn manifest(cue_path: &Path) -> Result<CueSheet> {
    use std::io::Read;
    let fail = |message: String| AppError::new("MEDIA_MISSING", message);
    let mut bytes = Vec::new();
    std::fs::File::open(cue_path)?
        .take(CUE_MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > CUE_MAX_BYTES {
        return Err(fail("CUE sheet exceeds 1 MiB".into()));
    }
    let sheet = parse(&String::from_utf8_lossy(&bytes));
    if data_track(&sheet).is_none() {
        return Err(fail("CUE has no data track with a valid INDEX 01".into()));
    }
    let parent = std::fs::canonicalize(cue_path.parent().unwrap_or(Path::new(".")))?;
    for track in &sheet.tracks {
        if !track.mode.eq_ignore_ascii_case("AUDIO")
            && !["MODE1/2048", "MODE1/2352", "MODE2/2352", "MODE2/2336"]
                .iter()
                .any(|mode| track.mode.eq_ignore_ascii_case(mode))
        {
            return Err(fail(format!("Unsupported CUE track mode: {}", track.mode)));
        }
        let candidate = track_path(cue_path, &track.file)
            .ok_or_else(|| fail(format!("Unsupported CUE track path: {}", track.file)))?;
        let canonical = std::fs::canonicalize(&candidate)
            .map_err(|_| fail(format!("Missing CUE track: {}", track.file)))?;
        if !canonical.starts_with(&parent) {
            return Err(fail(format!("CUE track leaves its folder: {}", track.file)));
        }
        let meta = std::fs::metadata(&canonical)?;
        if !meta.is_file() || std::fs::File::open(&canonical).is_err() {
            return Err(fail(format!("Unreadable CUE track: {}", track.file)));
        }
        let frames = track
            .start_frame
            .ok_or_else(|| fail(format!("Invalid INDEX 01 for CUE track {}", track.number)))?;
        let sector = track.sector_bytes().unwrap_or(2352);
        if frames
            .checked_mul(sector)
            .is_none_or(|offset| offset >= meta.len())
        {
            return Err(fail(format!(
                "CUE track {} starts beyond its file",
                track.number
            )));
        }
    }
    Ok(sheet)
}

pub fn resolve_track(cue_path: &Path) -> Option<PathBuf> {
    let sheet = manifest(cue_path).ok()?;
    track_path(cue_path, &data_track(&sheet)?.file)
}

pub fn identify_cue(cue_path: &Path) -> Result<crate::iso::DiscIdentity> {
    let sheet = manifest(cue_path)?;
    let track = data_track(&sheet).unwrap();
    let path = track_path(cue_path, &track.file).unwrap();
    let sector = track.sector_bytes().ok_or_else(|| {
        AppError::new(
            "MEDIA_MISSING",
            format!("Unsupported CUE data mode: {}", track.mode),
        )
    })?;
    let offset = track
        .start_frame
        .unwrap()
        .checked_mul(sector)
        .ok_or_else(|| AppError::new("MEDIA_MISSING", "CUE track offset overflow"))?;
    crate::iso::identify_disc_track(&path, offset, Some(sector))
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
        assert_eq!(track_path(base, "../../../../etc/passwd"), None);
        assert_eq!(track_path(base, "/etc/passwd"), None);
        assert_eq!(track_path(base, r"..\..\windows\system32\evil.bin"), None);
        assert_eq!(
            track_path(base, "tracks/disc.bin"),
            Some(PathBuf::from("/games/ps1/tracks/disc.bin"))
        );
        assert_eq!(track_path(base, ".."), None);
        assert_eq!(track_path(base, ""), None);
    }

    #[test]
    fn bare_unquoted_file_names_are_accepted() {
        let sheet = parse("FILE game.bin BINARY\nTRACK 01 MODE1/2352\nINDEX 01 00:00:00\n");
        assert_eq!(data_track(&sheet).unwrap().file, "game.bin");
    }

    #[test]
    fn non_utf8_sheet_still_resolves_and_oversized_sheet_does_not_hang() {
        let dir = tempfile::tempdir().unwrap();
        let cue = dir.path().join("g.cue");
        std::fs::write(dir.path().join("g.bin"), b"x").unwrap();
        let mut text =
            b"REM caf\xe9\nFILE \"g.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n"
                .to_vec();
        std::fs::write(&cue, &text).unwrap();
        assert_eq!(resolve_track(&cue), Some(dir.path().join("g.bin")));
        text.extend(std::iter::repeat_n(b'\n', 3 * 1024 * 1024));
        std::fs::write(&cue, &text).unwrap();
        assert!(resolve_track(&cue).is_none());
    }
}

#[cfg(test)]
mod correction_tests {
    use super::*;
    #[test]
    fn reads_data_after_audio_at_its_declared_frame_offset() {
        let dir = tempfile::tempdir().unwrap();
        let iso = crate::iso::build_test_iso(&[("SYSTEM.CNF", b"BOOT = cdrom:\\SLUS_005.94;1\n")]);
        let mut raw = vec![0; 150 * 2352];
        for chunk in iso.chunks(2048) {
            let mut sector = vec![0; 2352];
            sector[24..24 + chunk.len()].copy_from_slice(chunk);
            raw.extend(sector);
        }
        std::fs::write(dir.path().join("disc.bin"), raw).unwrap();
        let cue = dir.path().join("disc.cue");
        std::fs::write(&cue, "FILE \"disc.bin\" BINARY\nTRACK 01 AUDIO\nINDEX 01 00:00:00\nTRACK 02 MODE2/2352\nINDEX 01 00:02:00\n").unwrap();
        assert_eq!(
            identify_cue(&cue).unwrap().serial.as_deref(),
            Some("SLUS_005.94")
        );
    }
    #[test]
    fn rejects_invalid_indices_and_symlinks_outside_the_folder() {
        assert_eq!(parse_frames("00:60:00"), None);
        assert_eq!(parse_frames("00:00:75"), None);
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("data.bin"), b"test").unwrap();
        std::os::unix::fs::symlink(outside.path().join("data.bin"), dir.path().join("data.bin"))
            .unwrap();
        let cue = dir.path().join("disc.cue");
        std::fs::write(
            &cue,
            "FILE \"data.bin\" BINARY\nTRACK 01 MODE1/2048\nINDEX 01 00:00:00\n",
        )
        .unwrap();
        assert!(manifest(&cue)
            .unwrap_err()
            .message
            .contains("leaves its folder"));
    }
}
