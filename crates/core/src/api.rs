//! The single command surface. The Tauri window and the local HTTP server both
//! call `dispatch`, so a browser tab and the native app can never drift apart.

use crate::error::{AppError, Result};
use crate::sessions::EventSink;
use crate::Core;
use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;

fn text(args: &Value, key: &str) -> Result<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| AppError::new("BAD_REQUEST", format!("missing argument: {key}")))
}

fn maybe(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_string)
}

fn out<T: Serialize>(value: T) -> Result<Value> {
    Ok(serde_json::to_value(value)?)
}

pub fn dispatch(core: &Arc<Core>, cmd: &str, args: Value) -> Result<Value> {
    let sink: Arc<dyn EventSink> = core.hub();
    match cmd {
        "health" => out(core.health()?),
        "library_snapshot" => out(core.snapshot()?),
        "add_library_root" => {
            let path = PathBuf::from(text(&args, "path")?);
            out(core.add_root(path, Some("PlayStation 2".into()))?)
        }
        "scan_library_root" => {
            let root = maybe(&args, "root_id");
            out(core.scan(root.as_deref(), Some(sink))?)
        }
        "list_runners" => out(core.refresh_runners()?),
        "select_runner" => out(core.select_runner(&text(&args, "runner_id")?)?),
        "open_runner_setup" => out(core.open_setup(&text(&args, "runner_id")?)?),
        "launch_game" => out(core.launch(&text(&args, "edition_id")?, &[], sink)?),
        "set_favourite" => {
            let favorite = args
                .get("favorite")
                .and_then(Value::as_bool)
                .ok_or_else(|| AppError::new("BAD_REQUEST", "missing argument: favorite"))?;
            out(core.set_favourite(&text(&args, "edition_id")?, favorite)?)
        }
        "set_display_title" => {
            out(core.set_display_title(&text(&args, "edition_id")?, &text(&args, "title")?)?)
        }
        "install_pcsx2" => {
            let channel = maybe(&args, "channel").unwrap_or_else(|| "auto".into());
            out(core.install_pcsx2(&channel, sink)?)
        }
        "refresh_artwork" => {
            let edition = maybe(&args, "edition_id");
            out(core.refresh_artwork(edition.as_deref(), Some(sink))?)
        }
        "list_ports" => out(core.list_ports()?),
        "check_port_updates" => out(core.check_port_updates()?),
        "install_port" => out(core.install_port(&text(&args, "port_id")?, sink)?),
        "launch_port" => out(core.launch_port(&text(&args, "port_id")?)?),
        "open_port_folder" => out(core.open_port_folder(&text(&args, "port_id")?)?),
        other => Err(AppError::new(
            "BAD_REQUEST",
            format!("unknown command: {other}"),
        )),
    }
}
