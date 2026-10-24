//! The single command surface. The Tauri window and the local HTTP server both
//! call `dispatch`, so a browser tab and the native app can never drift apart.

use crate::catalog;
use crate::error::{AppError, Result};
use crate::sessions::EventSink;
use crate::transfer;
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

fn flag(args: &Value, key: &str) -> Result<bool> {
    args.get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| AppError::new("BAD_REQUEST", format!("missing argument: {key}")))
}

fn out<T: Serialize>(value: T) -> Result<Value> {
    Ok(serde_json::to_value(value)?)
}

pub fn dispatch(core: &Arc<Core>, cmd: &str, args: Value) -> Result<Value> {
    let sink: Arc<dyn EventSink> = core.hub();
    match cmd {
        "preview_app_list" => {
            out(core.preview_app_list(&text(&args, "source")?, &text(&args, "json")?)?)
        }
        "approve_app_list" => out(core.approve_app_list(
            &text(&args, "token")?,
            &serde_json::from_value::<Vec<String>>(
                args.get("ids").cloned().unwrap_or(Value::Null),
            )?,
        )?),
        "app_list_entries" => out(core.app_list_entries()?),
        "ps3_firmware_status" => out(core.ps3_firmware_status()?),
        "review_ps3_firmware" => {
            out(core.review_ps3_firmware(PathBuf::from(text(&args, "path")?).as_path())?)
        }
        "install_ps3_firmware" => out(core.install_ps3_firmware(&text(&args, "review_token")?)?),
        "import_ps3_disc" => {
            out(core.import_ps3_disc(PathBuf::from(text(&args, "path")?).as_path())?)
        }
        "inspect_ps3" => out(crate::ps3::inspect(
            PathBuf::from(text(&args, "path")?).as_path(),
        )?),
        "search_game_facts" => {
            out(core.search_game_facts(&text(&args, "edition_id")?, &text(&args, "query")?)?)
        }
        "apply_game_facts" => out(core.apply_game_facts(
            &text(&args, "edition_id")?,
            &text(&args, "token")?,
            args.get("candidate_id")
                .and_then(Value::as_u64)
                .ok_or_else(|| AppError::new("BAD_REQUEST", "Missing candidate ID"))?,
        )?),
        "search_game_metadata" => {
            out(core.search_game_metadata(&text(&args, "edition_id")?, &text(&args, "query")?)?)
        }
        "apply_metadata_match" => out(core.apply_metadata_match(
            &text(&args, "edition_id")?,
            &text(&args, "token")?,
            args.get("candidate_id")
                .and_then(Value::as_u64)
                .ok_or_else(|| AppError::new("BAD_REQUEST", "Missing candidate ID"))?,
        )?),
        "media_collections" => out(core.media_collections(maybe(&args, "game_key").as_deref())?),
        "media_create" => {
            out(core.media_create(&text(&args, "title")?, maybe(&args, "game_key").as_deref())?)
        }
        "media_link" => out(core.media_link(
            &text(&args, "id")?,
            &text(&args, "game_key")?,
            args.get("linked").and_then(Value::as_bool).unwrap_or(true),
        )?),
        "media_inspect_disc" => out(crate::game_media::inspect(
            PathBuf::from(text(&args, "path")?).as_path(),
        )?),
        "media_import" => out(core.media_import(
            &text(&args, "id")?,
            serde_json::from_value(args.get("paths").cloned().unwrap_or_default())?,
        )?),
        "media_import_disc" => out(core.media_import_disc(
            &text(&args, "id")?,
            PathBuf::from(text(&args, "path")?).as_path(),
            &text(&args, "signature")?,
            &text(&args, "member")?,
        )?),
        "media_position" => out(core.media_position(
            &text(&args, "id")?,
            args.get("position")
                .and_then(Value::as_f64)
                .ok_or_else(|| AppError::new("BAD_REQUEST", "Missing playback position"))?,
        )?),
        "media_update" => out(core.media_update(
            &text(&args, "id")?,
            &text(&args, "title")?,
            flag(&args, "spoiler")?,
        )?),
        "media_remove" => out(core.media_remove(&text(&args, "id")?)?),
        "music_search_youtube" => out(core.music_search_youtube(
            &text(&args, "query")?,
            serde_json::from_value(
                args.get("kind")
                    .cloned()
                    .unwrap_or(Value::String("playlist".into())),
            )?,
        )?),
        "music_inspect_youtube" => out(core.music_inspect_youtube(&text(&args, "url")?)?),
        "music_open_youtube" => out(core.music_open_youtube(&text(&args, "url")?)?),
        "music_link_youtube" => out(core.music_link_youtube(
            &text(&args, "game_key")?,
            &text(&args, "url")?,
            &text(&args, "title")?,
        )?),
        "music_find" => out(core.music_find(&text(&args, "title")?)?),
        "music_open_source" => out(core.music_open_source(&text(&args, "album_id")?)?),
        "music_albums" => out(core.music_albums(maybe(&args, "game_key").as_deref())?),
        "music_save_album" => out(core.music_save_album(serde_json::from_value(args)?)?),
        "music_set_cover" => out(core.music_set_cover(
            &text(&args, "album_id")?,
            maybe(&args, "path").map(PathBuf::from),
        )?),
        "music_link" => {
            out(core.music_link(&text(&args, "game_key")?, &text(&args, "album_id")?, true)?)
        }
        "music_unlink" => {
            out(core.music_link(&text(&args, "game_key")?, &text(&args, "album_id")?, false)?)
        }
        "music_import" => out(core.music_import(
            &text(&args, "album_id")?,
            serde_json::from_value(args.get("paths").cloned().unwrap_or_default())?,
        )?),
        "music_remove_track" => {
            out(core.music_remove_track(&text(&args, "album_id")?, &text(&args, "track_id")?)?)
        }
        "streaming_state" => core.streaming_state(),
        "streaming_save_connection" => {
            out(core.streaming_save_connection(serde_json::from_value(args)?)?)
        }
        "streaming_open_viewer" => out(core.streaming_open_viewer(text(&args, "app_id")?)?),
        "streaming_pair_viewer" => out(core.streaming_pair_viewer()?),
        "streaming_install_viewer" => out(core.streaming_install_viewer()?),
        "streaming_disconnect_viewer" => core.streaming_disconnect_viewer(),
        "streaming_pair" => {
            out(core.streaming_pair(&text(&args, "client_ip")?, &text(&args, "pin")?)?)
        }
        "streaming_revoke" => {
            if args.get("confirmed").and_then(Value::as_bool) != Some(true) {
                return Err(AppError::new(
                    "STREAMING_CONFIRM",
                    "Confirm revoking this client's access",
                ));
            }
            out(core.streaming_revoke(&text(&args, "client_id")?)?)
        }
        "streaming_end_game" => {
            if args.get("confirmed").and_then(Value::as_bool) != Some(true) {
                return Err(AppError::new(
                    "STREAMING_CONFIRM",
                    "Confirm ending this game; unsaved progress can be lost",
                ));
            }
            out(core.streaming_end_game(&text(&args, "session_id")?)?)
        }
        "health" => out(core.health()?),
        "library_snapshot" => out(core.snapshot()?),
        "import_catalogue" => {
            let data = serde_json::from_value(args.get("snapshot").cloned().unwrap_or(Value::Null))
                .map_err(|e| {
                    AppError::new("BAD_REQUEST", format!("Invalid catalogue snapshot: {e}"))
                })?;
            out(core.import_catalogue(data)?)
        }
        "link_catalogue" => out(core.link_catalogue(
            &text(&args, "edition_id")?,
            &text(&args, "record_id")?,
            &text(&args, "media_id")?,
        )?),
        "unlink_catalogue" => out(core.unlink_catalogue(&text(&args, "edition_id")?)?),
        "match_catalogue" => out(core.match_catalogue(&text(&args, "edition_id")?)?),
        "clear_catalogue" => out(core.clear_catalogue()?),
        "open_catalogue_page" => out(core.open_catalogue_page(&text(&args, "record_id")?)?),
        "add_library_root" => {
            let path = PathBuf::from(text(&args, "path")?);
            let system = maybe(&args, "system").unwrap_or_else(|| "ps2".into());
            let label = catalog::by_id(&system)?.display_name;
            out(core.add_root(&system, path, Some(label))?)
        }
        "scan_library_root" => {
            let root = maybe(&args, "root_id");
            out(core.scan(root.as_deref(), Some(sink))?)
        }
        "list_runners" => out(core.refresh_runners()?),
        "select_system_runner" => {
            out(core.select_system_runner(&text(&args, "system")?, &text(&args, "runner_id")?)?)
        }
        "set_game_runner" => out(core.set_game_runner(
            &text(&args, "edition_id")?,
            maybe(&args, "runner_id").as_deref(),
        )?),
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
        "add_wanted_game" => out(core.add_wanted(
            &text(&args, "system")?,
            &text(&args, "title")?,
            &text(&args, "status")?,
        )?),
        "set_wanted_status" => {
            out(core.set_wanted_status(&text(&args, "edition_id")?, &text(&args, "status")?)?)
        }
        "remove_wanted_game" => out(core.remove_wanted(&text(&args, "edition_id")?)?),
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
        "set_display_mode" => out(core.set_display_mode(&text(&args, "mode")?)?),
        "manager_status" => out(core.preparation.status()?),
        "torrent_downloads" => out(core.preparation.torrent_downloads()?),
        "inspect_torrent_preparation" => out(core.preparation.inspect_torrent(
            &text(&args, "info_hash")?,
            std::path::Path::new(&text(&args, "path")?),
            &text(&args, "system")?,
        )?),
        "browser_jobs" => out(core.preparation.browser_jobs()?),
        "inspect_browser_preparation" => out(core
            .preparation
            .inspect_browser(&text(&args, "id")?, &text(&args, "system")?)?),
        "cancel_browser_job" => out(core
            .preparation
            .request_browser_cancel(&text(&args, "id")?)?),
        "start_manager" => out(core.preparation.start_manager()?),
        "stop_manager" => out(core.preparation.stop()?),
        "preparation_systems" => out(catalog::all()?
            .into_iter()
            .filter(|d| d.id != "ps3")
            .map(|d| serde_json::json!({"id":d.id,"name":d.display_name}))
            .collect::<Vec<_>>()),
        "preparation_jobs" => out(core.preparation_jobs()?),
        "inspect_preparation" => out(core.inspect_preparation(
            std::path::Path::new(&text(&args, "path")?),
            &text(&args, "system")?,
        )?),
        "start_preparation" => {
            out(core.start_preparation(&text(&args, "id")?, &text(&args, "review_token")?)?)
        }
        "cancel_preparation" => out(core.cancel_preparation(&text(&args, "id")?)?),
        "intake_archive" => {
            let path = std::path::PathBuf::from(text(&args, "path")?);
            let delete = args
                .get("delete_source")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            out(core.intake_archive(&path, delete)?)
        }
        "transfer_status" => out(transfer::status(core)?),
        "transfer_share" => out(transfer::share(core, &text(&args, "edition_id")?)?),
        "transfer_get" => out(transfer::get(core, &text(&args, "link")?)?),
        "transfer_review" => out(transfer::review(
            core,
            std::path::Path::new(&text(&args, "path")?),
        )?),
        "transfer_add_file" => {
            let peers = args
                .get("peers")
                .cloned()
                .map(serde_json::from_value)
                .transpose()?
                .unwrap_or_default();
            out(transfer::add_file(
                core,
                std::path::Path::new(&text(&args, "path")?),
                &text(&args, "review_token")?,
                peers,
            )?)
        }
        "transfer_pause" => out(transfer::pause(core, &text(&args, "info_hash")?, true)?),
        "transfer_resume" => out(transfer::pause(core, &text(&args, "info_hash")?, false)?),
        "transfer_stop" => out(transfer::stop(core, &text(&args, "info_hash")?)?),
        "get_torrent_settings" => out(core.torrent_settings()?),
        "set_torrent_settings" => {
            let t = serde_json::from_value(args.get("settings").cloned().unwrap_or_default())?;
            out(core.set_torrent_settings(&t)?)
        }
        "prune_library" => out(core.prune_missing()?),
        "list_emulators" => out(core.list_emulators()?),
        "build_emulator" => out(core.build_emulator(&text(&args, "id")?)?),
        "emulator_build_status" => out(core
            .source_builds
            .status(&core.paths, &text(&args, "id")?)?),
        "cancel_emulator_build" => out(core.source_builds.cancel(&text(&args, "id")?)?),
        "rollback_emulator" => out(core.rollback_emulator(&text(&args, "id")?)?),
        "check_emulator_updates" => out(core.check_emulator_updates()?),
        "install_emulator" => out(core.install_emulator(&text(&args, "id")?, sink)?),
        "emulator_profile" => out(core.emulator_profile(&text(&args, "id")?)?),
        "configure_emulator_profile" => {
            let config = serde_json::from_value(args.get("config").cloned().unwrap_or(Value::Null))
                .map_err(|e| {
                    AppError::new("BAD_REQUEST", format!("Invalid settings profile: {e}"))
                })?;
            out(core.configure_emulator_profile(&text(&args, "id")?, config)?)
        }
        "configure_emulator" => {
            let list = args
                .get("args")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            out(core.configure_emulator(&text(&args, "id")?, list)?)
        }
        "open_emulator" => out(core.open_emulator(&text(&args, "id")?)?),
        "uninstall_emulator" => {
            let purge = args
                .get("purge_data")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            out(core.uninstall_emulator(&text(&args, "id")?, purge)?)
        }
        "list_bios" => out(core.list_bios()?),
        "import_bios" => out(core.import_bios(&PathBuf::from(text(&args, "path")?))?),
        "remove_bios" => out(core.remove_bios(&text(&args, "system")?)?),
        "uninstall_pcsx2" => out(core.uninstall_pcsx2()?),
        "list_keys" => out(core.list_keys()?),
        "set_key" => out(core.set_key(&text(&args, "id")?, &text(&args, "value")?)?),
        "clear_key" => out(core.clear_key(&text(&args, "id")?)?),
        "inspect_steam" => out(core.inspect_steam()),
        "lutris_library" => out(core.lutris_library()?),
        "configure_lutris" => {
            let config = serde_json::from_value(args.get("config").cloned().unwrap_or(Value::Null))
                .map_err(|e| {
                    AppError::new("BAD_REQUEST", format!("Invalid Lutris configuration: {e}"))
                })?;
            out(core.configure_lutris(config)?)
        }
        "open_lutris_game" => out(core.open_lutris_game(&text(&args, "id")?)?),
        "list_launchers" => out(core.list_launchers()?),
        "purple_status" => out(core.purple.status()?),
        "install_purple" => out(core.purple.start(Arc::clone(core))?),
        "cancel_purple" => out(core.purple.cancel()?),
        "open_purple" => out(core.purple.open(Arc::clone(core))?),
        "save_launcher" => {
            let launcher =
                serde_json::from_value(args.get("launcher").cloned().unwrap_or(Value::Null))
                    .map_err(|e| AppError::new("BAD_REQUEST", format!("Invalid launcher: {e}")))?;
            out(core.save_launcher(launcher)?)
        }
        "remove_launcher" => out(core.remove_launcher(&text(&args, "id")?)?),
        "open_launcher" => out(core.open_launcher(&text(&args, "id")?)?),
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
