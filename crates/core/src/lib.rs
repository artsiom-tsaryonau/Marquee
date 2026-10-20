pub mod artwork;
pub mod catalog;
pub mod db;
pub mod error;
pub mod http;
pub mod instance;
pub mod iso;
pub mod library;
pub mod model;
pub mod paths;
pub mod runners;
pub mod sessions;
pub mod tools;

use crate::db::Db;
use crate::error::{AppError, Result};
use crate::instance::InstanceLock;
use crate::model::{
    ArtworkReport, CoreEvent, LibraryRoot, LibrarySnapshot, Runner, RunnerKind, ScanReport, Session,
};
use crate::paths::Paths;
use crate::sessions::EventSink;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct Core {
    pub paths: Paths,
    db: Arc<Mutex<Db>>,
    _lock: InstanceLock,
    allow_fake: bool,
    discover_host: bool,
    installing: Mutex<bool>,
}

impl Core {
    pub fn open(data_dir: Option<PathBuf>, allow_fake: bool) -> Result<Self> {
        Self::open_with(data_dir, allow_fake, true)
    }

    /// Tests: do not pick up a host AppImage/Flatpak (that would launch real PCSX2).
    #[cfg(test)]
    pub fn open_isolated(data_dir: Option<PathBuf>, allow_fake: bool) -> Result<Self> {
        Self::open_with(data_dir, allow_fake, false)
    }

    fn open_with(data_dir: Option<PathBuf>, allow_fake: bool, discover_host: bool) -> Result<Self> {
        let paths = Paths::resolve(data_dir)?;
        let lock = InstanceLock::acquire(&paths.instance_lock())?;
        let db = Db::open(&paths)?;
        db.abandon_running_sessions()?;
        let core = Self {
            paths,
            db: Arc::new(Mutex::new(db)),
            _lock: lock,
            allow_fake,
            discover_host,
            installing: Mutex::new(false),
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

    pub fn health(&self) -> Result<String> {
        let snap = self.snapshot()?;
        Ok(format!(
            "ok:{}:{}",
            snap.editions.len(),
            self.paths.data_dir.display()
        ))
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
        let unpacked = self.paths.pcsx2_unpacked_current();
        let appimage = self.paths.pcsx2_appimage_path();
        let runners_dir = self.paths.data_dir.join("runners");
        let bios = self.paths.pcsx2_bios_dir();
        self.with_db(|db| {
            let mut extra = vec![unpacked.clone(), appimage.clone(), runners_dir.clone()];
            if let Some(path) = db.get_setting("pcsx2_appimage_path")? {
                extra.push(PathBuf::from(path));
            }
            runners::refresh(db, self.allow_fake, &extra, self.discover_host, Some(&bios))
        })
    }

    pub fn install_pcsx2(&self, channel: &str, sink: Arc<dyn EventSink>) -> Result<Vec<Runner>> {
        let channel = tools::Pcsx2Channel::parse(channel)?;
        {
            let mut flag = self
                .installing
                .lock()
                .map_err(|_| AppError::new("DB", "install lock poisoned"))?;
            if *flag {
                return Err(AppError::new("INSTALL_BUSY", "PCSX2 is already installing"));
            }
            *flag = true;
        }
        struct Guard<'a>(&'a Mutex<bool>);
        impl Drop for Guard<'_> {
            fn drop(&mut self) {
                if let Ok(mut flag) = self.0.lock() {
                    *flag = false;
                }
            }
        }
        let _guard = Guard(&self.installing);

        let existing = self.refresh_runners()?;
        if tools::already_have(channel, &self.paths, &existing) {
            let runner_id = existing
                .iter()
                .find(|r| r.kind == RunnerKind::Pcsx2 && r.available)
                .map(|r| r.id.clone());
            sink.emit(CoreEvent::ToolInstallFinished {
                tool_id: tools::TOOL_PCSX2.into(),
                runner_id,
            });
            return Ok(existing);
        }

        match tools::install_pcsx2(&self.paths, channel, Arc::clone(&sink)) {
            Ok(outcome) => {
                match &outcome {
                    tools::InstallOutcome::Unpacked { bin } => {
                        self.set_setting("pcsx2_unpacked_path", &bin.to_string_lossy())?;
                        self.set_setting("marquee_installed_pcsx2", "unpacked")?;
                    }
                    tools::InstallOutcome::AppImage { path } => {
                        self.set_setting("pcsx2_appimage_path", &path.to_string_lossy())?;
                        self.set_setting("marquee_installed_pcsx2", "appimage")?;
                    }
                    tools::InstallOutcome::Flatpak => {
                        self.set_setting("marquee_installed_pcsx2", "flatpak")?;
                    }
                    tools::InstallOutcome::AlreadyPresent => {}
                }
                let runners = self.refresh_runners()?;
                if let Some(runner) = runners::pick_default(&runners) {
                    self.select_runner(&runner.id)?;
                }
                let runner_id = runners
                    .iter()
                    .find(|r| r.kind == RunnerKind::Pcsx2 && r.available)
                    .map(|r| r.id.clone());
                sink.emit(CoreEvent::ToolInstallFinished {
                    tool_id: tools::TOOL_PCSX2.into(),
                    runner_id,
                });
                sink.emit(CoreEvent::LibraryChanged);
                Ok(runners)
            }
            Err(err) => {
                sink.emit(CoreEvent::ToolInstallFailed {
                    tool_id: tools::TOOL_PCSX2.into(),
                    message: err.message.clone(),
                });
                Err(err)
            }
        }
    }

    pub fn refresh_artwork(
        &self,
        edition_id: Option<&str>,
        sink: Option<Arc<dyn EventSink>>,
    ) -> Result<ArtworkReport> {
        let (jobs, skipped) =
            self.with_db(|db| artwork::pending_jobs(db, &self.paths.cache_dir, edition_id))?;
        let mut report = ArtworkReport {
            skipped,
            ..ArtworkReport::default()
        };
        for job in jobs {
            match artwork::fetch_cover(&job.url)
                .and_then(|bytes| self.with_db(|db| artwork::store_jpeg(db, &job, &bytes)))
            {
                Ok(()) => report.fetched += 1,
                Err(err) if err.code == "NOT_FOUND" => report.skipped += 1,
                Err(_) => report.failed += 1,
            }
        }
        Ok(artwork::finish(report, sink))
    }

    pub fn launch(
        &self,
        edition_id: &str,
        extra_args: &[String],
        sink: Arc<dyn EventSink>,
    ) -> Result<Session> {
        sessions::launch(
            Arc::clone(&self.db),
            edition_id,
            extra_args,
            sink,
            Some(self.paths.pcsx2_xdg_config()),
        )
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
        let home = self.paths.pcsx2_xdg_config();
        self.with_db(|db| runners::open_setup(db, runner_id, Some(&home)))
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
