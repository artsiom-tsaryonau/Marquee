use crate::db::{now_rfc3339, Db};
use crate::error::{AppError, Result};
use crate::http;
use crate::iso::cover_serial;
use crate::model::{ArtworkReport, CoreEvent};
use crate::sessions::EventSink;
use rusqlite::OptionalExtension;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const COVER_KIND: &str = "cover";

/// xlenore cover sets, one repository per system. Filenames are the disc serial
/// with the underscore swapped for a dash (`SLUS_209.63` → `SLUS-20963.jpg`);
/// both sets follow that convention.
const PS2_COVER_BASE: &str =
    "https://raw.githubusercontent.com/xlenore/ps2-covers/main/covers/default";
const PS1_COVER_BASE: &str =
    "https://raw.githubusercontent.com/xlenore/psx-covers/main/covers/default";

pub struct CoverJob {
    pub edition_id: String,
    pub url: String,
    pub dest: PathBuf,
    pub media_id: Option<String>,
    pub system: String,
    pub serial: String,
    existing: Option<(String, String, String)>,
}

fn existing_artwork(db: &Db, edition: &str) -> Result<Option<(String, String, String)>> {
    Ok(db
        .conn
        .query_row(
            "SELECT path,source,fetched_at FROM artwork WHERE edition_id=?1 AND kind='cover'",
            [edition],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?)
}

fn validate_jpeg(bytes: &[u8]) -> Result<()> {
    if bytes.len() as u64 > http::COVER_MAX_BYTES || !bytes.ends_with(&[0xff, 0xd9]) {
        return Err(AppError::new("NETWORK", "Cover is oversized or truncated"));
    }
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Jpeg);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    reader
        .decode()
        .map_err(|err| AppError::new("NETWORK", format!("Invalid cover image: {err}")))?;
    Ok(())
}

fn valid_cache(path: &Path) -> bool {
    let Ok(file) = fs::File::open(path) else {
        return false;
    };
    if !file
        .metadata()
        .is_ok_and(|m| m.len() <= http::COVER_MAX_BYTES)
    {
        return false;
    }
    let mut bytes = Vec::new();
    file.take(http::COVER_MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_ok()
        && validate_jpeg(&bytes).is_ok()
}

pub fn cover_base(system: &str) -> &'static str {
    match system {
        crate::model::SYSTEM_PS1 => PS1_COVER_BASE,
        _ => PS2_COVER_BASE,
    }
}

/// Cover URL for a system. Kept as `cover_url` for the PS2 path so existing
/// callers and tests stay valid.
pub fn cover_url_for(system: &str, normalized_serial: &str) -> Option<String> {
    if !matches!(system, crate::model::SYSTEM_PS1 | crate::model::SYSTEM_PS2) {
        return None;
    }
    cover_serial(normalized_serial).map(|id| format!("{}/{}.jpg", cover_base(system), id))
}

pub fn cover_url(normalized_serial: &str) -> Option<String> {
    cover_url_for(crate::model::SYSTEM_PS2, normalized_serial)
}

pub fn pending_jobs(
    db: &Db,
    cache_dir: &Path,
    edition_id: Option<&str>,
) -> Result<(Vec<CoverJob>, u32)> {
    let editions = crate::library::snapshot_editions(db)?;
    let art_dir = cache_dir.join("art");
    fs::create_dir_all(&art_dir)?;
    let mut jobs = Vec::new();
    let mut skipped = 0;
    for edition in editions {
        if let Some(wanted) = edition_id {
            if edition.id != wanted {
                continue;
            }
        }
        let existing = existing_artwork(db, &edition.id)?;
        if let Some((path, _, _)) = &existing {
            if valid_cache(Path::new(path)) {
                skipped += 1;
                continue;
            }
        }
        let Some(serial) = edition.serial.as_deref() else {
            skipped += 1;
            continue;
        };
        let Some(url) = cover_url_for(&edition.system, serial) else {
            skipped += 1;
            continue;
        };
        jobs.push(CoverJob {
            dest: art_dir.join(format!("{}.jpg", edition.id)),
            edition_id: edition.id,
            url,
            media_id: edition.media_id,
            system: edition.system,
            serial: serial.to_string(),
            existing,
        });
    }
    Ok((jobs, skipped))
}

pub fn store_jpeg(db: &Db, job: &CoverJob, bytes: &[u8]) -> Result<bool> {
    validate_jpeg(bytes)?;
    // A rescan/removal or correction while the download ran invalidates this job.
    let current = crate::library::snapshot_editions(db)?
        .into_iter()
        .find(|e| e.id == job.edition_id);
    if !current.is_some_and(|e| {
        e.system == job.system
            && e.media_id == job.media_id
            && e.serial.as_deref() == Some(&job.serial)
            && !e.missing
    }) || existing_artwork(db, &job.edition_id)? != job.existing
    {
        return Ok(false);
    }
    let mut tmp = tempfile::NamedTempFile::new_in(
        job.dest
            .parent()
            .ok_or_else(|| AppError::new("IO", "Cover cache has no parent"))?,
    )?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(&job.dest)
        .map_err(|err| AppError::new("IO", err.to_string()))?;
    // The source label records which cover set the file came from.
    let source = if job.url.contains("psx-covers") {
        "xlenore/psx-covers"
    } else {
        "xlenore/ps2-covers"
    };
    db.upsert_artwork(
        &job.edition_id,
        COVER_KIND,
        &job.dest.to_string_lossy(),
        source,
        &now_rfc3339(),
    )?;
    Ok(true)
}

pub fn fetch_cover(url: &str) -> crate::error::Result<Vec<u8>> {
    http::get_bytes(url)
}

pub fn finish(report: ArtworkReport, sink: Option<Arc<dyn EventSink>>) -> ArtworkReport {
    if let Some(sink) = sink {
        sink.emit(CoreEvent::ArtworkFinished {
            fetched: report.fetched,
            skipped: report.skipped,
            failed: report.failed,
        });
        sink.emit(CoreEvent::LibraryChanged);
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_url_uses_pcsx2_serial_filename() {
        assert_eq!(
            cover_url("SLUS_209.63").as_deref(),
            Some("https://raw.githubusercontent.com/xlenore/ps2-covers/main/covers/default/SLUS-20963.jpg")
        );
    }

    #[test]
    fn ps1_covers_use_the_psx_covers_set() {
        assert_eq!(
            cover_url_for("ps1", "SLUS_005.94").as_deref(),
            Some("https://raw.githubusercontent.com/xlenore/psx-covers/main/covers/default/SLUS-00594.jpg")
        );
        assert_eq!(
            cover_base("ps1"),
            "https://raw.githubusercontent.com/xlenore/psx-covers/main/covers/default"
        );
        assert_eq!(
            cover_base("ps2"),
            "https://raw.githubusercontent.com/xlenore/ps2-covers/main/covers/default"
        );
    }

    fn fixture() -> (tempfile::TempDir, crate::Core, CoverJob) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("roms");
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("game.iso"),
            crate::iso::build_test_iso(&[("SYSTEM.CNF", b"BOOT2 = cdrom0:\\SLUS_123.45;1\n")]),
        )
        .unwrap();
        let core = crate::Core::open_isolated(Some(temp.path().join("data")), false).unwrap();
        core.add_root("ps2", root, None).unwrap();
        core.scan(None, None).unwrap();
        let job = core
            .with_db(|db| Ok(pending_jobs(db, &core.paths.cache_dir, None)?.0.remove(0)))
            .unwrap();
        (temp, core, job)
    }

    fn jpeg() -> Vec<u8> {
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut bytes)
            .encode(&[128, 64, 32], 1, 1, image::ExtendedColorType::Rgb8)
            .unwrap();
        bytes
    }

    #[test]
    fn invalid_images_never_replace_the_cache_and_corrupt_caches_can_be_repaired() {
        let (_temp, core, job) = fixture();
        for bytes in [b"PNG".as_slice(), &[0xff, 0xd8, 0xff, 0xd9]] {
            assert_eq!(
                core.with_db(|db| store_jpeg(db, &job, bytes))
                    .unwrap_err()
                    .code,
                "NETWORK"
            );
            assert!(!job.dest.exists());
        }
        let bytes = jpeg();
        assert!(core.with_db(|db| store_jpeg(db, &job, &bytes)).unwrap());
        assert!(core
            .with_db(|db| store_jpeg(db, &job, &bytes[..bytes.len() - 2]))
            .is_err());
        assert_eq!(fs::read(&job.dest).unwrap(), bytes);
        assert!(core
            .with_db(|db| Ok(pending_jobs(db, &core.paths.cache_dir, None)?.0.is_empty()))
            .unwrap());
        fs::write(&job.dest, [0xff, 0xd8, 0xff, 0xd9]).unwrap();
        let replacement = core
            .with_db(|db| Ok(pending_jobs(db, &core.paths.cache_dir, None)?.0.remove(0)))
            .unwrap();
        assert!(core
            .with_db(|db| store_jpeg(db, &replacement, &bytes))
            .unwrap());
        assert_eq!(fs::read(&job.dest).unwrap(), bytes);
    }

    #[test]
    fn stale_jobs_cannot_publish_after_identity_changes_or_artwork_corrections() {
        let (_temp, core, job) = fixture();
        core.with_db(|db| {
            db.conn.execute(
                "UPDATE media SET serial='SLUS_999.99' WHERE edition_id=?1",
                [&job.edition_id],
            )?;
            assert!(!store_jpeg(db, &job, &jpeg())?);
            db.conn.execute(
                "UPDATE media SET serial=?1 WHERE edition_id=?2",
                [&job.serial, &job.edition_id],
            )?;
            db.upsert_artwork(&job.edition_id, "cover", "/my/cover.jpg", "user", "today")?;
            assert!(!store_jpeg(db, &job, &jpeg())?);
            assert_eq!(
                db.artwork_path(&job.edition_id, "cover")?.as_deref(),
                Some("/my/cover.jpg")
            );
            Ok(())
        })
        .unwrap();
        assert!(!job.dest.exists());
    }

    #[test]
    fn oversized_dimensions_and_unsupported_systems_are_rejected() {
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut bytes)
            .encode(&vec![0; 4097 * 3], 4097, 1, image::ExtendedColorType::Rgb8)
            .unwrap();
        assert!(validate_jpeg(&bytes).is_err());
        assert!(cover_url_for("pce", "SLUS_123.45").is_none());
    }
}
