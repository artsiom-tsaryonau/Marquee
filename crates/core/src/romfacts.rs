//! Genre, year, developer, publisher, player count and description.
//!
//! * **libretro-database** (`metadat/genre|releaseyear|developer|publisher`),
//!   keyed by the same ROM checksum as the cover lookup. Public, no account.
//! * **TheGamesDB** (the user's own key) for descriptions and player counts, and
//!   to fill what libretro lacks. Its free allowance is about 1,000 lookups a
//!   month, so it is asked only about games with an official retail name (or disc
//!   games), at most a few dozen per run, once per game.
//!
//! A game that is not found simply has no facts. A name search is accepted only
//! when the result is the same game (see `romart::names_agree`).

use crate::db::Db;
use crate::error::Result;
use crate::http;
use crate::model::{Edition, GameInfo};
use crate::romart::{self, CrcCache, Source};
use crate::Core;
use rusqlite::{params, Connection};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};
use std::sync::Mutex;

const TGDB: &str = "https://api.thegamesdb.net/v1";
const FIELDS: [&str; 4] = ["genre", "releaseyear", "developer", "publisher"];
/// Looked-up games are not asked about again for this long.
const RETRY_SECS: u64 = 7 * 24 * 3600;
/// At most this many TheGamesDB lookups per run.
const TGDB_BUDGET: usize = 500;
/// Stop asking when the provider says this little of the month is left.
const TGDB_RESERVE: i64 = 25;
const DESCRIPTION_MAX: usize = 4000;

pub fn ensure_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS edition_info (
            edition_id TEXT PRIMARY KEY,
            genre TEXT,
            year INTEGER,
            developer TEXT,
            publisher TEXT,
            players INTEGER,
            description TEXT,
            source TEXT NOT NULL DEFAULT '',
            fetched_at TEXT NOT NULL DEFAULT ''
        );",
    )?;
    Ok(())
}

pub fn list(db: &Db) -> Result<BTreeMap<String, GameInfo>> {
    let mut stmt = db.conn.prepare(
        "SELECT edition_id, genre, year, developer, publisher, players, description, source FROM edition_info",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            GameInfo {
                genre: r.get(1)?,
                year: r.get(2)?,
                developer: r.get(3)?,
                publisher: r.get(4)?,
                players: r.get(5)?,
                description: r.get(6)?,
                source: r.get(7)?,
            },
        ))
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn upsert(db: &Db, edition_id: &str, info: &GameInfo) -> Result<()> {
    db.conn.execute(
        "INSERT INTO edition_info(edition_id, genre, year, developer, publisher, players, description, source, fetched_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(edition_id) DO UPDATE SET
            genre = excluded.genre, year = excluded.year, developer = excluded.developer,
            publisher = excluded.publisher, players = excluded.players,
            description = excluded.description, source = excluded.source,
            fetched_at = excluded.fetched_at",
        params![
            edition_id,
            info.genre,
            info.year,
            info.developer,
            info.publisher,
            info.players,
            info.description,
            info.source,
            crate::db::now_rfc3339()
        ],
    )?;
    Ok(())
}

/// CRC-32 → value for one field of a libretro metadata list
/// (`game ( comment "…" genre "Action" rom ( crc 3577AB04 ) )`).
pub fn parse_meta(text: &str, field: &str) -> HashMap<u32, String> {
    let prefix = format!("{field} \"");
    let mut map = HashMap::new();
    let mut value: Option<String> = None;
    for line in text.lines() {
        let l = line.trim();
        if l == "game (" {
            value = None;
        } else if let Some(rest) = l.strip_prefix(&prefix) {
            value = rest.strip_suffix('"').map(str::to_string);
        } else if l.starts_with("rom (") {
            let mut tokens = l.split_whitespace();
            while let Some(t) = tokens.next() {
                if t == "crc" {
                    if let (Some(hex), Some(v)) = (tokens.next(), value.take()) {
                        if let Ok(crc) = u32::from_str_radix(hex, 16) {
                            if !v.trim().is_empty() {
                                map.entry(crc).or_insert(v);
                            }
                        }
                    }
                    break;
                }
            }
        }
    }
    map
}

fn load_meta(dir: &Path, field: &str, list: &str) -> HashMap<u32, String> {
    let file = dir.join(format!("meta-{field}-{}.dat", romart::thumb_name(list)));
    if !romart::fresh(&file) {
        let url = format!("{}/{field}/{}.dat", romart::DB_BASE, romart::encode(list));
        match http::get_text_limited(&url, romart::DAT_MAX_BYTES) {
            Ok(text) => {
                let _ = fs::create_dir_all(dir);
                let _ = fs::write(&file, text);
            }
            // A list that does not exist for this system is remembered as empty.
            Err(e) if e.code == "NOT_FOUND" => {
                let _ = fs::create_dir_all(dir);
                let _ = fs::write(&file, "");
            }
            Err(_) => return HashMap::new(),
        }
    }
    fs::read_to_string(&file)
        .map(|t| parse_meta(&t, field))
        .unwrap_or_default()
}

/// What libretro knows about every game of one system, by checksum.
struct SystemFacts {
    names: HashMap<u32, String>,
    genre: HashMap<u32, String>,
    year: HashMap<u32, String>,
    developer: HashMap<u32, String>,
    publisher: HashMap<u32, String>,
}

fn load_system(dir: &Path, list: &str) -> Option<SystemFacts> {
    let names = romart::load_dat(dir, list).ok()?;
    let [genre, year, developer, publisher] = FIELDS.map(|f| load_meta(dir, f, list));
    Some(SystemFacts {
        names,
        genre,
        year,
        developer,
        publisher,
    })
}

fn year_of(text: &str) -> Option<i64> {
    let y: i64 = text.trim().get(..4)?.parse().ok()?;
    (1950..=2100).contains(&y).then_some(y)
}

pub(crate) fn tgdb_platform(system: &str) -> Option<u32> {
    Some(match system {
        "nes" => 7,
        "snes" => 6,
        "gb" => 4,
        "gbc" => 41,
        "pce" => 34,
        "n64" => 3,
        "ds" => 8,
        "ps1" => 10,
        "ps2" => 11,
        "ps3" => 12,
        "dreamcast" => 16,
        "genesis" => 18,
        "neogeo" => 24,
        _ => return None,
    })
}

fn tidy_description(raw: &str) -> Option<String> {
    let text = raw.replace("\r\n", "\n").replace('\r', "\n");
    let text = text.trim();
    if text.chars().count() < 20 {
        return None;
    }
    Some(text.chars().take(DESCRIPTION_MAX).collect())
}

/// Use the first search result that is the same game.
fn pick_game(json: &Value, wanted: &str, genres: &BTreeMap<u64, String>) -> Option<GameInfo> {
    let game = json["data"]["games"].as_array()?.iter().find(|g| {
        g["game_title"]
            .as_str()
            .is_some_and(|t| romart::names_agree(wanted, t))
    })?;
    let genre = game["genres"].as_array().and_then(|ids| {
        let names: Vec<&str> = ids
            .iter()
            .filter_map(|i| i.as_u64())
            .filter_map(|i| genres.get(&i).map(String::as_str))
            .take(3)
            .collect();
        (!names.is_empty()).then(|| names.join(", "))
    });
    let info = GameInfo {
        genre,
        year: game["release_date"].as_str().and_then(year_of),
        developer: None,
        publisher: None,
        players: game["players"].as_i64().filter(|p| (1..=16).contains(p)),
        description: game["overview"].as_str().and_then(tidy_description),
        source: String::new(),
    };
    (info.description.is_some() || info.genre.is_some() || info.year.is_some()).then_some(info)
}

pub(crate) fn tgdb_genres(cache: &Path, key: &str) -> BTreeMap<u64, String> {
    let file = cache.join("tgdb-genres.json");
    let known: BTreeMap<u64, String> = romart::load(&file);
    if !known.is_empty() {
        return known;
    }
    let Ok(json) = http::get_json(&format!("{TGDB}/Genres?apikey={key}")) else {
        return known;
    };
    let mut out = BTreeMap::new();
    if let Some(map) = json["data"]["genres"].as_object() {
        for (id, g) in map {
            if let (Ok(id), Some(name)) = (id.parse::<u64>(), g["name"].as_str()) {
                out.insert(id, name.to_string());
            }
        }
    }
    romart::save(&file, &out);
    out
}

/// Ask TheGamesDB. The error text of a failed request contains the key in its
/// URL, so errors are dropped here and never shown or logged.
fn tgdb_lookup(
    key: &str,
    platform: u32,
    name: &str,
    genres: &BTreeMap<u64, String>,
    left: &AtomicI64,
) -> Option<GameInfo> {
    let term = romart::plain_title(name);
    let url = format!(
        "{TGDB}/Games/ByGameName?apikey={key}&name={}&fields=overview,players,genres&filter%5Bplatform%5D={platform}",
        romart::encode(&term)
    );
    let json = http::get_json(&url).ok()?;
    if let Some(n) = json["remaining_monthly_allowance"].as_i64() {
        left.store(n, Ordering::SeqCst);
    }
    pick_game(&json, &term, genres)
}

pub(crate) fn fill(info: &mut GameInfo, from: &GameInfo) {
    macro_rules! gap {
        ($($f:ident),*) => {$( if info.$f.is_none() { info.$f = from.$f.clone(); } )*};
    }
    gap!(genre, year, developer, publisher, players, description);
}

pub(crate) fn add_source(info: &mut GameInfo, label: &str) {
    let mut parts: Vec<&str> = info.source.split('+').filter(|s| !s.is_empty()).collect();
    if !parts.contains(&label) {
        parts.push(label);
    }
    info.source = parts.join("+");
}

struct Pending<'a> {
    edition: &'a Edition,
    info: GameInfo,
    platform: u32,
    name: String,
    ask: bool,
}

/// Fill in missing facts. Safe to call often: finished games are skipped, lists
/// and hashes are cached, and games that yielded nothing wait 30 days.
/// Returns how many games gained information.
pub fn run(core: &Core, edition_id: Option<&str>) -> usize {
    let Ok(snap) = core.snapshot() else { return 0 };
    let cache = core.paths.cache_dir.clone();
    let dat_dir = cache.join("libretro");
    let (crc_file, tried_file) = (cache.join("romcrc.json"), cache.join("romfacts-tried.json"));
    let key = core.key("thegamesdb").ok().flatten();
    let mut crc_cache: CrcCache = romart::load(&crc_file);
    // edition -> (when, whether TheGamesDB was asked)
    let mut tried: BTreeMap<String, (u64, bool)> = romart::load(&tried_file);
    let now = romart::now_secs();
    let mut systems: HashMap<&str, Option<SystemFacts>> = HashMap::new();
    let mut pending: Vec<Pending> = Vec::new();
    let mut asked: std::collections::HashSet<String> = Default::default();

    for e in snap
        .editions
        .iter()
        .filter(|e| (!e.missing || e.wanted.is_some()) && edition_id.is_none_or(|id| id == e.id))
    {
        let have = e.info.clone().unwrap_or_default();
        if edition_id.is_none() {
            if have.description.is_some() {
                continue;
            }
            if let Some((when, asked)) = tried.get(&e.id) {
                if now.saturating_sub(*when) < RETRY_SECS && (*asked || key.is_none()) {
                    continue;
                }
            }
        }
        let Some(src): Option<&Source> = romart::source_for(&e.system) else {
            continue;
        };
        let mut info = have.clone();
        let mut official: Option<String> = None;
        if let Some(list) = src.dat {
            let facts = systems
                .entry(list)
                .or_insert_with(|| load_system(&dat_dir, list));
            let media = snap
                .media
                .iter()
                .find(|m| m.edition_id == e.id && m.available);
            if let Some(facts) = facts.as_ref() {
                let crcs: Vec<u32> = match media {
                    Some(m) => {
                        let path = m.path.to_string_lossy().into_owned();
                        let stamp = (m.size_bytes, m.mtime);
                        match crc_cache.0.get(&path) {
                            Some((s, t, c)) if (*s, *t) == stamp => c.clone(),
                            _ => {
                                let c = romart::crcs_of(&m.path).unwrap_or_default();
                                crc_cache.0.insert(path, (stamp.0, stamp.1, c.clone()));
                                c
                            }
                        }
                    }
                    // A game without files: the typed title picks the list entry.
                    None if e.wanted.is_some() => {
                        let title = e.display_title.as_deref().unwrap_or(&e.scan_title);
                        crate::wanted::best_match(&facts.names, title)
                            .map(|(crc, _)| vec![crc])
                            .unwrap_or_default()
                    }
                    None => vec![],
                };
                let find =
                    |map: &HashMap<u32, String>| crcs.iter().find_map(|c| map.get(c).cloned());
                official = find(&facts.names);
                let found = GameInfo {
                    genre: find(&facts.genre),
                    year: find(&facts.year).as_deref().and_then(year_of),
                    developer: find(&facts.developer),
                    publisher: find(&facts.publisher),
                    ..GameInfo::default()
                };
                if found.genre.is_some()
                    || found.year.is_some()
                    || found.developer.is_some()
                    || found.publisher.is_some()
                {
                    fill(&mut info, &found);
                    add_source(&mut info, "libretro");
                }
            }
        }
        let disc = src.dat.is_none();
        let name = match (&official, disc) {
            (Some(n), _) => n.clone(),
            (None, true) => e
                .display_title
                .clone()
                .unwrap_or_else(|| e.scan_title.clone()),
            (None, false) => String::new(),
        };
        // Same gate as covers: homebrew and public-domain ROMs share names with
        // famous games, and a wrong description is worse than none.
        let ask = key.is_some()
            && !name.is_empty()
            && (official.as_deref().is_some_and(romart::looks_retail) || disc);
        pending.push(Pending {
            edition: e,
            info,
            platform: tgdb_platform(&e.system).unwrap_or(0),
            name,
            ask: ask && tgdb_platform(&e.system).is_some() && info_wants_tgdb(&e.info),
        });
    }

    if let Some(key) = key.as_deref() {
        let todo: Vec<usize> = pending
            .iter()
            .enumerate()
            .filter(|(_, p)| p.ask)
            .map(|(i, _)| i)
            .take(TGDB_BUDGET)
            .collect();
        if !todo.is_empty() {
            let genres = tgdb_genres(&cache, key);
            let left = AtomicI64::new(i64::MAX);
            let stop = AtomicBool::new(false);
            let next = AtomicUsize::new(0);
            let out: Mutex<Vec<Option<Option<GameInfo>>>> = Mutex::new(vec![None; todo.len()]);
            std::thread::scope(|scope| {
                for _ in 0..3 {
                    scope.spawn(|| loop {
                        let i = next.fetch_add(1, Ordering::SeqCst);
                        let Some(&at) = todo.get(i) else { break };
                        if stop.load(Ordering::SeqCst) {
                            break;
                        }
                        let p = &pending[at];
                        let found = tgdb_lookup(key, p.platform, &p.name, &genres, &left);
                        if left.load(Ordering::SeqCst) < TGDB_RESERVE {
                            stop.store(true, Ordering::SeqCst);
                        }
                        out.lock().unwrap_or_else(|e| e.into_inner())[i] = Some(found);
                    });
                }
            });
            let out = out.into_inner().unwrap_or_else(|e| e.into_inner());
            for (slot, &at) in todo.iter().enumerate() {
                // A lookup that never ran (quota reserve) is not marked as tried.
                let Some(result) = &out[slot] else { continue };
                let p = &mut pending[at];
                if let Some(found) = result {
                    fill(&mut p.info, found);
                    add_source(&mut p.info, "thegamesdb");
                }
                asked.insert(p.edition.id.clone());
                tried.insert(p.edition.id.clone(), (now, true));
            }
        }
    }

    let mut changed = 0;
    for p in &pending {
        if !asked.contains(&p.edition.id) {
            tried.insert(p.edition.id.clone(), (now, false));
        }
        if p.info != p.edition.info.clone().unwrap_or_default()
            && core
                .with_db(|db| upsert(db, &p.edition.id, &p.info))
                .is_ok()
        {
            changed += 1;
        }
    }
    romart::save(&crc_file, &crc_cache);
    romart::save(&tried_file, &tried);
    changed
}

fn info_wants_tgdb(have: &Option<GameInfo>) -> bool {
    have.as_ref().is_none_or(|i| i.description.is_none())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_lists_map_checksums_to_values() {
        let text = "clrmamepro (\n\tname \"X\"\n)\n\ngame (\n\tcomment \"A (Japan)\"\n\tgenre \"Simulation\"\n\trom ( crc 3577AB04 )\n)\n\ngame (\n\tcomment \"B (USA)\"\n\trom ( crc 00000001 )\n)\n\ngame (\n\tcomment \"C\"\n\tgenre \"Sports\"\n\trom ( crc 0000000A )\n)\n";
        let m = parse_meta(text, "genre");
        assert_eq!(m.get(&0x3577AB04).map(String::as_str), Some("Simulation"));
        // A game without the field must not inherit the previous game's value.
        assert!(!m.contains_key(&1));
        assert_eq!(m.get(&0xA).map(String::as_str), Some("Sports"));
    }

    #[test]
    fn years_are_sane_or_absent() {
        assert_eq!(year_of("1988"), Some(1988));
        assert_eq!(year_of("2000-12-14"), Some(2000));
        assert_eq!(year_of("0000-00-00"), None);
        assert_eq!(year_of("abc"), None);
    }

    #[test]
    fn only_the_same_game_gives_a_description() {
        let genres = BTreeMap::from([(2u64, "Adventure".to_string()), (4, "RPG".to_string())]);
        let json = serde_json::json!({"data": {"games": [
            {"game_title": "Pokemon Crystal 251", "overview": "A fan-made hack of the original game with many changes.", "genres": [4]},
            {"game_title": "Pokemon Crystal Version", "release_date": "2000-12-14", "players": 2,
             "overview": "Travel back to Johto.\r\nCatch them all, again and again.", "genres": [2, 4]}
        ]}});
        let got = pick_game(&json, "Pokemon Crystal Version (USA, Europe)", &genres).unwrap();
        assert_eq!(got.year, Some(2000));
        assert_eq!(got.players, Some(2));
        assert_eq!(got.genre.as_deref(), Some("Adventure, RPG"));
        assert!(got.description.unwrap().contains("Johto.\nCatch"));
        assert!(pick_game(&json, "Pokemon Gold", &genres).is_none());
        assert!(pick_game(
            &serde_json::json!({"data": {"count": 0}}),
            "Anything Goes",
            &genres
        )
        .is_none());
    }

    #[test]
    fn merging_fills_gaps_and_never_overwrites() {
        let mut a = GameInfo {
            genre: Some("Action".into()),
            ..GameInfo::default()
        };
        let b = GameInfo {
            genre: Some("RPG".into()),
            year: Some(1999),
            ..GameInfo::default()
        };
        fill(&mut a, &b);
        assert_eq!((a.genre.as_deref(), a.year), (Some("Action"), Some(1999)));
        add_source(&mut a, "libretro");
        add_source(&mut a, "thegamesdb");
        add_source(&mut a, "libretro");
        assert_eq!(a.source, "libretro+thegamesdb");
    }

    #[test]
    fn short_or_empty_descriptions_are_dropped() {
        assert_eq!(tidy_description("  tiny "), None);
        assert!(tidy_description(&"x".repeat(9000)).unwrap().chars().count() == DESCRIPTION_MAX);
    }
}
