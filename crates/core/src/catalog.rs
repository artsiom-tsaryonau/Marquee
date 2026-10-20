use crate::error::{AppError, Result};
use serde::Deserialize;

pub const PS2_JSON: &str = include_str!("../../../catalog/systems/ps2.json");

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SystemDescriptor {
    pub id: String,
    pub display_name: String,
    pub supported_extensions: Vec<String>,
    pub skipped_extensions: Vec<String>,
    pub runner_kind: String,
}

pub fn ps2() -> Result<SystemDescriptor> {
    load(PS2_JSON)
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
        assert!(desc.supported_extensions.contains(&"iso".into()));
        assert!(desc.skipped_extensions.contains(&"chd".into()));
    }
}
