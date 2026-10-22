use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn cli(data: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_marquee"))
        .arg("--data-dir")
        .arg(data)
        .args(args)
        .output()
        .unwrap()
}

fn json(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

struct WorkerGuard(std::path::PathBuf);
impl Drop for WorkerGuard {
    fn drop(&mut self) {
        let client = marquee_manager::Client::new(&self.0);
        let _ = client.stop();
        let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while client.status().is_ok_and(|s| s.running) && std::time::Instant::now() < end {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}

#[test]
fn reviewed_rar_preparation_is_durable_and_legacy_library_survives_migration() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let _worker = WorkerGuard(data.clone());
    let source = temp.path().join("game.rar");
    fs::write(
        &source,
        include_bytes!("../../core/tests/fixtures/rar/regular5.rar"),
    )
    .unwrap();
    // An existing v5 library migrates additively, without a reset.
    let core = marquee_core::Core::open(Some(data.clone()), true).unwrap();
    let wanted = core
        .add_wanted("ds", "Keep my favourite", "wishlist")
        .unwrap();
    core.set_favourite(&wanted, true).unwrap();
    drop(core);
    let paths = marquee_core::paths::Paths::resolve(Some(data.clone())).unwrap();
    let db = marquee_core::db::Db::open(&paths).unwrap();
    db.conn
        .execute_batch("DROP TABLE preparation_receipts; PRAGMA user_version=5;")
        .unwrap();
    drop(db);
    let review = json(cli(
        &data,
        &[
            "prepare",
            "inspect",
            source.to_str().unwrap(),
            "--system",
            "ds",
        ],
    ));
    assert_eq!(review["state"], "awaiting_review");
    let destination = review["destination"].as_str().unwrap();
    assert!(!Path::new(destination).exists());
    let id = review["id"].as_str().unwrap();
    let token = review["review_token"].as_str().unwrap();
    let rejected = cli(&data, &["prepare", "run", id, "--review-token", "stale"]);
    assert!(!rejected.status.success());
    let done = json(cli(&data, &["prepare", "run", id, "--review-token", token]));
    assert_eq!(done["state"], "registered");
    assert!(source.is_file());
    let history = json(cli(&data, &["prepare", "list"]));
    assert_eq!(history[0]["id"], review["id"]);
    let entries = json(cli(&data, &["list", "--json"]));
    assert_eq!(entries.as_array().unwrap().len(), 2);
    assert!(entries
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["id"] == wanted && e["favorite"] == true));
    let again = json(cli(&data, &["prepare", "run", id, "--review-token", token]));
    assert_eq!(again["edition_ids"], done["edition_ids"]);
}

#[test]
fn detached_cli_handoff_never_opens_library_and_reconnect_registers_once() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let _worker = WorkerGuard(data.clone());
    let source = temp.path().join("game.rar");
    fs::write(
        &source,
        include_bytes!("../../core/tests/fixtures/rar/regular5.rar"),
    )
    .unwrap();
    let review = json(cli(
        &data,
        &[
            "prepare",
            "inspect",
            source.to_str().unwrap(),
            "--system",
            "ds",
        ],
    ));
    assert!(!data.join("library.db").exists());
    let id = review["id"].as_str().unwrap();
    let token = review["review_token"].as_str().unwrap();
    let started = json(cli(
        &data,
        &["prepare", "start", id, "--review-token", token],
    ));
    assert!(matches!(
        started["state"].as_str(),
        Some("preparing" | "awaiting_library")
    ));
    let client = marquee_manager::Client::new(&data);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while client.get(id).unwrap().state == marquee_manager::State::Preparing {
        assert!(std::time::Instant::now() < until);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        !data.join("library.db").exists(),
        "daemon cannot create/write the library"
    );
    fs::remove_file(source).unwrap();
    let history = json(cli(&data, &["prepare", "list"]));
    assert_eq!(history[0]["state"], "registered");
    assert_eq!(history[0]["attempt"], 1);
    let ids = history[0]["edition_ids"].clone();
    assert_eq!(
        json(cli(&data, &["prepare", "list"]))[0]["edition_ids"],
        ids
    );
}

#[test]
fn missing_companion_does_not_prevent_existing_library_commands() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let output = Command::new(env!("CARGO_BIN_EXE_marquee"))
        .arg("--data-dir")
        .arg(&data)
        .args(["manager", "start"])
        .env("MARQUEE_MANAGER_BIN", "/nonexistent/marquee-manager")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("MANAGER_BINARY"));
    assert!(!data.join("library.db").exists());
    let output = Command::new(env!("CARGO_BIN_EXE_marquee"))
        .arg("--data-dir")
        .arg(&data)
        .arg("health")
        .env("MARQUEE_MANAGER_BIN", "/nonexistent/marquee-manager")
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
}
