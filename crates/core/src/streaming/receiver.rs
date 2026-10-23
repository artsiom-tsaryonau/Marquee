//! Optional pinned reference client. Installing it does not pair, launch a
//! window, start Wolf, open library.db, or enable an embedded Marquee player.
use crate::error::{AppError, Result};
use crate::{http, instance::InstanceLock, source_builds};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

const SPEC: &str = include_str!("../../../../catalog/streaming/moonlight/reference.json");

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    pub version: String,
    pub platform: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    pub source_url: String,
    pub source_sha256: String,
    pub license: String,
    pub qt_platform: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Installed {
    pub schema_version: u32,
    pub reference: Reference,
    pub program_dir: PathBuf,
    pub executable: PathBuf,
}

fn spec() -> Result<Reference> {
    Ok(serde_json::from_str(SPEC)?)
}
fn prefix(data: &Path) -> PathBuf {
    data.join("streaming/moonlight")
}
fn program(data: &Path, spec: &Reference) -> PathBuf {
    prefix(data).join(format!("{}-{}", spec.version, &spec.sha256[..12]))
}

pub fn status(data: &Path) -> Result<Option<Installed>> {
    let expected = spec()?;
    let dir = program(data, &expected);
    let path = dir.join("reference.json");
    if !path.exists() {
        return Ok(None);
    }
    use std::io::Read;
    let mut bytes = Vec::new();
    fs::File::open(&path)?
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 64 * 1024 {
        return Err(AppError::new(
            "RECEIVER_RECEIPT",
            "Reference receiver receipt is too large",
        ));
    }
    let installed: Installed = serde_json::from_slice(&bytes)?;
    if installed.schema_version != 1
        || installed.reference != expected
        || installed.program_dir != dir
        || installed.executable != dir.join("AppRun")
    {
        return Err(AppError::new(
            "RECEIVER_RECEIPT",
            "Reference receiver receipt differs from the reviewed release",
        ));
    }
    validate_tree(&dir)?;
    Ok(Some(installed))
}

fn validate_tree(root: &Path) -> Result<()> {
    let root = fs::canonicalize(root)?;
    for relative in ["AppRun", "usr/bin/moonlight"] {
        let path = fs::canonicalize(root.join(relative))?;
        if !path.starts_with(&root) || !path.is_file() {
            return Err(AppError::new(
                "RECEIVER_LAYOUT",
                "Reference receiver program escapes its installation",
            ));
        }
        use std::os::unix::fs::PermissionsExt;
        if fs::metadata(path)?.permissions().mode() & 0o111 == 0 {
            return Err(AppError::new(
                "RECEIVER_LAYOUT",
                "Reference receiver program is not executable",
            ));
        }
    }
    Ok(())
}

fn verify(got_hash: &str, size: u64, expected: &Reference) -> Result<()> {
    if got_hash != expected.sha256 || size != expected.size {
        return Err(AppError::new(
            "RECEIVER_CHECKSUM",
            "Receiver download differs from its published checksum or size",
        ));
    }
    Ok(())
}

pub fn install(data: &Path, cancel: &AtomicBool) -> Result<Installed> {
    if std::env::consts::OS != "linux" || std::env::consts::ARCH != "x86_64" {
        return Err(AppError::new(
            "RECEIVER_PLATFORM",
            "The reviewed reference receiver supports Linux x86_64",
        ));
    }
    let dir = prefix(data);
    fs::create_dir_all(&dir)?;
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    let _lock = InstanceLock::acquire(&dir.join("install.lock"))?;
    if let Some(installed) = status(data)? {
        return Ok(installed);
    }
    let expected = spec()?;
    let destination = program(data, &expected);
    if destination.exists() {
        return Err(AppError::new(
            "RECEIVER_LAYOUT",
            "An incomplete receiver installation exists; it was preserved",
        ));
    }
    let work = tempfile::Builder::new()
        .prefix(".install-")
        .tempdir_in(&dir)?;
    let archive = work.path().join("Moonlight.AppImage");
    let got = http::download_file(
        &expected.url,
        &archive,
        expected.size,
        Duration::from_secs(300),
        |_| {},
    )?;
    verify(&got, fs::metadata(&archive)?.len(), &expected)?;
    fs::set_permissions(&archive, fs::Permissions::from_mode(0o700))?;
    let log = dir.join("install.log");
    fs::write(&log, [])?;
    source_builds::run(
        Command::new(&archive)
            .arg("--appimage-extract")
            .current_dir(work.path()),
        &log,
        cancel,
        Duration::from_secs(60),
    )?;
    let extracted = fs::canonicalize(work.path().join("squashfs-root"))?;
    if !extracted.starts_with(fs::canonicalize(work.path())?) {
        return Err(AppError::new(
            "RECEIVER_LAYOUT",
            "Receiver extraction escaped staging",
        ));
    }
    validate_tree(&extracted)?;
    let installed = Installed {
        schema_version: 1,
        reference: expected,
        program_dir: destination.clone(),
        executable: destination.join("AppRun"),
    };
    fs::rename(archive, extracted.join("verified-source.AppImage"))?;
    fs::write(
        extracted.join("reference.json"),
        serde_json::to_vec_pretty(&installed)?,
    )?;
    fs::rename(extracted, &destination)?;
    Ok(installed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checksum_and_size_must_both_match_before_execution() {
        let expected = spec().unwrap();
        assert!(verify(&expected.sha256, expected.size, &expected).is_ok());
        assert_eq!(
            verify("bad", expected.size, &expected).unwrap_err().code,
            "RECEIVER_CHECKSUM"
        );
        assert!(verify(&expected.sha256, 1, &expected).is_err());
    }
    #[test]
    fn extraction_cannot_link_to_a_host_executable() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("usr/bin")).unwrap();
        std::os::unix::fs::symlink("/bin/sh", root.path().join("AppRun")).unwrap();
        std::os::unix::fs::symlink("/bin/sh", root.path().join("usr/bin/moonlight")).unwrap();
        assert_eq!(
            validate_tree(root.path()).unwrap_err().code,
            "RECEIVER_LAYOUT"
        );
    }
}
