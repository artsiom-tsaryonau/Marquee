pub mod api;
pub mod artwork;
pub mod bios;
pub mod catalog;
pub mod cue;
pub mod db;
pub mod desktop_rendering;
pub mod emulators;
pub mod error;
pub mod game_media;
pub mod http;
pub mod hub;
pub mod instance;
pub mod intake;
pub mod iso;
pub mod keys;
pub mod launchers;
pub mod library;
pub mod lutris;
pub mod metadata;
mod metadata_search;
pub mod model;
pub mod music;
mod music_metadata;
pub mod neogeo;
pub mod paths;
pub mod ports;
pub mod preparation;
pub mod profiles;
pub mod ps3;
mod ps3_firmware;
pub mod purple;
pub mod romart;
pub mod romfacts;
pub mod runners;
pub mod server;
pub mod sessions;
pub mod source_builds;
pub mod steam;
pub mod streaming;
pub mod transfer;
pub mod wanted;

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
    _lock: Arc<InstanceLock>,
    allow_fake: bool,
    discover_host: bool,
    installing_ports: Mutex<std::collections::HashSet<String>>,
    events: Arc<hub::EventHub>,
    keys: keys::KeyStore,
    pub purple: Arc<purple::Manager>,
    pub source_builds: source_builds::Manager,
    pub streaming_controls: Arc<streaming::controls::Manager>,
    pub(crate) transfers: transfer::Transfers,
    pub(crate) preparation: marquee_manager::Client,
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
        let keys = keys::KeyStore::new(&paths.config_dir);
        let lock = InstanceLock::acquire(&paths.instance_lock())?;
        let db = Db::open(&paths)?;
        let core = Self {
            source_builds: source_builds::Manager::default(),
            streaming_controls: Arc::new(streaming::controls::Manager::default()),
            preparation: marquee_manager::Client::for_library_owner(&paths.data_dir),
            transfers: transfer::Transfers::new(&paths.data_dir),
            purple: Arc::new(purple::Manager::new(&paths)),
            paths,
            db: Arc::new(Mutex::new(db)),
            _lock: Arc::new(lock),
            allow_fake,
            discover_host,
            installing_ports: Mutex::new(std::collections::HashSet::new()),
            events: Arc::new(hub::EventHub::new()),
            keys,
        };
        sessions::recover(
            Arc::clone(&core.db),
            core.events.clone(),
            core._lock.clone(),
        )?;
        core.refresh_runners()?;
        if let Ok(t) = core.torrent_settings() {
            if t.enabled && !core.paths.data_dir.join("manager/stopped").exists() {
                let _ = core
                    .preparation
                    .torrent::<transfer::TransferStatus>(marquee_manager::torrent::Request::Status);
            }
        }
        Ok(core)
    }

    fn with_db<T>(&self, f: impl FnOnce(&Db) -> Result<T>) -> Result<T> {
        // A panic in another thread must not brick the library; SQLite state is still consistent.
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        f(&db)
    }

    pub fn snapshot(&self) -> Result<LibrarySnapshot> {
        let mut snapshot = self.with_db(library::snapshot)?;
        snapshot.settings.torrents = self.torrent_settings()?;
        Ok(snapshot)
    }

    pub fn health(&self) -> Result<String> {
        let snap = self.snapshot()?;
        Ok(format!(
            "ok:{}:{}",
            snap.editions.len(),
            self.paths.data_dir.display()
        ))
    }

    pub fn add_root(
        &self,
        system: &str,
        path: PathBuf,
        label: Option<String>,
    ) -> Result<LibraryRoot> {
        let root = self.with_db(|db| library::add_root(db, &path, system, label.as_deref()))?;
        self.events.emit(CoreEvent::LibraryChanged);
        Ok(root)
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

    /// Remove library entries whose files no longer exist (never-played ones only).
    pub fn prune_missing(&self) -> Result<u32> {
        let removed = self.with_db(library::prune_missing)?;
        if removed > 0 {
            self.events.emit(CoreEvent::LibraryChanged);
        }
        Ok(removed)
    }

    pub fn refresh_runners(&self) -> Result<Vec<Runner>> {
        self.with_db(|db| {
            // Marquee owns its emulators: every runner comes from its catalogue.
            runners::refresh(db, self.allow_fake)?;
            emulators::sync_runners(db, &self.paths)?;
            runners::snapshot(db)
        })
    }

    /// Install PCSX2 through the catalogue (verified download, unpacked into
    /// Marquee's folder). Kept as its own command for the install dialog; the
    /// only channel is the managed one.
    pub fn install_pcsx2(&self, channel: &str, sink: Arc<dyn EventSink>) -> Result<Vec<Runner>> {
        if !matches!(channel, "" | "auto" | "unpacked") {
            return Err(AppError::new(
                "RUNNER_MISSING",
                format!("unknown PCSX2 channel: {channel} (Marquee installs PCSX2 itself)"),
            ));
        }
        let existing = self.refresh_runners()?;
        let have = |runners: &[Runner]| {
            runners
                .iter()
                .find(|r| r.kind == RunnerKind::Pcsx2 && r.available)
                .map(|r| r.id.clone())
        };
        if let Some(runner_id) = have(&existing) {
            sink.emit(CoreEvent::ToolInstallFinished {
                tool_id: "pcsx2".into(),
                runner_id: Some(runner_id),
            });
            return Ok(existing);
        }
        match self.install_emulator("pcsx2", Arc::clone(&sink)) {
            Ok(_) => {
                let runners = self.refresh_runners()?;
                if let Some(runner) = runners::pick_default(&runners) {
                    self.select_runner(&runner.id)?;
                }
                sink.emit(CoreEvent::ToolInstallFinished {
                    tool_id: "pcsx2".into(),
                    runner_id: have(&runners),
                });
                sink.emit(CoreEvent::LibraryChanged);
                Ok(runners)
            }
            Err(err) => Err(err),
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
                Ok(true) => report.fetched += 1,
                Ok(false) => report.skipped += 1,
                Err(err) if err.code == "NOT_FOUND" => report.skipped += 1,
                Err(_) => report.failed += 1,
            }
        }
        romart::run(self, edition_id, &mut report);
        romfacts::run(self, edition_id);
        Ok(artwork::finish(report, sink))
    }

    pub fn launch(
        &self,
        edition_id: &str,
        extra_args: &[String],
        sink: Arc<dyn EventSink>,
    ) -> Result<Session> {
        // Hands-off setup: pick up any BIOS dropped into the inbox folder.
        let inbox = self
            .with_db(|db| db.get_setting("bios_inbox"))
            .ok()
            .flatten();
        let imported = bios::import_inbox(&self.paths, inbox.as_deref());
        if !imported.added.is_empty() {
            let _ = self.refresh_runners();
        }
        sessions::launch(
            Arc::clone(&self.db),
            edition_id,
            extra_args,
            sink,
            Some(self.paths.pcsx2_xdg_config()),
            self._lock.clone(),
        )
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.with_db(|db| db.set_setting(key, value))
    }

    /// How games open: `window` (default) or `fullscreen`.
    pub fn set_display_mode(&self, mode: &str) -> Result<()> {
        if !matches!(mode, "window" | "fullscreen") {
            return Err(AppError::new(
                "BAD_REQUEST",
                "display mode must be `window` or `fullscreen`",
            ));
        }
        self.set_setting("display_mode", mode)
    }

    pub fn torrent_settings(&self) -> Result<model::TorrentSettings> {
        if marquee_manager::torrent::adopted(&self.paths.data_dir)? {
            marquee_manager::torrent::read_settings(&self.paths.data_dir)
        } else {
            Ok(self.with_db(|db| db.load_settings())?.torrents)
        }
    }

    pub fn set_torrent_settings(&self, t: &model::TorrentSettings) -> Result<()> {
        self.preparation
            .torrent::<()>(marquee_manager::torrent::Request::SetSettings {
                settings: t.clone(),
            })?;
        self.events.emit(CoreEvent::LibraryChanged);
        Ok(())
    }

    pub fn select_runner(&self, runner_id: &str) -> Result<()> {
        let runners = self.refresh_runners()?;
        let Some(runner) = runners.iter().find(|r| r.id == runner_id && r.available) else {
            return Err(AppError::new(
                "RUNNER_MISSING",
                format!("runner not found: {runner_id}"),
            ));
        };
        let system = match runner.kind {
            RunnerKind::Duckstation => "ps1",
            RunnerKind::Pcsx2 => "ps2",
            RunnerKind::Emulator => "emulator",
            RunnerKind::Fake => "test",
        };
        self.set_setting(&format!("selected_runner.{system}"), runner_id)?;
        self.set_setting("selected_runner_id", runner_id)?;
        self.events.emit(CoreEvent::LibraryChanged);
        Ok(())
    }

    pub fn select_system_runner(&self, system: &str, runner_id: &str) -> Result<()> {
        if !matches!(system, "ps1" | "ps2" | "gb" | "gbc") {
            return Err(AppError::new(
                "BAD_REQUEST",
                "Runner defaults are not exposed for this system yet",
            ));
        }
        let available = self.refresh_runners()?;
        let runner = available
            .iter()
            .find(|r| r.id == runner_id && runners::compatible(r, system))
            .ok_or_else(|| {
                AppError::new(
                    "BAD_REQUEST",
                    "This emulator does not support the selected platform",
                )
            })?;
        runners::require_launchable(runner)?;
        self.set_setting(&format!("selected_runner.{system}"), runner_id)?;
        self.events.emit(CoreEvent::LibraryChanged);
        Ok(())
    }

    pub fn set_game_runner(&self, edition_id: &str, runner_id: Option<&str>) -> Result<()> {
        self.refresh_runners()?;
        self.with_db(|db| {
            if db.running_session()?.is_some() {
                return Err(AppError::new(
                    "LAUNCH_BUSY",
                    "Close the running game before changing its emulator",
                ));
            }
            let edition = db
                .edition_by_id(edition_id)?
                .ok_or_else(|| AppError::new("MEDIA_MISSING", "Game is not in the library"))?;
            if let Some(id) = runner_id {
                let runners = runners::snapshot(db)?;
                let runner = runners
                    .iter()
                    .find(|r| r.id == id && runners::compatible(r, &edition.system))
                    .ok_or_else(|| {
                        AppError::new("BAD_REQUEST", "This emulator does not support this game")
                    })?;
                runners::require_launchable(runner)?;
                db.set_profile(edition_id, id)?;
            } else {
                db.clear_profile(edition_id)?;
            }
            Ok(())
        })?;
        self.events.emit(CoreEvent::LibraryChanged);
        Ok(())
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
        self.events.emit(CoreEvent::LibraryChanged);
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
        self.events.emit(CoreEvent::LibraryChanged);
        Ok(())
    }

    fn installed_port(&self, id: &str) -> Result<Option<ports::InstalledPort>> {
        Ok(self
            .with_db(|db| db.get_setting(&ports::setting_key(id)))?
            .and_then(|raw| serde_json::from_str(&raw).ok()))
    }

    /// Catalogue plus install state. Offline: "update" shows only after `check_port_updates`.
    pub fn list_ports(&self) -> Result<Vec<ports::PortInfo>> {
        ports::catalogue()?
            .iter()
            .map(|def| {
                let installed = self.installed_port(&def.id)?;
                let latest = self.with_db(|db| db.get_setting(&ports::latest_key(&def.id)))?;
                Ok(ports::info(
                    &self.paths,
                    def,
                    installed.as_ref(),
                    latest.as_deref(),
                ))
            })
            .collect()
    }

    /// Ask GitHub for the newest release of each installed port. A port that fails to
    /// answer keeps its previous state.
    pub fn check_port_updates(&self) -> Result<Vec<ports::PortInfo>> {
        for def in ports::catalogue()? {
            if self.installed_port(&def.id)?.is_none() {
                continue;
            }
            let Ok(release) = ports::latest_release(&def).and_then(|r| ports::pick_asset(&def, &r))
            else {
                continue;
            };
            self.set_setting(&ports::latest_key(&def.id), &release.tag)?;
        }
        self.list_ports()
    }

    /// Install a port, or update it to the newest release. User files in the folder are kept.
    pub fn install_port(&self, id: &str, sink: Arc<dyn EventSink>) -> Result<Vec<ports::PortInfo>> {
        let def = ports::find(id)?;
        {
            let mut busy = self
                .installing_ports
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if !busy.insert(def.id.clone()) {
                return Err(AppError::new(
                    "INSTALL_BUSY",
                    format!("{} is already installing", def.name),
                ));
            }
        }
        struct Done<'a>(&'a Mutex<std::collections::HashSet<String>>, String);
        impl Drop for Done<'_> {
            fn drop(&mut self) {
                self.0
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&self.1);
            }
        }
        let _done = Done(&self.installing_ports, def.id.clone());

        let current = self.installed_port(&def.id)?;
        let installed = ports::install_latest(
            &self.paths,
            &def,
            current.as_ref().map(|c| c.tag.as_str()),
            &sink,
        )?;
        self.set_setting(
            &ports::setting_key(&def.id),
            &serde_json::to_string(&installed)?,
        )?;
        self.set_setting(&ports::latest_key(&def.id), &installed.tag)?;
        sink.emit(CoreEvent::LibraryChanged);
        self.list_ports()
    }

    pub fn launch_port(&self, id: &str) -> Result<()> {
        let def = ports::find(id)?;
        let installed = self.installed_port(id)?.ok_or_else(|| {
            AppError::new(
                "PORT_NOT_INSTALLED",
                format!("{} is not installed", def.name),
            )
        })?;
        ports::launch(&self.paths, &def, &installed)
    }

    pub fn open_port_folder(&self, id: &str) -> Result<()> {
        ports::find(id)?;
        ports::open_folder(&self.paths, id)
    }

    pub fn open_setup(&self, runner_id: &str) -> Result<()> {
        let home = self.paths.pcsx2_xdg_config();
        self.with_db(|db| runners::open_setup(db, runner_id, Some(&home)))
    }

    pub fn inspect_steam(&self) -> steam::Inventory {
        if self.discover_host {
            steam::discover()
        } else {
            steam::Inventory::default()
        }
    }

    pub fn lutris_library(&self) -> Result<lutris::Snapshot> {
        let detected = if self.discover_host {
            lutris::discover()
        } else {
            vec![]
        };
        let config = self
            .with_db(|db| db.get_setting(lutris::CONFIG_KEY))?
            .map(|value| serde_json::from_str(&value))
            .transpose()?
            .unwrap_or_else(|| lutris::opt_in_config(&detected));
        lutris::snapshot(config, detected)
    }
    pub fn configure_lutris(&self, config: lutris::Config) -> Result<lutris::Snapshot> {
        let snapshot = lutris::snapshot(
            config.clone(),
            if self.discover_host {
                lutris::discover()
            } else {
                vec![]
            },
        )?;
        self.with_db(|db| db.set_setting(lutris::CONFIG_KEY, &serde_json::to_string(&config)?))?;
        self.events.emit(CoreEvent::LutrisChanged);
        Ok(snapshot)
    }
    pub fn open_lutris_game(&self, id: &str) -> Result<()> {
        let snapshot = self.lutris_library()?;
        let game = snapshot.games.iter().find(|g| g.id == id).ok_or_else(|| {
            AppError::new(
                "MEDIA_MISSING",
                "Entry is no longer in the connected Lutris library",
            )
        })?;
        let uri = lutris::uri(game)?;
        let mut launcher = self
            .list_launchers()?
            .into_iter()
            .find(|l| l.launcher.id == snapshot.config.launcher_id)
            .ok_or_else(|| {
                AppError::new("LAUNCHER_MISSING", "Configure a Lutris launcher in Tools")
            })?
            .launcher;
        launcher.args.push(uri);
        launchers::launch(&launcher)
    }

    /// External application configurations and their current readiness.
    pub fn list_launchers(&self) -> Result<Vec<launchers::LauncherInfo>> {
        self.with_db(|db| launchers::list(db, self.discover_host))
    }
    pub fn save_launcher(
        &self,
        launcher: launchers::Launcher,
    ) -> Result<Vec<launchers::LauncherInfo>> {
        self.with_db(|db| launchers::save(db, launcher))?;
        self.events.emit(CoreEvent::LaunchersChanged);
        self.list_launchers()
    }
    pub fn remove_launcher(&self, id: &str) -> Result<Vec<launchers::LauncherInfo>> {
        self.with_db(|db| launchers::remove(db, id))?;
        self.events.emit(CoreEvent::LaunchersChanged);
        self.list_launchers()
    }
    pub fn open_launcher(&self, id: &str) -> Result<()> {
        let info = self
            .list_launchers()?
            .into_iter()
            .find(|l| l.launcher.id == id)
            .ok_or_else(|| {
                AppError::new("LAUNCHER_MISSING", "Launcher configuration was not found")
            })?;
        launchers::launch(&info.launcher)
    }

    /// Emulators Marquee can install, with install state. Offline: an update shows
    /// only after `check_emulator_updates`.
    pub fn list_emulators(&self) -> Result<Vec<emulators::EmulatorInfo>> {
        emulators::catalogue()?
            .iter()
            .map(|def| {
                let installed = self.with_db(|db| emulators::installed(db, &def.id))?;
                let latest = if source_builds::supported(&def.id) {
                    Some(source_builds::version(&def.id)?)
                } else {
                    self.with_db(|db| db.get_setting(&emulators::latest_key(&def.id)))?
                };
                let cfg = self.with_db(|db| emulators::config(db, &def.id))?;
                Ok(emulators::info(
                    &self.paths,
                    def,
                    installed.as_ref(),
                    latest.as_deref().filter(|v| !v.is_empty()),
                    &cfg,
                ))
            })
            .collect()
    }

    /// Ask GitHub for the newest version of each installed emulator.
    pub fn check_emulator_updates(&self) -> Result<Vec<emulators::EmulatorInfo>> {
        for def in emulators::catalogue()? {
            if self
                .with_db(|db| emulators::installed(db, &def.id))?
                .is_none()
            {
                continue;
            }
            if let Ok(version) = emulators::latest_version(&def) {
                self.set_setting(&emulators::latest_key(&def.id), &version)?;
            }
        }
        self.list_emulators()
    }

    /// Install an emulator, or update it. Settings, saves and BIOS are kept.
    pub fn install_emulator(
        &self,
        id: &str,
        sink: Arc<dyn EventSink>,
    ) -> Result<Vec<emulators::EmulatorInfo>> {
        self.require_ps3_setup_idle(id)?;
        let def = emulators::find(id)?;
        if self.with_db(|db| db.running_session())?.is_some() {
            return Err(AppError::new("LAUNCH_BUSY", "close the running game first"));
        }
        {
            let mut busy = self
                .installing_ports
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if !busy.insert(format!("emulator:{id}")) {
                return Err(AppError::new(
                    "INSTALL_BUSY",
                    format!("{} is already installing", def.name),
                ));
            }
        }
        struct Done<'a>(&'a Mutex<std::collections::HashSet<String>>, String);
        impl Drop for Done<'_> {
            fn drop(&mut self) {
                self.0
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&self.1);
            }
        }
        let _done = Done(&self.installing_ports, format!("emulator:{id}"));
        sink.emit(CoreEvent::ToolInstallStarted {
            tool_id: id.into(),
            channel: "appimage".into(),
        });
        match emulators::install(&self.paths, &def, &sink) {
            Ok(installed) => {
                self.set_setting(
                    &emulators::setting_key(id),
                    &serde_json::to_string(&installed)?,
                )?;
                self.set_setting(&emulators::latest_key(id), &installed.version)?;
                self.refresh_runners()?;
                sink.emit(CoreEvent::ToolInstallFinished {
                    tool_id: id.into(),
                    runner_id: Some(emulators::runner_id(id)),
                });
                sink.emit(CoreEvent::LibraryChanged);
                self.list_emulators()
            }
            Err(err) => {
                sink.emit(CoreEvent::ToolInstallFailed {
                    tool_id: id.into(),
                    message: err.message.clone(),
                });
                Err(err)
            }
        }
    }

    /// Start a persistent-status build job, independent of the requesting window.
    pub fn build_emulator(self: &Arc<Self>, id: &str) -> Result<()> {
        emulators::find(id)?;
        if !source_builds::supported(id) {
            return Err(AppError::new(
                "SOURCE_UNSUPPORTED",
                "No reviewed source recipe for this emulator",
            ));
        }
        if self.with_db(|db| db.running_session())?.is_some() {
            return Err(AppError::new("LAUNCH_BUSY", "close the running game first"));
        }
        let key = format!("emulator:{id}");
        let mut busy = self
            .installing_ports
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if !busy.insert(key.clone()) {
            return Err(AppError::new(
                "INSTALL_BUSY",
                "This emulator is already being changed",
            ));
        }
        let cancel = match self.source_builds.reserve(&self.paths, id) {
            Ok(cancel) => cancel,
            Err(error) => {
                busy.remove(&key);
                return Err(error);
            }
        };
        drop(busy);
        let core = self.clone();
        let id = id.to_string();
        std::thread::spawn(move || {
            let sink: Arc<dyn EventSink> = core.hub();
            sink.emit(CoreEvent::ToolInstallStarted {
                tool_id: id.clone(),
                channel: "source".into(),
            });
            let result = (|| {
                let prepared = source_builds::build(&core.paths, &id, &cancel, &sink)?;
                // A game may have started during compilation. Keep it running and
                // leave the existing program alone rather than replacing it.
                if core.with_db(|db| db.running_session())?.is_some() {
                    return Err(AppError::new(
                        "LAUNCH_BUSY",
                        "Close the game and retry the build",
                    ));
                }
                if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                    return Err(AppError::new(
                        "BUILD_CANCELLED",
                        "Build cancelled before promotion",
                    ));
                }
                let old = core.with_db(|db| emulators::installed(db, &id))?;
                source_builds::publish(&core.paths, &id, prepared, old.as_ref(), |installed| {
                    core.set_setting(
                        &emulators::setting_key(&id),
                        &serde_json::to_string(installed)?,
                    )
                })?;
                core.refresh_runners()?;
                Ok(())
            })();
            core.source_builds.finish(&core.paths, &id, &result);
            core.installing_ports
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&key);
            match result {
                Ok(()) => sink.emit(CoreEvent::ToolInstallFinished {
                    tool_id: id,
                    runner_id: None,
                }),
                Err(error) => sink.emit(CoreEvent::ToolInstallFailed {
                    tool_id: id,
                    message: error.message,
                }),
            }
            sink.emit(CoreEvent::LibraryChanged);
        });
        Ok(())
    }

    pub fn rollback_emulator(&self, id: &str) -> Result<Vec<emulators::EmulatorInfo>> {
        let busy = self
            .installing_ports
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if busy.contains(&format!("emulator:{id}")) {
            return Err(AppError::new(
                "INSTALL_BUSY",
                "Wait for the build to finish or cancel it",
            ));
        }
        if self.with_db(|db| db.running_session())?.is_some() {
            return Err(AppError::new("LAUNCH_BUSY", "close the running game first"));
        }
        source_builds::rollback(&self.paths, id, |installed| {
            self.set_setting(
                &emulators::setting_key(id),
                &serde_json::to_string(installed)?,
            )
        })?;
        drop(busy);
        self.refresh_runners()?;
        self.events.emit(CoreEvent::LibraryChanged);
        self.list_emulators()
    }

    pub fn emulator_profile(&self, id: &str) -> Result<profiles::Status> {
        if id != "duckstation" {
            return Err(AppError::new(
                "PROFILE_UNSUPPORTED",
                "Settings profiles currently support DuckStation only",
            ));
        }
        profiles::get(&emulators::config_home(&self.paths, id))
    }

    pub fn configure_emulator_profile(
        &self,
        id: &str,
        config: profiles::Config,
    ) -> Result<profiles::Status> {
        if id != "duckstation" {
            return Err(AppError::new(
                "PROFILE_UNSUPPORTED",
                "Settings profiles currently support DuckStation only",
            ));
        }
        let status = self.with_db(|db| {
            if db
                .list_sessions()?
                .iter()
                .any(|s| s.state == "running" && s.runner_id == emulators::runner_id(id))
            {
                return Err(AppError::new(
                    "PROFILE_BUSY",
                    "Close the running DuckStation game before changing its settings profile",
                ));
            }
            profiles::configure(&emulators::config_home(&self.paths, id), config)
        })?;
        self.events.emit(CoreEvent::LibraryChanged);
        Ok(status)
    }

    /// Save what the user can configure for an emulator (extra launch arguments).
    pub fn configure_emulator(
        &self,
        id: &str,
        args: Vec<String>,
    ) -> Result<Vec<emulators::EmulatorInfo>> {
        emulators::find(id)?;
        self.with_db(|db| emulators::set_config(db, id, &args))?;
        self.refresh_runners()?;
        self.list_emulators()
    }

    /// Open an installed emulator's own window (not a game).
    pub fn open_emulator(&self, id: &str) -> Result<()> {
        self.require_ps3_setup_idle(id)?;
        let busy = self
            .installing_ports
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if busy.contains(&format!("emulator:{id}")) {
            return Err(AppError::new(
                "INSTALL_BUSY",
                "Wait for the emulator installation to finish",
            ));
        }
        let def = emulators::find(id)?;
        let installed = self
            .with_db(|db| emulators::installed(db, id))?
            .ok_or_else(|| {
                AppError::new("RUNNER_MISSING", format!("{} is not installed", def.name))
            })?;
        let cfg = self.with_db(|db| emulators::config(db, id))?;
        if id == "duckstation" {
            profiles::prepare(&emulators::config_home(&self.paths, id))?;
        }
        emulators::open(&self.paths, &def, &installed, &cfg)
    }

    /// Remove an emulator's program. `purge_data` also deletes its settings, saves and BIOS.
    /// Emulators the library needs but Marquee has not installed: those that run
    /// a system with a library folder, are integrated, and download from a link
    /// (source builds are never started automatically).
    pub fn needed_emulators(&self) -> Result<Vec<String>> {
        let systems: std::collections::HashSet<String> = self
            .snapshot()?
            .roots
            .into_iter()
            .map(|r| r.system)
            .collect();
        Ok(self
            .list_emulators()?
            .into_iter()
            .filter(|e| {
                e.integrated
                    && !e.source_build
                    && e.status == "not_installed"
                    && e.systems.iter().any(|s| systems.contains(s))
            })
            .map(|e| e.id)
            .collect())
    }

    /// Install every needed emulator, one after another; returns the ids installed.
    /// A failure stops the run and names the emulator.
    pub fn install_needed(&self, sink: Arc<dyn EventSink>) -> Result<Vec<String>> {
        let mut done = Vec::new();
        for id in self.needed_emulators()? {
            self.install_emulator(&id, Arc::clone(&sink))
                .map_err(|e| AppError::new(e.code.as_str(), format!("{id}: {}", e.message)))?;
            done.push(id);
        }
        Ok(done)
    }

    pub fn uninstall_emulator(
        &self,
        id: &str,
        purge_data: bool,
    ) -> Result<Vec<emulators::EmulatorInfo>> {
        self.require_ps3_setup_idle(id)?;
        let def = emulators::find(id)?;
        let busy = self
            .installing_ports
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if busy.contains(&format!("emulator:{id}")) {
            return Err(AppError::new(
                "INSTALL_BUSY",
                "Cancel the build and wait for it to stop before removing the emulator",
            ));
        }
        if self.with_db(|db| db.running_session())?.is_some() {
            return Err(AppError::new("LAUNCH_BUSY", "close the running game first"));
        }
        emulators::uninstall(&self.paths, &def, purge_data)?;
        self.with_db(|db| db.delete_runner(&emulators::runner_id(id)))?;
        for key in [emulators::setting_key(id), emulators::latest_key(id)] {
            self.set_setting(&key, "")?;
        }
        self.refresh_runners()?;
        self.events.emit(CoreEvent::LibraryChanged);
        self.list_emulators()
    }

    /// BIOS images Marquee holds, with the region read from each image.
    pub fn list_bios(&self) -> Result<Vec<bios::BiosFile>> {
        bios::list(&self.paths)
    }

    /// Copy every recognisable BIOS from a folder (or one file) into Marquee's own store.
    pub fn import_bios(&self, source: &std::path::Path) -> Result<bios::ImportReport> {
        let report = bios::import(&self.paths, source)?;
        self.refresh_runners()?;
        self.events.emit(CoreEvent::LibraryChanged);
        Ok(report)
    }

    /// Delete every BIOS Marquee holds for `system` (`ps1` or `ps2`).
    pub fn remove_bios(&self, system: &str) -> Result<usize> {
        let removed = bios::remove(&self.paths, system)?;
        self.refresh_runners()?;
        self.events.emit(CoreEvent::LibraryChanged);
        Ok(removed)
    }

    /// Delete the PCSX2 Marquee installed. Settings, memory cards, saves and BIOS stay
    /// until removed on purpose.
    pub fn uninstall_pcsx2(&self) -> Result<Vec<Runner>> {
        self.uninstall_emulator("pcsx2", false)?;
        self.refresh_runners()
    }

    /// Known API keys and where each comes from. Never contains a secret value.
    pub fn list_keys(&self) -> Result<Vec<keys::KeyInfo>> {
        self.keys.list()
    }

    /// Save a key from Settings (an empty value clears it).
    pub fn set_key(&self, id: &str, value: &str) -> Result<Vec<keys::KeyInfo>> {
        self.keys.set(id, value)?;
        self.keys.list()
    }

    /// Forget the value saved in Marquee; one from the environment or `.env` stays.
    pub fn clear_key(&self, id: &str) -> Result<Vec<keys::KeyInfo>> {
        self.keys.clear(id)?;
        self.keys.list()
    }

    /// The real value, for features inside Marquee (never sent to the UI).
    pub fn key(&self, id: &str) -> Result<Option<String>> {
        self.keys.get(id)
    }

    /// Every event the core emits goes here; the window and the web server both read it.
    pub fn hub(&self) -> Arc<hub::EventHub> {
        Arc::clone(&self.events)
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
