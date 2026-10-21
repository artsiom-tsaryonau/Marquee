//! Offline provider metadata. Catalogue records never become playable editions.
use crate::db::{now_rfc3339, Db};
use crate::error::{AppError, Result};
use crate::sessions::EventSink;
use crate::{model::CoreEvent, Core};
use base64::Engine;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::io::Read;
use std::os::unix::fs::MetadataExt;

fn file_matches(path: &std::path::Path, size: i64, mtime: i64) -> bool {
    std::fs::metadata(path)
        .is_ok_and(|m| m.is_file() && m.len() == size as u64 && m.mtime() == mtime)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Ratings {
    pub graphics: Option<f64>,
    pub sound: Option<f64>,
    pub gameplay: Option<f64>,
    pub overall: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cover {
    pub data_url: String,
    pub credit: String,
    pub source_url: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variant {
    #[serde(default)]
    pub regions: Vec<String>,
    #[serde(default)]
    pub languages: Vec<String>,
    pub verified_at: Option<String>,
    pub media_id: String,
    pub filename: String,
    pub version: String,
    pub size: String,
    pub serial: String,
    pub crc32: String,
    pub md5: String,
    pub sha1: String,
    #[serde(default)]
    pub download_url: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldProvenance {
    pub source_url: String,
    pub exported_at: String,
    pub imported_at: String,
    pub method: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    #[serde(default)]
    pub id: String,
    pub game_id: String,
    pub system: String,
    pub title: String,
    pub regions: String,
    pub languages: String,
    pub version: String,
    pub rating: String,
    pub url: String,
    pub has_detail: bool,
    pub media: Vec<Variant>,
    pub players: Option<u32>,
    pub year: Option<u32>,
    #[serde(default)]
    pub ratings: Ratings,
    pub rating_votes: Option<u32>,
    pub verified_at: Option<String>,
    pub cover: Option<Cover>,
    #[serde(default)]
    pub exported_at: String,
    #[serde(default)]
    pub imported_at: String,
    #[serde(default)]
    pub provenance: BTreeMap<String, FieldProvenance>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct System {
    pub slug: String,
    pub name: String,
    pub category: String,
    pub launch_year: String,
    pub url: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Import {
    pub schema_version: u32,
    pub provider: String,
    pub captured_at: String,
    pub systems: Vec<System>,
    pub games: Vec<Record>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub edition_id: String,
    pub record_id: String,
    pub media_id: String,
    pub method: String,
    pub valid: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CatalogueSnapshot {
    pub records: Vec<Record>,
    pub systems: Vec<System>,
    pub links: Vec<Link>,
}

pub fn system_id(slug: &str) -> &str {
    match slug {
        "Genesis" => "genesis",
        "PS" | "PS1" => "ps1",
        "PS2" => "ps2",
        "PS3" => "ps3",
        "NES" => "nes",
        "SNES" => "snes",
        "N64" => "n64",
        "GameCube" => "gamecube",
        "GBA" => "gba",
        "GB" => "gb",
        "GBC" => "gbc",
        "PSP" => "psp",
        "Atari2600" => "atari2600",
        "Atari5200" => "atari5200",
        "SMS" => "sms",
        "Atari7800" => "atari7800",
        "TG16" | "tg16" => "pce",
        "TGCD" => "tgcd",
        "CDi" => "cdi",
        "SegaCD" => "segacd",
        "Jaguar" => "jaguar",
        "32X" => "32x",
        "Saturn" => "saturn",
        "JaguarCD" => "jaguarcd",
        "Dreamcast" => "dreamcast",
        "Xbox" => "xbox",
        "Xbox360" => "xbox360",
        "X360-D" => "xbox360-digital",
        "Wii" => "wii",
        "WiiWare" => "wiiware",
        "WiiU" => "wiiu",
        "Lynx" => "lynx",
        "GG" => "gamegear",
        "VB" => "virtualboy",
        "DS" => "ds",
        "3DS" => "3ds",
        other => other,
    }
}
fn bad(message: impl Into<String>) -> AppError {
    AppError::new("METADATA", message)
}
fn vimm_source_url(url: &str) -> bool {
    url.starts_with("https://vimm.net/vault/") || url == "https://vimm.net/vault"
}
fn axekin_source_url(url: &str) -> bool {
    url.starts_with("https://www.axekin.com/") && bounded(url)
}
fn axekin_game_url(url: &str) -> bool {
    let prefix = "https://www.axekin.com/games/";
    url.starts_with(prefix) && url.len() > prefix.len() && bounded(url)
}
fn https_url(url: &str) -> bool {
    url.starts_with("https://") && bounded(url)
}
fn numeric(id: &str) -> bool {
    !id.is_empty() && id.len() <= 20 && id.bytes().all(|c| c.is_ascii_digit())
}
fn bounded(value: &str) -> bool {
    value.len() <= 2048 && !value.contains('\0')
}

fn validate(import: &mut Import) -> Result<()> {
    match import.provider.as_str() {
        "vimm" => validate_vimm(import),
        "axekin" => validate_axekin(import),
        _ => Err(bad("Unknown catalogue provider")),
    }
}

fn validate_shell(import: &mut Import, provider: &str) -> Result<(HashSet<String>, String)> {
    if import.schema_version != 1
        || import.provider != provider
        || import.games.is_empty()
        || import.games.len() > 10_000
        || import.systems.len() > 100
        || !bounded(&import.captured_at)
    {
        return Err(bad(
            "Expected a version 1 catalogue snapshot with 1–10000 games",
        ));
    }
    Ok((HashSet::new(), now_rfc3339()))
}

fn validate_record_shell(record: &Record, ids: &mut HashSet<String>) -> Result<()> {
    if !numeric(&record.game_id)
        || record.title.trim().is_empty()
        || !ids.insert(record.game_id.clone())
        || record.media.len() > 100
    {
        return Err(bad("Invalid or duplicate game identity"));
    }
    if [
        &record.system,
        &record.title,
        &record.regions,
        &record.languages,
        &record.version,
        &record.rating,
    ]
    .iter()
    .any(|s| !bounded(s))
        || record.verified_at.as_ref().is_some_and(|s| !bounded(s))
    {
        return Err(bad("Invalid game metadata"));
    }
    if record.players.is_some_and(|n| n == 0 || n > 1000)
        || record.year.is_some_and(|n| !(1900..=2200).contains(&n))
        || [
            &record.ratings.graphics,
            &record.ratings.sound,
            &record.ratings.gameplay,
            &record.ratings.overall,
        ]
        .iter()
        .any(|s| s.is_some_and(|n| !n.is_finite() || !(0.0..=10.0).contains(&n)))
    {
        return Err(bad("Invalid player count, year or source rating"));
    }
    Ok(())
}

fn validate_media_variant(media: &Variant, axekin: bool) -> Result<()> {
    if media.regions.len() > 50
        || media.languages.len() > 100
        || media
            .regions
            .iter()
            .chain(media.languages.iter())
            .any(|s| !bounded(s))
        || media.verified_at.as_ref().is_some_and(|s| !bounded(s))
    {
        return Err(bad("Invalid variant region/language/date"));
    }
    if !numeric(&media.media_id)
        || [&media.filename, &media.version, &media.size, &media.serial]
            .iter()
            .any(|s| !bounded(s))
    {
        return Err(bad("Invalid media variant"));
    }
    if axekin && !media.download_url.is_empty() && !https_url(&media.download_url) {
        return Err(bad("Invalid mirror URL"));
    }
    for (hash, length) in [(&media.crc32, 8), (&media.md5, 32), (&media.sha1, 40)] {
        if !hash.is_empty()
            && (hash.len() != length || !hash.bytes().all(|c| c.is_ascii_hexdigit()))
        {
            return Err(bad("Invalid media checksum"));
        }
    }
    Ok(())
}

fn validate_cover(record: &Record, axekin: bool) -> Result<()> {
    if let Some(cover) = &record.cover {
        let allowed_source = if axekin {
            cover.source_url.starts_with("https://www.axekin.com/")
        } else {
            cover.source_url == "https://github.com/libretro-thumbnails/"
        };
        if cover.data_url.len() > 2_800_000 || !bounded(&cover.credit) || !allowed_source {
            return Err(bad("Invalid cover provenance or size"));
        }
        let (prefix, data) = cover
            .data_url
            .split_once(',')
            .ok_or_else(|| bad("Invalid cover data"))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|_| bad("Invalid cover encoding"))?;
        let valid = match prefix {
            "data:image/png;base64" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            "data:image/jpeg;base64" => bytes.starts_with(b"\xff\xd8\xff"),
            "data:image/avif;base64" => {
                bytes.get(4..8) == Some(b"ftyp")
                    && bytes
                        .get(8..32.min(bytes.len()))
                        .is_some_and(|b| b.windows(4).any(|w| w == b"avif" || w == b"avis"))
            }
            "data:image/webp;base64" => {
                bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP")
            }
            _ => false,
        };
        if !valid {
            return Err(bad("Cover must be a PNG, JPEG, AVIF or WebP image"));
        }
    }
    Ok(())
}

fn finalize_records(import: &mut Import, imported_at: &str) -> Result<()> {
    for record in &mut import.games {
        record.id = format!("{}:{}", import.provider, record.game_id);
        record.system = system_id(&record.system).to_string();
        record.exported_at = import.captured_at.clone();
        record.imported_at = imported_at.to_string();
        record.provenance = provenance(record)?;
    }
    Ok(())
}

fn validate_vimm(import: &mut Import) -> Result<()> {
    let (mut ids, imported_at) = validate_shell(import, "vimm")?;
    for system in &import.systems {
        if !vimm_source_url(&system.url)
            || [
                &system.slug,
                &system.name,
                &system.category,
                &system.launch_year,
            ]
            .iter()
            .any(|s| !bounded(s))
        {
            return Err(bad("Invalid system metadata"));
        }
    }
    for record in &mut import.games {
        validate_record_shell(record, &mut ids)?;
        if record.url != format!("https://vimm.net/vault/{}", record.game_id) {
            return Err(bad("Invalid game metadata or source URL"));
        }
        let mut variants = HashSet::new();
        for media in &record.media {
            validate_media_variant(media, false)?;
            if !variants.insert(media.media_id.clone()) {
                return Err(bad("Invalid media variant"));
            }
        }
        validate_cover(record, false)?;
    }
    finalize_records(import, &imported_at)
}

fn validate_axekin(import: &mut Import) -> Result<()> {
    let (mut ids, imported_at) = validate_shell(import, "axekin")?;
    for system in &import.systems {
        if !axekin_source_url(&system.url)
            || [
                &system.slug,
                &system.name,
                &system.category,
                &system.launch_year,
            ]
            .iter()
            .any(|s| !bounded(s))
        {
            return Err(bad("Invalid system metadata"));
        }
    }
    for record in &mut import.games {
        validate_record_shell(record, &mut ids)?;
        if !axekin_game_url(&record.url) {
            return Err(bad("Invalid game metadata or source URL"));
        }
        let mut variants = HashSet::new();
        for media in &record.media {
            validate_media_variant(media, true)?;
            if !variants.insert(media.media_id.clone()) {
                return Err(bad("Invalid media variant"));
            }
        }
        validate_cover(record, true)?;
    }
    finalize_records(import, &imported_at)
}

pub fn import(db: &Db, mut data: Import) -> Result<CatalogueSnapshot> {
    if serde_json::to_vec(&data)?.len() > 900_000 {
        return Err(bad("Snapshot exceeds 900 KB; import a smaller batch"));
    }
    validate(&mut data)?;
    let tx = db.conn.unchecked_transaction()?;
    for mut record in data.games {
        let previous: Option<String> = tx
            .query_row(
                "SELECT payload FROM catalogue_records WHERE id=?1",
                [&record.id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(previous) = previous {
            let mut previous: Record = serde_json::from_str(&previous)?;
            if !record.has_detail
                && previous.has_detail
                && system_id(&previous.system) == record.system
            {
                // A listing did not observe the detail fields. Only a later
                // detail snapshot can authoritatively replace/remove them.
                if previous.provenance.is_empty() {
                    previous.provenance = provenance(&previous)?;
                }
                for (key, value) in previous.provenance {
                    if detail_field(&key) {
                        record.provenance.insert(key, value);
                    }
                }
                record.has_detail = true;
                record.media = previous.media;
                record.players = previous.players;
                record.year = previous.year;
                record.ratings = previous.ratings;
                record.rating_votes = previous.rating_votes;
                record.verified_at = previous.verified_at;
                record.cover = previous.cover;
            }
        }
        tx.execute("INSERT INTO catalogue_records(id,payload) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload", params![record.id, serde_json::to_string(&record)?])?;
    }
    for system in data.systems {
        let system_key = format!("{}:{}", data.provider, system.slug);
        tx.execute("INSERT INTO catalogue_systems(id,payload) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload", params![system_key, serde_json::to_string(&system)?])?;
    }
    tx.commit()?;
    snapshot(db)
}

fn detail_field(key: &str) -> bool {
    matches!(
        key,
        "players" | "year" | "rating_votes" | "verified_at" | "cover"
    ) || key.starts_with("ratings.")
        || key.starts_with("media.")
}

fn provenance(record: &Record) -> Result<BTreeMap<String, FieldProvenance>> {
    let mut result = BTreeMap::new();
    let payload = serde_json::to_value(record)?;
    let evidence = FieldProvenance {
        source_url: record.url.clone(),
        exported_at: record.exported_at.clone(),
        imported_at: record.imported_at.clone(),
        method: "saved_snapshot".into(),
    };
    for key in [
        "title",
        "regions",
        "languages",
        "version",
        "rating",
        "players",
        "year",
        "rating_votes",
        "verified_at",
        "cover",
    ] {
        if !payload[key].is_null()
            && payload[key] != ""
            && (!detail_field(key) || record.has_detail)
        {
            result.insert(key.into(), evidence.clone());
        }
    }
    if record.has_detail {
        for (key, value) in payload["ratings"].as_object().into_iter().flatten() {
            if !value.is_null() {
                result.insert(format!("ratings.{key}"), evidence.clone());
            }
        }
        for media in &record.media {
            let fields = serde_json::to_value(media)?;
            for (key, value) in fields.as_object().into_iter().flatten() {
                if !value.is_null() && value != "" && value != &serde_json::json!([]) {
                    result.insert(format!("media.{}.{key}", media.media_id), evidence.clone());
                }
            }
        }
    }
    Ok(result)
}

fn records<T: serde::de::DeserializeOwned>(db: &Db, sql: &str) -> Result<Vec<T>> {
    let mut query = db.conn.prepare(sql)?;
    let rows = query.query_map([], |r| r.get::<_, String>(0))?;
    rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
}
pub fn snapshot(db: &Db) -> Result<CatalogueSnapshot> {
    let mut records: Vec<Record> =
        records(db, "SELECT payload FROM catalogue_records ORDER BY id")?;
    for record in &mut records {
        record.system = system_id(&record.system).to_string();
    }
    let systems = self::records(db, "SELECT payload FROM catalogue_systems ORDER BY id")?;
    let media: BTreeMap<_, _> = db
        .list_media()?
        .into_iter()
        .map(|m| (m.edition_id.clone(), m))
        .collect();
    let editions: BTreeMap<_, _> = db
        .list_editions()?
        .into_iter()
        .map(|e| (e.id.clone(), e))
        .collect();
    let mut query = db.conn.prepare("SELECT edition_id,record_id,media_id,method,local_media_id,size_bytes,mtime,source_hash FROM catalogue_links")?;
    let links = query
        .query_map([], |r| {
            let mut link = Link {
                edition_id: r.get(0)?,
                record_id: r.get(1)?,
                media_id: r.get(2)?,
                method: r.get(3)?,
                valid: false,
            };
            let local_id: String = r.get(4)?;
            let size: i64 = r.get(5)?;
            let mtime: i64 = r.get(6)?;
            let source_hash: String = r.get(7)?;
            link.valid = media.get(&link.edition_id).is_some_and(|m| {
                m.id == local_id
                    && m.size_bytes == size
                    && m.mtime == mtime
                    && file_matches(&m.path, size, mtime)
            }) && records.iter().any(|record| {
                record.id == link.record_id
                    && editions.get(&link.edition_id).is_some_and(|e| {
                        e.system == record.system && e.media_id.as_ref() == Some(&local_id)
                    })
                    && record.media.iter().any(|m| {
                        m.media_id == link.media_id
                            && (link.method != "sha1" || m.sha1.eq_ignore_ascii_case(&source_hash))
                    })
            });
            Ok(link)
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(CatalogueSnapshot {
        records,
        systems,
        links,
    })
}

fn link(db: &Db, edition_id: &str, record_id: &str, media_id: &str, method: &str) -> Result<()> {
    let edition = db
        .edition_by_id(edition_id)?
        .ok_or_else(|| bad("Installation not found"))?;
    let record = snapshot(db)?
        .records
        .into_iter()
        .find(|r| r.id == record_id)
        .ok_or_else(|| bad("Catalogue record not found"))?;
    if edition.system != record.system || !record.media.iter().any(|m| m.media_id == media_id) {
        return Err(bad("Choose a media variant for the same platform"));
    }
    let media = db
        .list_media()?
        .into_iter()
        .find(|m| Some(&m.id) == edition.media_id.as_ref())
        .ok_or_else(|| bad("Installation has no media"))?;
    if !file_matches(&media.path, media.size_bytes, media.mtime) {
        return Err(bad(
            "Installation file is unavailable or changed; scan again before linking",
        ));
    }
    let source_hash = record
        .media
        .iter()
        .find(|m| m.media_id == media_id)
        .map(|m| m.sha1.as_str())
        .unwrap_or("");
    db.conn.execute("INSERT INTO catalogue_links(edition_id,record_id,media_id,local_media_id,size_bytes,mtime,method,source_hash) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(edition_id) DO UPDATE SET record_id=excluded.record_id,media_id=excluded.media_id,local_media_id=excluded.local_media_id,size_bytes=excluded.size_bytes,mtime=excluded.mtime,method=excluded.method,source_hash=excluded.source_hash", params![edition_id,record_id,media_id,media.id,media.size_bytes,media.mtime,method,source_hash])?;
    Ok(())
}

impl Core {
    pub fn import_catalogue(&self, data: Import) -> Result<CatalogueSnapshot> {
        let result = self.with_db(|db| import(db, data))?;
        self.hub().emit(CoreEvent::LibraryChanged);
        Ok(result)
    }
    pub fn link_catalogue(&self, edition: &str, record: &str, media: &str) -> Result<()> {
        self.with_db(|db| link(db, edition, record, media, "manual"))?;
        self.hub().emit(CoreEvent::LibraryChanged);
        Ok(())
    }
    pub fn unlink_catalogue(&self, edition: &str) -> Result<()> {
        self.with_db(|db| {
            db.conn
                .execute("DELETE FROM catalogue_links WHERE edition_id=?1", [edition])?;
            Ok(())
        })?;
        self.hub().emit(CoreEvent::LibraryChanged);
        Ok(())
    }
    pub fn clear_catalogue(&self) -> Result<()> {
        self.with_db(|db| { db.conn.execute_batch("BEGIN IMMEDIATE; DELETE FROM catalogue_links; DELETE FROM catalogue_records; DELETE FROM catalogue_systems; COMMIT;")?; Ok(()) })?;
        self.hub().emit(CoreEvent::LibraryChanged);
        Ok(())
    }
    pub fn match_catalogue(&self, edition_id: &str) -> Result<Link> {
        use sha1::{Digest, Sha1};
        let (edition, media, records) = self.with_db(|db| {
            let edition = db
                .edition_by_id(edition_id)?
                .ok_or_else(|| bad("Installation not found"))?;
            let media = db
                .list_media()?
                .into_iter()
                .find(|m| Some(&m.id) == edition.media_id.as_ref())
                .ok_or_else(|| bad("Installation has no media"))?;
            Ok((edition, media, snapshot(db)?.records))
        })?;
        // Hash only by explicit request; never hash large images during browsing.
        if !matches!(media.kind.as_str(), "ps1_iso" | "ps2_iso") {
            return Err(bad("Hash matching currently supports raw ISO files only; CHD and CUE require content-aware hashing"));
        }
        let candidates: Vec<_> = records
            .iter()
            .filter(|r| r.system == edition.system)
            .flat_map(|r| {
                r.media
                    .iter()
                    .filter(|m| !m.sha1.is_empty())
                    .map(move |m| (r, m))
            })
            .collect();
        if candidates.is_empty() {
            return Err(bad("No SHA-1 metadata for this platform has been imported"));
        }
        let mut file = std::fs::File::open(&media.path)?;
        let before = file.metadata()?;
        if before.len() != media.size_bytes as u64 || before.mtime() != media.mtime {
            return Err(bad(
                "File changed since scanning; scan again before matching",
            ));
        }
        let mut hasher = Sha1::new();
        let mut buffer = [0u8; 131072];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
        let digest = format!("{:x}", hasher.finalize());
        let after = file.metadata()?;
        let current = std::fs::metadata(&media.path)?;
        if before.len() != after.len()
            || before.modified()? != after.modified()?
            || before.ino() != current.ino()
            || before.dev() != current.dev()
            || before.modified()? != current.modified()?
        {
            return Err(bad("File changed during matching; scan it again"));
        }
        let matched: Vec<_> = candidates
            .into_iter()
            .filter(|(_, m)| m.sha1.eq_ignore_ascii_case(&digest))
            .collect();
        if matched.len() != 1 {
            return Err(bad(if matched.is_empty() {
                "No exact SHA-1 match; installation was not changed"
            } else {
                "Multiple metadata variants share this hash; choose one manually"
            }));
        }
        let (record, variant) = matched[0];
        self.with_db(|db| {
            let current = db
                .edition_by_id(edition_id)?
                .ok_or_else(|| bad("Installation changed while matching"))?;
            if current.media_id != edition.media_id {
                return Err(bad("Installation changed while matching; retry"));
            }
            let source = snapshot(db)?
                .records
                .into_iter()
                .find(|r| r.id == record.id)
                .ok_or_else(|| bad("Source metadata changed while matching"))?;
            if !source
                .media
                .iter()
                .any(|m| m.media_id == variant.media_id && m.sha1.eq_ignore_ascii_case(&digest))
            {
                return Err(bad("Source metadata changed while matching; retry"));
            }
            link(db, edition_id, &record.id, &variant.media_id, "sha1")
        })?;
        self.hub().emit(CoreEvent::LibraryChanged);
        Ok(Link {
            edition_id: edition_id.into(),
            record_id: record.id.clone(),
            media_id: variant.media_id.clone(),
            method: "sha1".into(),
            valid: true,
        })
    }
    pub fn open_catalogue_page(&self, record_id: &str) -> Result<()> {
        let url = self.with_db(|db| {
            snapshot(db)?
                .records
                .into_iter()
                .find(|r| r.id == record_id)
                .map(|r| r.url)
                .ok_or_else(|| bad("Catalogue record not found"))
        })?;
        let chrome = [
            "google-chrome",
            "google-chrome-stable",
            "chromium",
            "chromium-browser",
        ]
        .iter()
        .find_map(|name| crate::runners::which(name));
        let (binary, args) = match chrome {
            Some(binary) => (binary, vec!["--new-tab".to_string(), url]),
            None => (
                crate::runners::which("xdg-open").ok_or_else(|| bad("No browser opener found"))?,
                vec![url],
            ),
        };
        let mut child = std::process::Command::new(binary)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sha1::{Digest, Sha1};

    fn fixture() -> Import {
        serde_json::from_value(json!({
            "schema_version":1,"provider":"vimm","captured_at":"2026-10-21T12:00:00Z",
            "systems":[{"slug":"PS2","name":"PlayStation 2","category":"Consoles","launch_year":"2000","url":"https://vimm.net/vault/PS2"}],
            "games":[{"game_id":"123","system":"PS2","title":"Source title","regions":"Europe","languages":"en fr","version":"1.0","rating":"8.5","url":"https://vimm.net/vault/123","has_detail":true,
                "players":1,"year":null,"ratings":{"overall":8.5},"rating_votes":2,"verified_at":"2026-10-18","cover":null,
                "media":[{"media_id":"456","filename":"game.iso","version":"1.0","size":"1 MB","serial":"SLUS-12345","crc32":"","md5":"","sha1":""}]}]
        })).unwrap()
    }
    fn library() -> (tempfile::TempDir, Core, String, Import) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("roms");
        std::fs::create_dir(&root).unwrap();
        let bytes =
            crate::iso::build_test_iso(&[("SYSTEM.CNF", b"BOOT2 = cdrom0:\\SLUS_123.45;1\n")]);
        std::fs::write(root.join("game.iso"), &bytes).unwrap();
        let core = Core::open_isolated(Some(temp.path().join("data")), false).unwrap();
        core.add_root("ps2", root, None).unwrap();
        core.scan(None, None).unwrap();
        let id = core.snapshot().unwrap().editions[0].id.clone();
        let mut data = fixture();
        data.games[0].media[0].sha1 = format!("{:x}", Sha1::digest(bytes));
        (temp, core, id, data)
    }

    #[test]
    fn listing_refresh_keeps_detail_evidence_and_links_but_full_detail_can_remove_it() {
        let (_temp, core, id, mut data) = library();
        data.games[0].cover = Some(Cover {
            data_url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/l9sAAAAASUVORK5CYII=".into(),
            credit: "Saved source credit".into(),
            source_url: "https://github.com/libretro-thumbnails/".into(),
        });
        core.import_catalogue(data.clone()).unwrap();
        core.match_catalogue(&id).unwrap();
        let original = core.snapshot().unwrap().catalogue.records.remove(0);
        let mut listing = data.clone();
        listing.captured_at = "2026-10-22T12:00:00Z".into();
        let game = &mut listing.games[0];
        game.title = "Updated listing title".into();
        game.has_detail = false;
        game.media.clear();
        game.players = None;
        game.ratings = Ratings::default();
        game.verified_at = None;
        game.cover = None;
        let result = core.import_catalogue(listing).unwrap();
        let current = &result.records[0];
        assert_eq!(current.title, "Updated listing title");
        assert_eq!(current.media.len(), 1);
        assert_eq!(current.players, Some(1));
        assert_eq!(
            current.cover.as_ref().unwrap().credit,
            "Saved source credit"
        );
        assert_eq!(
            current.provenance["cover"].exported_at,
            original.exported_at
        );
        assert!(result.links[0].valid);
        assert_eq!(
            current.provenance["players"].exported_at,
            original.exported_at
        );
        assert_eq!(
            current.provenance["media.456.sha1"].exported_at,
            original.exported_at
        );
        assert_eq!(current.provenance["title"].exported_at, current.exported_at);
        let mut detail = data;
        detail.games[0].media.clear();
        detail.games[0].players = None;
        let result = core.import_catalogue(detail).unwrap();
        assert!(result.records[0].media.is_empty());
        assert!(!result.records[0].provenance.contains_key("players"));
        assert!(!result.links[0].valid);
    }

    #[test]
    fn old_tg16_records_link_to_local_pce_installations() {
        let (_temp, core, id, mut data) = library();
        core.with_db(|db| {
            db.conn
                .execute("UPDATE editions SET system='pce' WHERE id=?1", [&id])?;
            Ok(())
        })
        .unwrap();
        data.games[0].system = "TG16".into();
        let result = core.import_catalogue(data).unwrap();
        assert_eq!(result.records[0].system, "pce");
        // Read-time normalization keeps existing v5 payloads usable too.
        core.with_db(|db| {
            let mut legacy = result.records[0].clone();
            legacy.system = "tg16".into();
            db.conn.execute(
                "UPDATE catalogue_records SET payload=?1",
                [serde_json::to_string(&legacy)?],
            )?;
            Ok(())
        })
        .unwrap();
        core.link_catalogue(&id, "vimm:123", "456").unwrap();
        assert!(core.snapshot().unwrap().catalogue.links[0].valid);
    }

    fn axekin_fixture() -> Import {
        serde_json::from_value(json!({
            "schema_version":1,"provider":"axekin","captured_at":"2026-10-21T12:00:00Z",
            "systems":[{"slug":"switch","name":"Nintendo Switch","category":"Consoles","launch_year":"","url":"https://www.axekin.com/games?platform=switch"}],
            "games":[{"game_id":"401151","system":"switch","title":"Nightshade","regions":"JP","languages":"","version":"1.0.1","rating":"8.4","url":"https://www.axekin.com/games/010042300C4F6000","has_detail":true,
                "players":null,"year":2018,"ratings":{"overall":8.4},"rating_votes":null,"verified_at":null,"cover":null,
                "media":[{"media_id":"1","filename":"pixeldrain.com","version":"base","size":"3 GB","serial":"","crc32":"","md5":"","sha1":"","download_url":"https://pixeldrain.com/u/Y3M5U2GA"}]}]
        })).unwrap()
    }

    #[test]
    fn axekin_import_accepts_mirror_urls() {
        let db = Db::open_in_memory().unwrap();
        let snap = import(&db, axekin_fixture()).unwrap();
        assert_eq!(snap.records[0].id, "axekin:401151");
        assert_eq!(
            snap.records[0].media[0].download_url,
            "https://pixeldrain.com/u/Y3M5U2GA"
        );
        assert_eq!(snap.systems[0].slug, "switch");
    }

    #[test]
    fn imports_are_idempotent_and_do_not_create_installations() {
        let db = Db::open_in_memory().unwrap();
        import(&db, fixture()).unwrap();
        let result = import(&db, fixture()).unwrap();
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.systems.len(), 1);
        assert_eq!(result.records[0].system, "ps2");
        assert!(db.list_editions().unwrap().is_empty());
    }
    #[test]
    fn invalid_import_is_atomic_and_rejects_scripts_urls_and_bad_hashes() {
        let db = Db::open_in_memory().unwrap();
        let mut data = fixture();
        data.games[0].url = "https://evil.test/vault/123".into();
        assert!(import(&db, data).is_err());
        let mut data = fixture();
        data.games[0].media[0].sha1 = "not-a-hash".into();
        assert!(import(&db, data).is_err());
        let mut data = fixture();
        data.games[0].cover = Some(Cover {
            data_url: "data:image/svg+xml;base64,PHN2Zz4=".into(),
            credit: "source".into(),
            source_url: "https://github.com/libretro-thumbnails/".into(),
        });
        assert!(import(&db, data).is_err());
        let mut data = fixture();
        data.games.push(data.games[0].clone());
        assert!(import(&db, data).is_err());
        assert!(snapshot(&db).unwrap().records.is_empty());
    }
    #[test]
    fn exact_matching_reimport_and_unlink_preserve_personal_library() {
        let (_temp, core, id, data) = library();
        core.set_favourite(&id, true).unwrap();
        core.set_display_title(&id, "My title").unwrap();
        core.with_db(|db| {
            db.conn.execute(
                "UPDATE editions SET playtime_seconds=120,play_count=2 WHERE id=?1",
                [&id],
            )?;
            Ok(())
        })
        .unwrap();
        core.import_catalogue(data.clone()).unwrap();
        let matched = core.match_catalogue(&id).unwrap();
        assert_eq!(matched.method, "sha1");
        assert!(core.snapshot().unwrap().catalogue.links[0].valid);
        core.import_catalogue(data).unwrap();
        core.unlink_catalogue(&id).unwrap();
        let snap = core.snapshot().unwrap();
        assert!(snap.catalogue.links.is_empty());
        assert_eq!(snap.editions.len(), 1);
        assert_eq!(snap.editions[0].title, "My title");
        assert!(snap.editions[0].favorite);
        assert_eq!(snap.editions[0].playtime_seconds, 120);
        assert_eq!(snap.editions[0].play_count, 2);
    }
    #[test]
    fn manual_link_checks_platform_and_invalidates_changed_media() {
        let (_temp, core, id, data) = library();
        core.import_catalogue(data).unwrap();
        assert!(core.link_catalogue(&id, "vimm:123", "999").is_err());
        core.link_catalogue(&id, "vimm:123", "456").unwrap();
        let path = core.snapshot().unwrap().media[0].path.clone();
        std::fs::write(path, b"replaced file").unwrap();
        assert!(!core.snapshot().unwrap().catalogue.links[0].valid);
        assert!(core.match_catalogue(&id).is_err());
        assert!(core.link_catalogue(&id, "vimm:123", "456").is_err());
        let mut wrong = fixture();
        wrong.games[0].system = "Genesis".into();
        core.import_catalogue(wrong).unwrap();
        assert!(core.link_catalogue(&id, "vimm:123", "456").is_err());
    }
    #[test]
    fn no_hash_match_and_ambiguous_matches_do_not_link() {
        let (_temp, core, id, mut data) = library();
        let mut duplicate = data.games[0].clone();
        duplicate.game_id = "124".into();
        duplicate.url = "https://vimm.net/vault/124".into();
        data.games.push(duplicate);
        core.import_catalogue(data.clone()).unwrap();
        assert!(core
            .match_catalogue(&id)
            .unwrap_err()
            .message
            .contains("Multiple"));
        assert!(core.snapshot().unwrap().catalogue.links.is_empty());
        for game in &mut data.games {
            game.media[0].sha1 = "0".repeat(40);
        }
        core.import_catalogue(data).unwrap();
        assert!(core
            .match_catalogue(&id)
            .unwrap_err()
            .message
            .contains("No exact"));
        assert!(core.snapshot().unwrap().catalogue.links.is_empty());
    }
    #[test]
    fn migration_and_restart_preserve_library_and_catalogue() {
        let (temp, core, id, data) = library();
        core.set_favourite(&id, true).unwrap();
        core.with_db(|db| { db.conn.execute_batch("DROP TABLE catalogue_links; DROP TABLE catalogue_records; DROP TABLE catalogue_systems; PRAGMA user_version=4;")?; Ok(()) }).unwrap();
        drop(core);
        let core = Core::open_isolated(Some(temp.path().join("data")), false).unwrap();
        assert!(core.snapshot().unwrap().editions[0].favorite);
        core.import_catalogue(data).unwrap();
        core.link_catalogue(&id, "vimm:123", "456").unwrap();
        drop(core);
        let core = Core::open_isolated(Some(temp.path().join("data")), false).unwrap();
        assert_eq!(core.snapshot().unwrap().catalogue.records.len(), 1);
        assert!(core.snapshot().unwrap().catalogue.links[0].valid);
        core.clear_catalogue().unwrap();
        let snap = core.snapshot().unwrap();
        assert!(snap.catalogue.records.is_empty());
        assert!(snap.editions[0].favorite);
    }
}
