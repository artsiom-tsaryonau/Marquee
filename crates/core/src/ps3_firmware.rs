//! Library-side bridge to manager-owned firmware setup.
use crate::error::{AppError, Result};
use crate::sessions::EventSink;
use crate::{emulators, Core};
use marquee_manager::ps3_firmware::{Request, Status};
use std::path::Path;

impl Core {
    pub fn ps3_firmware_status(&self) -> Result<Status> {
        self.reconcile_ps3_firmware(false)
    }
    pub fn review_ps3_firmware(&self, source: &Path) -> Result<Status> {
        let installed = self
            .with_db(|db| emulators::installed(db, "rpcs3"))?
            .ok_or_else(|| {
                AppError::new(
                    "RUNNER_MISSING",
                    "Install RPCS3 in Tools before reviewing firmware",
                )
            })?;
        self.preparation.ps3_firmware(Request::Review {
            source: source.to_path_buf(),
            runner_version: installed.version,
        })
    }
    pub fn install_ps3_firmware(&self, review_token: &str) -> Result<Status> {
        if self.with_db(|db| db.running_session())?.is_some() {
            return Err(AppError::new(
                "LAUNCH_BUSY",
                "Close the running game before installing firmware",
            ));
        }
        let key = "emulator:rpcs3".to_string();
        {
            let mut busy = self
                .installing_ports
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if !busy.insert(key.clone()) {
                return Err(AppError::new(
                    "INSTALL_BUSY",
                    "RPCS3 setup is already running",
                ));
            }
        }
        struct Release<'a>(&'a Core, String);
        impl Drop for Release<'_> {
            fn drop(&mut self) {
                self.0
                    .installing_ports
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&self.1);
            }
        }
        let _release = Release(self, key);
        let result = self.preparation.ps3_firmware(Request::Start {
            review_token: review_token.into(),
        });
        self.events.emit(crate::model::CoreEvent::LibraryChanged);
        result
    }
    pub(crate) fn require_ps3_setup_idle(&self, id: &str) -> Result<()> {
        if id == "rpcs3" {
            self.reconcile_ps3_firmware(true)?;
            if marquee_manager::ps3_firmware::active(&self.paths.data_dir)? {
                return Err(AppError::new(
                    "INSTALL_BUSY",
                    "Finish RPCS3 firmware setup before opening, updating or removing RPCS3",
                ));
            }
        }
        Ok(())
    }

    fn reconcile_ps3_firmware(&self, idle_only: bool) -> Result<Status> {
        if self.preparation.status().is_ok_and(|s| s.running) {
            return if idle_only {
                marquee_manager::ps3_firmware::read_status(&self.paths.data_dir)
            } else {
                self.preparation.ps3_firmware(Request::Status)
            };
        }
        marquee_manager::ps3_firmware::recover(&self.paths.data_dir)?;
        marquee_manager::ps3_firmware::read_status(&self.paths.data_dir)
    }
}
