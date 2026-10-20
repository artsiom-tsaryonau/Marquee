use crate::db::Db;
use crate::error::{path_display, AppError, Result};
use crate::model::{BiosState, Runner, RunnerKind};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;

const FAKE_RUNNER_ID: &str = "runner.fake";
const PCSX2_APPIMAGE_ID: &str = "runner.pcsx2.appimage";
const PCSX2_FLATPAK_ID: &str = "runner.pcsx2.flatpak";
const FLATPAK_APP: &str = "net.pcsx2.PCSX2";

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
    if let Some(appimage) = find_pcsx2_appimage() {
        let (state, hint) = probe_appimage_bios(&appimage);
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
    if flatpak_pcsx2_installed() {
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
        .find(|r| r.kind == RunnerKind::Pcsx2 && r.available)
        .or_else(|| {
            runners
                .iter()
                .find(|r| r.kind == RunnerKind::Fake && r.available)
        })
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

fn which(bin: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let candidate = dir.join(bin);
            candidate.is_file().then_some(candidate)
        })
    })
}

fn find_pcsx2_appimage() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let dirs = [
        home.join("Applications"),
        home.join(".local/bin"),
        PathBuf::from("/opt"),
    ];
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if name.contains("pcsx2") && name.ends_with(".appimage") {
                return Some(entry.path());
            }
        }
    }
    which("pcsx2-qt").or_else(|| which("pcsx2"))
}

fn flatpak_pcsx2_installed() -> bool {
    Command::new("flatpak")
        .args(["info", FLATPAK_APP])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn probe_appimage_bios(bin: &Path) -> (BiosState, Option<String>) {
    // Default BIOS folders are hints. An empty default dir is unverified, not missing.
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let Some(home) = home else {
        return (
            BiosState::Unverified,
            Some("HOME unset; BIOS path unresolved".into()),
        );
    };
    let candidates = [
        home.join(".config/PCSX2/bios"),
        home.join(".var/app/net.pcsx2.PCSX2/config/PCSX2/bios"),
        bin.parent()
            .map(|p| p.join("bios"))
            .unwrap_or_else(|| PathBuf::from("bios")),
    ];
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

pub fn open_setup(db: &Db, runner_id: &str) -> Result<()> {
    let runners = snapshot(db)?;
    let runner = runners.iter().find(|r| r.id == runner_id).ok_or_else(|| {
        AppError::new(
            "RUNNER_MISSING",
            "PCSX2 is not installed. See docs/PS2.md.",
        )
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
    std::process::Command::new(bin)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|err| {
            AppError::new("SPAWN_FAILED", err.to_string())
                .with_context(json!({ "argv": runner.command }))
        })?;
    Ok(())
}
