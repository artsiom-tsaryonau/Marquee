//! Install PS3 packages (games, updates, add-ons) into Marquee's own RPCS3.
//!
//! RPCS3 does the installing. Marquee checks the licence the package needs
//! first, and afterwards that files really arrived: RPCS3 can exit cleanly
//! without installing anything.

use crate::{AppError, Core, Result};
use serde::Serialize;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PackageHeader {
    pub content_id: String,
    /// Folder under `dev_hdd0/game` the package installs into.
    pub install_dir: String,
    /// True when the content only runs with its `.rap` licence file.
    pub needs_license: bool,
}

#[derive(Debug, Serialize)]
pub struct Installed {
    pub content_id: String,
    pub destination: PathBuf,
    pub files_written: u64,
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::new("PS3_PACKAGE", message.into())
}

fn plain_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn read_header(path: &Path) -> Result<PackageHeader> {
    let mut file = fs::File::open(path)?;
    let mut head = [0u8; 0xC0];
    file.read_exact(&mut head)
        .map_err(|_| invalid("Not a PS3 package"))?;
    // PS3 and PSP packages share the magic; type 1 is PS3.
    if &head[..4] != b"\x7fPKG" || head[6..8] != [0, 1] {
        return Err(invalid("Not a PS3 package"));
    }
    let text = String::from_utf8_lossy(&head[0x30..0x54]);
    let content_id = text.trim_end_matches('\0').to_owned();
    if !plain_name(&content_id) || content_id.len() < 16 {
        return Err(invalid("Package has no usable content ID"));
    }
    let number = |at: usize| u32::from_be_bytes(head[at..at + 4].try_into().unwrap());
    let (offset, count) = (number(8), number(12));
    if count > 64 {
        return Err(invalid("Package metadata is not readable"));
    }
    file.seek(SeekFrom::Start(offset.into()))?;
    let (mut drm, mut install_dir) = (None, None);
    for _ in 0..count {
        let mut entry = [0u8; 8];
        file.read_exact(&mut entry)?;
        let id = u32::from_be_bytes(entry[..4].try_into().unwrap());
        let size = u32::from_be_bytes(entry[4..].try_into().unwrap()) as usize;
        if size > 4096 {
            return Err(invalid("Package metadata is not readable"));
        }
        let mut data = vec![0; size];
        file.read_exact(&mut data)?;
        match id {
            1 if size == 4 => drm = Some(u32::from_be_bytes(data.try_into().unwrap())),
            0xA => {
                let name = String::from_utf8_lossy(&data);
                let name = name.trim_matches('\0');
                if plain_name(name) {
                    install_dir = Some(name.to_owned());
                }
            }
            _ => {}
        }
    }
    Ok(PackageHeader {
        // The title ID sits at a fixed place in the content ID.
        install_dir: install_dir.unwrap_or_else(|| content_id[7..16].to_owned()),
        // 1 is a network licence and 2 a local one; 3 is free content.
        needs_license: matches!(drm, Some(1 | 2)),
        content_id,
    })
}

/// Make sure the licence `header` needs is in RPCS3's user folder, copying the
/// owner's file there if one was given. Never replaces an installed licence.
fn place_license(exdata: &Path, header: &PackageHeader, rap: &[PathBuf]) -> Result<()> {
    let name = format!("{}.rap", header.content_id);
    let target = exdata.join(&name);
    if !header.needs_license || target.is_file() {
        return Ok(());
    }
    let source = rap
        .iter()
        .find(|p| p.file_name().is_some_and(|n| n == name.as_str()))
        .ok_or_else(|| {
            invalid(format!(
                "This package needs its licence. Pass your own {name} with --rap"
            ))
        })?;
    let bytes = fs::read(source)?;
    if bytes.len() != 16 {
        return Err(invalid(format!("{name} is not a 16-byte licence file")));
    }
    fs::create_dir_all(exdata)?;
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(target)?;
    std::io::Write::write_all(&mut out, &bytes)?;
    Ok(())
}

/// Files below `dir` whose contents or metadata changed at or after `since`.
fn changed_since(dir: &Path, since: i64) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.metadata() {
            Ok(meta) if meta.is_dir() => changed_since(&entry.path(), since),
            Ok(meta) => u64::from(meta.ctime() >= since),
            Err(_) => 0,
        })
        .sum()
}

fn running_from(dir: &Path) -> bool {
    let Ok(processes) = fs::read_dir("/proc") else {
        return false;
    };
    processes.flatten().any(|process| {
        fs::read(process.path().join("cmdline")).is_ok_and(|raw| {
            let first = raw.split(|b| *b == 0).next().unwrap_or_default();
            String::from_utf8_lossy(first).starts_with(&*dir.to_string_lossy())
        })
    })
}

impl Core {
    /// Install packages in the order given (a game before its update).
    pub fn install_ps3_packages(
        &self,
        packages: &[PathBuf],
        rap: &[PathBuf],
    ) -> Result<Vec<Installed>> {
        self.require_ps3_setup_idle("rpcs3")?;
        let program = crate::emulators::program_dir(&self.paths, "rpcs3");
        let binary = program.join("AppRun");
        if !binary.is_file() {
            return Err(invalid("RPCS3 is not installed"));
        }
        if running_from(&program) {
            return Err(invalid("Close RPCS3 before installing packages"));
        }
        let home = self.paths.data_dir.join("rpcs3");
        let hdd = home.join("xdg-config/rpcs3/dev_hdd0");
        // Check every package and licence before installing any.
        let mut checked = Vec::new();
        for package in packages {
            let package = fs::canonicalize(package)?;
            let header = read_header(&package)?;
            place_license(&hdd.join("home/00000001/exdata"), &header, rap)?;
            checked.push((package, header));
        }
        let mut done = Vec::new();
        for (package, header) in checked {
            let started = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs() as i64 - 1);
            let argv: Vec<String> = vec![
                binary.to_string_lossy().into_owned(),
                "--headless".into(),
                "--installpkg".into(),
                package.to_string_lossy().into_owned(),
            ];
            let argv = crate::sessions::with_launch_prefix(&argv);
            let mut command = Command::new(&argv[0]);
            command.args(&argv[1..]);
            for (variable, folder) in [
                ("XDG_CONFIG_HOME", "xdg-config"),
                ("XDG_DATA_HOME", "xdg-data"),
                ("XDG_CACHE_HOME", "xdg-cache"),
            ] {
                command.env(variable, home.join(folder));
            }
            command
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?;
            let destination = hdd.join("game").join(&header.install_dir);
            let files_written = changed_since(&destination, started);
            if files_written == 0 {
                return Err(invalid(format!(
                    "RPCS3 did not install {}; its log is under rpcs3/xdg-cache/rpcs3",
                    header.content_id
                )));
            }
            done.push(Installed {
                content_id: header.content_id,
                destination,
                files_written,
            });
        }
        Ok(done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(path: &Path, content_id: &str, drm: u32, kind: u8) {
        let mut bytes = vec![0u8; 0x100];
        bytes[..4].copy_from_slice(b"\x7fPKG");
        bytes[6..8].copy_from_slice(&[0, kind]);
        bytes[8..12].copy_from_slice(&0xC0u32.to_be_bytes());
        bytes[12..16].copy_from_slice(&1u32.to_be_bytes());
        bytes[0x30..0x30 + content_id.len()].copy_from_slice(content_id.as_bytes());
        bytes[0xC0..0xC4].copy_from_slice(&1u32.to_be_bytes());
        bytes[0xC4..0xC8].copy_from_slice(&4u32.to_be_bytes());
        bytes[0xC8..0xCC].copy_from_slice(&drm.to_be_bytes());
        fs::write(path, bytes).unwrap();
    }

    const ID: &str = "EP0000-NPEB00000_00-EXAMPLE000000001";

    #[test]
    fn headers_say_which_packages_need_a_licence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.pkg");
        for (drm, needs) in [(1, true), (2, true), (3, false)] {
            package(&path, ID, drm, 1);
            let header = read_header(&path).unwrap();
            assert_eq!(header.needs_license, needs);
            assert_eq!(header.install_dir, "NPEB00000");
        }
        package(&path, ID, 3, 2); // a PSP package
        assert!(read_header(&path).is_err());
        package(&path, "../../escape/aaaaaaaaaa", 3, 1);
        assert!(read_header(&path).is_err());
    }

    #[test]
    fn a_licence_must_match_and_is_never_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let exdata = dir.path().join("exdata");
        let header = PackageHeader {
            content_id: ID.into(),
            install_dir: "NPEB00000".into(),
            needs_license: true,
        };
        let wrong = dir.path().join("other.rap");
        fs::write(&wrong, [1u8; 16]).unwrap();
        assert!(place_license(&exdata, &header, &[wrong]).is_err());
        let right = dir.path().join(format!("{ID}.rap"));
        fs::write(&right, [2u8; 15]).unwrap();
        assert!(place_license(&exdata, &header, std::slice::from_ref(&right)).is_err());
        fs::write(&right, [2u8; 16]).unwrap();
        place_license(&exdata, &header, std::slice::from_ref(&right)).unwrap();
        fs::write(&right, [3u8; 16]).unwrap();
        place_license(&exdata, &header, std::slice::from_ref(&right)).unwrap();
        assert_eq!(fs::read(exdata.join(format!("{ID}.rap"))).unwrap(), [2; 16]);
    }
}
