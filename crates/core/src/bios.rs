//! BIOS images kept inside Marquee's own data folder, identified by what is
//! inside them (never by filename), so a game can be paired with a BIOS of its
//! region and the whole set can be deleted in one step.
//!
//! BIOS storage and identification. Acquisition jobs are planned; folder import is the current fallback.

use crate::error::{AppError, Result};
use crate::paths::Paths;
use serde::Serialize;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const PS1_SIZE: u64 = 512 * 1024;
const PS2_SIZE: u64 = 4 * 1024 * 1024;
/// PCSX2 keeps per-BIOS extras (NVRAM, MEC, extra ROM banks) beside the image.
const PS2_SIDECARS: &[&str] = &["nvm", "mec", "erom", "rom1", "rom2"];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BiosFile {
    pub system: String,
    pub file: String,
    /// `NTSC-U`, `PAL`, `NTSC-J`, `Asia` or `China`.
    pub region: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ImportReport {
    pub added: Vec<BiosFile>,
    /// Files that were not a recognisable BIOS (or were already present).
    pub ignored: u32,
}

pub fn dir(paths: &Paths, system: &str) -> Result<PathBuf> {
    match system {
        "ps1" => Ok(paths.duckstation_bios_dir()),
        "ps2" => Ok(paths.pcsx2_bios_dir()),
        other => Err(AppError::new(
            "BAD_REQUEST",
            format!("no BIOS store for system: {other}"),
        )),
    }
}

/// A line such as `Version 2.2 12/04/95 A` near the end of the image; the last
/// letter is the region. Earlier "Version 1.0" strings are other components.
fn identify_ps1(bytes: &[u8]) -> Option<(String, String)> {
    const NEEDLE: &[u8] = b"Version ";
    let shape = |t: &[u8]| {
        t.len() >= 14
            && t[0].is_ascii_digit()
            && t[1] == b'.'
            && t[2].is_ascii_digit()
            && t[3] == b' '
            && t[4..12].iter().enumerate().all(|(i, c)| {
                if i % 3 == 2 {
                    *c == b'/'
                } else {
                    c.is_ascii_digit()
                }
            })
            && t[12] == b' '
    };
    let at = bytes
        .windows(NEEDLE.len())
        .enumerate()
        .filter(|(i, w)| *w == NEEDLE && shape(&bytes[i + NEEDLE.len()..]))
        .map(|(i, _)| i + NEEDLE.len())
        .next_back()?;
    let text = &bytes[at..at + 14];
    let region = match text[13] {
        b'A' => "NTSC-U",
        b'E' => "PAL",
        b'J' => "NTSC-J",
        _ => return None,
    };
    Some((
        region.into(),
        String::from_utf8_lossy(&text[..3]).into_owned(),
    ))
}

/// PS2 images carry a `ROMVER` record such as `0220EC20060905`: four digits of
/// version, a region letter, a type letter and the build date.
fn identify_ps2(bytes: &[u8]) -> Option<(String, String)> {
    if !bytes[..bytes.len().min(0x10000)]
        .windows(6)
        .any(|w| w == b"ROMVER")
    {
        return None;
    }
    bytes.windows(14).find_map(|w| {
        let digits = |s: &[u8]| s.iter().all(u8::is_ascii_digit);
        if !(digits(&w[..4]) && digits(&w[6..]) && w[5].is_ascii_uppercase()) {
            return None;
        }
        let region = match w[4] {
            b'A' => "NTSC-U",
            b'E' => "PAL",
            b'J' => "NTSC-J",
            b'H' => "Asia",
            b'C' => "China",
            _ => return None,
        };
        let major = (w[0] - b'0') * 10 + (w[1] - b'0');
        let minor = (w[2] - b'0') * 10 + (w[3] - b'0');
        Some((region.into(), format!("{major}.{minor:02}")))
    })
}

fn read_image(path: &Path, size: u64) -> Option<Vec<u8>> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() != size {
        return None;
    }
    let mut bytes = Vec::with_capacity(size as usize);
    fs::File::open(path)
        .ok()?
        .take(size)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(bytes)
}

fn identify(path: &Path) -> Option<(&'static str, String, String)> {
    let size = fs::metadata(path).ok()?.len();
    match size {
        PS1_SIZE => {
            let (r, v) = identify_ps1(&read_image(path, size)?)?;
            Some(("ps1", r, v))
        }
        PS2_SIZE => {
            let (r, v) = identify_ps2(&read_image(path, size)?)?;
            Some(("ps2", r, v))
        }
        _ => None,
    }
}

pub fn list(paths: &Paths) -> Result<Vec<BiosFile>> {
    let mut found = Vec::new();
    for system in ["ps1", "ps2"] {
        let folder = dir(paths, system)?;
        let Ok(entries) = fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some((sys, region, version)) = identify(&path) {
                if sys == system {
                    found.push(BiosFile {
                        system: sys.into(),
                        file: entry.file_name().to_string_lossy().into_owned(),
                        region,
                        version,
                    });
                }
            }
        }
    }
    found.sort_by(|a, b| (&a.system, &a.region, &a.file).cmp(&(&b.system, &b.region, &b.file)));
    Ok(found)
}

fn collect_files(root: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_file() {
            out.push(entry.path());
        } else if kind.is_dir() && depth > 0 {
            collect_files(&entry.path(), depth - 1, out);
        }
    }
}

fn safe_name(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    (!name.is_empty() && !name.starts_with('.')).then_some(name)
}

/// Copy every recognisable BIOS found under `source` (a folder or one file)
/// into Marquee's store. Anything unrecognised is left alone and not copied.
pub fn import(paths: &Paths, source: &Path) -> Result<ImportReport> {
    let mut files = Vec::new();
    if source.is_file() {
        files.push(source.to_path_buf());
    } else if source.is_dir() {
        collect_files(source, 3, &mut files);
    } else {
        return Err(AppError::new(
            "ROOT_OFFLINE",
            format!("not found: {}", source.display()),
        ));
    }
    files.sort();
    let mut report = ImportReport::default();
    for path in files {
        let (Some((system, region, version)), Some(name)) = (identify(&path), safe_name(&path))
        else {
            report.ignored += 1;
            continue;
        };
        let target_dir = dir(paths, system)?;
        fs::create_dir_all(&target_dir)?;
        let target = target_dir.join(&name);
        if target.exists() {
            report.ignored += 1;
            continue;
        }
        fs::copy(&path, &target)?;
        if system == "ps2" {
            copy_sidecars(&path, &target_dir);
        }
        report.added.push(BiosFile {
            system: system.into(),
            file: name,
            region,
            version,
        });
    }
    Ok(report)
}

fn copy_sidecars(image: &Path, target_dir: &Path) {
    let (Some(parent), Some(stem)) = (image.parent(), image.file_stem()) else {
        return;
    };
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let same_stem = path.file_stem() == Some(stem);
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if same_stem && path != image && PS2_SIDECARS.contains(&ext.as_str()) {
            if let Some(name) = safe_name(&path) {
                let dest = target_dir.join(name);
                if !dest.exists() {
                    let _ = fs::copy(&path, dest);
                }
            }
        }
    }
}

/// Delete every BIOS Marquee holds for one system. Returns how many files went.
pub fn remove(paths: &Paths, system: &str) -> Result<usize> {
    let folder = dir(paths, system)?;
    let Ok(entries) = fs::read_dir(&folder) else {
        return Ok(0);
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        if entry.file_type().is_ok_and(|k| k.is_file()) && fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

fn accepts(game_region: &str, bios_region: &str) -> bool {
    game_region == bios_region || (game_region == "NTSC-J" && bios_region == "Asia")
}

fn version_key(version: &str) -> (u32, u32) {
    let (a, b) = version.split_once('.').unwrap_or((version, "0"));
    (a.parse().unwrap_or(0), b.parse().unwrap_or(0))
}

/// The BIOS to use for a game of `game_region`: the newest one of that region.
/// With no match (or an unknown game region) there is no preference.
pub fn best_for<'a>(files: &'a [BiosFile], game_region: Option<&str>) -> Option<&'a BiosFile> {
    let region = game_region?;
    files
        .iter()
        .filter(|b| accepts(region, &b.region))
        .max_by(|a, b| {
            version_key(&a.version)
                .cmp(&version_key(&b.version))
                .then_with(|| b.file.cmp(&a.file))
        })
}

/// Point PCSX2 at the BIOS matching the game, before launch. Only edits a
/// settings file PCSX2 has already written itself, because a half-made file
/// makes it reset to defaults. Returns the BIOS chosen, if any.
pub fn select_for_pcsx2(xdg_config: &Path, game_region: Option<&str>) -> Result<Option<String>> {
    let bios_dir = xdg_config.join("PCSX2").join("bios");
    let ini = xdg_config.join("PCSX2").join("inis").join("PCSX2.ini");
    if !bios_dir.is_dir() {
        return Ok(None);
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(&bios_dir)?.flatten() {
        if let Some(("ps2", region, version)) = identify(&entry.path()) {
            files.push(BiosFile {
                system: "ps2".into(),
                file: entry.file_name().to_string_lossy().into_owned(),
                region,
                version,
            });
        }
    }
    let Some(chosen) = best_for(&files, game_region) else {
        return Ok(None);
    };
    // First run: seed the smallest file PCSX2 accepts (it resets files that lack
    // a matching SettingsVersion) so its setup wizard never opens. Marquee owns
    // BIOS and folder setup for this managed copy.
    let text = if ini.is_file() {
        fs::read_to_string(&ini)?
    } else {
        fs::create_dir_all(ini.parent().unwrap_or(xdg_config))?;
        "[UI]\nSettingsVersion = 1\n".to_string()
    };
    let updated = set_ini_value(&text, "Filenames", "BIOS", &chosen.file);
    let updated = set_ini_value(&updated, "UI", "SetupWizardIncomplete", "false");
    if updated != text {
        let mut tmp = tempfile::NamedTempFile::new_in(ini.parent().unwrap_or(xdg_config))?;
        std::io::Write::write_all(&mut tmp, updated.as_bytes())?;
        tmp.persist(&ini).map_err(|e| AppError::from(e.error))?;
    }
    Ok(Some(chosen.file.clone()))
}

/// Make DuckStation's first run silent: it fills every other setting with its
/// own defaults, so Marquee only writes the keys it owns. BIOS is found by
/// DuckStation itself from `duckstation/bios`.
pub fn seed_duckstation(xdg_config: &Path) -> Result<()> {
    let ini = xdg_config.join("duckstation").join("settings.ini");
    let text = if ini.is_file() {
        fs::read_to_string(&ini)?
    } else {
        String::new()
    };
    let updated = set_ini_value(&text, "Main", "SetupWizardIncomplete", "false");
    if updated != text {
        let parent = ini.parent().unwrap_or(xdg_config);
        fs::create_dir_all(parent)?;
        let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
        std::io::Write::write_all(&mut tmp, updated.as_bytes())?;
        tmp.persist(&ini).map_err(|e| AppError::from(e.error))?;
    }
    Ok(())
}

/// Folder Marquee watches for BIOS files or zips: drop them in and the next
/// launch imports whatever is recognised. A saved `bios_inbox` setting adds one more.
pub fn inbox_dirs(paths: &Paths, extra: Option<&str>) -> Vec<std::path::PathBuf> {
    let mut dirs = vec![paths.data_dir.join("bios-inbox")];
    if let Some(extra) = extra.filter(|e| !e.is_empty()) {
        dirs.push(std::path::PathBuf::from(extra));
    }
    dirs
}

/// Import every recognisable BIOS from the inbox folders, opening `.zip` files
/// into a temporary folder first. Never fails a launch: problems are skipped.
pub fn import_inbox(paths: &Paths, extra: Option<&str>) -> ImportReport {
    let mut total = ImportReport::default();
    for dir in inbox_dirs(paths, extra) {
        if !dir.is_dir() {
            continue;
        }
        let mut reports = Vec::new();
        reports.push(import(paths, &dir));
        let mut zips = Vec::new();
        collect_files(&dir, 2, &mut zips);
        for zip_path in zips
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")))
        {
            if let Ok(tmp) = tempfile::tempdir() {
                let ok = fs::File::open(&zip_path)
                    .ok()
                    .and_then(|f| zip::ZipArchive::new(f).ok())
                    .is_some_and(|mut z| z.extract(tmp.path()).is_ok());
                if ok {
                    reports.push(import(paths, tmp.path()));
                }
            }
        }
        for r in reports.into_iter().flatten() {
            total.added.extend(r.added);
            total.ignored += r.ignored;
        }
    }
    total
}

fn set_ini_value(text: &str, section: &str, key: &str, value: &str) -> String {
    let header = format!("[{section}]");
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let line = format!("{key} = {value}");
    let Some(start) = lines.iter().position(|l| l.trim() == header) else {
        if lines.last().is_some_and(|l| !l.is_empty()) {
            lines.push(String::new());
        }
        lines.push(header);
        lines.push(line);
        return lines.join("\n") + "\n";
    };
    let end = lines[start + 1..]
        .iter()
        .position(|l| l.trim_start().starts_with('['))
        .map_or(lines.len(), |n| start + 1 + n);
    let existing = (start + 1..end).find(|&i| {
        lines[i]
            .split_once('=')
            .is_some_and(|(k, _)| k.trim() == key)
    });
    match existing {
        Some(i) => lines[i] = line,
        None => lines.insert(start + 1, line),
    }
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ps1(region: u8) -> Vec<u8> {
        let mut image = vec![0u8; PS1_SIZE as usize];
        let text = format!("Version 1.0 Version 2.2 12/04/95 {}", region as char);
        image[0x1000..0x1000 + text.len()].copy_from_slice(text.as_bytes());
        image
    }

    fn ps2(region: u8, version: &str) -> Vec<u8> {
        let mut image = vec![0u8; PS2_SIZE as usize];
        image[..6].copy_from_slice(b"ROMVER");
        let record = format!("{version}{}C20060905", region as char);
        image[0x4000..0x4000 + record.len()].copy_from_slice(record.as_bytes());
        image
    }

    fn paths(root: &Path) -> Paths {
        Paths::resolve(Some(root.join("data"))).unwrap()
    }

    #[test]
    fn regions_come_from_the_image_not_the_filename() {
        assert_eq!(identify_ps1(&ps1(b'E')).unwrap().0, "PAL");
        assert_eq!(identify_ps1(&ps1(b'A')).unwrap().0, "NTSC-U");
        assert_eq!(identify_ps1(&ps1(b'J')).unwrap().0, "NTSC-J");
        assert!(identify_ps1(&ps1(b'X')).is_none());
        let (region, version) = identify_ps2(&ps2(b'E', "0220")).unwrap();
        assert_eq!((region.as_str(), version.as_str()), ("PAL", "2.20"));
        assert_eq!(identify_ps2(&ps2(b'H', "0100")).unwrap().0, "Asia");
        assert!(identify_ps2(&vec![0u8; PS2_SIZE as usize]).is_none());
    }

    #[test]
    fn import_keeps_only_real_bios_and_sidecars_then_remove_clears_them() {
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("pack/nested");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("misnamed.dat"), ps2(b'A', "0160")).unwrap();
        fs::write(src.join("misnamed.NVM"), [1u8; 1024]).unwrap();
        fs::write(src.join("eu.bin"), ps1(b'E')).unwrap();
        fs::write(src.join("readme.txt"), "hello").unwrap();
        fs::write(src.join("rom1.bin"), vec![0u8; 199_680]).unwrap();
        let paths = paths(t.path());
        let report = import(&paths, &t.path().join("pack")).unwrap();
        assert_eq!(report.added.len(), 2);
        assert_eq!(report.ignored, 3);
        let ps2_dir = dir(&paths, "ps2").unwrap();
        assert!(ps2_dir.join("misnamed.NVM").is_file());
        let listed = list(&paths).unwrap();
        assert!(listed
            .iter()
            .any(|b| b.system == "ps2" && b.region == "NTSC-U"));
        assert!(listed
            .iter()
            .any(|b| b.system == "ps1" && b.region == "PAL"));
        // Importing again changes nothing.
        assert!(import(&paths, &t.path().join("pack"))
            .unwrap()
            .added
            .is_empty());
        assert_eq!(remove(&paths, "ps1").unwrap(), 1);
        assert!(list(&paths).unwrap().iter().all(|b| b.system != "ps1"));
        assert!(remove(&paths, "wii").is_err());
    }

    #[test]
    fn newest_bios_of_the_game_region_wins() {
        let files = vec![
            BiosFile {
                system: "ps2".into(),
                file: "old.bin".into(),
                region: "PAL".into(),
                version: "1.60".into(),
            },
            BiosFile {
                system: "ps2".into(),
                file: "new.bin".into(),
                region: "PAL".into(),
                version: "2.00".into(),
            },
            BiosFile {
                system: "ps2".into(),
                file: "us.bin".into(),
                region: "NTSC-U".into(),
                version: "1.60".into(),
            },
            BiosFile {
                system: "ps2".into(),
                file: "asia.bin".into(),
                region: "Asia".into(),
                version: "1.60".into(),
            },
        ];
        assert_eq!(best_for(&files, Some("PAL")).unwrap().file, "new.bin");
        assert_eq!(best_for(&files, Some("NTSC-U")).unwrap().file, "us.bin");
        assert_eq!(best_for(&files, Some("NTSC-J")).unwrap().file, "asia.bin");
        assert!(best_for(&files, None).is_none());
        assert!(best_for(&files[..1], Some("NTSC-U")).is_none());
    }

    #[test]
    fn pcsx2_selection_seeds_first_run_and_edits_keys_in_place() {
        let t = tempfile::tempdir().unwrap();
        let xdg = t.path().join("xdg");
        let bios = xdg.join("PCSX2/bios");
        fs::create_dir_all(&bios).unwrap();
        fs::write(bios.join("eu.bin"), ps2(b'E', "0200")).unwrap();
        fs::write(bios.join("us.bin"), ps2(b'A', "0160")).unwrap();
        // No settings file yet: seed a minimal one that skips the wizard.
        let ini = xdg.join("PCSX2/inis/PCSX2.ini");
        assert_eq!(
            select_for_pcsx2(&xdg, Some("PAL")).unwrap().as_deref(),
            Some("eu.bin")
        );
        let seeded = fs::read_to_string(&ini).unwrap();
        assert!(seeded.contains("SettingsVersion = 1"));
        assert!(seeded.contains("SetupWizardIncomplete = false"));
        assert!(seeded.contains("BIOS = eu.bin"));
        // No usable BIOS: nothing is created.
        let empty = t.path().join("empty");
        fs::create_dir_all(empty.join("PCSX2/bios")).unwrap();
        assert_eq!(select_for_pcsx2(&empty, Some("PAL")).unwrap(), None);
        assert!(!empty.join("PCSX2/inis/PCSX2.ini").exists());
        fs::write(
            &ini,
            "[UI]\nTheme = dark\n[Filenames]\nBIOS = us.bin\nOther = 1\n",
        )
        .unwrap();
        assert_eq!(
            select_for_pcsx2(&xdg, Some("PAL")).unwrap().as_deref(),
            Some("eu.bin")
        );
        let text = fs::read_to_string(&ini).unwrap();
        assert!(text.contains("BIOS = eu.bin") && text.contains("Other = 1"));
        assert!(text.contains("Theme = dark"));
        assert_eq!(select_for_pcsx2(&xdg, Some("JP")).unwrap(), None);
    }

    #[test]
    fn duckstation_seed_skips_wizard_and_keeps_other_settings() {
        let t = tempfile::tempdir().unwrap();
        seed_duckstation(t.path()).unwrap();
        let ini = t.path().join("duckstation/settings.ini");
        assert_eq!(
            fs::read_to_string(&ini).unwrap(),
            "\n[Main]\nSetupWizardIncomplete = false\n".trim_start_matches('\n')
        );
        fs::write(&ini, "[Main]\nSetupWizardIncomplete = true\nA = 1\n").unwrap();
        seed_duckstation(t.path()).unwrap();
        let text = fs::read_to_string(&ini).unwrap();
        assert!(text.contains("SetupWizardIncomplete = false") && text.contains("A = 1"));
    }

    #[test]
    fn inbox_imports_a_zip_of_bios_images() {
        use std::io::Write;
        let t = tempfile::tempdir().unwrap();
        let paths = paths(t.path());
        let inbox = paths.data_dir.join("bios-inbox");
        fs::create_dir_all(&inbox).unwrap();
        let mut zw = zip::ZipWriter::new(fs::File::create(inbox.join("b.zip")).unwrap());
        zw.start_file("us.bin", zip::write::SimpleFileOptions::default())
            .unwrap();
        zw.write_all(&ps2(b'A', "0160")).unwrap();
        zw.finish().unwrap();
        let report = import_inbox(&paths, None);
        assert_eq!(report.added.len(), 1);
        assert!(paths.pcsx2_bios_dir().join("us.bin").is_file());
        assert!(import_inbox(&paths, None).added.is_empty());
    }

    #[test]
    fn ini_editor_adds_missing_section_and_key() {
        assert_eq!(
            set_ini_value("[UI]\nA = 1\n", "Filenames", "BIOS", "x.bin"),
            "[UI]\nA = 1\n\n[Filenames]\nBIOS = x.bin\n"
        );
        assert_eq!(
            set_ini_value(
                "[Filenames]\nOther = 1\n[UI]\n",
                "Filenames",
                "BIOS",
                "x.bin"
            ),
            "[Filenames]\nBIOS = x.bin\nOther = 1\n[UI]\n"
        );
    }
}
