//! Shared implementation; preserved public module path.
pub use marquee_media::catalog::*;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_generic_system_names_a_catalogue_emulator_that_lists_it() {
        for d in all().unwrap() {
            for id in &d.alternative_emulators {
                let alt = crate::emulators::find(id).unwrap();
                assert_eq!(alt.kind.as_deref(), Some("emulator"));
                assert!(alt.systems.contains(&d.id));
            }
            if let Some(emu) = d.emulator.as_deref() {
                let def = crate::emulators::find(emu).unwrap();
                assert!(def.systems.contains(&d.id), "{emu} does not list {}", d.id);
                assert_eq!(d.runner_kind, "emulator");
                assert!(!d.supported_extensions.is_empty());
            }
        }
    }
}
