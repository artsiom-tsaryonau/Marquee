use std::fs;
use std::process::Command;

#[test]
fn ps3_inspect_bypasses_library_resolution_and_keeps_unverified_sources() {
    let t = tempfile::tempdir().unwrap();
    let source = t.path().join("alleged.dec.iso");
    let content = b"Not a verified PS3 image";
    fs::write(&source, content).unwrap();
    let blocker = t.path().join("not-a-directory");
    fs::write(&blocker, "block library creation").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_marquee"))
        .arg("--data-dir")
        .arg(blocker.join("child"))
        .arg("ps3-inspect")
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["source_kind"], "file");
    assert_eq!(report["playable"], false);
    assert_eq!(report["items"][0]["kind"], "disc_image");
    assert!(report["items"][0]["metadata"].is_null());
    assert_eq!(fs::read(source).unwrap(), content);
    assert_eq!(fs::read_dir(t.path()).unwrap().count(), 2);
}
