use std::fs;
use std::process::Command;

const VALID_RAR: &[u8] = include_bytes!("../../core/tests/fixtures/rar/regular5.rar");

#[test]
fn failed_archive_returns_failure_but_later_imports_and_the_scan_complete() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let bad = temp.path().join("a-bad.rar");
    let good = temp.path().join("z-good.rar");
    fs::write(&bad, b"Rar!\x1a\x07\0 malformed").unwrap();
    fs::write(&good, VALID_RAR).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_marquee"))
        .arg("--data-dir")
        .arg(&data)
        .arg("intake")
        .arg(&bad)
        .arg(&good)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("INTAKE_FORMAT"), "{stdout}");
    assert!(stdout.contains("Imported (ds)"), "{stdout}");
    assert!(stdout.contains("\"scanned\": 1"), "{stdout}");
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("INTAKE_FAILED: 1 of 2"));
    assert_eq!(
        fs::read(data.join("roms/ds/Game.nds")).unwrap(),
        b"synthetic cartridge bytes\n"
    );
    assert!(bad.exists() && good.exists());
}

#[test]
fn successful_and_already_present_batches_return_success() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("game.rar");
    fs::write(&source, VALID_RAR).unwrap();
    for expected in ["Imported", "AlreadyPresent"] {
        let output = Command::new(env!("CARGO_BIN_EXE_marquee"))
            .arg("--data-dir")
            .arg(temp.path().join("data"))
            .arg("intake")
            .arg(&source)
            .arg("--no-scan")
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains(expected), "{stdout}");
        assert!(!stdout.contains("\"scanned\""));
    }
}
