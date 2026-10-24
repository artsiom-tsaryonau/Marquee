//! Reviewed provider identities. Search is separate from applying an artwork
//! match; provider IDs are resolved from a short-lived server-side review.
use crate::sessions::EventSink;
use crate::{
    error::{AppError, Result},
    Core,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
fn err(message: &str) -> AppError {
    AppError::new("METADATA_SEARCH", message)
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub facts: Option<crate::model::GameInfo>,
    #[serde(default)]
    pub system: Option<String>,
}
#[derive(Serialize)]
pub struct Review {
    pub token: String,
    pub provider: String,
    pub candidates: Vec<Candidate>,
}
pub fn ensure_tables(db: &rusqlite::Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS metadata_search_reviews(token TEXT PRIMARY KEY,edition_id TEXT NOT NULL,payload TEXT NOT NULL,created INTEGER NOT NULL);
    CREATE TABLE IF NOT EXISTS metadata_provider_matches(edition_id TEXT PRIMARY KEY,provider TEXT NOT NULL,provider_id TEXT NOT NULL,title TEXT NOT NULL);")?;
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
            "https://api.thegamesdb.net/v1/Games/ByGameName?apikey={}&name={}&fields=overview,players,genres,platform&filter%5Bplatform%5D={platform}",
            crate::romart::encode(&key), crate::romart::encode(query)
        )).map_err(|_| err("TheGamesDB search failed. Check your API key, allowance and connection."))?;
        let genres = crate::romfacts::tgdb_genres(&self.paths.cache_dir, &key);
        let items = fact_candidates(&json, &system, platform, &genres);
        let token = Uuid::new_v4().to_string();
        self.with_db(|db| {
            db.conn.execute(
                "DELETE FROM metadata_search_reviews WHERE edition_id=?1 OR created<?2",
                params![edition_id, now() - 1800],
            )?;
            db.conn.execute(
                "INSERT INTO metadata_search_reviews VALUES(?1,?2,?3,?4)",
                params![token, edition_id, serde_json::to_string(&items)?, now()],
            )?;
            Ok(())
        })?;
        Ok(Review {
            token,
            provider: format!("TheGamesDB · {system} game details · review the release"),
            candidates: items,
        })
    }

    pub fn apply_game_facts(&self, edition_id: &str, token: &str, id: u64) -> Result<()> {
        self.with_db(|db| {
            let transaction = db.conn.unchecked_transaction()?;
            let candidate = reviewed(&db.conn, edition_id, token, id)?;
            let facts = candidate
                .facts
                .ok_or_else(|| err("This is an artwork result, not a game-details match"))?;
            let edition = db
                .edition_by_id(edition_id)?
                .ok_or_else(|| err("Game was removed"))?;
            if candidate.system.as_deref() != Some(edition.system.as_str()) {
                return Err(err("Game platform changed; search again"));
            }
            let mut info = crate::romfacts::list(db)?
                .remove(edition_id)
                .unwrap_or_default();
            crate::romfacts::fill(&mut info, &facts);
            crate::romfacts::add_source(&mut info, &format!("thegamesdb:{id}"));
            crate::romfacts::upsert(db, edition_id, &info)?;
            db.conn.execute(
                "DELETE FROM metadata_search_reviews WHERE token=?1",
                [token],
            )?;
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
fn fact_candidates(
    json: &Value,
    system: &str,
    platform: u32,
    genres: &std::collections::BTreeMap<u64, String>,
) -> Vec<Candidate> {
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
                .filter_map(|id| genres.get(&id.as_u64()?))
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
                facts: Some(crate::model::GameInfo {
                    genre: (!genre.is_empty()).then(|| genre.join(", ")),
                    year,
                    players,
                    description,
                    developer: None,
                    publisher: None,
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
