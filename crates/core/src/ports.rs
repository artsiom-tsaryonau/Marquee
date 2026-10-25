//! Library-facing native-port catalogue and launch helpers. Installation belongs to the manager.
use crate::{paths::Paths, Result};
pub use marquee_manager::port_engine::{
    catalogue, find, latest_key, latest_release, pick_asset, setting_key, InstalledPort,
    PinnedRelease, PortDef, PortInfo,
};
use std::path::PathBuf;
pub fn port_dir(paths: &Paths, id: &str) -> PathBuf {
    marquee_manager::port_engine::port_dir(&paths.data_dir, id)
}
pub fn info(
    paths: &Paths,
    def: &PortDef,
    installed: Option<&InstalledPort>,
    latest: Option<&str>,
) -> PortInfo {
    marquee_manager::port_engine::info(&paths.data_dir, def, installed, latest)
}
pub fn launch(paths: &Paths, def: &PortDef, installed: &InstalledPort) -> Result<()> {
    marquee_manager::port_engine::launch(&paths.data_dir, def, installed)
}
pub fn open_folder(paths: &Paths, id: &str) -> Result<()> {
    marquee_manager::port_engine::open_folder(&paths.data_dir, id)
}

pub use marquee_manager::port_engine::{extract_tar_gz, extract_zip};
