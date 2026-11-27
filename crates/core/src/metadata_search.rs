//! Reviewed provider identities. Search is separate from applying an artwork
//! match; provider IDs are resolved from a short-lived server-side review.
use crate::sessions::EventSink;
use crate::{
    error::{AppError, Result},
    Core,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
fn err(message: &str) -> AppError {
    AppError::new("METADATA_SEARCH", message)
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Candidate {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub facts: Option<crate::model::GameInfo>,
    #[serde(default)]
    pub system: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub country_id: Option<u64>,
    #[serde(default)]
    pub region_id: Option<u64>,
}
#[derive(Serialize)]
pub struct Review {
    pub token: String,
    pub provider: String,
    pub candidates: Vec<Candidate>,
    pub current: Option<crate::model::GameInfo>,
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactField {
    Genre,
    Year,
    Developer,
    Publisher,
    Players,
    Description,
}

#[derive(Serialize)]
pub struct FactChange {
    pub id: String,
}
pub fn ensure_tables(db: &rusqlite::Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS metadata_search_reviews(token TEXT PRIMARY KEY,edition_id TEXT NOT NULL,payload TEXT NOT NULL,created INTEGER NOT NULL);
    CREATE TABLE IF NOT EXISTS metadata_provider_matches(edition_id TEXT PRIMARY KEY,provider TEXT NOT NULL,provider_id TEXT NOT NULL,title TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS metadata_fact_review_state(token TEXT PRIMARY KEY REFERENCES metadata_search_reviews(token) ON DELETE CASCADE,before_info TEXT NOT NULL,identity TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS metadata_fact_changes(id TEXT PRIMARY KEY,edition_id TEXT NOT NULL REFERENCES editions(id) ON DELETE CASCADE,before_info TEXT NOT NULL,after_info TEXT NOT NULL,identity TEXT NOT NULL);")?;
    Ok(())
}
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
fn candidates(json: &Value) -> Vec<Candidate> {
    json["data"]
        .as_array()
        .into_iter()
        .flatten()
        .take(20)
        .filter_map(|v| {
            let id = v["id"].as_u64().filter(|n| *n > 0)?;
            let title = v["name"].as_str()?.trim();
            if title.is_empty() || title.len() > 300 || title.chars().any(char::is_control) {
                return None;
            }
            Some(Candidate {
                id,
                title: title.into(),
                facts: None,
                system: None,
                ..Default::default()
            })
        })
        .collect()
}
impl Core {
    pub fn search_game_metadata(&self, edition_id: &str, query: &str) -> Result<Review> {
        let query = query.trim();
        if query.len() < 2 || query.len() > 200 || query.chars().any(char::is_control) {
            return Err(err("Enter a game title between 2 and 200 characters"));
        }
        self.with_db(|db| {
            if db.edition_by_id(edition_id)?.is_none() {
                return Err(err("Game is not in the library"));
            }
            Ok(())
        })?;
        let key = self.key("steamgriddb")?.ok_or_else(|| {
            err("Add a SteamGridDB API key in Settings to search artwork identities")
        })?;
        let json = crate::http::get_json_bearer(
            &format!(
                "https://www.steamgriddb.com/api/v2/search/autocomplete/{}",
                crate::romart::encode(query)
            ),
            &key,
        )?;
        let items = candidates(&json);
        let token = Uuid::new_v4().to_string();
        let payload = serde_json::to_string(&items)?;
        self.with_db(|db| {
            db.conn.execute(
                "DELETE FROM metadata_search_reviews WHERE edition_id=?1 OR created<?2",
                params![edition_id, now() - 1800],
            )?;
            db.conn.execute(
                "INSERT INTO metadata_search_reviews VALUES(?1,?2,?3,?4)",
                params![token, edition_id, payload, now()],
            )?;
            Ok(())
        })?;
        Ok(Review {
            token,
            provider: "SteamGridDB · artwork identity (platform not verified)".into(),
            candidates: items,
            current: None,
        })
    }
    pub fn search_game_facts(&self, edition_id: &str, query: &str) -> Result<Review> {
        let query = query.trim();
        if query.len() < 2 || query.len() > 200 || query.chars().any(char::is_control) {
            return Err(err("Enter a game title between 2 and 200 characters"));
        }
        let system = self.with_db(|db| {
            db.edition_by_id(edition_id)?
                .map(|e| e.system)
                .ok_or_else(|| err("Game is not in the library"))
        })?;
        let platform = crate::romfacts::tgdb_platform(&system).ok_or_else(|| {
            err("This platform is not supported by the game-details provider yet")
        })?;
        let key = self
            .key("thegamesdb")?
            .ok_or_else(|| err("Add a TheGamesDB API key in Settings to search game details"))?;
        // Query-string credentials must never be included in surfaced HTTP errors.
        let json = crate::http::get_json(&format!(
            "https://api.thegamesdb.net/v1/Games/ByGameName?apikey={}&name={}&fields=overview,players,genres,platform,publishers&filter%5Bplatform%5D={platform}",
            crate::romart::encode(&key), crate::romart::encode(query)
        )).map_err(|_| err("TheGamesDB search failed. Check your API key, allowance and connection."))?;
        let names = FactNames {
            genres: crate::romfacts::tgdb_genres(&self.paths.cache_dir, &key),
            developers: crate::romfacts::provider_names(&self.paths.cache_dir, &key, "developers"),
            publishers: crate::romfacts::provider_names(&self.paths.cache_dir, &key, "publishers"),
            countries: crate::romfacts::provider_names(&self.paths.cache_dir, &key, "countries"),
            regions: crate::romfacts::provider_names(&self.paths.cache_dir, &key, "regions"),
        };
        self.save_fact_review(
            edition_id,
            &system,
            fact_candidates(&json, &system, platform, &names),
        )
    }

    fn save_fact_review(
        &self,
        edition_id: &str,
        system: &str,
        items: Vec<Candidate>,
    ) -> Result<Review> {
        let token = Uuid::new_v4().to_string();
        let current = self.with_db(|db| {
            let transaction = db.conn.unchecked_transaction()?;
            let edition = db
                .edition_by_id(edition_id)?
                .ok_or_else(|| err("Game was removed"))?;
            if edition.system != system {
                return Err(err("Game platform changed; search again"));
            }
            let current = crate::romfacts::list(db)?.remove(edition_id);
            db.conn.execute(
                "DELETE FROM metadata_search_reviews WHERE edition_id=?1 OR created<?2",
                params![edition_id, now() - 1800],
            )?;
            db.conn.execute(
                "INSERT INTO metadata_search_reviews VALUES(?1,?2,?3,?4)",
                params![token, edition_id, serde_json::to_string(&items)?, now()],
            )?;
            db.conn.execute(
                "INSERT INTO metadata_fact_review_state VALUES(?1,?2,?3)",
                params![
                    token,
                    serde_json::to_string(&current)?,
                    fact_identity(db, edition_id)?
                ],
            )?;
            transaction.commit()?;
            Ok(current)
        })?;
        Ok(Review {
            token,
            provider: format!("TheGamesDB · {system} game details · review the release"),
            candidates: items,
            current,
        })
    }

    /// Compatibility entry point: existing callers only fill missing fields.
    pub fn apply_game_facts(&self, edition_id: &str, token: &str, id: u64) -> Result<()> {
        self.apply_fact_change(edition_id, token, id, None)
            .map(|_| ())
    }

    pub fn apply_game_fact_fields(
        &self,
        edition_id: &str,
        token: &str,
        id: u64,
        fields: Vec<FactField>,
    ) -> Result<FactChange> {
        let unique: std::collections::BTreeSet<_> = fields.iter().collect();
        if fields.len() > 6 || unique.len() != fields.len() {
            return Err(err("Choose each detail at most once"));
        }
        self.apply_fact_change(edition_id, token, id, Some(&fields))
    }

    fn apply_fact_change(
        &self,
        edition_id: &str,
        token: &str,
        id: u64,
        fields: Option<&[FactField]>,
    ) -> Result<FactChange> {
        let change = self.with_db(|db| {
            let transaction = db.conn.unchecked_transaction()?;
            let candidate = reviewed(&db.conn, edition_id, token, id)?;
            let facts = candidate.facts.ok_or_else(|| err("This is an artwork result, not a game-details match"))?;
            let edition = db.edition_by_id(edition_id)?.ok_or_else(|| err("Game was removed"))?;
            if candidate.system.as_deref() != Some(edition.system.as_str()) { return Err(err("Game platform changed; search again")); }
            let before = crate::romfacts::list(db)?.remove(edition_id);
            let identity = fact_identity(db, edition_id)?;
            let expected: Option<(String,String)> = db.conn.query_row("SELECT before_info,identity FROM metadata_fact_review_state WHERE token=?1", [token], |r| Ok((r.get(0)?,r.get(1)?))).optional()?;
            if let Some((expected, expected_identity)) = expected {
                let expected: Option<crate::model::GameInfo> = serde_json::from_str(&expected)?;
                if expected != before || expected_identity != identity { return Err(err("Game details changed since this review. Search again before saving.")); }
            } else if fields.is_some() { return Err(err("This review cannot edit individual fields. Search again.")); }
            let mut info = before.clone().unwrap_or_default();
            if let Some(fields) = fields { copy_fact_fields(&mut info, &facts, fields)?; }
            else { crate::romfacts::fill(&mut info, &facts); }
            crate::romfacts::select_tgdb_id(&mut info, id);
            let change = FactChange { id: Uuid::new_v4().to_string() };
            db.conn.execute("INSERT INTO metadata_fact_changes VALUES(?1,?2,?3,?4,?5)", params![change.id, edition_id, serde_json::to_string(&before)?, serde_json::to_string(&info)?, identity])?;
            crate::romfacts::upsert(db, edition_id, &info)?;
            db.conn.execute("DELETE FROM metadata_search_reviews WHERE token=?1", [token])?;
            db.conn.execute("DELETE FROM metadata_fact_changes WHERE edition_id=?1 AND id NOT IN (SELECT id FROM metadata_fact_changes WHERE edition_id=?1 ORDER BY rowid DESC LIMIT 10)", [edition_id])?;
            transaction.commit()?;
            Ok(change)
        })?;
        self.events.emit(crate::CoreEvent::LibraryChanged);
        Ok(change)
    }

    pub fn game_fact_change(&self, edition_id: &str) -> Result<Option<FactChange>> {
        self.with_db(|db| {
            let change: Option<(String,String,String)> = db.conn.query_row("SELECT id,after_info,identity FROM metadata_fact_changes WHERE edition_id=?1 ORDER BY rowid DESC LIMIT 1", [edition_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
            let Some((id, after, identity)) = change else { return Ok(None); };
            let after: crate::model::GameInfo = serde_json::from_str(&after)?;
            let current = crate::romfacts::list(db)?.remove(edition_id);
            Ok((current.as_ref()==Some(&after) && fact_identity(db, edition_id)?==identity).then_some(FactChange{id}))
        })
    }

    pub fn undo_game_fact_change(&self, edition_id: &str, change_id: &str) -> Result<()> {
        self.with_db(|db| {
            let transaction = db.conn.unchecked_transaction()?;
            let (before, after, identity): (String,String,String) = db.conn.query_row("SELECT before_info,after_info,identity FROM metadata_fact_changes WHERE id=?1 AND edition_id=?2", params![change_id, edition_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|err("This details change is no longer available to undo"))?;
            let before: Option<crate::model::GameInfo> = serde_json::from_str(&before)?;
            let after: crate::model::GameInfo = serde_json::from_str(&after)?;
            let current = crate::romfacts::list(db)?.remove(edition_id);
            if current.as_ref()!=Some(&after) || fact_identity(db, edition_id)?!=identity { return Err(err("Game details changed after this edit. Undo would replace newer information.")); }
            if let Some(before) = before { crate::romfacts::upsert(db, edition_id, &before)?; }
            else { db.conn.execute("DELETE FROM edition_info WHERE edition_id=?1", [edition_id])?; }
            db.conn.execute("DELETE FROM metadata_fact_changes WHERE id=?1", [change_id])?;
            db.conn.execute("DELETE FROM metadata_search_reviews WHERE edition_id=?1", [edition_id])?;
            transaction.commit()?;
            Ok(())
        })?;
        self.events.emit(crate::CoreEvent::LibraryChanged);
        Ok(())
    }

    pub fn apply_metadata_match(&self, edition_id: &str, token: &str, id: u64) -> Result<()> {
        let candidate = self.with_db(|db| reviewed(&db.conn, edition_id, token, id))?;
        if candidate.facts.is_some() {
            return Err(err("This is a game-details result, not an artwork match"));
        }
        let key = self
            .key("steamgriddb")?
            .ok_or_else(|| err("SteamGridDB API key is not configured"))?;
        let (bytes, source) = crate::romart::steamgriddb_id(&key, id)
            .ok_or_else(|| err("No compatible cover is available for this candidate"))?;
        let edition = self.with_db(|db| {
            crate::library::snapshot_editions(db)?
                .into_iter()
                .find(|e| e.id == edition_id)
                .ok_or_else(|| err("Game was removed"))
        })?;
        let art = self.paths.cache_dir.join("art");
        std::fs::create_dir_all(&art)?;
        crate::romart::store(self, &art, &edition, &bytes, source)?;
        self.with_db(|db|{db.conn.execute("INSERT INTO metadata_provider_matches VALUES(?1,'steamgriddb',?2,?3) ON CONFLICT(edition_id) DO UPDATE SET provider=excluded.provider,provider_id=excluded.provider_id,title=excluded.title",params![edition_id,id.to_string(),candidate.title])?;db.conn.execute("DELETE FROM metadata_search_reviews WHERE token=?1",[token])?;Ok(())})?;
        self.events.emit(crate::CoreEvent::LibraryChanged);
        Ok(())
    }
}
fn fact_identity(db: &crate::db::Db, edition_id: &str) -> Result<String> {
    let edition = db
        .edition_by_id(edition_id)?
        .ok_or_else(|| err("Game was removed"))?;
    Ok(serde_json::to_string(&(
        edition.system,
        edition.scan_title,
        edition.media_id,
    ))?)
}

fn copy_fact_fields(
    to: &mut crate::model::GameInfo,
    from: &crate::model::GameInfo,
    fields: &[FactField],
) -> Result<()> {
    macro_rules! copy {
        ($field:ident) => {{
            if from.$field.is_none() {
                return Err(err("The provider did not supply a selected detail"));
            }
            to.$field = from.$field.as_ref().cloned();
        }};
    }
    for field in fields {
        match field {
            FactField::Genre => copy!(genre),
            FactField::Year => copy!(year),
            FactField::Developer => copy!(developer),
            FactField::Publisher => copy!(publisher),
            FactField::Players => copy!(players),
            FactField::Description => copy!(description),
        }
    }
    Ok(())
}

#[derive(Default)]
struct FactNames {
    genres: std::collections::BTreeMap<u64, String>,
    developers: std::collections::BTreeMap<u64, String>,
    publishers: std::collections::BTreeMap<u64, String>,
    countries: std::collections::BTreeMap<u64, String>,
    regions: std::collections::BTreeMap<u64, String>,
}

fn valid_release_date(date: &str) -> bool {
    let parts: Vec<_> = date.split('-').collect();
    if parts.len() != 3
        || parts[0].len() != 4
        || parts[1].len() != 2
        || parts[2].len() != 2
        || !parts
            .iter()
            .all(|part| part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return false;
    }
    let (Ok(year), Ok(month), Ok(day)) = (
        parts[0].parse::<u32>(),
        parts[1].parse::<u32>(),
        parts[2].parse::<u32>(),
    ) else {
        return false;
    };
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 0,
    };
    (1950..=2100).contains(&year) && (1..=days).contains(&day)
}

fn fact_candidates(json: &Value, system: &str, platform: u32, names: &FactNames) -> Vec<Candidate> {
    json["data"]["games"]
        .as_array()
        .into_iter()
        .flatten()
        .take(100)
        .filter_map(|game| {
            if game["platform"].as_u64()? != u64::from(platform) {
                return None;
            }
            let id = game["id"].as_u64().filter(|id| *id > 0)?;
            let title = game["game_title"].as_str()?.trim();
            if title.is_empty() || title.len() > 300 || title.chars().any(char::is_control) {
                return None;
            }
            let genre: Vec<_> = game["genres"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|id| names.genres.get(&id.as_u64()?))
                .take(3)
                .cloned()
                .collect();
            let year = game["release_date"]
                .as_str()
                .and_then(|s| s.get(..4))
                .and_then(|s| s.parse::<i64>().ok())
                .filter(|y| (1950..=2100).contains(y));
            let description = game["overview"]
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.chars().take(4000).collect());
            let players = game["players"]
                .as_i64()
                .or_else(|| game["players"].as_str()?.parse().ok())
                .filter(|n| (1..=16).contains(n));
            Some(Candidate {
                id,
                title: title.into(),
                system: Some(system.into()),
                release_date: game["release_date"]
                    .as_str()
                    .filter(|date| valid_release_date(date))
                    .map(str::to_owned),
                country_id: game["country_id"].as_u64().filter(|id| *id > 0),
                region_id: game["region_id"].as_u64().filter(|id| *id > 0),
                country: game["country_id"]
                    .as_u64()
                    .and_then(|id| names.countries.get(&id))
                    .cloned(),
                region: game["region_id"]
                    .as_u64()
                    .and_then(|id| names.regions.get(&id))
                    .cloned(),
                facts: Some(crate::model::GameInfo {
                    genre: (!genre.is_empty()).then(|| genre.join(", ")),
                    year,
                    players,
                    description,
                    developer: crate::romfacts::company_names(
                        &game["developers"],
                        &names.developers,
                    ),
                    publisher: crate::romfacts::company_names(
                        &game["publishers"],
                        &names.publishers,
                    ),
                    source: format!("thegamesdb:{id}"),
                }),
            })
        })
        .take(20)
        .collect()
}

fn reviewed(db: &rusqlite::Connection, edition: &str, token: &str, id: u64) -> Result<Candidate> {
    let (payload, created): (String, i64) = db
        .query_row(
            "SELECT payload,created FROM metadata_search_reviews WHERE token=?1 AND edition_id=?2",
            params![token, edition],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| err("Search review changed; search again"))?;
    if created < now() - 1800 {
        return Err(err("Search review expired; search again"));
    }
    let items: Vec<Candidate> = serde_json::from_str(&payload)?;
    items
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| err("Candidate was not in this search review"))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn setup_facts(core: &Core) {
        core.with_db(|db| {
            db.conn.execute("INSERT INTO editions(id,system,scan_title,imported_at,favorite) VALUES('game','ps1','My title','2026-01-01',1),('other','ps1','Other title','2026-01-01',0)", [])?;
            Ok(())
        }).unwrap();
    }
    fn fact_candidate() -> Candidate {
        Candidate {
            id: 8,
            title: "Provider title".into(),
            system: Some("ps1".into()),
            facts: Some(crate::model::GameInfo {
                year: Some(2000),
                description: Some("Provider description".into()),
                developer: Some("Provider developer".into()),
                publisher: Some("Provider publisher".into()),
                source: "thegamesdb:8".into(),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn regional_candidates_keep_dates_country_region_and_company_names() {
        let names = FactNames {
            countries: [(50, "United States".into()), (18, "Japan".into())].into(),
            regions: [(1, "North America".into()), (3, "Japan".into())].into(),
            developers: [(7, "Game Freak".into())].into(),
            publishers: [(8, "Nintendo".into())].into(),
            ..Default::default()
        };
        let json = serde_json::json!({"data":{"games":[
            {"id":1,"game_title":"Pokémon Moon","platform":4912,"country_id":50,"region_id":1,"release_date":"2016-11-18","developers":[7],"publishers":[8]},
            {"id":2,"game_title":"Pokémon Moon","platform":4912,"country_id":18,"region_id":3,"release_date":"2016-11-23"},
            {"id":3,"game_title":"Invalid date","platform":4912,"release_date":"2023-02-29"}
        ]}});
        let found = fact_candidates(&json, "3ds", 4912, &names);
        assert_eq!(found[0].release_date.as_deref(), Some("2016-11-18"));
        assert_eq!(found[0].country.as_deref(), Some("United States"));
        assert_eq!(found[1].region.as_deref(), Some("Japan"));
        assert_eq!(
            found[0].facts.as_ref().unwrap().developer.as_deref(),
            Some("Game Freak")
        );
        assert_eq!(
            found[0].facts.as_ref().unwrap().publisher.as_deref(),
            Some("Nintendo")
        );
        assert!(found[2].release_date.is_none());
        assert!(valid_release_date("2024-02-29"));
        assert!(!valid_release_date("2024-13-01"));
    }

    #[test]
    fn selected_corrections_preserve_unchecked_fields_and_undo_after_reopen() {
        let temp = tempfile::tempdir().unwrap();
        let data = temp.path().join("library");
        let core = Core::open_isolated(Some(data.clone()), false).unwrap();
        setup_facts(&core);
        let before = crate::model::GameInfo {
            year: Some(1999),
            description: Some("My description".into()),
            developer: Some("My developer".into()),
            source: "manual+thegamesdb:7".into(),
            ..Default::default()
        };
        core.with_db(|db| crate::romfacts::upsert(db, "game", &before))
            .unwrap();
        let review = core
            .save_fact_review("game", "ps1", vec![fact_candidate()])
            .unwrap();
        assert_eq!(review.current, Some(before.clone()));
        let change = core
            .apply_game_fact_fields(
                "game",
                &review.token,
                8,
                vec![FactField::Year, FactField::Publisher],
            )
            .unwrap();
        let info = core
            .with_db(|db| Ok(crate::romfacts::list(db)?.remove("game").unwrap()))
            .unwrap();
        assert_eq!(info.year, Some(2000));
        assert_eq!(info.publisher.as_deref(), Some("Provider publisher"));
        assert_eq!(info.description, before.description);
        assert_eq!(info.developer, before.developer);
        assert_eq!(crate::romfacts::tgdb_id(&info), Some(8));
        assert!(core
            .apply_game_fact_fields("game", &review.token, 8, vec![])
            .is_err());
        drop(core);
        let core = Core::open_isolated(Some(data), false).unwrap();
        assert_eq!(
            core.game_fact_change("game").unwrap().unwrap().id,
            change.id
        );
        assert!(core.undo_game_fact_change("other", &change.id).is_err());
        core.undo_game_fact_change("game", &change.id).unwrap();
        assert_eq!(
            core.with_db(|db| Ok(crate::romfacts::list(db)?.remove("game")))
                .unwrap(),
            Some(before)
        );
        let edition = core
            .snapshot()
            .unwrap()
            .editions
            .into_iter()
            .find(|e| e.id == "game")
            .unwrap();
        assert_eq!(edition.title, "My title");
        assert!(edition.favorite);
        assert!(core.game_fact_change("game").unwrap().is_none());
        assert!(core.undo_game_fact_change("game", &change.id).is_err());
    }

    #[test]
    fn stale_reviews_and_undo_never_replace_later_details() {
        let temp = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(temp.path().join("library")), false).unwrap();
        setup_facts(&core);
        let review = core
            .save_fact_review("game", "ps1", vec![fact_candidate()])
            .unwrap();
        let later = crate::model::GameInfo {
            year: Some(1998),
            source: "manual".into(),
            ..Default::default()
        };
        core.with_db(|db| crate::romfacts::upsert(db, "game", &later))
            .unwrap();
        assert!(core
            .apply_game_fact_fields("game", &review.token, 8, vec![FactField::Year])
            .is_err());
        let review = core
            .save_fact_review("game", "ps1", vec![fact_candidate()])
            .unwrap();
        assert!(core
            .apply_game_fact_fields("game", &review.token, 8, vec![FactField::Players])
            .is_err());
        assert!(core
            .apply_game_fact_fields(
                "game",
                &review.token,
                8,
                vec![FactField::Year, FactField::Year]
            )
            .is_err());
        assert!(core
            .apply_game_fact_fields("other", &review.token, 8, vec![FactField::Year])
            .is_err());
        let change = core
            .apply_game_fact_fields("game", &review.token, 8, vec![FactField::Year])
            .unwrap();
        core.with_db(|db| crate::romfacts::upsert(db, "game", &later))
            .unwrap();
        assert!(core.game_fact_change("game").unwrap().is_none());
        assert!(core.undo_game_fact_change("game", &change.id).is_err());
        assert_eq!(
            core.with_db(|db| Ok(crate::romfacts::list(db)?.remove("game")))
                .unwrap(),
            Some(later)
        );
        let review = core
            .save_fact_review("game", "ps1", vec![fact_candidate()])
            .unwrap();
        core.with_db(|db| {
            db.conn.execute(
                "UPDATE editions SET scan_title='Changed game' WHERE id='game'",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        assert!(core
            .apply_game_fact_fields("game", &review.token, 8, vec![FactField::Year])
            .is_err());
        assert!(serde_json::from_str::<Vec<FactField>>(r#"["source"]"#).is_err());
    }

    #[test]
    fn reference_only_selection_and_undo_restore_a_missing_info_row() {
        let temp = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(temp.path().join("library")), false).unwrap();
        setup_facts(&core);
        let review = core
            .save_fact_review("game", "ps1", vec![fact_candidate()])
            .unwrap();
        let change = core
            .apply_game_fact_fields("game", &review.token, 8, vec![])
            .unwrap();
        let info = core
            .with_db(|db| Ok(crate::romfacts::list(db)?.remove("game").unwrap()))
            .unwrap();
        assert!(info.year.is_none() && info.description.is_none());
        assert_eq!(crate::romfacts::tgdb_id(&info), Some(8));
        core.undo_game_fact_change("game", &change.id).unwrap();
        assert!(core
            .with_db(|db| Ok(crate::romfacts::list(db)?.remove("game")))
            .unwrap()
            .is_none());
    }

    #[test]
    fn xbox360_fact_search_excludes_same_title_on_other_platforms() {
        let platform = crate::romfacts::tgdb_platform("xbox360").unwrap();
        let json = serde_json::json!({"data":{"games":[
            {"id":1,"game_title":"Vanquish","platform":15,"release_date":"2010-10-19","players":1},
            {"id":2,"game_title":"Vanquish","platform":12,"players":1}
        ]}});
        let candidates = fact_candidates(&json, "xbox360", platform, &Default::default());
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, 1);
        assert_eq!(candidates[0].system.as_deref(), Some("xbox360"));
    }

    #[test]
    fn facts_are_platform_scoped_and_only_fill_missing_fields() {
        let json = serde_json::json!({"data":{"games":[
            {"id":843,"game_title":"God of War III","platform":12,"release_date":"2010-03-16","overview":"A provider description","players":1},
            {"id":844,"game_title":"Remastered","platform":4919,"overview":"Wrong platform"}
        ]}});
        let candidates = fact_candidates(&json, "ps3", 12, &Default::default());
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].facts.as_ref().unwrap().year, Some(2010));
        let temp = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(temp.path().join("data")), false).unwrap();
        core.with_db(|db| {
            db.conn.execute("INSERT INTO editions(id,system,scan_title,imported_at) VALUES('game','ps3','My title','2026-01-01'),('other','ps2','Other title','2026-01-01')", [])?;
            crate::romfacts::upsert(db, "game", &crate::model::GameInfo { description: Some("My existing description".into()), developer: Some("My developer".into()), source: "manual".into(), ..Default::default() })?;
            db.conn.execute("INSERT INTO metadata_search_reviews VALUES('review','game',?1,?2)", params![serde_json::to_string(&candidates)?, now()])?;
            Ok(())
        }).unwrap();
        assert!(core.apply_game_facts("other", "review", 843).is_err());
        assert!(core.apply_metadata_match("game", "review", 843).is_err());
        core.apply_game_facts("game", "review", 843).unwrap();
        let edition = core
            .snapshot()
            .unwrap()
            .editions
            .into_iter()
            .find(|e| e.id == "game")
            .unwrap();
        assert_eq!(edition.title, "My title");
        let info = edition.info.unwrap();
        assert_eq!(info.description.as_deref(), Some("My existing description"));
        assert_eq!(info.developer.as_deref(), Some("My developer"));
        assert_eq!(info.year, Some(2010));
        assert!(info.source.contains("thegamesdb:843"));
        assert!(core.apply_game_facts("game", "review", 843).is_err());
    }
    #[test]
    fn replacing_a_reviewed_match_changes_the_reference_used_by_refresh() {
        let temp = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(temp.path().join("data")), false).unwrap();
        let candidate = Candidate {
            id: 8,
            title: "Correct game".into(),
            system: Some("ps1".into()),
            facts: Some(crate::model::GameInfo {
                year: Some(1999),
                source: "thegamesdb:8".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        core.with_db(|db| {
            db.conn.execute("INSERT INTO editions(id,system,scan_title,imported_at) VALUES('game','ps1','My game','2026-01-01')",[])?;
            crate::romfacts::upsert(db,"game",&crate::model::GameInfo{source:"libretro+thegamesdb:7".into(),..Default::default()})?;
            db.conn.execute("INSERT INTO metadata_search_reviews VALUES('review','game',?1,?2)",params![serde_json::to_string(&vec![candidate])?,now()])?;
            Ok(())
        }).unwrap();
        core.apply_game_facts("game", "review", 8).unwrap();
        let facts = core.snapshot().unwrap().editions[0].info.clone().unwrap();
        assert_eq!(crate::romfacts::tgdb_id(&facts), Some(8));
        assert!(facts.source.contains("libretro"));
        // Preserve historical contributors without letting the old reference win.
        assert!(facts.source.contains("thegamesdb:7"));
        let mut facts = facts;
        crate::romfacts::select_tgdb_id(&mut facts, 7);
        assert_eq!(crate::romfacts::tgdb_id(&facts), Some(7));
    }

    #[test]
    fn provider_reviews_are_bounded_edition_bound_and_expire() {
        let input = serde_json::json!({"data":[{"id":1,"name":"God of War III"},{"id":0,"name":"Invalid"},{"id":2,"name":"Bad\nname"}]});
        assert_eq!(candidates(&input).len(), 1);
        let db = rusqlite::Connection::open_in_memory().unwrap();
        ensure_tables(&db).unwrap();
        let payload = serde_json::to_string(&candidates(&input)).unwrap();
        db.execute(
            "INSERT INTO metadata_search_reviews VALUES('review','game',?1,?2)",
            params![payload, now()],
        )
        .unwrap();
        assert!(reviewed(&db, "other", "review", 1).is_err());
        assert!(reviewed(&db, "game", "review", 2).is_err());
        assert_eq!(
            reviewed(&db, "game", "review", 1).unwrap().title,
            "God of War III"
        );
        db.execute("UPDATE metadata_search_reviews SET created=0", [])
            .unwrap();
        assert!(reviewed(&db, "game", "review", 1).is_err());
    }
}
