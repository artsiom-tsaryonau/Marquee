use crate::{
    error::{AppError, Result},
    Core,
};
use marquee_manager::relocation::{self, Offline, Summary};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Default)]
pub struct MoveState {
    pub running: bool,
    pub action: Option<String>,
    pub error: Option<AppError>,
}

impl Core {
    pub(crate) fn storage_move_ready(&self) -> Result<()> {
        if self
            .storage_moves
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .running
            || relocation::incomplete(&self.paths.data_dir)?
        {
            return Err(AppError::new("STORAGE_BUSY", "Finish or recover the installation move in Storage settings before launching or scanning games"));
        }
        Ok(())
    }
    pub fn storage_move_status(&self) -> Result<Value> {
        let plan = relocation::status(&self.paths.data_dir)?;
        let affected = self.with_db(|db| {
            let mut games = Vec::new();
            if let Some(plan) = &plan {
                for id in &plan.receipt_ids {
                    let rows = db.conn.prepare("SELECT edition_ids FROM preparation_receipts WHERE id=?1")?.query_map([id], |r| r.get::<_,String>(0))?.collect::<std::result::Result<Vec<_>,_>>()?;
                    for row in rows {
                        for edition in serde_json::from_str::<Vec<String>>(&row)? {
                            if let Some(e)=db.edition_by_id(&edition)? { games.push(json!({"id":e.id,"title":e.display_title.unwrap_or(e.scan_title)})); }
                        }
                    }
                }
            }
            Ok(games)
        })?;
        let manager = self.preparation.status()?;
        let state = self.storage_moves.lock().unwrap_or_else(|e| e.into_inner());
        Ok(
            json!({"running":state.running,"action":state.action,"error":state.error,"plan":plan,"games":affected,"manager_running":manager.running,"manager_stopping":manager.stopping}),
        )
    }
    pub fn storage_move_start(self: &Arc<Self>, action: &str, value: &str) -> Result<()> {
        if !matches!(action, "review" | "apply" | "rollback") {
            return Err(AppError::new("STORAGE_MOVE", "Unknown move action"));
        }
        if value.is_empty() || value.len() > 8192 {
            return Err(AppError::new(
                "STORAGE_MOVE",
                "Choose a destination or current move review",
            ));
        }
        {
            let mut state = self.storage_moves.lock().unwrap_or_else(|e| e.into_inner());
            if state.running {
                return Err(AppError::new(
                    "STORAGE_BUSY",
                    "An installation move operation is already running",
                ));
            }
            state.running = true;
            state.action = Some(action.into());
            state.error = None;
        }
        let core = Arc::clone(self);
        let action = action.to_string();
        let value = value.to_string();
        std::thread::spawn(move || {
            let result: Result<Summary> = (|| {
                let _gate = core.headless_gate.lock().unwrap_or_else(|e| e.into_inner());
                if core.with_db(|db| db.running_session())?.is_some() {
                    return Err(AppError::new(
                        "STORAGE_BUSY",
                        "Close the running game before moving installations",
                    ));
                }
                if core.discover_host {
                    let status = core.headless_status(None)?;
                    if status["host_running"] == true
                        || status["busy"] == true
                        || !status["host_error"].is_null()
                    {
                        return Err(AppError::new(
                            "STORAGE_BUSY",
                            "Stop the owned streaming host before moving installations",
                        ));
                    }
                }
                if core.preparation.status()?.running {
                    return Err(AppError::new("STORAGE_BUSY", "Stop the manager and let accepted work finish before reviewing or moving installations"));
                }
                let owner = Offline::acquire(&core.paths.data_dir)?;
                match action.as_str() {
                    "review" => owner.review(&PathBuf::from(value)),
                    "apply" => owner.apply(&value),
                    _ => owner.rollback(&value),
                }
            })();
            if result.is_ok() {
                let mut scan = core.storage.lock().unwrap_or_else(|e| e.into_inner());
                scan.usage.clear();
                scan.measured_at = None;
            }
            let mut state = core.storage_moves.lock().unwrap_or_else(|e| e.into_inner());
            state.running = false;
            state.error = result.err();
        });
        Ok(())
    }
}
