//! Durable preparation service owned exclusively by the manager process.
use marquee_media::archive::ArchivePlan;
use marquee_media::error::{AppError, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const RECIPE: &str = "local-archive-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    AwaitingReview,
    Preparing,
    Interrupted,
    AwaitingLibrary,
    Registered,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Output {
    pub path: PathBuf,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub id: String,
    pub recipe: String,
    pub source_sha256: String,
    pub system: String,
    pub destination: PathBuf,
    pub files: Vec<Output>,
    /// Local hashes/structure are evidence, not verified release identity.
    pub content_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub source: PathBuf,
    pub source_size: u64,
    pub source_sha256: String,
    pub destination: PathBuf,
    pub plan: ArchivePlan,
    pub review_token: String,
    pub state: State,
    pub attempt: u32,
    pub error: Option<String>,
    pub error_code: Option<String>,
    pub receipt: Option<Receipt>,
    pub edition_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub struct Manager {
    dir: PathBuf,
    /// Only one preparation/promotion may run at once; readers stay responsive.
    active: AtomicBool,
}

fn error(code: &str, message: &str) -> AppError {
    AppError::new(code, message)
}

fn digest_file(path: &Path) -> Result<String> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() {
        return Err(error(
            "PREPARATION_UNSAFE",
            "Expected a regular file, not a link or directory",
        ));
    }
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buf = [0u8; 128 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub(crate) fn private_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)?;
    }
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(error(
            "PREPARATION_UNSAFE",
            "Manager storage must be a regular directory",
        ));
    }
    Ok(())
}

struct Lease<'a>(&'a AtomicBool);
impl Drop for Lease<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl Manager {
    fn claim(&self) -> Result<Lease<'_>> {
        self.active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                error(
                    "PREPARATION_BUSY",
                    "Another preparation operation is running",
                )
            })?;
        Ok(Lease(&self.active))
    }

    pub fn new(data: &Path) -> Self {
        Self {
            dir: fs::canonicalize(data)
                .unwrap_or_else(|_| data.to_path_buf())
                .join("manager"),
            active: AtomicBool::new(false),
        }
    }

    pub(crate) fn database(&self) -> Result<Connection> {
        private_dir(&self.dir)?;
        let path = self.dir.join("jobs.db");
        File::options()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)?;
        if !fs::symlink_metadata(&path)?.is_file() {
            return Err(error(
                "PREPARATION_UNSAFE",
                "Manager database is not a regular file",
            ));
        }
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > 2 {
            return Err(error(
                "PREPARATION_VERSION",
                "Manager database is newer than this build",
            ));
        }
        if version == 1 {
            return Err(error("MANAGER_LEGACY_STATE", "M1 jobs need an exclusive ownership cutover. Start the manager through the new launcher or CLI."));
        }
        if version == 0 {
            conn.execute_batch(
                "BEGIN IMMEDIATE;
                CREATE TABLE IF NOT EXISTS jobs(id TEXT PRIMARY KEY, payload TEXT NOT NULL);
                PRAGMA user_version=2; COMMIT;",
            )?;
        }
        Ok(conn)
    }

    fn save(&self, job: &mut Job) -> Result<()> {
        job.updated_at = marquee_media::time::now_rfc3339();
        self.database()?.execute(
            "INSERT INTO jobs(id,payload) VALUES(?1,?2)
            ON CONFLICT(id) DO UPDATE SET payload=excluded.payload",
            params![job.id, serde_json::to_string(job)?],
        )?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Job> {
        let data: Option<String> = self
            .database()?
            .query_row("SELECT payload FROM jobs WHERE id=?1", [id], |r| r.get(0))
            .optional()?;
        serde_json::from_str(
            &data.ok_or_else(|| error("PREPARATION_MISSING", "Preparation job not found"))?,
        )
        .map_err(Into::into)
    }

    pub fn list(&self) -> Result<Vec<Job>> {
        // Claim before reading: a snapshot read during extraction must never
        // overwrite a newer completed state when the worker finishes.
        let recovery = self.claim().ok();
        let conn = self.database()?;
        let mut query = conn.prepare("SELECT payload FROM jobs ORDER BY rowid DESC")?;
        let mut jobs = query
            .query_map([], |r| r.get::<_, String>(0))?
            .map(|r| Ok(serde_json::from_str(&r?)?))
            .collect::<Result<Vec<Job>>>()?;
        // No active operation can own a `preparing` checkpoint after restart.
        // Do not rewrite it while a live worker is extracting.
        if recovery.is_some() {
            for job in &mut jobs {
                if job.state == State::Preparing {
                    job.state = State::Interrupted;
                    job.error = Some(
                        "Preparation was interrupted. Retry to reconcile staged files.".into(),
                    );
                    job.error_code = Some("PREPARATION_INTERRUPTED".into());
                    self.save(job)?;
                }
            }
        }
        Ok(jobs)
    }

    pub fn inspect(&self, source: &Path, system: &str) -> Result<Job> {
        let _guard = self.claim()?;
        if !source.is_absolute() || !fs::symlink_metadata(source)?.is_file() {
            return Err(error(
                "PREPARATION_UNSAFE",
                "Choose an absolute regular archive path",
            ));
        }
        let source = fs::canonicalize(source)?;
        let plan = ArchivePlan::inspect(&source, system)?;
        let hash = digest_file(&source)?;
        let source_size = fs::metadata(&source)?.len();
        let id = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(RECIPE, &source, &hash, &plan))?)
        );
        let destination = self.dir.join("installations").join(&id);
        let review_token = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(&id, &destination, &plan))?)
        );
        let conn = self.database()?;
        if conn
            .query_row("SELECT 1 FROM jobs WHERE id=?1", [&id], |_| Ok(()))
            .optional()?
            .is_some()
        {
            let mut job = self.get(&id)?;
            if job.state == State::Cancelled {
                job.state = State::AwaitingReview;
                job.error = None;
                job.error_code = None;
                self.save(&mut job)?;
            }
            return Ok(job);
        }
        let now = marquee_media::time::now_rfc3339();
        let mut job = Job {
            id,
            source,
            source_size,
            source_sha256: hash,
            destination,
            plan,
            review_token,
            state: State::AwaitingReview,
            attempt: 0,
            error: None,
            error_code: None,
            receipt: None,
            edition_ids: vec![],
            created_at: now.clone(),
            updated_at: now,
        };
        self.save(&mut job)?;
        Ok(job)
    }

    fn staging(&self, job: &Job) -> PathBuf {
        self.dir
            .join("installations")
            .join(format!(".{}.staging", job.id))
    }

    fn prepare(&self, job: &mut Job) -> Result<()> {
        let parent = job
            .destination
            .parent()
            .ok_or_else(|| error("PREPARATION_UNSAFE", "Invalid destination"))?
            .to_path_buf();
        private_dir(&parent)?;
        let staging = self.staging(job);
        if let Some(receipt) = &job.receipt {
            // Receipt persisted before promotion: a crash on either side of the
            // rename recovers without re-extracting or requiring the archive.
            let root = if job.destination.exists() {
                &job.destination
            } else {
                &staging
            };
            receipt.verify(root)?;
        } else {
            if job.destination.exists() {
                return Err(error(
                    "PREPARATION_CONFLICT",
                    "Destination exists without a receipt; no files were overwritten",
                ));
            }
            if digest_file(&job.source)? != job.source_sha256
                || ArchivePlan::inspect(&job.source, &job.plan.system)? != job.plan
            {
                return Err(error(
                    "PREPARATION_CHANGED",
                    "Source changed after review. Inspect it again; original job retained.",
                ));
            }
            if staging.exists() {
                // Only our exact, private job path is eligible for interrupted
                // staging cleanup. Never follow a replacement symlink.
                if !fs::symlink_metadata(&staging)?.is_dir() {
                    return Err(error("PREPARATION_UNSAFE", "Staging path was replaced"));
                }
                fs::remove_dir_all(&staging)?;
            }
            job.plan.extract_into(&job.source, &staging)?;
            if digest_file(&job.source)? != job.source_sha256 {
                return Err(error(
                    "PREPARATION_CHANGED",
                    "Source changed during extraction; outputs were not published",
                ));
            }
            let files = job
                .plan
                .files
                .iter()
                .map(|f| {
                    Ok(Output {
                        path: f.path.clone(),
                        size: f.size,
                        sha256: digest_file(&staging.join(&f.path))?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            job.receipt = Some(Receipt {
                id: job.id.clone(),
                recipe: RECIPE.into(),
                source_sha256: job.source_sha256.clone(),
                system: job.plan.system.clone(),
                destination: job.destination.clone(),
                files,
                content_verified: false,
            });
            self.save(job)?;
        }
        if !job.destination.exists() {
            fs::rename(&staging, &job.destination)?;
            File::open(parent)?.sync_all()?;
        }
        job.state = State::AwaitingLibrary;
        self.save(job)
    }
}

impl Manager {
    pub fn active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    fn execute(&self, id: &str, token: &str) -> Result<Job> {
        let mut job = self.get(id)?;
        if job.review_token != token {
            return Err(error(
                "PREPARATION_REVIEW",
                "Inspect and review the job again",
            ));
        }
        if job.state == State::Cancelled {
            return Err(error(
                "PREPARATION_CANCELLED",
                "Inspect again to review this cancelled job",
            ));
        }
        if matches!(job.state, State::Registered | State::AwaitingLibrary) {
            return Ok(job);
        }
        job.state = State::Preparing;
        job.attempt += 1;
        job.error = None;
        job.error_code = None;
        self.save(&mut job)?;
        if let Err(err) = self.prepare(&mut job) {
            job.state = if job.destination.exists() && job.receipt.is_some() {
                State::AwaitingLibrary
            } else {
                State::Failed
            };
            job.error_code = Some(err.code);
            job.error = Some(err.message);
            self.save(&mut job)?;
        }
        Ok(job)
    }

    pub fn start(self: &Arc<Self>, id: &str, token: &str) -> Result<Job> {
        let mut job = self.get(id)?;
        if job.review_token != token {
            return Err(error(
                "PREPARATION_REVIEW",
                "Inspect and review the job again",
            ));
        }
        if job.state == State::Cancelled {
            return Err(error(
                "PREPARATION_CANCELLED",
                "Inspect again to review this cancelled job",
            ));
        }
        if matches!(job.state, State::Registered | State::AwaitingLibrary) {
            return Ok(job);
        }
        let lease = match self.claim() {
            Ok(lease) => lease,
            Err(_) if job.state == State::Preparing => return Ok(job),
            Err(error) => return Err(error),
        };
        // The phase may have advanced between the first read and claiming the
        // worker. Never overwrite a completed receipt with a stale snapshot.
        job = self.get(id)?;
        if matches!(job.state, State::Registered | State::AwaitingLibrary) {
            return Ok(job);
        }
        if job.state == State::Cancelled {
            return Err(error("PREPARATION_CANCELLED", "This job was cancelled"));
        }
        job.state = State::Preparing;
        self.save(&mut job)?;
        let manager = Arc::clone(self);
        let id = id.to_string();
        let token = token.to_string();
        std::mem::forget(lease);
        if let Err(err) = std::thread::Builder::new()
            .name("marquee-preparation".into())
            .spawn(move || {
                let _lease = Lease(&manager.active);
                let _ = manager.execute(&id, &token);
            })
        {
            self.active.store(false, Ordering::Release);
            return Err(err.into());
        }
        Ok(job)
    }

    pub fn cancel(&self, id: &str) -> Result<Job> {
        let _guard = self.claim()?;
        let mut job = self.get(id)?;
        if job.receipt.is_some() || job.state == State::Registered {
            return Err(error(
                "PREPARATION_STATE",
                "Prepared output is retained; source cleanup is separate",
            ));
        }
        job.state = State::Cancelled;
        self.save(&mut job)?;
        Ok(job)
    }

    pub fn acknowledge(
        &self,
        id: &str,
        fingerprint: &str,
        edition_ids: Vec<String>,
    ) -> Result<Job> {
        let _guard = self.claim()?;
        let mut job = self.get(id)?;
        let receipt = job
            .receipt
            .as_ref()
            .ok_or_else(|| error("PREPARATION_STATE", "No receipt is ready to acknowledge"))?;
        if receipt.fingerprint()? != fingerprint
            || edition_ids.is_empty()
            || edition_ids.len() > receipt.files.len()
        {
            return Err(error(
                "PREPARATION_CONFLICT",
                "Receipt acknowledgement does not match prepared output",
            ));
        }
        if job.state == State::Registered {
            if job.edition_ids != edition_ids {
                return Err(error(
                    "PREPARATION_CONFLICT",
                    "Receipt was already acknowledged differently",
                ));
            }
            return Ok(job);
        }
        if job.state != State::AwaitingLibrary {
            return Err(error(
                "PREPARATION_STATE",
                "Receipt is not ready for library acknowledgement",
            ));
        }
        job.edition_ids = edition_ids;
        job.state = State::Registered;
        job.error = None;
        job.error_code = None;
        self.save(&mut job)?;
        Ok(job)
    }

    pub fn publication_failed(
        &self,
        id: &str,
        fingerprint: &str,
        code: &str,
        message: &str,
    ) -> Result<Job> {
        let _guard = self.claim()?;
        let mut job = self.get(id)?;
        if job.state != State::AwaitingLibrary
            || job
                .receipt
                .as_ref()
                .map(Receipt::fingerprint)
                .transpose()?
                .as_deref()
                != Some(fingerprint)
        {
            return Err(error(
                "PREPARATION_STATE",
                "Receipt is no longer awaiting publication",
            ));
        }
        job.error_code = Some(code.chars().take(128).collect());
        job.error = Some(message.chars().take(4096).collect());
        self.save(&mut job)?;
        Ok(job)
    }
}

impl Receipt {
    pub fn verify(&self, root: &Path) -> Result<()> {
        if !fs::symlink_metadata(root)?.is_dir() {
            return Err(error(
                "PREPARATION_INVALID",
                "Prepared directory is not a regular directory",
            ));
        }
        let canonical = fs::canonicalize(root)?;
        for output in &self.files {
            let path = root.join(&output.path);
            if !fs::canonicalize(&path)?.starts_with(&canonical)
                || fs::symlink_metadata(&path)?.len() != output.size
                || digest_file(&path)? != output.sha256
            {
                return Err(error(
                    "PREPARATION_INVALID",
                    "Prepared files changed or are missing; source retained",
                ));
            }
        }
        Ok(())
    }

    pub fn fingerprint(&self) -> Result<String> {
        Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(self)?)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn zip(dir: &Path, name: &str, files: &[(&str, &[u8])]) -> PathBuf {
        let path = dir.join(name);
        let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
        for (name, bytes) in files {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        path
    }
    fn manager(dir: &Path) -> Manager {
        let data = dir.join("data");
        fs::create_dir_all(&data).unwrap();
        Manager::new(&data)
    }

    #[test]
    fn interrupted_staging_and_promoted_receipt_recover_without_redo_or_source() {
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path());
        let source = zip(
            dir.path(),
            "game.zip",
            &[("Game.gen", b"synthetic cartridge")],
        );
        let mut job = manager.inspect(&source, "genesis").unwrap();
        assert_eq!(manager.inspect(&source, "genesis").unwrap().id, job.id);
        job.state = State::Preparing;
        manager.save(&mut job).unwrap();
        let staging = manager.staging(&job);
        fs::create_dir_all(&staging).unwrap();
        fs::write(staging.join("partial"), b"partial").unwrap();
        assert_eq!(manager.list().unwrap()[0].state, State::Interrupted);
        let mut done = manager.execute(&job.id, &job.review_token).unwrap();
        assert_eq!(done.state, State::AwaitingLibrary);
        assert!(!done.destination.join("partial").exists());
        fs::rename(&done.destination, &staging).unwrap();
        done.state = State::Preparing;
        manager.save(&mut done).unwrap();
        fs::remove_file(source).unwrap();
        manager.list().unwrap();
        let recovered = manager.execute(&job.id, &job.review_token).unwrap();
        assert_eq!(recovered.state, State::AwaitingLibrary);
        assert!(recovered.destination.exists());
        let fingerprint = recovered.receipt.as_ref().unwrap().fingerprint().unwrap();
        assert_eq!(
            manager
                .acknowledge(&job.id, "wrong", vec!["edition".into()])
                .unwrap_err()
                .code,
            "PREPARATION_CONFLICT"
        );
        let ack = manager
            .acknowledge(&job.id, &fingerprint, vec!["edition".into()])
            .unwrap();
        assert_eq!(ack.state, State::Registered);
        assert_eq!(
            manager
                .acknowledge(&job.id, &fingerprint, vec!["edition".into()])
                .unwrap()
                .edition_ids,
            ack.edition_ids
        );
    }

    #[test]
    fn changed_source_bad_paths_packages_and_missing_cue_tracks_leave_no_installation() {
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path());
        let source = zip(dir.path(), "game.zip", &[("Game.gen", b"first")]);
        let job = manager.inspect(&source, "genesis").unwrap();
        zip(dir.path(), "game.zip", &[("Game.gen", b"other")]);
        let failed = manager.execute(&job.id, &job.review_token).unwrap();
        assert_eq!(failed.error_code.as_deref(), Some("PREPARATION_CHANGED"));
        assert!(!failed.destination.exists());
        let pkg = zip(dir.path(), "package.zip", &[("game.pkg", b"package")]);
        assert_eq!(
            manager.inspect(&pkg, "ps2").unwrap_err().code,
            "PREPARATION_UNSUPPORTED"
        );
        let bad = zip(dir.path(), "escape.zip", &[("../Game.gen", b"game")]);
        assert_eq!(
            manager.inspect(&bad, "genesis").unwrap_err().code,
            "INTAKE_UNSAFE"
        );
        let cue = zip(
            dir.path(),
            "tracks.zip",
            &[(
                "Game.cue",
                b"FILE \"Missing.bin\" BINARY\n TRACK 01 MODE2/2352\n INDEX 01 00:00:00\n",
            )],
        );
        let review = manager.inspect(&cue, "ps1").unwrap();
        let failed = manager.execute(&review.id, &review.review_token).unwrap();
        assert_eq!(failed.state, State::Failed);
        assert!(!failed.destination.exists());
        assert!(source.exists() && cue.exists());
    }

    #[test]
    fn cancelled_review_can_be_reopened_and_corruption_is_not_an_empty_history() {
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path());
        let source = zip(
            dir.path(),
            "game.zip",
            &[("Game.gen", b"synthetic cartridge")],
        );
        let job = manager.inspect(&source, "genesis").unwrap();
        manager.cancel(&job.id).unwrap();
        assert_eq!(
            manager
                .execute(&job.id, &job.review_token)
                .unwrap_err()
                .code,
            "PREPARATION_CANCELLED"
        );
        assert_eq!(
            manager.inspect(&source, "genesis").unwrap().state,
            State::AwaitingReview
        );
        let _lease = manager.claim().unwrap();
        assert_eq!(
            manager.inspect(&source, "genesis").unwrap_err().code,
            "PREPARATION_BUSY"
        );
        drop(_lease);
        let link = dir.path().join("link.zip");
        std::os::unix::fs::symlink(&source, &link).unwrap();
        assert!(manager.inspect(&link, "genesis").is_err());
        manager
            .database()
            .unwrap()
            .execute("UPDATE jobs SET payload='broken'", [])
            .unwrap();
        assert_eq!(manager.list().unwrap_err().code, "JSON");
    }

    #[test]
    fn seven_zip_uses_the_shared_recipe() {
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path());
        let source = dir.path().join("game.7z");
        fs::write(
            &source,
            include_bytes!("../../media/tests/fixtures/preparation/regular7z.fixture"),
        )
        .unwrap();
        let job = manager.inspect(&source, "ds").unwrap();
        let done = manager.execute(&job.id, &job.review_token).unwrap();
        assert_eq!(done.state, State::AwaitingLibrary, "{:?}", done.error);
        assert_eq!(
            fs::read(done.destination.join("Game.nds")).unwrap(),
            b"synthetic cartridge bytes\n"
        );
    }
}
