use crate::db::Db;
use crate::error::{path_display, AppError, Result};
use crate::model::{BiosState, Runner, RunnerKind};
use serde_json::json;
use std::path::{Path, PathBuf};

const FAKE_RUNNER_ID: &str = "runner.fake";
const PCSX2_UNPACKED_ID: &str = "runner.pcsx2.unpacked";

pub fn refresh(
    db: &Db,
    allow_fake: bool,
    extra_appimages: &[PathBuf],
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
    // Marquee owns its emulators. Host copies (Flatpak, system packages, loose
    // AppImages) found by older versions are made unusable, never picked up.
    for row in db.list_runners()? {
        if matches!(row.source.as_str(), "flatpak" | "native" | "appimage")
            && row.command_json != "[]"
        {
            // Past sessions reference the row, so keep it but make it unusable.
            db.upsert_runner(
                &row.id,
                &row.kind.clone(),
                &row.display_name,
                &[],
                BiosState::Unverified,
                Some("Host install ignored: Marquee manages its own emulators"),
                &row.source,
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
    if runner.kind == RunnerKind::Pcsx2 {
        // Hide the main library window while keeping the game's render window.
        argv.extend(["-batch".into(), "-fullscreen".into(), "-nogui".into()]);
        argv.extend(extra.iter().cloned());
        argv.push("--".into());
    } else {
        // Batch exits on game shutdown; nogui hides the main library window.
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
