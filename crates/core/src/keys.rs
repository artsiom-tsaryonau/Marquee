//! API keys and other per-user configuration (SteamGridDB, Steam Web API, IGDB, ...).
//!
//! * Adding a key to Marquee means adding one entry to [`KEYS`]; the Settings screen
//!   and the web/desktop API pick it up with no further code.
//! * A value is looked up in this order: **saved in Marquee**, then the process
//!   environment, then a `.env` file (the repo's, while developing).
//! * Saved values live in `<config dir>/keys.json`, readable only by the user.
//! * A secret is never sent to the UI or over HTTP. The UI sees only where a key comes
//!   from and a masked preview. Features read the real value through [`KeyStore::get`].

use crate::error::{AppError, Result};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct KeyDef {
    pub id: &'static str,
    /// Environment variable (and `.env` entry) name.
    pub env: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    /// Where the user gets it, in words (no link is opened for them).
    pub help: &'static str,
    pub used_by: &'static str,
    /// Secrets are masked in the UI; non-secrets (like a SteamID) are shown in full.
    pub secret: bool,
}

pub const KEYS: &[KeyDef] = &[
    KeyDef {
        id: "steamgriddb",
        env: "STEAM_GRID_DB_API_KEY",
        label: "SteamGridDB API key",
        description: "Covers, heroes and logos for Steam, Lutris and port games.",
        help: "Free: steamgriddb.com, sign in, Preferences, API.",
        used_by: "Artwork",
        secret: true,
    },
    KeyDef {
        id: "steam_web_api",
        env: "STEAM_WEB_API_KEY",
        label: "Steam Web API key",
        description: "Lists games you own but have not installed, with playtime. Your own key; never share it.",
        help: "Free: steamcommunity.com/dev/apikey (personal use).",
        used_by: "Steam library",
        secret: true,
    },
    KeyDef {
        id: "steam_id",
        env: "STEAM_ID64",
        label: "Your SteamID64",
        description: "The 17-digit number that identifies your Steam account. Not a secret.",
        help: "From your profile address, or a lookup such as steamid.io.",
        used_by: "Steam library, wishlist",
        secret: false,
    },
    KeyDef {
        id: "twitch_client_id",
        env: "TWITCH_CLIENT_ID",
        label: "IGDB (Twitch) client ID",
        description: "IGDB is run by Twitch. Used with the client secret for ratings and descriptions.",
        help: "Free: dev.twitch.tv, register an application.",
        used_by: "Game info",
        secret: false,
    },
    KeyDef {
        id: "twitch_client_secret",
        env: "TWITCH_CLIENT_SECRET",
        label: "IGDB (Twitch) client secret",
        description: "Pairs with the client ID above.",
        help: "Same Twitch application page.",
        used_by: "Game info",
        secret: true,
    },
    KeyDef {
        id: "rawg",
        env: "RAWG_API_KEY",
        label: "RAWG API key",
        description: "Fallback for game info and scores.",
        help: "Free: rawg.io/apidocs.",
        used_by: "Game info",
        secret: true,
    },
];

const MAX_VALUE_LEN: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Saved from Marquee's Settings.
    App,
    /// The process environment.
    Env,
    /// A `.env` file.
    DotEnv,
    None,
}

/// What the UI is allowed to see. Never contains a secret value.
#[derive(Debug, Clone, Serialize)]
pub struct KeyInfo {
    pub id: String,
    pub env: String,
    pub label: String,
    pub description: String,
    pub help: String,
    pub used_by: String,
    pub secret: bool,
    pub source: Source,
    pub set: bool,
    /// Masked for secrets (`••••1a2b`), full text for non-secrets, `None` when unset.
    pub preview: Option<String>,
}

pub struct KeyStore {
    file: PathBuf,
    /// Explicit `.env` path (tests, `MARQUEE_ENV_FILE`); otherwise found by walking up from the cwd.
    dotenv: Option<PathBuf>,
    /// Use the real process environment. Tests turn this off to stay hermetic.
    read_env: bool,
    write_lock: Mutex<()>,
}

pub fn find(id: &str) -> Result<&'static KeyDef> {
    KEYS.iter()
        .find(|k| k.id == id)
        .ok_or_else(|| AppError::new("KEY_UNKNOWN", format!("unknown key: {id}")))
}

impl KeyStore {
    pub fn new(config_dir: &Path) -> Self {
        Self {
            file: config_dir.join("keys.json"),
            dotenv: std::env::var_os("MARQUEE_ENV_FILE").map(PathBuf::from),
            read_env: true,
            write_lock: Mutex::new(()),
        }
    }

    /// A store that ignores the process environment and any `.env` unless one is given.
    #[cfg(test)]
    pub fn isolated(config_dir: &Path, dotenv: Option<PathBuf>) -> Self {
        Self {
            file: config_dir.join("keys.json"),
            dotenv,
            read_env: false,
            write_lock: Mutex::new(()),
        }
    }

    fn saved(&self) -> BTreeMap<String, String> {
        fs::read_to_string(&self.file)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    fn dotenv_values(&self) -> BTreeMap<String, String> {
        let path = self.dotenv.clone().or_else(|| {
            if !self.read_env {
                return None;
            }
            let cwd = std::env::current_dir().ok()?;
            cwd.ancestors()
                .take(6)
                .map(|dir| dir.join(".env"))
                .find(|p| p.is_file())
        });
        path.and_then(|p| fs::read_to_string(p).ok())
            .map(|raw| parse_dotenv(&raw))
            .unwrap_or_default()
    }

    /// The real value and where it came from. For features inside Marquee only.
    pub fn resolve(&self, id: &str) -> Result<Option<(String, Source)>> {
        let def = find(id)?;
        if let Some(v) = self.saved().get(def.env).filter(|v| !v.is_empty()) {
            return Ok(Some((v.clone(), Source::App)));
        }
        if self.read_env {
            if let Some(v) = std::env::var(def.env).ok().filter(|v| !v.trim().is_empty()) {
                return Ok(Some((v.trim().to_string(), Source::Env)));
            }
        }
        if let Some(v) = self.dotenv_values().get(def.env).filter(|v| !v.is_empty()) {
            return Ok(Some((v.clone(), Source::DotEnv)));
        }
        Ok(None)
    }

    pub fn get(&self, id: &str) -> Result<Option<String>> {
        Ok(self.resolve(id)?.map(|(v, _)| v))
    }

    pub fn list(&self) -> Result<Vec<KeyInfo>> {
        KEYS.iter()
            .map(|def| {
                let found = self.resolve(def.id)?;
                let (source, preview) = match &found {
                    Some((value, source)) => (*source, Some(preview(value, def.secret))),
                    None => (Source::None, None),
                };
                Ok(KeyInfo {
                    id: def.id.into(),
                    env: def.env.into(),
                    label: def.label.into(),
                    description: def.description.into(),
                    help: def.help.into(),
                    used_by: def.used_by.into(),
                    secret: def.secret,
                    source,
                    set: found.is_some(),
                    preview,
                })
            })
            .collect()
    }

    pub fn set(&self, id: &str, value: &str) -> Result<()> {
        let def = find(id)?;
        let value = value.trim();
        if value.is_empty() {
            return self.clear(id);
        }
        if value.len() > MAX_VALUE_LEN || value.chars().any(|c| c.is_control()) {
            return Err(AppError::new(
                "KEY_INVALID",
                "a key must be one line of at most 512 characters",
            ));
        }
        let _guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut all = self.saved();
        all.insert(def.env.to_string(), value.to_string());
        self.write(&all)
    }

    /// Forget the value saved in Marquee. A value from the environment or `.env` stays.
    pub fn clear(&self, id: &str) -> Result<()> {
        let def = find(id)?;
        let _guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut all = self.saved();
        if all.remove(def.env).is_some() {
            self.write(&all)?;
        }
        Ok(())
    }

    fn write(&self, all: &BTreeMap<String, String>) -> Result<()> {
        if let Some(dir) = self.file.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.file.with_extension("json.tmp");
        let body = serde_json::to_vec_pretty(all)?;
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp)?;
            file.write_all(&body)?;
            file.sync_all()?;
        }
        #[cfg(not(unix))]
        fs::write(&tmp, &body)?;
        fs::rename(&tmp, &self.file)?;
        Ok(())
    }
}

fn preview(value: &str, secret: bool) -> String {
    if !secret {
        return value.to_string();
    }
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 8 {
        return "••••".into();
    }
    format!(
        "••••{}",
        chars[chars.len() - 4..].iter().collect::<String>()
    )
}

/// `KEY=value` lines; `#` comments, blank lines, `export ` and matching quotes are handled.
pub fn parse_dotenv(raw: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        let value = if value.len() >= 2
            && ((value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\'')))
        {
            &value[1..value.len() - 1]
        } else {
            value
        };
        out.insert(key.trim().to_string(), value.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(dotenv: Option<&str>) -> (tempfile::TempDir, KeyStore) {
        let dir = tempfile::tempdir().unwrap();
        let env_path = dotenv.map(|body| {
            let p = dir.path().join(".env");
            fs::write(&p, body).unwrap();
            p
        });
        let store = KeyStore::isolated(&dir.path().join("config"), env_path);
        (dir, store)
    }

    #[test]
    fn registry_is_consistent() {
        let mut ids: Vec<_> = KEYS.iter().map(|k| k.id).collect();
        let mut envs: Vec<_> = KEYS.iter().map(|k| k.env).collect();
        ids.sort();
        envs.sort();
        ids.dedup();
        envs.dedup();
        assert_eq!(ids.len(), KEYS.len());
        assert_eq!(envs.len(), KEYS.len());
        // The name already used in the repo's .env must keep working.
        assert!(KEYS.iter().any(|k| k.env == "STEAM_GRID_DB_API_KEY"));
    }

    #[test]
    fn dotenv_parsing_handles_comments_quotes_and_export() {
        let parsed =
            parse_dotenv("# c\n\nA=1\nexport B=\"two\"\nC='three'\nnot a pair\nD = spaced \n");
        assert_eq!(parsed["A"], "1");
        assert_eq!(parsed["B"], "two");
        assert_eq!(parsed["C"], "three");
        assert_eq!(parsed["D"], "spaced");
        assert!(!parsed.contains_key("not a pair"));
    }

    #[test]
    fn saved_value_wins_over_dotenv_and_clearing_falls_back() {
        let (_dir, store) = store(Some("STEAM_GRID_DB_API_KEY=from-dotenv-1234\n"));
        let (v, source) = store.resolve("steamgriddb").unwrap().unwrap();
        assert_eq!((v.as_str(), source), ("from-dotenv-1234", Source::DotEnv));
        store.set("steamgriddb", "  saved-in-app-9999  ").unwrap();
        let (v, source) = store.resolve("steamgriddb").unwrap().unwrap();
        assert_eq!((v.as_str(), source), ("saved-in-app-9999", Source::App));
        store.clear("steamgriddb").unwrap();
        assert_eq!(
            store.resolve("steamgriddb").unwrap().unwrap().1,
            Source::DotEnv
        );
    }

    #[test]
    fn listing_never_contains_a_secret_value() {
        let (_dir, store) = store(None);
        store.set("steamgriddb", "super-secret-value-abcd").unwrap();
        store.set("steam_id", "76561198000000000").unwrap();
        let json = serde_json::to_string(&store.list().unwrap()).unwrap();
        assert!(!json.contains("super-secret-value"), "{json}");
        assert!(json.contains("••••abcd"));
        // Non-secrets are shown in full.
        assert!(json.contains("76561198000000000"));
    }

    #[test]
    fn unset_keys_report_none_and_unknown_ids_are_rejected() {
        let (_dir, store) = store(None);
        let all = store.list().unwrap();
        assert!(all
            .iter()
            .all(|k| !k.set && k.source == Source::None && k.preview.is_none()));
        assert_eq!(store.set("nope", "x").unwrap_err().code, "KEY_UNKNOWN");
        assert_eq!(store.get("nope").unwrap_err().code, "KEY_UNKNOWN");
    }

    #[test]
    fn rejects_multiline_or_oversized_values_and_empty_means_clear() {
        let (_dir, store) = store(None);
        assert_eq!(store.set("rawg", "a\nb").unwrap_err().code, "KEY_INVALID");
        assert_eq!(
            store
                .set("rawg", &"x".repeat(MAX_VALUE_LEN + 1))
                .unwrap_err()
                .code,
            "KEY_INVALID"
        );
        store.set("rawg", "valid-key-1234").unwrap();
        store.set("rawg", "   ").unwrap();
        assert_eq!(store.get("rawg").unwrap(), None);
    }

    #[cfg(unix)]
    #[test]
    fn the_key_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, store) = store(None);
        store.set("rawg", "valid-key-1234").unwrap();
        let mode = fs::metadata(dir.path().join("config/keys.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0);
    }
}
