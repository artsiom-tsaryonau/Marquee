//! Games in the library without files: a wishlist, or games you own whose files
//! are elsewhere.
//!
//! A wanted game is an ordinary edition with no media, so cover art, facts,
//! favourites and search work for it unchanged, and Play stays disabled until
//! there is something to play. It carries a row in `wanted` with its status.
//! When a scan or an archive intake brings in a file for the same system whose
//! name is the same game, the file is attached to that edition and the wanted
//! row is removed.

use crate::db::{now_rfc3339, Db};
use crate::error::{AppError, Result};
use crate::romart;
use crate::Core;
use crate::EventSink;
use rusqlite::{params, Connection};
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

pub const STATUSES: [&str; 2] = ["wishlist", "owned"];
const TITLE_MAX: usize = 200;

pub fn ensure_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS wanted (
            edition_id TEXT PRIMARY KEY,
            status TEXT NOT NULL
        );",
    )?;
    Ok(())
}

pub fn statuses(db: &Db) -> Result<BTreeMap<String, String>> {
    let mut stmt = db.conn.prepare("SELECT edition_id, status FROM wanted")?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// A wanted game that this file is: same system, no files yet, same game name.
/// The match removes its wanted row, so the entry becomes an ordinary game.
pub fn claim(db: &Db, system: &str, file_title: &str) -> Result<Option<String>> {
    let mut stmt = db.conn.prepare(
        "SELECT e.id, COALESCE(e.display_title, e.scan_title)
         FROM editions e JOIN wanted w ON w.edition_id = e.id
         WHERE e.system = ?1 AND e.media_id IS NULL",
    )?;
    let candidates = stmt
        .query_map([system], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    let found = candidates
        .into_iter()
        .find(|(_, title)| romart::names_agree(file_title, &romart::plain_title(title)));
    let Some((id, _)) = found else {
        return Ok(None);
    };
    db.conn
        .execute("DELETE FROM wanted WHERE edition_id = ?1", [&id])?;
    Ok(Some(id))
}

/// The official list entry for a typed title: the same game, preferring the
/// USA release, then Europe, then World, and never homebrew or a prototype
/// over a retail release. Returns the checksum and the official name.
pub fn best_match(list: &HashMap<u32, String>, title: &str) -> Option<(u32, String)> {
    let wanted = romart::plain_title(title);
    let rank = |name: &str| {
        let l = name.to_lowercase();
        let region = if l.contains("(usa") {
            0
        } else if l.contains("(europe") {
            1
        } else if l.contains("(world") {
            2
        } else {
            3
        };
        (u8::from(!romart::looks_retail(name)), region, name.len())
    };
    list.iter()
        .filter(|(_, name)| romart::names_agree(&wanted, &romart::plain_title(name)))
        .min_by(|a, b| rank(a.1).cmp(&rank(b.1)).then_with(|| a.1.cmp(b.1)))
        .map(|(crc, name)| (*crc, name.clone()))
}

impl Core {
    /// Add a game that has no files yet. Returns its edition id.
    pub fn add_wanted(&self, system: &str, title: &str, status: &str) -> Result<String> {
        let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
        if title.is_empty() || title.chars().count() > TITLE_MAX {
            return Err(AppError::new(
                "BAD_REQUEST",
                "a game title is one to 200 characters",
            ));
        }
        if !STATUSES.contains(&status) {
            return Err(AppError::new("BAD_REQUEST", "status is wishlist or owned"));
        }
        let descriptor = crate::catalog::by_id(system)?;
        let id = self.with_db(|db| {
            let duplicate: Option<String> = db
                .conn
                .query_row(
                    "SELECT e.id FROM editions e JOIN wanted w ON w.edition_id = e.id
                     WHERE e.system = ?1 AND lower(COALESCE(e.display_title, e.scan_title)) = lower(?2)",
                    params![descriptor.id, title],
                    |r| r.get(0),
                )
                .ok();
            if let Some(id) = duplicate {
                return Ok(id);
            }
            let id = Uuid::new_v4().to_string();
            db.upsert_edition(&id, &descriptor.id, &title, None, &now_rfc3339())?;
            // A typed title is the user's own name; it also keeps the entry
            // through "remove missing games".
            db.set_display_title(&id, Some(&title))?;
            db.conn.execute(
                "INSERT INTO wanted(edition_id, status) VALUES(?1, ?2)",
                params![id, status],
            )?;
            Ok(id)
        })?;
        self.events.emit(crate::CoreEvent::LibraryChanged);
        Ok(id)
    }

    pub fn set_wanted_status(&self, edition_id: &str, status: &str) -> Result<()> {
        if !STATUSES.contains(&status) {
            return Err(AppError::new("BAD_REQUEST", "status is wishlist or owned"));
        }
        let n = self.with_db(|db| {
            Ok(db.conn.execute(
                "UPDATE wanted SET status = ?2 WHERE edition_id = ?1",
                params![edition_id, status],
            )?)
        })?;
        if n == 0 {
            return Err(AppError::new("MEDIA_MISSING", "not a game without files"));
        }
        self.events.emit(crate::CoreEvent::LibraryChanged);
        Ok(())
    }

    /// Remove a game that has no files. A game with files is never removed here.
    pub fn remove_wanted(&self, edition_id: &str) -> Result<()> {
        self.with_db(|db| {
            let listed: i64 = db.conn.query_row(
                "SELECT COUNT(*) FROM wanted w JOIN editions e ON e.id = w.edition_id
                 WHERE w.edition_id = ?1 AND e.media_id IS NULL",
                [edition_id],
                |r| r.get(0),
            )?;
            if listed == 0 {
                return Err(AppError::new("MEDIA_MISSING", "not a game without files"));
            }
            for sql in [
                "DELETE FROM wanted WHERE edition_id = ?1",
                "DELETE FROM artwork WHERE edition_id = ?1",
                "DELETE FROM edition_info WHERE edition_id = ?1",
                "DELETE FROM launch_profiles WHERE edition_id = ?1",
                "DELETE FROM catalogue_links WHERE edition_id = ?1",
                "DELETE FROM editions WHERE id = ?1",
            ] {
                db.conn.execute(sql, [edition_id])?;
            }
            Ok(())
        })?;
        self.events.emit(crate::CoreEvent::LibraryChanged);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_usa_release_of_a_typed_title_is_preferred() {
        let list = HashMap::from([
            (1, "Super Mario Bros. (Japan)".to_string()),
            (2, "Super Mario Bros. (USA)".to_string()),
            (3, "Super Mario Bros. (World) (Homebrew)".to_string()),
            (4, "Super Mario Bros. 3 (USA)".to_string()),
        ]);
        assert_eq!(
            best_match(&list, "super mario bros."),
            Some((2, "Super Mario Bros. (USA)".to_string()))
        );
        assert_eq!(best_match(&list, "Zelda"), None);
    }

    #[test]
    fn a_file_claims_its_wanted_game_and_only_that_one() {
        let dir = tempfile::tempdir().unwrap();
        let core = Core::open(Some(dir.path().join("data")), true).unwrap();
        let wanted = core
            .add_wanted("nes", "Super Mario Bros.", "wishlist")
            .unwrap();
        let other = core.add_wanted("nes", "Metroid", "owned").unwrap();
        // Adding the same title again returns the same entry.
        assert_eq!(
            core.add_wanted("nes", "super  mario bros.", "owned")
                .unwrap(),
            wanted
        );
        let snap = core.snapshot().unwrap();
        let e = snap.editions.iter().find(|e| e.id == wanted).unwrap();
        assert_eq!(e.wanted.as_deref(), Some("wishlist"));
        assert!(e.media_id.is_none());
        core.with_db(|db| {
            assert_eq!(claim(db, "snes", "Super Mario Bros. (USA)")?, None);
            assert_eq!(
                claim(db, "nes", "Super Mario Bros. (USA)")?,
                Some(wanted.clone())
            );
            assert_eq!(claim(db, "nes", "Super Mario Bros. (USA)")?, None);
            Ok(())
        })
        .unwrap();
        let snap = core.snapshot().unwrap();
        assert!(snap
            .editions
            .iter()
            .find(|e| e.id == wanted)
            .unwrap()
            .wanted
            .is_none());
        assert_eq!(
            snap.editions
                .iter()
                .find(|e| e.id == other)
                .unwrap()
                .wanted
                .as_deref(),
            Some("owned")
        );
    }

    #[test]
    fn validation_status_changes_and_removal() {
        let dir = tempfile::tempdir().unwrap();
        let core = Core::open(Some(dir.path().join("data")), true).unwrap();
        assert!(core.add_wanted("nes", "   ", "wishlist").is_err());
        assert!(core.add_wanted("nes", "X", "maybe").is_err());
        assert!(core.add_wanted("amiga", "Lemmings", "wishlist").is_err());
        let id = core.add_wanted("ps2", "Okami", "wishlist").unwrap();
        core.set_wanted_status(&id, "owned").unwrap();
        let snap = core.snapshot().unwrap();
        assert_eq!(snap.editions[0].wanted.as_deref(), Some("owned"));
        // It survives "remove missing games".
        assert_eq!(core.prune_missing().unwrap(), 0);
        core.remove_wanted(&id).unwrap();
        assert!(core.snapshot().unwrap().editions.is_empty());
        assert!(core.remove_wanted(&id).is_err());
    }

    #[test]
    fn scanning_a_matching_file_makes_the_game_playable_without_a_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        let games = dir.path().join("nes");
        std::fs::create_dir_all(&games).unwrap();
        std::fs::write(games.join("Super Mario Bros. (USA).nes"), vec![1u8; 4096]).unwrap();
        std::fs::write(games.join("Other Thing.nes"), vec![2u8; 4096]).unwrap();
        let core = Core::open(Some(dir.path().join("data")), true).unwrap();
        let wanted = core
            .add_wanted("nes", "Super Mario Bros.", "wishlist")
            .unwrap();
        let root = core.add_root("nes", games, None).unwrap();
        core.scan(Some(&root.id), None).unwrap();
        let snap = core.snapshot().unwrap();
        assert_eq!(snap.editions.len(), 2, "no duplicate entry");
        let e = snap.editions.iter().find(|e| e.id == wanted).unwrap();
        assert!(e.media_id.is_some() && e.wanted.is_none());
        assert_eq!(e.title, "Super Mario Bros.", "the typed title is kept");
    }
}
