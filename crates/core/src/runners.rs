use crate::db::Db;
use crate::error::{path_display, AppError, Result};
use crate::model::{BiosState, Runner, RunnerKind};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;

const FAKE_RUNNER_ID: &str = "runner.fake";
const PCSX2_UNPACKED_ID: &str = "runner.pcsx2.unpacked";
const PCSX2_APPIMAGE_ID: &str = "runner.pcsx2.appimage";
const PCSX2_FLATPAK_ID: &str = "runner.pcsx2.flatpak";
pub(crate) const FLATPAK_APP: &str = "net.pcsx2.PCSX2";

pub fn refresh(
    db: &Db,
    allow_fake: bool,
    extra_appimages: &[PathBuf],
    discover_host: bool,
    marquee_bios: Option<&Path>,
) -> Result<Vec<Runner>> {
    if allow_fake {
        db.upsert_runner(
            FAKE_RUNNER_ID,
            RunnerKind::Fake.as_str(),
            "Test runner",
            &fake_command(),
            BiosState::LaunchTested,
            None,
            "test",
        )?;
    }
    if let Some(bin) = find_unpacked_pcsx2(extra_appimages) {
        let (state, hint) = probe_appimage_bios(&bin, marquee_bios);
        db.upsert_runner(
            PCSX2_UNPACKED_ID,
            RunnerKind::Pcsx2.as_str(),
            "PCSX2 (Marquee)",
            &[bin.to_string_lossy().into_owned()],
            state,
            hint.as_deref(),
            "marquee",
        )?;
    }
    if let Some(appimage) = find_pcsx2_appimage(extra_appimages, discover_host) {
        let (state, hint) = probe_appimage_bios(&appimage, marquee_bios);
        db.upsert_runner(
            PCSX2_APPIMAGE_ID,
            RunnerKind::Pcsx2.as_str(),
            "PCSX2 (AppImage)",
            &[appimage.to_string_lossy().into_owned()],
            state,
            hint.as_deref(),
            "appimage",
        )?;
    }
    if discover_host && flatpak_pcsx2_installed() {
        let (state, hint) = probe_flatpak_bios();
        db.upsert_runner(
            PCSX2_FLATPAK_ID,
            RunnerKind::Pcsx2.as_str(),
            "PCSX2 (Flatpak)",
            &["flatpak".into(), "run".into(), FLATPAK_APP.into()],
            state,
            hint.as_deref(),
            "flatpak",
        )?;
    }
    snapshot(db)
}

pub fn snapshot(db: &Db) -> Result<Vec<Runner>> {
    db.list_runners()?
        .into_iter()
        .map(|row| {
            let command: Vec<String> = serde_json::from_str(&row.command_json).unwrap_or_default();
            let kind = if row.kind == "fake" {
                RunnerKind::Fake
            } else {
                RunnerKind::Pcsx2
            };
            let available = command_available(&command);
            Ok(Runner {
                id: row.id,
                kind,
                display_name: row.display_name,
                command,
                bios_state: BiosState::parse(&row.bios_state),
                bios_hint: row.bios_hint,
                source: row.source,
                available,
            })
        })
        .collect()
}

pub fn pick_default(runners: &[Runner]) -> Option<&Runner> {
    runners
        .iter()
        .find(|r| r.id == PCSX2_UNPACKED_ID && r.available)
        .or_else(|| {
            runners
                .iter()
                .find(|r| r.kind == RunnerKind::Pcsx2 && r.available)
        })
        .or_else(|| {
            runners
                .iter()
                .find(|r| r.kind == RunnerKind::Fake && r.available)
        })
}

pub fn uses_marquee_prefix(runner: &Runner) -> bool {
    runner.source == "marquee" || runner.id == PCSX2_UNPACKED_ID
}

pub fn launch_argv(runner: &Runner, iso: &Path, extra: &[String]) -> Result<Vec<String>> {
    if runner.command.is_empty() {
        return Err(AppError::new("RUNNER_MISSING", "runner command is empty"));
    }
    let mut argv = runner.command.clone();
    if runner.kind == RunnerKind::Fake {
        return Ok(argv);
    }
    argv.extend(["-batch".into(), "-fullscreen".into()]);
    argv.extend(extra.iter().cloned());
    argv.push("--".into());
    argv.push(iso.to_string_lossy().into_owned());
    Ok(argv)
}

pub fn require_launchable(runner: &Runner) -> Result<()> {
    if !runner.available {
        return Err(AppError::new(
            "RUNNER_MISSING",
            format!("{} is not installed", runner.display_name),
        )
        .with_context(json!({ "runner_id": runner.id })));
    }
    match runner.bios_state {
        BiosState::Missing => Err(AppError::new(
            "SETUP_MISSING",
            "PCSX2 BIOS files are not present. See docs/PS2.md.",
        )
        .with_context(json!({ "runner_id": runner.id, "hint": runner.bios_hint }))),
        // Unverified must not block an explicit launch; only a confirmed-empty BIOS dir does.
        BiosState::Unverified | BiosState::LaunchTested => Ok(()),
    }
}

pub fn mark_launch_tested(db: &Db, runner_id: &str) -> Result<()> {
    db.set_bios_state(runner_id, BiosState::LaunchTested)
}

fn command_available(command: &[String]) -> bool {
    let Some(bin) = command.first() else {
        return false;
    };
    if Path::new(bin).is_file() {
        return true;
    }
    which(bin).is_some()
}

pub(crate) fn which(bin: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let candidate = dir.join(bin);
            candidate.is_file().then_some(candidate)
        })
    })
}

pub fn find_extracted_bin(root: &Path) -> Option<PathBuf> {
    for rel in ["usr/bin/pcsx2-qt", "usr/bin/pcsx2", "pcsx2-qt", "AppRun"] {
        let candidate = root.join(rel);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn find_unpacked_pcsx2(extra: &[PathBuf]) -> Option<PathBuf> {
    for extra_path in extra {
        if extra_path.is_file() {
            continue;
        }
        if let Some(bin) = find_extracted_bin(extra_path) {
            return Some(bin);
        }
        if let Some(bin) = find_extracted_bin(&extra_path.join("pcsx2").join("current")) {
            return Some(bin);
        }
        if let Some(bin) = find_extracted_bin(&extra_path.join("current")) {
            return Some(bin);
        }
    }
    None
}

fn find_pcsx2_appimage(extra: &[PathBuf], discover_host: bool) -> Option<PathBuf> {
    for extra_path in extra {
        if extra_path.is_file() {
            let name = extra_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase();
            if name.contains("pcsx2") {
                return Some(extra_path.clone());
            }
        } else if extra_path.is_dir() {
            if let Some(found) = scan_appimage_dir(extra_path) {
                return Some(found);
            }
        }
    }
    if !discover_host {
        return None;
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let dirs = [
        home.join("Applications"),
        home.join(".local/bin"),
        PathBuf::from("/opt"),
    ];
    for dir in dirs {
        if let Some(found) = scan_appimage_dir(&dir) {
            return Some(found);
        }
    }
    which("pcsx2-qt").or_else(|| which("pcsx2"))
}

fn scan_appimage_dir(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if name.contains("pcsx2") && name.ends_with(".appimage") {
            return Some(entry.path());
        }
    }
    None
}

fn flatpak_pcsx2_installed() -> bool {
    Command::new("flatpak")
        .args(["info", FLATPAK_APP])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn probe_appimage_bios(bin: &Path, marquee_bios: Option<&Path>) -> (BiosState, Option<String>) {
    // Default BIOS folders are hints. An empty default dir is unverified, not missing.
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let Some(home) = home else {
        if let Some(dir) = marquee_bios {
            if bios_dir_has_files(dir) {
                return (BiosState::Unverified, Some(path_display(dir)));
            }
        }
        return (
            BiosState::Unverified,
            Some("HOME unset; BIOS path unresolved".into()),
        );
    };
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = marquee_bios {
        candidates.push(dir.to_path_buf());
    }
    candidates.push(home.join(".config/PCSX2/bios"));
    candidates.push(home.join(".var/app/net.pcsx2.PCSX2/config/PCSX2/bios"));
    if let Some(parent) = bin.parent() {
        candidates.push(parent.join("bios"));
    }
    for dir in candidates {
        if bios_dir_has_files(&dir) {
            return (BiosState::Unverified, Some(path_display(&dir)));
        }
    }
    (
        BiosState::Unverified,
        Some("PCSX2 BIOS folder was not resolved from config; open setup to confirm".into()),
    )
}

fn probe_flatpak_bios() -> (BiosState, Option<String>) {
    let home = match std::env::var_os("HOME") {
        Some(h) => PathBuf::from(h),
        None => {
            return (
                BiosState::Unverified,
                Some("HOME unset; Flatpak BIOS path unresolved".into()),
            )
        }
    };
    let dir = home.join(".var/app/net.pcsx2.PCSX2/config/PCSX2/bios");
    if bios_dir_has_files(&dir) {
        (BiosState::Unverified, Some(path_display(&dir)))
    } else {
        (
            BiosState::Unverified,
            Some("Flatpak BIOS path not confirmed from PCSX2 config; open setup to confirm".into()),
        )
    }
}

fn bios_dir_has_files(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|e| {
        e.path()
            .extension()
            .and_then(|x| x.to_str())
            .map(|x| x.eq_ignore_ascii_case("bin") || x.eq_ignore_ascii_case("rom"))
            .unwrap_or(false)
    })
}

fn fake_command() -> Vec<String> {
    vec!["/bin/sleep".into(), "1".into()]
}

pub fn open_setup(db: &Db, runner_id: &str, xdg_config: Option<&Path>) -> Result<()> {
    let runners = snapshot(db)?;
    let runner = runners.iter().find(|r| r.id == runner_id).ok_or_else(|| {
        AppError::new("RUNNER_MISSING", "PCSX2 is not installed. See docs/PS2.md.")
    })?;
    if !runner.available || runner.command.is_empty() {
        return Err(AppError::new(
            "RUNNER_MISSING",
            format!("{} is not installed", runner.display_name),
        ));
    }
    let (bin, args) = runner
        .command
        .split_first()
        .ok_or_else(|| AppError::new("RUNNER_MISSING", "runner command is empty"))?;
    let mut cmd = std::process::Command::new(bin);
    cmd.args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if uses_marquee_prefix(runner) {
        if let Some(home) = xdg_config {
            cmd.env("XDG_CONFIG_HOME", home);
        }
    }
    cmd.spawn().map_err(|err| {
        AppError::new("SPAWN_FAILED", err.to_string())
            .with_context(json!({ "argv": runner.command }))
    })?;
    Ok(())
}
