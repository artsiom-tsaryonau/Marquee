//! Trusted, pinned source recipes. No user-supplied repository, shell or build flags.
//! Builds run in rootless Podman; only a validated image is promoted to `current`.
use crate::emulators::{self, Installed};
use crate::error::{AppError, Result};
use crate::model::CoreEvent;
use crate::paths::Paths;
use crate::sessions::EventSink;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const MGBA_COMMIT: &str = "26b7884bc25a5933960f3cdcd98bac1ae14d42e2";
const CONTAINERFILE: &str = include_str!("../../../catalog/source-builds/mgba/Containerfile");
const LAUNCHER: &str = include_str!("../../../catalog/source-builds/mgba/AppRun");
const MAX_LOG: u64 = 16 * 1024 * 1024;

pub const MELONDS_COMMIT: &str = "b86390e4428bf38ce4c1ce0e9ca446d6d25955e8";
pub const FBNEO_COMMIT: &str = "15272edc8824997b946f31fbe2306462b7a6624f";
pub const BLASTEM_COMMIT: &str = "ab743653ed110eeb7b1fe64d00cc5f9279c2bfec";
struct Recipe {
    commit: &'static str,
    repository: &'static str,
    version: &'static str,
    containerfile: &'static str,
    launcher: &'static str,
    probes: &'static [&'static [&'static str]],
    validation: &'static str,
}
fn recipe(id: &str) -> Result<Recipe> {
    match id {
        "mgba" => Ok(Recipe {
            commit: MGBA_COMMIT,
            repository: "https://github.com/mgba-emu/mgba",
            version: "0.10.5",
            containerfile: CONTAINERFILE,
            launcher: LAUNCHER,
            probes: &[&["mgba", "--version"], &["mgba-qt", "--version"]],
            validation: "Qt and SDL --version; gameplay not tested",
        }),
        "melonds" => Ok(Recipe {
            commit: MELONDS_COMMIT,
            repository: "https://github.com/melonDS-emu/melonDS",
            version: "1.1",
            containerfile: include_str!("../../../catalog/source-builds/melonds/Containerfile"),
            launcher: include_str!("../../../catalog/source-builds/melonds/AppRun"),
            probes: &[&["env", "QT_QPA_PLATFORM=offscreen", "melonDS", "--help"]],
            validation: "Qt offscreen --help; gameplay not tested",
        }),
        "blastem" => Ok(Recipe {
            commit: BLASTEM_COMMIT,
            repository: "https://github.com/libretro/blastem",
            version: "standalone-sdl",
            containerfile: include_str!("../../../catalog/source-builds/blastem/Containerfile"),
            launcher: include_str!("../../../catalog/source-builds/blastem/AppRun"),
            probes: &[&["blastem", "-v"], &["blastem", "-h"]],
            validation: "Version and help before SDL initialization; gameplay not tested",
        }),
        "fbneo" => Ok(Recipe {
            commit: FBNEO_COMMIT,
            repository: "https://github.com/finalburnneo/FBNeo",
            version: "sdl2",
            containerfile: include_str!("../../../catalog/source-builds/fbneo/Containerfile"),
            launcher: include_str!("../../../catalog/source-builds/fbneo/AppRun"),
            probes: &[&["fbneo"]],
            validation: "No-ROM usage exits before SDL initialization; gameplay not tested",
        }),
        _ => Err(AppError::new(
            "SOURCE_UNSUPPORTED",
            "No reviewed source recipe for this emulator",
        )),
    }
}
pub fn supported(id: &str) -> bool {
    matches!(id, "mgba" | "fbneo" | "melonds" | "blastem")
}
pub fn version(id: &str) -> Result<String> {
    let recipe = recipe(id)?;
    Ok(format!(
        "source-{}-{}",
        recipe.version,
        &recipe.commit[..12]
    ))
}

fn check_id(id: &str) -> Result<()> {
    if supported(id) {
        Ok(())
    } else {
        Err(AppError::new(
            "SOURCE_UNSUPPORTED",
            "No reviewed source recipe for this emulator",
        ))
    }
}

fn job_dir(paths: &Paths, id: &str) -> PathBuf {
    paths.data_dir.join("builds").join(id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildStatus {
    pub state: String,
    pub phase: String,
    pub message: String,
    pub commit: String,
    pub log_path: PathBuf,
    pub log_tail: String,
}

#[derive(Default)]
pub struct Manager {
    active: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl Manager {
    pub fn reserve(&self, paths: &Paths, id: &str) -> Result<Arc<AtomicBool>> {
        check_id(id)?;
        let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        if active.contains_key(id) {
            return Err(AppError::new(
                "INSTALL_BUSY",
                "A source build is already running",
            ));
        }
        fs::create_dir_all(job_dir(paths, id))?;
        File::create(job_dir(paths, id).join("build.log"))?;
        write_status(paths, id, "running", "queued", "Preparing source build")?;
        let cancel = Arc::new(AtomicBool::new(false));
        active.insert(id.into(), cancel.clone());
        Ok(cancel)
    }

    pub fn finish(&self, paths: &Paths, id: &str, result: &Result<()>) {
        let (state, message) = match result {
            Ok(()) => ("succeeded", "Installed and validated".to_string()),
            Err(e) if e.code == "BUILD_CANCELLED" => ("cancelled", e.message.clone()),
            Err(e) => ("failed", e.to_string()),
        };
        let _ = write_status(paths, id, state, "finished", &message);
        self.active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
    }

    pub fn cancel(&self, id: &str) -> Result<()> {
        check_id(id)?;
        if let Some(cancel) = self
            .active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
        {
            cancel.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    pub fn status(&self, paths: &Paths, id: &str) -> Result<Option<BuildStatus>> {
        check_id(id)?;
        let file = job_dir(paths, id).join("status.json");
        if !file.is_file() {
            return Ok(None);
        }
        // Serialize with reserve/finish so a queued build is never mistaken for a crash.
        let active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        let mut status: BuildStatus = serde_json::from_slice(&fs::read(file)?)?;
        if status.state == "running" && !active.contains_key(id) {
            status.state = "interrupted".into();
            status.message =
                "Marquee stopped before this build finished. Retry starts a clean build.".into();
        }
        status.log_tail = tail(&status.log_path)?;
        Ok(Some(status))
    }
}

fn write_status(paths: &Paths, id: &str, state: &str, phase: &str, message: &str) -> Result<()> {
    let dir = job_dir(paths, id);
    let status = BuildStatus {
        state: state.into(),
        phase: phase.into(),
        message: message.into(),
        commit: recipe(id)?.commit.into(),
        log_path: dir.join("build.log"),
        log_tail: String::new(),
    };
    let temp = dir.join("status.tmp");
    fs::write(&temp, serde_json::to_vec(&status)?)?;
    fs::rename(temp, dir.join("status.json"))?;
    Ok(())
}

fn tail(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    file.seek(SeekFrom::Start(size.saturating_sub(32 * 1024)))?;
    let mut bytes = Vec::new();
    file.take(32 * 1024).read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn cancelled(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::SeqCst) {
        Err(AppError::new(
            "BUILD_CANCELLED",
            "Build cancelled; the installed version was kept",
        ))
    } else {
        Ok(())
    }
}

/// Every subprocess has a deadline and its own process group. Cancellation kills
/// compiler/Git descendants, not just the shell that started them.
fn run(command: &mut Command, log: &Path, cancel: &AtomicBool, deadline: Duration) -> Result<()> {
    use std::os::unix::process::CommandExt;
    cancelled(cancel)?;
    let output = fs::OpenOptions::new().create(true).append(true).open(log)?;
    let mut child = command
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(output.try_clone()?)
        .stderr(output)
        .spawn()
        .map_err(|e| {
            AppError::new(
                "BUILD_DEPENDENCY",
                format!(
                    "Could not start {}: {e}",
                    command.get_program().to_string_lossy()
                ),
            )
        })?;
    let start = Instant::now();
    loop {
        let reason = if cancel.load(Ordering::SeqCst) {
            Some(AppError::new(
                "BUILD_CANCELLED",
                "Build cancelled; the installed version was kept",
            ))
        } else if start.elapsed() > deadline {
            Some(AppError::new(
                "BUILD_TIMEOUT",
                "Build stage exceeded its time limit",
            ))
        } else if fs::metadata(log)?.len() > MAX_LOG {
            Some(AppError::new(
                "BUILD_LOG_LIMIT",
                "Build stopped after producing 16 MiB of logs",
            ))
        } else {
            None
        };
        if let Some(error) = reason {
            // Give Podman/buildah a chance to remove its build container first.
            // SAFETY: child.id() is a live process group created above.
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGTERM);
            }
            for _ in 0..20 {
                if child.try_wait()?.is_some() {
                    return Err(error);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            // SAFETY: the owned process group has not been reaped or reused.
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        if let Some(status) = child.try_wait()? {
            return if status.success() {
                Ok(())
            } else {
                Err(AppError::new(
                    "BUILD_FAILED",
                    format!(
                        "{} failed ({status}); see the build log",
                        command.get_program().to_string_lossy()
                    ),
                ))
            };
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

pub struct Prepared {
    _work: tempfile::TempDir,
    stage: PathBuf,
    installed: Installed,
}

pub fn build(
    paths: &Paths,
    id: &str,
    cancel: &AtomicBool,
    sink: &Arc<dyn EventSink>,
) -> Result<Prepared> {
    check_id(id)?;
    if std::env::consts::OS != "linux" || std::env::consts::ARCH != "x86_64" {
        return Err(AppError::new(
            "BUILD_PLATFORM",
            "This pilot supports Linux x86_64 only",
        ));
    }
    let dir = job_dir(paths, id);
    let log = dir.join("build.log");
    let phase = |name: &str, message: &str| -> Result<()> {
        cancelled(cancel)?;
        write_status(paths, id, "running", name, message)?;
        sink.emit(CoreEvent::ToolInstallProgress {
            tool_id: id.into(),
            message: message.into(),
        });
        Ok(())
    };
    phase("preflight", "Checking rootless Podman and Git…")?;
    run(
        Command::new("git").arg("--version"),
        &log,
        cancel,
        Duration::from_secs(15),
    )?;
    let preflight = dir.join("podman.json");
    File::create(&preflight)?;
    run(
        Command::new("podman").args(["info", "--format", "json"]),
        &preflight,
        cancel,
        Duration::from_secs(30),
    )?;
    let info: serde_json::Value = serde_json::from_slice(&fs::read(preflight)?)?;
    if info
        .pointer("/host/security/rootless")
        .and_then(|v| v.as_bool())
        != Some(true)
    {
        return Err(AppError::new(
            "BUILD_DEPENDENCY",
            "Configure rootless Podman before building; Marquee never runs a rootful builder",
        ));
    }
    let prefix = paths.data_dir.join("runners").join(id);
    fs::create_dir_all(&prefix)?;
    let work = tempfile::Builder::new()
        .prefix(".source-")
        .tempdir_in(&prefix)?;
    let context = work.path().join("context");
    let source = context.join("source");
    fs::create_dir_all(&source)?;
    let recipe = recipe(id)?;
    phase("fetch", "Fetching emulator source at its pinned commit…")?;
    run(
        Command::new("git").args(["init", "--quiet"]).arg(&source),
        &log,
        cancel,
        Duration::from_secs(15),
    )?;
    run(
        Command::new("git").arg("-C").arg(&source).args([
            "-c",
            "core.hooksPath=/dev/null",
            "fetch",
            "--depth=1",
            recipe.repository,
            recipe.commit,
        ]),
        &log,
        cancel,
        Duration::from_secs(300),
    )?;
    run(
        Command::new("git").arg("-C").arg(&source).args([
            "-c",
            "core.hooksPath=/dev/null",
            "checkout",
            "--detach",
            recipe.commit,
        ]),
        &log,
        cancel,
        Duration::from_secs(30),
    )?;
    let head = fs::read_to_string(source.join(".git/HEAD"))?;
    if head.trim() != recipe.commit || source.join(".gitmodules").exists() {
        return Err(AppError::new(
            "BUILD_SOURCE",
            "Source identity or submodule layout differs from the reviewed recipe",
        ));
    }
    fs::remove_dir_all(source.join(".git"))?;
    fs::write(context.join("Containerfile"), recipe.containerfile)?;
    let image_file = work.path().join("image-id");
    phase(
        "compile",
        "Installing isolated dependencies and compiling the emulator…",
    )?;
    run(
        Command::new("podman")
            .args(["build", "--force-rm", "--layers", "--iidfile"])
            .arg(&image_file)
            .arg(&context),
        &log,
        cancel,
        Duration::from_secs(7200),
    )?;
    let image = fs::read_to_string(&image_file)?.trim().to_string();
    if !valid_image(&image) {
        return Err(AppError::new(
            "BUILD_OUTPUT",
            "Builder returned an invalid image identity",
        ));
    }
    phase(
        "validate",
        "Checking the compiled emulator without starting a game…",
    )?;
    for probe in recipe.probes {
        run(
            Command::new("podman")
                .args(["run", "--rm", "--network=none", "--pull=never", &image])
                .args(*probe),
            &log,
            cancel,
            Duration::from_secs(30),
        )?;
    }
    let stage = work.path().join("program");
    fs::create_dir(&stage)?;
    fs::write(stage.join("AppRun"), recipe.launcher)?;
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(stage.join("AppRun"), fs::Permissions::from_mode(0o755))?;
    fs::write(stage.join("image-id"), &image)?;
    let installed = Installed {
        version: version(id)?,
        bin: "AppRun".into(),
    };
    fs::write(
        stage.join("installed.json"),
        serde_json::to_vec(&installed)?,
    )?;
    fs::write(
        stage.join("source-build.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "recipe": format!("{id}-{}-v1", recipe.version), "repository": recipe.repository,
            "commit": recipe.commit, "image": image,
            "containerfile_sha256": format!("{:x}", Sha256::digest(recipe.containerfile.as_bytes())),
            "launcher_sha256": format!("{:x}", Sha256::digest(recipe.launcher.as_bytes())),
            "packages": format!("/opt/{id}/packages.txt inside the image"),
            "validation": recipe.validation
        }))?,
    )?;
    phase(
        "publish",
        "Promoting validated build; retaining the previous program…",
    )?;
    Ok(Prepared {
        _work: work,
        stage,
        installed,
    })
}

fn valid_image(image: &str) -> bool {
    image.strip_prefix("sha256:").is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    })
}

/// Upgrade a managed wrapper without recompiling an unchanged pinned emulator.
/// Preserve hand-edited launchers; interrupted updates can resume if the new
/// wrapper was written but its receipt was not.
pub fn refresh_launcher(paths: &Paths, id: &str) -> Result<()> {
    if !supported(id) {
        return Ok(());
    }
    let program = emulators::program_dir(paths, id);
    let receipt_path = program.join("source-build.json");
    if !receipt_path.is_file() {
        return Ok(());
    }
    let recipe = recipe(id)?;
    let mut receipt: serde_json::Value = serde_json::from_slice(&fs::read(&receipt_path)?)?;
    if receipt["commit"].as_str() != Some(recipe.commit) {
        return Ok(());
    }
    let actual = format!("{:x}", Sha256::digest(fs::read(program.join("AppRun"))?));
    let wanted = format!("{:x}", Sha256::digest(recipe.launcher.as_bytes()));
    if receipt["launcher_sha256"].as_str() == Some(&wanted) && actual == wanted {
        return Ok(());
    }
    if receipt["launcher_sha256"].as_str() != Some(&actual) && actual != wanted {
        return Ok(());
    }
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    let mut launcher = tempfile::NamedTempFile::new_in(&program)?;
    launcher.write_all(recipe.launcher.as_bytes())?;
    launcher
        .as_file()
        .set_permissions(fs::Permissions::from_mode(0o755))?;
    launcher
        .persist(program.join("AppRun"))
        .map_err(|e| AppError::from(e.error))?;
    receipt["launcher_sha256"] = wanted.into();
    let mut updated = tempfile::NamedTempFile::new_in(&program)?;
    updated.write_all(&serde_json::to_vec_pretty(&receipt)?)?;
    updated
        .persist(receipt_path)
        .map_err(|e| AppError::from(e.error))?;
    Ok(())
}

pub fn require_runtime(paths: &Paths, id: &str) -> Result<()> {
    if !supported(id) {
        return Ok(());
    }
    let program = emulators::program_dir(paths, id);
    if !program.join("source-build.json").is_file() {
        return Ok(());
    }
    let image = fs::read_to_string(program.join("image-id"))?;
    if !valid_image(image.trim()) {
        return Err(AppError::new(
            "BUILD_OUTPUT",
            "Invalid source-build image receipt; rebuild this emulator",
        ));
    }
    let exists = Command::new("podman")
        .args(["image", "exists", image.trim()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if !exists {
        return Err(AppError::new(
            "BUILD_RUNTIME_MISSING",
            "The local Podman image is unavailable. Start rootless Podman or rebuild this emulator.",
        ));
    }
    Ok(())
}

/// Keep filesystem and DB receipt consistent if persistence fails. One previous
/// program is retained; all saves live outside these program directories.
pub fn publish(
    paths: &Paths,
    id: &str,
    prepared: Prepared,
    old: Option<&Installed>,
    persist: impl FnOnce(&Installed) -> Result<()>,
) -> Result<()> {
    let current = emulators::program_dir(paths, id);
    let previous = current.with_file_name("previous");
    if previous.exists() {
        fs::remove_dir_all(&previous)?;
    }
    let had_current = current.exists();
    if had_current {
        if let Some(old) = old {
            fs::write(current.join("installed.json"), serde_json::to_vec(old)?)?;
        }
        fs::rename(&current, &previous)?;
    }
    let result = fs::rename(&prepared.stage, &current)
        .map_err(AppError::from)
        .and_then(|()| persist(&prepared.installed));
    if result.is_err() {
        if current.exists() {
            fs::remove_dir_all(&current)?;
        }
        if had_current {
            fs::rename(&previous, &current)?;
        }
    }
    result
}

pub fn rollback(
    paths: &Paths,
    id: &str,
    persist: impl FnOnce(&Installed) -> Result<()>,
) -> Result<()> {
    check_id(id)?;
    let current = emulators::program_dir(paths, id);
    let previous = current.with_file_name("previous");
    let receipt: Installed =
        serde_json::from_slice(&fs::read(previous.join("installed.json")).map_err(|_| {
            AppError::new("ROLLBACK_MISSING", "No previous source build is available")
        })?)?;
    let temp = current.with_file_name(format!(".swap-{}", uuid::Uuid::new_v4()));
    fs::rename(&current, &temp)?;
    if let Err(err) = fs::rename(&previous, &current) {
        fs::rename(temp, current)?;
        return Err(err.into());
    }
    if let Err(err) = persist(&receipt) {
        fs::rename(&current, &previous)?;
        fs::rename(temp, current)?;
        return Err(err);
    }
    fs::rename(temp, previous)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_reviewed_recipes_and_image_ids_are_accepted() {
        assert!(supported("mgba"));
        for id in ["mgba", "melonds", "fbneo", "blastem"] {
            let recipe = recipe(id).unwrap();
            assert!(supported(id));
            assert_eq!(recipe.commit.len(), 40);
            assert!(recipe
                .containerfile
                .starts_with("FROM docker.io/library/debian@sha256:"));
            assert!(!recipe.probes.is_empty());
        }
        assert!(!supported("../../evil"));
        assert!(valid_image(&format!("sha256:{}", "a".repeat(64))));
        assert!(!valid_image("debian:latest"));
        assert!(!valid_image("sha256:bad\ncommand"));
        assert_eq!(MGBA_COMMIT.len(), 40);
        assert!(CONTAINERFILE.starts_with("FROM docker.io/library/debian@sha256:"));
    }

    #[test]
    fn cancelled_process_group_stops_and_status_survives_restart() {
        let t = tempfile::tempdir().unwrap();
        let paths = Paths::resolve(Some(t.path().join("data"))).unwrap();
        let manager = Manager::default();
        let cancel = manager.reserve(&paths, "mgba").unwrap();
        assert!(manager.reserve(&paths, "mgba").is_err());
        manager.cancel("mgba").unwrap();
        let result = run(
            Command::new("sleep").arg("30"),
            &job_dir(&paths, "mgba").join("build.log"),
            &cancel,
            Duration::from_secs(1),
        );
        assert_eq!(result.unwrap_err().code, "BUILD_CANCELLED");
        assert_eq!(
            Manager::default()
                .status(&paths, "mgba")
                .unwrap()
                .unwrap()
                .state,
            "interrupted"
        );
        manager.finish(
            &paths,
            "mgba",
            &Err(AppError::new("BUILD_CANCELLED", "cancelled")),
        );
        assert_eq!(
            manager.status(&paths, "mgba").unwrap().unwrap().state,
            "cancelled"
        );
    }

    #[test]
    fn timeout_kills_a_running_process() {
        let t = tempfile::tempdir().unwrap();
        let start = Instant::now();
        let result = run(
            Command::new("sleep").arg("30"),
            &t.path().join("log"),
            &AtomicBool::new(false),
            Duration::from_millis(50),
        );
        assert_eq!(result.unwrap_err().code, "BUILD_TIMEOUT");
        assert!(start.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn cancellation_interrupts_a_running_child() {
        let t = tempfile::tempdir().unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let signal = cancel.clone();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            signal.store(true, Ordering::SeqCst);
        });
        let result = run(
            Command::new("sleep").arg("30"),
            &t.path().join("log"),
            &cancel,
            Duration::from_secs(10),
        );
        thread.join().unwrap();
        assert_eq!(result.unwrap_err().code, "BUILD_CANCELLED");
    }

    #[test]
    fn rollback_swaps_versions_and_undoes_a_database_failure() {
        let t = tempfile::tempdir().unwrap();
        let paths = Paths::resolve(Some(t.path().join("data"))).unwrap();
        let current = emulators::program_dir(&paths, "mgba");
        let previous = current.with_file_name("previous");
        for (dir, version) in [(&current, "new"), (&previous, "old")] {
            fs::create_dir_all(dir).unwrap();
            fs::write(dir.join("AppRun"), version).unwrap();
            fs::write(
                dir.join("installed.json"),
                serde_json::to_vec(&Installed {
                    version: version.into(),
                    bin: "AppRun".into(),
                })
                .unwrap(),
            )
            .unwrap();
        }
        assert!(rollback(&paths, "mgba", |_| Err(AppError::new("DB", "injected"))).is_err());
        assert_eq!(fs::read_to_string(current.join("AppRun")).unwrap(), "new");
        rollback(&paths, "mgba", |installed| {
            assert_eq!(installed.version, "old");
            Ok(())
        })
        .unwrap();
        assert_eq!(fs::read_to_string(current.join("AppRun")).unwrap(), "old");
        assert_eq!(fs::read_to_string(previous.join("AppRun")).unwrap(), "new");
    }

    #[test]
    fn managed_launcher_updates_preserve_custom_edits_and_recover_receipts() {
        let temp = tempfile::tempdir().unwrap();
        let paths = Paths::resolve(Some(temp.path().join("data"))).unwrap();
        let program = emulators::program_dir(&paths, "melonds");
        fs::create_dir_all(&program).unwrap();
        fs::write(program.join("AppRun"), "old managed wrapper").unwrap();
        let receipt = serde_json::json!({"commit": MELONDS_COMMIT,
            "launcher_sha256": format!("{:x}", Sha256::digest(b"old managed wrapper"))});
        fs::write(
            program.join("source-build.json"),
            serde_json::to_vec(&receipt).unwrap(),
        )
        .unwrap();
        refresh_launcher(&paths, "melonds").unwrap();
        assert_eq!(
            fs::read_to_string(program.join("AppRun")).unwrap(),
            recipe("melonds").unwrap().launcher
        );
        // Simulate a crash after replacing the launcher, before receipt update.
        fs::write(
            program.join("source-build.json"),
            serde_json::to_vec(&receipt).unwrap(),
        )
        .unwrap();
        refresh_launcher(&paths, "melonds").unwrap();
        let updated: serde_json::Value =
            serde_json::from_slice(&fs::read(program.join("source-build.json")).unwrap()).unwrap();
        assert_ne!(updated["launcher_sha256"], receipt["launcher_sha256"]);
        fs::write(program.join("AppRun"), "user modifications").unwrap();
        refresh_launcher(&paths, "melonds").unwrap();
        assert_eq!(
            fs::read_to_string(program.join("AppRun")).unwrap(),
            "user modifications"
        );
    }

    #[test]
    fn cartridge_and_arcade_launchers_mount_media_literally_and_preserve_settings() {
        use std::os::unix::fs::PermissionsExt;
        for (id, ext) in [("melonds", "nds"), ("fbneo", "zip"), ("blastem", "md")] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path();
            let tools = root.join("tools");
            let games = root.join("Games with spaces");
            fs::create_dir(&tools).unwrap();
            fs::create_dir(&games).unwrap();
            fs::create_dir(root.join("host-runtime")).unwrap();
            let game = games.join(format!("Game's $(touch injected).{ext}"));
            fs::write(&game, "fixture").unwrap();
            fs::write(root.join("image-id"), format!("sha256:{}", "a".repeat(64))).unwrap();
            let launcher = root.join("AppRun");
            fs::write(&launcher, recipe(id).unwrap().launcher).unwrap();
            let shim = tools.join("podman");
            fs::write(&shim, "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$ARG_LOG\"\nprintf '%s' \"${XDG_DATA_HOME-unset}\" > \"$ARG_LOG.env\"\n").unwrap();
            fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();
            let log = root.join("argv");
            let mut cmd = Command::new("/bin/bash");
            cmd.arg(&launcher)
                .env("PATH", format!("{}:/usr/bin:/bin", tools.display()))
                .env("ARG_LOG", &log)
                .env("HOME", root.join("home"))
                .env("XDG_CONFIG_HOME", root.join("config"))
                .env("XDG_DATA_HOME", root.join("data"))
                .env("XDG_CACHE_HOME", root.join("cache"))
                .env("MARQUEE_HOST_XDG_DATA_HOME_SET", "1")
                .env("MARQUEE_HOST_XDG_DATA_HOME", root.join("host-data"))
                .env("XDG_RUNTIME_DIR", root.join("host-runtime"))
                .env_remove("XAUTHORITY")
                .arg(&game);
            assert!(cmd.status().unwrap().success());
            let argv = fs::read_to_string(&log).unwrap();
            assert_eq!(
                fs::read_to_string(root.join("argv.env")).unwrap(),
                root.join("host-data").to_string_lossy()
            );
            assert!(argv.contains("--network=none"));
            let pulse = root.join("cache/runtime-v2/pulse");
            assert!(pulse.is_dir());
            assert_eq!(
                fs::metadata(&pulse).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert!(argv.contains(&format!(
                "{}:{}:rw",
                root.join("cache/runtime-v2").display(),
                root.join("host-runtime").display()
            )));
            if id != "fbneo" {
                assert!(argv.contains(&format!("{}:{}:ro", game.display(), game.display())));
                assert!(argv.ends_with(&format!("{}\n", game.display())));
            } else {
                assert!(argv.contains(&format!("{}:/roms:ro", games.display())));
                assert!(argv.ends_with("Game's $(touch injected)\n"));
            }
            let settings = if id == "melonds" {
                root.join("config/melonDS/melonDS.toml")
            } else if id == "blastem" {
                assert!(argv.contains("SDL_RENDER_DRIVER=software"));
                assert!(argv.contains("blastem\n-g\n"));
                assert!(!argv.contains("--device"));
                assert!(argv.contains(&format!(
                    "{}:/tmp/marquee-home/.config:rw",
                    root.join("config").display()
                )));
                assert!(argv.contains(&format!(
                    "{}:/tmp/marquee-home/.local/share:rw",
                    root.join("data").display()
                )));
                root.join("config/blastem/blastem.cfg")
            } else {
                root.join("data/fbneo/config/fbneo.ini")
            };
            fs::write(&settings, "user corrections").unwrap();
            assert!(cmd.status().unwrap().success());
            assert_eq!(fs::read_to_string(settings).unwrap(), "user corrections");
            assert!(!root.join("injected").exists());
            assert!(Command::new("/bin/bash")
                .arg(&launcher)
                .env("PATH", format!("{}:/usr/bin:/bin", tools.display()))
                .env("ARG_LOG", &log)
                .arg("--version")
                .status()
                .unwrap()
                .success());
            assert!(!fs::read_to_string(log).unwrap().contains("--volume"));
        }
    }

    #[test]
    fn launcher_mounts_only_explicit_games_and_forwards_arguments_without_a_shell() {
        use std::os::unix::fs::PermissionsExt;
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let program = root.join("program");
        let tools = root.join("tools");
        let home = root.join("home");
        let games = root.join("Game collection");
        for dir in [&program, &tools, &home, &games] {
            fs::create_dir(dir).unwrap();
        }
        let game = games.join("Game's $(touch injected).gba");
        fs::write(&game, "not a real ROM").unwrap();
        fs::write(
            program.join("image-id"),
            format!("sha256:{}", "a".repeat(64)),
        )
        .unwrap();
        let launcher = program.join("AppRun");
        fs::write(&launcher, LAUNCHER).unwrap();
        fs::set_permissions(&launcher, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(
            tools.join("podman"),
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$ARG_LOG\"\n",
        )
        .unwrap();
        fs::set_permissions(tools.join("podman"), fs::Permissions::from_mode(0o755)).unwrap();
        let log = root.join("argv");
        // Exercise the wrapper without immediately exec'ing a just-written
        // temporary file (which can return ETXTBSY during parallel spawn tests).
        let mut cmd = Command::new("/bin/bash");
        cmd.arg(&launcher);
        cmd.env("PATH", format!("{}:/usr/bin:/bin", tools.display()))
            .env("ARG_LOG", &log)
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_DATA_HOME", root.join("data"))
            .env("XDG_CACHE_HOME", root.join("cache"))
            .env_remove("XDG_RUNTIME_DIR")
            .env_remove("XAUTHORITY")
            .arg(format!("--marquee-games={}", games.display()))
            .args(["--marquee-sdl", "-f"])
            .arg(&game);
        assert!(cmd.status().unwrap().success());
        let args = fs::read_to_string(&log).unwrap();
        assert!(args.contains(&format!("{}:{}:ro", games.display(), games.display())));
        assert!(!args.contains(&format!("{}:{}:ro", home.display(), home.display())));
        assert!(args.ends_with(&format!("-f\n{}\n", game.display())));
        assert!(!root.join("injected").exists());
        let status = Command::new("/bin/bash")
            .arg(&launcher)
            .env("PATH", format!("{}:/usr/bin:/bin", tools.display()))
            .env("ARG_LOG", &log)
            .arg("--version")
            .status()
            .unwrap();
        assert!(status.success());
        let probe = fs::read_to_string(log).unwrap();
        assert!(!probe.contains("--volume"));
        assert!(!probe.contains("XAUTHORITY"));
        assert!(!probe.contains("--device"));
    }

    #[test]
    fn failed_database_publish_restores_previous_program_and_saves() {
        let t = tempfile::tempdir().unwrap();
        let paths = Paths::resolve(Some(t.path().join("data"))).unwrap();
        let current = emulators::program_dir(&paths, "mgba");
        fs::create_dir_all(&current).unwrap();
        fs::write(current.join("AppRun"), "old").unwrap();
        let save = paths.data_dir.join("mgba/save");
        fs::create_dir_all(save.parent().unwrap()).unwrap();
        fs::write(&save, "save").unwrap();
        let work = tempfile::tempdir_in(current.parent().unwrap()).unwrap();
        let stage = work.path().join("program");
        fs::create_dir(&stage).unwrap();
        fs::write(stage.join("AppRun"), "new").unwrap();
        let installed = Installed {
            version: "new".into(),
            bin: "AppRun".into(),
        };
        let prepared = Prepared {
            _work: work,
            stage,
            installed,
        };
        assert!(
            publish(&paths, "mgba", prepared, None, |_| Err(AppError::new(
                "DB", "injected"
            )))
            .is_err()
        );
        assert_eq!(fs::read_to_string(current.join("AppRun")).unwrap(), "old");
        assert_eq!(fs::read_to_string(save).unwrap(), "save");
    }
}
