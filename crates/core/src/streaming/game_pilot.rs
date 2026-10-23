//! Explicit ST2b image composition. No game data is copied into the image and
//! building never starts a backend, edits its catalogue or opens the library.
use crate::error::{AppError, Result};
use crate::instance::InstanceLock;
use crate::source_builds::{run, valid_image};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

const FILES: &[(&str, &[u8])] = &[
    (
        "Containerfile",
        include_bytes!("../../../../catalog/streaming/sonic/Containerfile"),
    ),
    (
        "game-entrypoint.py",
        include_bytes!("../../../../catalog/streaming/sonic/game-entrypoint.py"),
    ),
    (
        "game.py",
        include_bytes!("../../../../catalog/streaming/sonic/game.py"),
    ),
];

fn read_small(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(65537).read_to_end(&mut bytes)?;
    if bytes.len() > 65536 {
        return Err(AppError::new(
            "STREAMING_GAME_RECEIPT",
            "Emulator receipt is too large",
        ));
    }
    Ok(bytes)
}

fn emulator_image(program: &Path) -> Result<String> {
    let image = String::from_utf8(read_small(&program.join("image-id"))?)
        .map_err(|_| AppError::new("STREAMING_GAME_RECEIPT", "Invalid image receipt"))?;
    let image = image.trim();
    let receipt: Value = serde_json::from_slice(&read_small(&program.join("source-build.json"))?)?;
    if !valid_image(image)
        || receipt["image"] != image
        || receipt["repository"] != "https://github.com/libretro/blastem"
        || receipt["commit"] != "ab743653ed110eeb7b1fe64d00cc5f9279c2bfec"
        || receipt["recipe"] != "blastem-standalone-sdl-v1"
    {
        return Err(AppError::new(
            "STREAMING_GAME_RECEIPT",
            "Use the reviewed managed BlastEm source build",
        ));
    }
    Ok(image.into())
}

pub fn build(data: &Path, blastem_program: &Path, cancel: &AtomicBool) -> Result<Value> {
    let host = super::runtime::status(data)?.ok_or_else(|| {
        AppError::new("STREAMING_GAME_HOST", "Build the reviewed Wolf host first")
    })?;
    let emulator = emulator_image(blastem_program)?;
    let dir = data.join("streaming/sonic-pilot");
    fs::create_dir_all(&dir)?;
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    let _lock = InstanceLock::acquire(&dir.join("build.lock"))?;
    let work = tempfile::Builder::new()
        .prefix(".build-")
        .tempdir_in(&dir)?;
    let log = dir.join("build.log");
    fs::write(&log, [])?;
    let info_path = work.path().join("podman.json");
    run(
        Command::new("podman").args(["info", "--format=json"]),
        &info_path,
        cancel,
        Duration::from_secs(30),
    )?;
    let info: Value = serde_json::from_slice(&read_small(&info_path)?)?;
    if info
        .pointer("/host/security/rootless")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err(AppError::new(
            "STREAMING_ROOTLESS",
            "Configure rootless Podman first",
        ));
    }
    let mut hash = Sha256::new();
    for (name, bytes) in FILES {
        fs::write(work.path().join(name), bytes)?;
        hash.update(name);
        hash.update([0]);
        hash.update(bytes);
    }
    let image_file = work.path().join("image-id");
    run(
        Command::new("podman")
            .args([
                "build",
                "--pull=never",
                "--force-rm",
                "--layers",
                "--iidfile",
            ])
            .arg(&image_file)
            .arg("--build-arg")
            .arg(format!("WOLF_IMAGE={}", host.image))
            .arg("--build-arg")
            .arg(format!("BLASTEM_IMAGE={emulator}"))
            .arg(work.path()),
        &log,
        cancel,
        Duration::from_secs(900),
    )?;
    let image = String::from_utf8(read_small(&image_file)?)
        .map_err(|_| AppError::new("STREAMING_IMAGE", "Invalid game image identity"))?;
    let image = image.trim();
    if !valid_image(image) {
        return Err(AppError::new(
            "STREAMING_IMAGE",
            "Invalid game image identity",
        ));
    }
    run(
        Command::new("podman").args([
            "run",
            "--rm",
            "--pull=never",
            "--network=none",
            "--entrypoint",
            "/opt/blastem/blastem",
            image,
            "-v",
        ]),
        &log,
        cancel,
        Duration::from_secs(30),
    )?;
    let receipt = json!({"schema_version":1, "image":image, "wolf":host,
        "blastem_image":emulator, "recipe_sha256":format!("{:x}",hash.finalize()),
        "built_at":crate::db::now_rfc3339(), "streamed_game_verified":false,
        "scope":"Operator-driven Sonic acceptance image; no game data or host lifecycle automation"});
    let mut file = tempfile::NamedTempFile::new_in(&dir)?;
    file.write_all(&serde_json::to_vec_pretty(&receipt)?)?;
    file.as_file().sync_all()?;
    file.persist(dir.join("current.json"))
        .map_err(|e| AppError::from(e.error))?;
    fs::File::open(&dir)?.sync_all()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mismatched_emulator_image_and_unreviewed_source_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("image-id"),
            format!("sha256:{}", "a".repeat(64)),
        )
        .unwrap();
        fs::write(dir.path().join("source-build.json"), b"{}").unwrap();
        assert_eq!(
            emulator_image(dir.path()).unwrap_err().code,
            "STREAMING_GAME_RECEIPT"
        );
        fs::write(dir.path().join("source-build.json"), vec![b' '; 65537]).unwrap();
        assert_eq!(
            emulator_image(dir.path()).unwrap_err().code,
            "STREAMING_GAME_RECEIPT"
        );
    }
}
