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
    pub music_server: std::sync::Mutex<Option<server::ServerHandle>>,
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
    // This must happen before GTK opens a display or starts a WebKit process.
    #[cfg(target_os = "linux")]
    {
        let requested = std::env::var("MARQUEE_RENDERER").ok();
        let disable = std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").ok();
        let shm = std::env::var("WEBKIT_DMABUF_RENDERER_FORCE_SHM").ok();
        let mode = marquee_core::desktop_rendering::select(
            requested.as_deref(),
            disable.as_deref(),
            shm.as_deref(),
            marquee_core::desktop_rendering::nvidia_wayland(),
        )
        .unwrap_or_else(|error| {
            eprintln!("{error}");
            std::process::exit(2);
        });
        for (key, value) in mode.environment() {
            std::env::set_var(key, value);
        }
        eprintln!(
            "Marquee renderer: {} (MARQUEE_RENDERER=auto|gpu|shared-memory|software|x11)",
            mode.name()
        );
    }
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

            // CLI preparations may share this library while using its instance-local
            // cache. Grant only artwork trees, never the whole data/home directory.
            for art in [
                core.paths.cache_dir.join("art"),
                core.paths.data_dir.join("cache/art"),
            ] {
                std::fs::create_dir_all(&art)?;
                app.asset_protocol_scope().allow_directory(art, true)?;
            }

            // The desktop app is standalone by default. Set MARQUEE_WEB=1 (or MARQUEE_PORT)
            // to also serve this same library to a browser; `marquee run web` runs the web
            // UI without the app.
            let port = std::env::var("MARQUEE_PORT")
                .ok()
                .and_then(|p| p.parse().ok());
            let web_wanted = port.is_some()
                || std::env::var("MARQUEE_WEB").is_ok_and(|v| !matches!(v.as_str(), "" | "0"));
            let web_url = if web_wanted {
                match server::start(
                    Arc::clone(&core),
                    Some(web_assets(app.handle().clone())),
                    port,
                ) {
                    Ok(handle) => Some(handle.url()),
                    Err(err) => {
                        eprintln!("web access disabled: {err}");
                        None
                    }
                }
            } else {
                None
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
                        if matches!(
                            &event,
                            marquee_core::model::CoreEvent::SessionEnded { .. }
                                | marquee_core::model::CoreEvent::SessionFailed { .. }
                        ) {
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.show();
                            }
                        }
                        let _ = app_handle.emit("core-event", event);
                    }
                }
            });

            app.manage(AppState {
                core,
                web_url,
                music_server: std::sync::Mutex::new(None),
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if let Some(state) = window.app_handle().try_state::<AppState>() {
                    if state.core.snapshot().is_ok_and(|s| {
                        s.sessions
                            .iter()
                            .any(|s| s.state == marquee_core::model::SessionState::Running)
                    }) {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::core_call,
            commands::web_url,
            commands::music_url,
            commands::media_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running Marquee");
}
