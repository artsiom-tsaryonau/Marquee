#![cfg(unix)]

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixListener;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[test]
fn streaming_cli_works_with_an_open_library_or_no_data_directory_and_redacts_pairing() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("library");
    let _core = marquee_core::Core::open(Some(data.clone()), false).unwrap();
    let untouched = temp.path().join("must-not-be-created");
    let socket = temp.path().join("wolf.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let fixture: Value =
        serde_json::from_str(include_str!("../../core/tests/fixtures/wolf.json")).unwrap();
    let server = thread::spawn(move || {
        for route in [
            "apps",
            "clients",
            "sessions",
            "apps",
            "clients",
            "sessions",
            "pair/pending",
            "pair/client",
        ] {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let method = if route == "pair/client" {
                "POST"
            } else {
                "GET"
            };
            assert_eq!(line, format!("{method} /api/v1/{route} HTTP/1.0\r\n"));
            let mut length = 0;
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.strip_prefix("Content-Length: ") {
                    length = value.trim().parse::<usize>().unwrap();
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let response = match route {
                "pair/pending" => fixture["pending"].clone(),
                "pair/client" => {
                    assert_eq!(
                        serde_json::from_slice::<Value>(&body).unwrap(),
                        json!({"pair_secret": "DO_NOT_PRINT_PAIR_SECRET", "pin": "0123"})
                    );
                    json!({"success": true})
                }
                _ => fixture[route].clone(),
            }
            .to_string();
            write!(
                reader.get_mut(),
                "HTTP/1.0 200 OK\r\nContent-Length: {}\r\n\r\n{response}",
                response.len()
            )
            .unwrap();
        }
    });
    for directory in [&data, &untouched] {
        let output = Command::new(env!("CARGO_BIN_EXE_marquee"))
            .arg("--data-dir")
            .arg(directory)
            .args(["streaming", "--socket"])
            .arg(&socket)
            .arg("inspect")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(!text.contains("DO_NOT_PRINT"));
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["sessions"][0]["session_id"], "42");
    }
    let mut child = Command::new(env!("CARGO_BIN_EXE_marquee"))
        .arg("--data-dir")
        .arg(&untouched)
        .args(["streaming", "--socket"])
        .arg(&socket)
        .args(["pair", "--client-ip", "127.0.0.1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"0123\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains("0123") && !text.contains("DO_NOT_PRINT"));
    assert_eq!(
        serde_json::from_str::<Value>(&text).unwrap()["accepted"],
        true
    );
    assert!(!untouched.exists());
    server.join().unwrap();
}
