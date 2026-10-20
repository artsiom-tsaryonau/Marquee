use crate::error::{AppError, Result};
use crate::http;
use crate::model::CoreEvent;
use crate::paths::Paths;
use crate::runners::{self, FLATPAK_APP};
use crate::sessions::EventSink;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

pub const TOOL_PCSX2: &str = "pcsx2";
const GITHUB_LATEST: &str = "https://api.github.com/repos/PCSX2/pcsx2/releases/latest";
const APPIMAGE_SUFFIX: &str = "linux-appimage-x64-Qt.AppImage";
const FLATHUB_REPO: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pcsx2Channel {
    Auto,
    Unpacked,
    AppImage,
    Flatpak,
}

impl Pcsx2Channel {
    pub fn parse(raw: &str) -> Result<Self> {
        match raw {
            "auto" | "" => Ok(Self::Auto),
            "unpacked" => Ok(Self::Unpacked),
            "flatpak" => Ok(Self::Flatpak),
            "appimage" => Ok(Self::AppImage),
            other => Err(AppError::new(
                "RUNNER_MISSING",
                format!("unknown PCSX2 channel: {other}"),
            )),
        }
    }

    pub fn resolve(self) -> Self {
        match self {
            Self::Auto => Self::Unpacked,
            other => other,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Unpacked => "unpacked",
            Self::Flatpak => "flatpak",
            Self::AppImage => "appimage",
        }
    }
}

#[derive(Debug, Clone)]
pub enum InstallOutcome {
    AlreadyPresent,
    Flatpak,
    AppImage { path: PathBuf },
    Unpacked { bin: PathBuf },
}

pub fn already_have(
    channel: Pcsx2Channel,
    paths: &Paths,
    runners: &[crate::model::Runner],
) -> bool {
    match channel.resolve() {
        Pcsx2Channel::Unpacked => {
            runners::find_extracted_bin(&paths.pcsx2_unpacked_current()).is_some()
        }
        Pcsx2Channel::AppImage => paths.pcsx2_appimage_path().is_file(),
        Pcsx2Channel::Flatpak => runners
            .iter()
            .any(|r| r.id == "runner.pcsx2.flatpak" && r.available),
        Pcsx2Channel::Auto => unreachable!(),
    }
}

pub fn install_pcsx2(
    paths: &Paths,
    channel: Pcsx2Channel,
    sink: Arc<dyn EventSink>,
) -> Result<InstallOutcome> {
    let resolved = channel.resolve();
    sink.emit(CoreEvent::ToolInstallStarted {
        tool_id: TOOL_PCSX2.into(),
        channel: resolved.as_str().into(),
    });
    prepare_pcsx2_home(paths)?;
    let outcome = match resolved {
        Pcsx2Channel::Unpacked => {
            let bin = install_unpacked(paths, &sink)?;
            InstallOutcome::Unpacked { bin }
        }
        Pcsx2Channel::Flatpak => {
            install_flatpak(&sink)?;
            InstallOutcome::Flatpak
        }
        Pcsx2Channel::AppImage => {
            let path = download_official_appimage(&sink, &paths.pcsx2_appimage_path())?;
            InstallOutcome::AppImage { path }
        }
        Pcsx2Channel::Auto => unreachable!(),
    };
    Ok(outcome)
}

fn prepare_pcsx2_home(paths: &Paths) -> Result<()> {
    fs::create_dir_all(paths.pcsx2_bios_dir())?;
    Ok(())
}

fn emit_progress(sink: &Arc<dyn EventSink>, message: impl Into<String>) {
    sink.emit(CoreEvent::ToolInstallProgress {
        tool_id: TOOL_PCSX2.into(),
        message: message.into(),
    });
}

fn install_flatpak(sink: &Arc<dyn EventSink>) -> Result<()> {
    if runners::which("flatpak").is_none() {
        return Err(AppError::new(
            "RUNNER_MISSING",
            "flatpak is not installed. Install Flatpak or choose the AppImage channel.",
        ));
    }
    emit_progress(sink, "Adding Flathub (user) if needed…");
    run_argv(
        &[
            "flatpak".into(),
            "remote-add".into(),
            "--user".into(),
            "--if-not-exists".into(),
            "flathub".into(),
            FLATHUB_REPO.into(),
        ],
        sink,
    )?;
    emit_progress(sink, "Installing net.pcsx2.PCSX2 from Flathub…");
    run_argv(
        &[
            "flatpak".into(),
            "install".into(),
            "--user".into(),
            "--noninteractive".into(),
            "flathub".into(),
            FLATPAK_APP.into(),
        ],
        sink,
    )
}

fn install_unpacked(paths: &Paths, sink: &Arc<dyn EventSink>) -> Result<PathBuf> {
    let dest = paths.pcsx2_unpacked_current();
    if let Some(bin) = runners::find_extracted_bin(&dest) {
        emit_progress(sink, format!("Using unpacked PCSX2 at {}", bin.display()));
        return Ok(bin);
    }
    let archive = download_official_appimage(sink, &paths.pcsx2_appimage_path())?;
    emit_progress(sink, "Unpacking AppImage into Marquee’s data dir…");
    extract_appimage(&archive, &dest)?;
    let bin = runners::find_extracted_bin(&dest)
        .ok_or_else(|| AppError::new("SPAWN_FAILED", "unpacked AppImage has no pcsx2-qt binary"))?;
    emit_progress(sink, format!("Unpacked {}", bin.display()));
    Ok(bin)
}

pub(crate) fn extract_appimage(appimage: &Path, dest: &Path) -> Result<()> {
    let parent = dest
        .parent()
        .ok_or_else(|| AppError::new("PATH", "unpacked PCSX2 destination has no parent"))?;
    let work = parent.join(".extracting");
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work)?;
    let output = Command::new(appimage)
        .arg("--appimage-extract")
        .current_dir(&work)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| {
            let _ = fs::remove_dir_all(&work);
            AppError::new("SPAWN_FAILED", format!("could not unpack AppImage: {err}"))
        })?;
    let root = work.join("squashfs-root");
    if !output.status.success() || runners::find_extracted_bin(&root).is_none() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = fs::remove_dir_all(&work);
        return Err(AppError::new(
            "SPAWN_FAILED",
            if stderr.trim().is_empty() {
                "AppImage --appimage-extract did not produce pcsx2-qt".into()
            } else {
                stderr.lines().rev().take(8).collect::<Vec<_>>().join("\n")
            },
        ));
    }
    let _ = fs::remove_dir_all(dest);
    fs::rename(&root, dest).map_err(|err| {
        let _ = fs::remove_dir_all(&work);
        AppError::new("IO", format!("could not move unpacked PCSX2: {err}"))
    })?;
    let _ = fs::remove_dir_all(&work);
    Ok(())
}

fn download_official_appimage(sink: &Arc<dyn EventSink>, dest: &Path) -> Result<PathBuf> {
    emit_progress(sink, "Looking up the latest official PCSX2 AppImage…");
    let json = http::get_json(GITHUB_LATEST)?;
    let asset = appimage_asset(&json)?;
    emit_progress(sink, format!("Downloading {}…", asset.name));
    if let Some(dir) = dest.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = dest.with_extension("AppImage.part");
    let _ = fs::remove_file(&tmp);
    let mut last_progress = 0u64;
    let got = http::download_file(
        &asset.url,
        &tmp,
        http::APPIMAGE_MAX_BYTES,
        std::time::Duration::from_secs(600),
        |n| {
            if n.saturating_sub(last_progress) >= 8 * 1024 * 1024 {
                last_progress = n;
                emit_progress(sink, format!("Downloaded {} MiB…", n / (1024 * 1024)));
            }
        },
    )
    .inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })?;
    if got != asset.sha256 {
        let _ = fs::remove_file(&tmp);
        return Err(AppError::new(
            "NETWORK",
            "AppImage checksum did not match the GitHub release digest",
        )
        .with_context(json!({ "expected": asset.sha256, "got": got })));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&tmp)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&tmp, perms)?;
    }
    fs::rename(&tmp, dest)?;
    emit_progress(sink, format!("Saved {}", dest.display()));
    Ok(dest.to_path_buf())
}

struct GithubAsset {
    name: String,
    url: String,
    sha256: String,
}

fn appimage_asset(release: &serde_json::Value) -> Result<GithubAsset> {
    let assets = release
        .get("assets")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::new("NETWORK", "GitHub release has no assets"))?;
    let asset = assets
        .iter()
        .find(|a| {
            a.get("name")
                .and_then(|n| n.as_str())
                .is_some_and(|n| n.ends_with(APPIMAGE_SUFFIX))
        })
        .ok_or_else(|| {
            AppError::new(
                "NETWORK",
                "GitHub release has no linux-appimage-x64-Qt.AppImage",
            )
        })?;
    let name = asset
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("pcsx2.AppImage")
        .to_string();
    let url = asset
        .get("browser_download_url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::new("NETWORK", "AppImage asset has no URL"))?
        .to_string();
    let digest = asset
        .get("digest")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let sha256 = digest
        .strip_prefix("sha256:")
        .ok_or_else(|| AppError::new("NETWORK", "AppImage asset has no sha256 digest"))?
        .to_ascii_lowercase();
    Ok(GithubAsset { name, url, sha256 })
}

fn run_argv(argv: &[String], sink: &Arc<dyn EventSink>) -> Result<()> {
    let (bin, args) = argv
        .split_first()
        .ok_or_else(|| AppError::new("SPAWN_FAILED", "install command is empty"))?;
    if bin.contains('/') || bin.contains('\\') {
        let path = Path::new(bin);
        if !path.is_absolute() {
            return Err(AppError::new(
                "SPAWN_FAILED",
                "refusing relative installer path",
            ));
        }
    }
    emit_progress(sink, argv.join(" "));
    let output = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| {
            AppError::new("SPAWN_FAILED", err.to_string()).with_context(json!({ "argv": argv }))
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    for line in stdout.lines().chain(stderr.lines()) {
        let line = line.trim();
        if !line.is_empty() {
            emit_progress(sink, line);
        }
    }
    if !output.status.success() {
        let tail = stderr.lines().rev().take(8).collect::<Vec<_>>().join("\n");
        return Err(AppError::new(
            "SPAWN_FAILED",
            if tail.is_empty() {
                format!("{bin} exited {}", output.status)
            } else {
                tail
            },
        )
        .with_context(json!({ "argv": argv, "status": output.status.code() })));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_official_appimage_asset_and_digest() {
        let json = serde_json::json!({
            "assets": [
                {
                    "name": "pcsx2-v2.8.2-windows-x64-Qt.7z",
                    "browser_download_url": "https://example.invalid/win",
                    "digest": "sha256:aaaa"
                },
                {
                    "name": "pcsx2-v2.8.2-linux-appimage-x64-Qt.AppImage",
                    "browser_download_url": "https://example.invalid/app",
                    "digest": "sha256:0c46bb6a88aa2782b10853a7b07cf3387ba99cbef2b966372cd2315b8571abea"
                }
            ]
        });
        let asset = appimage_asset(&json).unwrap();
        assert!(asset.name.ends_with("linux-appimage-x64-Qt.AppImage"));
        assert_eq!(
            asset.sha256,
            "0c46bb6a88aa2782b10853a7b07cf3387ba99cbef2b966372cd2315b8571abea"
        );
    }

    #[test]
    fn auto_channel_unpacks_instead_of_flatpak() {
        assert_eq!(Pcsx2Channel::Auto.resolve(), Pcsx2Channel::Unpacked);
    }

    #[test]
    fn unpacks_a_fake_appimage_into_current() {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("fake.AppImage");
        std::fs::write(
            &fake,
            "#!/bin/sh\nmkdir -p squashfs-root/usr/bin\nprintf '#!/bin/sh\\nexit 0\\n' > squashfs-root/usr/bin/pcsx2-qt\nchmod +x squashfs-root/usr/bin/pcsx2-qt\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&fake).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&fake, perms).unwrap();
        }
        let dest = dir.path().join("current");
        extract_appimage(&fake, &dest).unwrap();
        assert!(dest.join("usr/bin/pcsx2-qt").is_file());
        assert_eq!(
            runners::find_extracted_bin(&dest)
                .unwrap()
                .file_name()
                .unwrap(),
            "pcsx2-qt"
        );
    }
}
