use crate::error::AppError;
use crate::iso::build_test_iso;
use crate::model::{BiosState, RunnerKind, SessionState};
use crate::runners;
use crate::{Core, NoopSink};
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn write_iso(path: &Path, serial_line: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let bytes = build_test_iso(&[(
        "SYSTEM.CNF",
        format!("{serial_line}\nVER = 1.00\n").as_bytes(),
    )]);
    File::create(path).unwrap().write_all(&bytes).unwrap();
}

fn wait_session(core: &Core, id: &str) -> crate::model::Session {
    for _ in 0..80 {
        let snap = core.snapshot().unwrap();
        if let Some(session) = snap.sessions.iter().find(|s| s.id == id) {
            if session.state != SessionState::Running {
                return session.clone();
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("session {id} did not finish");
}

#[test]
fn schema_survives_reopen_and_keeps_overrides() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let games = dir.path().join("games");
    write_iso(&games.join("Shadow.iso"), "BOOT2 = cdrom0:\\SLUS_209.63;1");
    let id = {
        let core = Core::open(Some(data.clone()), true).unwrap();
        let root = core.add_root(games.clone(), Some("PS2".into())).unwrap();
        core.scan(Some(&root.id), None).unwrap();
        let edition = core.snapshot().unwrap().editions.remove(0);
        core.set_favourite(&edition.id, true).unwrap();
        core.set_display_title(&edition.id, "Shadow of the Colossus")
            .unwrap();
        edition.id
    };
    let core = Core::open(Some(data), true).unwrap();
    let edition = core
        .snapshot()
        .unwrap()
        .editions
        .into_iter()
        .find(|e| e.id == id)
        .unwrap();
    assert!(edition.favorite);
    assert_eq!(edition.title, "Shadow of the Colossus");
    assert_eq!(edition.scan_title, "Shadow");
    assert_eq!(edition.serial.as_deref(), Some("SLUS_209.63"));
}

#[test]
fn scan_nested_uppercase_iso_and_counts_skipped_formats() {
    let dir = tempfile::tempdir().unwrap();
    let games = dir.path().join("library");
    write_iso(
        &games.join("nested").join("GAME.ISO"),
        "BOOT2 = cdrom0:\\SLES_123.45;1",
    );
    File::create(games.join("skip.chd"))
        .unwrap()
        .write_all(b"chd")
        .unwrap();
    File::create(games.join("disc.cue"))
        .unwrap()
        .write_all(b"cue")
        .unwrap();
    File::create(games.join("track.bin"))
        .unwrap()
        .write_all(b"bin")
        .unwrap();
    File::create(games.join("dump.gz"))
        .unwrap()
        .write_all(b"gz")
        .unwrap();
    File::create(games.join("comp.cso"))
        .unwrap()
        .write_all(b"cso")
        .unwrap();
    let core = Core::open(Some(dir.path().join("data")), true).unwrap();
    let root = core.add_root(games, None).unwrap();
    let report = core.scan(Some(&root.id), None).unwrap();
    assert_eq!(report.added, 1);
    assert_eq!(report.skipped_unsupported, 5);
    let kinds: Vec<_> = report
        .skipped_by_kind
        .iter()
        .map(|s| (s.kind.as_str(), s.count))
        .collect();
    assert!(kinds.contains(&("ps2_chd", 1)));
    assert!(kinds.contains(&("ps2_cue", 1)));
    let editions = core.snapshot().unwrap().editions;
    assert_eq!(editions.len(), 1);
    assert_eq!(editions[0].serial.as_deref(), Some("SLES_123.45"));
}

#[test]
fn rescan_keeps_ids_and_does_not_overwrite_title() {
    let dir = tempfile::tempdir().unwrap();
    let games = dir.path().join("games");
    write_iso(&games.join("Alpha.iso"), "BOOT2 = cdrom0:\\SCES_503.82;1");
    let core = Core::open(Some(dir.path().join("data")), true).unwrap();
    let root = core.add_root(games, None).unwrap();
    core.scan(Some(&root.id), None).unwrap();
    let first = core.snapshot().unwrap().editions.remove(0);
    core.set_display_title(&first.id, "Kept Title").unwrap();
    let report = core.scan(Some(&root.id), None).unwrap();
    assert_eq!(report.added, 0);
    let second = core.snapshot().unwrap().editions.remove(0);
    assert_eq!(first.id, second.id);
    assert_eq!(second.title, "Kept Title");
    assert_eq!(second.scan_title, "Alpha");
}

#[test]
fn missing_file_differs_from_offline_root_and_move_is_not_merged() {
    let dir = tempfile::tempdir().unwrap();
    let games = dir.path().join("games");
    write_iso(&games.join("One.iso"), "BOOT2 = cdrom0:\\SLUS_200.01;1");
    let core = Core::open(Some(dir.path().join("data")), true).unwrap();
    let root = core.add_root(games.clone(), None).unwrap();
    core.scan(Some(&root.id), None).unwrap();
    let original = core.snapshot().unwrap().editions.remove(0);

    fs::rename(games.join("One.iso"), games.join("Two.iso")).unwrap();
    core.scan(Some(&root.id), None).unwrap();
    let snap = core.snapshot().unwrap();
    assert_eq!(snap.editions.len(), 2);
    let old = snap.editions.iter().find(|e| e.id == original.id).unwrap();
    let new = snap.editions.iter().find(|e| e.id != original.id).unwrap();
    assert!(old.missing);
    assert!(!new.missing);
    assert_ne!(old.id, new.id);

    let offline = dir.path().join("offline-root");
    fs::rename(&games, &offline).unwrap();
    let snap = core.snapshot().unwrap();
    assert!(matches!(
        snap.roots[0].status,
        crate::model::RootStatus::Offline
    ));
    assert_eq!(snap.editions.len(), 2);
}

#[test]
fn unresolved_serial_is_kept_and_not_merged_by_title() {
    let dir = tempfile::tempdir().unwrap();
    let games = dir.path().join("games");
    fs::create_dir_all(&games).unwrap();
    File::create(games.join("Same Title.iso"))
        .unwrap()
        .write_all(&[0u8; 100])
        .unwrap();
    write_iso(
        &games.join("nested").join("Same Title.iso"),
        "BOOT2 = cdrom0:\\SLPM_650.01;1",
    );
    let core = Core::open(Some(dir.path().join("data")), true).unwrap();
    let root = core.add_root(games, None).unwrap();
    let report = core.scan(Some(&root.id), None).unwrap();
    assert!(report.unresolved_serial >= 1);
    let editions = core.snapshot().unwrap().editions;
    assert_eq!(editions.len(), 2);
    assert!(editions.iter().any(|e| e.serial.is_none()));
    assert!(editions
        .iter()
        .any(|e| e.serial.as_deref() == Some("SLPM_650.01")));
}

#[test]
fn launch_argv_keeps_metacharacters_inert() {
    let runner = crate::model::Runner {
        id: "runner.pcsx2.appimage".into(),
        kind: RunnerKind::Pcsx2,
        display_name: "PCSX2".into(),
        command: vec!["/opt/PCSX2-v2.AppImage".into()],
        bios_state: BiosState::Unverified,
        bios_hint: None,
        source: "appimage".into(),
        available: true,
    };
    let iso = PathBuf::from("/games/weird/My Game's (USA) --iso.iso");
    let argv = runners::launch_argv(&runner, &iso, &[]).unwrap();
    assert_eq!(
        argv,
        vec![
            "/opt/PCSX2-v2.AppImage",
            "-batch",
            "-fullscreen",
            "--",
            "/games/weird/My Game's (USA) --iso.iso"
        ]
    );
    let dash = PathBuf::from("/games/-leading.iso");
    let argv = runners::launch_argv(&runner, &dash, &[]).unwrap();
    assert_eq!(argv.last().unwrap(), "/games/-leading.iso");

    let flatpak = crate::model::Runner {
        command: vec!["flatpak".into(), "run".into(), "net.pcsx2.PCSX2".into()],
        source: "flatpak".into(),
        ..runner
    };
    let argv =
        runners::launch_argv(&flatpak, Path::new("/media/quote\"and$meta.iso"), &[]).unwrap();
    assert_eq!(argv[0], "flatpak");
    assert_eq!(argv[1], "run");
    assert_eq!(argv[2], "net.pcsx2.PCSX2");
    assert_eq!(argv.last().unwrap(), "/media/quote\"and$meta.iso");
}

#[test]
fn empty_default_bios_dir_is_unverified_not_missing() {
    assert_eq!(
        runners::require_launchable(&crate::model::Runner {
            id: "r".into(),
            kind: RunnerKind::Pcsx2,
            display_name: "PCSX2".into(),
            command: vec!["/bin/true".into()],
            bios_state: BiosState::Unverified,
            bios_hint: None,
            source: "appimage".into(),
            available: true,
        })
        .unwrap(),
        ()
    );
    let err = runners::require_launchable(&crate::model::Runner {
        id: "r".into(),
        kind: RunnerKind::Pcsx2,
        display_name: "PCSX2".into(),
        command: vec!["/bin/true".into()],
        bios_state: BiosState::Missing,
        bios_hint: None,
        source: "appimage".into(),
        available: true,
    })
    .unwrap_err();
    assert_eq!(err.code, "SETUP_MISSING");
}

#[test]
fn fake_runner_session_records_duration_once_and_rejects_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let games = dir.path().join("games");
    write_iso(&games.join("Play.iso"), "BOOT2 = cdrom0:\\SLUS_209.63;1");
    let core = Core::open_isolated(Some(dir.path().join("data")), true).unwrap();
    core.select_runner("runner.fake").unwrap();
    let root = core.add_root(games, None).unwrap();
    core.scan(Some(&root.id), None).unwrap();
    let edition = core.snapshot().unwrap().editions.remove(0);
    let first = core.launch(&edition.id, &[], Arc::new(NoopSink)).unwrap();
    let busy = core
        .launch(&edition.id, &[], Arc::new(NoopSink))
        .unwrap_err();
    assert_eq!(busy.code, "LAUNCH_BUSY");
    let ended = wait_session(&core, &first.id);
    assert_eq!(ended.state, SessionState::Ended);
    assert!(ended.duration_seconds.unwrap() >= 1);
    let edition = core.snapshot().unwrap().editions.remove(0);
    assert!(edition.playtime_seconds >= 1);
    assert_eq!(edition.play_count, 1);
    let again = core.launch(&edition.id, &[], Arc::new(NoopSink)).unwrap();
    wait_session(&core, &again.id);
    let edition = core.snapshot().unwrap().editions.remove(0);
    assert_eq!(edition.play_count, 2);
}

#[test]
fn spawn_failed_and_missing_runner_are_typed() {
    let dir = tempfile::tempdir().unwrap();
    let games = dir.path().join("games");
    write_iso(&games.join("Play.iso"), "BOOT2 = cdrom0:\\SLUS_209.63;1");
    let core = Core::open_isolated(Some(dir.path().join("data")), false).unwrap();
    let root = core.add_root(games, None).unwrap();
    core.scan(Some(&root.id), None).unwrap();
    let edition_id = core.snapshot().unwrap().editions[0].id.clone();
    let err = core
        .launch(&edition_id, &[], Arc::new(NoopSink))
        .unwrap_err();
    assert_eq!(err.code, "RUNNER_MISSING");

    let core = Core::open_isolated(Some(dir.path().join("data2")), true).unwrap();
    let root = core.add_root(dir.path().join("games"), None).unwrap();
    core.scan(Some(&root.id), None).unwrap();
    let edition_id = core.snapshot().unwrap().editions[0].id.clone();
    let bad = dir.path().join("not-executable");
    File::create(&bad).unwrap().write_all(b"nope").unwrap();
    let mut perm = fs::metadata(&bad).unwrap().permissions();
    perm.set_mode(0o644);
    fs::set_permissions(&bad, perm).unwrap();
    {
        let db = core.db_arc();
        let db = db.lock().unwrap();
        db.upsert_runner(
            "runner.bad",
            "fake",
            "Broken",
            &[bad.to_string_lossy().into_owned()],
            BiosState::LaunchTested,
            None,
            "test",
        )
        .unwrap();
    }
    core.select_runner("runner.bad").unwrap();
    let err = core
        .launch(&edition_id, &[], Arc::new(NoopSink))
        .unwrap_err();
    assert_eq!(err.code, "SPAWN_FAILED");
}

#[test]
fn reopen_abandons_stale_running_session_without_playtime() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let games = dir.path().join("games");
    write_iso(&games.join("Shadow.iso"), "BOOT2 = cdrom0:\\SLUS_209.63;1");
    let edition_id = {
        let core = Core::open_isolated(Some(data.clone()), true).unwrap();
        let root = core.add_root(games, Some("PS2".into())).unwrap();
        core.scan(Some(&root.id), None).unwrap();
        let edition = core.snapshot().unwrap().editions.remove(0);
        {
            let db = core.db_arc();
            let db = db.lock().unwrap();
            db.insert_session(
                "stale-session",
                &edition.id,
                "runner.fake",
                "2026-01-01T00:00:00Z",
                "running",
            )
            .unwrap();
        }
        edition.id
    };
    let core = Core::open_isolated(Some(data), true).unwrap();
    core.select_runner("runner.fake").unwrap();
    let snap = core.snapshot().unwrap();
    let stale = snap
        .sessions
        .iter()
        .find(|s| s.id == "stale-session")
        .unwrap();
    assert_eq!(stale.state, SessionState::Failed);
    assert!(stale.error.as_deref().unwrap().contains("exited while"));
    let edition = snap.editions.iter().find(|e| e.id == edition_id).unwrap();
    assert_eq!(edition.playtime_seconds, 0);
    let session = core.launch(&edition_id, &[], Arc::new(NoopSink)).unwrap();
    let ended = wait_session(&core, &session.id);
    assert_eq!(ended.state, SessionState::Ended);
}

#[test]
fn schema_rejects_a_newer_library() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    Core::open(Some(data.clone()), true).unwrap();
    {
        let conn = rusqlite::Connection::open(data.join("library.db")).unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
    }
    let err = match Core::open(Some(data), true) {
        Ok(_) => panic!("newer schema should fail"),
        Err(err) => err,
    };
    assert_eq!(err.code, "DB");
    assert!(err.message.contains("newer"));
}

#[test]
fn instance_lock_rejects_a_second_owner() {
    let dir = tempfile::tempdir().unwrap();
    let _core = Core::open(Some(dir.path().to_path_buf()), true).unwrap();
    let err = match Core::open(Some(dir.path().to_path_buf()), true) {
        Ok(_) => panic!("second owner should fail"),
        Err(err) => err,
    };
    assert_eq!(err.code, "INSTANCE_BUSY");
}

#[test]
fn add_root_missing_folder_is_root_offline() {
    let dir = tempfile::tempdir().unwrap();
    let core = Core::open(Some(dir.path().join("data")), true).unwrap();
    let err = core.add_root(dir.path().join("nope"), None).unwrap_err();
    assert_eq!(err.code, "ROOT_OFFLINE");
}

#[test]
fn error_envelope_serializes_code() {
    let err = AppError::new("MEDIA_MISSING", "gone");
    let json = serde_json::to_value(&err).unwrap();
    assert_eq!(json["code"], "MEDIA_MISSING");
}

#[test]
fn unpacked_tree_is_a_managed_runner_without_network() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let bin = data.join("runners/pcsx2/current/usr/bin/pcsx2-qt");
    fs::create_dir_all(bin.parent().unwrap()).unwrap();
    File::create(&bin)
        .unwrap()
        .write_all(b"#!/bin/sh\nexit 0\n")
        .unwrap();
    let mut perm = fs::metadata(&bin).unwrap().permissions();
    perm.set_mode(0o755);
    fs::set_permissions(&bin, perm).unwrap();
    let core = Core::open_isolated(Some(data), true).unwrap();
    let runners = core.install_pcsx2("auto", Arc::new(NoopSink)).unwrap();
    assert!(
        runners
            .iter()
            .any(|r| r.id == "runner.pcsx2.unpacked" && r.available),
        "{runners:?}"
    );
}

#[test]
fn unknown_pcsx2_channel_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let core = Core::open(Some(dir.path().join("data")), true).unwrap();
    let err = core.install_pcsx2("steam", Arc::new(NoopSink)).unwrap_err();
    assert_eq!(err.code, "RUNNER_MISSING");
}

#[test]
fn artwork_refresh_on_empty_library_fetches_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let core = Core::open(Some(dir.path().join("data")), true).unwrap();
    let report = core.refresh_artwork(None, None).unwrap();
    assert_eq!(report.fetched, 0);
    assert_eq!(report.failed, 0);
}

#[test]
fn artwork_refresh_skips_unresolved_serial_without_network() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let games = dir.path().join("games");
    write_iso(&games.join("unknown.iso"), "VER = 1.00");
    let core = Core::open(Some(data), true).unwrap();
    let root = core.add_root(games, Some("PS2".into())).unwrap();
    core.scan(Some(&root.id), None).unwrap();
    let report = core.refresh_artwork(None, None).unwrap();
    assert_eq!(report.fetched, 0);
    assert_eq!(report.failed, 0);
    assert!(report.skipped >= 1);
    assert!(core.snapshot().unwrap().editions[0].cover_path.is_none());
}
