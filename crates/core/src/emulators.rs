//! Emulators Marquee installs and owns, described by `catalog/emulators/*.json`.
//!
//! Everything lives under Marquee's data folder: the program in
//! `runners/<id>/current`, and the emulator's own settings, BIOS, memory cards and
//! save states in `<id>/xdg-config`. Updating replaces the program only. Removing
//! the program keeps the user data until that is removed on purpose.
//!
//! Package managers (Flatpak and friends) are an advanced alternative: the
//! catalogue lists the command so it can be shown, but Marquee never runs it.

use crate::db::Db;
use crate::error::{AppError, Result};
use crate::http;
use crate::model::{BiosState, CoreEvent};
use crate::paths::Paths;
use crate::sessions::EventSink;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

const CATALOGUE: &[&str] = &[
    include_str!("../../../catalog/emulators/duckstation.json"),
    include_str!("../../../catalog/emulators/flycast.json"),
    include_str!("../../../catalog/emulators/cemu.json"),
    include_str!("../../../catalog/emulators/rpcs3.json"),
    include_str!("../../../catalog/emulators/ppsspp.json"),
    include_str!("../../../catalog/emulators/xemu.json"),
    include_str!("../../../catalog/emulators/azahar.json"),
];

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PackageHint {
    pub manager: String,
    pub command: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmulatorDef {
    pub id: String,
    pub name: String,
    /// Runner kind this emulator registers as (`duckstation`). Without one the
    /// emulator can be installed and opened, but library games do not launch
    /// through it yet.
    #[serde(default)]
    pub kind: Option<String>,
    pub systems: Vec<String>,
    pub github: String,
    pub asset_suffix: String,
    #[serde(default)]
    pub asset_excludes: Vec<String>,
    /// The release keeps one tag forever, so the asset checksum identifies a version.
    #[serde(default)]
    pub rolling: bool,
    /// Launcher inside the extracted AppImage.
    pub binary: String,
    /// Folder under `XDG_CONFIG_HOME` that holds the emulator's user data.
    pub config_subdir: String,
    pub bios_subdir: Option<String>,
    pub bios_system: Option<String>,
    pub requirement: String,
    #[serde(default)]
    pub packages: Vec<PackageHint>,
}

/// What the user can change per emulator.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    /// Extra command-line arguments, one value each, added before any disc.
    #[serde(default)]
    pub args: Vec<String>,
}

pub fn config_key(id: &str) -> String {
    format!("emulator.{id}.config")
}

pub fn config(db: &Db, id: &str) -> Result<Config> {
    Ok(db
        .get_setting(&config_key(id))?
        .filter(|raw| !raw.is_empty())
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default())
}

/// Trim, drop empty values and refuse anything that could not be one plain argument.
pub fn clean_args(args: &[String]) -> Result<Vec<String>> {
    let cleaned: Vec<String> = args
        .iter()
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty())
        .collect();
    if cleaned.len() > 16
        || cleaned
            .iter()
            .any(|a| a.len() > 512 || a.contains(['\0', '\n', '\r']))
    {
        return Err(AppError::new(
            "BAD_REQUEST",
            "Use up to 16 arguments of at most 512 characters, one per field",
        ));
    }
    Ok(cleaned)
}

pub fn set_config(db: &Db, id: &str, args: &[String]) -> Result<Config> {
    let cfg = Config {
        args: clean_args(args)?,
    };
    db.set_setting(&config_key(id), &serde_json::to_string(&cfg)?)?;
    Ok(cfg)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Installed {
    pub version: String,
    pub bin: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmulatorInfo {
    pub id: String,
    pub name: String,
    pub systems: Vec<String>,
    /// True when games in the library launch through this emulator.
    pub integrated: bool,
    pub upstream: String,
    pub requirement: String,
    pub installed_version: Option<String>,
    pub latest_version: Option<String>,
    /// `not_installed`, `ready` or `update`.
    pub status: String,
    /// Where the program lives (when installed).
    pub program_dir: Option<PathBuf>,
    /// Where settings, saves and BIOS live.
    pub data_dir: PathBuf,
    pub bios_dir: Option<PathBuf>,
    /// The emulator's own icon, copied out of the install so the UI can show it.
    pub icon: Option<PathBuf>,
    /// Extra launch arguments the user configured.
    pub args: Vec<String>,
    /// Advanced: commands you could run yourself instead. Marquee never runs them.
    pub alternatives: Vec<PackageHint>,
}

pub fn catalogue() -> Result<Vec<EmulatorDef>> {
    CATALOGUE
        .iter()
        .map(|raw| Ok(serde_json::from_str(raw)?))
        .collect()
}

pub fn find(id: &str) -> Result<EmulatorDef> {
    catalogue()?
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(|| AppError::new("EMULATOR_UNKNOWN", format!("unknown emulator: {id}")))
}

pub fn setting_key(id: &str) -> String {
    format!("emulator.{id}")
}

pub fn latest_key(id: &str) -> String {
    format!("emulator.{id}.latest")
}

pub fn runner_id(id: &str) -> String {
    format!("runner.{id}.marquee")
}

pub fn program_dir(paths: &Paths, id: &str) -> PathBuf {
    paths.data_dir.join("runners").join(id).join("current")
}

/// The emulator's isolated `XDG_CONFIG_HOME`.
pub fn config_home(paths: &Paths, id: &str) -> PathBuf {
    paths.data_dir.join(id).join("xdg-config")
}

pub fn bios_dir(paths: &Paths, def: &EmulatorDef) -> Option<PathBuf> {
    def.bios_subdir
        .as_ref()
        .map(|sub| config_home(paths, &def.id).join(sub))
}

pub fn installed(db: &Db, id: &str) -> Result<Option<Installed>> {
    Ok(db
        .get_setting(&setting_key(id))?
        .filter(|raw| !raw.is_empty())
        .and_then(|raw| serde_json::from_str(&raw).ok()))
}

/// The icon shipped inside the unpacked AppImage, copied into the art cache once.
fn ensure_icon(paths: &Paths, def: &EmulatorDef) -> Option<PathBuf> {
    let dir = paths.cache_dir.join("art");
    for ext in ["png", "svg"] {
        let cached = dir.join(format!("emulator-{}.{ext}", def.id));
        if cached.is_file() {
            return Some(cached);
        }
    }
    let mut found: Vec<(u8, PathBuf)> = fs::read_dir(program_dir(paths, &def.id))
        .ok()?
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let name = e.file_name().to_string_lossy().to_ascii_lowercase();
            let ext = path.extension()?.to_str()?.to_ascii_lowercase();
            if !path.is_file() || !matches!(ext.as_str(), "png" | "svg") || name.starts_with('.') {
                return None;
            }
            let rank = if name.contains(&def.id) { 0 } else { 1 };
            Some((rank, path))
        })
        .collect();
    found.sort();
    let (_, source) = found.into_iter().next()?;
    if fs::metadata(&source).ok()?.len() > 4 * 1024 * 1024 {
        return None;
    }
    let ext = source.extension()?.to_str()?.to_ascii_lowercase();
    fs::create_dir_all(&dir).ok()?;
    let target = dir.join(format!("emulator-{}.{ext}", def.id));
    fs::copy(&source, &target).ok()?;
    Some(target)
}

pub fn info(
    paths: &Paths,
    def: &EmulatorDef,
    installed: Option<&Installed>,
    latest: Option<&str>,
    cfg: &Config,
) -> EmulatorInfo {
    let present = installed.is_some_and(|i| program_dir(paths, &def.id).join(&i.bin).is_file());
    let status = match (present, installed, latest) {
        (false, _, _) | (_, None, _) => "not_installed",
        (_, Some(i), Some(l)) if i.version != l => "update",
        _ => "ready",
    };
    EmulatorInfo {
        id: def.id.clone(),
        name: def.name.clone(),
        systems: def.systems.clone(),
        integrated: def.kind.is_some(),
        upstream: def.github.clone(),
        requirement: def.requirement.clone(),
        installed_version: installed.filter(|_| present).map(|i| i.version.clone()),
        latest_version: latest.map(str::to_string),
        status: status.into(),
        program_dir: present.then(|| program_dir(paths, &def.id)),
        data_dir: config_home(paths, &def.id),
        bios_dir: bios_dir(paths, def),
        icon: if present {
            ensure_icon(paths, def)
        } else {
            None
        },
        args: cfg.args.clone(),
        alternatives: def.packages.clone(),
    }
}

struct Release {
    version: String,
    name: String,
    url: String,
    sha256: String,
}

fn pick(def: &EmulatorDef, release: &serde_json::Value) -> Result<Release> {
    let tag = release
        .get("tag_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::new("NETWORK", "GitHub release has no tag"))?;
    let suffix = def.asset_suffix.to_ascii_lowercase();
    let asset = release
        .get("assets")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .find(|a| {
            a.get("name").and_then(|n| n.as_str()).is_some_and(|n| {
                let lower = n.to_ascii_lowercase();
                lower.ends_with(&suffix) && !def.asset_excludes.iter().any(|x| lower.contains(x))
            })
        })
        .ok_or_else(|| {
            AppError::new(
                "NETWORK",
                format!(
                    "{} {tag} has no download named *{}",
                    def.name, def.asset_suffix
                ),
            )
        })?;
    let text = |key: &str| asset.get(key).and_then(|v| v.as_str());
    let sha256 = text("digest")
        .and_then(|d| d.strip_prefix("sha256:"))
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| {
            AppError::new(
                "NETWORK",
                format!(
                    "{} publishes no checksum, so Marquee will not install it",
                    def.name
                ),
            )
        })?;
    let version = if def.rolling {
        format!("{tag}-{}", &sha256[..sha256.len().min(8)])
    } else {
        tag.to_string()
    };
    Ok(Release {
        version,
        name: text("name").unwrap_or("emulator.AppImage").to_string(),
        url: text("browser_download_url")
            .ok_or_else(|| AppError::new("NETWORK", "download has no URL"))?
            .to_string(),
        sha256,
    })
}

fn latest(def: &EmulatorDef) -> Result<Release> {
    let json = http::get_json(&format!(
        "https://api.github.com/repos/{}/releases/latest",
        def.github
    ))?;
    pick(def, &json)
}

/// Ask GitHub which version is newest, without downloading anything.
pub fn latest_version(def: &EmulatorDef) -> Result<String> {
    Ok(latest(def)?.version)
}

fn say(sink: &Arc<dyn EventSink>, id: &str, message: impl Into<String>) {
    sink.emit(CoreEvent::ToolInstallProgress {
        tool_id: id.into(),
        message: message.into(),
    });
}

/// Download, verify against the published checksum, unpack into a staging folder
/// and only then swap it in, so a failed update leaves the working install alone.
pub fn install(paths: &Paths, def: &EmulatorDef, sink: &Arc<dyn EventSink>) -> Result<Installed> {
    say(sink, &def.id, format!("Looking up {}…", def.name));
    let release = latest(def)?;
    let prefix = paths.data_dir.join("runners").join(&def.id);
    fs::create_dir_all(&prefix)?;
    let work = tempfile::Builder::new()
        .prefix(".install-")
        .tempdir_in(&prefix)?;
    let part = work.path().join("download.AppImage");
    say(sink, &def.id, format!("Downloading {}…", release.name));
    let mut last = 0u64;
    let got = http::download_file(
        &release.url,
        &part,
        http::APPIMAGE_MAX_BYTES,
        Duration::from_secs(900),
        |n| {
            if n.saturating_sub(last) >= 8 * 1024 * 1024 {
                last = n;
                say(sink, &def.id, format!("Downloaded {} MiB…", n / 1_048_576));
            }
        },
    )?;
    if got != release.sha256 {
        return Err(AppError::new(
            "NETWORK",
            "download did not match the checksum GitHub published for it",
        ));
    }
    set_executable(&part)?;
    say(sink, &def.id, "Unpacking…");
    let output = Command::new(&part)
        .arg("--appimage-extract")
        .current_dir(work.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| AppError::new("SPAWN_FAILED", format!("could not unpack: {e}")))?;
    // Some AppImages extract to a symlink (`squashfs-root -> ./AppDir`). Use the real folder,
    // and only if it stays inside the staging area.
    let root = fs::canonicalize(work.path().join("squashfs-root"))
        .ok()
        .filter(|real| real.starts_with(fs::canonicalize(work.path()).unwrap_or_default()))
        .unwrap_or_else(|| work.path().join("squashfs-root"));
    if !output.status.success() || !root.join(&def.binary).is_file() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::new(
            "SPAWN_FAILED",
            format!(
                "unpacking failed: {}",
                stderr.lines().rev().take(4).collect::<Vec<_>>().join(" | ")
            ),
        ));
    }
    let current = prefix.join("current");
    let old = prefix.join(format!(".old-{}", uuid::Uuid::new_v4()));
    let had_previous = current.exists();
    if had_previous {
        fs::rename(&current, &old)?;
    }
    if let Err(err) = fs::rename(&root, &current) {
        if had_previous {
            let _ = fs::rename(&old, &current);
        }
        return Err(err.into());
    }
    if had_previous {
        let _ = fs::remove_dir_all(&old);
    }
    fs::create_dir_all(config_home(paths, &def.id))?;
    Ok(Installed {
        version: release.version,
        bin: def.binary.clone(),
    })
}

fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(perms.mode() | 0o755);
    fs::set_permissions(path, perms)?;
    Ok(())
}

/// Delete the program. Settings, saves and BIOS stay unless `purge_data` is set.
pub fn uninstall(paths: &Paths, def: &EmulatorDef, purge_data: bool) -> Result<()> {
    let prefix = paths.data_dir.join("runners").join(&def.id);
    if prefix.exists() {
        fs::remove_dir_all(&prefix)?;
    }
    if purge_data {
        let data = paths.data_dir.join(&def.id);
        if data.exists() {
            fs::remove_dir_all(&data)?;
        }
    }
    Ok(())
}

/// `env XDG_*=… <program>`: the isolation travels with the command, so every
/// way of starting the emulator (library launch, Open button) is contained.
fn launch_command(paths: &Paths, id: &str, env: &str, bin: &Path, args: &[String]) -> Vec<String> {
    let root = paths.data_dir.join(id);
    let mut command = vec![
        env.to_string(),
        format!("XDG_CONFIG_HOME={}", config_home(paths, id).display()),
        format!("XDG_DATA_HOME={}", root.join("xdg-data").display()),
        format!("XDG_CACHE_HOME={}", root.join("xdg-cache").display()),
        bin.to_string_lossy().into_owned(),
    ];
    command.extend(args.iter().cloned());
    command
}

/// Start the emulator's own window (not a game), contained in its own folders.
pub fn open(paths: &Paths, def: &EmulatorDef, installed: &Installed, cfg: &Config) -> Result<()> {
    let bin = program_dir(paths, &def.id).join(&installed.bin);
    if !bin.is_file() {
        return Err(AppError::new(
            "RUNNER_MISSING",
            format!("{} is not installed", def.name),
        ));
    }
    for dir in ["xdg-config", "xdg-data", "xdg-cache"] {
        fs::create_dir_all(paths.data_dir.join(&def.id).join(dir))?;
    }
    let env = if Path::new("/usr/bin/env").is_file() {
        "/usr/bin/env"
    } else {
        "env"
    };
    let command = launch_command(paths, &def.id, env, &bin, &cfg.args);
    let (program, args) = command.split_first().expect("command is never empty");
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| AppError::new("SPAWN_FAILED", format!("could not open {}: {e}", def.name)))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Make each installed emulator available as a launcher, run under its own
/// isolated config home. The launch command is `env XDG_CONFIG_HOME=… <program>`
/// so the isolation travels with the runner and the launch code needs no special case.
pub fn sync_runners(db: &Db, paths: &Paths) -> Result<()> {
    for def in catalogue()? {
        let Some(kind) = def.kind.as_deref() else {
            continue;
        };
        let Some(found) = installed(db, &def.id)? else {
            continue;
        };
        let bin = program_dir(paths, &def.id).join(&found.bin);
        if !bin.is_file() {
            continue;
        }
        let env = if Path::new("/usr/bin/env").is_file() {
            "/usr/bin/env"
        } else {
            "env"
        };
        let has_bios = bios_dir(paths, &def).is_some_and(|dir| {
            fs::read_dir(dir).is_ok_and(|mut e| e.any(|f| f.is_ok_and(|f| f.path().is_file())))
        });
        let (state, hint) = if has_bios {
            (BiosState::Unverified, None)
        } else {
            (
                BiosState::Missing,
                bios_dir(paths, &def).map(|d| d.to_string_lossy().into_owned()),
            )
        };
        db.upsert_runner(
            &runner_id(&def.id),
            kind,
            &format!("{} (Marquee)", def.name),
            &launch_command(paths, &def.id, env, &bin, &config(db, &def.id)?.args),
            state,
            hint.as_deref(),
            "marquee",
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn catalogue_parses_and_rolling_releases_are_identified_by_checksum() {
        let def = find("duckstation").unwrap();
        let release = json!({
            "tag_name": "latest",
            "assets": [
                {"name": "DuckStation-x64-SSE2.AppImage", "browser_download_url": "u1", "digest": "sha256:aaaa1111bbbb"},
                {"name": "DuckStation-arm64.AppImage", "browser_download_url": "u2", "digest": "sha256:cccc"},
                {"name": "DuckStation-x64.AppImage", "browser_download_url": "u3", "digest": "sha256:C5E6F045F6A2"}
            ]
        });
        let picked = pick(&def, &release).unwrap();
        assert_eq!(picked.url, "u3");
        assert_eq!(picked.sha256, "c5e6f045f6a2");
        assert_eq!(picked.version, "latest-c5e6f045");
    }

    #[test]
    fn a_download_without_a_published_checksum_is_refused() {
        let def = find("duckstation").unwrap();
        let release = json!({
            "tag_name": "latest",
            "assets": [{"name": "DuckStation-x64.AppImage", "browser_download_url": "u"}]
        });
        assert_eq!(pick(&def, &release).err().unwrap().code, "NETWORK");
    }

    #[test]
    fn status_follows_what_is_on_disk_and_the_latest_known_version() {
        let t = tempfile::tempdir().unwrap();
        let paths = Paths::resolve(Some(t.path().join("data"))).unwrap();
        let def = find("duckstation").unwrap();
        let record = Installed {
            version: "latest-aaaa".into(),
            bin: "AppRun".into(),
        };
        // Recorded but the files are gone (deleted by hand): not installed.
        assert_eq!(
            info(&paths, &def, Some(&record), None, &Config::default()).status,
            "not_installed"
        );
        fs::create_dir_all(program_dir(&paths, "duckstation")).unwrap();
        fs::write(program_dir(&paths, "duckstation").join("AppRun"), "x").unwrap();
        assert_eq!(
            info(&paths, &def, Some(&record), None, &Config::default()).status,
            "ready"
        );
        assert_eq!(
            info(
                &paths,
                &def,
                Some(&record),
                Some("latest-bbbb"),
                &Config::default()
            )
            .status,
            "update"
        );
        assert_eq!(
            info(&paths, &def, None, None, &Config::default()).status,
            "not_installed"
        );
    }

    #[test]
    fn runner_runs_under_the_isolated_home_and_missing_bios_is_reported() {
        let t = tempfile::tempdir().unwrap();
        let paths = Paths::resolve(Some(t.path().join("data"))).unwrap();
        let db = Db::open_in_memory().unwrap();
        let def = find("duckstation").unwrap();
        fs::create_dir_all(program_dir(&paths, "duckstation")).unwrap();
        fs::write(program_dir(&paths, "duckstation").join("AppRun"), "x").unwrap();
        db.set_setting(
            &setting_key("duckstation"),
            &serde_json::to_string(&Installed {
                version: "v".into(),
                bin: "AppRun".into(),
            })
            .unwrap(),
        )
        .unwrap();
        sync_runners(&db, &paths).unwrap();
        let row = db
            .list_runners()
            .unwrap()
            .into_iter()
            .find(|r| r.id == runner_id("duckstation"))
            .unwrap();
        let command: Vec<String> = serde_json::from_str(&row.command_json).unwrap();
        assert!(command[1].starts_with("XDG_CONFIG_HOME="));
        assert!(command[1].contains("duckstation/xdg-config"));
        assert!(command[2].starts_with("XDG_DATA_HOME="));
        assert!(command[3].starts_with("XDG_CACHE_HOME="));
        assert_eq!(row.bios_state, "missing");
        let bios = bios_dir(&paths, &def).unwrap();
        assert_eq!(bios, paths.duckstation_bios_dir());
        fs::create_dir_all(&bios).unwrap();
        fs::write(bios.join("scph5501.bin"), "x").unwrap();
        sync_runners(&db, &paths).unwrap();
        let row = db
            .list_runners()
            .unwrap()
            .into_iter()
            .find(|r| r.id == runner_id("duckstation"))
            .unwrap();
        assert_eq!(row.bios_state, "unverified");
    }

    #[test]
    fn every_catalogue_entry_parses_and_only_integrated_ones_become_runners() {
        let all = catalogue().unwrap();
        assert!(all.len() >= 7);
        let ids: std::collections::HashSet<_> = all.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids.len(), all.len(), "ids are unique");
        let t = tempfile::tempdir().unwrap();
        let paths = Paths::resolve(Some(t.path().join("data"))).unwrap();
        let db = Db::open_in_memory().unwrap();
        for def in &all {
            fs::create_dir_all(program_dir(&paths, &def.id)).unwrap();
            fs::write(program_dir(&paths, &def.id).join(&def.binary), "x").unwrap();
            db.set_setting(
                &setting_key(&def.id),
                &serde_json::to_string(&Installed {
                    version: "v".into(),
                    bin: def.binary.clone(),
                })
                .unwrap(),
            )
            .unwrap();
        }
        sync_runners(&db, &paths).unwrap();
        let rows = db.list_runners().unwrap();
        assert_eq!(
            rows.len(),
            1,
            "an emulator with no library integration must not be a runner"
        );
        assert_eq!(rows[0].kind, "duckstation");
    }

    #[test]
    fn arguments_are_cleaned_saved_and_added_to_the_runner_command() {
        assert_eq!(
            clean_args(&["  -foo ".into(), "".into(), "bar".into()]).unwrap(),
            vec!["-foo", "bar"]
        );
        assert!(clean_args(&["a\nb".into()]).is_err());
        assert!(clean_args(&vec!["x".to_string(); 17]).is_err());
        let t = tempfile::tempdir().unwrap();
        let paths = Paths::resolve(Some(t.path().join("data"))).unwrap();
        let db = Db::open_in_memory().unwrap();
        set_config(&db, "duckstation", &["-nofullscreen-test".into()]).unwrap();
        assert_eq!(
            config(&db, "duckstation").unwrap().args,
            vec!["-nofullscreen-test"]
        );
        let cmd = launch_command(
            &paths,
            "duckstation",
            "env",
            Path::new("/p/AppRun"),
            &["--x".into()],
        );
        assert_eq!(cmd.last().unwrap(), "--x");
        assert_eq!(cmd[cmd.len() - 2], "/p/AppRun");
    }

    #[test]
    fn the_emulators_own_icon_is_copied_to_the_art_cache() {
        let t = tempfile::tempdir().unwrap();
        let paths = Paths::resolve(Some(t.path().join("data"))).unwrap();
        let def = find("flycast").unwrap();
        let dir = program_dir(&paths, "flycast");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("other.png"), "x").unwrap();
        fs::write(dir.join("flycast.png"), "icon").unwrap();
        let icon = ensure_icon(&paths, &def).unwrap();
        assert!(icon.ends_with("art/emulator-flycast.png"));
        assert_eq!(fs::read_to_string(icon).unwrap(), "icon");
    }

    #[test]
    fn uninstall_keeps_user_data_unless_asked() {
        let t = tempfile::tempdir().unwrap();
        let paths = Paths::resolve(Some(t.path().join("data"))).unwrap();
        let def = find("duckstation").unwrap();
        fs::create_dir_all(program_dir(&paths, "duckstation")).unwrap();
        let save = config_home(&paths, "duckstation").join("duckstation/memcards");
        fs::create_dir_all(&save).unwrap();
        uninstall(&paths, &def, false).unwrap();
        assert!(!program_dir(&paths, "duckstation").exists());
        assert!(save.exists());
        uninstall(&paths, &def, true).unwrap();
        assert!(!save.exists());
    }
}
