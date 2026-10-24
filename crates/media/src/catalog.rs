use crate::error::{AppError, Result};
use serde::Deserialize;

pub const PS2_JSON: &str = include_str!("../../../catalog/systems/ps2.json");
pub const PS1_JSON: &str = include_str!("../../../catalog/systems/ps1.json");

/// Cartridge and disc systems that run through a catalogue emulator. Adding a
/// system is one JSON file here plus the emulator it names.
const GENERIC_JSON: &[&str] = &[
    include_str!("../../../catalog/systems/genesis.json"),
    include_str!("../../../catalog/systems/ds.json"),
    include_str!("../../../catalog/systems/neogeo.json"),
    include_str!("../../../catalog/systems/dreamcast.json"),
    include_str!("../../../catalog/systems/nes.json"),
    include_str!("../../../catalog/systems/snes.json"),
    include_str!("../../../catalog/systems/gb.json"),
    include_str!("../../../catalog/systems/gbc.json"),
    include_str!("../../../catalog/systems/pce.json"),
    include_str!("../../../catalog/systems/n64.json"),
];

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SystemDescriptor {
    pub id: String,
    pub display_name: String,
    pub supported_extensions: Vec<String>,
    pub skipped_extensions: Vec<String>,
    pub runner_kind: String,
    /// Cover URL template with a `${serial}` placeholder, if the system has one.
    #[serde(default)]
    pub covers: Option<String>,
    /// Catalogue emulator that runs this system (`catalog/emulators/<id>.json`),
    /// for systems with no dedicated runner kind.
    #[serde(default)]
    pub emulator: Option<String>,
    /// Reviewed alternative engines; `emulator` remains the default.
    #[serde(default)]
    pub alternative_emulators: Vec<String>,
}

impl SystemDescriptor {
    pub fn supports_extension(&self, ext: &str) -> bool {
        self.supported_extensions
            .iter()
            .any(|e| e.eq_ignore_ascii_case(ext))
    }

    pub fn skips_extension(&self, ext: &str) -> bool {
        self.skipped_extensions
            .iter()
            .any(|e| e.eq_ignore_ascii_case(ext))
    }
}

pub fn ps2() -> Result<SystemDescriptor> {
    load(PS2_JSON)
}

pub fn ps1() -> Result<SystemDescriptor> {
    load(PS1_JSON)
}

/// Every system Marquee ships a descriptor for.
pub fn all() -> Result<Vec<SystemDescriptor>> {
    let mut all = vec![ps1()?, ps2()?];
    for json in GENERIC_JSON {
        all.push(load(json)?);
    }
    Ok(all)
}

/// Look up a descriptor by system id (`ps2`, `ps1`). Unknown ids are a caller error.
pub fn by_id(system: &str) -> Result<SystemDescriptor> {
    all()?
        .into_iter()
        .find(|d| d.id == system)
        .ok_or_else(|| AppError::new("ROOT_OFFLINE", format!("unknown system: {system}")))
}

pub fn load(json: &str) -> Result<SystemDescriptor> {
    let value: serde_json::Value = serde_json::from_str(json)?;
    if value.get("command").is_some() || value.get("argv").is_some() {
        return Err(AppError::new(
            "JSON",
            "system catalogue must not include command/argv fields",
        ));
    }
    Ok(serde_json::from_value(value)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ps2_descriptor_is_safe() {
        let desc = ps2().unwrap();
        assert_eq!(desc.id, "ps2");
        assert_eq!(desc.runner_kind, "pcsx2");
        assert!(desc.supports_extension("iso"));
        assert!(desc.skips_extension("chd"));
    }

    #[test]
    fn ps1_descriptor_is_safe() {
        let desc = ps1().unwrap();
        assert_eq!(desc.id, "ps1");
        assert_eq!(desc.runner_kind, "duckstation");
        assert!(desc.supports_extension("iso"));
        assert!(desc.supports_extension("chd"));
        assert!(desc.supports_extension("cue"));
        assert!(desc.skips_extension("bin"));
        assert!(desc.covers.as_deref().unwrap().contains("psx-covers"));
    }

    #[test]
    fn descriptors_reject_command_fields() {
        assert!(load(r#"{"id":"x","display_name":"X","supported_extensions":[],"skipped_extensions":[],"runner_kind":"k","command":["rm"]}"#).is_err());
    }

    #[test]
    fn by_id_unknown_is_error() {
        assert_eq!(by_id("atari-jaguar").unwrap_err().code, "ROOT_OFFLINE");
        assert_eq!(by_id("ps2").unwrap().display_name, "PlayStation 2");
    }
}
