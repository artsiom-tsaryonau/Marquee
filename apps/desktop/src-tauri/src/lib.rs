mod commands;

use marquee_core::model::CoreEvent;
use marquee_core::sessions::EventSink;
use marquee_core::Core;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

pub struct AppState {
    pub core: std::sync::Arc<Core>,
}

struct TauriSink {
    app: AppHandle,
}

impl EventSink for TauriSink {
    fn emit(&self, event: CoreEvent) {
        let _ = self.app.emit("core-event", event);
    }
}

pub fn event_sink(app: &AppHandle) -> Arc<dyn EventSink> {
    Arc::new(TauriSink { app: app.clone() })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = std::env::var_os("MARQUEE_DATA_DIR").map(PathBuf::from);
            let allow_fake = std::env::var_os("MARQUEE_ALLOW_FAKE").is_some();
            match Core::open(data_dir, allow_fake) {
                Ok(core) => {
                    app.manage(AppState {
                        core: std::sync::Arc::new(core),
                    });
                    Ok(())
                }
                Err(err) => Err(Box::new(err)),
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::health,
            commands::library_snapshot,
            commands::add_library_root,
            commands::scan_library_root,
            commands::list_runners,
            commands::select_runner,
            commands::open_runner_setup,
            commands::launch_game,
            commands::set_favourite,
            commands::set_display_title,
            commands::install_pcsx2,
            commands::refresh_artwork,
            commands::list_ports,
            commands::check_port_updates,
            commands::install_port,
            commands::launch_port,
            commands::open_port_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Marquee");
}
