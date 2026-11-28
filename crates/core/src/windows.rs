//! Windows games, run by Marquee itself through umu-launcher and Proton.
//!
//! A game is one small JSON file (`<data>/windows/games/<slug>.mqwin`); that
//! folder is the library root of the `windows` system. The launcher script
//! (`catalog/launchers/windows.py`) reads it. Marquee owns umu (pinned) and its
//! own copies of the Proton builds, so nothing here needs Lutris. Prefixes stay
//! wherever the game already has one, because saves live inside them.

use crate::{AppError, Core, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Game {
    pub schema: u32,
    pub name: String,
    pub exe: PathBuf,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// The Wine prefix. Saves live in it, so it is always explicit.
    pub prefix: PathBuf,
    /// A Proton build's folder name.
    pub proton: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    /// Run on the host after the game exits (EA App repairs itself this way).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_exit: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
pub struct Imported {
    pub name: String,
    pub game: PathBuf,
    pub prefix: PathBuf,
    pub proton: String,
    pub playtime_hours: f64,
}

#[derive(Debug, Default, Serialize)]
pub struct ImportReport {
    pub imported: Vec<Imported>,
    /// `name: why` for entries Marquee does not take over.
    pub skipped: Vec<String>,
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::new("WINDOWS", message.into())
}

fn slug(name: &str) -> Result<String> {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() {
        return Err(invalid("give the game a name with letters or numbers"));
    }
    Ok(out)
}

pub fn check(game: &Game) -> Result<()> {
    if game.schema != 1 || game.name.trim().is_empty() {
        return Err(invalid("not a Marquee Windows game file"));
    }
    if !game.exe.is_absolute() || !game.prefix.is_absolute() {
        return Err(invalid("the program and prefix paths must be absolute"));
    }
    let plain = !game.proton.is_empty()
        && !game.proton.starts_with('.')
        && !game.proton.contains(['/', '\\']);
    if !plain {
        return Err(invalid("the Proton build must be a folder name"));
    }
    if game
        .env
        .keys()
        .any(|k| k.is_empty() || !k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
    {
        return Err(invalid("an environment variable name is not valid"));
    }
    Ok(())
}

pub fn read(path: &Path) -> Result<Game> {
    if fs::metadata(path)?.len() > 256 * 1024 {
        return Err(invalid("game file is too large"));
    }
    let game: Game = serde_json::from_slice(&fs::read(path)?)?;
    check(&game)?;
    Ok(game)
}

/// Write a new game file; an existing one is never replaced.
fn write_new(dir: &Path, slug: &str, game: &Game) -> Result<PathBuf> {
    check(game)?;
    fs::create_dir_all(dir)?;
    let target = dir.join(format!("{slug}.mqwin"));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|e| invalid(format!("{}: {e}", target.display())))?;
    file.write_all(serde_json::to_string_pretty(game)?.as_bytes())?;
    Ok(target)
}

#[derive(Debug, Deserialize, Default)]
struct LutrisFile {
    #[serde(default)]
    game: Option<LutrisGame>,
    #[serde(default)]
    system: Option<LutrisSystem>,
    #[serde(default)]
    wine: Option<LutrisWine>,
}

#[derive(Debug, Deserialize, Default)]
struct LutrisGame {
    exe: Option<String>,
    args: Option<String>,
    prefix: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct LutrisSystem {
    env: Option<BTreeMap<String, serde_yaml_ng::Value>>,
    postexit_command: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct LutrisWine {
    version: Option<String>,
}

/// One row of Lutris's game database.
#[derive(Debug)]
struct LutrisRow {
    name: String,
    config: String,
    runner: String,
    hours: f64,
    last_played: Option<i64>,
}

/// Turn a Lutris config into a Marquee game. `default_prefix` is the one umu uses
/// when Lutris set none, which is where those games' saves already are.
fn convert(name: &str, yaml: &str, default_proton: &str, default_prefix: &Path) -> Result<Game> {
    let file: LutrisFile =
        serde_yaml_ng::from_str(yaml).map_err(|e| invalid(format!("unreadable config: {e}")))?;
    let game = file.game.unwrap_or_default();
    let system = file.system.unwrap_or_default();
    let exe = game.exe.ok_or_else(|| invalid("no program is set"))?;
    let prefix = game.prefix.map(PathBuf::from);
    let mut exe = PathBuf::from(exe);
    if exe.is_relative() {
        let base = prefix
            .as_ref()
            .ok_or_else(|| invalid("a relative program path needs a prefix"))?;
        exe = base.join(exe);
    }
    let env = system
        .env
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(key, value)| {
            let value = match value {
                serde_yaml_ng::Value::String(s) => s,
                serde_yaml_ng::Value::Number(n) => n.to_string(),
                serde_yaml_ng::Value::Bool(b) => u8::from(b).to_string(),
                _ => return None,
            };
            Some((key, value))
        })
        .collect();
    let game = Game {
        schema: 1,
        name: name.to_owned(),
        exe,
        // Arguments are plain words in these configs; quoting is not interpreted.
        args: game
            .args
            .map(|a| a.split_whitespace().map(str::to_owned).collect())
            .unwrap_or_default(),
        prefix: prefix.unwrap_or_else(|| default_prefix.to_path_buf()),
        proton: file
            .wine
            .and_then(|w| w.version)
            .unwrap_or_else(|| default_proton.to_owned()),
        env,
        post_exit: system.postexit_command.map(PathBuf::from),
    };
    check(&game)?;
    Ok(game)
}

fn lutris_rows(database: &Path) -> Result<Vec<LutrisRow>> {
    let db = rusqlite::Connection::open_with_flags(
        database,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let mut query = db.prepare(
        "SELECT name, configpath, runner, COALESCE(playtime, 0), lastplayed
         FROM games WHERE installed = 1 ORDER BY name",
    )?;
    let rows = query.query_map([], |r| {
        Ok(LutrisRow {
            name: r.get(0)?,
            config: r.get(1)?,
            runner: r.get(2)?,
            hours: r.get(3)?,
            last_played: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| invalid("HOME is not set"))
}

impl Core {
    pub fn windows_dir(&self) -> PathBuf {
        self.paths.data_dir.join("windows")
    }

    fn proton_sources(&self, home: &Path) -> Vec<PathBuf> {
        vec![
            home.join(".local/share/Steam/compatibilitytools.d"),
            home.join(".local/share/lutris/runners/wine"),
        ]
    }

    /// Give Marquee its own copy of a Proton build (a reflink where the disk
    /// supports it), so removing whatever installed it breaks nothing.
    pub fn adopt_proton(&self, name: &str, home: &Path) -> Result<PathBuf> {
        let target = self.windows_dir().join("proton").join(name);
        if target.join("proton").is_file() {
            return Ok(target);
        }
        let source = self
            .proton_sources(home)
            .into_iter()
            .map(|dir| dir.join(name))
            .find(|dir| dir.join("proton").is_file())
            .ok_or_else(|| invalid(format!("Proton build {name} was not found")))?;
        let parent = target.parent().ok_or_else(|| invalid("bad Proton path"))?;
        fs::create_dir_all(parent)?;
        let staging = tempfile::Builder::new()
            .prefix(".copy-")
            .tempdir_in(parent)?;
        let copy = staging.path().join(name);
        let status = Command::new("cp")
            .args(["-a", "--reflink=auto"])
            .arg(&source)
            .arg(&copy)
            .stdin(Stdio::null())
            .status()?;
        if !status.success() {
            return Err(invalid(format!("copying {name} failed")));
        }
        fs::rename(copy, &target)?;
        Ok(target)
    }

    /// Download Marquee's pinned umu-launcher if it is not there yet.
    pub fn setup_windows(&self) -> Result<PathBuf> {
        let target = self.windows_dir().join("umu/umu-run");
        if !target.is_file() {
            crate::purple::fetch_umu(&target)?;
        }
        // Writes the current launcher script and registers the runner.
        self.install_emulator("windows", self.events.clone())?;
        fs::create_dir_all(self.windows_dir().join("games"))?;
        Ok(target)
    }

    fn ensure_windows_root(&self) -> Result<PathBuf> {
        let games = self.windows_dir().join("games");
        fs::create_dir_all(&games)?;
        let games = fs::canonicalize(games)?;
        if !self
            .with_db(|db| db.list_roots())?
            .iter()
            .any(|r| r.path == games)
        {
            let label = crate::catalog::by_id("windows")?.display_name;
            self.add_root("windows", games.clone(), Some(label))?;
        }
        Ok(games)
    }

    /// Add a Windows program to the library. Without a prefix it gets its own.
    pub fn add_windows_game(
        &self,
        exe: &Path,
        name: &str,
        prefix: Option<PathBuf>,
        proton: &str,
    ) -> Result<PathBuf> {
        let exe = fs::canonicalize(exe)?;
        let slug = slug(name)?;
        self.setup_windows()?;
        self.adopt_proton(proton, &home()?)?;
        let games = self.ensure_windows_root()?;
        let prefix = prefix.unwrap_or_else(|| self.windows_dir().join("prefixes").join(&slug));
        let game = Game {
            schema: 1,
            name: name.to_owned(),
            exe,
            args: Vec::new(),
            prefix,
            proton: proton.to_owned(),
            env: BTreeMap::new(),
            post_exit: None,
        };
        let path = write_new(&games, &slug, &game)?;
        self.scan(None, None)?;
        Ok(path)
    }

    /// Take over the Windows games set up in Lutris: same programs, prefixes and
    /// Proton builds, with their playtime. Lutris's own files are only read.
    pub fn import_lutris(&self, dry_run: bool) -> Result<ImportReport> {
        let home = home()?;
        let lutris = home.join(".local/share/lutris");
        let default_proton = fs::read_to_string(lutris.join("runners/wine.yml"))
            .ok()
            .and_then(|raw| serde_yaml_ng::from_str::<LutrisFile>(&raw).ok())
            .and_then(|f| f.wine)
            .and_then(|w| w.version);
        let default_prefix = home.join("Games/umu/umu-default");
        let games_dir = self.windows_dir().join("games");
        let mut report = ImportReport::default();
        let mut pending = Vec::new();
        for row in lutris_rows(&lutris.join("pga.db"))? {
            if row.runner != "wine" {
                report.skipped.push(format!(
                    "{}: a {} entry; it stays with {}",
                    row.name, row.runner, row.runner
                ));
                continue;
            }
            let converted = fs::read_to_string(lutris.join(format!("games/{}.yml", row.config)))
                .map_err(AppError::from)
                .and_then(|yaml| {
                    let proton = default_proton.as_deref().unwrap_or("");
                    convert(&row.name, &yaml, proton, &default_prefix)
                })
                .and_then(|game| {
                    if !game.exe.is_file() {
                        return Err(invalid("the program file is missing"));
                    }
                    Ok(game)
                });
            match converted {
                Ok(game) => pending.push((row, game)),
                Err(error) => report
                    .skipped
                    .push(format!("{}: {}", row.name, error.message)),
            }
        }
        if !dry_run {
            self.setup_windows()?;
            self.ensure_windows_root()?;
        }
        let mut history = Vec::new();
        for (row, game) in pending {
            let slug = slug(&game.name)?;
            let path = games_dir.join(format!("{slug}.mqwin"));
            if path.exists() {
                report
                    .skipped
                    .push(format!("{}: already in Marquee", row.name));
                continue;
            }
            if !dry_run {
                self.adopt_proton(&game.proton, &home)?;
                write_new(&games_dir, &slug, &game)?;
                history.push((fs::canonicalize(&path)?, row.hours, row.last_played));
            }
            report.imported.push(Imported {
                name: game.name,
                game: path,
                prefix: game.prefix,
                proton: game.proton,
                playtime_hours: row.hours,
            });
        }
        if !dry_run {
            self.scan(None, None)?;
            self.with_db(|db| {
                for (path, hours, last) in &history {
                    if let Some(media) = db.media_by_path(path)? {
                        // Only an entry Marquee has never played takes the history.
                        db.conn.execute(
                            "UPDATE editions SET playtime_seconds = ?2,
                                 last_played_at = CASE WHEN ?3 > 0
                                     THEN strftime('%Y-%m-%dT%H:%M:%SZ', ?3, 'unixepoch')
                                     END
                             WHERE id = ?1 AND play_count = 0 AND playtime_seconds = 0",
                            rusqlite::params![
                                media.edition_id,
                                (hours * 3600.0).round() as i64,
                                last.unwrap_or(0)
                            ],
                        )?;
                    }
                }
                Ok(())
            })?;
        }
        Ok(report)
    }

    /// Run any program in a game's prefix with its Proton: an installer or patch,
    /// `winecfg`, or `winetricks <verbs>`. Waits for it to finish.
    pub fn run_in_windows_game(&self, game_file: &Path, program: &[String]) -> Result<i32> {
        read(game_file)?;
        if program.is_empty() {
            return Err(invalid("say what to run, for example winecfg"));
        }
        self.setup_windows()?;
        let launcher = crate::emulators::program_dir(&self.paths, "windows").join("marquee-run.py");
        let status = Command::new(launcher)
            .arg(fs::canonicalize(game_file)?)
            .arg("--run")
            .args(program)
            .env(
                "XDG_CONFIG_HOME",
                crate::emulators::config_home(&self.paths, "windows"),
            )
            .status()?;
        Ok(status.code().unwrap_or(-1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lutris_configs_keep_their_program_prefix_proton_and_extras() {
        let yaml = "game:\n  args: -one -two\n  exe: drive_c/App/app.exe\n  prefix: /games/app\n\
                    system:\n  env:\n    DXVK_HUD: compiler\n    A_NUMBER: 1\n  \
                    postexit_command: /games/app/after.sh\nwine:\n  version: dw-proton\n";
        let game = convert("App", yaml, "GE-Default", Path::new("/default")).unwrap();
        assert_eq!(game.exe, Path::new("/games/app/drive_c/App/app.exe"));
        assert_eq!(game.prefix, Path::new("/games/app"));
        assert_eq!(game.proton, "dw-proton");
        assert_eq!(game.args, ["-one", "-two"]);
        assert_eq!(game.env["A_NUMBER"], "1");
        assert_eq!(
            game.post_exit.as_deref(),
            Some(Path::new("/games/app/after.sh"))
        );

        // No prefix and a folded long path: the shared default, where saves are.
        let yaml = "game:\n  exe: /g/Long Folder/Some\n    Game.exe\n";
        let game = convert("Some", yaml, "GE-Default", Path::new("/default")).unwrap();
        assert_eq!(game.exe, Path::new("/g/Long Folder/Some Game.exe"));
        assert_eq!(game.prefix, Path::new("/default"));
        assert_eq!(game.proton, "GE-Default");
    }

    #[test]
    fn game_files_are_checked_and_never_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let mut game = Game {
            schema: 1,
            name: "My Game!".into(),
            exe: "/g/a.exe".into(),
            args: Vec::new(),
            prefix: "/p".into(),
            proton: "GE-Proton".into(),
            env: BTreeMap::new(),
            post_exit: None,
        };
        let slug = slug(&game.name).unwrap();
        assert_eq!(slug, "my-game");
        let path = write_new(dir.path(), &slug, &game).unwrap();
        assert_eq!(read(&path).unwrap(), game);
        assert!(write_new(dir.path(), &slug, &game).is_err());
        for bad in ["../proton", "", ".hidden"] {
            game.proton = bad.into();
            assert!(check(&game).is_err(), "{bad:?}");
        }
        game.proton = "GE-Proton".into();
        game.exe = "relative.exe".into();
        assert!(check(&game).is_err());
    }
}
