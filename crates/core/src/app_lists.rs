//! Original, offline importer for the pinned Quiver apps-list schema.
//! Discovery records only: no runtime paths, downloads or launch commands.
use crate::error::{AppError, Result};
use crate::sessions::EventSink;
use crate::Core;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::Read;
use std::path::Path;

pub const MAX_BYTES: usize = 384 * 1024;
const MAX_ENTRIES: usize = 500;
const FIELDS: &[&str] = &[
    "name",
    "project",
    "customDisplayName",
    "repository",
    "repositorySource",
    "folderName",
    "releaseAssetFilter",
    "tags",
    "appIconUrl",
];
fn error(message: impl Into<String>) -> AppError {
    AppError::new("APP_LIST", message)
}
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
pub fn ensure_tables(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS app_list_reviews(token TEXT PRIMARY KEY,source TEXT NOT NULL,payload TEXT NOT NULL,original TEXT NOT NULL,created INTEGER NOT NULL);
    CREATE TABLE IF NOT EXISTS app_list_sources(source TEXT PRIMARY KEY,sha256 TEXT NOT NULL,original TEXT NOT NULL,imported_at INTEGER NOT NULL);
    CREATE TABLE IF NOT EXISTS app_list_entries(id TEXT PRIMARY KEY,source TEXT NOT NULL,payload TEXT NOT NULL,approved_at INTEGER NOT NULL);")?;
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub source: String,
    pub name: String,
    pub project: Option<String>,
    pub display_name: String,
    pub repository: String,
    pub provider: String,
    pub folder_name: String,
    pub release_asset_filter: Option<String>,
    pub tags: Vec<String>,
    pub icon_url: Option<String>,
    pub recipe_id: Option<String>,
    /// supported_recipe, needs_recipe or unsupported; never playable readiness.
    pub status: String,
    /// add, already_present or update; only discovery records are compared.
    pub change: String,
    pub diagnostics: Vec<String>,
    pub original: Value,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Review {
    pub token: String,
    pub source: String,
    pub sha256: String,
    pub diagnostics: Vec<String>,
    pub entries: Vec<Entry>,
}
fn value(
    map: &serde_json::Map<String, Value>,
    key: &str,
    required: bool,
    max: usize,
) -> Result<Option<String>> {
    let Some(raw) = map.get(key).filter(|v| !v.is_null()) else {
        return if required {
            Err(error(format!("Missing {key}")))
        } else {
            Ok(None)
        };
    };
    let raw = raw
        .as_str()
        .ok_or_else(|| error(format!("{key} must be text")))?;
    if raw.len() > max || raw.chars().any(char::is_control) {
        return Err(error(format!("Invalid or oversized {key}")));
    }
    let text = raw.trim();
    if text.is_empty() && required {
        return Err(error(format!("Empty {key}")));
    }
    Ok((!text.is_empty()).then(|| text.to_string()))
}
fn source_name(source: &str) -> Result<String> {
    let source = source.trim();
    if source.is_empty() || source.len() > 120 || source.chars().any(char::is_control) {
        return Err(error("Use a source name of 1–120 characters"));
    }
    Ok(source.to_lowercase())
}
fn safe_folder(name: &str) -> bool {
    !matches!(name, "." | "..") && !name.contains(['/', '\\', ':']) && !name.ends_with(['.', ' '])
}
fn repository_valid(repository: &str, provider: &str) -> bool {
    let parts: Vec<_> = repository.split('/').collect();
    (if provider == "github" {
        parts.len() == 2
    } else {
        parts.len() >= 2
    }) && parts.iter().all(|part| {
        !part.is_empty()
            && !matches!(*part, "." | "..")
            && part
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
    })
}
fn parse(source: &str, raw: &str) -> Result<(Vec<Entry>, Vec<String>)> {
    if raw.len() > MAX_BYTES {
        return Err(error("App list exceeds 384 KiB"));
    }
    let root: Value = serde_json::from_str(raw)
        .map_err(|_| error("Invalid JSON; no discovery entries changed"))?;
    let object = root.as_object().ok_or_else(|| {
        error("Expected an object containing an apps array; legacy arrays are not supported")
    })?;
    let apps = object
        .get("apps")
        .and_then(Value::as_array)
        .ok_or_else(|| error("Expected an apps array"))?;
    if apps.len() > MAX_ENTRIES {
        return Err(error("App list exceeds 500 entries"));
    }
    let diagnostics = object
        .keys()
        .filter(|key| !["apps", "name", "description", "version"].contains(&key.as_str()))
        .map(|key| {
            format!("Unknown root field {key:?}: retained in the source snapshot, not applied")
        })
        .collect();
    let ports = crate::ports::catalogue()?;
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    for (index, original) in apps.iter().enumerate() {
        let map = original
            .as_object()
            .ok_or_else(|| error(format!("Entry {} must be an object", index + 1)))?;
        let name = value(map, "name", true, 300)?.unwrap();
        let project = value(map, "project", false, 300)?;
        let display_name =
            value(map, "customDisplayName", false, 300)?.unwrap_or_else(|| name.clone());
        let folder_name = value(map, "folderName", true, 160)?.unwrap();
        let repository = value(map, "repository", false, 300)?
            .unwrap_or_default()
            .to_ascii_lowercase();
        let raw_provider = value(map, "repositorySource", false, 40)?;
        let provider = if repository.is_empty() {
            "manual".into()
        } else {
            raw_provider
                .unwrap_or_else(|| "github".into())
                .to_ascii_lowercase()
        };
        let release_asset_filter = value(map, "releaseAssetFilter", false, 200)?;
        let icon_url = value(map, "appIconUrl", false, 2048)?;
        let mut tags = Vec::new();
        if let Some(raw) = map.get("tags").filter(|v| !v.is_null()) {
            let items = raw
                .as_array()
                .filter(|a| a.len() <= 30)
                .ok_or_else(|| error("tags must contain at most 30 strings"))?;
            for item in items {
                let tag = item
                    .as_str()
                    .filter(|s| s.len() <= 80 && !s.chars().any(char::is_control))
                    .ok_or_else(|| error("Invalid tag"))?
                    .trim()
                    .to_lowercase();
                if !tag.is_empty() && !tags.contains(&tag) {
                    tags.push(tag);
                }
            }
        }
        let unknown: Vec<_> = map
            .keys()
            .filter(|key| !FIELDS.contains(&key.as_str()))
            .collect();
        let mut notes: Vec<_> = unknown
            .iter()
            .map(|key| {
                format!("Unsupported field {key:?}: retained for review, never executed or applied")
            })
            .collect();
        let invalid = !safe_folder(&folder_name)
            || (!repository.is_empty()
                && (!matches!(provider.as_str(), "github" | "gitlab")
                    || !repository_valid(&repository, &provider)));
        let matching = ports
            .iter()
            .find(|p| provider == "github" && p.github.eq_ignore_ascii_case(&repository));
        let supported =
            matching.is_some() && unknown.is_empty() && release_asset_filter.is_none() && !invalid;
        if !safe_folder(&folder_name) {
            notes.push(
                "Path-like folderName is unsupported; it is never used as an installation path"
                    .into(),
            );
        }
        if provider == "manual" {
            notes.push("Manual listing: no executable or runtime recipe is inferred".into());
        } else if provider == "gitlab" {
            notes.push("GitLab release installation requires a reviewed Marquee recipe".into());
        } else if !matches!(provider.as_str(), "github" | "gitlab") {
            notes.push("Unsupported repository provider; it is not treated as GitHub".into());
        } else if !repository_valid(&repository, &provider) {
            notes.push("Invalid repository path; expected owner/repository".into());
        }
        if release_asset_filter.is_some() {
            notes.push(
                "External asset filter retained; it cannot change an existing managed recipe"
                    .into(),
            );
        }
        if !supported && !invalid {
            notes.push("A reviewed setup/launch recipe is required before installation".into());
        }
        if icon_url.is_some() {
            notes.push("Remote icon retained as source metadata; it is not fetched".into());
        }
        let identity = format!(
            "{source}\0{provider}\0{repository}\0{}",
            folder_name.to_lowercase()
        );
        let id = format!("app-list:{:x}", Sha256::digest(identity.as_bytes()));
        if !seen.insert(id.clone()) {
            return Err(error(format!(
                "Duplicate app identity at entry {}; give distinct apps distinct folderName values",
                index + 1
            )));
        }
        entries.push(Entry {
            id,
            source: source.into(),
            name,
            project,
            display_name,
            repository,
            provider,
            folder_name,
            release_asset_filter,
            tags,
            icon_url,
            recipe_id: supported.then(|| matching.unwrap().id.clone()),
            status: if invalid {
                "unsupported"
            } else if supported {
                "supported_recipe"
            } else {
                "needs_recipe"
            }
            .into(),
            change: "add".into(),
            diagnostics: notes,
            original: original.clone(),
        });
    }
    Ok((entries, diagnostics))
}
pub fn read_local(path: &Path) -> Result<String> {
    if !std::fs::symlink_metadata(path)?.is_file() {
        return Err(error("Choose a regular local JSON file"));
    }
    let mut raw = String::new();
    std::fs::File::open(path)?
        .take(MAX_BYTES as u64 + 1)
        .read_to_string(&mut raw)?;
    if raw.len() > MAX_BYTES {
        return Err(error("App list exceeds 384 KiB"));
    }
    Ok(raw)
}
impl Core {
    pub fn preview_app_list(&self, source: &str, raw: &str) -> Result<Review> {
        let source = source_name(source)?;
        let (mut entries, diagnostics) = parse(&source, raw)?;
        let token = uuid::Uuid::new_v4().to_string();
        let sha256 = format!("{:x}", Sha256::digest(raw.as_bytes()));
        self.with_db(|db| {
            for entry in &mut entries {
                let stored: Option<String> = db
                    .conn
                    .query_row(
                        "SELECT payload FROM app_list_entries WHERE id=?1",
                        [&entry.id],
                        |r| r.get(0),
                    )
                    .optional()?;
                if let Some(stored) = stored {
                    let previous: Entry = serde_json::from_str(&stored)?;
                    entry.change = if previous.original == entry.original {
                        "already_present"
                    } else {
                        "update"
                    }
                    .into();
                }
            }
            let review = Review {
                token: token.clone(),
                source: source.clone(),
                sha256: sha256.clone(),
                diagnostics: diagnostics.clone(),
                entries: entries.clone(),
            };
            let tx = db.conn.unchecked_transaction()?;
            tx.execute(
                "DELETE FROM app_list_reviews WHERE source=?1 OR created<?2",
                params![source, now() - 1800],
            )?;
            tx.execute(
                "INSERT INTO app_list_reviews VALUES(?1,?2,?3,?4,?5)",
                params![token, source, serde_json::to_string(&review)?, raw, now()],
            )?;
            tx.commit()?;
            Ok(())
        })?;
        Ok(Review {
            token,
            source,
            sha256,
            diagnostics,
            entries,
        })
    }
    pub fn approve_app_list(&self, token: &str, ids: &[String]) -> Result<usize> {
        if ids.is_empty()
            || ids.len() > MAX_ENTRIES
            || ids.iter().collect::<HashSet<_>>().len() != ids.len()
        {
            return Err(error("Choose 1–500 distinct reviewed entries"));
        }
        let count = self.with_db(|db| {
            let tx = db.conn.unchecked_transaction()?;
            let (payload,original,created):(String,String,i64)=tx.query_row("SELECT payload,original,created FROM app_list_reviews WHERE token=?1",[token],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|error("Review changed; preview this list again"))?;
            if created<now()-1800 { return Err(error("Review expired; preview this list again")); }
            let review:Review=serde_json::from_str(&payload)?;
            let selected:Vec<_>=review.entries.iter().filter(|e|ids.contains(&e.id)).collect();
            if selected.len()!=ids.len() { return Err(error("An entry was not in this review")); }
            tx.execute("INSERT INTO app_list_sources VALUES(?1,?2,?3,?4) ON CONFLICT(source) DO UPDATE SET sha256=excluded.sha256,original=excluded.original,imported_at=excluded.imported_at",params![review.source,review.sha256,original,now()])?;
            for entry in selected {
                tx.execute("INSERT INTO app_list_entries VALUES(?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload,approved_at=excluded.approved_at",params![entry.id,review.source,serde_json::to_string(entry)?,now()])?;
            }
            tx.execute("DELETE FROM app_list_reviews WHERE token=?1",[token])?;
            tx.commit()?; Ok(ids.len())
        })?;
        self.events.emit(crate::CoreEvent::LibraryChanged);
        Ok(count)
    }
    pub fn app_list_entries(&self) -> Result<Vec<Entry>> {
        self.with_db(|db| {
            let mut stmt = db
                .conn
                .prepare("SELECT payload FROM app_list_entries ORDER BY source,id")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const PILOTS: &str = include_str!("../../../catalog/app-lists/pilots.json");
    #[test]
    fn bounded_schema_and_unsupported_requirements_never_become_recipes() {
        let (entries, _) = parse("pilot", PILOTS).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].recipe_id.as_deref(), Some("shipwright"));
        assert_eq!(entries[1].status, "needs_recipe");
        for bad in [
            "{}",
            "[]",
            "{\"apps\":[{}]}",
            "{\"apps\":[1]}",
            "{\"apps\":null}",
        ] {
            assert!(parse("pilot", bad).is_err());
        }
        assert!(parse("pilot", &" ".repeat(MAX_BYTES + 1)).is_err());
        assert!(parse(
            "pilot",
            r#"{"apps":[{"name":"Manual","folderName":"Manual","repositorySource":{}}]}"#
        )
        .is_err());
        let mut root: Value = serde_json::from_str(PILOTS).unwrap();
        root["apps"][0]["linuxCustomLaunchCommand"] = Value::String("touch /tmp/never-run".into());
        root["apps"][1]["folderName"] = Value::String("../../escape".into());
        let (entries, _) = parse("pilot", &root.to_string()).unwrap();
        assert!(entries[0].recipe_id.is_none());
        assert!(entries[0]
            .original
            .get("linuxCustomLaunchCommand")
            .is_some());
        assert_eq!(entries[1].status, "unsupported");
        root["apps"][1] = root["apps"][0].clone();
        assert!(parse("pilot", &root.to_string()).is_err());
        root["apps"][1]["folderName"] = Value::String("AnotherApp".into());
        assert_eq!(parse("pilot", &root.to_string()).unwrap().0.len(), 2);
        root["apps"][1]["repositorySource"] = Value::String("unknown".into());
        assert_eq!(
            parse("pilot", &root.to_string()).unwrap().0[1].status,
            "unsupported"
        );
    }
    #[test]
    fn source_updates_keep_identity_and_replace_only_the_reviewed_discovery_record() {
        let temp = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(temp.path().join("data")), true).unwrap();
        let first = core.preview_app_list("source", PILOTS).unwrap();
        let ids: Vec<_> = first.entries.iter().map(|e| e.id.clone()).collect();
        core.approve_app_list(&first.token, &ids).unwrap();
        let stale = core.preview_app_list("source", PILOTS).unwrap();
        let mut updated: Value = serde_json::from_str(PILOTS).unwrap();
        updated["apps"][0]["customDisplayName"] = Value::String("A new discovery label".into());
        let newest = core
            .preview_app_list("source", &updated.to_string())
            .unwrap();
        assert_eq!(newest.entries[0].id, ids[0]);
        assert_eq!(newest.entries[0].change, "update");
        assert_eq!(newest.entries[1].change, "already_present");
        assert!(core.approve_app_list(&stale.token, &ids).is_err());
        core.approve_app_list(&newest.token, &ids[..1]).unwrap();
        let entries = core.app_list_entries().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries
                .iter()
                .find(|e| e.id == ids[0])
                .unwrap()
                .display_name,
            "A new discovery label"
        );
    }
    #[test]
    fn reviewed_import_survives_restart_and_preserves_the_game_library() {
        let temp = tempfile::tempdir().unwrap();
        let data = temp.path().join("data");
        let core = Core::open_isolated(Some(data.clone()), true).unwrap();
        core.with_db(|db| {db.conn.execute("INSERT INTO editions(id,system,scan_title,display_title,favorite,playtime_seconds,imported_at) VALUES('game','ps2','Original','My title',1,123,'2026-01-01')",[])?;Ok(())}).unwrap();
        let review = core.preview_app_list("My list", PILOTS).unwrap();
        assert!(core.app_list_entries().unwrap().is_empty());
        let ids: Vec<_> = review.entries.iter().map(|e| e.id.clone()).collect();
        assert!(core
            .approve_app_list(&review.token, &["unreviewed".into()])
            .is_err());
        assert_eq!(core.approve_app_list(&review.token, &ids).unwrap(), 2);
        assert!(core.approve_app_list(&review.token, &ids).is_err());
        let before = core.snapshot().unwrap().editions[0].clone();
        drop(core);
        let core = Core::open_isolated(Some(data), true).unwrap();
        assert_eq!(core.app_list_entries().unwrap().len(), 2);
        let review = core.preview_app_list("MY LIST", PILOTS).unwrap();
        assert!(review.entries.iter().all(|e| e.change == "already_present"));
        assert_eq!(
            review
                .entries
                .iter()
                .map(|e| e.id.clone())
                .collect::<Vec<_>>(),
            ids
        );
        let other = core.preview_app_list("another source", PILOTS).unwrap();
        assert_ne!(other.entries[0].id, ids[0]);
        core.with_db(|db| {
            db.conn.execute(
                "UPDATE app_list_reviews SET created=0 WHERE token=?1",
                [&review.token],
            )?;
            Ok(())
        })
        .unwrap();
        assert!(core.approve_app_list(&review.token, &ids).is_err());
        assert!(core.preview_app_list("My list", "bad JSON").is_err());
        assert_eq!(core.app_list_entries().unwrap().len(), 2);
        let after = core.snapshot().unwrap().editions[0].clone();
        assert_eq!(after.title, before.title);
        assert_eq!(after.favorite, before.favorite);
        assert_eq!(after.playtime_seconds, before.playtime_seconds);
        assert!(core.paths.data_dir.join("ports").read_dir().is_err());
    }
}
