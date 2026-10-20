pub mod catalog;
pub mod db;
pub mod error;
pub mod instance;
pub mod iso;
pub mod library;
pub mod model;
pub mod paths;
pub mod runners;
pub mod sessions;

use crate::db::Db;
use crate::error::{AppError, Result};
use crate::instance::InstanceLock;
use crate::model::{CoreEvent, LibraryRoot, LibrarySnapshot, Runner, ScanReport, Session};
use crate::paths::Paths;
use crate::sessions::EventSink;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct Core {
    pub paths: Paths,
    db: Arc<Mutex<Db>>,
    _lock: InstanceLock,
    allow_fake: bool,
}

impl Core {
    pub fn open(data_dir: Option<PathBuf>, allow_fake: bool) -> Result<Self> {
        let paths = Paths::resolve(data_dir)?;
        let lock = InstanceLock::acquire(&paths.instance_lock())?;
        let db = Db::open(&paths)?;
        let core = Self {
            paths,
            db: Arc::new(Mutex::new(db)),
            _lock: lock,
            allow_fake,
        };
        core.refresh_runners()?;
        Ok(core)
    }

    fn with_db<T>(&self, f: impl FnOnce(&Db) -> Result<T>) -> Result<T> {
        let db = self
            .db
            .lock()
            .map_err(|_| AppError::new("DB", "database lock poisoned"))?;
        f(&db)
    }

    pub fn snapshot(&self) -> Result<LibrarySnapshot> {
        self.with_db(library::snapshot)
    }

    pub fn add_root(&self, path: PathBuf, label: Option<String>) -> Result<LibraryRoot> {
        self.with_db(|db| library::add_root(db, &path, label.as_deref()))
    }

    pub fn scan(
        &self,
        root_id: Option<&str>,
        sink: Option<Arc<dyn EventSink>>,
    ) -> Result<ScanReport> {
        if let Some(id) = root_id {
            if let Some(sink) = &sink {
                sink.emit(CoreEvent::ScanStarted {
                    root_id: id.to_string(),
                });
            }
        }
        let report = self.with_db(|db| library::scan_root(db, root_id))?;
        if let Some(sink) = sink {
            sink.emit(CoreEvent::ScanFinished {
                report: report.clone(),
            });
            sink.emit(CoreEvent::LibraryChanged);
        }
        Ok(report)
    }

    pub fn refresh_runners(&self) -> Result<Vec<Runner>> {
        self.with_db(|db| runners::refresh(db, self.allow_fake))
    }

    pub fn launch(
        &self,
        edition_id: &str,
        extra_args: &[String],
        sink: Arc<dyn EventSink>,
    ) -> Result<Session> {
        sessions::launch(Arc::clone(&self.db), edition_id, extra_args, sink)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.with_db(|db| db.set_setting(key, value))
    }

    pub fn select_runner(&self, runner_id: &str) -> Result<()> {
        let runners = self.refresh_runners()?;
        if !runners.iter().any(|r| r.id == runner_id) {
            return Err(AppError::new(
                "RUNNER_MISSING",
                format!("runner not found: {runner_id}"),
            ));
        }
        self.set_setting("selected_runner_id", runner_id)
    }

    pub fn set_display_title(&self, edition_id: &str, title: &str) -> Result<()> {
        let trimmed = title.trim();
        if trimmed.is_empty() {
            return Err(AppError::new("MEDIA_MISSING", "title cannot be empty"));
        }
        let n = self.with_db(|db| db.set_display_title(edition_id, Some(trimmed)))?;
        if n == 0 {
            return Err(AppError::new(
                "MEDIA_MISSING",
                "title is not in the library",
            ));
        }
        Ok(())
    }

    pub fn set_favourite(&self, edition_id: &str, favorite: bool) -> Result<()> {
        let n = self.with_db(|db| db.set_favourite(edition_id, favorite))?;
        if n == 0 {
            return Err(AppError::new(
                "MEDIA_MISSING",
                "title is not in the library",
            ));
        }
        Ok(())
    }

    pub fn open_setup(&self, runner_id: &str) -> Result<()> {
        self.with_db(|db| runners::open_setup(db, runner_id))
    }

    pub fn db_arc(&self) -> Arc<Mutex<Db>> {
        Arc::clone(&self.db)
    }
}

pub struct FnSink<F>(pub F)
where
    F: Fn(CoreEvent) + Send + Sync;

impl<F> EventSink for FnSink<F>
where
    F: Fn(CoreEvent) + Send + Sync,
{
    fn emit(&self, event: CoreEvent) {
        (self.0)(event);
    }
}

pub struct NoopSink;

impl EventSink for NoopSink {
    fn emit(&self, _event: CoreEvent) {}
}

#[cfg(test)]
mod tests;
