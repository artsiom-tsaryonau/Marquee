//! The `.marquee` game package manifest (`marquee.json`). Export and import live in
//! `marquee_core::packages`.
//!
//! Schema 2 replaces the unused schema 1 contract: one package carries a whole
//! game (`game/`, plus PS4 `update/` and `addcont/`), and it needs no manager
//! preparation receipt, which games prepared by hand do not have.

use crate::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Component, Path};

pub const SCHEMA: u32 = 2;
pub const MANIFEST_NAME: &str = "marquee.json";
const MAX_FILES: usize = 500_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    pub platform: String,
    /// Serial or title ID; empty when the platform has none.
    #[serde(default)]
    pub title_id: String,
    pub title: String,
    /// Folder the game lives in, for folder games. A single-file game has none.
    #[serde(default)]
    pub folder: Option<String>,
    /// The file the library launches, relative to `game/`.
    pub entry: String,
    pub files: Vec<PayloadFile>,
    pub runtime: RuntimeRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

/// What ran the game where it was exported. Informational: the receiving Marquee
/// uses its own catalogue recipe and never runs anything from a package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeRef {
    pub id: String,
    #[serde(default)]
    pub version: Option<String>,
    pub os: String,
    pub cpu: String,
}

impl Manifest {
    pub fn total_bytes(&self) -> u64 {
        self.files
            .iter()
            .fold(0u64, |sum, file| sum.saturating_add(file.size))
    }
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::new("PORTABLE", message.into())
}

pub fn parse(raw: &str) -> Result<Manifest> {
    let manifest: Manifest = serde_json::from_str(raw)?;
    check(&manifest)?;
    Ok(manifest)
}

pub fn check(manifest: &Manifest) -> Result<()> {
    if manifest.schema != SCHEMA {
        return Err(invalid(format!(
            "portable package schema {} is not supported",
            manifest.schema
        )));
    }
    if manifest.files.is_empty() || manifest.files.len() > MAX_FILES {
        return Err(invalid("package has no payload files, or too many"));
    }
    // The title ID names a folder in the PS4 add-on tree.
    if !manifest
        .title_id
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(invalid("title ID has characters that cannot name a folder"));
    }
    if let Some(folder) = &manifest.folder {
        check_name(folder)?;
        if ["-UPDATE", "-patch", "-mods"]
            .iter()
            .any(|suffix| folder.ends_with(suffix))
        {
            return Err(invalid("a game folder cannot be named like an overlay"));
        }
    }
    check_relative(&manifest.entry)?;
    let ps4 = manifest.platform == "ps4";
    if ps4 && (manifest.folder.is_none() || manifest.title_id.is_empty()) {
        return Err(invalid("a PS4 package needs its folder and title ID"));
    }
    let mut seen = HashSet::new();
    for file in &manifest.files {
        check_relative(&file.path)?;
        let (part, rest) = file.path.split_once('/').unwrap_or((&file.path, ""));
        let allowed = part == "game" || (ps4 && (part == "update" || part == "addcont"));
        // Add-ons sit in one folder per entitlement.
        if !allowed || rest.is_empty() || (part == "addcont" && !rest.contains('/')) {
            return Err(invalid(format!(
                "unexpected payload location: {}",
                file.path
            )));
        }
        if !seen.insert(file.path.as_str()) {
            return Err(invalid(format!("{} is listed twice", file.path)));
        }
        if file.sha256.len() != 64 || !file.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(invalid(format!("{} has no 64-character sha256", file.path)));
        }
    }
    let entry = format!("game/{}", manifest.entry);
    if !seen.contains(entry.as_str()) {
        return Err(invalid("the launch entry is not in the package"));
    }
    // A single-file game is exactly its entry.
    if manifest.folder.is_none() && (manifest.files.len() != 1 || manifest.entry.contains('/')) {
        return Err(invalid("a single-file game holds one file"));
    }
    Ok(())
}

fn check_name(name: &str) -> Result<()> {
    check_relative(name)?;
    if name.contains('/') || name.starts_with('.') {
        return Err(invalid(format!("not a plain folder name: {name}")));
    }
    Ok(())
}

fn check_relative(path: &str) -> Result<()> {
    let value = Path::new(path);
    if path.is_empty()
        || path.contains(['\\', '\0'])
        || path.ends_with('/')
        || path.contains("//")
        || value.is_absolute()
        || value
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid(format!(
            "payload path must stay inside the package: {path}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixture_parses_and_unsafe_manifests_are_rejected() {
        let raw = include_str!("../tests/fixtures/portable-v2.json");
        let manifest = parse(raw).unwrap();
        assert_eq!(manifest.total_bytes(), 36);
        for bad in [
            "../escape.bin",
            "/tmp/game.bin",
            "game/../../x.bin",
            "game//x",
            "saves/slot",
            "addcont/loose-file",
            "",
        ] {
            let mut copy = manifest.clone();
            copy.files[1].path = bad.into();
            assert!(check(&copy).is_err(), "{bad}");
        }
        let changes: [fn(&mut Manifest); 6] = [
            |m| m.schema = 1,
            |m| m.folder = Some("../Game".into()),
            |m| m.folder = Some("Game-UPDATE".into()),
            |m| m.title_id = "../x".into(),
            |m| m.entry = "missing.bin".into(),
            |m| m.files.push(m.files[0].clone()),
        ];
        for (index, change) in changes.iter().enumerate() {
            let mut copy = manifest.clone();
            change(&mut copy);
            assert!(check(&copy).is_err(), "change {index}");
        }
    }
}
