//! Exclusive offline installation moves. Receipt paths stay stable via a
//! journal-owned parent link; files, receipt payloads and library IDs are not rewritten.
use crate::{endpoint, service, service::Manager, AppError, Receipt, Result, State};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

fn error(message: &str) -> AppError {
    AppError::new("STORAGE_MOVE", message)
}
fn present(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}
fn sync(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}
fn write(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| error("Missing journal parent"))?;
    endpoint::private_directory(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(&serde_json::to_vec(value)?)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    sync(parent)
}
fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let file = File::options()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > 32 * 1024 * 1024 {
        return Err(error("Invalid storage journal"));
    }
    Ok(serde_json::from_reader(file.take(32 * 1024 * 1024 + 1))?)
}
fn rename(source: &Path, destination: &Path) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let a = std::ffi::CString::new(source.as_os_str().as_bytes())
        .map_err(|_| error("Invalid source"))?;
    let b = std::ffi::CString::new(destination.as_os_str().as_bytes())
        .map_err(|_| error("Invalid destination"))?;
    // SAFETY: valid NUL-terminated paths. NOREPLACE never overwrites another tree.
    if unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            a.as_ptr(),
            libc::AT_FDCWD,
            b.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    sync(source.parent().unwrap())?;
    sync(destination.parent().unwrap())
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Identity {
    device: u64,
    inode: u64,
}
fn identity(path: &Path) -> Result<Identity> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_dir() {
        return Err(error("Expected a real directory, not a link"));
    }
    Ok(Identity {
        device: m.dev(),
        inode: m.ino(),
    })
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Entry {
    path: PathBuf,
    directory: bool,
    bytes: u64,
    mode: u32,
    sha256: String,
}
fn inventory(root: &Path) -> Result<Vec<Entry>> {
    identity(root)?;
    let mut pending = vec![root.to_path_buf()];
    let mut rows = Vec::new();
    while let Some(dir) = pending.pop() {
        for child in fs::read_dir(dir)? {
            if rows.len() >= 100_000 {
                return Err(error(
                    "Installation tree exceeds the relocation entry limit",
                ));
            }
            let path = child?.path();
            let meta = fs::symlink_metadata(&path)?;
            let directory = meta.is_dir();
            if !directory && !meta.is_file() {
                return Err(error("Installation contains a link or special file"));
            }
            let mut sha256 = String::new();
            if directory {
                pending.push(path.clone());
            } else {
                let mut file = File::options()
                    .read(true)
                    .custom_flags(libc::O_NOFOLLOW)
                    .open(&path)?;
                let mut hash = Sha256::new();
                let mut buf = [0u8; 128 * 1024];
                loop {
                    let n = file.read(&mut buf)?;
                    if n == 0 {
                        break;
                    }
                    hash.update(&buf[..n]);
                }
                let after = fs::symlink_metadata(&path)?;
                if (
                    meta.dev(),
                    meta.ino(),
                    meta.len(),
                    meta.mtime(),
                    meta.mtime_nsec(),
                ) != (
                    after.dev(),
                    after.ino(),
                    after.len(),
                    after.mtime(),
                    after.mtime_nsec(),
                ) {
                    return Err(error("Installation changed while hashing"));
                }
                sha256 = format!("{:x}", hash.finalize());
            }
            rows.push(Entry {
                path: path
                    .strip_prefix(root)
                    .map_err(|_| error("Invalid installation path"))?
                    .into(),
                directory,
                bytes: if directory { 0 } else { meta.len() },
                mode: meta.mode(),
                sha256,
            });
        }
    }
    rows.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(rows)
}

#[derive(Clone, Serialize, Deserialize)]
struct Location {
    path: PathBuf,
    identity: Identity,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub stage: String,
    pub source: PathBuf,
    pub destination: PathBuf,
    pub bytes: u64,
    pub files: usize,
    pub receipt_ids: Vec<String>,
    pub free_bytes: Option<u64>,
    source_identity: Identity,
    parent_identity: Identity,
    previous_location: Option<Location>,
    entries: Vec<Entry>,
    receipts: Vec<Receipt>,
}
#[derive(Clone, Serialize)]
pub struct Summary {
    pub id: String,
    pub stage: String,
    pub source: PathBuf,
    pub destination: PathBuf,
    pub bytes: u64,
    pub files: usize,
    pub receipt_ids: Vec<String>,
    pub free_bytes: Option<u64>,
}
impl Plan {
    pub fn summary(&self) -> Summary {
        Summary {
            id: self.id.clone(),
            stage: self.stage.clone(),
            source: self.source.clone(),
            destination: self.destination.clone(),
            bytes: self.bytes,
            files: self.files,
            receipt_ids: self.receipt_ids.clone(),
            free_bytes: self.free_bytes,
        }
    }
}
fn folder(data: &Path) -> PathBuf {
    data.join("manager/installation-moves")
}
fn legacy(data: &Path) -> PathBuf {
    data.join("manager/installations")
}
fn journal(data: &Path, id: &str) -> Result<PathBuf> {
    uuid::Uuid::parse_str(id).map_err(|_| error("Invalid move ID"))?;
    Ok(folder(data).join(format!("{id}.json")))
}
fn current(data: &Path) -> Result<Option<Plan>> {
    let pointer = folder(data).join("current.json");
    if !present(&pointer) {
        return Ok(None);
    }
    let id: String = read(&pointer)?;
    let plan: Plan = read(&journal(data, &id)?)?;
    if plan.id != id {
        return Err(error("Move journal identity mismatch"));
    }
    Ok(Some(plan))
}
pub fn status(data: &Path) -> Result<Option<Summary>> {
    Ok(current(data)?.map(|p| p.summary()))
}
pub fn incomplete(data: &Path) -> Result<bool> {
    Ok(current(data)?
        .is_some_and(|p| !matches!(p.stage.as_str(), "reviewed" | "complete" | "rolled_back")))
}
fn configured(data: &Path) -> Result<Option<Location>> {
    let path = folder(data).join("location.json");
    if present(&path) {
        read(&path).map(Some)
    } else {
        Ok(None)
    }
}
pub(crate) fn installation_root(data: &Path) -> Result<PathBuf> {
    if incomplete(data)? {
        return Err(error(
            "Finish or roll back the interrupted installation move in Storage settings",
        ));
    }
    let stable = legacy(data);
    match configured(data)? {
        Some(location) => {
            if identity(&location.path)? != location.identity
                || fs::canonicalize(&stable)? != location.path
            {
                return Err(error(
                    "Installation location changed; recover it in Storage settings",
                ));
            }
            service::private_dir(&location.path)?;
            Ok(location.path)
        }
        None => {
            service::private_dir(&stable)?;
            Ok(stable)
        }
    }
}

pub struct Offline {
    data: PathBuf,
    _lease: endpoint::Lock,
}
impl Offline {
    pub fn acquire(data: &Path) -> Result<Self> {
        let data = fs::canonicalize(data)?;
        let lease = endpoint::Lock::acquire(&data)?;
        endpoint::private_directory(&folder(&data))?;
        Ok(Self {
            data,
            _lease: lease,
        })
    }
    fn idle_jobs(&self) -> Result<Vec<crate::Job>> {
        let manager = Manager::new(&self.data);
        let jobs = manager.list()?;
        if jobs.iter().any(|j| {
            !matches!(
                j.state,
                State::Registered | State::Failed | State::Cancelled
            )
        }) {
            return Err(error(
                "Finish or cancel pending preparation jobs before relocating installations",
            ));
        }
        if manager.browser_jobs()?.iter().any(|j| {
            matches!(
                j.state,
                crate::browser::State::Waiting | crate::browser::State::Downloading
            )
        }) {
            return Err(error(
                "Finish or cancel browser downloads before relocating installations",
            ));
        }
        let settings = self.data.join("manager/torrents/settings.json");
        if present(&settings) && crate::torrent::read_settings(&self.data)?.enabled {
            return Err(error("Disable torrent transport before relocating installations; its payloads will stay in place"));
        }
        Ok(jobs)
    }
    pub fn review(&self, parent: &Path) -> Result<Summary> {
        if incomplete(&self.data)? {
            return Err(error(
                "Recover the interrupted move before reviewing another destination",
            ));
        }
        let source = installation_root(&self.data)?;
        let source_identity = identity(&source)?;
        if !parent.is_absolute() {
            return Err(error("Choose an absolute destination parent"));
        }
        let parent = fs::canonicalize(parent)?;
        let parent_identity = identity(&parent)?;
        let destination = parent.join("managed-installations");
        if source_identity.device != parent_identity.device {
            return Err(error(
                "This first move workflow supports the same filesystem only",
            ));
        }
        if destination.starts_with(&source)
            || source.starts_with(&destination)
            || destination.starts_with(&self.data)
            || self.data.starts_with(&destination)
            || present(&destination)
        {
            return Err(error("Choose an unused destination outside the application data and current installation tree"));
        }
        let jobs = self.idle_jobs()?;
        let receipts = jobs
            .iter()
            .filter(|j| j.state == State::Registered)
            .map(|j| {
                j.receipt
                    .clone()
                    .ok_or_else(|| error("Registered job has no receipt"))
            })
            .collect::<Result<Vec<_>>>()?;
        if receipts.is_empty() {
            return Err(error(
                "There are no registered manager installations to move",
            ));
        }
        for receipt in &receipts {
            if receipt.destination.file_name().and_then(|n| n.to_str()) != Some(receipt.id.as_str())
                || fs::canonicalize(
                    receipt
                        .destination
                        .parent()
                        .ok_or_else(|| error("Receipt parent missing"))?,
                )? != source
            {
                return Err(error("Receipt has a nonstandard installation location"));
            }
            receipt.verify(&receipt.destination)?;
        }
        let entries = inventory(&source)?;
        for entry in &entries {
            let owner = receipts
                .iter()
                .find(|r| entry.path.starts_with(&r.id))
                .ok_or_else(|| {
                    error("Unregistered or staged data is present; resolve it before moving")
                })?;
            if !entry.directory
                && !owner.files.iter().any(|f| {
                    entry.path == Path::new(&owner.id).join(&f.path)
                        && entry.bytes == f.size
                        && entry.sha256 == f.sha256
                })
            {
                return Err(error(
                    "Installation contains files not covered by its receipt",
                ));
            }
        }
        let plan = Plan {
            id: uuid::Uuid::new_v4().to_string(),
            stage: "reviewed".into(),
            bytes: entries.iter().map(|e| e.bytes).sum(),
            files: entries.iter().filter(|e| !e.directory).count(),
            receipt_ids: receipts.iter().map(|r| r.id.clone()).collect(),
            source,
            destination,
            free_bytes: marquee_media::archive::free_bytes(&parent),
            source_identity,
            parent_identity,
            previous_location: configured(&self.data)?,
            entries,
            receipts,
        };
        write(&journal(&self.data, &plan.id)?, &plan)?;
        write(&folder(&self.data).join("current.json"), &plan.id)?;
        Ok(plan.summary())
    }
    fn load(&self, id: &str) -> Result<Plan> {
        let plan = current(&self.data)?.ok_or_else(|| error("Review a move first"))?;
        if plan.id != id {
            return Err(error("Move review was replaced; review the current plan"));
        }
        Ok(plan)
    }
    fn save(&self, plan: &Plan) -> Result<()> {
        write(&journal(&self.data, &plan.id)?, plan)
    }
    fn verify_jobs(&self, plan: &Plan) -> Result<()> {
        let jobs = self.idle_jobs()?;
        let mut actual = jobs
            .iter()
            .filter(|j| j.state == State::Registered)
            .map(|j| {
                j.receipt
                    .as_ref()
                    .ok_or_else(|| error("Receipt missing"))
                    .and_then(|r| r.fingerprint())
            })
            .collect::<Result<Vec<_>>>()?;
        let mut expected = plan
            .receipts
            .iter()
            .map(Receipt::fingerprint)
            .collect::<Result<Vec<_>>>()?;
        actual.sort();
        expected.sort();
        if actual != expected {
            return Err(error(
                "Manager jobs changed since review; review the destination again",
            ));
        }
        Ok(())
    }
    pub fn apply(&self, id: &str) -> Result<Summary> {
        let mut plan = self.load(id)?;
        if plan.stage == "complete" {
            return Ok(plan.summary());
        }
        if plan.stage == "rolled_back" || plan.stage == "rolling_back" {
            return Err(error(
                "This move was rolled back; review another destination",
            ));
        }
        self.verify_jobs(&plan)?;
        if identity(plan.destination.parent().unwrap())? != plan.parent_identity {
            return Err(error("Destination volume or folder changed"));
        }
        if !present(&plan.destination) {
            if identity(&plan.source)? != plan.source_identity
                || inventory(&plan.source)? != plan.entries
            {
                return Err(error("Installation contents changed after review"));
            }
            // Schema 5 retires older managers which cannot validate a relocated parent.
            let db = Manager::new(&self.data).database()?;
            db.pragma_update(None, "user_version", 5)?;
            plan.stage = "moving".into();
            self.save(&plan)?;
            rename(&plan.source, &plan.destination)?;
        }
        if identity(&plan.destination)? != plan.source_identity {
            return Err(error("Destination is not the reviewed installation tree"));
        }
        if !present(&plan.source) {
            std::os::unix::fs::symlink(&plan.destination, &plan.source)?;
            sync(plan.source.parent().unwrap())?;
        }
        if fs::read_link(&plan.source)? != plan.destination {
            return Err(error("The compatibility path was replaced"));
        }
        plan.stage = "verifying".into();
        self.save(&plan)?;
        if inventory(&plan.destination)? != plan.entries {
            return Err(error(
                "Destination verification failed; files retained for recovery",
            ));
        }
        for receipt in &plan.receipts {
            receipt.verify(&receipt.destination)?;
        }
        write(
            &folder(&self.data).join("location.json"),
            &Location {
                path: plan.destination.clone(),
                identity: plan.source_identity.clone(),
            },
        )?;
        plan.stage = "complete".into();
        self.save(&plan)?;
        Ok(plan.summary())
    }
    pub fn rollback(&self, id: &str) -> Result<Summary> {
        let mut plan = self.load(id)?;
        self.verify_jobs(&plan)?;
        if plan.stage == "rolled_back" {
            return Ok(plan.summary());
        }
        if !present(&plan.destination) && identity(&plan.source)? == plan.source_identity {
            if inventory(&plan.source)? != plan.entries {
                return Err(error("Source changed; rollback refused"));
            }
        } else {
            if identity(&plan.destination)? != plan.source_identity
                || inventory(&plan.destination)? != plan.entries
            {
                return Err(error("Destination changed; rollback refused"));
            }
            if present(&plan.source) && fs::read_link(&plan.source)? != plan.destination {
                return Err(error("Compatibility path is occupied; rollback refused"));
            }
            plan.stage = "rolling_back".into();
            self.save(&plan)?;
            if present(&plan.source) {
                fs::remove_file(&plan.source)?;
                sync(plan.source.parent().unwrap())?;
            }
            rename(&plan.destination, &plan.source)?;
        }
        match &plan.previous_location {
            Some(location) => write(&folder(&self.data).join("location.json"), location)?,
            None => {
                let path = folder(&self.data).join("location.json");
                if present(&path) {
                    fs::remove_file(path)?;
                    sync(&folder(&self.data))?;
                }
            }
        }
        for receipt in &plan.receipts {
            receipt.verify(&receipt.destination)?;
        }
        plan.stage = "rolled_back".into();
        self.save(&plan)?;
        Ok(plan.summary())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        temp: tempfile::TempDir,
        data: PathBuf,
        parent: PathBuf,
        manager: Manager,
        receipt: Receipt,
    }
    impl Fixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let data = temp.path().join("data");
            fs::create_dir(&data).unwrap();
            let parent = temp.path().join("destination");
            fs::create_dir(&parent).unwrap();
            let manager = Manager::new(&data);
            let receipt = Self::prepare(&manager, temp.path(), "first");
            Self {
                temp,
                data,
                parent,
                manager,
                receipt,
            }
        }
        fn prepare(manager: &Manager, base: &Path, name: &str) -> Receipt {
            let path = base.join(format!("{name}.zip"));
            let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
            zip.start_file(
                format!("{name}.gen"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(&vec![0u8; 1024]).unwrap();
            zip.finish().unwrap();
            let job = manager.inspect(&path, "genesis").unwrap();
            let done = manager.execute(&job.id, &job.review_token).unwrap();
            assert_eq!(done.state, State::AwaitingLibrary);
            let receipt = done.receipt.unwrap();
            manager
                .acknowledge(&job.id, &receipt.fingerprint().unwrap(), vec![name.into()])
                .unwrap();
            receipt
        }
    }
    #[test]
    fn verified_move_preserves_receipts_and_rollback_restores_the_tree() {
        let f = Fixture::new();
        let owner = Offline::acquire(&f.data).unwrap();
        let fingerprint = f.receipt.fingerprint().unwrap();
        let plan = owner.review(&f.parent).unwrap();
        assert_eq!(plan.files, 1);
        assert_eq!(plan.bytes, 1024);
        assert_eq!(owner.apply(&plan.id).unwrap().stage, "complete");
        assert!(legacy(&f.data).is_symlink());
        f.receipt.verify(&f.receipt.destination).unwrap();
        assert_eq!(
            f.manager
                .get(&f.receipt.id)
                .unwrap()
                .receipt
                .unwrap()
                .fingerprint()
                .unwrap(),
            fingerprint
        );
        assert_eq!(installation_root(&f.data).unwrap(), plan.destination);
        assert_eq!(owner.rollback(&plan.id).unwrap().stage, "rolled_back");
        assert!(!legacy(&f.data).is_symlink());
        f.receipt.verify(&f.receipt.destination).unwrap();
    }
    #[test]
    fn interrupted_rename_recovers_from_the_durable_review() {
        let f = Fixture::new();
        let owner = Offline::acquire(&f.data).unwrap();
        let review = owner.review(&f.parent).unwrap();
        let mut plan = owner.load(&review.id).unwrap();
        plan.stage = "moving".into();
        owner.save(&plan).unwrap();
        f.manager
            .database()
            .unwrap()
            .pragma_update(None, "user_version", 5)
            .unwrap();
        fs::rename(&plan.source, &plan.destination).unwrap();
        drop(owner);
        assert!(incomplete(&f.data).unwrap());
        assert!(installation_root(&f.data).is_err());
        let owner = Offline::acquire(&f.data).unwrap();
        assert_eq!(owner.apply(&review.id).unwrap().stage, "complete");
        f.receipt.verify(&f.receipt.destination).unwrap();
    }
    #[test]
    fn changed_inputs_and_occupied_destinations_are_never_overwritten() {
        let f = Fixture::new();
        let owner = Offline::acquire(&f.data).unwrap();
        let plan = owner.review(&f.parent).unwrap();
        fs::create_dir(&plan.destination).unwrap();
        fs::write(plan.destination.join("keep"), b"keep").unwrap();
        assert!(owner.apply(&plan.id).is_err());
        assert_eq!(fs::read(plan.destination.join("keep")).unwrap(), b"keep");
        fs::remove_file(plan.destination.join("keep")).unwrap();
        fs::remove_dir(&plan.destination).unwrap();
        fs::write(
            f.receipt.destination.join(&f.receipt.files[0].path),
            b"changed",
        )
        .unwrap();
        assert!(owner.apply(&plan.id).is_err());
        assert!(!legacy(&f.data).is_symlink());
    }
    #[test]
    fn new_preparation_uses_physical_destination_and_prevents_stale_rollback() {
        let f = Fixture::new();
        let owner = Offline::acquire(&f.data).unwrap();
        let plan = owner.review(&f.parent).unwrap();
        owner.apply(&plan.id).unwrap();
        let next = Fixture::prepare(&f.manager, f.temp.path(), "second");
        assert!(next.destination.starts_with(&plan.destination));
        next.verify(&next.destination).unwrap();
        assert!(owner.rollback(&plan.id).is_err());
        assert!(next.destination.is_dir());
        let second_parent = f.temp.path().join("second-destination");
        fs::create_dir(&second_parent).unwrap();
        let second = owner.review(&second_parent).unwrap();
        owner.apply(&second.id).unwrap();
        f.receipt.verify(&f.receipt.destination).unwrap();
        next.verify(&next.destination).unwrap();
        owner.rollback(&second.id).unwrap();
        next.verify(&next.destination).unwrap();
    }
    #[test]
    fn rollback_recovers_after_the_reverse_rename() {
        let f = Fixture::new();
        let owner = Offline::acquire(&f.data).unwrap();
        let review = owner.review(&f.parent).unwrap();
        owner.apply(&review.id).unwrap();
        let mut plan = owner.load(&review.id).unwrap();
        plan.stage = "rolling_back".into();
        owner.save(&plan).unwrap();
        fs::remove_file(&plan.source).unwrap();
        fs::rename(&plan.destination, &plan.source).unwrap();
        assert_eq!(owner.rollback(&review.id).unwrap().stage, "rolled_back");
        f.receipt.verify(&f.receipt.destination).unwrap();
        assert_eq!(installation_root(&f.data).unwrap(), legacy(&f.data));
    }
    #[test]
    fn exclusive_manager_lease_blocks_relocation() {
        let f = Fixture::new();
        let _lease = endpoint::Lock::acquire(&f.data).unwrap();
        assert!(Offline::acquire(&f.data).is_err());
    }
}
