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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser_source: Option<crate::browser::Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub torrent_source: Option<crate::torrent_source::Source>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser_source: Option<crate::browser::Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub torrent_source: Option<crate::torrent_source::Source>,
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

pub(crate) struct Lease<'a>(&'a AtomicBool);
impl Drop for Lease<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl Manager {
    pub(crate) fn claim(&self) -> Result<Lease<'_>> {
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
        if version > 4 {
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
                PRAGMA user_version=4; COMMIT;",
            )?;
        }
        if version == 2 || version == 3 {
            // The daemon holds the exclusive manager lease before opening its
            // state. Back up without rewriting payloads; schema 4 retires older
            // writers that would silently drop torrent provenance fields.
            let backup = self.dir.join(format!(
                "jobs-v{version}-backup-{}.db",
                uuid::Uuid::new_v4()
            ));
            File::options()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&backup)?;
            conn.execute("VACUUM INTO ?1", [backup.to_string_lossy().as_ref()])?;
            File::open(&backup)?.sync_all()?;
            File::open(&self.dir)?.sync_all()?;
            conn.execute_batch("BEGIN IMMEDIATE; PRAGMA user_version=4; COMMIT;")?;
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
        let mut review = self.build_review(source, system, None, None)?;
        let conn = self.database()?;
        let existing: Option<String> = conn
            .query_row("SELECT payload FROM jobs WHERE id=?1", [&review.id], |r| {
                r.get(0)
            })
            .optional()?;
        if let Some(existing) = existing {
            review = serde_json::from_str(&existing)?;
            if review.state != State::Cancelled {
                return Ok(review);
            }
            review.state = State::AwaitingReview;
            review.error = None;
            review.error_code = None;
        }
        self.save(&mut review)?;
        Ok(review)
    }

    pub(crate) fn build_review(
        &self,
        source: &Path,
        system: &str,
        browser_source: Option<crate::browser::Source>,
        torrent_source: Option<crate::torrent_source::Source>,
    ) -> Result<Job> {
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
        if browser_source.is_some() && torrent_source.is_some() {
            return Err(error(
                "PREPARATION_INVALID",
                "A preparation source must have one transport identity",
            ));
        }
        let identity = if let Some(provenance) = &torrent_source {
            serde_json::to_vec(&(
                RECIPE,
                "torrent-source-v1",
                &source,
                &hash,
                &plan,
                provenance,
            ))?
        } else if let Some(provenance) = &browser_source {
            serde_json::to_vec(&(
                RECIPE,
                "browser-source-v1",
                &source,
                &hash,
                &plan,
                provenance,
            ))?
        } else {
            serde_json::to_vec(&(RECIPE, &source, &hash, &plan))?
        };
        let id = format!("{:x}", Sha256::digest(identity));
        let destination = self.dir.join("installations").join(&id);
        let review_token = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(&id, &destination, &plan))?)
        );
        let now = marquee_media::time::now_rfc3339();
        let job = Job {
            torrent_source,
            browser_source,
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
        Ok(job)
    }

    /// A browser completion is not an install. Recheck the exact retained bytes,
    /// bind the explicit platform review once, then require normal approval.
    pub fn inspect_browser(&self, browser_id: &str, system: &str) -> Result<Job> {
        let _guard = self.claim()?;
        let mut conn = self.browser_database()?;
        let browser = self.completed_browser(browser_id)?;
        if browser
            .selection
            .link_type
            .as_deref()
            .is_some_and(|kind| kind != "base")
        {
            return Err(error(
                "BROWSER_PREPARATION",
                "Updates and DLC need their own supported preparation workflow",
            ));
        }
        let linked: Option<String> = conn
            .query_row(
                "SELECT preparation_id FROM browser_preparations WHERE browser_job_id=?1",
                [browser_id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = &linked {
            let job = self.get(id)?;
            if job.plan.system != system {
                return Err(error(
                    "BROWSER_PREPARATION",
                    "This download already has a platform review; its existing job was kept",
                ));
            }
            if job.receipt.is_some() || job.state == State::Preparing {
                return Ok(job);
            }
        }
        let source = browser.source()?;
        let evidence = crate::browser::artifact(
            &source.artifact.filename,
            &browser.download_root,
            source.artifact.bytes,
        )?;
        if evidence.sha256 != source.artifact.sha256 {
            return Err(error(
                "BROWSER_SOURCE_CHANGED",
                "Downloaded archive changed since completion; no preparation was created",
            ));
        }
        let mut job = self.build_review(&evidence.filename, system, Some(source.clone()), None)?;
        if job.source_sha256 != source.artifact.sha256 || job.source_size != source.artifact.bytes {
            return Err(error(
                "BROWSER_SOURCE_CHANGED",
                "Downloaded archive changed during inspection; no preparation was created",
            ));
        }
        let checked = crate::browser::artifact(
            &source.artifact.filename,
            &browser.download_root,
            source.artifact.bytes,
        )?;
        if checked.sha256 != source.artifact.sha256 {
            return Err(error(
                "BROWSER_SOURCE_CHANGED",
                "Downloaded archive changed during inspection",
            ));
        }
        if linked.as_ref().is_some_and(|id| *id != job.id) {
            return Err(error(
                "BROWSER_PREPARATION",
                "The saved preparation review differs; existing records were kept",
            ));
        }
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let prior: Option<String> = tx
            .query_row("SELECT payload FROM jobs WHERE id=?1", [&job.id], |r| {
                r.get(0)
            })
            .optional()?;
        if let Some(prior) = prior {
            job = serde_json::from_str(&prior)?;
            if job.browser_source.as_ref() != Some(&source) {
                return Err(error(
                    "BROWSER_PREPARATION",
                    "Preparation provenance does not match this download",
                ));
            }
            if job.state == State::Cancelled {
                job.state = State::AwaitingReview;
                job.error = None;
                job.error_code = None;
            }
        }
        job.updated_at = marquee_media::time::now_rfc3339();
        tx.execute("INSERT INTO jobs(id,payload) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload", params![job.id,serde_json::to_string(&job)?])?;
        tx.execute("INSERT INTO browser_preparations(browser_job_id,preparation_id) VALUES(?1,?2) ON CONFLICT(browser_job_id) DO NOTHING", params![browser_id,job.id])?;
        tx.commit()?;
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
                browser_source: job.browser_source.clone(),
                torrent_source: job.torrent_source.clone(),
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

    pub(crate) fn execute(&self, id: &str, token: &str) -> Result<Job> {
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
        if self.torrent_source.as_ref().is_some_and(|s| {
            !s.valid() || s.artifact.sha256 != self.source_sha256 || self.browser_source.is_some()
        }) {
            return Err(error(
                "PREPARATION_INVALID",
                "Receipt torrent-source evidence is inconsistent",
            ));
        }
        if self.browser_source.as_ref().is_some_and(|s| {
            s.artifact.sha256 != self.source_sha256
                || s.artifact.content_verified
                || s.selection
                    .link_type
                    .as_deref()
                    .is_some_and(|kind| kind != "base")
        }) {
            return Err(error(
                "PREPARATION_INVALID",
                "Receipt browser-source evidence is inconsistent",
            ));
        }
        if !fs::symlink_metadata(root)?.is_dir() {
            return Err(error(
                "PREPARATION_INVALID",
                "Prepared directory is not a regular directory",
            ));
        }
        let canonical = fs::canonicalize(root)?;
        for output in &self.files {
            let path = root.join(&output.path);
            if self.system == "xbox360" {
                marquee_media::xbox360::inspect(&path)?;
            }
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

    fn completed(manager: &Manager, source: &Path, kind: Option<&str>) -> String {
        use crate::browser::{Request, Selection};
        let selected: Selection = if let Some(kind) = kind {
            serde_json::from_value(serde_json::json!({"provider":"axekin","game_id":"12","title":"Fixture","source_url":"https://www.axekin.com/games/fixture","mirror_url":"https://mirror.test/12","link_type":kind})).unwrap()
        } else {
            serde_json::from_value(serde_json::json!({"provider":"vimm","game_id":"1","media_id":"2","title":"Fixture","source_url":"https://vimm.net/vault/1","format":"0","disc":""})).unwrap()
        };
        let client = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let id = uuid::Uuid::new_v4().to_string();
        manager
            .browser_request(
                client,
                Request::Start {
                    id: id.clone(),
                    selection: selected,
                    download_root: source.parent().unwrap().into(),
                },
            )
            .unwrap();
        manager
            .browser_request(
                client,
                Request::Bind {
                    job_id: id.clone(),
                    download_id: 7,
                    url: "https://cdn.test/fixture.zip".into(),
                    referrer: String::new(),
                    confirmed: true,
                },
            )
            .unwrap();
        manager
            .browser_request(
                client,
                Request::Complete {
                    job_id: id.clone(),
                    download_id: 7,
                    filename: source.into(),
                    bytes: fs::metadata(source).unwrap().len(),
                },
            )
            .unwrap();
        id
    }

    #[test]
    fn v2_and_v3_upgrade_back_up_and_preserves_payloads_before_retiring_old_writers() {
        use std::os::unix::fs::PermissionsExt;
        for version in [2, 3] {
            let temp = tempfile::tempdir().unwrap();
            let manager = Manager::new(temp.path());
            let path = zip(temp.path(), "source.zip", &[("Game.gen", b"fixture")]);
            let review = manager.inspect(&path, "genesis").unwrap();
            let conn = manager.database().unwrap();
            let before: String = conn
                .query_row("SELECT payload FROM jobs WHERE id=?1", [&review.id], |r| {
                    r.get(0)
                })
                .unwrap();
            conn.pragma_update(None, "user_version", version).unwrap();
            drop(conn);
            let conn = manager.database().unwrap();
            assert_eq!(
                conn.query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
                    .unwrap(),
                4
            );
            let after: String = conn
                .query_row("SELECT payload FROM jobs WHERE id=?1", [&review.id], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(before, after);
            let backups: Vec<_> = fs::read_dir(temp.path().join("manager"))
                .unwrap()
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with(&format!("jobs-v{version}-backup-"))
                })
                .collect();
            assert_eq!(backups.len(), 1);
            assert_eq!(
                fs::metadata(&backups[0]).unwrap().permissions().mode() & 0o777,
                0o600
            );
            let backup = Connection::open(&backups[0]).unwrap();
            assert_eq!(
                backup
                    .query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
                    .unwrap(),
                version
            );
            assert_eq!(
                before,
                backup
                    .query_row("SELECT payload FROM jobs WHERE id=?1", [&review.id], |r| {
                        r.get::<_, String>(0)
                    })
                    .unwrap()
            );
        }
    }

    #[test]
    fn browser_review_links_atomically_replays_and_retains_immutable_provenance() {
        let temp = tempfile::tempdir().unwrap();
        let manager = Arc::new(Manager::new(temp.path()));
        let path = zip(temp.path(), "source.zip", &[("Game.gen", b"fixture")]);
        let browser_id = completed(&manager, &path, None);
        // Inject failure between saving review and linking it: both must roll back.
        manager.browser_database().unwrap().execute_batch("CREATE TRIGGER fail_browser_link BEFORE INSERT ON browser_preparations BEGIN SELECT RAISE(ABORT,'injected link failure'); END;").unwrap();
        assert!(manager.inspect_browser(&browser_id, "genesis").is_err());
        assert!(manager.list().unwrap().is_empty());
        assert!(manager.browser_jobs().unwrap()[0].preparation_id.is_none());
        manager
            .browser_database()
            .unwrap()
            .execute_batch("DROP TRIGGER fail_browser_link")
            .unwrap();
        let review = manager.inspect_browser(&browser_id, "genesis").unwrap();
        let repeated = manager.inspect_browser(&browser_id, "genesis").unwrap();
        assert_eq!(review.id, repeated.id);
        assert_eq!(manager.list().unwrap().len(), 1);
        assert_eq!(review.state, State::AwaitingReview);
        assert!(!review.destination.exists());
        assert!(manager.inspect_browser(&browser_id, "ds").is_err());
        let provenance = review.browser_source.as_ref().unwrap();
        assert_eq!(provenance.browser_job_id, browser_id);
        assert_eq!(provenance.artifact.sha256, review.source_sha256);
        let mut prepared = manager.execute(&review.id, &review.review_token).unwrap();
        assert_eq!(prepared.state, State::AwaitingLibrary);
        assert_eq!(
            prepared.receipt.as_ref().unwrap().browser_source,
            review.browser_source
        );
        let receipt_json = serde_json::to_string(prepared.receipt.as_ref().unwrap()).unwrap();
        fs::remove_file(&path).unwrap();
        let reopened = Manager::new(temp.path());
        assert_eq!(
            reopened.inspect_browser(&browser_id, "genesis").unwrap().id,
            review.id
        );
        assert_eq!(
            reopened.browser_jobs().unwrap()[0].source_available,
            Some(false)
        );
        assert_eq!(
            reopened.browser_jobs().unwrap()[0]
                .preparation_id
                .as_deref(),
            Some(review.id.as_str())
        );
        let fingerprint = prepared.receipt.as_ref().unwrap().fingerprint().unwrap();
        prepared = reopened
            .acknowledge(&review.id, &fingerprint, vec!["edition".into()])
            .unwrap();
        assert_eq!(prepared.state, State::Registered);
        assert_eq!(
            serde_json::to_string(prepared.receipt.as_ref().unwrap()).unwrap(),
            receipt_json
        );
        assert_eq!(prepared.attempt, 1);
    }

    #[test]
    fn browser_changed_archive_and_update_dlc_never_create_preparation_jobs() {
        let temp = tempfile::tempdir().unwrap();
        let manager = Manager::new(temp.path());
        let path = zip(temp.path(), "source.zip", &[("Game.gen", b"fixture")]);
        let browser_id = completed(&manager, &path, None);
        let mut bytes = fs::read(&path).unwrap();
        bytes[0] ^= 1;
        fs::write(&path, &bytes).unwrap();
        assert_eq!(
            manager
                .inspect_browser(&browser_id, "genesis")
                .unwrap_err()
                .code,
            "BROWSER_SOURCE_CHANGED"
        );
        assert!(manager.list().unwrap().is_empty());
        for kind in ["update", "dlc"] {
            let path = zip(
                temp.path(),
                &format!("{kind}.zip"),
                &[("Game.gen", b"fixture")],
            );
            let id = completed(&manager, &path, Some(kind));
            assert_eq!(
                manager.inspect_browser(&id, "genesis").unwrap_err().code,
                "BROWSER_PREPARATION"
            );
        }
        assert!(manager.list().unwrap().is_empty());
    }

    #[test]
    fn legacy_review_and_receipt_serialization_keep_their_original_identity() {
        let temp = tempfile::tempdir().unwrap();
        let manager = Manager::new(temp.path());
        let source = zip(temp.path(), "source.zip", &[("Game.gen", b"fixture")]);
        let review = manager.inspect(&source, "genesis").unwrap();
        let expected = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&(RECIPE, &review.source, &review.source_sha256, &review.plan))
                    .unwrap()
            )
        );
        assert_eq!(review.id, expected);
        assert!(!serde_json::to_value(&review)
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("browser_source"));
        let prepared = manager.execute(&review.id, &review.review_token).unwrap();
        let value = serde_json::to_value(prepared.receipt.unwrap()).unwrap();
        assert!(!value.as_object().unwrap().contains_key("browser_source"));
    }

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
