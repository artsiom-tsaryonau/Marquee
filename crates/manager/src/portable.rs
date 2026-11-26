//! Portable package manifest (MP0). This is the contract only: nothing exports or imports yet.

use crate::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    pub platform: String,
    pub title_id: String,
    pub edition: String,
    pub role: String,
    pub files: Vec<PayloadFile>,
    pub recipe: String,
    pub recipe_version: String,
    pub output_receipt: crate::service::Receipt,
    pub runtime: RuntimeRef,
    /// Setting names that may travel with the package. Values stay on each machine.
    #[serde(default)]
    pub settings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeRef {
    pub id: String,
    pub version: String,
    pub os: String,
    pub cpu: String,
    pub launch_recipe_id: String,
}

impl Manifest {
    /// Identity a receiving library compares. It does not include machine paths.
    pub fn identity(&self) -> String {
        let mut files: Vec<_> = self
            .files
            .iter()
            .map(|file| format!("{}:{}:{}", file.path, file.size, file.sha256))
            .collect();
        files.sort();
        format!(
            "{}|{}|{}|{}|{}|{}",
            self.platform,
            self.title_id,
            self.edition,
            self.role,
            self.runtime.launch_recipe_id,
            files.join(",")
        )
    }
}

pub fn parse(raw: &str) -> Result<Manifest> {
    let manifest: Manifest = serde_json::from_str(raw)?;
    if manifest.schema != 1 {
        return Err(AppError::new(
            "PORTABLE",
            format!(
                "portable package schema {} is not supported",
                manifest.schema
            ),
        ));
    }
    if !matches!(manifest.role.as_str(), "base" | "update" | "dlc") {
        return Err(AppError::new(
            "PORTABLE",
            "package role must be base, update or dlc",
        ));
    }
    if manifest.files.is_empty() {
        return Err(AppError::new("PORTABLE", "package has no payload files"));
    }
    for file in &manifest.files {
        check_relative(&file.path)?;
        if file.sha256.len() != 64 || !file.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(AppError::new(
                "PORTABLE",
                format!("{} has no 64-character sha256", file.path),
            ));
        }
    }
    for file in &manifest.output_receipt.files {
        check_relative(&file.path.to_string_lossy())?;
    }
    Ok(manifest)
}

fn check_relative(path: &str) -> Result<()> {
    let value = Path::new(path);
    if path.is_empty()
        || value.is_absolute()
        || value
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(AppError::new(
            "PORTABLE",
            format!("payload path must stay inside the package: {path}"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixture_round_trips_and_rejects_paths_that_leave_the_package() {
        let raw = include_str!("../tests/fixtures/portable-v1.json");
        let manifest = parse(raw).unwrap();
        let again = parse(&serde_json::to_string(&manifest).unwrap()).unwrap();
        assert_eq!(manifest.identity(), again.identity());
        assert!(!manifest.identity().contains("destination"));
        let moved = {
            let mut copy = manifest.clone();
            copy.output_receipt.destination = "/var/lib/other-machine".into();
            copy
        };
        assert_eq!(manifest.identity(), moved.identity());
        for bad in ["../escape.bin", "/tmp/game.bin", "disk/../../x.bin", ""] {
            let mut copy = manifest.clone();
            copy.files[0].path = bad.into();
            assert!(
                parse(&serde_json::to_string(&copy).unwrap()).is_err(),
                "{bad}"
            );
        }
        let mut future = manifest.clone();
        future.schema = 2;
        assert!(parse(&serde_json::to_string(&future).unwrap()).is_err());
    }
}
