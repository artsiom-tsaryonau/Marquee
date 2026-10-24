use crate::catalog;
use crate::db::{now_rfc3339, Db};
use crate::error::{path_display, AppError, Result};
use crate::iso::identify_disc_iso;
use crate::model::{
    Edition, LibraryRoot, LibrarySnapshot, MediaFile, MediaKind, RootKind, RootStatus, ScanReport,
    SkipCount,
};
use crate::runners;
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub fn add_root(db: &Db, path: &Path, system: &str, label: Option<&str>) -> Result<LibraryRoot> {
    // Validates the system id against the shipped descriptors.
    let descriptor = catalog::by_id(system)?;
    let canonical = fs::canonicalize(path).map_err(|_| {
        AppError::new(
            "ROOT_OFFLINE",
            format!("folder not found: {}", path_display(path)),
        )
        .with_context(json!({ "path": path_display(path) }))
    })?;
    if !canonical.is_dir() {
        return Err(AppError::new(
            "ROOT_OFFLINE",
            format!("not a folder: {}", path_display(&canonical)),
        ));
    }
    if let Some((id, _)) = db.root_by_path(&canonical)? {
        let roots = snapshot_roots(db)?;
        let existing = roots
            .into_iter()
            .find(|r| r.id == id)
            .ok_or_else(|| AppError::new("DB", "root vanished"))?;
        if existing.system != system {
            return Err(AppError::new("ROOT_SYSTEM_CONFLICT", format!("This folder is already registered as {}. Choose its existing category or a different folder; its games and history were kept.", existing.system)));
        }
        return Ok(existing);
    }
    let id = Uuid::new_v4().to_string();
    db.upsert_root(&id, &canonical, "folder", &descriptor.id, label)?;
    Ok(LibraryRoot {
        id,
        path: canonical,
        kind: RootKind::Folder,
        system: descriptor.id,
        label: label.map(str::to_string),
        status: RootStatus::Online,
        last_scan_at: None,
    })
}

pub fn scan_root(db: &Db, root_id: Option<&str>) -> Result<ScanReport> {
    let roots = snapshot_roots(db)?;
    let selected: Vec<LibraryRoot> = match root_id {
        Some(id) => roots.into_iter().filter(|r| r.id == id).collect(),
        None => roots,
    };
    if selected.is_empty() {
        return Err(AppError::new(
            "ROOT_OFFLINE",
            "no library folder configured",
        ));
    }
    let mut report = ScanReport::default();
    let mut skip_map: BTreeMap<String, u32> = BTreeMap::new();
    for root in &selected {
        if !root.path.is_dir() {
            return Err(AppError::new(
                "ROOT_OFFLINE",
                format!(
                    "library folder is not available: {}",
                    path_display(&root.path)
                ),
            )
            .with_context(json!({ "root_id": root.id, "path": path_display(&root.path) })));
        }
        let descriptor = catalog::by_id(&root.system)?;
        walk_dir(db, &root.path, &descriptor, &mut report, &mut skip_map)?;
        db.update_scan_time(&root.id, &now_rfc3339())?;
    }
    report.skipped_by_kind = skip_map
        .into_iter()
        .map(|(kind, count)| SkipCount { kind, count })
        .collect();
    report.skipped_unsupported = report.skipped_by_kind.iter().map(|s| s.count).sum();
    report.missing = snapshot_media(db)?.iter().filter(|m| !m.available).count() as u32;
    Ok(report)
}

/// Drop library entries whose files are gone, keeping any with play history, a
/// favourite mark or a custom title (those stay and show as missing).
pub fn prune_missing(db: &Db) -> Result<u32> {
    let mut removed = 0;
    for edition in snapshot_editions(db)? {
        let file_gone = find_media_for_edition(db, &edition.id)?.is_none_or(|m| !m.path.exists());
        if edition.missing && file_gone && db.delete_unplayed_edition(&edition.id)? {
            removed += 1;
        }
    }
    Ok(removed)
}

fn walk_dir(
    db: &Db,
    dir: &Path,
    descriptor: &catalog::SystemDescriptor,
    report: &mut ScanReport,
    skip_map: &mut BTreeMap<String, u32>,
) -> Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let mut meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        // Symlinked disc images are common (one folder pointing at another drive).
        // Follow links to files only; linked folders stay skipped so a loop cannot recurse forever.
        if meta.file_type().is_symlink() {
            match fs::metadata(&path) {
                Ok(target) if target.is_file() => meta = target,
                _ => continue,
            }
        }
        if meta.is_dir() {
            walk_dir(db, &path, descriptor, report, skip_map)?;
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        if descriptor.id == "neogeo" && entry.file_name().eq_ignore_ascii_case("neogeo.zip") {
            continue; // Firmware is a dependency, not a playable game.
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        // Only extensions this system knows about count toward the report, so a
        // stray README or cover PNG is silent rather than "unsupported".
        if !descriptor.supports_extension(&ext) && !descriptor.skips_extension(&ext) {
            continue;
        }
        report.scanned += 1;
        if !descriptor.supports_extension(&ext) {
            *skip_map.entry(ext).or_default() += 1;
            continue;
        }
        let Some(kind) = MediaKind::for_system_extension(&descriptor.id, &ext) else {
            *skip_map.entry(ext).or_default() += 1;
            continue;
        };
        if let Err(err) = ingest_disc(db, &path, &meta, descriptor, kind, report) {
            if err.code == "DB" {
                return Err(err);
            }
            report
                .warnings
                .push(format!("{}: {}", path_display(&path), err.message));
        }
    }
    Ok(())
}

fn ingest_disc(
    db: &Db,
    path: &Path,
    meta: &fs::Metadata,
    descriptor: &catalog::SystemDescriptor,
    kind: MediaKind,
    report: &mut ScanReport,
) -> Result<()> {
    let mtime = meta.mtime();
    let size = meta.len() as i64;
    let existing = db.media_by_path(path)?;
    let unchanged = existing
        .as_ref()
        .map(|row| row.size_bytes == size && row.mtime == mtime)
        .unwrap_or(false);
    // CUE identity depends on companion tracks. Re-read bounded metadata on
    // every scan so restored/replaced tracks cannot leave a stale identity.
    if unchanged && kind != MediaKind::Ps1Cue {
        report.updated += 1;
        return Ok(());
    }
    // `.chd` is compressed and not ISO9660 on disk, so it carries no serial.
    // `.cue` is a sheet; its data track holds the filesystem.
    let identity = match kind {
        MediaKind::Ps1Cue => match crate::cue::identify_cue(path) {
            Ok(identity) => identity,
            Err(err) => {
                report
                    .warnings
                    .push(format!("{}: {}", path_display(path), err.message));
                crate::iso::unresolved()
            }
        },
        MediaKind::Ps1Chd | MediaKind::Rom => crate::iso::unresolved(),
        _ => identify_disc_iso(path)?,
    };
    if identity.serial.is_none() {
        report.unresolved_serial += 1;
    }
    let edition_id = match existing.as_ref() {
        Some(row) => row.edition_id.clone(),
        // A game added without files takes this file when it is the same game.
        None => match crate::wanted::claim(db, &descriptor.id, &title_from_filename(path))? {
            Some(id) => id,
            None => Uuid::new_v4().to_string(),
        },
    };
    let media_id = match existing.as_ref() {
        Some(row) => row.id.clone(),
        None => Uuid::new_v4().to_string(),
    };
    let title = title_from_filename(path);
    db.upsert_edition(
        &edition_id,
        &descriptor.id,
        &title,
        Some(&media_id),
        &now_rfc3339(),
    )?;
    db.upsert_media(
        &media_id,
        &edition_id,
        kind.as_str(),
        path,
        size,
        mtime,
        identity.serial.as_deref(),
        identity.serial_raw.as_deref(),
        identity.region.as_deref(),
        None,
    )?;
    if existing
        .as_ref()
        .is_some_and(|m| m.serial != identity.serial)
    {
        // Keep cache files harmlessly on disk; detach stale art until the
        // optional background lookup fetches the new identity's cover.
        db.conn
            .execute("DELETE FROM artwork WHERE edition_id=?1", [&edition_id])?;
    }
    if existing.is_some() {
        report.updated += 1;
    } else {
        report.added += 1;
    }
    Ok(())
}

fn title_from_filename(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown".to_string())
}

pub fn snapshot(db: &Db) -> Result<LibrarySnapshot> {
    Ok(LibrarySnapshot {
        catalogue: crate::metadata::snapshot(db)?,
        roots: snapshot_roots(db)?,
        editions: snapshot_editions(db)?,
        media: snapshot_media(db)?,
        runners: runners::snapshot(db)?,
        sessions: crate::sessions::list(db)?,
        settings: db.load_settings()?,
    })
}

pub fn snapshot_roots(db: &Db) -> Result<Vec<LibraryRoot>> {
    db.list_roots()?
        .into_iter()
        .map(|row| {
            let status = if row.path.is_dir() {
                RootStatus::Online
            } else {
                RootStatus::Offline
            };
            Ok(LibraryRoot {
                id: row.id,
                path: row.path,
                kind: RootKind::Folder,
                system: row.system,
                label: row.label,
                status,
                last_scan_at: row.last_scan_at,
            })
        })
        .collect()
}

pub fn snapshot_media(db: &Db) -> Result<Vec<MediaFile>> {
    db.list_media()?
        .into_iter()
        .map(|row| {
            let kind = MediaKind::parse(&row.kind).unwrap_or(MediaKind::Iso);
            let mut available = row.path.is_file();
            let mut skip_reason = row.skip_reason;
            if kind == MediaKind::Ps1Cue && available {
                if let Err(err) = crate::cue::manifest(&row.path) {
                    available = false;
                    skip_reason = Some(err.message);
                }
            }
            if kind == MediaKind::Rom
                && available
                && row
                    .path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
                && db
                    .edition_by_id(&row.edition_id)?
                    .is_some_and(|e| e.system == "neogeo")
            {
                skip_reason = crate::neogeo::issue(&row.path);
            }
            Ok(MediaFile {
                id: row.id,
                edition_id: row.edition_id,
                kind,
                available,
                path: row.path,
                size_bytes: row.size_bytes as u64,
                mtime: row.mtime,
                serial: row.serial,
                serial_raw: row.serial_raw,
                region: row.region,
                skip_reason,
            })
        })
        .collect()
}

pub fn snapshot_editions(db: &Db) -> Result<Vec<Edition>> {
    let media = snapshot_media(db)?;
    let covers: BTreeMap<String, String> = db.list_cover_paths()?.into_iter().collect();
    let infos = crate::romfacts::list(db)?;
    let wanted = crate::wanted::statuses(db)?;
    db.list_editions()?
        .into_iter()
        .map(|row| {
            let media_missing = match &row.media_id {
                Some(mid) => media
                    .iter()
                    .find(|m| &m.id == mid)
                    .map(|m| !m.available || m.skip_reason.is_some())
                    .unwrap_or(true),
                None => true,
            };
            let attached = row
                .media_id
                .as_ref()
                .and_then(|mid| media.iter().find(|m| &m.id == mid));
            Ok(Edition {
                runner_override: db.profile_runner(&row.id)?,
                cover_path: covers.get(&row.id).map(PathBuf::from),
                info: infos.get(&row.id).cloned(),
                wanted: wanted.get(&row.id).cloned(),
                id: row.id,
                system: row.system,
                title: row
                    .display_title
                    .clone()
                    .unwrap_or_else(|| row.scan_title.clone()),
                scan_title: row.scan_title,
                display_title: row.display_title,
                favorite: row.favorite,
                media_id: row.media_id,
                last_played_at: row.last_played_at,
                playtime_seconds: row.playtime_seconds,
                play_count: row.play_count,
                imported_at: row.imported_at,
                missing: media_missing,
                serial: attached.and_then(|m| m.serial.clone()),
                region: attached.and_then(|m| m.region.clone()),
            })
        })
        .collect()
}

pub fn find_media_for_edition(db: &Db, edition_id: &str) -> Result<Option<MediaFile>> {
    Ok(snapshot_media(db)?
        .into_iter()
        .find(|m| m.edition_id == edition_id))
}
