use std::fs;
use std::process::Command;

#[test]
fn torrent_inspect_is_offline_and_does_not_open_a_library() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("synthetic.torrent");
    fs::write(
        &source,
        b"d4:infod6:lengthi1e4:name5:a.bin12:piece lengthi16384e6:pieces20:abcdefghijklmnopqrstee",
    )
    .unwrap();
    let data = temp.path().join("must-not-be-created");
    let output = Command::new(env!("CARGO_BIN_EXE_marquee"))
        .arg("--data-dir")
        .arg(&data)
        .arg("torrent")
        .arg("inspect")
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    let review: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(review["total"], 1);
    assert_eq!(review["files"][0]["path"], "a.bin");
    assert_eq!(review["review_token"].as_str().unwrap().len(), 64);
    assert!(
        !data.exists(),
        "inspect must not restore transfers, acquire the DB or create a data directory"
    );
}
