//! Cover art for cartridge and disc games without a serial.
//!
//! 1. Hash the ROM (CRC-32, with and without a copier/iNES header) and look the
//!    hash up in the libretro-database No-Intro lists to get the official game
//!    name, so a messy file name does not matter.
//! 2. Fetch that game's box art from libretro-thumbnails.
//! 3. For anything still without a cover, search SteamGridDB (needs the user's
//!    key) and accept only a result whose name matches closely.
//!
//! Nothing here blocks Play, nothing replaces a cover that already exists, and
//! every failure is just a skipped game. Sources: libretro-database and
//! libretro-thumbnails (public, no account) and SteamGridDB.

use crate::error::{AppError, Result};
use crate::http;
use crate::model::{ArtworkReport, Edition};
use crate::Core;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, SystemTime};

pub(crate) const DB_BASE: &str =
    "https://raw.githubusercontent.com/libretro/libretro-database/master/metadat";
const THUMB_BASE: &str = "https://thumbnails.libretro.com";
const MAX_HASH_BYTES: u64 = 1024 * 1024 * 1024;
pub(crate) const DAT_MAX_BYTES: u64 = 40 * 1024 * 1024;
const DAT_MAX_AGE: Duration = Duration::from_secs(60 * 60 * 24 * 30);

/// How one system is looked up. `dat` is a No-Intro list; None means only the
/// file/title name is tried against the thumbnails.
pub(crate) struct Source {
    pub(crate) system: &'static str,
    pub(crate) thumbs: &'static str,
    pub(crate) dat: Option<&'static str>,
}

const SOURCES: &[Source] = &[
    Source {
        system: "genesis",
        thumbs: "Sega - Mega Drive - Genesis",
        dat: Some("Sega - Mega Drive - Genesis"),
    },
    Source {
        system: "nes",
        thumbs: "Nintendo - Nintendo Entertainment System",
        dat: Some("Nintendo - Nintendo Entertainment System"),
    },
    Source {
        system: "snes",
        thumbs: "Nintendo - Super Nintendo Entertainment System",
        dat: Some("Nintendo - Super Nintendo Entertainment System"),
    },
    Source {
        system: "gb",
        thumbs: "Nintendo - Game Boy",
        dat: Some("Nintendo - Game Boy"),
    },
    Source {
        system: "gbc",
        thumbs: "Nintendo - Game Boy Color",
        dat: Some("Nintendo - Game Boy Color"),
    },
    Source {
        system: "pce",
        thumbs: "NEC - PC Engine - TurboGrafx 16",
        dat: Some("NEC - PC Engine - TurboGrafx 16"),
    },
    Source {
        system: "n64",
        thumbs: "Nintendo - Nintendo 64",
        dat: Some("Nintendo - Nintendo 64"),
    },
    Source {
        system: "ds",
        thumbs: "Nintendo - Nintendo DS",
        dat: Some("Nintendo - Nintendo DS"),
    },
    Source {
        system: "dreamcast",
        thumbs: "Sega - Dreamcast",
        dat: None,
    },
    Source {
        system: "ps1",
        thumbs: "Sony - PlayStation",
        dat: None,
    },
    Source {
        system: "ps2",
        thumbs: "Sony - PlayStation 2",
        dat: None,
    },
];

pub(crate) fn source_for(system: &str) -> Option<&'static Source> {
    SOURCES.iter().find(|s| s.system == system)
}

pub(crate) fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// libretro-thumbnails replaces these characters in file names with `_`.
pub fn thumb_name(name: &str) -> String {
    name.chars()
        .map(|c| if "&*/:`<>?\\|\"".contains(c) { '_' } else { c })
        .collect()
}

/// CRC-32 → game name from a No-Intro list in clrmamepro format.
pub fn parse_dat(text: &str) -> HashMap<u32, String> {
    let mut map = HashMap::new();
    let mut name: Option<String> = None;
    for line in text.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("name \"") {
            name = rest.strip_suffix('"').map(str::to_string);
        } else if l.starts_with("rom (") {
            let mut tokens = l.split_whitespace();
            while let Some(t) = tokens.next() {
                if t == "crc" {
                    if let (Some(hex), Some(n)) = (tokens.next(), name.as_ref()) {
                        if let Ok(crc) = u32::from_str_radix(hex, 16) {
                            map.entry(crc).or_insert_with(|| n.clone());
                        }
                    }
                    break;
                }
            }
        }
    }
    map
}

/// CRC-32 of a file, plus the CRC with a leading header removed when one is
/// recognised (iNES 16 bytes, SNES copier 512 bytes). Both are candidates
/// because the lists hash headerless data for some systems.
pub fn crcs_of(path: &Path) -> Result<Vec<u32>> {
    let size = fs::metadata(path)?.len();
    if size == 0 || size > MAX_HASH_BYTES {
        return Ok(vec![]);
    }
    let mut f = File::open(path)?;
    let mut buf = vec![0u8; 1 << 20];
    let mut full = flate2::Crc::new();
    let mut skipped = flate2::Crc::new();
    let mut seen: u64 = 0;
    let mut skip: Option<u64> = None;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        if seen == 0 {
            skip = if n >= 16 && &buf[..4] == b"NES\x1a" {
                Some(16)
            } else if size % 1024 == 512 {
                Some(512)
            } else {
                None
            };
        }
        full.update(&buf[..n]);
        if let Some(k) = skip {
            let start = k.saturating_sub(seen).min(n as u64) as usize;
            if seen + n as u64 > k {
                skipped.update(&buf[start..n]);
            }
        }
        seen += n as u64;
    }
    let mut out = vec![full.sum()];
    if skip.is_some() {
        out.push(skipped.sum());
    }
    Ok(out)
}

#[derive(Serialize, Deserialize, Default)]
pub(crate) struct CrcCache(pub BTreeMap<String, (u64, i64, Vec<u32>)>);

pub(crate) fn load<T: for<'a> Deserialize<'a> + Default>(p: &Path) -> T {
    fs::read(p)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub(crate) fn save<T: Serialize>(p: &Path, v: &T) {
    if let Ok(b) = serde_json::to_vec(v) {
        let _ = fs::write(p, b);
    }
}

pub(crate) fn fresh(path: &Path) -> bool {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < DAT_MAX_AGE)
}

pub(crate) fn load_dat(dir: &Path, name: &str) -> Result<HashMap<u32, String>> {
    let file = dir.join(format!("{}.dat", thumb_name(name)));
    if !fresh(&file) {
        let url = format!("{DB_BASE}/no-intro/{}.dat", encode(name));
        let text = http::get_text_limited(&url, DAT_MAX_BYTES)?;
        fs::create_dir_all(dir)?;
        fs::write(&file, text)?;
    }
    Ok(parse_dat(&fs::read_to_string(&file)?))
}

/// Image bytes with a recognised magic number and a sane size, and their extension.
pub fn image_kind(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() < 64 || bytes.len() as u64 > http::COVER_MAX_BYTES {
        return None;
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.ends_with(b"IEND\xaeB`\x82") {
        Some("png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) && bytes.ends_with(&[0xff, 0xd9]) {
        Some("jpg")
    } else if bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

/// Drop `(USA)`, `[!]` and similar so a title can be searched by name.
pub fn plain_title(title: &str) -> String {
    let mut out = String::new();
    let mut depth = 0i32;
    for c in title.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = (depth - 1).max(0),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    out.trim_matches(|c: char| c == '-' || c.is_whitespace())
        .to_string()
}

fn squash(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// A search result is accepted only when its name is the same game.
pub fn names_agree(wanted: &str, found: &str) -> bool {
    let (a, b) = (squash(&plain_title(wanted)), squash(found));
    if a.len() < 4 || b.len() < 4 {
        return false;
    }
    let (short, long) = if a.len() <= b.len() {
        (&a, &b)
    } else {
        (&b, &a)
    };
    // "Tetris" must not stand in for "Tetris Effect".
    // ...and "Underground" must not stand in for "Underground 2".
    a == b
        || (long.contains(short.as_str())
            && short.len() * 10 >= long.len() * 8
            && !long
                .replacen(short.as_str(), "", 1)
                .chars()
                .any(|c| c.is_ascii_digit()))
}

/// No-Intro tags that mark homebrew, prototypes and unlicensed releases.
/// Those share names with famous games, so a name search would mislead.
pub fn looks_retail(official: &str) -> bool {
    let l = official.to_lowercase();
    ![
        "(world)",
        "(proto",
        "(beta",
        "(demo",
        "(homebrew",
        "(aftermarket",
        "(unl",
        "(pirate",
        "(hack",
        "(sample",
        "(kiosk",
    ]
    .iter()
    .any(|t| l.contains(t))
}

fn thumbnail_names(name: &str) -> Vec<&str> {
    let mut names = vec![name];
    // No-Intro may append an alternate compilation title that the thumbnail
    // repository has not adopted. Keep the region and revision intact.
    if let Some(without_close) = name.strip_suffix(')') {
        if let Some((base, annotation)) = without_close.rsplit_once(" (") {
            if annotation.contains(" ~ ") && !annotation.contains('(') {
                names.push(base);
            }
        }
    }
    names
}

fn thumbnail(system: &Source, name: &str) -> Option<(Vec<u8>, &'static str)> {
    for kind in ["Named_Boxarts", "Named_Titles", "Named_Snaps"] {
        for name in thumbnail_names(name) {
            let url = format!(
                "{THUMB_BASE}/{}/{kind}/{}.png",
                encode(system.thumbs),
                encode(&thumb_name(name))
            );
            match http::get_bytes(&url) {
                Ok(bytes) if image_kind(&bytes).is_some() => {
                    return Some((bytes, "libretro-thumbnails"))
                }
                Err(e) if e.code == "NOT_FOUND" => continue,
                _ => continue,
            }
        }
    }
    None
}

fn steamgriddb(key: &str, title: &str) -> Option<(Vec<u8>, &'static str)> {
    let term = plain_title(title);
    if term.len() < 3 {
        return None;
    }
    let search = http::get_json_bearer(
        &format!(
            "https://www.steamgriddb.com/api/v2/search/autocomplete/{}",
            encode(&term)
        ),
        key,
    )
    .ok()?;
    let game = search["data"]
        .as_array()?
        .iter()
        .find(|g| g["name"].as_str().is_some_and(|n| names_agree(&term, n)))?;
    let id = game["id"].as_u64()?;
    steamgriddb_id(key, id)
}
pub(crate) fn steamgriddb_id(key: &str, id: u64) -> Option<(Vec<u8>, &'static str)> {
    let grids = http::get_json_bearer(
        &format!("https://www.steamgriddb.com/api/v2/grids/game/{id}?dimensions=600x900&types=static&limit=3"),
        key,
    )
    .ok()?;
    for g in grids["data"].as_array()? {
        let Some(url) = g["url"].as_str().filter(|u| u.starts_with("https://")) else {
            continue;
        };
        if let Ok(bytes) = http::get_bytes(url) {
            if image_kind(&bytes).is_some() {
                return Some((bytes, "steamgriddb"));
            }
        }
    }
    None
}

pub(crate) fn store(
    core: &Core,
    art_dir: &Path,
    e: &Edition,
    bytes: &[u8],
    source: &str,
) -> Result<()> {
    let ext = image_kind(bytes).ok_or_else(|| AppError::new("NETWORK", "Not a usable image"))?;
    let dest = art_dir.join(format!("{}.{ext}", e.id));
    let mut tmp = tempfile::NamedTempFile::new_in(art_dir)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(&dest)
        .map_err(|err| AppError::new("IO", err.to_string()))?;
    core.with_db(|db| {
        db.upsert_artwork(
            &e.id,
            "cover",
            &dest.to_string_lossy(),
            source,
            &crate::db::now_rfc3339(),
        )
    })
}

struct Task<'a> {
    edition: &'a Edition,
    src: &'static Source,
    /// Name to try against the thumbnails and SteamGridDB.
    name: String,
    /// Whether SteamGridDB may be asked: only for games with an official name
    /// or disc games. Public-domain and homebrew ROMs share names with famous
    /// games, and a wrong cover is worse than none.
    sgdb: bool,
}

const MISS_TTL_SECS: u64 = 14 * 24 * 3600;

pub(crate) fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn lookup(task: &Task, key: Option<&str>) -> Option<(Vec<u8>, &'static str)> {
    thumbnail(task.src, &task.name).or_else(|| {
        key.filter(|_| task.sgdb)
            .and_then(|k| steamgriddb(k, &task.name))
    })
}

/// Fill in missing covers. Safe to call often: games that already have a cover
/// are skipped, hashes and lists are cached, and games that found nothing are
/// not asked again for two weeks (unless one game is requested by name).
pub fn run(core: &Core, edition_id: Option<&str>, report: &mut ArtworkReport) {
    let Ok(snap) = core.snapshot() else { return };
    let cache = core.paths.cache_dir.clone();
    let (art_dir, dat_dir) = (cache.join("art"), cache.join("libretro"));
    if fs::create_dir_all(&art_dir).is_err() {
        return;
    }
    let wanted: Vec<&Edition> = snap
        .editions
        .iter()
        .filter(|e| (!e.missing || e.wanted.is_some()) && edition_id.is_none_or(|id| id == e.id))
        .filter(|e| !e.cover_path.as_ref().is_some_and(|p| p.is_file()))
        .collect();
    let (crc_file, names_file, miss_file) = (
        cache.join("romcrc.json"),
        cache.join("romnames.json"),
        cache.join("romart-miss.json"),
    );
    let mut crc_cache: CrcCache = load(&crc_file);
    let mut names: BTreeMap<String, String> = load(&names_file);
    let mut misses: BTreeMap<String, u64> = load(&miss_file);
    let mut dats: HashMap<&str, Option<HashMap<u32, String>>> = HashMap::new();
    let mut tasks: Vec<Task> = Vec::new();
    let now = now_secs();

    for e in wanted {
        if edition_id.is_none()
            && misses
                .get(&e.id)
                .is_some_and(|t| now.saturating_sub(*t) < MISS_TTL_SECS)
        {
            report.skipped += 1;
            continue;
        }
        let Some(src) = source_for(&e.system) else {
            report.skipped += 1;
            continue;
        };
        let mut official: Option<String> = None;
        if let Some(dat) = src.dat {
            if let Some(m) = snap
                .media
                .iter()
                .find(|m| m.edition_id == e.id && m.available)
            {
                let map = dats
                    .entry(dat)
                    .or_insert_with(|| load_dat(&dat_dir, dat).ok());
                if let Some(map) = map {
                    let key = m.path.to_string_lossy().into_owned();
                    let stamp = (m.size_bytes, m.mtime);
                    let crcs = match crc_cache.0.get(&key) {
                        Some((s, t, c)) if (*s, *t) == stamp => c.clone(),
                        _ => {
                            let c = crcs_of(&m.path).unwrap_or_default();
                            crc_cache.0.insert(key, (stamp.0, stamp.1, c.clone()));
                            c
                        }
                    };
                    official = crcs.iter().find_map(|c| map.get(c).cloned());
                }
            } else if e.wanted.is_some() {
                // No file to hash: find the typed title in the official list.
                let map = dats
                    .entry(dat)
                    .or_insert_with(|| load_dat(&dat_dir, dat).ok());
                let title = e.display_title.as_deref().unwrap_or(&e.scan_title);
                official = map
                    .as_ref()
                    .and_then(|m| crate::wanted::best_match(m, title))
                    .map(|(_, name)| name);
            }
        }
        if let Some(n) = &official {
            names.insert(e.id.clone(), n.clone());
        }
        let disc = src.dat.is_none();
        let name = match (&official, disc) {
            (Some(n), _) => n.clone(),
            (None, true) => e
                .display_title
                .clone()
                .unwrap_or_else(|| e.scan_title.clone()),
            // A cartridge the lists do not know: a hack, hack-free homebrew
            // or public-domain ROM. No cover rather than a wrong one.
            (None, false) => {
                misses.insert(e.id.clone(), now);
                report.skipped += 1;
                continue;
            }
        };
        tasks.push(Task {
            edition: e,
            src,
            name,
            sgdb: official.as_deref().is_some_and(looks_retail) || disc,
        });
    }

    // Network phase: a few lookups at a time.
    let key = core.key("steamgriddb").ok().flatten();
    let results: Vec<Option<(Vec<u8>, &'static str)>> = {
        let next = std::sync::atomic::AtomicUsize::new(0);
        let out = std::sync::Mutex::new(vec![None; tasks.len()]);
        std::thread::scope(|scope| {
            for _ in 0..6 {
                scope.spawn(|| loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some(task) = tasks.get(i) else { break };
                    let found = lookup(task, key.as_deref());
                    out.lock().unwrap_or_else(|e| e.into_inner())[i] = found;
                });
            }
        });
        out.into_inner().unwrap_or_else(|e| e.into_inner())
    };
    for (task, found) in tasks.iter().zip(results) {
        match found {
            Some((bytes, label)) => match store(core, &art_dir, task.edition, &bytes, label) {
                Ok(()) => {
                    report.fetched += 1;
                    misses.remove(&task.edition.id);
                }
                Err(_) => report.failed += 1,
            },
            None => {
                misses.insert(task.edition.id.clone(), now);
                report.skipped += 1;
            }
        }
    }
    save(&crc_file, &crc_cache);
    save(&names_file, &names);
    save(&miss_file, &misses);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dat_maps_crc_to_game_name() {
        let dat = "clrmamepro (\n\tname \"X\"\n)\n\ngame (\n\tname \"Advance Wars (USA)\"\n\tregion \"USA\"\n\trom ( name \"Advance Wars (USA).nds\" size 1 crc EAC313F3 md5 AA sha1 BB )\n)\n";
        let m = parse_dat(dat);
        assert_eq!(
            m.get(&0xEAC313F3).map(String::as_str),
            Some("Advance Wars (USA)")
        );
    }

    #[test]
    fn thumbnail_names_follow_libretro_rules() {
        assert_eq!(
            thumb_name("Star Wars: Rogue Squadron / A&B"),
            "Star Wars_ Rogue Squadron _ A_B"
        );
    }

    #[test]
    fn crc_is_taken_with_and_without_an_ines_header() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("g.nes");
        let body = vec![7u8; 4096];
        let mut file = b"NES\x1a".to_vec();
        file.extend(vec![0u8; 12]);
        file.extend(&body);
        fs::write(&p, &file).unwrap();
        let crcs = crcs_of(&p).unwrap();
        let mut h = flate2::Crc::new();
        h.update(&body);
        assert_eq!(crcs.len(), 2);
        assert_eq!(crcs[1], h.sum());
        let mut all = flate2::Crc::new();
        all.update(&file);
        assert_eq!(crcs[0], all.sum());
    }

    #[test]
    fn plain_files_have_one_crc_and_empty_files_none() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("g.gb");
        fs::write(&p, vec![1u8; 2048]).unwrap();
        assert_eq!(crcs_of(&p).unwrap().len(), 1);
        fs::write(&p, b"").unwrap();
        assert!(crcs_of(&p).unwrap().is_empty());
    }

    #[test]
    fn images_are_recognised_by_content() {
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend(vec![0u8; 80]);
        png.extend(b"IEND\xaeB`\x82");
        assert_eq!(image_kind(&png), Some("png"));
        assert_eq!(
            image_kind(
                b"<html>not an image, a long enough error page body.....................</html>"
            ),
            None
        );
        assert_eq!(image_kind(&[0u8; 10]), None);
    }

    #[test]
    fn search_results_must_name_the_same_game() {
        assert_eq!(
            thumbnail_names(
                "Sonic The Hedgehog 2 (World) (Rev B) (Sonic Compilation ~ Sonic Classics)"
            ),
            vec![
                "Sonic The Hedgehog 2 (World) (Rev B) (Sonic Compilation ~ Sonic Classics)",
                "Sonic The Hedgehog 2 (World) (Rev B)"
            ]
        );
        assert_eq!(
            thumbnail_names("Sonic The Hedgehog 2 (World) (Rev A)"),
            vec!["Sonic The Hedgehog 2 (World) (Rev A)"]
        );
        let source = source_for("genesis").unwrap();
        assert_eq!(source.dat, Some("Sega - Mega Drive - Genesis"));
        assert_eq!(source.thumbs, "Sega - Mega Drive - Genesis");
        assert_eq!(plain_title("Crazy Taxi (USA) [!]"), "Crazy Taxi");
        assert!(names_agree("Crazy Taxi (USA)", "Crazy Taxi"));
        assert!(names_agree(
            "Grand Theft Auto - Vice City (USA) (v4.00)",
            "Grand Theft Auto: Vice City"
        ));
        assert!(!names_agree("Crazy Taxi", "Crazy Machines 2"));
        assert!(!names_agree("Tetris", "Tetris Effect"));
        assert!(!names_agree(
            "Need for Speed - Underground (USA)",
            "Need for Speed: Underground 2"
        ));
        assert!(names_agree(
            "Need for Speed - Underground (USA)",
            "Need for Speed: Underground"
        ));
        assert!(looks_retail("Super Mario Bros. (USA)"));
        assert!(!looks_retail("Pitfall! (World) (Homebrew)"));
        assert!(!looks_retail("Alter Ego (World)"));
        assert!(!names_agree("Go", "Gone Home"));
    }
}
