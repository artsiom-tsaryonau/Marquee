//! Reviewed rootless source build, immutable image receipt and offline probes.
//! No library database, host services, network listeners or gameplay are started.
use crate::error::{AppError, Result};
use crate::instance::InstanceLock;
use crate::source_builds::{run, valid_image};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

const FILES: &[(&str, &[u8])] = &[
    (
        "Containerfile",
        include_bytes!("../../../../catalog/streaming/wolf/Containerfile"),
    ),
    (
        "source-lock.json",
        include_bytes!("../../../../catalog/streaming/wolf/source-lock.json"),
    ),
    (
        "patch-source.py",
        include_bytes!("../../../../catalog/streaming/wolf/patch-source.py"),
    ),
    (
        "entrypoint.py",
        include_bytes!("../../../../catalog/streaming/wolf/entrypoint.py"),
    ),
    (
        "probe.py",
        include_bytes!("../../../../catalog/streaming/wolf/probe.py"),
    ),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    #[serde(default = "receipt_schema")]
    pub schema_version: u32,
    pub image: String,
    pub wolf_revision: String,
    pub compositor_revision: String,
    pub recipe_sha256: String,
    pub built_at: String,
    pub baseline_probe: Value,
    pub license_inventory: String,
    pub live_streaming_verified: bool,
}

fn receipt_schema() -> u32 {
    1
}

fn root(data: &Path) -> PathBuf {
    data.join("streaming/wolf")
}

fn recipe_hash() -> String {
    let mut hash = Sha256::new();
    for (name, bytes) in FILES {
        hash.update(name);
        hash.update([0]);
        hash.update(bytes);
    }
    format!("{:x}", hash.finalize())
}

pub fn status(data: &Path) -> Result<Option<Receipt>> {
    read_receipt(&root(data).join("current.json"))
}

fn read_receipt(path: &Path) -> Result<Option<Receipt>> {
    use std::io::Read;
    match fs::File::open(path) {
        Ok(file) => {
            let mut bytes = Vec::new();
            file.take(64 * 1024 + 1).read_to_end(&mut bytes)?;
            if bytes.len() > 64 * 1024 {
                return Err(AppError::new(
                    "STREAMING_RECEIPT",
                    "Streaming receipt is too large",
                ));
            }
            let receipt: Receipt = serde_json::from_slice(&bytes)?;
            if receipt.schema_version != receipt_schema() || !valid_image(&receipt.image) {
                return Err(AppError::new(
                    "STREAMING_RECEIPT",
                    "Invalid streaming image receipt",
                ));
            }
            Ok(Some(receipt))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn fetch(
    context: &Path,
    name: &str,
    repository: &str,
    revision: &str,
    log: &Path,
    cancel: &AtomicBool,
) -> Result<()> {
    let source = context.join(name);
    fs::create_dir(&source)?;
    run(
        Command::new("git").args(["init", "--quiet"]).arg(&source),
        log,
        cancel,
        Duration::from_secs(15),
    )?;
    run(
        Command::new("git").arg("-C").arg(&source).args([
            "-c",
            "core.hooksPath=/dev/null",
            "fetch",
            "--depth=1",
            repository,
            revision,
        ]),
        log,
        cancel,
        Duration::from_secs(300),
    )?;
    run(
        Command::new("git").arg("-C").arg(&source).args([
            "-c",
            "core.hooksPath=/dev/null",
            "checkout",
            "--detach",
            revision,
        ]),
        log,
        cancel,
        Duration::from_secs(30),
    )?;
    if fs::read_to_string(source.join(".git/HEAD"))?.trim() != revision
        || source.join(".gitmodules").exists()
    {
        return Err(AppError::new(
            "STREAMING_SOURCE",
            "Streaming source differs from the reviewed recipe",
        ));
    }
    fs::remove_dir_all(source.join(".git"))?;
    Ok(())
}

/// Compile/probe before atomically promoting a receipt. Failed attempts preserve
/// the current image and do not delete old images, runtime state or paired clients.
pub fn build(data: &Path, cancel: &AtomicBool) -> Result<Receipt> {
    if std::env::consts::OS != "linux" || std::env::consts::ARCH != "x86_64" {
        return Err(AppError::new(
            "STREAMING_PLATFORM",
            "The reviewed Wolf build currently supports Linux x86_64",
        ));
    }
    let dir = root(data);
    fs::create_dir_all(&dir)?;
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    let _lock = InstanceLock::acquire(&dir.join("build.lock"))?;
    let work = tempfile::Builder::new()
        .prefix(".build-")
        .tempdir_in(&dir)?;
    let log = dir.join("build.log");
    fs::write(&log, [])?;
    let preflight = work.path().join("podman.json");
    run(
        Command::new("podman").args(["info", "--format=json"]),
        &preflight,
        cancel,
        Duration::from_secs(30),
    )?;
    let info: Value = serde_json::from_slice(&fs::read(&preflight)?)?;
    if info
        .pointer("/host/security/rootless")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err(AppError::new(
            "STREAMING_ROOTLESS",
            "Configure rootless Podman before building Wolf",
        ));
    }
    let lock: Value = serde_json::from_slice(FILES[1].1)?;
    let pinned = |key: &str| -> Result<&str> {
        lock[key]
            .as_str()
            .ok_or_else(|| AppError::new("STREAMING_RECIPE", "Invalid embedded source lock"))
    };
    let context = work.path().join("context");
    fs::create_dir(&context)?;
    for (name, bytes) in FILES {
        fs::write(context.join(name), bytes)?;
    }
    fetch(
        &context,
        "wolf",
        "https://github.com/games-on-whales/wolf.git",
        pinned("wolf")?,
        &log,
        cancel,
    )?;
    fetch(
        &context,
        "compositor",
        "https://github.com/games-on-whales/gst-wayland-display.git",
        pinned("compositor")?,
        &log,
        cancel,
    )?;
    let image_file = work.path().join("image-id");
    run(
        Command::new("podman")
            .args(["build", "--force-rm", "--layers", "--iidfile"])
            .arg(&image_file)
            .arg("--build-arg")
            .arg(format!("GSTREAMER_IMAGE={}", pinned("gstreamer_image")?))
            .arg("--build-arg")
            .arg(format!("RUST_IMAGE={}", pinned("rust_image")?))
            .arg(&context),
        &log,
        cancel,
        Duration::from_secs(7200),
    )?;
    let image = fs::read_to_string(image_file)?.trim().to_owned();
    if !valid_image(&image) {
        return Err(AppError::new(
            "STREAMING_IMAGE",
            "Builder returned an invalid image identity",
        ));
    }
    let baseline_probe = probe_image(&image, &work.path().join("probe.json"), cancel, false, None)?;
    let receipt = Receipt {
        schema_version: receipt_schema(),
        image,
        wolf_revision: pinned("wolf")?.into(),
        compositor_revision: pinned("compositor")?.into(),
        recipe_sha256: recipe_hash(),
        built_at: crate::db::now_rfc3339(),
        baseline_probe,
        license_inventory:
            "/opt/marquee-wolf/licenses inside the image; distribution audit remains open".into(),
        live_streaming_verified: false,
    };
    promote(&dir, &receipt)?;
    Ok(receipt)
}

fn promote(dir: &Path, receipt: &Receipt) -> Result<()> {
    if receipt.schema_version != receipt_schema() {
        return Err(AppError::new(
            "STREAMING_RECEIPT",
            "Unsupported streaming receipt version",
        ));
    }
    if !valid_image(&receipt.image) {
        return Err(AppError::new("STREAMING_IMAGE", "Invalid image identity"));
    }
    let current = dir.join("current.json");
    if current.exists() {
        atomic_write(&dir.join("previous.json"), &fs::read(&current)?)?;
    }
    atomic_write(&current, &serde_json::to_vec_pretty(receipt)?)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut file = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path)
        .map_err(|error| AppError::from(error.error))?;
    fs::File::open(path.parent().unwrap())?.sync_all()?;
    Ok(())
}

fn probe_image(
    image: &str,
    output: &Path,
    cancel: &AtomicBool,
    nvidia: bool,
    render_node: Option<&Path>,
) -> Result<Value> {
    if !valid_image(image) {
        return Err(AppError::new("STREAMING_IMAGE", "Invalid image identity"));
    }
    let mut command = Command::new("podman");
    command.args([
        "run",
        "--rm",
        "--pull=never",
        "--network=none",
        "--cap-drop=all",
        "--security-opt=no-new-privileges",
    ]);
    if nvidia {
        command.args([
            "--device=nvidia.com/gpu=all",
            "--security-opt=label=disable",
        ]);
    }
    if let Some(node) = render_node {
        if node.parent() != Some(Path::new("/dev/dri"))
            || !node.file_name().and_then(|s| s.to_str()).is_some_and(|s| {
                s.strip_prefix("renderD")
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
            })
        {
            return Err(AppError::new(
                "STREAMING_DEVICE",
                "Choose a /dev/dri/renderD<number> render device",
            ));
        }
        command
            .arg("--device")
            .arg(node)
            .arg("--security-opt=label=disable");
    }
    command.args(["--entrypoint=python3", image, "/opt/marquee-wolf/probe.py"]);
    fs::write(output, [])?;
    run(&mut command, output, cancel, Duration::from_secs(180))?;
    let value: Value = serde_json::from_slice(&fs::read(output)?)?;
    if value["software_h264_encode_test"] != true
        || value["factories"]["waylanddisplaysrc"] != true
        || value["factories"]["opusenc"] != true
    {
        return Err(AppError::new(
            "STREAMING_PROBE",
            "Streaming image failed its compositor/audio/software-encoding baseline",
        ));
    }
    if nvidia && value["nvidia_h264_encode_test"] != true {
        return Err(AppError::new(
            "STREAMING_ENCODER",
            "The selected NVIDIA device failed its H.264 encoding test",
        )
        .with_context(value));
    }
    Ok(value)
}

pub fn probe(data: &Path, nvidia: bool, render_node: Option<&Path>) -> Result<Value> {
    let receipt = status(data)?.ok_or_else(|| {
        AppError::new("STREAMING_MISSING", "Build the managed Wolf backend first")
    })?;
    let output = tempfile::NamedTempFile::new_in(root(data))?;
    probe_image(
        &receipt.image,
        output.path(),
        &AtomicBool::new(false),
        nvidia,
        render_node,
    )
}

/// Select the previous image for subsequent starts, after rerunning its baseline.
/// Does not interrupt external Wolf processes or modify runtime/pairing state.
pub fn rollback(data: &Path) -> Result<Receipt> {
    let dir = root(data);
    if status(data)?.is_none() {
        return Err(AppError::new(
            "STREAMING_MISSING",
            "No managed Wolf backend is installed",
        ));
    }
    let _lock = InstanceLock::acquire(&dir.join("build.lock"))?;
    let receipt = read_receipt(&dir.join("previous.json"))?.ok_or_else(|| {
        AppError::new(
            "STREAMING_ROLLBACK",
            "No previous streaming backend is recorded",
        )
    })?;
    let output = tempfile::NamedTempFile::new_in(&dir)?;
    probe_image(
        &receipt.image,
        output.path(),
        &AtomicBool::new(false),
        false,
        None,
    )?;
    promote(&dir, &receipt)?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(image: &str) -> Receipt {
        Receipt {
            schema_version: receipt_schema(),
            image: image.into(),
            wolf_revision: super::super::wolf::REVIEWED_REVISION.into(),
            compositor_revision: "test".into(),
            recipe_sha256: recipe_hash(),
            built_at: "test".into(),
            baseline_probe: serde_json::json!({}),
            license_inventory: "test".into(),
            live_streaming_verified: false,
        }
    }

    #[test]
    fn receipt_promotion_preserves_previous_image_and_rejects_invalid_candidates() {
        let data = tempfile::tempdir().unwrap();
        let dir = root(data.path());
        fs::create_dir_all(&dir).unwrap();
        let first = receipt(&format!("sha256:{}", "a".repeat(64)));
        promote(&dir, &first).unwrap();
        assert_eq!(status(data.path()).unwrap().unwrap().image, first.image);
        assert!(promote(&dir, &receipt("untrusted:latest")).is_err());
        let mut future = first.clone();
        future.schema_version = 99;
        assert!(promote(&dir, &future).is_err());
        assert_eq!(status(data.path()).unwrap().unwrap().image, first.image);
        let second = receipt(&format!("sha256:{}", "b".repeat(64)));
        promote(&dir, &second).unwrap();
        assert_eq!(
            serde_json::from_slice::<Receipt>(&fs::read(dir.join("previous.json")).unwrap())
                .unwrap()
                .image,
            first.image
        );
        assert_eq!(status(data.path()).unwrap().unwrap().image, second.image);
    }

    #[test]
    fn source_recipe_matches_the_control_contract_and_pins_base_images() {
        let lock: Value = serde_json::from_slice(FILES[1].1).unwrap();
        assert_eq!(lock["wolf"], super::super::wolf::REVIEWED_REVISION);
        for key in ["gstreamer_image", "rust_image"] {
            assert!(lock[key].as_str().unwrap().contains("@sha256:"));
        }
    }

    #[test]
    fn arbitrary_devices_and_oversized_receipts_are_rejected_without_starting_a_container() {
        let data = tempfile::tempdir().unwrap();
        let output = data.path().join("probe.json");
        let image = format!("sha256:{}", "a".repeat(64));
        for path in [
            "/dev/mem",
            "/dev/dri/../uinput",
            "/dev/dri/renderD;bad",
            "relative",
        ] {
            assert_eq!(
                probe_image(
                    &image,
                    &output,
                    &AtomicBool::new(false),
                    false,
                    Some(Path::new(path))
                )
                .unwrap_err()
                .code,
                "STREAMING_DEVICE"
            );
        }
        assert!(!output.exists());
        let dir = root(data.path());
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("current.json"), vec![b' '; 65537]).unwrap();
        assert_eq!(status(data.path()).unwrap_err().code, "STREAMING_RECEIPT");
    }
}
