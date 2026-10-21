use crate::db::Db;
use crate::error::{AppError, Result};
use crate::model::{BiosState, Runner, RunnerKind};
use serde_json::json;
use std::path::{Path, PathBuf};

const FAKE_RUNNER_ID: &str = "runner.fake";
/// The PCSX2 runner as registered by the catalogue.
const PCSX2_RUNNER_ID: &str = "runner.pcsx2.marquee";
/// Row id used before PCSX2 moved into the catalogue.
const LEGACY_PCSX2_ID: &str = "runner.pcsx2.unpacked";

pub fn refresh(db: &Db, allow_fake: bool) -> Result<Vec<Runner>> {
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
    // Marquee owns its emulators, all registered from the catalogue. Rows left by
    // older versions (host Flatpak/system/AppImage copies, the pre-catalogue PCSX2)
    // are kept only because past sessions reference them; make them unusable.
    for row in db.list_runners()? {
        let stale = matches!(row.source.as_str(), "flatpak" | "native" | "appimage")
            || row.id == LEGACY_PCSX2_ID;
        if stale && row.command_json != "[]" {
            db.upsert_runner(
                &row.id,
                &row.kind.clone(),
                &row.display_name,
                &[],
                BiosState::Unverified,
                Some("Replaced: Marquee manages its own emulators"),
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
        .find(|r| r.id == PCSX2_RUNNER_ID && r.available)
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

/// The catalogue emulator that runs a generic (cartridge) system, as a runner id.
pub fn emulator_runner_for(system: &str) -> Option<String> {
    crate::catalog::by_id(system)
        .ok()
        .and_then(|d| d.emulator)
        .map(|id| crate::emulators::runner_id(&id))
}

pub fn pick_for_system<'a>(runners: &'a [Runner], system: &str) -> Option<&'a Runner> {
    if let Some(id) = emulator_runner_for(system) {
        return runners.iter().find(|r| r.id == id && r.available);
    }
    let wanted = preferred_kind(system);
    runners
        .iter()
        .find(|r| {
            r.kind == wanted && (r.id == PCSX2_RUNNER_ID || r.source == "marquee") && r.available
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
    if let Some(runner_id) = emulator_runner_for(system) {
        return runners
            .iter()
            .find(|r| r.id == id && r.id == runner_id)
            .map(|r| r.id.as_str());
    }
    let wanted = preferred_kind(system);
    runners
        .iter()
        .find(|r| r.id == id && (r.kind == wanted || r.kind == RunnerKind::Fake))
        .map(|r| r.id.as_str())
}

pub fn uses_marquee_prefix(runner: &Runner) -> bool {
    runner.source == "marquee"
}

pub fn launch_argv(runner: &Runner, img: &Path, extra: &[String]) -> Result<Vec<String>> {
    if runner.command.is_empty() {
        return Err(AppError::new("RUNNER_MISSING", "runner command is empty"));
    }
    let mut argv = runner.command.clone();
    if runner.kind == RunnerKind::Fake {
        return Ok(argv);
    }
    if runner.kind == RunnerKind::Emulator {
        // The emulator's own catalogue entry says how to start a game; `{rom}`
        // is replaced by the image path as one argument (no shell involved).
        let template = emulator_for_runner(&runner.id)
            .map(|d| d.launch)
            .unwrap_or_else(|| vec!["{rom}".into()]);
        argv.extend(extra.iter().cloned());
        let rom = img.to_string_lossy();
        argv.extend(template.iter().map(|a| a.replace("{rom}", &rom)));
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

fn emulator_for_runner(runner_id: &str) -> Option<crate::emulators::EmulatorDef> {
    crate::emulators::catalogue()
        .ok()?
        .into_iter()
        .find(|d| crate::emulators::runner_id(&d.id) == runner_id)
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
