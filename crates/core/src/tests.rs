use crate::error::AppError;
use crate::iso::build_test_iso;
use crate::model::{BiosState, MediaKind, RunnerKind, SessionState};
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
        let root = core
            .add_root("ps2", games.clone(), Some("PS2".into()))
            .unwrap();
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
    let root = core.add_root("ps2", games, None).unwrap();
    let report = core.scan(Some(&root.id), None).unwrap();
    assert_eq!(report.added, 1);
    assert_eq!(report.skipped_unsupported, 5);
    let kinds: Vec<_> = report
        .skipped_by_kind
        .iter()
        .map(|s| (s.kind.as_str(), s.count))
        .collect();
    assert!(kinds.contains(&("chd", 1)));
    assert!(kinds.contains(&("cue", 1)));
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
    let root = core.add_root("ps2", games, None).unwrap();
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
    let root = core.add_root("ps2", games.clone(), None).unwrap();
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
    let root = core.add_root("ps2", games, None).unwrap();
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
            "-nogui",
            "--",
            "/games/weird/My Game's (USA) --iso.iso"
        ]
    );
    let dash = PathBuf::from("/games/-leading.iso");
    let argv = runners::launch_argv(&runner, &dash, &[]).unwrap();
    assert_eq!(argv.last().unwrap(), "/games/-leading.iso");
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

/// A PS1 root imports `.iso`, launches via DuckStation's argv shape, and keeps
/// CUE imports once while BIN companions are skipped.
#[test]
fn ps1_root_imports_iso_and_uses_duckstation_argv() {
    let dir = tempfile::tempdir().unwrap();
    let games = dir.path().join("ps1");
    write_iso(&games.join("Cave.iso"), "BOOT = cdrom:\\SLUS_005.94;1");
    write_iso(&games.join("Twin.chd"), "BOOT = cdrom:\\SLES_011.11;1");
    // A real cue/bin pair, plus an orphaned track that must not double-count.
    write_iso(&games.join("Pair.bin"), "BOOT = cdrom:\\SLUS_007.76;1");
    File::create(games.join("Pair.cue"))
        .unwrap()
        .write_all(b"FILE \"Pair.bin\" BINARY\n  TRACK 01 MODE1/2048\n    INDEX 01 00:00:00\n")
        .unwrap();
    File::create(games.join("Orphan.bin"))
        .unwrap()
        .write_all(b"no sheet")
        .unwrap();
    let core = Core::open_isolated(Some(dir.path().join("data")), true).unwrap();
    let root = core.add_root("ps1", games, None).unwrap();
    assert_eq!(root.system, "ps1");
    let report = core.scan(Some(&root.id), None).unwrap();
    // `.iso`, `.chd` and the `.cue` are supported; both `.bin` files are skipped
    // (the paired one is part of the cue, the orphan has no sheet).
    assert_eq!(report.added, 3);
    assert_eq!(report.skipped_unsupported, 2);
    let snap = core.snapshot().unwrap();
    assert!(snap.editions.iter().all(|e| e.system == "ps1"));
    let cave = snap
        .editions
        .iter()
        .find(|e| e.scan_title == "Cave")
        .unwrap();
    assert_eq!(cave.serial.as_deref(), Some("SLUS_005.94"));
    assert_eq!(cave.region.as_deref(), Some("NTSC-U"));

    // The `.cue` entry resolves its serial from the sibling track named in the
    // sheet, and is the thing that gets launched (not the `.bin`).
    let pair = snap
        .editions
        .iter()
        .find(|e| e.scan_title == "Pair")
        .unwrap();
    assert_eq!(pair.serial.as_deref(), Some("SLUS_007.76"));
    assert_eq!(pair.region.as_deref(), Some("NTSC-U"));
    let pair_media = snap.media.iter().find(|m| m.edition_id == pair.id).unwrap();
    assert_eq!(pair_media.kind, MediaKind::Ps1Cue);
    assert_eq!(pair_media.path.extension().unwrap(), "cue");

    let duck = crate::model::Runner {
        id: "runner.duckstation.marquee".into(),
        kind: RunnerKind::Duckstation,
        display_name: "DuckStation".into(),
        command: vec!["/data/runners/duckstation/current/AppRun".into()],
        bios_state: BiosState::Unverified,
        bios_hint: None,
        source: "marquee".into(),
        available: true,
    };
    let img = PathBuf::from("/games/ps1/Cave.iso");
    let argv = runners::launch_argv(&duck, &img, &[]).unwrap();
    assert_eq!(
        argv,
        vec![
            "/data/runners/duckstation/current/AppRun",
            "-batch",
            "-fullscreen",
            "-nogui",
            "/games/ps1/Cave.iso"
        ]
    );
    // DuckStation does not take PCSX2's `--` separator.
    assert!(!argv.iter().any(|a| a == "--"));
}

/// A PS1 folder must not silently adopt a PS2 runner, and vice versa: the
/// system on the edition picks the emulator.
#[test]
fn pick_for_system_keeps_emulators_apart() {
    let ps2 = crate::model::Runner {
        id: "runner.pcsx2.appimage".into(),
        kind: RunnerKind::Pcsx2,
        display_name: "PCSX2".into(),
        command: vec!["/bin/true".into()],
        bios_state: BiosState::Unverified,
        bios_hint: None,
        source: "appimage".into(),
        available: true,
    };
    let duck = crate::model::Runner {
        id: "runner.duckstation.flatpak".into(),
        kind: RunnerKind::Duckstation,
        display_name: "DuckStation".into(),
        command: vec!["/bin/true".into()],
        bios_state: BiosState::Unverified,
        bios_hint: None,
        source: "flatpak".into(),
        available: true,
    };
    let all = vec![ps2.clone(), duck.clone()];
    assert_eq!(runners::pick_for_system(&all, "ps1").unwrap().id, duck.id);
    assert_eq!(runners::pick_for_system(&all, "ps2").unwrap().id, ps2.id);
}

/// A stored runner selection must only apply to its own emulator family, so a
/// PS2 pin cannot hijack a PS1 launch.
#[test]
fn stored_selection_is_scoped_to_the_systems_emulator() {
    let ps2 = crate::model::Runner {
        id: "runner.pcsx2.flatpak".into(),
        kind: RunnerKind::Pcsx2,
        display_name: "PCSX2 (Flatpak)".into(),
        command: vec!["/bin/true".into()],
        bios_state: BiosState::Unverified,
        bios_hint: None,
        source: "flatpak".into(),
        available: true,
    };
    let duck = crate::model::Runner {
        id: "runner.duckstation.flatpak".into(),
        kind: RunnerKind::Duckstation,
        display_name: "DuckStation (Flatpak)".into(),
        command: vec!["/bin/true".into()],
        bios_state: BiosState::Unverified,
        bios_hint: None,
        source: "flatpak".into(),
        available: true,
    };
    let all = vec![ps2.clone(), duck.clone()];
    // A PS2 pin is ignored for a PS1 title, and honoured for a PS2 one.
    assert_eq!(
        runners::selection_for(&all, Some("runner.pcsx2.flatpak"), "ps1"),
        None
    );
    assert_eq!(
        runners::selection_for(&all, Some("runner.pcsx2.flatpak"), "ps2"),
        Some("runner.pcsx2.flatpak")
    );
    assert_eq!(
        runners::selection_for(&all, Some("runner.duckstation.flatpak"), "ps1"),
        Some("runner.duckstation.flatpak")
    );
    assert_eq!(runners::selection_for(&all, None, "ps1"), None);

    // The fake stand-in is system-agnostic, so explicit test selections survive.
    let fake = crate::model::Runner {
        id: "runner.fake".into(),
        kind: RunnerKind::Fake,
        display_name: "Test runner".into(),
        command: vec!["/bin/sleep".into(), "1".into()],
        bios_state: BiosState::LaunchTested,
        bios_hint: None,
        source: "test".into(),
        available: true,
    };
    let with_fake = vec![ps2, duck, fake];
    assert_eq!(
        runners::selection_for(&with_fake, Some("runner.fake"), "ps1"),
        Some("runner.fake")
    );
}

#[test]
fn unknown_system_root_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let err = Core::open_isolated(Some(dir.path().join("data")), true)
        .unwrap()
        .add_root("dreamcast", dir.path().join("dc"), None)
        .unwrap_err();
    assert_eq!(err.code, "ROOT_OFFLINE");
}

#[test]
fn fake_runner_session_records_duration_once_and_rejects_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let games = dir.path().join("games");
    write_iso(&games.join("Play.iso"), "BOOT2 = cdrom0:\\SLUS_209.63;1");
    let core = Core::open_isolated(Some(dir.path().join("data")), true).unwrap();
    core.select_runner("runner.fake").unwrap();
    let root = core.add_root("ps2", games, None).unwrap();
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
    let root = core.add_root("ps2", games, None).unwrap();
    core.scan(Some(&root.id), None).unwrap();
    let edition_id = core.snapshot().unwrap().editions[0].id.clone();
    let err = core
        .launch(&edition_id, &[], Arc::new(NoopSink))
        .unwrap_err();
    assert_eq!(err.code, "RUNNER_MISSING");

    let core = Core::open_isolated(Some(dir.path().join("data2")), true).unwrap();
    let root = core
        .add_root("ps2", dir.path().join("games"), None)
        .unwrap();
    core.scan(Some(&root.id), None).unwrap();
    let edition_id = core.snapshot().unwrap().editions[0].id.clone();
    let bad = dir.path().join("not-executable");
    File::create(&bad).unwrap().write_all(b"nope").unwrap();
    let mut perm = fs::metadata(&bad).unwrap().permissions();
    // Availability checks reject mode 0644 before spawn. Use an executable
    // with an invalid binary format to exercise the actual spawn failure.
    perm.set_mode(0o755);
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
        let root = core.add_root("ps2", games, Some("PS2".into())).unwrap();
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
    let err = core
        .add_root("ps2", dir.path().join("nope"), None)
        .unwrap_err();
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
            .any(|r| r.id == "runner.pcsx2.marquee" && r.available),
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
    let root = core.add_root("ps2", games, Some("PS2".into())).unwrap();
    core.scan(Some(&root.id), None).unwrap();
    let report = core.refresh_artwork(None, None).unwrap();
    assert_eq!(report.fetched, 0);
    assert_eq!(report.failed, 0);
    assert!(report.skipped >= 1);
    assert!(core.snapshot().unwrap().editions[0].cover_path.is_none());
}

#[test]
fn a_bad_disc_does_not_abort_the_rest_of_the_scan() {
    let t = tempfile::tempdir().unwrap();
    let games = t.path().join("games");
    write_iso(&games.join("good.iso"), "BOOT2 = cdrom0:\\SCUS_971.24;1");
    write_iso(&games.join("bad.iso"), "BOOT2 = cdrom0:\\SCUS_971.24;1");
    let bad = fs::OpenOptions::new()
        .write(true)
        .open(games.join("bad.iso"))
        .unwrap();
    bad.set_len(17 * 2048).unwrap();
    let core = Core::open_isolated(Some(t.path().join("data")), true).unwrap();
    let root = core.add_root("ps2", games, None).unwrap();
    let report = core.scan(Some(&root.id), None).unwrap();
    assert_eq!(report.added, 1);
    assert_eq!(report.warnings.len(), 1);
    assert_eq!(core.snapshot().unwrap().editions.len(), 1);
}

#[test]
fn selected_runner_changes_after_first_launch_and_mutations_emit_events() {
    let t = tempfile::tempdir().unwrap();
    let games = t.path().join("games");
    write_iso(&games.join("game.iso"), "BOOT2 = cdrom0:\\SCUS_971.24;1");
    let core = Core::open_isolated(Some(t.path().join("data")), true).unwrap();
    let root = core.add_root("ps2", games, None).unwrap();
    core.scan(Some(&root.id), None).unwrap();
    let edition = core.snapshot().unwrap().editions[0].id.clone();
    let db = core.db_arc();
    db.lock()
        .unwrap()
        .upsert_runner(
            "second",
            "fake",
            "Second",
            &["/bin/true".into()],
            BiosState::LaunchTested,
            None,
            "test",
        )
        .unwrap();
    core.select_runner("runner.fake").unwrap();
    let first = core.launch(&edition, &[], Arc::new(NoopSink)).unwrap();
    wait_session(&core, &first.id);
    core.select_runner("second").unwrap();
    let second = core.launch(&edition, &[], Arc::new(NoopSink)).unwrap();
    assert_eq!(second.runner_id, "second");
    wait_session(&core, &second.id);
    let seq = core.hub().seq();
    core.set_favourite(&edition, true).unwrap();
    core.set_display_title(&edition, "My title").unwrap();
    assert!(core.hub().seq() >= seq + 2);
    db.lock()
        .unwrap()
        .complete_session(&second.id, 999, Some(0), "ended", None, true)
        .unwrap();
    assert_eq!(core.snapshot().unwrap().editions[0].play_count, 2);
}

#[test]
fn schema_v2_upgrade_clears_implicit_profiles_and_preserves_library_data() {
    let t = tempfile::tempdir().unwrap();
    let data = t.path().join("data");
    let games = t.path().join("games");
    write_iso(&games.join("game.iso"), "BOOT2 = cdrom0:\\SCUS_971.24;1");
    {
        let core = Core::open_isolated(Some(data.clone()), true).unwrap();
        let root = core.add_root("ps2", games, None).unwrap();
        core.scan(Some(&root.id), None).unwrap();
        let edition = core.snapshot().unwrap().editions[0].id.clone();
        core.set_display_title(&edition, "Personal title").unwrap();
        core.set_favourite(&edition, true).unwrap();
        let db = core.db_arc();
        let db = db.lock().unwrap();
        db.set_profile(&edition, "runner.fake").unwrap();
        db.conn
            .execute_batch(
                "ALTER TABLE sessions DROP COLUMN process_identity; PRAGMA user_version=2;",
            )
            .unwrap();
    }
    let core = Core::open_isolated(Some(data), true).unwrap();
    let edition = core.snapshot().unwrap().editions.remove(0);
    assert_eq!(edition.title, "Personal title");
    assert!(edition.favorite);
    assert!(core
        .db_arc()
        .lock()
        .unwrap()
        .profile_runner(&edition.id)
        .unwrap()
        .is_none());
}

#[test]
fn launcher_commands_share_persistence_validation_and_invalidation() {
    let t = tempfile::tempdir().unwrap();
    let data = t.path().join("data");
    let core = Arc::new(Core::open_isolated(Some(data.clone()), true).unwrap());
    let config = serde_json::json!({"id":"custom.test", "name":"My app", "executable":"/bin/true", "args":[], "working_directory":null, "pinned":true});
    let seq = core.hub().seq();
    let saved = crate::api::dispatch(
        &core,
        "save_launcher",
        serde_json::json!({"launcher":config}),
    )
    .unwrap();
    assert_eq!(saved[0]["name"], "My app");
    assert_eq!(core.hub().seq(), seq + 1);
    assert_eq!(
        crate::api::dispatch(&core, "open_launcher", serde_json::json!({"id":"unknown"}))
            .unwrap_err()
            .code,
        "LAUNCHER_MISSING"
    );
    assert_eq!(
        crate::api::dispatch(
            &core,
            "save_launcher",
            serde_json::json!({"launcher":{"id": "bad"}})
        )
        .unwrap_err()
        .code,
        "BAD_REQUEST"
    );
    drop(core);
    let core = Arc::new(Core::open_isolated(Some(data), true).unwrap());
    assert_eq!(
        crate::api::dispatch(&core, "list_launchers", serde_json::json!({})).unwrap()[0]["id"],
        "custom.test"
    );
    crate::api::dispatch(
        &core,
        "remove_launcher",
        serde_json::json!({"id":"custom.test"}),
    )
    .unwrap();
    assert!(core.list_launchers().unwrap().is_empty());
}

#[test]
fn cue_tracks_restore_replace_and_invalidate_art_without_losing_titles() {
    let t = tempfile::tempdir().unwrap();
    let games = t.path().join("games");
    fs::create_dir_all(games.join("tracks")).unwrap();
    let cue = games.join("game.cue");
    let bin = games.join("tracks/data.bin");
    let audio = games.join("audio.bin");
    fs::write(&cue, "FILE \"tracks/data.bin\" BINARY\nTRACK 01 MODE1/2048\nINDEX 01 00:00:00\nFILE \"audio.bin\" BINARY\nTRACK 02 AUDIO\nINDEX 01 00:00:00\n").unwrap();
    write_iso(&bin, "BOOT = cdrom:\\SLUS_007.07;1");
    let core = Core::open_isolated(Some(t.path().join("data")), true).unwrap();
    let root = core.add_root("ps1", games, None).unwrap();
    let report = core.scan(Some(&root.id), None).unwrap();
    assert_eq!(report.added, 1);
    assert!(!report.warnings.is_empty());
    let before = core.snapshot().unwrap();
    let id = before.editions[0].id.clone();
    assert!(before.editions[0].missing);
    assert!(before.media[0]
        .skip_reason
        .as_ref()
        .unwrap()
        .contains("audio.bin"));
    assert_eq!(
        core.launch(&id, &[], Arc::new(NoopSink)).unwrap_err().code,
        "MEDIA_MISSING"
    );
    fs::write(&audio, [0; 2352]).unwrap();
    assert!(!core.snapshot().unwrap().editions[0].missing);
    core.scan(Some(&root.id), None).unwrap();
    assert_eq!(
        core.snapshot().unwrap().editions[0].serial.as_deref(),
        Some("SLUS_007.07")
    );
    core.set_display_title(&id, "My disc").unwrap();
    core.set_favourite(&id, true).unwrap();
    core.db_arc()
        .lock()
        .unwrap()
        .upsert_artwork(&id, "cover", "/tmp/old-cover.jpg", "test", "2026-01-01")
        .unwrap();
    let cue_before = fs::metadata(&cue).unwrap().modified().unwrap();
    write_iso(&bin, "BOOT = cdrom:\\SLUS_999.99;1");
    core.scan(Some(&root.id), None).unwrap();
    let after = core.snapshot().unwrap();
    let edition = &after.editions[0];
    assert_eq!(edition.id, id);
    assert_eq!(edition.title, "My disc");
    assert!(edition.favorite);
    assert_eq!(edition.serial.as_deref(), Some("SLUS_999.99"));
    assert!(edition.cover_path.is_none());
    assert_eq!(fs::metadata(&cue).unwrap().modified().unwrap(), cue_before);
    fs::remove_file(&bin).unwrap();
    assert!(core.snapshot().unwrap().editions[0].missing);
    assert_eq!(
        core.launch(&id, &[], Arc::new(NoopSink)).unwrap_err().code,
        "MEDIA_MISSING"
    );
}

#[test]
fn conflicting_root_system_is_explicit_and_keeps_editions() {
    let t = tempfile::tempdir().unwrap();
    let games = t.path().join("games");
    write_iso(&games.join("game.iso"), "BOOT = cdrom:\\SLUS_005.94;1");
    let core = Core::open_isolated(Some(t.path().join("data")), true).unwrap();
    let root = core.add_root("ps1", games.clone(), None).unwrap();
    core.scan(Some(&root.id), None).unwrap();
    let id = core.snapshot().unwrap().editions[0].id.clone();
    assert_eq!(
        core.add_root("ps2", games.clone(), None).unwrap_err().code,
        "ROOT_SYSTEM_CONFLICT"
    );
    assert_eq!(core.add_root("ps1", games, None).unwrap().id, root.id);
    assert_eq!(core.snapshot().unwrap().editions[0].id, id);
}

#[test]
fn ps1_and_ps2_defaults_persist_independently() {
    let t = tempfile::tempdir().unwrap();
    let data = t.path().join("data");
    let core = Core::open_isolated(Some(data.clone()), false).unwrap();
    {
        let db = core.db_arc();
        let db = db.lock().unwrap();
        db.upsert_runner(
            "test.ps1",
            "duckstation",
            "PS1",
            &["/bin/true".into()],
            BiosState::Unverified,
            None,
            "test",
        )
        .unwrap();
        db.upsert_runner(
            "test.ps2",
            "pcsx2",
            "PS2",
            &["/bin/true".into()],
            BiosState::Unverified,
            None,
            "test",
        )
        .unwrap();
    }
    core.select_runner("test.ps2").unwrap();
    core.select_runner("test.ps1").unwrap();
    drop(core);
    let core = Core::open_isolated(Some(data), false).unwrap();
    let settings = core.snapshot().unwrap().settings;
    assert_eq!(settings.selected_runner_ids["ps1"], "test.ps1");
    assert_eq!(settings.selected_runner_ids["ps2"], "test.ps2");
}

#[test]
fn failed_process_keeps_attempt_duration_without_crediting_play() {
    let t = tempfile::tempdir().unwrap();
    let games = t.path().join("games");
    write_iso(&games.join("game.iso"), "BOOT2 = cdrom0:\\SLUS_209.63;1");
    let core = Core::open_isolated(Some(t.path().join("data")), true).unwrap();
    let root = core.add_root("ps2", games, None).unwrap();
    core.scan(Some(&root.id), None).unwrap();
    let id = core.snapshot().unwrap().editions[0].id.clone();
    core.db_arc()
        .lock()
        .unwrap()
        .upsert_runner(
            "test.failure",
            "fake",
            "Failure",
            &["/bin/sh".into(), "-c".into(), "sleep 1; exit 1".into()],
            BiosState::Unverified,
            None,
            "test",
        )
        .unwrap();
    core.select_runner("test.failure").unwrap();
    let session = core.launch(&id, &[], Arc::new(NoopSink)).unwrap();
    let ended = wait_session(&core, &session.id);
    assert_eq!(ended.state, SessionState::Failed);
    assert!(ended.duration_seconds.unwrap() >= 1);
    let edition = core.snapshot().unwrap().editions.remove(0);
    assert_eq!(edition.play_count, 0);
    assert_eq!(edition.playtime_seconds, 0);
    assert!(edition.last_played_at.is_none());
}

#[test]
fn cartridge_systems_scan_pick_their_emulator_and_build_argv() {
    let dir = tempfile::tempdir().unwrap();
    let roms = dir.path().join("nes");
    std::fs::create_dir_all(&roms).unwrap();
    std::fs::write(roms.join("Frog Quest (PD).nes"), b"NES\x1a").unwrap();
    std::fs::write(roms.join("notes.txt"), b"x").unwrap();
    let core = Core::open_isolated(Some(dir.path().join("data")), false).unwrap();
    core.add_root("nes", roms.clone(), None).unwrap();
    let report = core.scan(None, None).unwrap();
    assert_eq!(report.added, 1);
    let snap = core.snapshot().unwrap();
    assert_eq!(snap.editions[0].system, "nes");
    assert_eq!(snap.editions[0].title, "Frog Quest (PD)");
    assert_eq!(snap.media[0].kind, MediaKind::Rom);
    assert!(snap.media[0].serial.is_none());

    let runner = |id: &str, source: &str| crate::model::Runner {
        id: id.into(),
        kind: RunnerKind::Emulator,
        display_name: id.into(),
        command: vec![
            "/usr/bin/env".into(),
            "/data/runners/mesen/current/Mesen".into(),
        ],
        bios_state: BiosState::Unverified,
        bios_hint: None,
        source: source.into(),
        available: true,
    };
    let mesen = runner("runner.mesen.marquee", "marquee");
    let other = runner("runner.flycast.marquee", "marquee");
    let runners = vec![other, mesen.clone()];
    assert_eq!(
        runners::pick_for_system(&runners, "nes").unwrap().id,
        "runner.mesen.marquee"
    );
    assert_eq!(
        runners::pick_for_system(&runners, "dreamcast").unwrap().id,
        "runner.flycast.marquee"
    );
    assert_eq!(
        runners::pick_for_system(&runners, "n64").map(|r| r.id.as_str()),
        None
    );
    let rom = PathBuf::from("/roms/My Game's (USA).nes");
    let argv = runners::launch_argv(&mesen, &rom, &["--x".into()]).unwrap();
    assert_eq!(
        argv,
        vec![
            "/usr/bin/env",
            "/data/runners/mesen/current/Mesen",
            "--x",
            "--fullscreen",
            "/roms/My Game's (USA).nes"
        ]
    );
}
