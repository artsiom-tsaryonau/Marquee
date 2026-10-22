//! Manager client and library-only receipt publication.
use crate::error::{AppError, Result};
use crate::model::CoreEvent;
use crate::sessions::EventSink;
use crate::Core;
pub use marquee_manager::{Job, Receipt, State};
use rusqlite::{params, OptionalExtension};
use std::path::Path;
fn error(code: &str, message: &str) -> AppError {
    AppError::new(code, message)
}
use std::sync::Arc;

impl Core {
    pub fn inspect_preparation(&self, source: &Path, system: &str) -> Result<Job> {
        self.preparation.inspect(source, system)
    }
    pub fn cancel_preparation(&self, id: &str) -> Result<Job> {
        self.preparation.cancel(id)
    }
    pub fn start_preparation(self: &Arc<Self>, id: &str, token: &str) -> Result<Job> {
        self.preparation.start_job(id, token)
    }

    fn reconcile_preparation(&self, job: Job) -> Result<Job> {
        if job.state != State::AwaitingLibrary {
            return Ok(job);
        }
        let receipt = job
            .receipt
            .as_ref()
            .ok_or_else(|| AppError::new("PREPARATION_STATE", "Prepared job has no receipt"))?;
        let fingerprint = receipt.fingerprint()?;
        match self.publish_preparation(receipt) {
            Ok(ids) => {
                let job = self.preparation.acknowledge(&job.id, &fingerprint, ids)?;
                self.events.emit(CoreEvent::LibraryChanged);
                Ok(job)
            }
            Err(err) => {
                self.preparation
                    .publication_failed(&job.id, &fingerprint, &err.code, &err.message)
            }
        }
    }

    pub fn preparation_jobs(&self) -> Result<Vec<Job>> {
        self.preparation
            .list()?
            .into_iter()
            .map(|job| self.reconcile_preparation(job))
            .collect()
    }

    pub fn run_preparation(&self, id: &str, token: &str) -> Result<Job> {
        let mut job = self.preparation.start_job(id, token)?;
        while job.state == State::Preparing {
            std::thread::sleep(std::time::Duration::from_millis(50));
            job = self.preparation.get(id)?;
        }
        self.reconcile_preparation(job)
    }

    fn publish_preparation(&self, receipt: &Receipt) -> Result<Vec<String>> {
        receipt.verify(&receipt.destination)?;
        self.with_db(|db| {
            let tx = db.conn.unchecked_transaction()?;
            let payload = serde_json::to_string(receipt)?;
            let saved: Option<(String, String)> = db
                .conn
                .query_row(
                    "SELECT payload,edition_ids FROM preparation_receipts WHERE id=?1",
                    [&receipt.id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            if let Some((prior, ids)) = saved {
                if prior != payload {
                    return Err(error("PREPARATION_CONFLICT", "Receipt identity changed"));
                }
                return Ok(serde_json::from_str(&ids)?);
            }
            let root = crate::library::add_root(
                db,
                &receipt.destination,
                &receipt.system,
                Some("Prepared game"),
            )?;
            crate::library::scan_root(db, Some(&root.id))?;
            let media = crate::library::snapshot_media(db)?;
            let mut ids = Vec::new();
            for file in &receipt.files {
                if let Some(item) = media
                    .iter()
                    .find(|m| m.path == receipt.destination.join(&file.path))
                {
                    if !item.available || item.skip_reason.is_some() {
                        return Err(AppError::new(
                            "PREPARATION_NOT_READY",
                            item.skip_reason
                                .clone()
                                .unwrap_or_else(|| "Prepared media unavailable".into()),
                        ));
                    }
                    ids.push(item.edition_id.clone());
                }
            }
            ids.sort();
            ids.dedup();
            if ids.is_empty() {
                return Err(error(
                    "PREPARATION_NOT_READY",
                    "No supported library media found in prepared outputs",
                ));
            }
            db.conn.execute(
                "INSERT INTO preparation_receipts(id,payload,edition_ids) VALUES(?1,?2,?3)",
                params![receipt.id, payload, serde_json::to_string(&ids)?],
            )?;
            tx.commit()?;
            Ok(ids)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    struct Data(tempfile::TempDir);
    impl Data {
        fn new() -> Self {
            Self(tempfile::tempdir().unwrap())
        }
        fn path(&self) -> &Path {
            self.0.path()
        }
    }
    impl Drop for Data {
        fn drop(&mut self) {
            let client = marquee_manager::Client::new(&self.path().join("data"));
            let _ = client.stop();
            let until = Instant::now() + Duration::from_secs(5);
            while client.status().is_ok_and(|s| s.running) && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
    fn zip(dir: &Path, files: &[(&str, &[u8])]) -> PathBuf {
        let path = dir.join("game.zip");
        let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
        for (name, bytes) in files {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        path
    }
    fn open(data: &Path) -> Core {
        Core::open_isolated(Some(data.into()), true).unwrap()
    }

    #[test]
    fn browser_artifact_links_survive_failed_publication_source_cleanup_and_restart() {
        use marquee_manager::browser::{Request, Selection};
        let dir = Data::new();
        let data = dir.path().join("data");
        let source = zip(dir.path(), &[("Game.gen", b"synthetic cartridge")]);
        let core = open(&data);
        let client = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let browser_id = uuid::Uuid::new_v4().to_string();
        let selected: Selection=serde_json::from_value(serde_json::json!({"provider":"vimm","game_id":"1","media_id":"2","format":"0","disc":"","title":"Synthetic","source_url":"https://vimm.net/vault/1"})).unwrap();
        core.preparation
            .browser_request(
                client,
                Request::Start {
                    id: browser_id.clone(),
                    selection: selected,
                    download_root: dir.path().into(),
                },
            )
            .unwrap();
        core.preparation
            .browser_request(
                client,
                Request::Bind {
                    job_id: browser_id.clone(),
                    download_id: 7,
                    url: "https://dl3.vimm.net/fixture.zip".into(),
                    referrer: "https://vimm.net/vault/1".into(),
                    confirmed: false,
                },
            )
            .unwrap();
        core.preparation
            .browser_request(
                client,
                Request::Complete {
                    job_id: browser_id.clone(),
                    download_id: 7,
                    filename: source.clone(),
                    bytes: fs::metadata(&source).unwrap().len(),
                },
            )
            .unwrap();
        let review = core
            .preparation
            .inspect_browser(&browser_id, "genesis")
            .unwrap();
        assert_eq!(
            review.id,
            core.preparation
                .inspect_browser(&browser_id, "genesis")
                .unwrap()
                .id
        );
        core.with_db(|db| {db.conn.execute_batch("CREATE TRIGGER fail_browser_receipt BEFORE INSERT ON preparation_receipts BEGIN SELECT RAISE(ABORT,'injected publication failure'); END;")?; Ok(())}).unwrap();
        let failed = core
            .run_preparation(&review.id, &review.review_token)
            .unwrap();
        assert_eq!(failed.state, State::AwaitingLibrary);
        assert!(core.snapshot().unwrap().editions.is_empty());
        let fingerprint = failed.receipt.as_ref().unwrap().fingerprint().unwrap();
        fs::remove_file(&source).unwrap();
        core.with_db(|db| {
            db.conn.execute_batch("DROP TRIGGER fail_browser_receipt")?;
            Ok(())
        })
        .unwrap();
        core.preparation.stop().unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        while core.preparation.status().unwrap().running {
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(core);
        let core = open(&data);
        core.preparation.start_manager().unwrap();
        let registered = core.preparation_jobs().unwrap().remove(0);
        assert_eq!(registered.state, State::Registered);
        assert_eq!(registered.attempt, 1);
        assert_eq!(
            registered.receipt.as_ref().unwrap().fingerprint().unwrap(),
            fingerprint
        );
        assert_eq!(
            registered.browser_source.as_ref().unwrap().browser_job_id,
            browser_id
        );
        assert_eq!(
            core.preparation
                .inspect_browser(&browser_id, "genesis")
                .unwrap()
                .id,
            review.id
        );
        let browser = core.preparation.browser_jobs().unwrap().remove(0);
        assert_eq!(browser.source_available, Some(false));
        assert_eq!(browser.preparation_id.as_deref(), Some(review.id.as_str()));
        core.preparation_jobs().unwrap();
        assert_eq!(core.snapshot().unwrap().editions.len(), 1);
        core.with_db(|db| {
            let payload: String = db.conn.query_row(
                "SELECT payload FROM preparation_receipts WHERE id=?1",
                [&review.id],
                |r| r.get(0),
            )?;
            let receipt: Receipt = serde_json::from_str(&payload)?;
            assert_eq!(receipt.browser_source, registered.browser_source);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn detached_worker_prepares_without_core_and_reconnect_publishes_once() {
        let dir = Data::new();
        let data = dir.path().join("data");
        let source = zip(dir.path(), &[("Original/Game.gen", b"cartridge bytes")]);
        let core = Arc::new(open(&data));
        let review = core.inspect_preparation(&source, "genesis").unwrap();
        assert_eq!(review.state, State::AwaitingReview);
        assert!(!review.destination.exists());
        assert_eq!(
            core.run_preparation(&review.id, "stale").unwrap_err().code,
            "PREPARATION_REVIEW"
        );
        let job = core
            .start_preparation(&review.id, &review.review_token)
            .unwrap();
        assert_eq!(job.state, State::Preparing);
        let pid = core.preparation.status().unwrap().pid;
        drop(core);
        let client = marquee_manager::Client::new(&data);
        let until = Instant::now() + Duration::from_secs(5);
        while client.get(&review.id).unwrap().state == State::Preparing {
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(client.status().unwrap().pid, pid);
        assert_eq!(
            client.get(&review.id).unwrap().state,
            State::AwaitingLibrary
        );
        // The worker neither needs nor takes the launcher's data-directory lock.
        let core = open(&data);
        let jobs = core.preparation_jobs().unwrap();
        assert_eq!(jobs[0].state, State::Registered);
        assert_eq!(jobs[0].attempt, 1);
        let id = jobs[0].edition_ids[0].clone();
        core.set_favourite(&id, true).unwrap();
        assert_eq!(
            core.preparation_jobs().unwrap()[0].edition_ids,
            jobs[0].edition_ids
        );
        assert!(core.snapshot().unwrap().editions[0].favorite);
        let _ = core.preparation.stop().unwrap();
        // Existing library operations are independent of manager availability.
        core.set_display_title(&id, "My title").unwrap();
        assert_eq!(core.snapshot().unwrap().editions.len(), 1);
        assert!(source.exists());
    }

    #[test]
    fn failed_publication_rolls_back_and_retries_without_the_archive() {
        let dir = Data::new();
        let data = dir.path().join("data");
        let source = zip(dir.path(), &[("Game.nds", b"cartridge bytes")]);
        let core = open(&data);
        let review = core.inspect_preparation(&source, "ds").unwrap();
        core.with_db(|db| { db.conn.execute_batch("CREATE TRIGGER fail_receipt BEFORE INSERT ON preparation_receipts BEGIN SELECT RAISE(ABORT,'simulated publication failure'); END;")?; Ok(()) }).unwrap();
        let failed = core
            .run_preparation(&review.id, &review.review_token)
            .unwrap();
        assert_eq!(failed.state, State::AwaitingLibrary);
        assert!(failed.destination.join("Game.nds").exists());
        assert!(core.snapshot().unwrap().roots.is_empty());
        assert!(core.snapshot().unwrap().editions.is_empty());
        fs::remove_file(&source).unwrap();
        core.with_db(|db| {
            db.conn.execute_batch("DROP TRIGGER fail_receipt")?;
            Ok(())
        })
        .unwrap();
        drop(core);
        let core = open(&data);
        let done = core
            .run_preparation(&review.id, &review.review_token)
            .unwrap();
        assert_eq!(done.state, State::Registered, "{:?}", done.error);
        assert_eq!(
            done.attempt, 1,
            "publication retry never starts another extraction"
        );
        assert_eq!(core.snapshot().unwrap().editions.len(), 1);
    }

    #[test]
    fn current_library_owner_can_adopt_m1_jobs_without_releasing_its_lease() {
        let dir = Data::new();
        let data = dir.path().join("data");
        let source = zip(dir.path(), &[("Game.gen", b"cartridge bytes")]);
        let core = open(&data);
        let fixture = marquee_manager::service::Manager::new(&data);
        let review = fixture.inspect(&source, "genesis").unwrap();
        drop(fixture);
        let conn = rusqlite::Connection::open(data.join("manager/jobs.db")).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        drop(conn);
        let retained = core.inspect_preparation(&source, "genesis").unwrap();
        assert_eq!(retained.id, review.id);
        assert_eq!(retained.review_token, review.review_token);
        assert!(core.snapshot().unwrap().editions.is_empty());
    }

    #[test]
    fn corrupted_prepared_output_never_publishes_and_unknown_platform_is_refused() {
        let dir = Data::new();
        let data = dir.path().join("data");
        let source = zip(dir.path(), &[("Game.gen", b"cartridge bytes")]);
        let core = open(&data);
        assert!(core.inspect_preparation(&source, "switch").is_err());
        let review = core.inspect_preparation(&source, "genesis").unwrap();
        core.preparation
            .start_job(&review.id, &review.review_token)
            .unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        while core.preparation.get(&review.id).unwrap().state == State::Preparing {
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(10));
        }
        fs::write(review.destination.join("Game.gen"), b"changed").unwrap();
        let jobs = core.preparation_jobs().unwrap();
        assert_eq!(jobs[0].state, State::AwaitingLibrary);
        assert_eq!(jobs[0].error_code.as_deref(), Some("PREPARATION_INVALID"));
        assert!(core.snapshot().unwrap().editions.is_empty());
    }
}
