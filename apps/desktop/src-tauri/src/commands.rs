use crate::{event_sink, AppState};
use marquee_core::error::{AppError, Result};
use marquee_core::model::{LibraryRoot, LibrarySnapshot, Runner, ScanReport, Session};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, State};

fn core(state: &State<AppState>) -> Arc<marquee_core::Core> {
    Arc::clone(&state.core)
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|err| AppError::new("IO", err.to_string()))?
}

#[tauri::command]
pub async fn health(state: State<'_, AppState>) -> Result<String> {
    let core = core(&state);
    blocking(move || {
        let snap = core.snapshot()?;
        Ok(format!(
            "ok:{}:{}",
            snap.editions.len(),
            core.paths.data_dir.display()
        ))
    })
    .await
}

#[tauri::command]
pub async fn library_snapshot(state: State<'_, AppState>) -> Result<LibrarySnapshot> {
    let core = core(&state);
    blocking(move || core.snapshot()).await
}

#[tauri::command]
pub async fn add_library_root(state: State<'_, AppState>, path: String) -> Result<LibraryRoot> {
    let core = core(&state);
    blocking(move || core.add_root(PathBuf::from(path), Some("PlayStation 2".into()))).await
}

#[tauri::command]
pub async fn scan_library_root(
    app: AppHandle,
    state: State<'_, AppState>,
    root_id: Option<String>,
) -> Result<ScanReport> {
    let core = core(&state);
    let sink = event_sink(&app);
    blocking(move || core.scan(root_id.as_deref(), Some(sink))).await
}

#[tauri::command]
pub async fn list_runners(state: State<'_, AppState>) -> Result<Vec<Runner>> {
    let core = core(&state);
    blocking(move || core.refresh_runners()).await
}

#[tauri::command]
pub async fn select_runner(state: State<'_, AppState>, runner_id: String) -> Result<()> {
    let core = core(&state);
    blocking(move || core.select_runner(&runner_id)).await
}

#[tauri::command]
pub async fn open_runner_setup(state: State<'_, AppState>, runner_id: String) -> Result<()> {
    let core = core(&state);
    blocking(move || core.open_setup(&runner_id)).await
}

#[tauri::command]
pub async fn launch_game(
    app: AppHandle,
    state: State<'_, AppState>,
    edition_id: String,
) -> Result<Session> {
    let core = core(&state);
    let sink = event_sink(&app);
    blocking(move || core.launch(&edition_id, &[], sink)).await
}

#[tauri::command]
pub async fn set_favourite(
    state: State<'_, AppState>,
    edition_id: String,
    favorite: bool,
) -> Result<()> {
    let core = core(&state);
    blocking(move || core.set_favourite(&edition_id, favorite)).await
}

#[tauri::command]
pub async fn set_display_title(
    state: State<'_, AppState>,
    edition_id: String,
    title: String,
) -> Result<()> {
    let core = core(&state);
    blocking(move || core.set_display_title(&edition_id, &title)).await
}
