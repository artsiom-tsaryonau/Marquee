#![cfg(unix)]

use marquee_core::streaming::wolf::WolfClient;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

fn fixtures() -> Value {
    serde_json::from_str(include_str!("fixtures/wolf.json")).unwrap()
}

struct Exchange {
    method: &'static str,
    path: &'static str,
    body: Value,
    reply: Vec<u8>,
}

fn exchange(method: &'static str, path: &'static str, body: Value, response: Value) -> Exchange {
    Exchange {
        method,
        path,
        body,
        reply: reply(200, &response.to_string()),
    }
}

fn reply(status: u16, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.0 {status} OK\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

fn mock(exchanges: Vec<Exchange>) -> (tempfile::TempDir, PathBuf, thread::JoinHandle<()>) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("wolf.sock");
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let task = thread::spawn(move || {
        for expected in exchanges {
            let end = Instant::now() + Duration::from_secs(5);
            let stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < end =>
                    {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(e) => panic!("Expected {} {}: {e}", expected.method, expected.path),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert_eq!(
                line,
                format!("{} {} HTTP/1.0\r\n", expected.method, expected.path)
            );
            let mut length = None;
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some(s) = line.strip_prefix("Content-Length: ") {
                    length = Some(s.trim().parse::<usize>().unwrap());
                }
            }
            let mut body = vec![0; length.unwrap()];
            reader.read_exact(&mut body).unwrap();
            let actual = if body.is_empty() {
                Value::Null
            } else {
                serde_json::from_slice(&body).unwrap()
            };
            assert_eq!(actual, expected.body);
            // Fragmented, length-delimited replies; keep open until the client
            // closes. Waiting for EOF to finish a response would time out.
            for fragment in expected.reply.chunks(11) {
                if reader.get_mut().write_all(fragment).is_err() {
                    break;
                }
            }
            let _ = reader.read(&mut [0]);
        }
    });
    (temp, path, task)
}

fn inspection() -> Vec<Exchange> {
    let f = fixtures();
    vec![
        exchange("GET", "/api/v1/apps", Value::Null, f["apps"].clone()),
        exchange("GET", "/api/v1/clients", Value::Null, f["clients"].clone()),
        exchange(
            "GET",
            "/api/v1/sessions",
            Value::Null,
            f["sessions"].clone(),
        ),
    ]
}

#[test]
fn inspection_projects_the_source_contract_without_keys_commands_or_pair_secrets() {
    let (_temp, path, server) = mock(inspection());
    let snapshot = WolfClient::new(path).unwrap().inspect().unwrap();
    assert_eq!(snapshot.sessions[0].session_id, "42");
    assert_eq!(snapshot.clients[0].client_id, "71");
    assert_eq!(snapshot.sessions[0].video_width, 1280);
    let output = serde_json::to_string(&snapshot).unwrap();
    assert!(!output.contains("DO_NOT_PRINT"));
    assert!(!format!("{snapshot:?}").contains("DO_NOT_PRINT"));
    assert!(!output.contains("aes_"));
    assert_eq!(
        serde_json::to_value(snapshot).unwrap()["sessions"][0]["session_id"],
        "42"
    );
    server.join().unwrap();
}

#[test]
fn pairing_uses_the_unique_pending_secret_internally_and_requires_no_argv_pin() {
    let f = fixtures();
    let (_temp, path, server) = mock(vec![
        exchange(
            "GET",
            "/api/v1/pair/pending",
            Value::Null,
            f["pending"].clone(),
        ),
        exchange(
            "GET",
            "/api/v1/pair/pending",
            Value::Null,
            f["pending"].clone(),
        ),
        exchange(
            "POST",
            "/api/v1/pair/client",
            json!({"pair_secret": "DO_NOT_PRINT_PAIR_SECRET", "pin": "0123"}),
            json!({"success": true}),
        ),
        exchange(
            "POST",
            "/api/v1/sessions/stop",
            json!({"session_id": "42"}),
            json!({"success": true}),
        ),
        exchange(
            "POST",
            "/api/v1/unpair/client",
            json!({"client_id": "71"}),
            json!({"success": true}),
        ),
    ]);
    let client = WolfClient::new(path).unwrap();
    let pending = client.pending().unwrap();
    assert!(!serde_json::to_string(&pending)
        .unwrap()
        .contains("DO_NOT_PRINT"));
    assert!(!format!("{pending:?}").contains("DO_NOT_PRINT"));
    client.pair("127.0.0.1", "0123").unwrap();
    client.stop_session("42").unwrap();
    client.unpair("71").unwrap();
    server.join().unwrap();
}

#[test]
fn no_implicit_pairing_for_an_ambiguous_or_absent_client() {
    let request = fixtures()["pending"]["requests"][0].clone();
    for requests in [vec![], vec![request.clone(), request]] {
        let (_temp, path, server) = mock(vec![exchange(
            "GET",
            "/api/v1/pair/pending",
            Value::Null,
            json!({"success": true, "requests": requests}),
        )]);
        assert_eq!(
            WolfClient::new(path)
                .unwrap()
                .pair("127.0.0.1", "0123")
                .unwrap_err()
                .code,
            "STREAMING_PAIR_SELECTION"
        );
        server.join().unwrap();
    }
}

#[test]
fn malformed_framing_and_error_bodies_cannot_leak_secrets_or_redirect_to_tcp() {
    let cases = [
        (reply(404, ""), "STREAMING_API_UNSUPPORTED"),
        (reply(500, "DO_NOT_PRINT_AES_KEY"), "STREAMING_REJECTED"),
        (reply(302, "DO_NOT_PRINT_PAIR_SECRET"), "STREAMING_REJECTED"),
        (
            reply(200, r#"{"success":false,"error":"DO_NOT_PRINT_AES_KEY"}"#),
            "STREAMING_REJECTED",
        ),
        (
            reply(200, r#"{"success":true,"apps":"DO_NOT_PRINT_AES_KEY"}"#),
            "STREAMING_PROTOCOL",
        ),
        (
            b"HTTP/1.0 200 OK\r\nContent-Length: 2097153\r\n\r\n".to_vec(),
            "STREAMING_PROTOCOL",
        ),
        (
            b"HTTP/1.0 200 OK\r\nContent-Length: 0\r\nContent-Length: 8\r\n\r\n".to_vec(),
            "STREAMING_PROTOCOL",
        ),
        (
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec(),
            "STREAMING_PROTOCOL",
        ),
        (
            b"HTTP/1.0 200 OK\r\nContent-Length: not-a-number\r\n\r\n".to_vec(),
            "STREAMING_PROTOCOL",
        ),
    ];
    for (response, code) in cases {
        let (_temp, path, server) = mock(vec![Exchange {
            method: "GET",
            path: "/api/v1/apps",
            body: Value::Null,
            reply: response,
        }]);
        let error = WolfClient::new(path).unwrap().inspect().unwrap_err();
        assert_eq!(error.code, code);
        assert!(!serde_json::to_string(&error)
            .unwrap()
            .contains("DO_NOT_PRINT"));
        server.join().unwrap();
    }
}

#[test]
fn invalid_inputs_fail_without_touching_a_backend() {
    assert_eq!(
        WolfClient::new("relative.sock").unwrap_err().code,
        "STREAMING_SOCKET"
    );
    let client = WolfClient::new("/does-not-exist/wolf.sock").unwrap();
    for pin in ["123", "12345", "12a4", "１２３４"] {
        assert_eq!(
            client.pair("127.0.0.1", pin).unwrap_err().code,
            "STREAMING_PIN"
        );
    }
    for id in ["", "1\r\nInjected: yes", "18446744073709551616", "-1"] {
        assert_eq!(client.stop_session(id).unwrap_err().code, "STREAMING_ID");
        assert_eq!(client.unpair(id).unwrap_err().code, "STREAMING_ID");
    }
    assert_eq!(client.inspect().unwrap_err().code, "STREAMING_UNAVAILABLE");
}

#[test]
fn retained_upstream_schema_agrees_with_supported_routes_and_fixture_field_names() {
    let schema: Value =
        serde_json::from_str(include_str!("../../../third_party/wolf/openapi.json")).unwrap();
    for (method, route) in [
        ("get", "/api/v1/apps"),
        ("get", "/api/v1/clients"),
        ("get", "/api/v1/sessions"),
        ("get", "/api/v1/pair/pending"),
        ("post", "/api/v1/pair/client"),
        ("post", "/api/v1/unpair/client"),
        ("post", "/api/v1/sessions/stop"),
    ] {
        assert!(schema["paths"][route][method].is_object(), "{route}");
    }
    for (name, fixture) in [
        (
            "rfl__Reflector_wolf__core__events__App___ReflType",
            fixtures()["apps"]["apps"][0].clone(),
        ),
        (
            "rfl__Reflector_wolf__core__events__StreamSession___ReflType",
            fixtures()["sessions"]["sessions"][0].clone(),
        ),
        (
            "wolf__api__PendingPairClient",
            fixtures()["pending"]["requests"][0].clone(),
        ),
    ] {
        let properties = schema["components"]["schemas"][name]["properties"]
            .as_object()
            .unwrap();
        for field in fixture.as_object().unwrap().keys() {
            assert!(properties.contains_key(field), "{name}.{field}");
        }
    }
}
