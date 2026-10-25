//! Detached native-engine jobs. Only shipped recipes run; no library database access.
use crate::{port_engine, service::Manager, AppError, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    List,
    Start { id: String },
    Cancel { id: String },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub attempt_id: String,
    pub port_id: String,
    pub recipe_sha256: String,
    pub installed: port_engine::InstalledPort,
    pub binary_sha256: String,
    pub binary_bytes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub attempt_id: String,
    pub state: String,
    pub message: String,
    pub error_code: Option<String>,
    pub receipt: Option<Receipt>,
}
struct Active {
    id: String,
    cancel: bool,
    publishing: bool,
}
pub struct Host {
    data: PathBuf,
    manager: Arc<Manager>,
    active: Mutex<Option<Active>>,
}
impl Host {
    pub fn new(data: &Path, manager: Arc<Manager>) -> Result<Arc<Self>> {
        let host = Arc::new(Self {
            data: data.into(),
            manager,
            active: Mutex::new(None),
        });
        host.manager.database()?.execute_batch("CREATE TABLE IF NOT EXISTS native_port_jobs(id TEXT PRIMARY KEY, payload TEXT NOT NULL); CREATE TABLE IF NOT EXISTS native_port_receipts(attempt_id TEXT PRIMARY KEY, payload TEXT NOT NULL);")?;
        // Called once under the daemon lease, never by a snapshot reader.
        for mut job in host.list()? {
            if job.state == "installing" {
                job.state = "interrupted".into();
                job.message = "Engine installation interrupted. Retry installation to recover; game files are retained.".into();
                job.error_code = Some("PORT_INTERRUPTED".into());
                host.save(&job)?;
            }
        }
        Ok(host)
    }
    fn save(&self, job: &Job) -> Result<()> {
        self.manager.database()?.execute("INSERT INTO native_port_jobs(id,payload) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload", params![job.id, serde_json::to_string(job)?])?;
        Ok(())
    }
    pub fn list(&self) -> Result<Vec<Job>> {
        let db = self.manager.database()?;
        let mut q = db.prepare("SELECT payload FROM native_port_jobs ORDER BY id")?;
        let jobs = q
            .query_map([], |r| r.get::<_, String>(0))?
            .map(|r| Ok(serde_json::from_str(&r?)?))
            .collect();
        jobs
    }
    pub fn active(&self) -> bool {
        self.active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
    }
    pub fn dispatch(self: &Arc<Self>, request: Request) -> Result<Vec<Job>> {
        match request {
            Request::List => self.list(),
            Request::Start { id } => {
                // Resolve inside authority. No client-provided recipe, URL, executable or destination.
                port_engine::find(&id)?;
                let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(existing) = active.as_ref() {
                    if existing.id == id {
                        return self.list();
                    }
                    return Err(AppError::new(
                        "INSTALL_BUSY",
                        "Another native engine is installing",
                    ));
                }
                let previous = self
                    .list()?
                    .into_iter()
                    .find(|j| j.id == id)
                    .and_then(|j| j.receipt);
                let job = Job {
                    id: id.clone(),
                    attempt_id: uuid::Uuid::new_v4().to_string(),
                    state: "installing".into(),
                    message: "Starting engine installation…".into(),
                    error_code: None,
                    receipt: previous,
                };
                self.save(&job)?;
                *active = Some(Active {
                    id,
                    cancel: false,
                    publishing: false,
                });
                let host = self.clone();
                let mut failed = job.clone();
                if let Err(error) = std::thread::Builder::new()
                    .name("native-engine".into())
                    .spawn(move || host.execute(job))
                {
                    *active = None;
                    failed.state = "failed".into();
                    failed.message = error.to_string();
                    failed.error_code = Some("IO".into());
                    self.save(&failed)?;
                    return Err(error.into());
                }
                drop(active);
                self.list()
            }
            Request::Cancel { id } => {
                let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(job) = active.as_mut().filter(|j| j.id == id) {
                    if job.publishing {
                        return Err(AppError::new(
                            "PORT_PUBLISHING",
                            "Engine publication has started; wait for completion",
                        ));
                    }
                    job.cancel = true;
                }
                self.list()
            }
        }
    }
    fn checkpoint(&self, job: &mut Job, message: &str) -> Result<()> {
        let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        let control = active
            .as_mut()
            .ok_or_else(|| AppError::new("PORT_INTERRUPTED", "Native worker lost ownership"))?;
        if control.cancel {
            return Err(AppError::new(
                "PORT_CANCELLED",
                "Engine installation cancelled; existing game files are retained",
            ));
        }
        if message == "Publishing engine…" {
            control.publishing = true;
        }
        if !message.is_empty() && job.message != message {
            job.message = message.into();
            self.save(job)?;
        }
        Ok(())
    }
    fn execute(&self, mut job: Job) {
        let result = (|| {
            let def = port_engine::find(&job.id)?;
            let recipe_sha256 = format!("{:x}", Sha256::digest(serde_json::to_vec(&def)?));
            let installed = port_engine::install_latest(&self.data, &def, None, &mut |m| {
                self.checkpoint(&mut job, m)
            })?;
            let bin = port_engine::port_dir(&self.data, &def.id).join(&installed.bin);
            let meta = std::fs::symlink_metadata(&bin)?;
            if !meta.is_file()
                || meta.file_type().is_symlink()
                || meta.len() > marquee_media::http::PORT_MAX_BYTES
            {
                return Err(AppError::new("PORT_OUTPUT", "Invalid engine output"));
            }
            let mut hash = Sha256::new();
            let mut file = std::fs::File::open(&bin)?.take(marquee_media::http::PORT_MAX_BYTES + 1);
            let bytes = std::io::copy(&mut file, &mut hash)?;
            if bytes != meta.len() {
                return Err(AppError::new(
                    "PORT_OUTPUT",
                    "Engine changed during verification",
                ));
            }
            let receipt = Receipt {
                attempt_id: job.attempt_id.clone(),
                port_id: job.id.clone(),
                recipe_sha256,
                installed,
                binary_sha256: format!("{:x}", hash.finalize()),
                binary_bytes: bytes,
            };
            job.state = "installed".into();
            job.message = "Engine installed · game setup unchecked".into();
            job.receipt = Some(receipt.clone());
            let mut db = self.manager.database()?;
            let tx = db.transaction()?;
            tx.execute(
                "INSERT INTO native_port_receipts(attempt_id,payload) VALUES(?1,?2)",
                params![receipt.attempt_id, serde_json::to_string(&receipt)?],
            )?;
            tx.execute(
                "UPDATE native_port_jobs SET payload=?2 WHERE id=?1",
                params![job.id, serde_json::to_string(&job)?],
            )?;
            tx.commit()?;
            Ok::<_, AppError>(())
        })();
        if let Err(error) = result {
            job.state = if error.code == "PORT_CANCELLED" {
                "cancelled"
            } else {
                "failed"
            }
            .into();
            job.message = error.message;
            job.error_code = Some(error.code);
            if let Err(error) = self.save(&job) {
                eprintln!("Native engine checkpoint failed: {}", error.message);
            }
        }
        *self.active.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// The library owner rechecks the exact output before publishing a completed attempt.
pub fn verify_receipt(data: &Path, job: &Job) -> Result<()> {
    let receipt = job
        .receipt
        .as_ref()
        .ok_or_else(|| AppError::new("PORT_RECEIPT", "Missing engine receipt"))?;
    let def = port_engine::find(&job.id)?;
    if receipt.port_id != job.id
        || receipt.attempt_id.is_empty()
        || receipt.installed.bin.is_empty()
    {
        return Err(AppError::new(
            "PORT_RECEIPT",
            "Engine receipt identity does not match",
        ));
    }
    let relative = Path::new(&receipt.installed.bin);
    if relative
        .components()
        .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(AppError::new("PORT_RECEIPT", "Invalid executable path"));
    }
    let mut current = data.to_path_buf();
    for part in Path::new("ports").join(&def.id).join(relative).components() {
        current.push(part);
        if std::fs::symlink_metadata(&current)?
            .file_type()
            .is_symlink()
        {
            return Err(AppError::new("PORT_RECEIPT", "Engine path contains a link"));
        }
    }
    let meta = std::fs::symlink_metadata(&current)?;
    if !meta.is_file()
        || meta.len() != receipt.binary_bytes
        || meta.len() > marquee_media::http::PORT_MAX_BYTES
    {
        return Err(AppError::new("PORT_RECEIPT", "Engine output size changed"));
    }
    let mut hash = Sha256::new();
    std::io::copy(
        &mut std::fs::File::open(current)?.take(marquee_media::http::PORT_MAX_BYTES + 1),
        &mut hash,
    )?;
    if format!("{:x}", hash.finalize()) != receipt.binary_sha256 {
        return Err(AppError::new(
            "PORT_RECEIPT",
            "Engine output checksum changed",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn host(data: &Path) -> Arc<Host> {
        Host::new(data, Arc::new(Manager::new(data))).unwrap()
    }
    fn pending() -> Job {
        Job {
            id: "2ship2harkinian".into(),
            attempt_id: "attempt".into(),
            state: "installing".into(),
            message: "Downloading…".into(),
            error_code: None,
            receipt: None,
        }
    }
    #[test]
    fn restart_recovers_checkpoints_without_creating_a_library_or_running_an_engine() {
        let dir = tempfile::tempdir().unwrap();
        let old = host(dir.path());
        old.save(&pending()).unwrap();
        drop(old);
        let recovered = host(dir.path());
        let jobs = recovered.list().unwrap();
        assert_eq!(jobs[0].state, "interrupted");
        assert_eq!(jobs[0].attempt_id, "attempt");
        assert!(!dir.path().join("library.db").exists());
        assert!(!dir.path().join("ports").exists());
        assert_eq!(
            recovered
                .dispatch(Request::Start {
                    id: "../custom-command".into()
                })
                .unwrap_err()
                .code,
            "PORT_UNKNOWN"
        );
    }
    #[test]
    fn cancellation_stops_before_publication_but_cannot_interrupt_the_commit() {
        let dir = tempfile::tempdir().unwrap();
        let host = host(dir.path());
        let mut job = pending();
        host.save(&job).unwrap();
        *host.active.lock().unwrap() = Some(Active {
            id: job.id.clone(),
            cancel: false,
            publishing: false,
        });
        host.dispatch(Request::Cancel { id: job.id.clone() })
            .unwrap();
        assert_eq!(
            host.checkpoint(&mut job, "Unpacking…").unwrap_err().code,
            "PORT_CANCELLED"
        );
        host.active.lock().unwrap().as_mut().unwrap().cancel = false;
        host.checkpoint(&mut job, "Publishing engine…").unwrap();
        assert_eq!(
            host.dispatch(Request::Cancel { id: job.id.clone() })
                .unwrap_err()
                .code,
            "PORT_PUBLISHING"
        );
        assert!(host.active());
    }
    #[test]
    fn receipt_checks_exact_binary_and_rejects_paths_links_and_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let engine = dir.path().join("ports/2ship2harkinian");
        std::fs::create_dir_all(&engine).unwrap();
        std::fs::write(engine.join("engine"), b"test binary").unwrap();
        let mut job = pending();
        job.state = "installed".into();
        job.receipt = Some(Receipt {
            attempt_id: job.attempt_id.clone(),
            port_id: job.id.clone(),
            recipe_sha256: "test".into(),
            installed: port_engine::InstalledPort {
                tag: "test".into(),
                bin: "engine".into(),
            },
            binary_sha256: format!("{:x}", Sha256::digest(b"test binary")),
            binary_bytes: 11,
        });
        verify_receipt(dir.path(), &job).unwrap();
        let mut changed = job.clone();
        changed.receipt.as_mut().unwrap().installed.bin = "../engine".into();
        assert!(verify_receipt(dir.path(), &changed).is_err());
        std::fs::write(engine.join("engine"), b"bad! binary").unwrap();
        assert!(verify_receipt(dir.path(), &job).is_err());
        std::fs::remove_file(engine.join("engine")).unwrap();
        std::os::unix::fs::symlink("/dev/null", engine.join("engine")).unwrap();
        assert!(verify_receipt(dir.path(), &job).is_err());
    }
}
