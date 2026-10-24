use crate::AppState;
use marquee_core::api;
use marquee_core::error::{AppError, Result};
use serde_json::Value;
use std::sync::Arc;
use tauri::State;

/// Every UI action goes through the same `dispatch` the local web server uses.
#[tauri::command]
pub async fn core_call(
    state: State<'_, AppState>,
    cmd: String,
    args: Option<Value>,
) -> Result<Value> {
    let core = Arc::clone(&state.core);
    tauri::async_runtime::spawn_blocking(move || {
        api::dispatch(&core, &cmd, args.unwrap_or(Value::Null))
    })
    .await
    .map_err(|err| AppError::new("IO", err.to_string()))?
}

/// Address (with token) that opens this same library in a browser.
#[tauri::command]
pub fn web_url(state: State<'_, AppState>) -> Option<String> {
    state.web_url.clone()
}

/// Lazily start a loopback media surface; URLs grant read access to one track.
#[tauri::command]
pub fn music_url(state: State<'_, AppState>, track_id: String) -> Result<String> {
    state.core.music_open(&track_id)?;
    let mut server = state.music_server.lock().unwrap_or_else(|e| e.into_inner());
    if server.is_none() {
        *server = Some(marquee_core::server::start(
            Arc::clone(&state.core),
            None,
            None,
        )?);
    }
    Ok(server
        .as_ref()
        .expect("server initialized")
        .music_url(&track_id))
}
