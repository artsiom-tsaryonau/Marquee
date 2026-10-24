//! Manager-owned PS3 firmware setup. RPCS3 installs into a disposable profile;
//! only validated dev_flash is promoted. Existing firmware and user data stay put.
use crate::{AppError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MAX_PUP: u64 = 512 * 1024 * 1024;
const MAX_FLASH: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Status,
    Review {
        source: PathBuf,
        runner_version: String,
    },
    Start {
        review_token: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub source: PathBuf,
    pub version: String,
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub version: String,
    pub files: usize,
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    #[serde(default)]
    pub recipe: String,
    pub id: String,
    pub state: String,
    pub message: String,
    pub review_token: String,
    pub package: Package,
    pub runner_version: String,
    pub program_sha256: String,
    pub output: Option<Manifest>,
    pub exit_code: Option<i32>,
    pub log_path: Option<PathBuf>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub job: Option<Job>,
    pub installed_version: Option<String>,
    pub managed_receipt: bool,
}
fn error(code: &str, message: &str) -> AppError {
    AppError::new(code, message)
}
fn base(data: &Path) -> PathBuf {
    data.join("manager/ps3-firmware")
}
pub fn flash_path(data: &Path) -> PathBuf {
    data.join("rpcs3/xdg-config/rpcs3/dev_flash")
}
fn record(data: &Path) -> PathBuf {
    base(data).join("job.json")
}
fn read_json<T: for<'a> Deserialize<'a>>(path: &Path) -> Result<Option<T>> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
        Ok(meta) if !meta.is_file() || meta.file_type().is_symlink() => {
            return Err(error(
                "FIRMWARE_STATE",
                "Firmware checkpoint is not a regular file",
            ))
        }
        Ok(_) => {}
    }
    let file = open_regular(path)?;
    if file.metadata()?.len() > 1024 * 1024 {
        return Err(error(
            "FIRMWARE_STATE",
            "Firmware checkpoint exceeds its size limit",
        ));
    }
    Ok(Some(serde_json::from_reader(file)?))
}

fn safe_dirs(root: &Path, path: &Path) -> Result<()> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| error("FIRMWARE_PATH", "Path is outside managed storage"))?;
    let mut current = root.to_path_buf();
    for part in relative.components() {
        if !matches!(part, std::path::Component::Normal(_)) {
            return Err(error("FIRMWARE_PATH", "Invalid managed storage path"));
        }
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(error(
                    "FIRMWARE_PATH",
                    "Managed storage contains a link or non-directory",
                ))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current)?;
                fs::set_permissions(&current, fs::Permissions::from_mode(0o700))?;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
fn save<T: Serialize>(data: &Path, path: &Path, value: &T) -> Result<()> {
    safe_dirs(data, path.parent().unwrap())?;
    let mut tmp = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    tmp.as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    serde_json::to_writer(tmp.as_file_mut(), value)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| AppError::from(e.error))?;
    File::open(path.parent().unwrap())?.sync_all()?;
    Ok(())
}
// Preserve only the welcome visibility preference, never whole profiles.
// Firmware setup hides the informational welcome to avoid overlapping modal
// dialogs with --installfw. RPCS3 still owns firmware confirmation and validation.
const WELCOME_KEY: &str = "infoBoxEnabledWelcome";
fn gui_ini(profile: &Path) -> PathBuf {
    profile.join("rpcs3/GuiConfigs/CurrentSettings.ini")
}
fn read_ini(path: &Path) -> Result<String> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(e) => return Err(e.into()),
        Ok(_) => {}
    }
    let file = open_regular(path)?;
    if file.metadata()?.len() > 1024 * 1024 {
        return Err(error(
            "FIRMWARE_SETTINGS",
            "RPCS3 GUI settings exceed the size limit",
        ));
    }
    let mut text = String::new();
    file.take(1024 * 1024 + 1).read_to_string(&mut text)?;
    if text.len() > 1024 * 1024 {
        return Err(error(
            "FIRMWARE_SETTINGS",
            "RPCS3 GUI settings grew beyond the size limit",
        ));
    }
    Ok(text)
}
fn copy_welcome_opt_out(data: &Path, from_profile: &Path, to_profile: &Path) -> Result<()> {
    let source = read_ini(&gui_ini(from_profile))?;
    let mut in_main = false;
    let mut value = None;
    for line in source.lines().map(str::trim) {
        if line.starts_with('[') {
            in_main = line == "[main_window]";
        }
        if in_main {
            if let Some((key, v)) = line.split_once('=') {
                if key.trim() == WELCOME_KEY {
                    value = Some(v.trim());
                }
            }
        }
    }
    if value != Some("false") {
        return Ok(());
    }
    hide_welcome(data, to_profile)
}
fn hide_welcome(data: &Path, to_profile: &Path) -> Result<()> {
    let destination = gui_ini(to_profile);
    safe_dirs(data, destination.parent().unwrap())?;
    let original = read_ini(&destination)?;
    let mut output = Vec::new();
    let mut in_main = false;
    let mut found_main = false;
    for line in original.lines() {
        if line.trim().starts_with('[') {
            in_main = line.trim() == "[main_window]";
            if in_main {
                found_main = true;
                output.push(line.to_owned());
                output.push(format!("{WELCOME_KEY}=false"));
                continue;
            }
        }
        if in_main
            && line
                .split_once('=')
                .is_some_and(|(k, _)| k.trim() == WELCOME_KEY)
        {
            continue;
        }
        output.push(line.to_owned());
    }
    if !found_main {
        output.extend(["[main_window]".into(), format!("{WELCOME_KEY}=false")]);
    }
    let mut tmp = tempfile::NamedTempFile::new_in(destination.parent().unwrap())?;
    tmp.as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    writeln!(tmp, "{}", output.join("\n"))?;
    tmp.as_file().sync_all()?;
    tmp.persist(&destination)
        .map_err(|e| AppError::from(e.error))?;
    File::open(destination.parent().unwrap())?.sync_all()?;
    Ok(())
}
fn save_job(data: &Path, job: &Job) -> Result<()> {
    let id = uuid::Uuid::parse_str(&job.id)
        .map_err(|_| error("FIRMWARE_STATE", "Invalid firmware job identity"))?;
    save(
        data,
        &base(data).join("jobs").join(format!("{id}.json")),
        job,
    )?;
    save(data, &record(data), job)
}
fn ensure_fresh_destination(data: &Path) -> Result<()> {
    if data.join("rpcs3/xdg-config/rpcs3/vfs.yml").exists() {
        return Err(error("FIRMWARE_VFS", "Custom RPCS3 filesystem settings are preserved. This installer currently requires the default dev_flash location"));
    }
    let path = flash_path(data);
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
        Ok(meta)
            if meta.is_dir()
                && !meta.file_type().is_symlink()
                && fs::read_dir(path)?.next().is_none() =>
        {
            Ok(())
        }
        Ok(_) => Err(error(
            "FIRMWARE_EXISTS",
            "Existing firmware is retained. Automatic replacement is not supported yet",
        )),
    }
}
fn open_regular(path: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(error(
            "FIRMWARE_PATH",
            "Choose a regular file, not a link or device",
        ));
    }
    Ok(file)
}
fn hash_file(path: &Path, limit: u64) -> Result<String> {
    let mut file = open_regular(path)?;
    if file.metadata()?.len() > limit {
        return Err(error(
            "FIRMWARE_LIMIT",
            "File exceeds the firmware workflow size limit",
        ));
    }
    let mut hash = Sha256::new();
    let mut buf = [0; 65536];
    let mut size = 0;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        size += n as u64;
        if size > limit {
            return Err(error("FIRMWARE_LIMIT", "File grew beyond its size limit"));
        }
        hash.update(&buf[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn be64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_be_bytes(bytes[offset..offset + 8].try_into().unwrap())
}
/// Structural validation only. RPCS3 performs PUP content/HMAC verification.
pub fn inspect_package(source: &Path) -> Result<Package> {
    if !source.is_absolute() {
        return Err(error(
            "FIRMWARE_PATH",
            "Choose an absolute firmware path on the Marquee computer",
        ));
    }
    let mut file = open_regular(source)?;
    let size = file.metadata()?.len();
    if !(48..=MAX_PUP).contains(&size) {
        return Err(error("FIRMWARE_FORMAT", "Invalid PS3 firmware size"));
    }
    let mut header = [0; 48];
    file.read_exact(&mut header)?;
    if &header[..8] != b"SCEUF\0\0\0" {
        return Err(error(
            "FIRMWARE_FORMAT",
            "This is not a PS3 PUP update package",
        ));
    }
    let count = be64(&header, 24);
    let hlen = be64(&header, 32);
    let dlen = be64(&header, 40);
    if count == 0 || count > 512 || hlen < 48 + count * 64 || hlen > size || dlen != size - hlen {
        return Err(error(
            "FIRMWARE_FORMAT",
            "PS3 firmware header or declared size is invalid",
        ));
    }
    let mut version = None;
    let mut ids = BTreeSet::new();
    let mut database = false;
    for _ in 0..count {
        let mut entry = [0; 32];
        file.read_exact(&mut entry)?;
        let id = be64(&entry, 0);
        let offset = be64(&entry, 8);
        let len = be64(&entry, 16);
        if !ids.insert(id) || offset < hlen || offset > size || len > size - offset {
            return Err(error(
                "FIRMWARE_FORMAT",
                "PS3 firmware entry table is invalid",
            ));
        }
        if id == 0x100 {
            version = Some((offset, len));
        }
        if id == 0x300 && len > 0 {
            database = true;
        }
    }
    let (offset, len) =
        version.ok_or_else(|| error("FIRMWARE_FORMAT", "PS3 firmware has no version entry"))?;
    if len == 0 || len > 64 || !database {
        return Err(error(
            "FIRMWARE_FORMAT",
            "PS3 firmware lacks a valid version or installation database",
        ));
    }
    file.seek(SeekFrom::Start(offset))?;
    let mut text = vec![0; len as usize];
    file.read_exact(&mut text)?;
    let text = std::str::from_utf8(&text)
        .map_err(|_| error("FIRMWARE_FORMAT", "Invalid firmware version text"))?
        .trim_matches(['\0', '\n', '\r', ' ']);
    if normalized_version(text).is_none() {
        return Err(error("FIRMWARE_FORMAT", "Invalid PS3 firmware version"));
    }
    Ok(Package {
        source: source.to_path_buf(),
        version: text.into(),
        bytes: size,
        sha256: hash_file(source, MAX_PUP)?,
    })
}
fn normalized_version(text: &str) -> Option<String> {
    let (major, minor) = text.split_once('.')?;
    if major.is_empty()
        || minor.is_empty()
        || !major
            .bytes()
            .chain(minor.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    Some(format!(
        "{}.{}",
        major.trim_start_matches('0'),
        minor.trim_end_matches('0')
    ))
}
fn child_file(root: &Path, rel: &Path) -> Result<PathBuf> {
    let mut path = root.to_path_buf();
    for part in rel.components() {
        if !matches!(part, std::path::Component::Normal(_)) {
            return Err(error("FIRMWARE_PATH", "Invalid firmware member path"));
        }
        path.push(part);
        let meta = fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            return Err(error(
                "FIRMWARE_PATH",
                "Firmware paths cannot contain symbolic links",
            ));
        }
    }
    Ok(path)
}
pub fn installed_version(data: &Path) -> Result<Option<String>> {
    version_at(&flash_path(data))
}
fn version_at(flash: &Path) -> Result<Option<String>> {
    if !flash.exists() {
        return Ok(None);
    }
    if fs::symlink_metadata(flash)?.file_type().is_symlink() {
        return Err(error(
            "FIRMWARE_PATH",
            "Firmware directory is a symbolic link",
        ));
    }
    for name in [
        "vsh/etc/version.txt",
        "vsh/module/vsh.self",
        "sys/external/liblv2.sprx",
    ] {
        let path = match child_file(flash, Path::new(name)) {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };
        let mut file = open_regular(&path)?;
        if name.ends_with("version.txt") {
            if file.metadata()?.len() > 16384 {
                return Err(error(
                    "FIRMWARE_FORMAT",
                    "Installed firmware version file is oversized",
                ));
            }
        } else {
            let mut magic = [0; 4];
            if file.read_exact(&mut magic).is_err() || &magic != b"SCE\0" {
                return Ok(None);
            }
        }
    }
    let mut file = open_regular(&flash.join("vsh/etc/version.txt"))?;
    let mut text = String::new();
    (&mut file).take(16385).read_to_string(&mut text)?;
    Ok(text.lines().find_map(|line| {
        line.strip_prefix("release:")
            .and_then(|v| v.split(':').next())
            .and_then(normalized_version)
    }))
}
fn installer_reported_success(log: &Path, version: &str) -> Result<bool> {
    let mut file = open_regular(log)?;
    let needle = format!("Successfully installed PS3 firmware version {version}.");
    let mut tail = Vec::new();
    let mut buf = [0; 65536];
    let mut total = 0;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            return Ok(false);
        }
        total += n;
        if total > 64 * 1024 * 1024 {
            return Err(error(
                "FIRMWARE_LIMIT",
                "Installer log exceeds the verification limit",
            ));
        }
        tail.extend_from_slice(&buf[..n]);
        if tail.windows(needle.len()).any(|v| v == needle.as_bytes()) {
            return Ok(true);
        }
        let retain = needle.len().saturating_sub(1).min(tail.len());
        tail.drain(..tail.len() - retain);
    }
}
fn manifest(root: &Path) -> Result<Manifest> {
    let version = version_at(root)?.ok_or_else(|| {
        error(
            "FIRMWARE_INCOMPLETE",
            "RPCS3 did not produce the required firmware files and version",
        )
    })?;
    let mut stack = vec![root.to_path_buf()];
    let mut files = vec![];
    let mut directories = vec![];
    let mut bytes = 0_u64;
    let mut entries = 0;
    while let Some(dir) = stack.pop() {
        directories.push(dir.clone());
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            entries += 1;
            if entries > 20_000 {
                return Err(error("FIRMWARE_LIMIT", "Firmware has too many entries"));
            }
            let meta = entry.file_type()?;
            if meta.is_symlink() {
                return Err(error(
                    "FIRMWARE_PATH",
                    "RPCS3 produced linked firmware content",
                ));
            }
            if meta.is_dir() {
                stack.push(entry.path());
            } else if meta.is_file() {
                bytes = bytes.saturating_add(entry.metadata()?.len());
                if bytes > MAX_FLASH {
                    return Err(error("FIRMWARE_LIMIT", "Firmware output exceeds 2 GiB"));
                }
                files.push(entry.path());
            } else {
                return Err(error(
                    "FIRMWARE_PATH",
                    "RPCS3 produced unsupported firmware content",
                ));
            }
        }
    }
    files.sort();
    let mut hash = Sha256::new();
    for file in &files {
        let rel = file
            .strip_prefix(root)
            .unwrap()
            .to_str()
            .ok_or_else(|| error("FIRMWARE_PATH", "Invalid firmware filename encoding"))?;
        hash.update((rel.len() as u64).to_le_bytes());
        hash.update(rel.as_bytes());
        hash.update(hash_file(file, MAX_FLASH)?.as_bytes());
        open_regular(file)?.sync_all()?;
    }
    for dir in directories.iter().rev() {
        File::open(dir)?.sync_all()?;
    }
    Ok(Manifest {
        version,
        files: files.len(),
        bytes,
        sha256: format!("{:x}", hash.finalize()),
    })
}
fn program_hash(data: &Path) -> Result<String> {
    let root = data.join("runners/rpcs3/current");
    let mut hash = Sha256::new();
    for name in ["AppRun", "usr/bin/rpcs3"] {
        let path = child_file(&root, Path::new(name))?;
        hash.update(hash_file(&path, MAX_PUP)?.as_bytes());
    }
    if root.join("portable").exists() || root.join("config").exists() {
        return Err(error(
            "FIRMWARE_PATH",
            "Portable RPCS3 layouts are not supported by this setup recipe",
        ));
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub fn read_status(data: &Path) -> Result<Status> {
    let job: Option<Job> = read_json(&record(data))?;
    let version = installed_version(data)?;
    let receipt: Option<Job> = read_json(&base(data).join("receipt.json"))?;
    let managed_receipt = version.as_ref().is_some_and(|v| {
        receipt
            .as_ref()
            .and_then(|r| r.output.as_ref())
            .is_some_and(|m| &m.version == v)
    });
    Ok(Status {
        job,
        installed_version: version,
        managed_receipt,
    })
}
pub fn active(data: &Path) -> Result<bool> {
    Ok(read_status(data)?
        .job
        .is_some_and(|j| matches!(j.state.as_str(), "running" | "promoting")))
}
pub struct Host {
    data: PathBuf,
    gate: Mutex<()>,
}
impl Host {
    pub fn new(data: &Path) -> Result<Self> {
        let host = Self {
            data: data.to_path_buf(),
            gate: Mutex::new(()),
        };
        if let Err(e) = host.recover() {
            eprintln!("PS3 firmware recovery: {}", e.message);
        }
        Ok(host)
    }
    fn recover(&self) -> Result<()> {
        if let Some(mut job) = read_status(&self.data)?.job {
            if matches!(job.state.as_str(), "running" | "promoting") {
                if job.state == "promoting"
                    && job.output.as_ref().is_some_and(|m| {
                        manifest(&flash_path(&self.data)).is_ok_and(|actual| &actual == m)
                    })
                {
                    job.state = "installed".into();
                    job.message = "Recovered verified firmware promotion".into();
                    save(&self.data, &base(&self.data).join("receipt.json"), &job)?;
                } else {
                    job.state = "interrupted".into();
                    job.message = "Setup was interrupted. Existing firmware was not replaced; inspect and review before retrying".into();
                }
                save_job(&self.data, &job)?;
            }
        }
        Ok(())
    }
    pub fn dispatch(&self, request: Request) -> Result<Status> {
        if matches!(request, Request::Status) {
            if let Ok(_guard) = self.gate.try_lock() {
                self.recover()?;
            }
            return read_status(&self.data);
        }
        let _lock = self
            .gate
            .try_lock()
            .map_err(|_| error("FIRMWARE_BUSY", "Firmware setup is already running"))?;
        match request {
            Request::Status => unreachable!(),
            Request::Review {
                source,
                runner_version,
            } => {
                if runner_version.is_empty() || runner_version.len() > 200 {
                    return Err(error("FIRMWARE_RUNNER", "RPCS3 build identity is missing"));
                }
                ensure_fresh_destination(&self.data)?;
                let job = Job {
                    recipe: "rpcs3-firmware-gui-v2".into(),
                    id: uuid::Uuid::new_v4().to_string(),
                    state: "reviewed".into(),
                    message:
                        "Package structure checked; RPCS3 verifies its contents during installation"
                            .into(),
                    review_token: uuid::Uuid::new_v4().to_string(),
                    package: inspect_package(&source)?,
                    runner_version,
                    program_sha256: program_hash(&self.data)?,
                    output: None,
                    exit_code: None,
                    log_path: None,
                };
                save_job(&self.data, &job)?;
            }
            Request::Start { review_token } => {
                let mut job: Job = read_json(&record(&self.data))?
                    .ok_or_else(|| error("FIRMWARE_REVIEW", "Review a firmware file first"))?;
                if job.recipe != "rpcs3-firmware-gui-v2" {
                    return Err(error(
                        "FIRMWARE_REVIEW",
                        "Firmware recipe changed; review again",
                    ));
                }
                if review_token != job.review_token {
                    return Err(error(
                        "FIRMWARE_REVIEW",
                        "Firmware review changed; review the file again",
                    ));
                }
                if job.state == "installed" {
                    return read_status(&self.data);
                }
                if job.state != "reviewed" {
                    return Err(error(
                        "FIRMWARE_REVIEW",
                        "Review the firmware again before retrying",
                    ));
                }
                job.state = "running".into();
                job.message = "Preparing isolated RPCS3 firmware installer".into();
                save_job(&self.data, &job)?;
                if let Err(e) = self.install(&mut job) {
                    if !(job.state == "promoting"
                        && job.output.as_ref().is_some_and(|m| {
                            manifest(&flash_path(&self.data)).is_ok_and(|actual| &actual == m)
                        }))
                    {
                        job.state = "failed".into();
                    }
                    job.message = e.message.clone();
                    save_job(&self.data, &job)?;
                    return Err(e);
                }
            }
        }
        read_status(&self.data)
    }
    fn install(&self, job: &mut Job) -> Result<()> {
        let data = &self.data;
        let id = uuid::Uuid::parse_str(&job.id)
            .map_err(|_| error("FIRMWARE_STATE", "Invalid firmware job identity"))?;
        ensure_fresh_destination(data)?;
        if program_hash(data)? != job.program_sha256 {
            return Err(error(
                "FIRMWARE_CHANGED",
                "RPCS3 changed after review; review firmware again",
            ));
        }
        let work = base(data).join("work").join(id.to_string());
        safe_dirs(data, &work)?;
        if marquee_media::archive::free_bytes(&work).is_none_or(|n| n < MAX_FLASH + MAX_PUP) {
            return Err(error(
                "FIRMWARE_SPACE",
                "Firmware setup needs at least 2.5 GiB of free space",
            ));
        }
        let snapshot = work.join("PS3UPDAT.PUP");
        let mut source = open_regular(&job.package.source)?;
        let mut out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&snapshot)?;
        let copied = std::io::copy(&mut (&mut source).take(MAX_PUP + 1), &mut out)?;
        out.sync_all()?;
        File::open(&work)?.sync_all()?;
        if copied != job.package.bytes || hash_file(&snapshot, MAX_PUP)? != job.package.sha256 {
            return Err(error(
                "FIRMWARE_CHANGED",
                "Firmware changed after review; the original was not installed",
            ));
        }
        for dir in ["home", "config/rpcs3/dev_flash", "data", "cache"] {
            safe_dirs(data, &work.join(dir))?;
        }
        let main_profile = data.join("rpcs3/xdg-config");
        hide_welcome(data, &work.join("config"))?;
        let log_path = work.join("cache/rpcs3/RPCS3.log");
        job.log_path = Some(log_path);
        job.message = "Finish the firmware prompts in RPCS3, then close it normally. Setup continues if Marquee closes".into();
        save_job(data, job)?;
        let mut command = Command::new(data.join("runners/rpcs3/current/AppRun"));
        command
            .args(["--installfw"])
            .arg(&snapshot)
            .env("HOME", work.join("home"))
            .env("XDG_CONFIG_HOME", work.join("config"))
            .env("XDG_DATA_HOME", work.join("data"))
            .env("XDG_CACHE_HOME", work.join("cache"))
            .env_remove("RPCS3_CONFIG_DIR")
            .env_remove("APPIMAGE")
            .env_remove("APPDIR")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let owner = std::process::id();
        // SAFETY: only async-signal-safe syscalls run between fork and exec.
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0
                    || libc::getppid() as u32 != owner
                {
                    return Err(std::io::Error::other(
                        "Firmware installer lost its manager owner",
                    ));
                }
                Ok(())
            });
        }
        let mut child = command.spawn()?;
        let until = Instant::now() + Duration::from_secs(1200);
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if Instant::now() >= until {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error(
                    "FIRMWARE_TIMEOUT",
                    "Firmware installer timed out; existing firmware was retained",
                ));
            }
            std::thread::sleep(Duration::from_millis(200));
        };
        job.exit_code = status.code();
        // A preference write must never turn a valid firmware installation into
        // a failed publication. Failed copies simply leave the welcome visible.
        if let Err(err) = copy_welcome_opt_out(data, &work.join("config"), &main_profile) {
            eprintln!(
                "PS3 welcome preference retained in staging: {}",
                err.message
            );
        }
        if !status.success() {
            return Err(error(
                "FIRMWARE_INSTALL",
                "RPCS3 did not exit cleanly; firmware output was not promoted",
            ));
        }
        if !installer_reported_success(job.log_path.as_ref().unwrap(), &job.package.version)? {
            return Err(error(
                "FIRMWARE_INSTALL",
                "RPCS3 did not report a completed firmware installation; no output was promoted",
            ));
        }
        let staged_flash = work.join("config/rpcs3/dev_flash");
        let output = manifest(&staged_flash)?;
        if Some(output.version.clone()) != normalized_version(&job.package.version) {
            return Err(error(
                "FIRMWARE_VERSION",
                "Installed firmware version differs from the reviewed package",
            ));
        }
        job.output = Some(output);
        job.state = "promoting".into();
        job.message = "Publishing verified firmware files".into();
        save_job(data, job)?;
        safe_dirs(data, flash_path(data).parent().unwrap())?;
        ensure_fresh_destination(data)?;
        if flash_path(data).is_dir() {
            fs::remove_dir(flash_path(data))?;
        }
        // Linux no-replace promotion closes the race with another first install.
        use std::os::unix::ffi::OsStrExt;
        let from = std::ffi::CString::new(staged_flash.as_os_str().as_bytes())
            .map_err(|_| error("FIRMWARE_PATH", "Invalid firmware path"))?;
        let to = std::ffi::CString::new(flash_path(data).as_os_str().as_bytes())
            .map_err(|_| error("FIRMWARE_PATH", "Invalid firmware path"))?;
        // SAFETY: pointers are valid NUL-terminated paths; RENAME_NOREPLACE never overwrites a destination.
        if unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        } != 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        File::open(flash_path(data).parent().unwrap())?.sync_all()?;
        job.state = "installed".into();
        job.message = "Firmware installed and output fingerprint recorded. Game compatibility is not yet tested".into();
        save(data, &base(data).join("receipt.json"), job)?;
        save_job(data, job)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pup(path: &Path) {
        let count = 2_u64;
        let hlen = 48 + count * 64;
        let version = b"4.93\n";
        let database = b"fixture";
        let mut b = b"SCEUF\0\0\0".to_vec();
        for n in [
            1,
            68500,
            count,
            hlen,
            version.len() as u64 + database.len() as u64,
        ] {
            b.extend(n.to_be_bytes());
        }
        for (id, off, len) in [
            (0x100, hlen, version.len() as u64),
            (0x300, hlen + version.len() as u64, database.len() as u64),
        ] {
            for n in [id, off, len, 0] {
                b.extend(n.to_be_bytes());
            }
        }
        b.resize(hlen as usize, 0);
        b.extend(version);
        b.extend(database);
        fs::write(path, b).unwrap();
    }
    fn program(data: &Path, success: bool, marker: bool) {
        let program = data.join("runners/rpcs3/current");
        fs::create_dir_all(program.join("usr/bin")).unwrap();
        fs::write(program.join("usr/bin/rpcs3"), "fixture program identity").unwrap();
        let script = format!(
            r#"#!/bin/sh
set -eu
f="$XDG_CONFIG_HOME/rpcs3/dev_flash"
mkdir -p "$f/vsh/etc" "$f/vsh/module" "$f/sys/external" "$XDG_CACHE_HOME/rpcs3"
printf 'release:04.9300:\n' > "$f/vsh/etc/version.txt"
printf 'SCE\000fixture' > "$f/vsh/module/vsh.self"
printf 'SCE\000fixture' > "$f/sys/external/liblv2.sprx"
{}
exit {}
"#,
            if marker {
                "printf 'Successfully installed PS3 firmware version 4.93.\n' > \"$XDG_CACHE_HOME/rpcs3/RPCS3.log\""
            } else {
                "touch \"$XDG_CACHE_HOME/rpcs3/RPCS3.log\""
            },
            if success { 0 } else { 1 }
        );
        fs::write(program.join("AppRun"), script).unwrap();
        fs::set_permissions(program.join("AppRun"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    fn review(host: &Host, path: &Path) -> Job {
        host.dispatch(Request::Review {
            source: path.to_path_buf(),
            runner_version: "fixture-build".into(),
        })
        .unwrap()
        .job
        .unwrap()
    }
    #[test]
    fn welcome_opt_out_requires_explicit_saved_false_and_preserves_other_settings() {
        let t = tempfile::tempdir().unwrap();
        let source = t.path().join("stage");
        let target = t.path().join("main");
        fs::create_dir_all(gui_ini(&source).parent().unwrap()).unwrap();
        fs::write(
            gui_ini(&source),
            "[main_window]\ninfoBoxEnabledWelcome=true\n",
        )
        .unwrap();
        copy_welcome_opt_out(t.path(), &source, &target).unwrap();
        assert!(!gui_ini(&target).exists());
        fs::create_dir_all(gui_ini(&target).parent().unwrap()).unwrap();
        fs::write(gui_ini(&target), "[main_window]\ninfoBoxEnabledWelcome=true\ngeometry=keep\n[Other]\ninfoBoxEnabledWelcome=true\n").unwrap();
        fs::write(
            gui_ini(&source),
            "[main_window]\ninfoBoxEnabledWelcome=false\ngeometry=do-not-copy\n",
        )
        .unwrap();
        copy_welcome_opt_out(t.path(), &source, &target).unwrap();
        let result = fs::read_to_string(gui_ini(&target)).unwrap();
        assert_eq!(result, "[main_window]\ninfoBoxEnabledWelcome=false\ngeometry=keep\n[Other]\ninfoBoxEnabledWelcome=true\n");
        copy_welcome_opt_out(t.path(), &source, &target).unwrap();
        assert_eq!(fs::read_to_string(gui_ini(&target)).unwrap(), result);
        let retry = t.path().join("retry");
        copy_welcome_opt_out(t.path(), &target, &retry).unwrap();
        assert_eq!(
            fs::read_to_string(gui_ini(&retry)).unwrap(),
            "[main_window]\ninfoBoxEnabledWelcome=false\n"
        );
    }
    #[test]
    fn package_checks_bounds_not_names_and_rejects_links_and_wrong_versions() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("arbitrary-name.bin");
        pup(&path);
        let package = inspect_package(&path).unwrap();
        assert_eq!(package.version, "4.93");
        assert_eq!(package.sha256.len(), 64);
        let mut bytes = fs::read(&path).unwrap();
        bytes[32..40].copy_from_slice(&u64::MAX.to_be_bytes());
        fs::write(&path, &bytes).unwrap();
        assert_eq!(inspect_package(&path).unwrap_err().code, "FIRMWARE_FORMAT");
        pup(&path);
        let link = t.path().join("link.PUP");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(inspect_package(&link).is_err());
        assert_eq!(normalized_version("04.9300"), normalized_version("4.93"));
        assert!(normalized_version("4.x").is_none());
    }
    #[test]
    fn verified_install_preserves_source_saves_and_settings_and_replays_without_reinstalling() {
        let t = tempfile::tempdir().unwrap();
        let data = t.path().join("data");
        fs::create_dir(&data).unwrap();
        program(&data, true, true);
        let source = t.path().join("PS3UPDAT.PUP");
        pup(&source);
        let original = fs::read(&source).unwrap();
        let saves = data.join("rpcs3/xdg-config/rpcs3/dev_hdd0/home/00000001/savedata/game");
        fs::create_dir_all(&saves).unwrap();
        fs::write(saves.join("save.bin"), "personal save").unwrap();
        let settings = data.join("rpcs3/xdg-config/rpcs3/config.yml");
        fs::write(&settings, "personal settings").unwrap();
        let host = Host::new(&data).unwrap();
        let job = review(&host, &source);
        let result = host
            .dispatch(Request::Start {
                review_token: job.review_token.clone(),
            })
            .unwrap();
        assert_eq!(result.installed_version.as_deref(), Some("4.93"));
        assert!(result.managed_receipt);
        assert_eq!(
            result.job.as_ref().unwrap().output.as_ref().unwrap().files,
            3
        );
        assert_eq!(fs::read(&source).unwrap(), original);
        assert_eq!(fs::read_to_string(&settings).unwrap(), "personal settings");
        assert_eq!(
            fs::read_to_string(saves.join("save.bin")).unwrap(),
            "personal save"
        );
        assert_eq!(
            host.dispatch(Request::Start {
                review_token: job.review_token
            })
            .unwrap()
            .job
            .unwrap()
            .state,
            "installed"
        );
        assert_eq!(
            host.dispatch(Request::Review {
                source,
                runner_version: "fixture".into()
            })
            .unwrap_err()
            .code,
            "FIRMWARE_EXISTS"
        );
    }
    #[test]
    fn no_success_marker_or_bad_exit_never_promotes_even_complete_looking_outputs() {
        for (clean, marker) in [(true, false), (false, true)] {
            let t = tempfile::tempdir().unwrap();
            let data = t.path().join("data");
            fs::create_dir(&data).unwrap();
            program(&data, clean, marker);
            let source = t.path().join("PS3UPDAT.PUP");
            pup(&source);
            let host = Host::new(&data).unwrap();
            let job = review(&host, &source);
            assert!(host
                .dispatch(Request::Start {
                    review_token: job.review_token
                })
                .is_err());
            assert!(!flash_path(&data).exists());
            assert_eq!(read_status(&data).unwrap().job.unwrap().state, "failed");
        }
    }
    #[test]
    fn changed_sources_stale_tokens_and_existing_firmware_are_refused() {
        let t = tempfile::tempdir().unwrap();
        let data = t.path().join("data");
        fs::create_dir(&data).unwrap();
        program(&data, true, true);
        let source = t.path().join("PS3UPDAT.PUP");
        pup(&source);
        let host = Host::new(&data).unwrap();
        let job = review(&host, &source);
        assert_eq!(
            host.dispatch(Request::Start {
                review_token: "stale".into()
            })
            .unwrap_err()
            .code,
            "FIRMWARE_REVIEW"
        );
        fs::write(&source, "changed").unwrap();
        assert_eq!(
            host.dispatch(Request::Start {
                review_token: job.review_token
            })
            .unwrap_err()
            .code,
            "FIRMWARE_CHANGED"
        );
        assert!(!flash_path(&data).exists());
        pup(&source);
        let job = review(&host, &source);
        fs::create_dir_all(flash_path(&data)).unwrap();
        fs::write(flash_path(&data).join("keep"), "existing").unwrap();
        assert_eq!(
            host.dispatch(Request::Start {
                review_token: job.review_token
            })
            .unwrap_err()
            .code,
            "FIRMWARE_EXISTS"
        );
        assert_eq!(
            fs::read_to_string(flash_path(&data).join("keep")).unwrap(),
            "existing"
        );
    }
    #[test]
    fn promotion_receipts_recover_and_corrupt_feature_state_does_not_prevent_manager_start() {
        let t = tempfile::tempdir().unwrap();
        let data = t.path().join("data");
        fs::create_dir(&data).unwrap();
        program(&data, true, true);
        let source = t.path().join("PS3UPDAT.PUP");
        pup(&source);
        let host = Host::new(&data).unwrap();
        let job = review(&host, &source);
        let status = host
            .dispatch(Request::Start {
                review_token: job.review_token,
            })
            .unwrap();
        let mut job = status.job.unwrap();
        fs::remove_file(base(&data).join("receipt.json")).unwrap();
        job.state = "promoting".into();
        save_job(&data, &job).unwrap();
        let host = Host::new(&data).unwrap();
        assert!(host.dispatch(Request::Status).unwrap().managed_receipt);
        fs::write(record(&data), "broken checkpoint").unwrap();
        assert!(Host::new(&data).is_ok());
        assert!(read_status(&data).is_err());
    }
}
