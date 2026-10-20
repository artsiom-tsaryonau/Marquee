//! External application shortcuts. Opening a launcher does not imply importing
//! its games or tracking its game sessions.
use crate::db::Db;
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const KEY: &str = "external_launchers.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Launcher {
    pub id: String,
    pub name: String,
    pub executable: String,
    pub args: Vec<String>,
    pub working_directory: Option<String>,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LauncherInfo {
    #[serde(flatten)]
    pub launcher: Launcher,
    pub configured: bool,
    pub available: bool,
    pub status: String,
}

pub fn presets() -> Vec<Launcher> {
    let mut defaults: Vec<_> = [
        ("steam", "Steam"),
        ("lutris", "Lutris"),
        ("heroic", "Heroic"),
    ]
    .into_iter()
    .map(|(bin, name)| Launcher {
        id: format!("detected.{bin}"),
        name: name.into(),
        executable: bin.into(),
        args: vec![],
        working_directory: None,
        pinned: false,
    })
    .collect();
    defaults.push(Launcher {
        id: "detected.lutris.flatpak".into(),
        name: "Lutris (Flatpak)".into(),
        executable: "flatpak".into(),
        args: vec!["run".into(), "net.lutris.Lutris".into()],
        working_directory: None,
        pinned: false,
    });
    defaults
}
fn flatpak_lutris_installed() -> bool {
    let Some(bin) = resolve("flatpak") else {
        return false;
    };
    Command::new(bin)
        .args(["info", "net.lutris.Lutris"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}
fn saved(db: &Db) -> Result<Vec<Launcher>> {
    db.get_setting(KEY)?
        .map(|s| serde_json::from_str(&s).map_err(Into::into))
        .unwrap_or_else(|| Ok(vec![]))
}
pub fn list(db: &Db, discover: bool) -> Result<Vec<LauncherInfo>> {
    let custom = saved(db)?;
    let mut all: Vec<_> = if discover { presets() } else { vec![] };
    for item in &custom {
        if let Some(old) = all.iter_mut().find(|l| l.id == item.id) {
            *old = item.clone();
        } else {
            all.push(item.clone());
        }
    }
    let mut info: Vec<_> = all
        .into_iter()
        .map(|launcher| {
            let configured = custom.iter().any(|c| c.id == launcher.id);
            let flatpak_missing = !configured
                && launcher.id == "detected.lutris.flatpak"
                && !flatpak_lutris_installed();
            let status = if flatpak_missing {
                "Lutris Flatpak is not installed".into()
            } else {
                readiness(&launcher)
                    .err()
                    .map(|e| e.message)
                    .unwrap_or_else(|| "Ready to open".into())
            };
            let available = !flatpak_missing
                && resolve(&launcher.executable).is_some()
                && launcher
                    .working_directory
                    .as_ref()
                    .is_none_or(|p| Path::new(p).is_dir());
            LauncherInfo {
                launcher,
                available,
                configured,
                status,
            }
        })
        .collect();
    info.sort_by(|a, b| {
        b.launcher.pinned.cmp(&a.launcher.pinned).then_with(|| {
            a.launcher
                .name
                .to_lowercase()
                .cmp(&b.launcher.name.to_lowercase())
        })
    });
    Ok(info)
}
pub fn validate(item: &Launcher) -> Result<()> {
    fn valid(value: &str, max: usize) -> bool {
        !value.contains('\0') && value.len() <= max
    }
    if item.id.is_empty()
        || item.id.len() > 100
        || !item
            .id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        || item.name.trim().is_empty()
        || !valid(&item.name, 100)
        || item.executable.trim().is_empty()
        || !valid(&item.executable, 4096)
        || item.args.len() > 64
        || item.args.iter().any(|a| !valid(a, 4096))
        || item
            .working_directory
            .as_ref()
            .is_some_and(|p| p.is_empty() || !valid(p, 4096) || !Path::new(p).is_absolute())
    {
        return Err(AppError::new("BAD_REQUEST", "Use a name, executable, up to 64 arguments, and an absolute working directory if supplied"));
    }
    let path = Path::new(&item.executable);
    if path.components().count() > 1 && !path.is_absolute() {
        return Err(AppError::new(
            "BAD_REQUEST",
            "Executable must be an absolute path or a command on PATH",
        ));
    }
    Ok(())
}
pub fn save(db: &Db, mut item: Launcher) -> Result<()> {
    item.name = item.name.trim().to_string();
    item.executable = item.executable.trim().to_string();
    validate(&item)?;
    let mut items = saved(db)?;
    if let Some(old) = items.iter_mut().find(|l| l.id == item.id) {
        *old = item;
    } else {
        if items.len() >= 100 {
            return Err(AppError::new(
                "BAD_REQUEST",
                "Maximum of 100 custom launcher configurations",
            ));
        }
        items.push(item);
    }
    db.set_setting(KEY, &serde_json::to_string(&items)?)
}
pub fn remove(db: &Db, id: &str) -> Result<()> {
    let mut items = saved(db)?;
    items.retain(|l| l.id != id);
    db.set_setting(KEY, &serde_json::to_string(&items)?)
}
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}
pub(crate) fn resolve(bin: &str) -> Option<PathBuf> {
    let path = Path::new(bin);
    if path.is_absolute() {
        return executable(path).then(|| path.to_path_buf());
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|p| p.join(bin))
            .find(|p| executable(p))
    })
}
fn readiness(item: &Launcher) -> Result<PathBuf> {
    let path = resolve(&item.executable).ok_or_else(|| {
        AppError::new("LAUNCHER_MISSING", "Executable not found or not executable")
    })?;
    if item
        .working_directory
        .as_ref()
        .is_some_and(|p| !Path::new(p).is_dir())
    {
        return Err(AppError::new(
            "LAUNCHER_DIRECTORY_MISSING",
            "Working directory is unavailable",
        ));
    }
    Ok(path)
}
pub fn launch(item: &Launcher) -> Result<()> {
    validate(item)?;
    let executable = readiness(item)?;
    let mut command = Command::new(executable);
    command
        .args(&item.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(dir) = &item.working_directory {
        command.current_dir(dir);
    }
    let mut child = command
        .spawn()
        .map_err(|e| AppError::new("SPAWN_FAILED", format!("Could not open {}: {e}", item.name)))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item() -> Launcher {
        Launcher {
            id: "custom.test".into(),
            name: "My launcher".into(),
            executable: "/bin/true".into(),
            args: vec![],
            working_directory: None,
            pinned: true,
        }
    }
    #[test]
    fn saved_configuration_survives_reopen_without_launching() {
        let t = tempfile::tempdir().unwrap();
        let paths = crate::paths::Paths::resolve(Some(t.path().join("data"))).unwrap();
        {
            let db = Db::open(&paths).unwrap();
            save(&db, item()).unwrap();
        }
        let db = Db::open(&paths).unwrap();
        let info = list(&db, false).unwrap();
        assert_eq!(info.len(), 1);
        assert!(info[0].available);
        assert!(info[0].launcher.pinned);
        remove(&db, "custom.test").unwrap();
        assert!(list(&db, false).unwrap().is_empty());
    }
    #[test]
    fn missing_program_or_working_directory_blocks_launch() {
        let mut l = item();
        l.executable = "/no/such/marquee-executable".into();
        assert_eq!(launch(&l).unwrap_err().code, "LAUNCHER_MISSING");
        l.executable = "/bin/true".into();
        l.working_directory = Some("/no/such/marquee-directory".into());
        assert_eq!(launch(&l).unwrap_err().code, "LAUNCHER_DIRECTORY_MISSING");
    }
    #[test]
    fn arguments_are_literal_and_working_directory_is_honored() {
        let t = tempfile::tempdir().unwrap();
        let script = t.path().join("capture.py");
        std::fs::write(&script, "import pathlib,sys\npathlib.Path('args.json').write_text(__import__('json').dumps(sys.argv[1:]))\n").unwrap();
        let mut l = item();
        l.executable = "python3".into();
        l.working_directory = Some(t.path().to_string_lossy().into_owned());
        l.args = vec![
            script.to_string_lossy().into_owned(),
            "space in argument".into(),
            "$(touch injected); `touch injected`".into(),
        ];
        launch(&l).unwrap();
        let out = t.path().join("args.json");
        for _ in 0..100 {
            if out.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let args: Vec<String> = serde_json::from_slice(&std::fs::read(out).unwrap()).unwrap();
        assert_eq!(args, l.args[1..]);
        assert!(!t.path().join("injected").exists());
    }
    #[test]
    fn invalid_configuration_does_not_replace_existing_item() {
        let db = Db::open_in_memory().unwrap();
        save(&db, item()).unwrap();
        let mut bad = item();
        bad.executable = "relative/path".into();
        assert!(save(&db, bad).is_err());
        let mut bad = item();
        bad.args = vec!["null\0byte".into()];
        assert!(save(&db, bad).is_err());
        assert_eq!(
            list(&db, false).unwrap()[0].launcher.executable,
            "/bin/true"
        );
    }
}
