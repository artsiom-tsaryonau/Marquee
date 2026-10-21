//! Versioned, conservative DuckStation profiles. Existing and externally edited
//! values belong to the user; only unchanged values in our receipt are managed.
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const VERSION: u32 = 1;
pub const SOURCE: &str = "https://github.com/stenzek/duckstation/blob/48a1028f9a2f3a3c907cca82ce41d30c9ef2d747/src/core/settings.cpp";
static LOCK: Mutex<()> = Mutex::new(());
const KEYS: [(&str, &str); 3] = [
    ("GPU", "Renderer"),
    ("GPU", "ResolutionScale"),
    ("Display", "VSync"),
];

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Host {
    pub gpus: Vec<Gpu>,
    pub displays: Vec<Display>,
    pub session: String,
    pub notes: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Gpu {
    pub vendor: String,
    pub device: String,
    pub driver: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Display {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub refresh_hz: f64,
    pub source: String,
}

/// Output is redirected to a bounded temporary file, so a faulty probe cannot
/// fill a pipe and hang the API. Never invoke a shell or modify display state.
fn probe(program: &str, args: &[&str]) -> Option<String> {
    if !Path::new(program).is_file() {
        return None;
    }
    let mut output = tempfile::tempfile().ok()?;
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(output.try_clone().ok()?)
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_millis(500);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                break;
            }
            Ok(None)
                if Instant::now() < deadline
                    && output.metadata().is_ok_and(|m| m.len() <= 128 * 1024) =>
            {
                std::thread::sleep(Duration::from_millis(10))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    if output.metadata().ok()?.len() > 128 * 1024 {
        return None;
    }
    use std::io::{Seek, SeekFrom};
    output.seek(SeekFrom::Start(0)).ok()?;
    let mut text = String::new();
    output.take(128 * 1024).read_to_string(&mut text).ok()?;
    Some(text)
}

fn xrandr_displays(text: &str, source: &str) -> Vec<Display> {
    let mut name = None;
    let mut result = vec![];
    for line in text.lines() {
        if !line.starts_with(char::is_whitespace) {
            let mut words = line.split_whitespace();
            let candidate = words.next();
            name = if words.next() == Some("connected") {
                candidate
            } else {
                None
            };
            continue;
        }
        let Some(name) = name else {
            continue;
        };
        let mut words = line.split_whitespace();
        let Some((width, height)) = words.next().and_then(|w| w.split_once('x')) else {
            continue;
        };
        let Some(rate) = words.find(|w| w.contains('*')) else {
            continue;
        };
        let (Ok(width), Ok(height), Ok(refresh_hz)) = (
            width.parse(),
            height.parse(),
            rate.trim_end_matches(['*', '+']).parse::<f64>(),
        ) else {
            continue;
        };
        if width > 0 && height > 0 && refresh_hz.is_finite() && refresh_hz > 0.0 {
            result.push(Display {
                name: name.into(),
                width,
                height,
                refresh_hz,
                source: source.into(),
            });
        }
    }
    result
}

pub fn detect_host() -> Host {
    let mut host = Host {
        session: std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".into()),
        ..Host::default()
    };
    if let Ok(entries) = fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name
                .strip_prefix("card")
                .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
            {
                continue;
            }
            let device = entry.path().join("device");
            let vendor = fs::read_to_string(device.join("vendor"))
                .unwrap_or_default()
                .trim()
                .to_string();
            let id = fs::read_to_string(device.join("device"))
                .unwrap_or_default()
                .trim()
                .to_string();
            let driver = fs::read_link(device.join("driver"))
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                .unwrap_or_else(|| "unknown".into());
            let vendor = match vendor.as_str() {
                "0x10de" => "NVIDIA",
                "0x1002" => "AMD",
                "0x8086" => "Intel",
                _ => &vendor,
            }
            .to_string();
            host.gpus.push(Gpu {
                vendor,
                device: id,
                driver,
            });
        }
    }
    host.gpus
        .sort_by(|a, b| (&a.vendor, &a.device).cmp(&(&b.vendor, &b.device)));
    if std::env::var_os("DISPLAY").is_some() {
        if let Some(text) = probe("/usr/bin/xrandr", &["--query"]) {
            let source = if host.session == "wayland" {
                "Xwayland reported mode"
            } else {
                "X11 active mode"
            };
            host.displays = xrandr_displays(&text, source);
        }
    }
    if host.gpus.is_empty() {
        host.notes
            .push("GPU and driver could not be detected; using conservative defaults.".into());
    }
    if host.displays.is_empty() {
        host.notes.push(
            "Active display resolution/refresh unavailable. No monitor mode is assumed.".into(),
        );
    }
    if host.session == "wayland" {
        host.notes.push("Xwayland modes may differ from physical monitor modes; the recommendation stays at native scale.".into());
    }
    host
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub renderer: String,
    pub resolution_scale: u8,
    pub vsync: bool,
}
impl Options {
    fn validate(&self) -> Result<()> {
        if !matches!(
            self.renderer.as_str(),
            "Automatic" | "Vulkan" | "OpenGL" | "Software"
        ) || ![1, 2, 3, 4, 6, 8].contains(&self.resolution_scale)
        {
            return Err(AppError::new(
                "BAD_REQUEST",
                "Choose a supported renderer and resolution scale",
            ));
        }
        Ok(())
    }
    fn values(&self) -> [String; 3] {
        [
            self.renderer.clone(),
            self.resolution_scale.to_string(),
            self.vsync.to_string(),
        ]
    }
}
pub fn recommended(host: &Host) -> Options {
    let height = host.displays.iter().map(|d| d.height).min().unwrap_or(0);
    let hardware = host
        .gpus
        .iter()
        .any(|g| matches!(g.vendor.as_str(), "NVIDIA" | "AMD" | "Intel") && g.driver != "unknown");
    let scale = if hardware && host.session == "x11" {
        if height >= 1440 {
            3
        } else if height >= 720 {
            2
        } else {
            1
        }
    } else {
        1
    };
    Options {
        renderer: "Automatic".into(),
        resolution_scale: scale,
        vsync: host.session == "x11"
            && host
                .displays
                .iter()
                .any(|d| (50.0..=360.0).contains(&d.refresh_hz)),
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Recommended,
    Custom,
    Disabled,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub mode: Mode,
    pub custom: Options,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Applied {
    value: String,
    before: Option<String>,
    released: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Receipt {
    version: u32,
    config: Config,
    applied: BTreeMap<String, Applied>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Field {
    pub section: String,
    pub key: String,
    pub desired: String,
    pub current: Option<String>,
    pub managed: bool,
    pub preserved: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub version: u32,
    pub source: &'static str,
    pub host: Host,
    pub recommended: Options,
    pub config: Config,
    pub fields: Vec<Field>,
    pub settings_path: PathBuf,
}

fn files(home: &Path) -> (PathBuf, PathBuf) {
    let dir = home.join("duckstation");
    (dir.join("settings.ini"), dir.join("marquee-profile.json"))
}
fn checked_read(path: &Path, max: u64) -> Result<String> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.into()),
        Ok(meta) => {
            if !meta.is_file() || meta.len() > max {
                return Err(AppError::new(
                    "PROFILE_CONFIG",
                    "Profile files must be regular bounded files, not symlinks",
                ));
            }
            Ok(fs::read_to_string(path)?)
        }
    }
}
fn read_state(home: &Path, host: &Host) -> Result<(String, Receipt)> {
    // Refuse redirected managed config directories rather than modifying external settings.
    for path in [
        home.parent().unwrap_or(home).to_path_buf(),
        home.to_path_buf(),
        home.join("duckstation"),
    ] {
        if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(AppError::new(
                "PROFILE_CONFIG",
                "Managed profile directory must not be a symlink",
            ));
        }
    }
    let (ini, receipt) = files(home);
    let text = checked_read(&ini, 1024 * 1024)?;
    let raw = checked_read(&receipt, 64 * 1024)?;
    let state = if raw.is_empty() {
        Receipt {
            version: VERSION,
            config: Config {
                mode: Mode::Recommended,
                custom: recommended(host),
            },
            applied: BTreeMap::new(),
        }
    } else {
        serde_json::from_str::<Receipt>(&raw).map_err(|_| {
            AppError::new(
                "PROFILE_CONFIG",
                "Profile receipt is invalid; existing settings were left unchanged",
            )
        })?
    };
    if state.version != VERSION {
        return Err(AppError::new(
            "PROFILE_VERSION",
            "Unsupported profile version; settings were left unchanged",
        ));
    }
    state.config.custom.validate()?;
    Ok((text, state))
}
fn values(text: &str, section: &str, key: &str) -> Vec<String> {
    let mut current = "";
    let mut result = vec![];
    for line in text.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.starts_with('[') && line.ends_with(']') {
            current = &line[1..line.len() - 1];
        } else if current.eq_ignore_ascii_case(section) && !line.starts_with([';', '#']) {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim().eq_ignore_ascii_case(key) {
                    result.push(v.trim().into());
                }
            }
        }
    }
    result
}
fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}
fn status(home: &Path, host: Host, text: &str, state: &Receipt) -> Status {
    let rec = recommended(&host);
    let options = if state.config.mode == Mode::Recommended {
        &rec
    } else {
        &state.config.custom
    };
    let fields = KEYS
        .iter()
        .zip(options.values())
        .map(|((section, key), desired)| {
            let current = values(text, section, key);
            let applied = state.applied.get(&format!("{section}.{key}"));
            let managed = current.len() == 1
                && applied.is_some_and(|a| !a.released && same(&current[0], &a.value));
            Field {
                section: (*section).into(),
                key: (*key).into(),
                desired,
                current: current.first().cloned(),
                managed,
                preserved: !current.is_empty() && !managed,
            }
        })
        .collect();
    Status {
        version: VERSION,
        source: SOURCE,
        host,
        recommended: rec,
        config: state.config.clone(),
        fields,
        settings_path: files(home).0,
    }
}
pub fn get(home: &Path) -> Result<Status> {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let host = detect_host();
    let (text, state) = read_state(home, &host)?;
    Ok(status(home, host, &text, &state))
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::new("PROFILE_CONFIG", "Invalid managed path"))?;
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| AppError::from(e.error))?;
    Ok(())
}
/// Managed processes keep their executable under runners/duckstation/current.
/// Inspect executable paths only, never process environments or credentials.
fn running(home: &Path) -> bool {
    let Some(data) = home.parent().and_then(Path::parent) else {
        return false;
    };
    let program = data.join("runners/duckstation/current");
    let program = fs::canonicalize(&program).unwrap_or(program);
    fs::read_dir("/proc").ok().is_some_and(|entries| {
        entries.flatten().any(|e| {
            e.file_name()
                .to_string_lossy()
                .chars()
                .all(|c| c.is_ascii_digit())
                && fs::read_link(e.path().join("exe")).is_ok_and(|p| p.starts_with(&program))
        })
    })
}
fn apply(home: &Path, host: Host, config: Option<Config>) -> Result<Status> {
    if running(home) {
        return Err(AppError::new(
            "PROFILE_BUSY",
            "Close DuckStation before changing its settings profile",
        ));
    }
    let (original, mut state) = read_state(home, &host)?;
    if let Some(config) = config {
        config.custom.validate()?;
        state.config = config;
    }
    let options = if state.config.mode == Mode::Recommended {
        recommended(&host)
    } else {
        state.config.custom.clone()
    };
    let mut text = original.clone();
    if state.config.mode != Mode::Disabled {
        for ((section, key), desired) in KEYS.iter().zip(options.values()) {
            let id = format!("{section}.{key}");
            let current = values(&text, section, key);
            let owned = state
                .applied
                .get(&id)
                .is_some_and(|a| !a.released && current.len() == 1 && same(&current[0], &a.value));
            let new = !state.applied.contains_key(&id) && current.is_empty();
            if owned || new {
                if current.len() == 1 && same(&current[0], &desired) {
                    continue;
                }
                text = crate::bios::set_ini_value(&text, section, key, &desired);
                state.applied.insert(
                    id,
                    Applied {
                        value: desired,
                        before: current.first().cloned(),
                        released: false,
                    },
                );
            } else if let Some(applied) = state.applied.get_mut(&id) {
                applied.released = true;
            }
        }
    }
    let (ini, receipt) = files(home);
    let bytes = serde_json::to_vec_pretty(&state)?;
    // Receipt first: a crash between writes conservatively releases ownership;
    // it can never make a later launch overwrite a user's changed value.
    if checked_read(&receipt, 64 * 1024)?.as_bytes() != bytes {
        atomic(&receipt, &bytes)?;
    }
    if text != original {
        atomic(&ini, text.as_bytes())?;
    }
    Ok(status(home, host, &text, &state))
}
pub fn configure(home: &Path, config: Config) -> Result<Status> {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    apply(home, detect_host(), Some(config))
}
/// First-run flags are launch-owned; user rendering settings remain receipt-owned.
pub fn prepare(home: &Path) -> Result<()> {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if running(home) {
        return Err(AppError::new(
            "PROFILE_BUSY",
            "DuckStation is already running",
        ));
    }
    // Validate before the wizard seed to leave malformed/symlinked profiles alone.
    read_state(home, &Host::default())?;
    crate::bios::seed_duckstation(home)?;
    apply(home, detect_host(), None)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn host() -> Host {
        Host {
            session: "x11".into(),
            gpus: vec![Gpu {
                vendor: "AMD".into(),
                device: "test".into(),
                driver: "amdgpu".into(),
            }],
            displays: vec![Display {
                name: "DP-1".into(),
                width: 2560,
                height: 1440,
                refresh_hz: 144.0,
                source: "fixture".into(),
            }],
            notes: vec![],
        }
    }
    fn custom(renderer: &str, scale: u8) -> Config {
        Config {
            mode: Mode::Custom,
            custom: Options {
                renderer: renderer.into(),
                resolution_scale: scale,
                vsync: false,
            },
        }
    }
    #[test]
    fn recommendations_are_conservative_when_hardware_or_display_is_unknown() {
        assert_eq!(recommended(&Host::default()).resolution_scale, 1);
        assert_eq!(recommended(&host()).resolution_scale, 3);
        let mut wayland = host();
        wayland.session = "wayland".into();
        assert_eq!(recommended(&wayland).resolution_scale, 1);
        assert!(!recommended(&wayland).vsync);
    }
    #[test]
    fn active_display_parser_ignores_supported_but_inactive_modes() {
        let result=xrandr_displays("Screen 0: minimum 8 x 8\nDP-1 connected primary 2560x1440+0+0\n   2560x1440  143.91*+ 60.00\n   1920x1080  60.00\nHDMI-1 disconnected\n   800x600 60.00\n", "fixture");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].width, 2560);
        assert_eq!(result[0].refresh_hz, 143.91);
    }
    #[test]
    fn first_apply_records_changes_and_managed_reapply_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let first = apply(dir.path(), host(), None).unwrap();
        assert!(first.fields.iter().all(|f| f.managed));
        let (ini, receipt) = files(dir.path());
        let before = fs::read(&ini).unwrap();
        let rec = fs::read(&receipt).unwrap();
        apply(dir.path(), host(), None).unwrap();
        assert_eq!(fs::read(&ini).unwrap(), before);
        assert_eq!(fs::read(&receipt).unwrap(), rec);
        let updated = apply(dir.path(), host(), Some(custom("OpenGL", 2))).unwrap();
        assert_eq!(updated.fields[0].current.as_deref(), Some("OpenGL"));
        let state: Receipt = serde_json::from_slice(&fs::read(receipt).unwrap()).unwrap();
        assert_eq!(
            state.applied["GPU.Renderer"].before.as_deref(),
            Some("Automatic")
        );
    }
    #[test]
    fn preexisting_settings_and_unrelated_comments_are_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let (ini, _) = files(dir.path());
        fs::create_dir_all(ini.parent().unwrap()).unwrap();
        fs::write(&ini,"; user settings\n[GPU]\nRenderer = Software\nResolutionScale = 4\n[Pad1]\nCross = Keyboard/X\n").unwrap();
        let result = apply(dir.path(), host(), Some(custom("Vulkan", 2))).unwrap();
        assert!(result.fields[0].preserved);
        assert!(result.fields[1].preserved);
        let text = fs::read_to_string(ini).unwrap();
        assert!(text.contains("Renderer = Software"));
        assert!(text.contains("Cross = Keyboard/X"));
        assert!(text.starts_with("; user settings"));
    }
    #[test]
    fn external_changes_or_deletions_release_ownership_permanently() {
        let dir = tempfile::tempdir().unwrap();
        apply(dir.path(), host(), None).unwrap();
        let (ini, _) = files(dir.path());
        let text = fs::read_to_string(&ini)
            .unwrap()
            .replace("Renderer = Automatic", "Renderer = Software")
            .replace("ResolutionScale = 3\n", "");
        fs::write(&ini, &text).unwrap();
        apply(dir.path(), host(), Some(custom("Vulkan", 2))).unwrap();
        let text = fs::read_to_string(&ini).unwrap();
        assert!(text.contains("Renderer = Software"));
        assert!(!text.contains("ResolutionScale"));
        // Even if an external value later equals the old managed value it stays released.
        fs::write(
            &ini,
            text.replace("Renderer = Software", "Renderer = Automatic"),
        )
        .unwrap();
        apply(dir.path(), host(), Some(custom("Vulkan", 2))).unwrap();
        assert!(fs::read_to_string(ini)
            .unwrap()
            .contains("Renderer = Automatic"));
    }
    #[test]
    fn duplicate_keys_and_disabled_mode_never_overwrite_user_values() {
        let dir = tempfile::tempdir().unwrap();
        let (ini, _) = files(dir.path());
        fs::create_dir_all(ini.parent().unwrap()).unwrap();
        fs::write(&ini, "[GPU]\nRenderer = Software\nRenderer = Vulkan\n").unwrap();
        assert!(apply(dir.path(), host(), None).unwrap().fields[0].preserved);
        let before = fs::read(&ini).unwrap();
        let mut cfg = custom("OpenGL", 2);
        cfg.mode = Mode::Disabled;
        apply(dir.path(), host(), Some(cfg)).unwrap();
        assert_eq!(fs::read(&ini).unwrap(), before);
    }
    #[test]
    fn invalid_options_or_receipts_leave_the_settings_untouched() {
        let dir = tempfile::tempdir().unwrap();
        apply(dir.path(), host(), None).unwrap();
        let (ini, receipt) = files(dir.path());
        let before = fs::read(&ini).unwrap();
        assert!(apply(dir.path(), host(), Some(custom("unsupported", 255))).is_err());
        fs::write(&receipt, b"invalid json").unwrap();
        assert!(prepare(dir.path()).is_err());
        assert_eq!(fs::read(ini).unwrap(), before);
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_settings_and_parent_directories_are_refused() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("original.ini");
        fs::write(&target, "untouched").unwrap();
        let home = dir.path().join("managed/xdg-config");
        let (ini, _) = files(&home);
        fs::create_dir_all(ini.parent().unwrap()).unwrap();
        symlink(&target, &ini).unwrap();
        assert!(apply(&home, host(), None).is_err());
        assert_eq!(fs::read_to_string(target).unwrap(), "untouched");
        let home = dir.path().join("redirected/xdg-config");
        symlink(dir.path().join("managed"), dir.path().join("redirected")).unwrap();
        assert!(apply(&home, host(), None).is_err());
    }
    #[test]
    fn preparation_suppresses_the_wizard_without_claiming_user_graphics_settings() {
        let dir = tempfile::tempdir().unwrap();
        let (ini, _) = files(dir.path());
        fs::create_dir_all(ini.parent().unwrap()).unwrap();
        fs::write(
            &ini,
            "[Main]\nSetupWizardIncomplete = true\n[GPU]\nRenderer = Software\n",
        )
        .unwrap();
        prepare(dir.path()).unwrap();
        let text = fs::read_to_string(&ini).unwrap();
        assert!(text.contains("SetupWizardIncomplete = false"));
        assert!(text.contains("Renderer = Software"));
    }
    #[test]
    fn shared_api_and_restart_preserve_the_profile_without_touching_the_library() {
        use crate::{api, Core};
        use serde_json::json;
        use std::sync::Arc;
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        {
            let core = Arc::new(Core::open_isolated(Some(data.clone()), true).unwrap());
            assert!(core.emulator_profile("pcsx2").is_err());
            let before = core.snapshot().unwrap().editions.len();
            let preview =
                api::dispatch(&core, "emulator_profile", json!({"id":"duckstation"})).unwrap();
            assert_eq!(preview["version"], VERSION);
            assert!(
                !files(&crate::emulators::config_home(&core.paths, "duckstation"))
                    .0
                    .exists()
            );
            let result = api::dispatch(&core,"configure_emulator_profile",json!({"id":"duckstation","config":{"mode":"custom","custom":{"renderer":"OpenGL","resolution_scale":2,"vsync":true}}})).unwrap();
            assert_eq!(result["config"]["custom"]["renderer"], "OpenGL");
            assert_eq!(core.snapshot().unwrap().editions.len(), before);
        }
        let core = Core::open_isolated(Some(data), true).unwrap();
        let status = core.emulator_profile("duckstation").unwrap();
        assert_eq!(status.config.mode, Mode::Custom);
        assert_eq!(status.config.custom.renderer, "OpenGL");
        assert!(status.fields.iter().all(|f| f.managed));
    }
}
