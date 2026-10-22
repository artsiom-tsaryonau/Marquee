//! Legacy immediate intake. Durable jobs use the same shared ArchivePlan.
use crate::catalog;
use crate::error::{AppError, Result};
use crate::Core;
pub use marquee_media::archive::*;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;
const FREE_MARGIN: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Files were unpacked into the system's library folder.
    Imported,
    /// An arcade ZIP set was placed unchanged.
    KeptArchive,
    /// Files were stored, but Marquee has no emulator for this system yet.
    StoredNoEmulator,
    /// Everything was already in the library.
    AlreadyPresent,
    /// Nothing was changed.
    Skipped,
}

#[derive(Debug, Clone, Serialize)]
pub struct IntakeReport {
    pub archive: PathBuf,
    pub outcome: Outcome,
    pub system: Option<String>,
    pub dest: Option<PathBuf>,
    pub files: Vec<String>,
    pub message: String,
    pub source_removed: bool,
}

impl Core {
    /// Folder for a system's library files, creating and registering one when
    /// no root exists yet. Returns the path and whether the system is known.
    pub(crate) fn intake_root(&self, system: &str) -> Result<PathBuf> {
        let snap = self.snapshot()?;
        if let Some(root) = snap
            .roots
            .iter()
            .find(|r| r.system == system && r.path.is_dir())
        {
            return Ok(root.path.clone());
        }
        let base = snap
            .roots
            .iter()
            .find_map(|r| r.path.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| self.paths.data_dir.join("roms"));
        let dir = base.join(system);
        fs::create_dir_all(&dir)?;
        if catalog::by_id(system).is_ok() {
            self.add_root(system, dir.clone(), None)?;
        }
        Ok(dir)
    }

    /// Unpack one archive into the library. Does not rescan; call `scan` after.
    pub fn intake_archive(&self, archive: &Path, delete_source: bool) -> Result<IntakeReport> {
        let report = |outcome, system: Option<String>, dest, files, message: String| IntakeReport {
            archive: archive.to_path_buf(),
            outcome,
            system,
            dest,
            files,
            message,
            source_removed: false,
        };
        let kind = kind_of(archive)?;
        let entries = list_entries(archive, kind)?;
        let stem = archive
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "game".into());
        let playable: Vec<Entry> = entries
            .iter()
            .filter(|e| !is_junk(&e.rel))
            .cloned()
            .collect();
        if playable.is_empty() {
            return Ok(report(
                Outcome::Skipped,
                None,
                None,
                vec![],
                "Nothing playable in this archive (only documents or images).".into(),
            ));
        }
        let has_cue = playable.iter().any(|e| ext_of(&e.rel) == "cue");
        let systems: BTreeSet<String> = playable
            .iter()
            .filter_map(|e| system_for(&e.rel, e.size, has_cue))
            .collect();

        // Arcade sets stay zipped.
        if systems.is_empty() && kind == Kind::Zip && looks_like_neogeo_set(&stem, &playable) {
            let dest_dir = self.intake_root("neogeo")?;
            let name = archive
                .file_name()
                .map(|n| n.to_os_string())
                .unwrap_or_else(|| "set.zip".into());
            let dest = dest_dir.join(&name);
            if dest.exists() {
                return Ok(report(
                    Outcome::AlreadyPresent,
                    Some("neogeo".into()),
                    Some(dest_dir),
                    vec![name.to_string_lossy().into_owned()],
                    "This set is already in the library.".into(),
                ));
            }
            fs::copy(archive, &dest)?;
            let mut rep = report(
                Outcome::KeptArchive,
                Some("neogeo".into()),
                Some(dest_dir),
                vec![name.to_string_lossy().into_owned()],
                "Arcade set kept as a ZIP. Its BIOS and parent sets must sit beside it.".into(),
            );
            if delete_source {
                fs::remove_file(archive)?;
                rep.source_removed = true;
            }
            return Ok(rep);
        }

        let system = match systems.len() {
            1 => systems.into_iter().next().unwrap(),
            0 => {
                return Ok(report(
                    Outcome::Skipped,
                    None,
                    None,
                    vec![],
                    "Could not tell which system this belongs to. If it is a BIOS pack use `marquee bios import`.".into(),
                ))
            }
            _ => {
                return Ok(report(
                    Outcome::Skipped,
                    None,
                    None,
                    vec![],
                    format!("Archive mixes systems ({}); unpack it by hand.", systems.into_iter().collect::<Vec<_>>().join(", ")),
                ))
            }
        };

        let supported = catalog::by_id(&system).is_ok();
        let dest_dir = if supported {
            self.intake_root(&system)?
        } else {
            let snap = self.snapshot()?;
            let base = snap
                .roots
                .iter()
                .find_map(|r| r.path.parent().map(Path::to_path_buf))
                .unwrap_or_else(|| self.paths.data_dir.join("roms"));
            let dir = base.join(&system);
            fs::create_dir_all(&dir)?;
            dir
        };

        // One file goes straight in; several keep a folder named after the archive.
        let folder = playable.len() > 1;
        let common_top = {
            let first = playable[0].rel.components().next();
            first.filter(|f| {
                playable.len() > 1
                    && playable.iter().all(|e| {
                        e.rel.components().count() > 1 && e.rel.components().next() == Some(*f)
                    })
            })
        };
        let wanted: Vec<(Entry, PathBuf)> = playable
            .iter()
            .map(|e| {
                let rel: PathBuf = match (folder, common_top) {
                    (false, _) => e
                        .rel
                        .file_name()
                        .map(PathBuf::from)
                        .unwrap_or_else(|| e.rel.clone()),
                    (true, Some(_)) => e.rel.clone(),
                    (true, None) => Path::new(&stem).join(&e.rel),
                };
                (e.clone(), rel)
            })
            .collect();

        let need: u64 = wanted.iter().map(|(e, _)| e.size).sum();
        if let Some(free) = free_bytes(&dest_dir) {
            if free < need.saturating_add(FREE_MARGIN) {
                return Err(AppError::new(
                    "INTAKE_DISK",
                    format!(
                        "Not enough free space in {}: need about {} MiB, have {} MiB",
                        dest_dir.display(),
                        need / 1048576,
                        free / 1048576
                    ),
                ));
            }
        }

        let pending: Vec<&(Entry, PathBuf)> = wanted
            .iter()
            .filter(|(e, rel)| {
                let path = dest_dir.join(rel);
                fs::metadata(&path).map_or(true, |m| m.len() != e.size)
                    || (ext_of(rel) == "cue"
                        && !matches!(crate::cue::normalized_import(&path), Ok(None)))
            })
            .collect();
        if pending.is_empty() {
            return Ok(report(
                Outcome::AlreadyPresent,
                Some(system),
                Some(dest_dir),
                wanted
                    .iter()
                    .map(|(_, r)| r.display().to_string())
                    .collect(),
                "Everything in this archive is already in the library.".into(),
            ));
        }

        let staging = dest_dir.join(format!(".marquee-intake-{}", Uuid::new_v4()));
        fs::create_dir_all(&staging)?;
        let result = (|| -> Result<()> {
            let todo: Vec<(Entry, PathBuf)> = pending.iter().map(|p| (*p).clone()).collect();
            extract(archive, kind, &todo, &staging)?;
            // Repair provider CUE quoting before publishing any files. Replacing
            // the two delimiter bytes preserves archive size verification.
            for (_, rel) in &todo {
                if ext_of(rel) == "cue" {
                    let path = staging.join(rel);
                    if let Some(bytes) = crate::cue::normalized_import(&path)? {
                        fs::write(path, bytes)?;
                    }
                }
            }
            for (_, rel) in &todo {
                let to = dest_dir.join(rel);
                if let Some(parent) = to.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::rename(staging.join(rel), &to)?;
            }
            Ok(())
        })();
        let _ = fs::remove_dir_all(&staging);
        result?;

        let files: Vec<String> = wanted
            .iter()
            .map(|(_, r)| r.display().to_string())
            .collect();
        let (outcome, message) = if supported {
            (
                Outcome::Imported,
                format!("Unpacked {} file(s).", files.len()),
            )
        } else {
            (
                Outcome::StoredNoEmulator,
                format!(
                    "Stored {} file(s) under {system}, but Marquee has no emulator for it yet.",
                    files.len()
                ),
            )
        };
        let message = match possible_duplicates(&dest_dir, &wanted) {
            d if d.is_empty() => message,
            d => format!("{message} Possible duplicate of: {}.", d.join(", ")),
        };
        let mut rep = report(
            outcome,
            Some(system),
            Some(dest_dir.clone()),
            files,
            message,
        );
        if delete_source {
            let verified = wanted
                .iter()
                .all(|(e, r)| fs::metadata(dest_dir.join(r)).is_ok_and(|m| m.len() == e.size));
            if verified {
                fs::remove_file(archive)?;
                rep.source_removed = true;
            }
        }
        Ok(rep)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    fn make_zip(path: &Path, files: &[(&str, &[u8])]) {
        let mut w = zip::ZipWriter::new(File::create(path).unwrap());
        for (name, data) in files {
            w.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap();
    }

    fn core(dir: &Path) -> Core {
        Core::open(Some(dir.to_path_buf()), true).unwrap()
    }

    #[test]
    fn preparation_space_refuses_insufficient_or_unknown_capacity_before_writing() {
        assert_eq!(
            require_preparation_space(Some(99), 100).unwrap_err().code,
            "INTAKE_DISK"
        );
        assert_eq!(
            require_preparation_space(None, 100).unwrap_err().code,
            "INTAKE_DISK"
        );
        assert!(require_preparation_space(Some(100), 100).is_ok());
    }

    #[test]
    fn rejects_paths_that_escape() {
        assert!(safe_rel("../evil").is_none());
        assert!(safe_rel("a/../../evil").is_none());
        assert!(safe_rel("/etc/passwd").is_none());
        assert_eq!(safe_rel("a\\b.nds").unwrap(), PathBuf::from("a/b.nds"));
    }

    #[test]
    fn single_cartridge_lands_in_its_system_root_and_source_can_go() {
        let tmp = tempfile::tempdir().unwrap();
        let c = core(&tmp.path().join("data"));
        let zip = tmp.path().join("Game (Spain).zip");
        make_zip(
            &zip,
            &[("Game (Spain).nds", b"nds-bytes"), ("readme.txt", b"hi")],
        );
        let r = c.intake_archive(&zip, true).unwrap();
        assert_eq!(r.outcome, Outcome::Imported);
        assert_eq!(r.system.as_deref(), Some("ds"));
        assert_eq!(r.files, vec!["Game (Spain).nds"]);
        let dest = r.dest.unwrap().join("Game (Spain).nds");
        assert_eq!(fs::read(dest).unwrap(), b"nds-bytes");
        assert!(r.source_removed && !zip.exists());
        // Second time: nothing to do.
        make_zip(&zip, &[("Game (Spain).nds", b"nds-bytes")]);
        let again = c.intake_archive(&zip, false).unwrap();
        assert_eq!(again.outcome, Outcome::AlreadyPresent);
    }

    #[test]
    fn multi_file_disc_gets_a_folder_and_leaves_no_staging() {
        let tmp = tempfile::tempdir().unwrap();
        let c = core(&tmp.path().join("data"));
        let zip = tmp.path().join("Disc (USA).zip");
        make_zip(
            &zip,
            &[
                ("Disc (USA).cue", b"FILE \"Disc (USA).bin\" BINARY"),
                ("Disc (USA).bin", b"xx"),
            ],
        );
        let r = c.intake_archive(&zip, false).unwrap();
        assert_eq!(r.system.as_deref(), Some("ps1"));
        let dest = r.dest.unwrap();
        assert!(dest.join("Disc (USA)/Disc (USA).cue").is_file());
        assert!(fs::read_dir(&dest).unwrap().flatten().all(|e| !e
            .file_name()
            .to_string_lossy()
            .starts_with(".marquee-intake")));
        assert!(zip.exists());
    }

    #[test]
    fn arcade_set_stays_zipped() {
        let tmp = tempfile::tempdir().unwrap();
        let c = core(&tmp.path().join("data"));
        let zip = tmp.path().join("kof98.zip");
        make_zip(&zip, &[("242-p1.p1", b"a"), ("242-c1.rom", b"b")]);
        let r = c.intake_archive(&zip, false).unwrap();
        assert_eq!(r.outcome, Outcome::KeptArchive);
        assert!(r.dest.unwrap().join("kof98.zip").is_file());
    }

    #[test]
    fn provider_cue_quotes_are_repaired_on_import_and_reimport() {
        let tmp = tempfile::tempdir().unwrap();
        let c = core(&tmp.path().join("data"));
        let archive = tmp.path().join("Disc.zip");
        let cue = b"FILE 'Disc (Track 1).bin' BINARY\r\nTRACK 01 MODE2/2352\r\nINDEX 01 00:00:00\r\nFILE 'Disc (Track 2).bin' BINARY\r\nTRACK 02 AUDIO\r\nINDEX 01 00:00:00\r\n";
        make_zip(
            &archive,
            &[
                ("Disc.cue", cue),
                ("Disc (Track 1).bin", b"data"),
                ("Disc (Track 2).bin", b"audio"),
            ],
        );
        let report = c.intake_archive(&archive, false).unwrap();
        let imported = report.dest.unwrap().join("Disc/Disc.cue");
        assert_eq!(crate::cue::manifest(&imported).unwrap().tracks.len(), 2);
        assert_eq!(fs::read(&imported).unwrap().len(), cue.len());
        // Simulate a copy extracted by the old importer: same path and size.
        fs::write(&imported, cue).unwrap();
        assert_eq!(
            c.intake_archive(&archive, false).unwrap().outcome,
            Outcome::Imported
        );
        assert!(crate::cue::manifest(&imported).is_ok());
        assert_eq!(
            c.intake_archive(&archive, false).unwrap().outcome,
            Outcome::AlreadyPresent
        );
        assert!(archive.exists());
    }

    #[test]
    fn hostile_archive_is_refused_and_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let c = core(&tmp.path().join("data"));
        let zip = tmp.path().join("evil.zip");
        make_zip(&zip, &[("../escape.nds", b"x")]);
        let err = c.intake_archive(&zip, false).unwrap_err();
        assert_eq!(err.code, "INTAKE_UNSAFE");
        assert!(!tmp.path().join("escape.nds").exists());
    }

    #[test]
    fn bios_pack_and_documents_are_skipped_not_guessed() {
        let tmp = tempfile::tempdir().unwrap();
        let c = core(&tmp.path().join("data"));
        let bios = tmp.path().join("bios.zip");
        make_zip(&bios, &[("SCPH-70012.bin", b"b")]);
        assert_eq!(
            c.intake_archive(&bios, false).unwrap().outcome,
            Outcome::Skipped
        );
        let docs = tmp.path().join("docs.zip");
        make_zip(&docs, &[("readme.txt", b"b")]);
        assert_eq!(
            c.intake_archive(&docs, false).unwrap().outcome,
            Outcome::Skipped
        );
    }

    #[test]
    fn corrupt_rar_is_detected_by_content_and_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let c = core(&tmp.path().join("data"));
        let f = tmp.path().join("x.zip");
        fs::write(&f, b"Rar!\x1a\x07\x00 not a zip").unwrap();
        assert_eq!(
            c.intake_archive(&f, false).unwrap_err().code,
            "INTAKE_FORMAT"
        );
    }

    #[test]
    fn stored_rar4_and_rar5_import_real_bytes_and_preserve_sources() {
        for fixture in ["regular4.rar", "regular5.rar"] {
            let tmp = tempfile::tempdir().unwrap();
            let c = core(&tmp.path().join("data"));
            // Content detection must work independently of the extension.
            let source = tmp.path().join("download.zip");
            fs::copy(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/rar")
                    .join(fixture),
                &source,
            )
            .unwrap();
            let before = fs::read(&source).unwrap();
            let report = c.intake_archive(&source, false).unwrap();
            assert_eq!(report.outcome, Outcome::Imported);
            assert_eq!(report.system.as_deref(), Some("ds"));
            assert_eq!(
                fs::read(report.dest.unwrap().join("Game.nds")).unwrap(),
                b"synthetic cartridge bytes\n"
            );
            assert_eq!(fs::read(&source).unwrap(), before);
        }
    }

    #[test]
    fn rar_links_and_traversal_are_refused_before_extraction() {
        for fixture in [
            "symlink4.rar",
            "symlink5.rar",
            "hardlink5.rar",
            "filecopy5.rar",
            "directory-link5.rar",
            "traversal5.rar",
        ] {
            let tmp = tempfile::tempdir().unwrap();
            let c = core(&tmp.path().join("data"));
            let source = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/rar")
                .join(fixture);
            let copy = tmp.path().join(fixture);
            fs::copy(&source, &copy).unwrap();
            let error = c.intake_archive(&copy, true).unwrap_err();
            assert_eq!(error.code, "INTAKE_UNSAFE", "{fixture}: {error}");
            assert!(source.exists());
            assert!(!tmp.path().join("outside.nds").exists());
            assert!(!c.paths.data_dir.join("roms").exists());
        }
    }

    #[test]
    fn damaged_rar_payload_leaves_no_partial_game_and_preserves_source() {
        let tmp = tempfile::tempdir().unwrap();
        let c = core(&tmp.path().join("data"));
        let mut bytes = include_bytes!("../tests/fixtures/rar/regular5.rar").to_vec();
        let offset = bytes.windows(9).position(|v| v == b"synthetic").unwrap();
        bytes[offset] ^= 1; // Keep valid headers, break the payload's CRC.
        let source = tmp.path().join("damaged.rar");
        fs::write(&source, bytes).unwrap();
        assert_eq!(
            c.intake_archive(&source, true).unwrap_err().code,
            "INTAKE_FORMAT"
        );
        assert!(source.exists());
        let root = c.paths.data_dir.join("roms/ds");
        assert!(fs::read_dir(root).unwrap().next().is_none());
    }
}
