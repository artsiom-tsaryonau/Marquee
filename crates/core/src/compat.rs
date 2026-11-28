//! The compatibility registry (`catalog/compat.json`): what has been seen to run
//! on the owner's machine, and what each game needed. It is the one place play
//! results are recorded.

use crate::{AppError, Core, Result};
use serde::{Deserialize, Serialize};

const REGISTRY: &str = include_str!("../../../catalog/compat.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Controlled in the game for a while; not completed.
    Gameplay,
    /// Menus, story or setup reached.
    Menus,
    /// Video or intro only.
    Boots,
    /// Waits for a decision only the owner can make (an agreement).
    Owner,
    Broken,
    Untested,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub system: String,
    /// The game's serial or title ID; the library title when it has none.
    pub key: String,
    pub status: Status,
    pub checked: String,
    pub note: String,
    /// What the emulator's own public list said when checked, for comparison.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runner: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Row {
    pub system: String,
    pub title: String,
    pub status: Status,
    pub checked: Option<String>,
    pub note: Option<String>,
    pub upstream: Option<String>,
}

#[derive(Deserialize)]
struct File {
    schema: u32,
    games: Vec<Entry>,
}

pub fn entries() -> Result<Vec<Entry>> {
    let file: File = serde_json::from_str(REGISTRY)?;
    if file.schema != 1 {
        return Err(AppError::new("COMPAT", "unsupported registry schema"));
    }
    Ok(file.games)
}

/// The entry for a game: by serial when it has one, else by title.
pub fn lookup<'a>(
    entries: &'a [Entry],
    system: &str,
    serial: Option<&str>,
    title: &str,
) -> Option<&'a Entry> {
    let of_system = || entries.iter().filter(move |e| e.system == system);
    serial
        .and_then(|serial| of_system().find(|e| e.key == serial))
        .or_else(|| of_system().find(|e| e.key == title))
}

impl Core {
    /// Every library game with what the registry knows about it, best first.
    pub fn compat_report(&self) -> Result<Vec<Row>> {
        let registry = entries()?;
        let mut rows: Vec<Row> = self
            .snapshot()?
            .editions
            .into_iter()
            .map(|game| {
                let found = lookup(
                    &registry,
                    &game.system,
                    game.serial.as_deref(),
                    &game.scan_title,
                );
                Row {
                    status: found.map_or(Status::Untested, |e| e.status),
                    checked: found.map(|e| e.checked.clone()),
                    note: found.map(|e| e.note.clone()),
                    upstream: found.and_then(|e| e.upstream.clone()),
                    system: game.system,
                    title: game.title,
                }
            })
            .collect();
        rows.sort_by(|a, b| (a.status, &a.system, &a.title).cmp(&(b.status, &b.system, &b.title)));
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_parses_and_names_each_game_once() {
        let all = entries().unwrap();
        let mut seen = std::collections::HashSet::new();
        for entry in &all {
            assert!(
                crate::catalog::by_id(&entry.system).is_ok(),
                "{}",
                entry.key
            );
            assert!(seen.insert((&entry.system, &entry.key)), "{}", entry.key);
            assert!(entry.checked.len() == 10 && !entry.note.is_empty());
            if let Some(runner) = &entry.runner {
                assert!(crate::emulators::find(runner).is_ok(), "{runner}");
            }
        }
        let found = lookup(&all, "ps4", Some("CUSA00556"), "renamed").unwrap();
        assert_eq!(found.status, Status::Gameplay);
        assert!(lookup(&all, "ps3", Some("CUSA00556"), "renamed").is_none());
    }
}
