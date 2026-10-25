use crate::error::Result;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const SECTOR: u64 = 2048;
const MAX_DIR: usize = 64 * 1024;
const MAX_CNF: usize = 8 * 1024;

/// A physical disc layout: how far apart logical 2048-byte blocks sit on disk,
/// and where the user data starts inside each raw sector.
///
/// `.iso` rips are 2048-byte user-data-only images. PS1 discs are usually ripped
/// raw at 2352 bytes per sector (12-byte sync + 4-byte header + data, plus an
/// 8-byte Mode 2 subheader). 2336-byte rips drop the sync. Marquee probes all of
/// them so a classic PS1 dump resolves without conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Probe {
    stride: u64,
    data_offset: u64,
}

const PROBES: [Probe; 4] = [
    Probe {
        stride: 2048,
        data_offset: 0,
    },
    Probe {
        stride: 2352,
        data_offset: 16,
    }, // Mode 1 raw
    Probe {
        stride: 2352,
        data_offset: 24,
    }, // Mode 2 Form 1 raw
    Probe {
        stride: 2336,
        data_offset: 12,
    }, // Mode 2 without sync
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscIdentity {
    pub serial: Option<String>,
    pub serial_raw: Option<String>,
    pub region: Option<String>,
}

/// An identity with nothing resolved.
pub fn unresolved() -> DiscIdentity {
    DiscIdentity {
        serial: None,
        serial_raw: None,
        region: None,
    }
}

/// Read a disc's `SYSTEM.CNF` regardless of rip layout. Works for both PS1
/// (`BOOT`) and PS2 (`BOOT2`) discs.
pub fn identify_disc_iso(path: &Path) -> Result<DiscIdentity> {
    if crate::xbox360::has_partition(path)? {
        return Err(crate::error::AppError::new("DISC_PLATFORM", "Xbox-family disc detected. Choose Xbox 360 explicitly for XEX2 discs; original Xbox needs xemu preparation."));
    }
    identify_disc_track(path, 0, None)
}

pub fn identify_disc_track(path: &Path, base: u64, stride: Option<u64>) -> Result<DiscIdentity> {
    let mut file = File::open(path)?;
    for probe in PROBES
        .into_iter()
        .filter(|p| stride.is_none_or(|s| p.stride == s))
    {
        if let Some(cnf) = read_system_cnf(&mut file, probe, base)? {
            return Ok(identity_from_system_cnf(&cnf));
        }
    }
    Ok(unresolved())
}

pub fn identify_ps2_iso(path: &Path) -> Result<DiscIdentity> {
    identify_disc_iso(path)
}

pub fn identity_from_system_cnf(text: &str) -> DiscIdentity {
    let raw = extract_boot_serial(text);
    let serial = raw.as_deref().and_then(normalize_serial);
    let region = serial
        .as_deref()
        .and_then(region_from_serial)
        .map(str::to_string);
    DiscIdentity {
        serial,
        serial_raw: raw,
        region,
    }
}

fn extract_boot_serial(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("BOOT2") && !key.trim().eq_ignore_ascii_case("BOOT") {
            continue;
        }
        let value = value.trim();
        if let Some(serial) = scan_serial_token(value) {
            return Some(serial);
        }
    }
    scan_serial_token(text)
}

fn scan_serial_token(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut i = 0;
    while i + 8 <= bytes.len() {
        if bytes[i].is_ascii_alphabetic()
            && bytes[i + 1].is_ascii_alphabetic()
            && bytes[i + 2].is_ascii_alphabetic()
            && bytes[i + 3].is_ascii_alphabetic()
        {
            let rest = &value[i..];
            let token: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
                .collect();
            if normalize_serial(&token).is_some() {
                return Some(token);
            }
        }
        i += 1;
    }
    None
}

pub fn normalize_serial(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if cleaned.len() < 9 {
        return None;
    }
    let prefix = &cleaned[..4];
    if !prefix.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let digits = &cleaned[4..];
    if digits.len() < 5 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let (body, tail) = digits.split_at(digits.len() - 2);
    Some(format!("{prefix}_{body}.{tail}"))
}

/// PCSX2 / xlenore cover filename: `SLUS_209.63` → `SLUS-20963`.
pub fn cover_serial(normalized: &str) -> Option<String> {
    let cleaned: String = normalized
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if cleaned.len() < 9 || !cleaned[..4].chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    Some(format!("{}-{}", &cleaned[..4], &cleaned[4..]))
}

fn region_from_serial(serial: &str) -> Option<&'static str> {
    match serial.get(..4)? {
        "SLUS" | "SCUS" => Some("NTSC-U"),
        "SLES" | "SCES" => Some("PAL"),
        "SLPS" | "SCPS" | "SLPM" => Some("NTSC-J"),
        _ => None,
    }
}

/// Byte offset of logical block `block`'s user data for a given layout.
fn block_offset(probe: Probe, block: u64) -> u64 {
    block * probe.stride + probe.data_offset
}

fn read_system_cnf(file: &mut File, probe: Probe, base: u64) -> Result<Option<String>> {
    let mut pvd = [0u8; SECTOR as usize];
    file.seek(SeekFrom::Start(base + block_offset(probe, 16)))?;
    if file.read(&mut pvd)? < SECTOR as usize {
        return Ok(None);
    }
    if pvd[0] != 1 || &pvd[1..6] != b"CD001" {
        return Ok(None);
    }
    let block = u16::from_le_bytes(pvd[128..130].try_into().unwrap_or([0, 8])) as u64;
    let block = if block == 0 { SECTOR } else { block };
    let root = &pvd[156..190];
    if root[0] < 34 {
        return Ok(None);
    }
    let extent = u32::from_le_bytes(root[2..6].try_into().unwrap()) as u64;
    let size = u32::from_le_bytes(root[10..14].try_into().unwrap()) as usize;
    if extent == 0 || size == 0 || size > MAX_DIR {
        return Ok(None);
    }
    // `extent` counts `block`-byte logical blocks; normalise to 2048-byte units.
    let root_block = if block == SECTOR {
        extent
    } else {
        extent * block / SECTOR
    };
    let mut dir = vec![0u8; size];
    read_logical(file, probe, base, root_block, &mut dir)?;
    let Some((file_extent, file_size)) = find_cnf(&dir) else {
        return Ok(None);
    };
    if file_size == 0 || file_size as usize > MAX_CNF {
        return Ok(None);
    }
    let mut buf = vec![0u8; file_size as usize];
    read_logical(file, probe, base, u64::from(file_extent), &mut buf)?;
    Ok(Some(String::from_utf8_lossy(&buf).into_owned()))
}

fn read_logical(
    file: &mut File,
    probe: Probe,
    base: u64,
    block: u64,
    dest: &mut [u8],
) -> Result<()> {
    for (i, chunk) in dest.chunks_mut(SECTOR as usize).enumerate() {
        file.seek(SeekFrom::Start(
            base + block_offset(probe, block + i as u64),
        ))?;
        file.read_exact(chunk)?;
    }
    Ok(())
}

fn find_cnf(dir: &[u8]) -> Option<(u32, u32)> {
    let mut i = 0;
    while i < dir.len() {
        let len = dir[i] as usize;
        if len == 0 {
            i = (i / SECTOR as usize + 1) * SECTOR as usize;
            continue;
        }
        if i + len > dir.len() || len < 34 {
            break;
        }
        let rec = &dir[i..i + len];
        let ident_len = rec[32] as usize;
        if 33 + ident_len <= rec.len() {
            let ident = rec[33..33 + ident_len].to_ascii_uppercase();
            let name = ident.split(|b| *b == b';').next().unwrap_or(&ident);
            if name == b"SYSTEM.CNF" {
                let extent = u32::from_le_bytes(rec[2..6].try_into().ok()?);
                let size = u32::from_le_bytes(rec[10..14].try_into().ok()?);
                return Some((extent, size));
            }
        }
        i += len;
    }
    None
}

#[cfg(any(test, feature = "test-fixtures"))]
pub fn build_test_iso(files: &[(&str, &[u8])]) -> Vec<u8> {
    const N: usize = 32;
    let mut image = vec![0u8; N * SECTOR as usize];
    let mut next = 20u32;
    let mut records = Vec::new();
    records.extend(dir_record(19, 2048, 2, &[0]));
    records.extend(dir_record(19, 2048, 2, &[1]));
    for (name, data) in files {
        let extent = next;
        let start = extent as usize * SECTOR as usize;
        image[start..start + data.len()].copy_from_slice(data);
        let mut ident = name.to_ascii_uppercase().into_bytes();
        if !ident.contains(&b';') {
            ident.extend_from_slice(b";1");
        }
        records.extend(dir_record(extent, data.len() as u32, 0, &ident));
        next += 1 + (data.len() as u32 / SECTOR as u32);
    }
    let dir_start = 19 * SECTOR as usize;
    image[dir_start..dir_start + records.len()].copy_from_slice(&records);

    let pvd_at = (16 * SECTOR) as usize;
    let pvd = &mut image[pvd_at..pvd_at + SECTOR as usize];
    pvd[0] = 1;
    pvd[1..6].copy_from_slice(b"CD001");
    pvd[6] = 1;
    pvd[128..130].copy_from_slice(&2048u16.to_le_bytes());
    pvd[130..132].copy_from_slice(&2048u16.to_be_bytes());
    let vol = (N as u32).to_le_bytes();
    pvd[80..84].copy_from_slice(&vol);
    let root = dir_record(19, records.len() as u32, 2, &[0]);
    pvd[156..156 + root.len()].copy_from_slice(&root);
    image[17 * SECTOR as usize] = 255;
    image[17 * SECTOR as usize + 1..17 * SECTOR as usize + 6].copy_from_slice(b"CD001");
    image
}

#[cfg(any(test, feature = "test-fixtures"))]
fn dir_record(extent: u32, size: u32, flags: u8, name: &[u8]) -> Vec<u8> {
    let mut len = 33 + name.len();
    if len % 2 == 1 {
        len += 1;
    }
    let mut rec = vec![0u8; len];
    rec[0] = len as u8;
    rec[2..6].copy_from_slice(&extent.to_le_bytes());
    rec[6..10].copy_from_slice(&extent.to_be_bytes());
    rec[10..14].copy_from_slice(&size.to_le_bytes());
    rec[14..18].copy_from_slice(&size.to_be_bytes());
    rec[25] = flags;
    rec[28..30].copy_from_slice(&1u16.to_le_bytes());
    rec[30..32].copy_from_slice(&1u16.to_be_bytes());
    rec[32] = name.len() as u8;
    rec[33..33 + name.len()].copy_from_slice(name);
    rec
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_boot2_serial_variants() {
        let id = identity_from_system_cnf("BOOT2 = cdrom0:\\SLUS_209.63;1\nVER = 1.00\n");
        assert_eq!(id.serial.as_deref(), Some("SLUS_209.63"));
        assert_eq!(id.region.as_deref(), Some("NTSC-U"));
        let id = identity_from_system_cnf("BOOT2=cdrom0:\\SLES-12345;1");
        assert_eq!(id.serial.as_deref(), Some("SLES_123.45"));
        assert_eq!(identity_from_system_cnf("nope").serial, None);
        assert_eq!(cover_serial("SLUS_209.63").as_deref(), Some("SLUS-20963"));
        assert_eq!(cover_serial("SLES_123.45").as_deref(), Some("SLES-12345"));
    }

    #[test]
    fn reads_system_cnf_from_tiny_iso() {
        let bytes = build_test_iso(&[(
            "SYSTEM.CNF",
            b"BOOT2 = cdrom0:\\SCES_503.82;1\nVER = 1.00\nVMODE = PAL\n",
        )]);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("GAME.ISO");
        File::create(&path).unwrap().write_all(&bytes).unwrap();
        let id = identify_ps2_iso(&path).unwrap();
        assert_eq!(id.serial.as_deref(), Some("SCES_503.82"));
        assert_eq!(id.region.as_deref(), Some("PAL"));
    }

    /// Re-lay a 2048-byte `.iso` as a raw 2352-byte-sector Mode 1 disc, the way
    /// a classic PS1 dump is stored. Sync pattern + header + the user data.
    fn raw_2352(bytes: &[u8]) -> Vec<u8> {
        const RAW: usize = 2352;
        const DATA: usize = 2048;
        assert_eq!(bytes.len() % DATA, 0);
        let sectors = bytes.len() / DATA;
        let mut out = Vec::with_capacity(sectors * RAW);
        for i in 0..sectors {
            let mut sector = vec![0u8; RAW];
            sector[0] = 0x00;
            sector[1..11].copy_from_slice(&[0xFF; 10]);
            sector[11] = 0x00;
            // minute / second / sector (BCD), Mode 1.
            sector[15] = 0x01;
            // Mode 1: 16 bytes of sync/header, then 2048 bytes of user data,
            // then 288 bytes of EDC/ECC that the reader never looks at.
            sector[16..16 + DATA].copy_from_slice(&bytes[i * DATA..(i + 1) * DATA]);
            out.extend_from_slice(&sector);
        }
        out
    }

    #[test]
    fn reads_serial_from_raw_2352_ps1_disc() {
        let iso = build_test_iso(&[(
            "SYSTEM.CNF",
            b"BOOT = cdrom:\\SLUS_005.94;1\nTCB = 4\nEVENT = 10\nSTACK = 801FFF00\n",
        )]);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("PS1GAME.BIN");
        File::create(&path)
            .unwrap()
            .write_all(&raw_2352(&iso))
            .unwrap();
        let id = identify_disc_iso(&path).unwrap();
        assert_eq!(id.serial.as_deref(), Some("SLUS_005.94"));
        assert_eq!(id.region.as_deref(), Some("NTSC-U"));
    }

    #[test]
    fn raw_filesystem_reads_cross_sector_boundaries_without_headers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("multi.bin");
        let cnf = format!(
            "# comment\n{}\nBOOT = cdrom:\\SLUS_005.94;1\n",
            " ".repeat(2500)
        );
        let iso = build_test_iso(&[("SYSTEM.CNF", cnf.as_bytes())]);
        std::fs::write(&path, raw_2352(&iso)).unwrap();
        assert_eq!(
            identify_disc_iso(&path).unwrap().serial.as_deref(),
            Some("SLUS_005.94")
        );
    }
    #[test]
    fn malformed_and_missing_cnf_are_unresolved() {
        let dir = tempfile::tempdir().unwrap();
        let empty = dir.path().join("empty.iso");
        File::create(&empty)
            .unwrap()
            .write_all(&[0u8; 100])
            .unwrap();
        assert_eq!(identify_ps2_iso(&empty).unwrap().serial, None);
        let bytes = build_test_iso(&[("README.TXT", b"hello")]);
        let path = dir.path().join("nocnf.iso");
        File::create(&path).unwrap().write_all(&bytes).unwrap();
        assert_eq!(identify_ps2_iso(&path).unwrap().serial, None);
    }
}
