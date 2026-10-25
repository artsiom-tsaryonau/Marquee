//! Shared desktop/browser controls for an explicitly connected local Wolf host.
use crate::error::{AppError, Result};
use crate::Core;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const KEY: &str = "streaming.local_connection";

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    pub data_dir: PathBuf,
    pub socket: PathBuf,
}

impl Connection {
    pub(crate) fn validate(&self) -> Result<()> {
        use std::os::unix::ffi::OsStrExt;
        for path in [&self.data_dir, &self.socket] {
            if !path.is_absolute()
                || path
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
                || path.as_os_str().as_bytes().contains(&0)
            {
                return Err(AppError::new(
                    "STREAMING_CONNECTION",
                    "Choose absolute local paths without parent-directory segments",
                ));
            }
        }
        if self.socket.as_os_str().as_bytes().len() >= 108
            || self
                .data_dir
                .join("streaming/moonlight/viewer/control.sock")
                .as_os_str()
                .as_bytes()
                .len()
                >= 108
        {
            return Err(AppError::new(
                "STREAMING_CONNECTION",
                "Choose a shorter streaming folder/socket path for local controls",
            ));
        }
        Ok(())
    }
}

#[derive(Default)]
struct Operation {
    name: Option<&'static str>,
    error: Option<AppError>,
}

#[derive(Default)]
pub struct Manager {
    operation: Mutex<Operation>,
}

enum Action {
    View(String),
    Install,
    Pair,
}

fn completed(result: Value) -> Result<()> {
    if result["reason"] == "receiver-exited" && result["exit_code"].as_i64() != Some(0) {
        return Err(AppError::new(
            "VIEWER_EXIT",
            "The reference viewer exited unexpectedly; check its private log before retrying",
        ));
    }
    Ok(())
}

impl Manager {
    fn start(
        self: &Arc<Self>,
        config: impl FnOnce() -> Result<Connection>,
        action: Action,
    ) -> Result<()> {
        let mut operation = self.operation.lock().unwrap_or_else(|e| e.into_inner());
        if operation.name.is_some() {
            return Err(AppError::new(
                "STREAMING_BUSY",
                "A viewer operation is already active",
            ));
        }
        let config = config()?;
        config.validate()?;
        operation.name = Some(match &action {
            Action::View(_) => "viewer",
            Action::Install => "install",
            Action::Pair => "pair",
        });
        operation.error = None;
        let manager = Arc::clone(self);
        let thread = std::thread::Builder::new()
            .name("marquee-viewer".into())
            .spawn(move || {
                let result = match action {
                    Action::View(app_id) => {
                        super::viewer::view(&config.data_dir, &config.socket, &app_id)
                            .and_then(completed)
                    }
                    Action::Pair => super::wolf::WolfClient::new(&config.socket)
                        .and_then(|client| client.inspect())
                        .and_then(|_| super::viewer::pair(&config.data_dir))
                        .and_then(completed),
                    Action::Install => super::receiver::install(
                        &config.data_dir,
                        &std::sync::atomic::AtomicBool::new(false),
                    )
                    .map(|_| ()),
                };
                let mut state = manager.operation.lock().unwrap_or_else(|e| e.into_inner());
                state.name = None;
                state.error = result.err();
            });
        if let Err(e) = thread {
            operation.name = None;
            return Err(e.into());
        }
        Ok(())
    }
}

impl Core {
    pub fn streaming_connection(&self) -> Result<Connection> {
        let config = self
            .with_db(|db| db.get_setting(KEY))?
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or_else(|| Connection {
                data_dir: self.paths.data_dir.clone(),
                socket: self
                    .paths
                    .data_dir
                    .join("streaming/wolf/state/runtime/wolf.sock"),
            });
        Ok(config)
    }

    pub fn streaming_save_connection(&self, config: Connection) -> Result<()> {
        let _gate = self.headless_gate.lock().unwrap_or_else(|e| e.into_inner());
        let host = self.headless_status(None)?;
        if host["busy"] == true || host["host_running"] == true {
            return Err(AppError::new(
                "STREAMING_BUSY",
                "Stop the owned headless host before changing its connection",
            ));
        }
        config.validate()?;
        let operation = self
            .streaming_controls
            .operation
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if operation.name.is_some() {
            return Err(AppError::new(
                "STREAMING_BUSY",
                "Close the viewer or finish its installation before changing the connection",
            ));
        }
        let current = self.streaming_connection()?;
        let _lease = super::viewer::configuration_lease(&current.data_dir)?;
        self.set_setting(KEY, &serde_json::to_string(&config)?)
    }

    pub fn streaming_state(&self) -> Result<Value> {
        let config = self.streaming_connection()?;
        let host = super::wolf::WolfClient::new(&config.socket)?.inspect();
        let (host, host_error, pending) = match host {
            Ok(host) => {
                let pending = super::wolf::WolfClient::new(&config.socket)?.pending();
                match pending {
                    Ok(p) => (Some(host), None, p),
                    Err(e) => (Some(host), Some(e), vec![]),
                }
            }
            Err(e) => (None, Some(e), vec![]),
        };
        let (viewer, viewer_error) = match super::viewer::status(&config.data_dir) {
            Ok(v) => (v, None),
            Err(e) => (json!({"running":false}), Some(e)),
        };
        let (receiver_installed, receiver_error) = match super::receiver::status(&config.data_dir) {
            Ok(r) => (r.is_some(), None),
            Err(e) => (false, Some(e)),
        };
        let operation = self
            .streaming_controls
            .operation
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        Ok(
            json!({"connection":config,"host":host,"host_error":host_error,"pending":pending,
            "viewer":viewer,"viewer_error":viewer_error,"receiver_installed":receiver_installed,
            "receiver_error":receiver_error,"operation":operation.name,"last_error":operation.error,"headless":self.headless_status(None)?}),
        )
    }

    pub fn streaming_open_viewer(&self, app_id: String) -> Result<()> {
        self.streaming_controls
            .start(|| self.streaming_connection(), Action::View(app_id))
    }

    pub fn streaming_install_viewer(&self) -> Result<()> {
        self.streaming_controls
            .start(|| self.streaming_connection(), Action::Install)
    }

    pub fn streaming_pair_viewer(&self) -> Result<()> {
        self.streaming_controls
            .start(|| self.streaming_connection(), Action::Pair)
    }

    pub fn streaming_disconnect_viewer(&self) -> Result<Value> {
        super::viewer::disconnect(&self.streaming_connection()?.data_dir)
    }

    pub fn streaming_pair(&self, client_ip: &str, pin: &str) -> Result<()> {
        super::wolf::WolfClient::new(self.streaming_connection()?.socket)?.pair(client_ip, pin)
    }

    pub fn streaming_revoke(&self, client_id: &str) -> Result<()> {
        super::wolf::WolfClient::new(self.streaming_connection()?.socket)?.unpair(client_id)
    }

    pub fn streaming_end_game(&self, session_id: &str) -> Result<()> {
        super::wolf::WolfClient::new(self.streaming_connection()?.socket)?.stop_session(session_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn connection_validation_rejects_relative_parent_and_oversized_control_paths() {
        let mut c = Connection {
            data_dir: "/tmp/marquee".into(),
            socket: "/tmp/wolf.sock".into(),
        };
        assert!(c.validate().is_ok());
        c.socket = "wolf.sock".into();
        assert!(c.validate().is_err());
        c.socket = "/tmp/../wolf.sock".into();
        assert!(c.validate().is_err());
        c.socket = "/tmp/wolf.sock".into();
        c.data_dir = std::path::Path::new("/").join("x".repeat(100));
        assert!(c.validate().is_err());
    }
    #[test]
    fn connection_save_obeys_owned_viewer_lease_and_busy_operation() {
        let dir = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(dir.path().join("data")), true).unwrap();
        let config = core.streaming_connection().unwrap();
        let lease = super::super::viewer::configuration_lease(&config.data_dir).unwrap();
        assert!(core.streaming_save_connection(config.clone()).is_err());
        drop(lease);
        core.streaming_save_connection(config.clone()).unwrap();
        assert_eq!(core.streaming_connection().unwrap().socket, config.socket);
        core.streaming_controls.operation.lock().unwrap().name = Some("viewer");
        assert_eq!(
            core.streaming_save_connection(config).unwrap_err().code,
            "STREAMING_BUSY"
        );
    }

    #[test]
    fn destructive_api_calls_require_confirmation_before_contacting_host() {
        let dir = tempfile::tempdir().unwrap();
        let core = Arc::new(Core::open_isolated(Some(dir.path().join("data")), true).unwrap());
        for command in ["streaming_end_game", "streaming_revoke"] {
            assert_eq!(
                crate::api::dispatch(&core, command, json!({}))
                    .unwrap_err()
                    .code,
                "STREAMING_CONFIRM"
            );
        }
        let snapshot = core.streaming_state().unwrap();
        assert!(snapshot["host"].is_null());
        assert!(!snapshot["host_error"].is_null());
        assert_eq!(snapshot["receiver_installed"], false);
    }

    #[test]
    fn receiver_failure_is_not_reported_as_successful_disconnect() {
        assert!(completed(json!({"reason":"receiver-exited","exit_code":7})).is_err());
        assert!(completed(json!({"reason":"receiver-exited","exit_code":null})).is_err());
        assert!(completed(json!({"reason":"receiver-exited","exit_code":0})).is_ok());
        assert!(completed(json!({"reason":"viewer-disconnected"})).is_ok());
    }

    #[test]
    fn background_failure_releases_operation_and_exposes_error() {
        let dir = tempfile::tempdir().unwrap();
        let manager = Arc::new(Manager::default());
        manager
            .start(
                || {
                    Ok(Connection {
                        data_dir: dir.path().join("data"),
                        socket: dir.path().join("absent.sock"),
                    })
                },
                Action::View("1".into()),
            )
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            {
                let operation = manager.operation.lock().unwrap();
                if operation.name.is_none() {
                    assert!(operation.error.is_some());
                    break;
                }
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}
