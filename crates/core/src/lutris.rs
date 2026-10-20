//! Optional read-only view of Lutris's local library. Credentials, installer
//! recipes, Wine prefixes, and database migrations remain owned by Lutris.
use crate::error::{AppError, Result};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

pub const CONFIG_KEY: &str = "source.lutris.v1";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub enabled: bool,
    pub include_owned: bool,
    pub database_path: Option<PathBuf>,
    pub launcher_id: String,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            include_owned: false,
            database_path: None,
            launcher_id: "detected.lutris".into(),
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Game {
    pub id: String,
    pub name: String,
    pub service: String,
    pub store_id: Option<String>,
    pub lutris_id: Option<i64>,
    pub runner: Option<String>,
    pub platform: String,
    pub installed: bool,
    pub available: bool,
    pub playtime_minutes: f64,
}
#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub config: Config,
    pub detected_paths: Vec<PathBuf>,
    pub database_path: Option<PathBuf>,
    pub games: Vec<Game>,
    pub cached_by_service: BTreeMap<String, usize>,
    pub configured_count: usize,
    pub installed_count: usize,
    pub warnings: Vec<String>,
}
pub fn discover() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return vec![];
    };
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));
    [
        data.join("lutris/pga.db"),
        home.join(".var/app/net.lutris.Lutris/data/lutris/pga.db"),
    ]
    .into_iter()
    .filter(|p| p.is_file())
    .collect()
}
/// Reuse existing store sync and runner configuration without a setup wizard.
/// A persisted explicit disconnect always takes precedence over this default.
pub fn automatic_config(paths: &[PathBuf]) -> Config {
    let path = paths.first().cloned();
    let flatpak = path
        .as_ref()
        .is_some_and(|p| p.to_string_lossy().contains("/.var/app/net.lutris.Lutris/"));
    Config {
        enabled: path.is_some(),
        include_owned: true,
        database_path: None,
        launcher_id: if flatpak {
            "detected.lutris.flatpak"
        } else {
            "detected.lutris"
        }
        .into(),
    }
}

pub fn validate(config: &Config) -> Result<()> {
    if config.launcher_id.is_empty()
        || config.launcher_id.len() > 100
        || config
            .database_path
            .as_ref()
            .is_some_and(|p| !p.is_absolute())
    {
        return Err(AppError::new(
            "BAD_REQUEST",
            "Choose a launcher and an absolute Lutris database path",
        ));
    }
    Ok(())
}
fn columns(db: &Connection, table: &str) -> Result<HashSet<String>> {
    let mut stmt = db.prepare(&format!("PRAGMA table_info({table})"))?;
    let names = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(names)
}
fn optional(cols: &HashSet<String>, col: &str, fallback: &str) -> String {
    if cols.contains(col) {
        col.into()
    } else {
        fallback.into()
    }
}
pub fn snapshot(config: Config, detected_paths: Vec<PathBuf>) -> Result<Snapshot> {
    validate(&config)?;
    let path = config
        .database_path
        .clone()
        .or_else(|| detected_paths.first().cloned());
    let mut result = Snapshot {
        config,
        detected_paths,
        database_path: path.clone(),
        games: vec![],
        cached_by_service: BTreeMap::new(),
        configured_count: 0,
        installed_count: 0,
        warnings: vec![],
    };
    if !result.config.enabled {
        return Ok(result);
    }
    let path = path.ok_or_else(|| {
        AppError::new(
            "LUTRIS_MISSING",
            "No Lutris database found. Choose pga.db or set up Lutris first.",
        )
    })?;
    let db = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| {
        AppError::new(
            "LUTRIS_READ",
            format!("Could not read Lutris database: {e}"),
        )
    })?;
    db.busy_timeout(std::time::Duration::from_secs(2))?;
    db.execute_batch("PRAGMA query_only=ON; BEGIN DEFERRED;")?;
    let cols = columns(&db, "games")?;
    if !["id", "name", "installed"]
        .iter()
        .all(|c| cols.contains(*c))
    {
        return Err(AppError::new(
            "LUTRIS_SCHEMA",
            "Lutris games table is missing required columns",
        ));
    }
    let namespace = format!(
        "{:x}",
        Sha256::digest(crate::paths::path_bytes(&std::fs::canonicalize(&path)?))
    );
    let namespace = &namespace[..16];
    let fields: Vec<_> = [
        "service",
        "service_id",
        "runner",
        "platform",
        "directory",
        "playtime",
    ]
    .iter()
    .map(|col| optional(&cols, col, "NULL"))
    .collect();
    let query = format!(
        "SELECT id,name,installed,{} FROM games ORDER BY id LIMIT 50001",
        fields.join(",")
    );
    let mut stmt = db.prepare(&query)?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<i64>>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, Option<String>>(5)?,
            r.get::<_, Option<String>>(6)?,
            r.get::<_, Option<String>>(7)?,
            r.get::<_, Option<f64>>(8)?,
        ))
    })?;
    let mut represented = HashSet::new();
    for row in rows {
        let (id, name, installed, service, store_id, runner, platform, directory, playtime) = row?;
        result.configured_count += 1;
        if result.configured_count > 50_000 {
            return Err(AppError::new(
                "LUTRIS_LIMIT",
                "Lutris library exceeds 50,000 configured games",
            ));
        }
        if id <= 0 {
            result
                .warnings
                .push("Skipped an entry with an invalid Lutris ID".into());
            continue;
        }
        let Some(name) = name.filter(|n| !n.trim().is_empty()) else {
            continue;
        };
        let installed = installed == Some(1);
        if installed {
            result.installed_count += 1;
        }
        let service = service
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "local".into());
        let store_id = store_id.filter(|s| !s.is_empty());
        if let Some(ref appid) = store_id {
            represented.insert((service.clone(), appid.clone()));
        }
        let available = installed
            && directory
                .as_ref()
                .filter(|p| !p.is_empty())
                .is_none_or(|p| Path::new(p).is_dir());
        let platform = match platform.as_deref() {
            Some("Linux") => "pc-linux",
            Some("Windows") => "pc-windows",
            _ => "unknown",
        };
        let minutes = playtime
            .filter(|p| p.is_finite() && *p >= 0.0)
            .unwrap_or(0.0)
            * 60.0;
        result.games.push(Game {
            id: format!("lutris:{namespace}:game:{id}"),
            name,
            service,
            store_id,
            lutris_id: Some(id),
            runner,
            platform: platform.into(),
            installed,
            available,
            playtime_minutes: minutes,
        });
    }
    let service_cols = columns(&db, "service_games")?;
    if ["service", "appid", "name"]
        .iter()
        .all(|c| service_cols.contains(*c))
    {
        let mut stmt = db.prepare(
            "SELECT service,appid,name FROM service_games ORDER BY service,appid LIMIT 100001",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?;
        let mut seen = HashSet::new();
        for (n, row) in rows.enumerate() {
            if n >= 100_000 {
                return Err(AppError::new(
                    "LUTRIS_LIMIT",
                    "Lutris service cache exceeds 100,000 entries",
                ));
            }
            let (Some(service), Some(appid), Some(name)) = row? else {
                continue;
            };
            if name.trim().is_empty()
                || service.is_empty()
                || appid.is_empty()
                || !seen.insert((service.clone(), appid.clone()))
            {
                continue;
            }
            *result.cached_by_service.entry(service.clone()).or_default() += 1;
            if !result.config.include_owned
                || represented.contains(&(service.clone(), appid.clone()))
            {
                continue;
            }
            let key = format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&(&service, &appid))?)
            );
            result.games.push(Game {
                id: format!("lutris:{namespace}:owned:{key}"),
                name,
                service,
                store_id: Some(appid),
                lutris_id: None,
                runner: None,
                platform: "unknown".into(),
                installed: false,
                available: false,
                playtime_minutes: 0.0,
            });
        }
    } else {
        result
            .warnings
            .push("Store library cache is unavailable; showing configured games only".into());
    }
    if !result.config.include_owned {
        result.games.retain(|g| g.installed);
    }
    result.games.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.id.cmp(&b.id))
    });
    Ok(result)
}
fn encode_segment(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                (*b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
pub fn uri(game: &Game) -> Result<String> {
    if game.installed {
        if !game.available {
            return Err(AppError::new(
                "MEDIA_MISSING",
                "Lutris installation directory is unavailable",
            ));
        }
        let id = game
            .lutris_id
            .filter(|id| *id > 0)
            .ok_or_else(|| AppError::new("BAD_REQUEST", "Missing Lutris game ID"))?;
        return Ok(format!("lutris:rungameid/{id}"));
    }
    match (game.service.as_str(), game.store_id.as_deref()) {
        ("gog" | "egs" | "steam" | "humble" | "amazon", Some(appid)) => Ok(format!(
            "lutris:install/{}",
            encode_segment(&format!("{}:{appid}", game.service))
        )),
        _ => Err(AppError::new(
            "LUTRIS_INSTALL",
            "Open Lutris to install this entry, then refresh Marquee",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("pga.db");
        let db = Connection::open(&path).unwrap();
        db.execute_batch("CREATE TABLE games(id INTEGER,name TEXT,installed INTEGER,service TEXT,service_id TEXT,runner TEXT,platform TEXT,directory TEXT,playtime REAL);
          CREATE TABLE service_games(service TEXT,appid TEXT,name TEXT);
          INSERT INTO games VALUES(1,'Installed GOG',1,'gog','same-id','wine','Windows',NULL,1.5);
          INSERT INTO games VALUES(2,'Missing files',1,NULL,NULL,'linux','Linux','/nonexistent/marquee-lutris-fixture',0);
          INSERT INTO games VALUES(3,'Not installed',0,'gog','other','wine','Windows',NULL,0);
          INSERT INTO service_games VALUES('gog','same-id','Already configured');
          INSERT INTO service_games VALUES('egs','same-id','Different store');
          INSERT INTO service_games VALUES('gog','new','Cached GOG');
          INSERT INTO service_games VALUES('gog','new','Duplicate cached GOG');
          INSERT INTO service_games VALUES('gog','other','Configured not installed');").unwrap();
        (t, path)
    }
    #[test]
    fn optional_source_uses_store_and_product_identity_without_modifying_database() {
        let (_t, path) = fixture();
        let before = std::fs::read(&path).unwrap();
        let s = snapshot(
            Config {
                enabled: true,
                include_owned: true,
                database_path: Some(path.clone()),
                ..Config::default()
            },
            vec![],
        )
        .unwrap();
        assert_eq!(s.configured_count, 3);
        assert_eq!(s.installed_count, 2);
        assert_eq!(s.games.len(), 5);
        assert_eq!(s.cached_by_service["gog"], 3);
        assert_eq!(s.cached_by_service["egs"], 1);
        assert_eq!(
            s.games
                .iter()
                .find(|g| g.lutris_id == Some(1))
                .unwrap()
                .playtime_minutes,
            90.0
        );
        assert!(
            !s.games
                .iter()
                .find(|g| g.lutris_id == Some(2))
                .unwrap()
                .available
        );
        assert!(s
            .games
            .iter()
            .any(|g| g.service == "egs" && g.store_id.as_deref() == Some("same-id")));
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
    #[test]
    fn installed_only_and_explicit_disconnect_are_respected() {
        let (_t, path) = fixture();
        let s = snapshot(
            Config {
                enabled: true,
                database_path: Some(path.clone()),
                ..Config::default()
            },
            vec![],
        )
        .unwrap();
        assert_eq!(s.games.len(), 2);
        let disabled = snapshot(
            Config {
                database_path: Some(path),
                ..Config::default()
            },
            vec![],
        )
        .unwrap();
        assert!(disabled.games.is_empty());
    }
    #[test]
    fn default_connection_detects_native_or_flatpak_without_manual_configuration() {
        let native = automatic_config(&[PathBuf::from("/home/test/.local/share/lutris/pga.db")]);
        assert!(native.enabled && native.include_owned);
        assert_eq!(native.launcher_id, "detected.lutris");
        let flatpak = automatic_config(&[PathBuf::from(
            "/home/test/.var/app/net.lutris.Lutris/data/lutris/pga.db",
        )]);
        assert_eq!(flatpak.launcher_id, "detected.lutris.flatpak");
        assert!(!automatic_config(&[]).enabled);
    }
    #[test]
    fn handoff_uses_numeric_launch_id_or_one_encoded_service_segment() {
        let (_t, path) = fixture();
        let s = snapshot(
            Config {
                enabled: true,
                include_owned: true,
                database_path: Some(path),
                ..Config::default()
            },
            vec![],
        )
        .unwrap();
        assert_eq!(
            uri(s.games.iter().find(|g| g.lutris_id == Some(1)).unwrap()).unwrap(),
            "lutris:rungameid/1"
        );
        assert_eq!(
            uri(s.games.iter().find(|g| g.lutris_id == Some(2)).unwrap())
                .unwrap_err()
                .code,
            "MEDIA_MISSING"
        );
        let mut owned = s.games.iter().find(|g| g.service == "egs").unwrap().clone();
        owned.store_id = Some("a/b ?#;$(x)".into());
        assert_eq!(
            uri(&owned).unwrap(),
            "lutris:install/egs%3Aa%2Fb%20%3F%23%3B%24%28x%29"
        );
    }
    #[test]
    fn missing_database_is_not_created_and_unknown_schema_is_reported() {
        let t = tempfile::tempdir().unwrap();
        let missing = t.path().join("missing.db");
        assert!(snapshot(
            Config {
                enabled: true,
                database_path: Some(missing.clone()),
                ..Config::default()
            },
            vec![]
        )
        .is_err());
        assert!(!missing.exists());
        let path = t.path().join("other.db");
        let _db = Connection::open(&path).unwrap();
        assert_eq!(
            snapshot(
                Config {
                    enabled: true,
                    database_path: Some(path),
                    ..Config::default()
                },
                vec![]
            )
            .unwrap_err()
            .code,
            "LUTRIS_SCHEMA"
        );
    }
}
