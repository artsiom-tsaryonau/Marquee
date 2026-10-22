use marquee_manager::{AppError, Result};
use std::path::PathBuf;
fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--data-dir")) {
        return Err(AppError::new(
            "MANAGER_ARGS",
            "usage: marquee-manager --data-dir ABSOLUTE_DATA_DIRECTORY",
        ));
    }
    let data = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| AppError::new("MANAGER_ARGS", "Missing data directory"))?;
    if !data.is_absolute() || args.next().is_some() {
        return Err(AppError::new(
            "MANAGER_ARGS",
            "Use one absolute data directory",
        ));
    }
    marquee_manager::daemon::run(&data)
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
