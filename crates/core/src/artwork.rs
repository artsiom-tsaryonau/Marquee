use crate::db::{now_rfc3339, Db};
use crate::error::Result;
use crate::http;
use crate::iso::cover_serial;
use crate::model::{ArtworkReport, CoreEvent};
use crate::sessions::EventSink;
use std::fs;
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
        if let Some(existing) = db.artwork_path(&edition.id, COVER_KIND)? {
            if PathBuf::from(&existing).is_file() {
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
        });
    }
    Ok((jobs, skipped))
}

pub fn store_jpeg(db: &Db, job: &CoverJob, bytes: &[u8]) -> Result<()> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return Err(crate::error::AppError::new(
            "NETWORK",
            "cover was not a JPEG",
        ));
    }
    let tmp = job.dest.with_extension("jpg.part");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, &job.dest)?;
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
    Ok(())
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
    use crate::db::Db;

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

    #[test]
    fn store_jpeg_rejects_non_jpeg_and_keeps_a_valid_file() {
        let db = Db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("cover.jpg");
        let job = CoverJob {
            edition_id: "edition-1".into(),
            url: "https://example.invalid/cover.jpg".into(),
            dest: dest.clone(),
        };
        assert_eq!(store_jpeg(&db, &job, b"PNG").unwrap_err().code, "NETWORK");
        store_jpeg(&db, &job, &[0xFF, 0xD8, 0xFF, 0xD9]).unwrap();
        assert!(dest.is_file());
        assert_eq!(
            db.artwork_path("edition-1", COVER_KIND).unwrap().as_deref(),
            Some(dest.to_string_lossy().as_ref())
        );
    }
}
