//! Authoritative browser transport jobs. The paired native host is a worker,
//! never a library writer. No provider URL is fetched by this module.
use crate::{service::Manager, AppError, Result};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use url::Url;

const MAX_BYTES: u64 = 128 * 1024 * 1024 * 1024;
fn invalid(message: &str) -> AppError {
    AppError::new("BROWSER_REQUEST", message)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub provider: String,
    pub game_id: String,
    pub title: String,
    pub source_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mirror_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_type: Option<String>,
}
fn https(value: &str) -> Result<Url> {
    let url = Url::parse(value).map_err(|_| invalid("Expected an HTTPS URL"))?;
    if value.len() > 2048
        || value.chars().any(|c| c.is_whitespace() || c.is_control())
        || value.contains('\\')
        || url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return Err(invalid(
            "Expected an HTTPS URL without credentials or a custom port",
        ));
    }
    Ok(url)
}
fn numeric(value: &str) -> bool {
    !value.is_empty() && value.len() <= 20 && value.bytes().all(|b| b.is_ascii_digit())
}
impl Selection {
    fn validate(&self) -> Result<()> {
        if !numeric(&self.game_id) || self.title.trim().is_empty() || self.title.len() > 2000 {
            return Err(invalid("Invalid provider identity or title"));
        }
        let source = https(&self.source_url)?;
        match self.provider.as_str() {
            "vimm" => {
                if source.host_str() != Some("vimm.net")
                    || source.path().trim_end_matches('/') != format!("/vault/{}", self.game_id)
                    || !self.media_id.as_deref().is_some_and(numeric)
                    || self.format.as_ref().is_none_or(|v| v.len() > 100)
                    || self.disc.as_ref().is_none_or(|v| v.len() > 100)
                    || self.mirror_url.is_some()
                    || self.link_type.is_some()
                {
                    return Err(invalid("Invalid reviewed Vimm selection"));
                }
            }
            "axekin" => {
                let slug = source
                    .path()
                    .strip_prefix("/games/")
                    .unwrap_or("")
                    .trim_end_matches('/');
                if source.host_str() != Some("www.axekin.com")
                    || slug.is_empty()
                    || slug.len() > 128
                    || !slug
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                    || !matches!(self.link_type.as_deref(), Some("base" | "update" | "dlc"))
                    || self.media_id.is_some()
                    || self.format.is_some()
                    || self.disc.is_some()
                {
                    return Err(invalid("Invalid reviewed Axekin selection"));
                }
                https(
                    self.mirror_url
                        .as_deref()
                        .ok_or_else(|| invalid("Missing mirror URL"))?,
                )?;
            }
            _ => return Err(invalid("Unsupported browser provider")),
        }
        Ok(())
    }
    fn attributed(&self, url: &str, referrer: &str) -> Result<bool> {
        let target = https(url)?;
        let source = if self.provider == "vimm" {
            &self.source_url
        } else {
            self.mirror_url.as_ref().unwrap()
        };
        let page = https(source)?;
        let Ok(referrer) = https(referrer) else {
            return Ok(false);
        };
        let host = target.host_str().unwrap();
        Ok((if self.provider == "vimm" {
            host == "vimm.net" || host.ends_with(".vimm.net")
        } else {
            target.host_str() == page.host_str()
        }) && referrer.host_str() == page.host_str()
            && referrer.path().trim_end_matches('/') == page.path().trim_end_matches('/')
            && referrer.query() == page.query())
    }
}

/// Protocol v1 is explicit inside the existing manager envelope. Start's stable
/// request UUID is supplied before browser interaction, and is also the job ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Hello,
    Start {
        id: String,
        selection: Selection,
        download_root: PathBuf,
    },
    Status {
        job_id: String,
    },
    Bind {
        job_id: String,
        download_id: u64,
        url: String,
        referrer: String,
        confirmed: bool,
    },
    Progress {
        job_id: String,
        download_id: u64,
        bytes: u64,
        total: Option<u64>,
    },
    Complete {
        job_id: String,
        download_id: u64,
        filename: PathBuf,
        bytes: u64,
    },
    Interrupt {
        job_id: String,
        download_id: u64,
        error: String,
    },
    Cancel {
        job_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Waiting,
    Downloading,
    Completed,
    Interrupted,
    Cancelled,
}
impl State {
    fn terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Interrupted | Self::Cancelled)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Artifact {
    pub filename: PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub content_verified: bool,
}
/// Immutable provenance carried into both review and receipt. No credentials.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Source {
    pub browser_job_id: String,
    pub selection: Selection,
    pub artifact: Artifact,
    pub download_id: u64,
    pub attribution: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub job_id: String,
    pub client_id: String,
    pub selection: Selection,
    pub download_root: PathBuf,
    pub state: State,
    pub started_at: f64,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_id: Option<u64>,
    pub bytes: u64,
    pub total: Option<u64>,
    pub attribution: Option<String>,
    pub cancel_requested: bool,
    pub error: Option<String>,
    pub result: Option<Artifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preparation_id: Option<String>,
    /// Snapshot availability, not a fresh hash verification; never authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_available: Option<bool>,
}
impl Job {
    pub(crate) fn source(&self) -> Result<Source> {
        if self.state != State::Completed {
            return Err(invalid("This browser download is not complete"));
        }
        Ok(Source {
            browser_job_id: self.job_id.clone(),
            selection: self.selection.clone(),
            artifact: self
                .result
                .clone()
                .ok_or_else(|| invalid("Completed download has no byte evidence"))?,
            download_id: self
                .download_id
                .ok_or_else(|| invalid("Completed download has no browser binding"))?,
            attribution: self
                .attribution
                .clone()
                .ok_or_else(|| invalid("Completed download has no attribution"))?,
        })
    }
}
fn root(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() || !path.is_dir() || fs::symlink_metadata(path)?.file_type().is_symlink()
    {
        return Err(invalid("Choose an existing absolute download directory"));
    }
    let canonical = fs::canonicalize(path)?;
    if canonical.parent().is_none() || std::env::var_os("HOME").is_some_and(|h| canonical == h) {
        return Err(invalid("Home and filesystem root cannot be download roots"));
    }
    Ok(canonical)
}
pub(crate) fn open_completed_file(path: &Path, root: &Path, bytes: u64) -> Result<File> {
    if bytes == 0 || bytes > MAX_BYTES || !path.is_absolute() {
        return Err(invalid("Invalid completed file size or path"));
    }
    let relative = path
        .strip_prefix(root)
        .map_err(|_| invalid("Completed file is outside the approved root"))?;
    if relative
        .components()
        .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(invalid("Completed file must stay inside the approved root"));
    }
    // Walk relative to held directory descriptors; no symlink component can
    // escape the approved root while the file is being opened.
    use std::os::fd::{AsRawFd, FromRawFd};
    let mut file = File::options()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(root)?;
    let parts: Vec<_> = relative.components().collect();
    for (index, part) in parts.iter().enumerate() {
        let name = std::ffi::CString::new(part.as_os_str().as_encoded_bytes())
            .map_err(|_| invalid("Invalid media path"))?;
        let flags = libc::O_RDONLY
            | libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | libc::O_NONBLOCK
            | if index + 1 < parts.len() {
                libc::O_DIRECTORY
            } else {
                0
            };
        // SAFETY: valid held directory fd and nul-terminated single component.
        let fd = unsafe { libc::openat(file.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        // SAFETY: openat returned a new descriptor, now owned by File.
        file = unsafe { File::from_raw_fd(fd) };
    }
    if !file.metadata()?.is_file() || file.metadata()?.len() != bytes {
        return Err(invalid("Completed file is missing or changed size"));
    }
    Ok(file)
}

pub(crate) fn artifact(path: &Path, root: &Path, bytes: u64) -> Result<Artifact> {
    let suffix = path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ![
        "zip", "7z", "rar", "iso", "chd", "cue", "bin", "md", "nes", "sfc", "smc", "nds", "gba",
        "gb", "gbc", "n64", "z64", "v64", "cso", "pbp",
    ]
    .contains(&suffix.as_str())
    {
        return Err(invalid("Unsupported downloaded media format"));
    }
    let mut file = open_completed_file(path, root, bytes)?;
    let before = file.metadata()?;
    if !before.is_file() || before.len() != bytes {
        return Err(invalid("Completed file is missing or changed size"));
    }
    let mut digest = Sha256::new();
    let mut buffer = [0; 128 * 1024];
    let mut read = 0u64;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        read += count as u64;
        if read > bytes {
            return Err(invalid("Completed file grew during verification"));
        }
        digest.update(&buffer[..count]);
    }
    let after = file.metadata()?;
    if read != bytes
        || before.len() != after.len()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.mtime() != after.mtime()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
        || !fs::metadata(path).is_ok_and(|m| m.ino() == after.ino() && m.dev() == after.dev())
    {
        return Err(invalid("Completed file changed during verification"));
    }
    Ok(Artifact {
        filename: path.into(),
        bytes,
        sha256: format!("{:x}", digest.finalize()),
        content_verified: false,
    })
}

impl Manager {
    pub(crate) fn browser_database(&self) -> Result<rusqlite::Connection> {
        let db = self.database()?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS browser_jobs(id TEXT PRIMARY KEY, payload TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS browser_preparations(browser_job_id TEXT PRIMARY KEY, preparation_id TEXT NOT NULL UNIQUE)",
        )?;
        Ok(db)
    }
    pub fn browser_jobs(&self) -> Result<Vec<Job>> {
        let db = self.browser_database()?;
        let mut query = db.prepare("SELECT payload FROM browser_jobs ORDER BY rowid DESC")?;
        let mut jobs: Vec<Job> = query
            .query_map([], |r| r.get::<_, String>(0))?
            .map(|row| Ok(serde_json::from_str(&row?)?))
            .collect::<Result<_>>()?;
        for job in &mut jobs {
            job.preparation_id = db
                .query_row(
                    "SELECT preparation_id FROM browser_preparations WHERE browser_job_id=?1",
                    [&job.job_id],
                    |r| r.get(0),
                )
                .optional()?;
            job.source_available = job.result.as_ref().map(|r| {
                fs::symlink_metadata(&r.filename).is_ok_and(|m| m.is_file() && m.len() == r.bytes)
            });
        }
        Ok(jobs)
    }
    pub(crate) fn completed_browser(&self, id: &str) -> Result<Job> {
        let payload: Option<String> = self
            .browser_database()?
            .query_row("SELECT payload FROM browser_jobs WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .optional()?;
        let job: Job =
            serde_json::from_str(&payload.ok_or_else(|| invalid("Unknown browser download"))?)?;
        job.source()?;
        Ok(job)
    }
    pub fn request_browser_cancel(&self, id: &str) -> Result<Job> {
        let mut db = self.browser_database()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let payload: String =
            tx.query_row("SELECT payload FROM browser_jobs WHERE id=?1", [id], |r| {
                r.get(0)
            })?;
        let mut job: Job = serde_json::from_str(&payload)?;
        if !job.state.terminal() {
            job.cancel_requested = true;
        }
        save(&tx, &mut job)?;
        tx.commit()?;
        Ok(job)
    }
    pub fn browser_request(&self, client: &str, request: Request) -> Result<Value> {
        if client.len() != 32 || !client.bytes().all(|b| (b'a'..=b'p').contains(&b)) {
            return Err(invalid("Invalid paired extension identity"));
        }
        let mut db = self.browser_database()?;
        // Hash outside the write transaction so history/progress/cancellation
        // and preparation remain responsive for large archives. Revalidate the
        // job state inside the transaction before publishing the result.
        let verified = if let Request::Complete {
            job_id,
            download_id,
            filename,
            bytes,
        } = &request
        {
            let payload: Option<String> = db
                .query_row(
                    "SELECT payload FROM browser_jobs WHERE id=?1",
                    [job_id],
                    |r| r.get(0),
                )
                .optional()?;
            let prior: Job =
                serde_json::from_str(&payload.ok_or_else(|| invalid("Unknown browser job"))?)?;
            if prior.client_id != client || prior.download_id != Some(*download_id) {
                return Err(invalid("Download is not bound to this job"));
            }
            if prior.state == State::Completed {
                if !prior
                    .result
                    .as_ref()
                    .is_some_and(|r| r.filename == *filename && r.bytes == *bytes)
                {
                    return Err(invalid("Completion replay changed the recorded file"));
                }
                return Ok(json!({"result":prior.result}));
            }
            if prior.state != State::Downloading || prior.cancel_requested {
                return Err(invalid("Download is not active or cancellation is pending"));
            }
            Some(artifact(filename, &prior.download_root, *bytes)?)
        } else {
            None
        };
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if matches!(request, Request::Hello) {
            let payload: Option<String> = tx.query_row("SELECT payload FROM browser_jobs WHERE json_extract(payload,'$.client_id')=?1 ORDER BY rowid DESC LIMIT 1", [client], |r| r.get(0)).optional()?;
            let job = payload
                .map(|p| serde_json::from_str::<Job>(&p))
                .transpose()?;
            return Ok(json!({"protocol_version":1,"authority":"marquee-manager","job":job}));
        }
        if let Request::Start {
            id,
            selection,
            download_root,
        } = request
        {
            uuid::Uuid::parse_str(&id).map_err(|_| invalid("Start needs a stable UUID"))?;
            selection.validate()?;
            let download_root = root(&download_root)?;
            let prior: Option<String> = tx
                .query_row("SELECT payload FROM browser_jobs WHERE id=?1", [&id], |r| {
                    r.get(0)
                })
                .optional()?;
            if let Some(prior) = prior {
                let job: Job = serde_json::from_str(&prior)?;
                if job.client_id != client
                    || job.selection != selection
                    || job.download_root != download_root
                {
                    return Err(invalid("Start ID already belongs to a different selection"));
                }
                return Ok(json!({"job_id":id,"job":job}));
            }
            let count: u64 = tx.query_row("SELECT count(*) FROM browser_jobs", [], |r| r.get(0))?;
            if count >= 1000 {
                return Err(invalid(
                    "Browser history limit reached; no jobs were removed",
                ));
            }
            let active: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM browser_jobs WHERE json_extract(payload,'$.client_id')=?1 AND json_extract(payload,'$.state') IN ('waiting','downloading'))", [client], |r| r.get(0))?;
            if active {
                return Err(invalid(
                    "An existing browser job needs completion or cancellation",
                ));
            }
            let mut job = Job {
                job_id: id.clone(),
                client_id: client.into(),
                selection,
                download_root,
                state: State::Waiting,
                started_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64(),
                updated_at: String::new(),
                download_id: None,
                bytes: 0,
                total: None,
                attribution: None,
                cancel_requested: false,
                error: None,
                result: None,
                preparation_id: None,
                source_available: None,
            };
            save(&tx, &mut job)?;
            tx.commit()?;
            return Ok(json!({"job_id":id,"job":job}));
        }
        let id = match &request {
            Request::Status { job_id }
            | Request::Bind { job_id, .. }
            | Request::Progress { job_id, .. }
            | Request::Complete { job_id, .. }
            | Request::Interrupt { job_id, .. }
            | Request::Cancel { job_id } => job_id,
            _ => unreachable!(),
        };
        let payload: Option<String> = tx
            .query_row("SELECT payload FROM browser_jobs WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .optional()?;
        let mut job: Job =
            serde_json::from_str(&payload.ok_or_else(|| invalid("Unknown browser job"))?)?;
        if job.client_id != client {
            return Err(invalid("Job belongs to another paired extension"));
        }
        if matches!(request, Request::Status { .. }) {
            return Ok(json!({"job":job}));
        }
        if job.state.terminal() {
            return match request {
                Request::Complete {
                    download_id,
                    filename,
                    bytes,
                    ..
                } if job.state == State::Completed
                    && job.download_id == Some(download_id)
                    && job
                        .result
                        .as_ref()
                        .is_some_and(|r| r.filename == filename && r.bytes == bytes) =>
                {
                    Ok(json!({"result":job.result}))
                }
                Request::Cancel { .. } if job.state == State::Cancelled => {
                    Ok(json!({"state":"cancelled"}))
                }
                Request::Interrupt { download_id, .. }
                    if job.state == State::Interrupted && job.download_id == Some(download_id) =>
                {
                    Ok(json!({"state":"interrupted"}))
                }
                _ => Err(invalid("Job is terminal; start a new reviewed selection")),
            };
        }
        let bound = |id| -> Result<()> {
            if job.download_id == Some(id) && job.state == State::Downloading {
                Ok(())
            } else {
                Err(invalid("Download is not bound to this job"))
            }
        };
        let result = match request {
            Request::Bind {
                download_id,
                url,
                referrer,
                confirmed,
                ..
            } => {
                if job.cancel_requested {
                    return Err(invalid("Cancellation is pending"));
                }
                https(&url)?;
                if job.download_id.is_some_and(|old| old != download_id) {
                    return Err(invalid("Job already tracks a different download"));
                }
                if !confirmed && !job.selection.attributed(&url, &referrer)? {
                    return Err(invalid(
                        "Download cannot be attributed to reviewed selection",
                    ));
                }
                if job.download_id.is_none() {
                    job.attribution = Some(if confirmed { "manual" } else { "referrer" }.into());
                }
                job.state = State::Downloading;
                job.download_id = Some(download_id);
                json!({"state":"downloading"})
            }
            Request::Progress {
                download_id,
                bytes,
                total,
                ..
            } => {
                bound(download_id)?;
                if bytes > MAX_BYTES
                    || total.is_some_and(|t| t > MAX_BYTES || bytes > t)
                    || bytes < job.bytes
                {
                    return Err(invalid("Invalid or stale browser progress"));
                }
                job.bytes = bytes;
                job.total = total;
                json!({"state":"downloading"})
            }
            Request::Complete {
                download_id, bytes, ..
            } => {
                bound(download_id)?;
                if job.cancel_requested {
                    return Err(invalid("Cancellation is pending"));
                }
                job.bytes = bytes;
                job.total = Some(bytes);
                job.result = verified;
                job.state = State::Completed;
                json!({"result":job.result})
            }
            Request::Interrupt {
                download_id, error, ..
            } => {
                bound(download_id)?;
                job.state = State::Interrupted;
                job.error = Some(error.chars().take(1000).collect());
                json!({"state":"interrupted"})
            }
            Request::Cancel { .. } => {
                job.state = State::Cancelled;
                job.cancel_requested = false;
                json!({"state":"cancelled"})
            }
            _ => unreachable!(),
        };
        save(&tx, &mut job)?;
        tx.commit()?;
        Ok(result)
    }
}
fn save(tx: &rusqlite::Transaction<'_>, job: &mut Job) -> Result<()> {
    job.updated_at = marquee_media::time::now_rfc3339();
    tx.execute("INSERT INTO browser_jobs(id,payload) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload", params![job.job_id,serde_json::to_string(job)?])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const CLIENT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    fn chosen() -> Selection {
        serde_json::from_value(json!({"provider":"vimm","game_id":"1","media_id":"2","title":"Fixture","source_url":"https://vimm.net/vault/1","format":"0","disc":""})).unwrap()
    }
    fn start(manager: &Manager, root: &Path) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        manager
            .browser_request(
                CLIENT,
                Request::Start {
                    id: id.clone(),
                    selection: chosen(),
                    download_root: root.into(),
                },
            )
            .unwrap();
        id
    }
    fn bind(manager: &Manager, id: &str) {
        manager
            .browser_request(
                CLIENT,
                Request::Bind {
                    job_id: id.into(),
                    download_id: 7,
                    url: "https://dl3.vimm.net/file.zip".into(),
                    referrer: "https://vimm.net/vault/1".into(),
                    confirmed: false,
                },
            )
            .unwrap();
    }
    #[test]
    fn reviewed_start_replays_without_duplicates_and_rejects_conflicts() {
        let temp = tempfile::tempdir().unwrap();
        let manager = Manager::new(temp.path());
        let id = start(&manager, temp.path());
        let request = Request::Start {
            id: id.clone(),
            selection: chosen(),
            download_root: temp.path().into(),
        };
        manager.browser_request(CLIENT, request.clone()).unwrap();
        assert_eq!(manager.browser_jobs().unwrap().len(), 1);
        let mut other = chosen();
        other.media_id = Some("9".into());
        assert!(manager
            .browser_request(
                CLIENT,
                Request::Start {
                    id: id.clone(),
                    selection: other,
                    download_root: temp.path().into()
                }
            )
            .is_err());
        assert!(manager
            .browser_request(
                CLIENT,
                Request::Start {
                    id: uuid::Uuid::new_v4().to_string(),
                    selection: chosen(),
                    download_root: temp.path().into()
                }
            )
            .is_err());
        assert!(manager
            .browser_request(
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                Request::Status { job_id: id.clone() }
            )
            .is_err());
        manager
            .browser_request(CLIENT, Request::Cancel { job_id: id.clone() })
            .unwrap();
        manager
            .browser_request(CLIENT, Request::Cancel { job_id: id })
            .unwrap();
        start(&manager, temp.path());
    }
    #[test]
    fn attribution_progress_completion_and_history_survive_reconnect() {
        let temp = tempfile::tempdir().unwrap();
        let manager = Manager::new(temp.path());
        let id = start(&manager, temp.path());
        assert!(manager
            .browser_request(
                CLIENT,
                Request::Bind {
                    job_id: id.clone(),
                    download_id: 7,
                    url: "https://vimm.net.evil.test/file.zip".into(),
                    referrer: "https://vimm.net/vault/1".into(),
                    confirmed: false
                }
            )
            .is_err());
        bind(&manager, &id);
        manager
            .browser_request(
                CLIENT,
                Request::Progress {
                    job_id: id.clone(),
                    download_id: 7,
                    bytes: 3,
                    total: Some(7),
                },
            )
            .unwrap();
        assert!(manager
            .browser_request(
                CLIENT,
                Request::Progress {
                    job_id: id.clone(),
                    download_id: 7,
                    bytes: 2,
                    total: Some(7)
                }
            )
            .is_err());
        let file = temp.path().join("fixture.rar");
        fs::write(&file, b"payload").unwrap();
        let complete = Request::Complete {
            job_id: id.clone(),
            download_id: 7,
            filename: file.clone(),
            bytes: 7,
        };
        let result = manager.browser_request(CLIENT, complete.clone()).unwrap();
        assert_eq!(
            result["result"]["sha256"],
            format!("{:x}", Sha256::digest(b"payload"))
        );
        assert_eq!(result["result"]["content_verified"], false);
        fs::remove_file(&file).unwrap();
        let reopened = Manager::new(temp.path());
        assert_eq!(reopened.browser_request(CLIENT, complete).unwrap(), result);
        assert_eq!(reopened.browser_jobs().unwrap()[0].state, State::Completed);
        assert!(!temp.path().join("library.db").exists());
    }
    #[test]
    fn cancellation_requires_browser_ack_and_blocks_late_completion() {
        let temp = tempfile::tempdir().unwrap();
        let manager = Manager::new(temp.path());
        let id = start(&manager, temp.path());
        bind(&manager, &id);
        let job = manager.request_browser_cancel(&id).unwrap();
        assert!(job.cancel_requested);
        assert_eq!(job.state, State::Downloading);
        let file = temp.path().join("fixture.zip");
        fs::write(&file, b"payload").unwrap();
        assert!(manager
            .browser_request(
                CLIENT,
                Request::Complete {
                    job_id: id.clone(),
                    download_id: 7,
                    filename: file,
                    bytes: 7
                }
            )
            .is_err());
        let remote = manager.browser_request(CLIENT, Request::Hello).unwrap();
        assert_eq!(remote["job"]["cancel_requested"], true);
        manager
            .browser_request(CLIENT, Request::Cancel { job_id: id })
            .unwrap();
        assert_eq!(manager.browser_jobs().unwrap()[0].state, State::Cancelled);
    }
    #[test]
    fn files_must_be_regular_unchanged_and_beneath_root_without_links() {
        let temp = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let file = temp.path().join("file.zip");
        fs::write(&file, b"payload").unwrap();
        assert!(artifact(&file, temp.path(), 8).is_err());
        assert!(artifact(&file, other.path(), 7).is_err());
        let linked = temp.path().join("linked.zip");
        std::os::unix::fs::symlink(&file, &linked).unwrap();
        assert!(artifact(&linked, temp.path(), 7).is_err());
        let dir = temp.path().join("linked-dir");
        std::os::unix::fs::symlink(other.path(), &dir).unwrap();
        fs::write(other.path().join("outside.zip"), b"payload").unwrap();
        assert!(artifact(&dir.join("outside.zip"), temp.path(), 7).is_err());
        assert!(artifact(&temp.path().join("../outside.zip"), temp.path(), 7).is_err());
        assert!(artifact(temp.path(), temp.path(), 7).is_err());
        assert!(root(Path::new("/")).is_err());
    }
    #[test]
    fn axekin_manual_attachment_and_interruption_are_truthful() {
        let temp = tempfile::tempdir().unwrap();
        let manager = Manager::new(temp.path());
        let selected: Selection = serde_json::from_value(json!({"provider":"axekin","game_id":"12","title":"Fixture","source_url":"https://www.axekin.com/games/fixture","mirror_url":"https://mirror.test/page/12","link_type":"base"})).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        manager
            .browser_request(
                CLIENT,
                Request::Start {
                    id: id.clone(),
                    selection: selected,
                    download_root: temp.path().into(),
                },
            )
            .unwrap();
        let binding = Request::Bind {
            job_id: id.clone(),
            download_id: 7,
            url: "https://cdn.test/file.zip".into(),
            referrer: String::new(),
            confirmed: true,
        };
        manager.browser_request(CLIENT, binding).unwrap();
        let request = Request::Interrupt {
            job_id: id.clone(),
            download_id: 7,
            error: "NETWORK_FAILED".into(),
        };
        manager.browser_request(CLIENT, request.clone()).unwrap();
        manager.browser_request(CLIENT, request).unwrap();
        let job = &manager.browser_jobs().unwrap()[0];
        assert_eq!(job.state, State::Interrupted);
        assert!(job.result.is_none());
        assert_eq!(job.attribution.as_deref(), Some("manual"));
    }
}
