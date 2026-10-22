use crate::error::{AppError, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Paths {
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
}

impl Paths {
    pub fn resolve(data_dir: Option<PathBuf>) -> Result<Self> {
        let mut paths = match data_dir {
            Some(root) => Self {
                config_dir: root.join("config"),
                cache_dir: root.join("cache"),
                data_dir: root,
            },
            None => {
                let home = std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .ok_or_else(|| AppError::new("PATH", "HOME is not set; pass --data-dir"))?;
                let data = env_dir("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share"));
                let config = env_dir("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config"));
                let cache = env_dir("XDG_CACHE_HOME").unwrap_or_else(|| home.join(".cache"));
                Self {
                    data_dir: data.join("marquee"),
                    config_dir: config.join("marquee"),
                    cache_dir: cache.join("marquee"),
                }
            }
        };
        fs::create_dir_all(&paths.data_dir)?;
        fs::create_dir_all(&paths.config_dir)?;
        fs::create_dir_all(&paths.cache_dir)?;
        // Runners use these as XDG paths and container mount sources. Relative
        // paths are not valid there, and aliases must share one library lease.
        paths.data_dir = fs::canonicalize(&paths.data_dir)?;
        paths.config_dir = fs::canonicalize(&paths.config_dir)?;
        paths.cache_dir = fs::canonicalize(&paths.cache_dir)?;
        Ok(paths)
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("library.db")
    }

    pub fn instance_lock(&self) -> PathBuf {
        self.data_dir.join("instance.lock")
    }

    /// Marquee-managed PCSX2 tree (downloaded AppImage + unpacked AppDir).
    /// Isolated `XDG_CONFIG_HOME` so PCSX2 writes under Marquee, not `~/.config/PCSX2`.
    pub fn pcsx2_xdg_config(&self) -> PathBuf {
        self.data_dir.join("pcsx2").join("xdg-config")
    }

    pub fn pcsx2_bios_dir(&self) -> PathBuf {
        self.pcsx2_xdg_config().join("PCSX2").join("bios")
    }
    /// Isolated `XDG_CONFIG_HOME` for Marquee's DuckStation. DuckStation keeps its
    /// whole user folder (settings, BIOS, memory cards, save states) under it.
    pub fn duckstation_xdg_config(&self) -> PathBuf {
        self.data_dir.join("duckstation").join("xdg-config")
    }

    pub fn duckstation_bios_dir(&self) -> PathBuf {
        self.duckstation_xdg_config()
            .join("duckstation")
            .join("bios")
    }
}

fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

pub fn path_bytes(path: &Path) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().to_vec()
    }
    #[cfg(not(unix))]
    {
        path.to_string_lossy().as_bytes().to_vec()
    }
}

pub fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        PathBuf::from(std::ffi::OsString::from_vec(bytes.to_vec()))
    }
    #[cfg(not(unix))]
    {
        PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relative_data_paths_become_absolute_before_runner_mounts_and_xdg_environment() {
        let temp = tempfile::tempdir_in(".").unwrap();
        let relative = temp
            .path()
            .strip_prefix(std::env::current_dir().unwrap())
            .unwrap()
            .join("library");
        assert!(!relative.is_absolute());
        let paths = Paths::resolve(Some(relative)).unwrap();
        assert!(paths.data_dir.is_absolute());
        assert!(paths.config_dir.is_absolute());
        assert!(paths.cache_dir.is_absolute());
        assert_eq!(paths.config_dir, paths.data_dir.join("config"));
        assert_eq!(paths.cache_dir, paths.data_dir.join("cache"));
        let reopened = Paths::resolve(Some(paths.data_dir.clone())).unwrap();
        assert_eq!(reopened.data_dir, paths.data_dir);
    }

    #[cfg(unix)]
    #[test]
    fn alias_paths_share_one_canonical_library_and_runner_tree() {
        let temp = tempfile::tempdir().unwrap();
        let real = temp.path().join("real");
        fs::create_dir(&real).unwrap();
        let alias = temp.path().join("alias");
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        let paths = Paths::resolve(Some(alias)).unwrap();
        assert_eq!(paths.data_dir, real);
        assert_eq!(paths.config_dir, real.join("config"));
        assert_eq!(paths.cache_dir, real.join("cache"));
    }
}
