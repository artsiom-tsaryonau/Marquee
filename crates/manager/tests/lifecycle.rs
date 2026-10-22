//! Actual processes and private sockets: never an in-process worker fallback.
use marquee_manager::{
    protocol::{self, Operation, Reply, Request},
    Client, State,
};
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Fixture {
    temp: tempfile::TempDir,
    client: Client,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let data = temp.path().join("data");
        fs::create_dir_all(&data).unwrap();
        Self {
            client: Client::new(&data),
            temp,
        }
    }
    fn source(&self, bytes: &[u8]) -> std::path::PathBuf {
        let source = self.temp.path().join("game.zip");
        let mut zip = zip::ZipWriter::new(File::create(&source).unwrap());
        zip.start_file("Game.gen", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
        zip.finish().unwrap();
        source
    }
    fn wait(&self, condition: impl Fn() -> bool) {
        let until = Instant::now() + Duration::from_secs(10);
        while !condition() {
            assert!(Instant::now() < until, "manager lifecycle timed out");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.client.stop();
        let until = Instant::now() + Duration::from_secs(10);
        while self.client.status().is_ok_and(|s| s.running) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

#[test]
fn single_owner_private_socket_protocol_rejection_and_clean_stop() {
    let fixture = Fixture::new();
    assert!(!fixture.client.status().unwrap().running);
    let first = fixture.client.start_manager().unwrap();
    assert!(first.running);
    assert_ne!(first.pid, Some(std::process::id()));
    let second = Command::new(env!("CARGO_BIN_EXE_marquee-manager"))
        .arg("--data-dir")
        .arg(fixture.client.data_dir())
        .output()
        .unwrap();
    assert!(!second.status.success());
    assert!(String::from_utf8_lossy(&second.stderr).contains("MANAGER_BUSY"));
    assert_eq!(fixture.client.status().unwrap().pid, first.pid);
    let socket = fixture.client.socket_path().unwrap();
    assert_eq!(
        fs::metadata(&socket).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(socket.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let mut stream = UnixStream::connect(&socket).unwrap();
    protocol::write_frame(
        &mut stream,
        &Request {
            protocol_version: 999,
            request_id: "old-client".into(),
            operation: Operation::Stop,
        },
    )
    .unwrap();
    let reply: Reply = protocol::read_frame(&mut stream).unwrap();
    assert_eq!(reply.result.unwrap_err().code, "MANAGER_VERSION");
    assert!(fixture.client.status().unwrap().running);
    assert!(!fixture.client.data_dir().join("manager/stopped").exists());
    // v1 clients would drop provenance fields before receipt publication.
    let mut legacy = UnixStream::connect(&socket).unwrap();
    protocol::write_frame(
        &mut legacy,
        &Request {
            protocol_version: 1,
            request_id: "legacy-client".into(),
            operation: Operation::Stop,
        },
    )
    .unwrap();
    let reply: Reply = protocol::read_frame(&mut legacy).unwrap();
    assert_eq!(reply.result.unwrap_err().code, "MANAGER_VERSION");
    assert!(!fixture.client.data_dir().join("manager/stopped").exists());
    let mut stream = UnixStream::connect(&socket).unwrap();
    stream
        .write_all(&((protocol::MAX_FRAME + 1) as u32).to_le_bytes())
        .unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    assert!(protocol::read_frame::<Reply>(&mut stream).is_err());
    drop(stream);
    assert!(fixture.client.stop().unwrap().stopping);
    fixture.wait(|| !fixture.client.status().unwrap().running);
    assert!(!socket.exists());
    // Stop is persistent: list/reconnect cannot silently undo the user's stop.
    assert!(fixture.client.data_dir().join("manager/stopped").exists());
}

#[test]
fn concurrent_on_demand_start_has_one_pid_and_isolated_libraries_have_distinct_owners() {
    let fixture = Fixture::new();
    let client = Arc::new(fixture.client.clone());
    let threads: Vec<_> = (0..6)
        .map(|_| {
            let client = client.clone();
            std::thread::spawn(move || client.start_manager().unwrap().pid)
        })
        .collect();
    let pids: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert!(pids.iter().all(|pid| *pid == pids[0]));
    let another = Fixture::new();
    assert_ne!(another.client.start_manager().unwrap().pid, pids[0]);
    assert_ne!(
        another.client.socket_path().unwrap(),
        fixture.client.socket_path().unwrap()
    );
}

#[test]
fn accepted_work_survives_client_exit_and_receipt_waits_without_a_library_database() {
    let fixture = Fixture::new();
    let source = fixture.source(&vec![b'g'; 16 * 1024 * 1024]);
    let client = fixture.client.clone();
    let review = client.inspect(&source, "genesis").unwrap();
    let pid = client.status().unwrap().pid;
    assert!(!fixture.client.data_dir().join("library.db").exists());
    let started = client.start_job(&review.id, &review.review_token).unwrap();
    assert_eq!(started.state, State::Preparing);
    drop(client);
    fixture.wait(|| fixture.client.get(&review.id).unwrap().state != State::Preparing);
    let prepared = fixture.client.get(&review.id).unwrap();
    assert_eq!(
        prepared.state,
        State::AwaitingLibrary,
        "{:?}",
        prepared.error
    );
    assert_eq!(fixture.client.status().unwrap().pid, pid);
    assert!(!fixture.client.data_dir().join("library.db").exists());
    assert_eq!(
        fs::read(prepared.destination.join("Game.gen"))
            .unwrap()
            .len(),
        16 * 1024 * 1024
    );
    assert!(source.is_file());
    fixture.client.stop().unwrap();
    fixture.wait(|| !fixture.client.status().unwrap().running);
    assert!(
        fixture.client.list().is_err(),
        "read-only polling cannot restart an explicitly stopped manager"
    );
    fixture.client.start_manager().unwrap();
    let pending = fixture.client.get(&review.id).unwrap();
    assert_eq!(pending.state, State::AwaitingLibrary);
    assert_eq!(pending.attempt, 1);
    let hash = pending.receipt.as_ref().unwrap().fingerprint().unwrap();
    let ack = fixture
        .client
        .acknowledge(&pending.id, &hash, vec!["library-edition".into()])
        .unwrap();
    assert_eq!(ack.state, State::Registered);
    assert_eq!(
        fixture
            .client
            .start_job(&ack.id, &ack.review_token)
            .unwrap()
            .attempt,
        1
    );
}

#[test]
fn stale_socket_after_abrupt_death_is_replaced_without_losing_reviews() {
    let fixture = Fixture::new();
    let source = fixture.source(b"synthetic cartridge");
    let review = fixture.client.inspect(&source, "genesis").unwrap();
    let pid = fixture.client.status().unwrap().pid.unwrap();
    // SAFETY: this PID is the manager spawned by this test, running as this user.
    assert_eq!(unsafe { libc::kill(pid as i32, libc::SIGKILL) }, 0);
    fixture.wait(|| !fixture.client.status().unwrap().running);
    assert!(fixture.client.socket_path().unwrap().exists());
    let restarted = fixture.client.start_manager().unwrap();
    assert_ne!(restarted.pid, Some(pid));
    let retained = fixture.client.get(&review.id).unwrap();
    assert_eq!(retained.id, review.id);
    assert_eq!(retained.state, State::AwaitingReview);
    assert!(source.is_file());
}

#[test]
fn insecure_manager_storage_is_rejected_without_opening_a_library() {
    let fixture = Fixture::new();
    // A directory mode conflict is deterministic without changing global env.
    let path = fixture.client.data_dir().join("manager");
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        fixture.client.start_manager().unwrap_err().code,
        "MANAGER_PRIVATE_PATH"
    );
    assert!(!Path::new(fixture.client.data_dir())
        .join("library.db")
        .exists());
}

#[test]
fn m1_cutover_requires_exclusive_ownership_and_preserves_a_private_backup() {
    use std::os::fd::AsRawFd;
    let fixture = Fixture::new();
    let source = fixture.source(b"synthetic cartridge");
    let manager = marquee_manager::service::Manager::new(fixture.client.data_dir());
    let mut review = manager.inspect(&source, "genesis").unwrap();
    review.state = State::Preparing;
    let db_path = fixture.client.data_dir().join("manager/jobs.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute(
        "UPDATE jobs SET payload=?1",
        [serde_json::to_string(&review).unwrap()],
    )
    .unwrap();
    conn.pragma_update(None, "user_version", 1).unwrap();
    drop(conn);
    drop(manager);
    let library_lock = File::options()
        .write(true)
        .create(true)
        .truncate(false)
        .open(fixture.client.data_dir().join("instance.lock"))
        .unwrap();
    assert_eq!(
        unsafe { libc::flock(library_lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    assert_eq!(
        fixture.client.start_manager().unwrap_err().code,
        "MANAGER_MIGRATION_BUSY"
    );
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    assert_eq!(
        conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    drop(conn);
    drop(library_lock);
    fixture.client.start_manager().unwrap();
    let retained = fixture.client.get(&review.id).unwrap();
    assert_eq!(retained.state, State::Interrupted);
    assert_eq!(retained.review_token, review.review_token);
    let backups: Vec<_> = fs::read_dir(fixture.client.data_dir().join("manager"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("jobs-m1-backup-")
        })
        .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(
        fs::metadata(&backups[0]).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let backup = rusqlite::Connection::open(&backups[0]).unwrap();
    assert_eq!(
        backup
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    let current = rusqlite::Connection::open(&db_path).unwrap();
    assert_eq!(
        current
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        3
    );
    assert!(source.is_file());
}

#[test]
fn explicit_stop_drains_accepted_work_and_never_auto_restarts_it() {
    let fixture = Fixture::new();
    let source = fixture.source(&vec![b'g'; 8 * 1024 * 1024]);
    let review = fixture.client.inspect(&source, "genesis").unwrap();
    fixture
        .client
        .start_job(&review.id, &review.review_token)
        .unwrap();
    assert!(fixture.client.stop().unwrap().stopping);
    fixture.wait(|| !fixture.client.status().unwrap().running);
    let conn = rusqlite::Connection::open_with_flags(
        fixture.client.data_dir().join("manager/jobs.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let payload: String = conn
        .query_row("SELECT payload FROM jobs WHERE id=?1", [&review.id], |r| {
            r.get(0)
        })
        .unwrap();
    let prepared: marquee_manager::Job = serde_json::from_str(&payload).unwrap();
    assert_eq!(prepared.state, State::AwaitingLibrary);
    assert_eq!(prepared.attempt, 1);
    assert!(fixture.client.list().is_err());
    assert!(!fixture.client.status().unwrap().running);
    assert!(source.exists());
}

#[test]
fn adopted_torrent_survives_client_exit_pause_and_manager_restart() {
    use marquee_manager::torrent::{Request as Torrent, TorrentSettings, TransferStatus};
    let fixture = Fixture::new();
    let data = fixture.client.data_dir();
    let source_dir = fixture.temp.path().join("seed");
    fs::create_dir(&source_dir).unwrap();
    let payload: Vec<u8> = (0..8 * 1024 * 1024u32)
        .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
        .collect();
    let source = source_dir.join("synthetic.pkg");
    fs::write(&source, &payload).unwrap();
    let mut cfg = marquee_torrent::Config::private(&source_dir);
    cfg.listen = "127.0.0.1:0".parse().unwrap();
    let seed = marquee_torrent::Engine::start(&cfg).unwrap();
    let made = seed.create(&source, vec![]).unwrap();
    let id = seed.seed(made.bytes.clone(), &source_dir).unwrap();
    seed.wait_finished(id, Duration::from_secs(5)).unwrap();
    let hash = made.info_hash.clone();
    let destination = fixture.temp.path().join("original-downloads");
    let directory = destination.join(&hash);
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        destination.join(format!(".marquee-transfer-{hash}")),
        hash.as_bytes(),
    )
    .unwrap();
    fs::create_dir_all(data.join("torrents")).unwrap();
    fs::write(data.join(format!("torrents/{hash}.torrent")), made.bytes).unwrap();
    let legacy = serde_json::json!([{"kind":"get","info_hash":hash,"name":"synthetic.pkg","edition_id":null,"system":null,"dir":directory,"destination":destination,"peers":[seed.listen_addr().unwrap()],"paused":true,"owned":true,"downloaded":0,"uploaded":0,"total":payload.len(),"public":true}]);
    fs::write(
        data.join("torrents/entries.json"),
        serde_json::to_vec_pretty(&legacy).unwrap(),
    )
    .unwrap();
    let db = rusqlite::Connection::open(data.join("library.db")).unwrap();
    db.execute_batch("CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT); INSERT INTO settings VALUES('torrent.enabled','1'),('torrent.seeding','0'),('torrent.download_kbps','1024');").unwrap();
    drop(db);
    let library_before = fs::read(data.join("library.db")).unwrap();
    let status: TransferStatus = fixture.client.torrent(Torrent::Status).unwrap();
    assert_eq!(status.items[0].state, "paused");
    assert_eq!(status.items[0].directory, directory);
    fixture
        .client
        .torrent::<()>(Torrent::Pause {
            hash: hash.clone(),
            paused: false,
        })
        .unwrap();
    fixture.wait(|| {
        let s: TransferStatus = fixture.client.torrent(Torrent::Status).unwrap();
        s.items[0].downloaded > 0 && s.items[0].downloaded < s.items[0].total
    });
    fixture
        .client
        .torrent::<()>(Torrent::Pause {
            hash: hash.clone(),
            paused: true,
        })
        .unwrap();
    let paused: TransferStatus = fixture.client.torrent(Torrent::Status).unwrap();
    fixture.client.stop().unwrap();
    fixture.wait(|| !fixture.client.status().unwrap().running);
    // A passive status request must not undo an explicit manager stop.
    assert_eq!(
        fixture
            .client
            .torrent::<TransferStatus>(Torrent::Status)
            .unwrap_err()
            .code,
        "MANAGER_OFFLINE"
    );
    assert!(!fixture.client.status().unwrap().running);
    fixture.client.start_manager().unwrap();
    let reopened: TransferStatus = fixture.client.torrent(Torrent::Status).unwrap();
    assert_eq!(reopened.items[0].state, "paused");
    assert!(reopened.items[0].downloaded >= paused.items[0].downloaded);
    let mut settings: TorrentSettings = fixture.client.torrent(Torrent::Settings).unwrap();
    settings.download_kbps = 0;
    fixture
        .client
        .torrent::<()>(Torrent::SetSettings { settings })
        .unwrap();
    let transient = fixture.client.clone();
    transient
        .torrent::<()>(Torrent::Pause {
            hash: hash.clone(),
            paused: false,
        })
        .unwrap();
    drop(transient);
    // No IPC/status polling: the detached manager heartbeat saves completion.
    fixture.wait(|| {
        let rows: serde_json::Value =
            serde_json::from_slice(&fs::read(data.join("manager/torrents/entries.json")).unwrap())
                .unwrap();
        rows[0]["complete"] == true
    });
    assert_eq!(fs::read(directory.join("synthetic.pkg")).unwrap(), payload);
    assert_eq!(
        fs::read(data.join("library.db")).unwrap(),
        library_before,
        "transport must never write library.db or install packages"
    );
    fixture.client.stop().unwrap();
    fixture.wait(|| !fixture.client.status().unwrap().running);
    fs::remove_file(directory.join("synthetic.pkg")).unwrap();
    fixture.client.start_manager().unwrap();
    let missing: TransferStatus = fixture.client.torrent(Torrent::Status).unwrap();
    assert_eq!(missing.items[0].state, "source_missing");
    std::thread::sleep(Duration::from_millis(100));
    assert!(!directory.join("synthetic.pkg").exists());
    seed.shutdown();
}

#[test]
fn background_seed_metadata_and_listener_policy_survive_restart_without_touching_source() {
    use marquee_manager::torrent::{Created, Request as Torrent, TorrentSettings, TransferStatus};
    let fixture = Fixture::new();
    let source = fixture.temp.path().join("source.bin");
    fs::write(&source, b"synthetic source to retain").unwrap();
    fixture
        .client
        .torrent::<()>(Torrent::SetSettings {
            settings: TorrentSettings {
                enabled: true,
                ..Default::default()
            },
        })
        .unwrap();
    let created: Created = fixture
        .client
        .torrent(Torrent::Create {
            path: source.clone(),
            listen: Some("127.0.0.1:0".parse().unwrap()),
            seed: true,
        })
        .unwrap();
    assert_eq!(
        marquee_manager::torrent::inspect_file(&created.path)
            .unwrap()
            .0
            .info_hash,
        created.info_hash
    );
    fixture.wait(|| {
        let state: TransferStatus = fixture.client.torrent(Torrent::Status).unwrap();
        state.items[0].link.is_some()
    });
    fixture.client.stop().unwrap();
    fixture.wait(|| !fixture.client.status().unwrap().running);
    assert_eq!(fs::read(&source).unwrap(), b"synthetic source to retain");
    assert_eq!(
        fs::read(
            fixture
                .client
                .data_dir()
                .join("manager/torrents/private-listen.json")
        )
        .unwrap(),
        b"\"127.0.0.1:0\""
    );
    fixture.client.start_manager().unwrap();
    fixture.wait(|| {
        let state: TransferStatus = fixture.client.torrent(Torrent::Status).unwrap();
        state.items[0].link.is_some()
    });
    fixture
        .client
        .torrent::<()>(Torrent::Stop {
            hash: created.info_hash,
        })
        .unwrap();
    assert_eq!(fs::read(source).unwrap(), b"synthetic source to retain");
}
