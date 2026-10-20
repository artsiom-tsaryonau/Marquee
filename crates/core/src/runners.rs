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
const DUCKSTATION_FLATPAK_ID: &str = "runner.duckstation.flatpak";
pub(crate) const DUCKSTATION_FLATPAK_APP: &str = "org.duckstation.DuckStation";

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
    if discover_host && flatpak_installed(DUCKSTATION_FLATPAK_APP) {
        let (state, hint) =
            probe_flatpak_bios_for(".var/app/org.duckstation.DuckStation/config/duckstation/bios");
        db.upsert_runner(
            DUCKSTATION_FLATPAK_ID,
            RunnerKind::Duckstation.as_str(),
            "DuckStation (Flatpak)",
            &[
                "flatpak".into(),
                "run".into(),
                DUCKSTATION_FLATPAK_APP.into(),
            ],
            state,
            hint.as_deref(),
            "flatpak",
        )?;
    }
    if discover_host {
        if let Some(bin) = which("duckstation-qt").or_else(|| which("duckstation")) {
            db.upsert_runner(
                "runner.duckstation.native",
                "duckstation",
                "DuckStation (native)",
                &[path_display(&bin)],
                BiosState::Unverified,
                Some("Open DuckStation setup to select a PS1 BIOS"),
                "native",
            )?;
        }
        if let Some(bin) = find_duckstation_appimage() {
            db.upsert_runner(
                "runner.duckstation.appimage",
                "duckstation",
                "DuckStation (AppImage)",
                &[path_display(&bin)],
                BiosState::Unverified,
                Some("Open DuckStation setup to select a PS1 BIOS"),
                "appimage",
            )?;
        }
    }
    snapshot(db)
}

pub fn snapshot(db: &Db) -> Result<Vec<Runner>> {
    db.list_runners()?
        .into_iter()
        .map(|row| {
            let command: Vec<String> = serde_json::from_str(&row.command_json).unwrap_or_default();
            let kind = RunnerKind::parse(&row.kind);
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
                .find(|r| r.kind == RunnerKind::Duckstation && r.available)
        })
        .or_else(|| {
            runners
                .iter()
                .find(|r| r.kind == RunnerKind::Fake && r.available)
        })
}

/// Preferred runner for a system: emulator kind first, then the fake test runner
/// so an explicit `--allow-fake` launch still works in tests.
pub fn preferred_kind(system: &str) -> RunnerKind {
    match system {
        crate::model::SYSTEM_PS1 => RunnerKind::Duckstation,
        _ => RunnerKind::Pcsx2,
    }
}

pub fn pick_for_system<'a>(runners: &'a [Runner], system: &str) -> Option<&'a Runner> {
    let wanted = preferred_kind(system);
    runners
        .iter()
        .find(|r| {
            r.kind == wanted && (r.id == PCSX2_UNPACKED_ID || r.source == "marquee") && r.available
        })
        .or_else(|| runners.iter().find(|r| r.kind == wanted && r.available))
        .or_else(|| {
            runners
                .iter()
                .find(|r| r.kind == RunnerKind::Fake && r.available)
        })
}

/// Honour a stored runner selection only when it belongs to this system's
/// emulator family. Otherwise a PCSX2 pin left by a PS2 session hijacks a PS1
/// launch (and vice versa), booting the wrong emulator.
///
/// The fake runner is deliberately system-agnostic: it is the explicit test
/// stand-in (`--allow-fake`) and never boots a real disc.
pub fn selection_for<'a>(
    runners: &'a [Runner],
    selected: Option<&str>,
    system: &str,
) -> Option<&'a str> {
    let id = selected?;
    let wanted = preferred_kind(system);
    runners
        .iter()
        .find(|r| r.id == id && (r.kind == wanted || r.kind == RunnerKind::Fake))
        .map(|r| r.id.as_str())
}

pub fn uses_marquee_prefix(runner: &Runner) -> bool {
    runner.source == "marquee" || runner.id == PCSX2_UNPACKED_ID
}

pub fn launch_argv(runner: &Runner, img: &Path, extra: &[String]) -> Result<Vec<String>> {
    if runner.command.is_empty() {
        return Err(AppError::new("RUNNER_MISSING", "runner command is empty"));
    }
    let mut argv = runner.command.clone();
    if runner.kind == RunnerKind::Fake {
        return Ok(argv);
    }
    if argv
        .first()
        .and_then(|p| Path::new(p).file_name())
        .is_some_and(|name| name == "flatpak")
        && argv.get(1).is_some_and(|arg| arg == "run")
    {
        // One-shot read access; never change persistent Flatpak overrides.
        // A CUE needs its contained companion tracks. Other formats need only
        // the selected image, not the whole library or the user's home.
        let is_cue = img
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("cue"));
        let grant = if is_cue {
            img.parent().unwrap_or(img)
        } else {
            img
        };
        let text = grant
            .to_str()
            .ok_or_else(|| AppError::new("PATH", "Flatpak media paths must be UTF-8"))?;
        if !grant.is_absolute() || text.contains(':') {
            return Err(AppError::new("PATH", "Flatpak cannot grant this media path. Use an absolute path without a colon, or a native emulator."));
        }
        argv.insert(2, format!("--filesystem={text}:ro"));
    }
    if runner.kind == RunnerKind::Pcsx2 {
        argv.extend(["-batch".into(), "-fullscreen".into()]);
        argv.extend(extra.iter().cloned());
        argv.push("--".into());
    } else {
        // DuckStation: `-batch` boots without the UI, `-fullscreen` fills the
        // screen, and `-nogui` keeps it from offering a console UI for disc
        // images. The image itself is a positional argument.
        argv.extend(["-batch".into(), "-fullscreen".into(), "-nogui".into()]);
        argv.extend(extra.iter().cloned());
    }
    argv.push(img.to_string_lossy().into_owned());
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
    // Unverified must not block an explicit launch; only a confirmed-empty BIOS
    // directory (Missing) does.
    if runner.kind.needs_bios() && runner.bios_state == BiosState::Missing {
        return Err(AppError::new(
            "SETUP_MISSING",
            format!("{} BIOS files are not present.", runner.display_name),
        )
        .with_context(json!({
            "runner_id": runner.id,
            "hint": runner.bios_hint,
        })));
    }
    Ok(())
}

pub fn mark_launch_tested(db: &Db, runner_id: &str) -> Result<()> {
    db.set_bios_state(runner_id, BiosState::LaunchTested)
}

fn command_available(command: &[String]) -> bool {
    let Some(bin) = command.first() else {
        return false;
    };
    if !executable(Path::new(bin)) && which(bin).is_none() {
        return false;
    }
    if Path::new(bin)
        .file_name()
        .is_some_and(|name| name == "flatpak")
        && command.get(1).is_some_and(|s| s == "run")
    {
        return command.get(2).is_some_and(|id| flatpak_installed(id));
    }
    true
}

fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

pub(crate) fn which(bin: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let candidate = dir.join(bin);
            executable(&candidate).then_some(candidate)
        })
    })
}

fn find_duckstation_appimage() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    find_duckstation_in(&[
        home.join("Applications"),
        home.join(".local/bin"),
        home.join("Downloads"),
    ])
}

fn find_duckstation_in(dirs: &[PathBuf]) -> Option<PathBuf> {
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        if let Some(path) = paths.into_iter().find(|p| {
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase();
            name.contains("duckstation") && name.ends_with(".appimage") && executable(p)
        }) {
            return Some(path);
        }
    }
    None
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
    flatpak_installed(FLATPAK_APP)
}

fn flatpak_installed(app_id: &str) -> bool {
    Command::new("flatpak")
        .args(["info", app_id])
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
    probe_flatpak_bios_for(".var/app/net.pcsx2.PCSX2/config/PCSX2/bios")
}

fn probe_flatpak_bios_for(relative: &str) -> (BiosState, Option<String>) {
    let home = match std::env::var_os("HOME") {
        Some(h) => PathBuf::from(h),
        None => {
            return (
                BiosState::Unverified,
                Some("HOME unset; Flatpak BIOS path unresolved".into()),
            )
        }
    };
    let dir = home.join(relative);
    if bios_dir_has_files(&dir) {
        (BiosState::Unverified, Some(path_display(&dir)))
    } else {
        (
            BiosState::Unverified,
            Some("Flatpak BIOS path not confirmed; open setup to confirm".into()),
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

#[cfg(test)]
mod discovery_correction_tests {
    use super::*;

    #[test]
    fn flatpak_cue_grants_just_its_folder_and_no_persistent_overrides() {
        let runner = Runner {
            id: "test.duck".into(),
            kind: RunnerKind::Duckstation,
            display_name: "DuckStation".into(),
            command: vec![
                "flatpak".into(),
                "run".into(),
                DUCKSTATION_FLATPAK_APP.into(),
            ],
            bios_state: BiosState::Unverified,
            bios_hint: None,
            source: "flatpak".into(),
            available: true,
        };
        let args = launch_argv(&runner, Path::new("/games/My disc/disc.cue"), &[]).unwrap();
        assert_eq!(args[2], "--filesystem=/games/My disc:ro");
        assert_eq!(args[3], DUCKSTATION_FLATPAK_APP);
        assert_eq!(args.last().unwrap(), "/games/My disc/disc.cue");
        assert!(!args
            .iter()
            .any(|a| a == "override" || a.contains("filesystem=home")));
        assert_eq!(
            launch_argv(&runner, Path::new("/games/colon:folder/disc.cue"), &[])
                .unwrap_err()
                .code,
            "PATH"
        );
    }
    #[test]
    fn duckstation_appimage_discovery_requires_an_executable_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("DuckStation-x64.AppImage");
        std::fs::write(&image, b"fixture").unwrap();
        std::fs::set_permissions(&image, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(find_duckstation_in(&[dir.path().into()]).is_none());
        std::fs::set_permissions(&image, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(find_duckstation_in(&[dir.path().into()]), Some(image));
    }
}
