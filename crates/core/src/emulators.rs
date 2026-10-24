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
    include_str!("../../../catalog/emulators/blastem.json"),
    include_str!("../../../catalog/emulators/duckstation.json"),
    include_str!("../../../catalog/emulators/pcsx2.json"),
    include_str!("../../../catalog/emulators/flycast.json"),
    include_str!("../../../catalog/emulators/cemu.json"),
    include_str!("../../../catalog/emulators/rpcs3.json"),
    include_str!("../../../catalog/emulators/ppsspp.json"),
    include_str!("../../../catalog/emulators/xemu.json"),
    include_str!("../../../catalog/emulators/azahar.json"),
    include_str!("../../../catalog/emulators/mgba.json"),
    include_str!("../../../catalog/emulators/mesen.json"),
    include_str!("../../../catalog/emulators/rmg.json"),
    include_str!("../../../catalog/emulators/melonds.json"),
    include_str!("../../../catalog/emulators/fbneo.json"),
    include_str!("../../../catalog/emulators/shadps4.json"),
    include_str!("../../../catalog/emulators/kytyps5.json"),
];

/// A file Marquee writes into an emulator's config folder before it starts, only
/// when it is missing, so the emulator skips its first-run wizard.
#[derive(Debug, Clone, Deserialize)]
pub struct SeedFile {
    /// Relative to the emulator's `XDG_CONFIG_HOME`.
    pub path: String,
    pub contents: String,
}

/// Write the entry's seed files under `home` unless they already exist.
pub fn seed_config(def: &EmulatorDef, home: &Path) {
    for seed in &def.seed {
        let rel = Path::new(&seed.path);
        let safe = rel
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)));
        if !safe {
            continue;
        }
        let target = home.join(rel);
        if target.exists() {
            continue;
        }
        if let Some(parent) = target.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&target, &seed.contents);
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// An icon fetched at install time for emulators that ship no image file.
fn fetch_icon(paths: &Paths, def: &EmulatorDef) {
    let Some(icon) = &def.icon_download else {
        return;
    };
    let dir = paths.cache_dir.join("art");
    let target = dir.join(format!("emulator-{}.png", def.id));
    if target.is_file() {
        return;
    }
    let Ok(bytes) = http::get_bytes(&icon.url) else {
        return;
    };
    if hex_sha256(&bytes) == icon.sha256.to_ascii_lowercase() && fs::create_dir_all(&dir).is_ok() {
        let _ = fs::write(&target, bytes);
    }
}

/// A firmware or BIOS file an emulator reads, from the emulator's own source.
#[derive(Debug, Clone, Deserialize)]
pub struct FirmwareDef {
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub required: bool,
    /// Exact size in bytes, when the emulator checks one.
    #[serde(default)]
    pub size: Option<u64>,
    /// `config` or `data`: which isolated XDG folder holds the file.
    #[serde(default = "default_root")]
    pub root: String,
    /// Folder under that root.
    pub dir: String,
    #[serde(default)]
    pub note: String,
}

fn default_root() -> String {
    "config".into()
}

#[derive(Debug, Clone, Serialize)]
pub struct FirmwareStatus {
    pub name: String,
    pub required: bool,
    pub present: bool,
    pub path: Option<String>,
    pub note: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IconDownload {
    pub url: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DirectDownload {
    pub url: String,
    pub sha256: String,
    pub version: String,
    /// Folder inside the archive that holds the program, when it is wrapped in one.
    #[serde(default)]
    pub archive_root: Option<String>,
    /// AppImage member inside a verified ZIP, unpacked before publication.
    #[serde(default)]
    pub appimage: Option<String>,
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
    /// GitHub `owner/repo` whose latest release carries the download. Leave empty
    /// and give `download` instead to use a direct link.
    #[serde(default)]
    pub github: String,
    #[serde(default)]
    pub asset_suffix: String,
    /// A direct link to an archive (`.AppImage`, `.zip`, `.tar.gz`) with its pinned
    /// SHA-256. Changing the link or checksum in the catalogue file is how an
    /// emulator is updated.
    #[serde(default)]
    pub download: Option<DirectDownload>,
    #[serde(default)]
    pub asset_excludes: Vec<String>,
    /// When set, pick a GitHub release asset whose name contains this (case-insensitive).
    #[serde(default)]
    pub asset_contains: Option<String>,
    /// AppImage member inside a verified `.zip` when it is not a direct `download.appimage`.
    #[serde(default)]
    pub archive_appimage: Option<String>,
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
    /// Arguments that start a game, with `{rom}` standing for the image path.
    /// Used by systems that run through this emulator (`kind` = `emulator`).
    #[serde(default = "default_launch")]
    pub launch: Vec<String>,
    /// Arguments required to open the emulator's own UI, without a game.
    #[serde(default)]
    pub open_args: Vec<String>,
    /// Extra `NAME=value` environment for every start (for example a locale).
    #[serde(default)]
    pub env: Vec<String>,
    /// Added before the launch arguments when games open fullscreen.
    #[serde(default)]
    pub fullscreen_args: Vec<String>,
    /// Added when games open in a window (for emulators that remember fullscreen).
    #[serde(default)]
    pub windowed_args: Vec<String>,
    /// Config files written before launch when missing (skips first-run wizards).
    #[serde(default)]
    pub seed: Vec<SeedFile>,
    /// Icon to fetch at install time when the download holds no image.
    #[serde(default)]
    pub icon_download: Option<IconDownload>,
    /// Firmware or BIOS files this emulator reads, checked and filled from the inbox.
    #[serde(default)]
    pub firmware: Vec<FirmwareDef>,
}

fn default_launch() -> Vec<String> {
    vec!["{rom}".into()]
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
    let mut args = clean_args(args)?;
    if id == "mgba" {
        let mut count = 0;
        for arg in &mut args {
            if let Some(folder) = arg.strip_prefix("--marquee-games=") {
                count += 1;
                let folder = fs::canonicalize(folder)
                    .map_err(|_| AppError::new("PATH", "Select an existing game folder"))?;
                let home = std::env::var_os("HOME").and_then(|h| fs::canonicalize(h).ok());
                if !folder.is_dir()
                    || folder.parent().is_none()
                    || home.as_ref() == Some(&folder)
                    || folder.to_string_lossy().contains(':')
                {
                    return Err(AppError::new("PATH", "Select a specific game folder, not your home or filesystem root, without a colon in its path"));
                }
                *arg = format!("--marquee-games={}", folder.display());
            }
        }
        if count > 1 {
            return Err(AppError::new("BAD_REQUEST", "Select one mGBA game folder"));
        }
    }
    let cfg = Config { args };
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
    /// Firmware files this emulator reads and whether each is in place.
    pub firmware: Vec<FirmwareStatus>,
    pub launch_presentation: String,
    pub source_build: bool,
    pub source_receipt: Option<serde_json::Value>,
    pub can_rollback: bool,
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
        launch_presentation: match def.id.as_str() {
            "melonds" => "Library Play boots a DS cartridge directly. Mouse controls the touch screen. Source builds keep saves and states in Marquee's isolated data folders; DSi is not configured.",
            "blastem" => "Library Play boots a Genesis cartridge directly. No BIOS is needed. Arrows move; A/S/D are A/B/C; Enter is Start; Esc opens the menu. Config, saves and states stay in Marquee’s isolated folders.",
            "fbneo" => "Library Play passes a zipped Neo Geo ROM set to the SDL2 frontend. A matching BIOS and any parent sets must be beside the game. F12 exits; Tab opens in-game controls. A display is required.",
            "duckstation" => "Library Play hides the main window with -nogui and shows the game fullscreen. A display is still required.",
            "mgba" => "Open uses Qt. To launch only the game window, add --marquee-sdl, -f, then an absolute game path as separate arguments. GB/GBC library Play uses Qt with the selected game. SDL is an optional game-only frontend; neither is displayless emulation.",
            "rpcs3" => "Upstream supports --no-gui --fullscreen with a boot target, and a separate --headless mode. Marquee currently opens the tool; it has no PS3 game adapter.",
            "shadps4" => "Experimental. Marquee installs the official Linux SDL build and opens Big Picture mode. No PS4 library adapter or automated firmware flow yet.",
            "kytyps5" => "Experimental research runner. Marquee unpacks the published Linux tarball and opens kyty_launcher. Compatibility and config paths are upstream-defined.",
            _ => "Marquee currently opens this emulator’s own interface. Direct game launch and displayless execution need a separate verified adapter; no generic headless flag is applied.",
        }.into(),
        source_build: crate::source_builds::supported(&def.id),
        source_receipt: fs::read(program_dir(paths, &def.id).join("source-build.json"))
            .ok()
            .and_then(|raw| serde_json::from_slice(&raw).ok()),
        can_rollback: program_dir(paths, &def.id)
            .with_file_name("previous")
            .join("installed.json")
            .is_file(),
        id: def.id.clone(),
        name: def.name.clone(),
        systems: def.systems.clone(),
        integrated: def.kind.is_some(),
        firmware: firmware_status(paths, def),
        upstream: def
            .download
            .as_ref()
            .map_or_else(|| def.github.clone(), |d| d.url.clone()),
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
    let contains = def.asset_contains.as_deref().map(str::to_ascii_lowercase);
    let asset = release
        .get("assets")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .find(|a| {
            a.get("name").and_then(|n| n.as_str()).is_some_and(|n| {
                let lower = n.to_ascii_lowercase();
                let matches = if let Some(needle) = &contains {
                    lower.contains(needle)
                } else {
                    lower.ends_with(&suffix)
                };
                matches && !def.asset_excludes.iter().any(|x| lower.contains(x))
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
    if let Some(d) = &def.download {
        let sha256 = d.sha256.to_ascii_lowercase();
        if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(AppError::new(
                "NETWORK",
                format!(
                    "{} has no valid pinned SHA-256 in its catalogue file",
                    def.name
                ),
            ));
        }
        let name = d.url.rsplit('/').next().unwrap_or("emulator").to_string();
        return Ok(Release {
            version: d.version.clone(),
            name,
            url: d.url.clone(),
            sha256,
        });
    }
    let json = http::get_json(&format!(
        "https://api.github.com/repos/{}/releases/latest",
        def.github
    ))?;
    pick(def, &json)
}

/// Ask GitHub which version is newest, without downloading anything.
pub fn latest_version(def: &EmulatorDef) -> Result<String> {
    if crate::source_builds::supported(&def.id) {
        return crate::source_builds::version(&def.id);
    }
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
    if crate::source_builds::supported(&def.id) {
        return Err(AppError::new(
            "SOURCE_BUILD_REQUIRED",
            "Use Build from source for this emulator",
        ));
    }
    say(sink, &def.id, format!("Looking up {}…", def.name));
    let release = latest(def)?;
    let prefix = paths.data_dir.join("runners").join(&def.id);
    fs::create_dir_all(&prefix)?;
    let work = tempfile::Builder::new()
        .prefix(".install-")
        .tempdir_in(&prefix)?;
    let part = work.path().join(format!(
        "download-{}",
        release.name.replace(['/', '\\'], "_")
    ));
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
            "download did not match its published or pinned checksum",
        ));
    }
    say(sink, &def.id, "Unpacking…");
    let lower = release.name.to_ascii_lowercase();
    let root = if lower.ends_with(".zip") || lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
        let out = work.path().join("extracted");
        fs::create_dir_all(&out)?;
        if lower.ends_with(".zip") {
            crate::ports::extract_zip(&part, &out)?;
        } else {
            crate::ports::extract_tar_gz(&part, &out)?;
        }
        let root = match def
            .download
            .as_ref()
            .and_then(|d| d.archive_root.as_deref())
        {
            Some(sub) if !sub.contains("..") && !sub.starts_with('/') => out.join(sub),
            _ => out,
        };
        let appimage_member = def
            .download
            .as_ref()
            .and_then(|d| d.appimage.as_deref())
            .or(def.archive_appimage.as_deref());
        let root = if let Some(member) = appimage_member {
            let path = Path::new(member);
            if path.is_absolute()
                || path
                    .components()
                    .any(|c| !matches!(c, std::path::Component::Normal(_)))
            {
                return Err(AppError::new("NETWORK", "Invalid AppImage member path"));
            }
            unpack_appimage(&root.join(path), work.path(), &def.binary)?
        } else {
            root
        };
        if !root.join(&def.binary).is_file() {
            return Err(AppError::new(
                "SPAWN_FAILED",
                format!(
                    "the archive has no {} (check `binary`/`archive_root`)",
                    def.binary
                ),
            ));
        }
        root
    } else {
        unpack_appimage(&part, work.path(), &def.binary)?
    };
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
    fetch_icon(paths, def);
    Ok(Installed {
        version: release.version,
        bin: def.binary.clone(),
    })
}

fn unpack_appimage(part: &Path, work: &Path, binary: &str) -> Result<PathBuf> {
    set_executable(part)?;
    let output = Command::new(part)
        .arg("--appimage-extract")
        .current_dir(work)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| AppError::new("SPAWN_FAILED", format!("could not unpack: {e}")))?;
    // Some AppImages extract to a symlink (`squashfs-root -> ./AppDir`). Use the real
    // folder, and only if it stays inside the staging area.
    let root = fs::canonicalize(work.join("squashfs-root"))
        .ok()
        .filter(|real| real.starts_with(fs::canonicalize(work).unwrap_or_default()))
        .unwrap_or_else(|| work.join("squashfs-root"));
    if !output.status.success() || !root.join(binary).is_file() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::new(
            "SPAWN_FAILED",
            format!(
                "unpacking failed: {}",
                stderr.lines().rev().take(4).collect::<Vec<_>>().join(" | ")
            ),
        ));
    }
    Ok(root)
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
fn launch_command(
    paths: &Paths,
    id: &str,
    env: &str,
    extra_env: &[String],
    bin: &Path,
    args: &[String],
) -> Vec<String> {
    let root = paths.data_dir.join(id);
    let mut command = vec![
        env.to_string(),
        format!("XDG_CONFIG_HOME={}", config_home(paths, id).display()),
        format!("XDG_DATA_HOME={}", root.join("xdg-data").display()),
        format!("XDG_CACHE_HOME={}", root.join("xdg-cache").display()),
    ];
    if crate::source_builds::supported(id) {
        for name in ["XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME"] {
            let value = std::env::var(name).ok();
            command.push(format!(
                "MARQUEE_HOST_{name}_SET={}",
                u8::from(value.is_some())
            ));
            if let Some(value) = value {
                command.push(format!("MARQUEE_HOST_{name}={value}"));
            }
        }
    }
    command.extend(extra_env.iter().cloned());
    command.push(bin.to_string_lossy().into_owned());
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
    crate::source_builds::require_runtime(paths, &def.id)?;
    for dir in ["xdg-config", "xdg-data", "xdg-cache"] {
        fs::create_dir_all(paths.data_dir.join(&def.id).join(dir))?;
    }
    let env = if Path::new("/usr/bin/env").is_file() {
        "/usr/bin/env"
    } else {
        "env"
    };
    let mut open_args: Vec<_> = def
        .open_args
        .iter()
        .filter(|arg| {
            !(cfg.args.contains(arg)
                || (def.id == "shadps4"
                    && arg.as_str() == "-b"
                    && cfg
                        .args
                        .iter()
                        .any(|v| matches!(v.as_str(), "-b" | "--big-picture"))))
        })
        .cloned()
        .collect();
    open_args.extend(cfg.args.iter().cloned());
    let command = launch_command(paths, &def.id, env, &def.env, &bin, &open_args);
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
fn firmware_dir(paths: &Paths, def: &EmulatorDef, fw: &FirmwareDef) -> PathBuf {
    let root = if fw.root == "data" {
        paths.data_dir.join(&def.id).join("xdg-data")
    } else {
        config_home(paths, &def.id)
    };
    root.join(&fw.dir)
}

fn name_matches(fw: &FirmwareDef, file: &str) -> bool {
    std::iter::once(&fw.name)
        .chain(fw.aliases.iter())
        .any(|n| n.eq_ignore_ascii_case(file))
}

/// Which of an emulator's firmware files are in its folder.
pub fn firmware_status(paths: &Paths, def: &EmulatorDef) -> Vec<FirmwareStatus> {
    if def.id == "rpcs3" {
        let version = marquee_manager::ps3_firmware::installed_version(&paths.data_dir)
            .ok()
            .flatten();
        return vec![FirmwareStatus { name: "PS3 firmware".into(), required: true, present: version.is_some(),
            path: version.as_ref().map(|_| marquee_manager::ps3_firmware::flash_path(&paths.data_dir).to_string_lossy().into_owned()),
            note: version.map_or_else(|| "A stored PUP is not an installed firmware. Use RPCS3 firmware setup.".into(), |v| format!("Installed firmware files detected: {v}. Game compatibility is not yet tested.")) }];
    }

    def.firmware
        .iter()
        .map(|fw| {
            let dir = firmware_dir(paths, def, fw);
            let found = fs::read_dir(&dir).ok().and_then(|entries| {
                entries.flatten().map(|e| e.path()).find(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| name_matches(fw, n))
                        && fw
                            .size
                            .is_none_or(|s| fs::metadata(p).is_ok_and(|m| m.len() == s))
                })
            });
            FirmwareStatus {
                name: fw.name.clone(),
                required: fw.required,
                present: found.is_some(),
                path: found.map(|p| p.to_string_lossy().into_owned()),
                note: fw.note.clone(),
            }
        })
        .collect()
}

/// Copy any file under `source` (up to 3 levels deep) that an emulator's catalogue
/// entry names as firmware into that emulator's folder. Returns the names added.
/// Existing files are never overwritten.
pub fn import_firmware(paths: &Paths, source: &Path) -> Vec<String> {
    let Ok(defs) = catalogue() else {
        return Vec::new();
    };
    let mut files = Vec::new();
    collect(source, 3, &mut files);
    let mut added = Vec::new();
    for def in &defs {
        for fw in &def.firmware {
            let Some(src) = files.iter().find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| name_matches(fw, n))
                    && fw
                        .size
                        .is_none_or(|s| fs::metadata(p).is_ok_and(|m| m.len() == s))
            }) else {
                continue;
            };
            let dir = firmware_dir(paths, def, fw);
            let target = dir.join(&fw.name);
            if target.exists() || fs::create_dir_all(&dir).is_err() {
                continue;
            }
            if fs::copy(src, &target).is_ok() {
                added.push(format!("{} ({})", fw.name, def.name));
            }
        }
    }
    added
}

fn collect(root: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(t) if t.is_file() => out.push(path),
            Ok(t) if t.is_dir() && depth > 0 => collect(&path, depth - 1, out),
            _ => {}
        }
    }
}

/// An install made before this emulator moved into the catalogue (or by hand into
/// Marquee's own folder) is adopted as-is, so nothing is downloaded twice.
fn adopt_existing(db: &Db, paths: &Paths, def: &EmulatorDef) -> Result<()> {
    if installed(db, &def.id)?.is_none() && program_dir(paths, &def.id).join(&def.binary).is_file()
    {
        db.set_setting(
            &setting_key(&def.id),
            &serde_json::to_string(&Installed {
                version: "adopted".into(),
                bin: def.binary.clone(),
            })?,
        )?;
    }
    Ok(())
}

pub fn sync_runners(db: &Db, paths: &Paths) -> Result<()> {
    for def in catalogue()? {
        adopt_existing(db, paths, &def)?;
        crate::source_builds::refresh_launcher(paths, &def.id)?;
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
        let (state, hint) = if has_bios || bios_dir(paths, &def).is_none() {
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
            &launch_command(
                paths,
                &def.id,
                env,
                &def.env,
                &bin,
                &config(db, &def.id)?.args,
            ),
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
    fn firmware_is_reported_and_filled_from_a_folder_without_overwriting() {
        let t = tempfile::tempdir().unwrap();
        let paths = Paths::resolve(Some(t.path().join("data"))).unwrap();
        let def = find("flycast").unwrap();
        assert!(!firmware_status(&paths, &def)[0].present);
        let inbox = t.path().join("pack/dc");
        fs::create_dir_all(&inbox).unwrap();
        fs::write(inbox.join("DC_BIOS.BIN"), vec![0u8; 2_097_152]).unwrap();
        fs::write(inbox.join("wrong-size.bin"), b"x").unwrap();
        let added = import_firmware(&paths, &t.path().join("pack"));
        assert_eq!(added, ["dc_boot.bin (Flycast)"]);
        let st = &firmware_status(&paths, &def)[0];
        assert!(st.present && !st.required);
        assert!(st
            .path
            .as_deref()
            .unwrap()
            .ends_with("xdg-data/flycast/dc_boot.bin"));
        assert!(import_firmware(&paths, &t.path().join("pack")).is_empty());
        // A file of the wrong size under the right name is not accepted.
        let other = t.path().join("bad");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("dc_boot.bin"), b"tiny").unwrap();
        let fresh = Paths::resolve(Some(t.path().join("data2"))).unwrap();
        assert!(import_firmware(&fresh, &other).is_empty());
        let xemu = find("xemu").unwrap();
        assert_eq!(
            firmware_status(&paths, &xemu)
                .iter()
                .filter(|f| f.required)
                .count(),
            3
        );
    }

    #[test]
    fn seed_files_are_written_once_and_cannot_escape_the_config_folder() {
        let t = tempfile::tempdir().unwrap();
        let home = t.path().join("cfg");
        let mut def = find("mesen").unwrap();
        seed_config(&def, &home);
        let file = home.join("Mesen2/settings.json");
        assert!(fs::read_to_string(&file)
            .unwrap()
            .contains("\"ConfigUpgrade\":1"));
        fs::write(&file, "mine").unwrap();
        seed_config(&def, &home);
        assert_eq!(fs::read_to_string(&file).unwrap(), "mine");
        def.seed = vec![SeedFile {
            path: "../escape.json".into(),
            contents: "x".into(),
        }];
        seed_config(&def, &home);
        assert!(!t.path().join("escape.json").exists());
    }

    #[test]
    fn a_direct_link_uses_its_pinned_checksum_and_version() {
        let mut def = find("duckstation").unwrap();
        def.download = Some(DirectDownload {
            url: "https://example.invalid/dl/Emu-1.2.tar.gz".into(),
            sha256: "A".repeat(64),
            version: "1.2".into(),
            archive_root: Some("Emu-1.2".into()),
            appimage: None,
        });
        let r = latest(&def).unwrap();
        assert_eq!(r.name, "Emu-1.2.tar.gz");
        assert_eq!(r.version, "1.2");
        assert_eq!(r.sha256, "a".repeat(64));
        def.download.as_mut().unwrap().sha256 = "not-a-hash".into();
        assert_eq!(latest(&def).err().unwrap().code, "NETWORK");
    }

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
    fn github_assets_can_be_picked_by_substring() {
        let def = find("shadps4").unwrap();
        let release = json!({
            "tag_name": "v.0.20.0",
            "assets": [
                {"name": "shadps4-macos-sdl-0.20.0.zip", "browser_download_url": "u1", "digest": "sha256:aaaa"},
                {"name": "shadps4-linux-sdl-0.20.0.zip", "browser_download_url": "u2", "digest": "sha256:bbbbccccdddd"}
            ]
        });
        let picked = pick(&def, &release).unwrap();
        assert_eq!(picked.url, "u2");
        assert_eq!(picked.sha256, "bbbbccccdddd");
    }

    #[test]
    fn shadps4_open_starts_big_picture_and_preserves_user_arguments() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let paths = Paths::resolve(Some(temp.path().join("data"))).unwrap();
        let mut def = find("shadps4").unwrap();
        let dir = program_dir(&paths, &def.id);
        fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("AppRun");
        fs::write(
            &bin,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$MARQUEE_TEST_ARGS\"\n",
        )
        .unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
        let installed = Installed {
            version: "test".into(),
            bin: "AppRun".into(),
        };
        for (index, (args, expected)) in [
            (vec![], "-b\n"),
            (vec!["--show-fps".into()], "-b\n--show-fps\n"),
            (vec!["--big-picture".into()], "--big-picture\n"),
            (vec!["-b".into()], "-b\n"),
        ]
        .into_iter()
        .enumerate()
        {
            let capture = temp.path().join(format!("args-{index}"));
            def.env = vec![format!("MARQUEE_TEST_ARGS={}", capture.display())];
            open(&paths, &def, &installed, &Config { args }).unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            loop {
                if fs::read_to_string(&capture).ok().as_deref() == Some(expected) {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "missing/wrong argv: {:?}",
                    fs::read_to_string(&capture)
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }
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
            let bin = program_dir(&paths, &def.id).join(&def.binary);
            fs::create_dir_all(bin.parent().unwrap()).unwrap();
            fs::write(&bin, "x").unwrap();
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
        let mut kinds: Vec<_> = rows.iter().map(|r| r.kind.as_str()).collect();
        kinds.sort();
        assert_eq!(
            kinds,
            [
                "duckstation",
                "emulator",
                "emulator",
                "emulator",
                "emulator",
                "emulator",
                "emulator",
                "emulator",
                "pcsx2"
            ],
            "only emulators with a library integration become runners"
        );
        for def in all.iter().filter(|d| d.kind.is_none()) {
            assert!(rows.iter().all(|r| r.id != runner_id(&def.id)));
        }
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
            &[],
            Path::new("/p/AppRun"),
            &["--x".into()],
        );
        assert_eq!(cmd.last().unwrap(), "--x");
        assert_eq!(cmd[cmd.len() - 2], "/p/AppRun");
    }

    #[test]
    fn source_game_folder_is_explicit_and_invalid_changes_keep_saved_settings() {
        let t = tempfile::tempdir().unwrap();
        let folder = t.path().join("Game collection");
        fs::create_dir(&folder).unwrap();
        let db = Db::open_in_memory().unwrap();
        let arg = format!("--marquee-games={}", folder.display());
        let saved = set_config(&db, "mgba", std::slice::from_ref(&arg)).unwrap();
        assert!(set_config(&db, "mgba", &["--marquee-games=/".into()]).is_err());
        assert!(set_config(&db, "mgba", &[arg.clone(), arg]).is_err());
        assert_eq!(config(&db, "mgba").unwrap(), saved);
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
