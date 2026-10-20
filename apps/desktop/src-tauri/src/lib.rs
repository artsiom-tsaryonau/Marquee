mod commands;

use marquee_core::server::{self, Assets};
use marquee_core::Core;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tauri::{Emitter, Manager};

pub struct AppState {
    pub core: Arc<Core>,
    pub web_url: Option<String>,
}

/// UI files for the browser: the bundled assets in a release build, or the built
/// `apps/desktop/dist` folder when running from the source tree.
fn web_assets(handle: tauri::AppHandle) -> Assets {
    let dist = std::env::current_exe().ok().and_then(|exe| {
        exe.ancestors()
            .map(|dir| dir.join("apps/desktop/dist"))
            .find(|dir| dir.join("index.html").is_file())
    });
    let from_dir = dist.map(server::dir_assets);
    Arc::new(move |rel: &str| {
        handle
            .asset_resolver()
            .get(rel.to_string())
            .map(|asset| (asset.bytes, asset.mime_type))
            .or_else(|| from_dir.as_ref().and_then(|dir| dir(rel)))
    })
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
            let core = Arc::new(Core::open(data_dir, allow_fake).map_err(Box::new)?);

            // Browser access to this same library; the app still works if it cannot bind.
            let port = std::env::var("MARQUEE_PORT")
                .ok()
                .and_then(|p| p.parse().ok());
            let web_url = match server::start(
                Arc::clone(&core),
                Some(web_assets(app.handle().clone())),
                port,
            ) {
                Ok(handle) => Some(handle.url()),
                Err(err) => {
                    eprintln!("web access disabled: {err}");
                    None
                }
            };

            // Forward core events (from this window or from a browser tab) to the window.
            let hub = core.hub();
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                let mut after = hub.seq();
                loop {
                    let (seq, events) = hub.wait_after(after, Duration::from_secs(30));
                    after = seq;
                    for event in events {
                        let _ = app_handle.emit("core-event", event);
                    }
                }
            });

            app.manage(AppState { core, web_url });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::core_call,
            commands::web_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running Marquee");
}
