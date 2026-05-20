//! CLI tests for `cowork doctor`.

use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use assert_cmd::Command;
use serde_json::{Value, json};

#[test]
fn doctor_exits_zero_when_all_checks_pass() {
    let dirs = test_dirs();
    let (host, handle) = spawn_server(ok_response(&response_envelope(r#"{"ok":true}"#)));

    let assert = Command::cargo_bin("cowork")
        .expect("binary should build")
        .current_dir(&dirs.project)
        .env("HOME", &dirs.home)
        .args(["doctor", "--model", "gemma3:12b", "--host", &host])
        .assert()
        .success();
    let output = assert.get_output().stdout.clone();
    let value = parse_json_output(&output);
    let request = handle.join().expect("server should join");
    let request_text = String::from_utf8(request).expect("request should be utf-8");

    assert!(request_text.starts_with("POST /api/generate HTTP/1.1\r\n"));
    assert_eq!(
        value,
        json!({
            "schema_version": "1.0",
            "command": "doctor",
            "status": "ok",
            "checks": [
                {
                    "name": "config_files_loaded",
                    "status": "ok",
                    "message": "Loaded 0 config files, using built-in defaults."
                },
                {
                    "name": "effective_model_chosen",
                    "status": "ok",
                    "message": "Using model `gemma3:12b`."
                },
                {
                    "name": "host_url_parsed",
                    "status": "ok",
                    "message": format!("Host URL parsed: `{host}`.")
                },
                {
                    "name": "generate_endpoint_reachable",
                    "status": "ok",
                    "message": "Reached `/api/generate`."
                },
                {
                    "name": "tiny_json_probe_succeeds",
                    "status": "ok",
                    "message": "Probe request returned JSON text."
                },
                {
                    "name": "probe_output_shape_valid",
                    "status": "ok",
                    "message": "Probe JSON matched `{ \"ok\": true }`."
                }
            ]
        })
    );
}

#[test]
fn doctor_missing_model_returns_exit_code_seven() {
    let dirs = test_dirs();

    let assert = Command::cargo_bin("cowork")
        .expect("binary should build")
        .current_dir(&dirs.project)
        .env("HOME", &dirs.home)
        .args(["doctor", "--host", "http://localhost:11434"])
        .assert()
        .code(7);
    let output = assert.get_output().stdout.clone();
    let value = parse_json_output(&output);

    assert_eq!(value["status"], "error");
    assert_eq!(value["checks"][1]["name"], "effective_model_chosen");
    assert_eq!(value["checks"][1]["status"], "error");
    assert_eq!(value["checks"][2]["status"], "skipped");
    assert_eq!(value["checks"][5]["status"], "skipped");
}

#[test]
fn doctor_bad_host_returns_exit_code_six() {
    let dirs = test_dirs();

    let assert = Command::cargo_bin("cowork")
        .expect("binary should build")
        .current_dir(&dirs.project)
        .env("HOME", &dirs.home)
        .args(["doctor", "--model", "gemma3:12b", "--host", "://bad"])
        .assert()
        .code(6);
    let output = assert.get_output().stdout.clone();
    let value = parse_json_output(&output);

    assert_eq!(value["status"], "error");
    assert_eq!(value["checks"][2]["name"], "host_url_parsed");
    assert_eq!(value["checks"][2]["status"], "error");
    assert_eq!(value["checks"][3]["status"], "skipped");
}

#[test]
fn doctor_unreachable_host_returns_exit_code_eight() {
    let dirs = test_dirs();
    let host = unused_host();

    let assert = Command::cargo_bin("cowork")
        .expect("binary should build")
        .current_dir(&dirs.project)
        .env("HOME", &dirs.home)
        .args(["doctor", "--model", "gemma3:12b", "--host", &host])
        .assert()
        .code(8);
    let output = assert.get_output().stdout.clone();
    let value = parse_json_output(&output);

    assert_eq!(value["status"], "error");
    assert_eq!(value["checks"][3]["name"], "generate_endpoint_reachable");
    assert_eq!(value["checks"][3]["status"], "error");
    assert_eq!(value["checks"][4]["status"], "skipped");
}

#[test]
fn doctor_bad_probe_json_returns_exit_code_ten() {
    let dirs = test_dirs();
    let (host, handle) = spawn_server(ok_response(&response_envelope(r#"{"ready":true}"#)));

    let assert = Command::cargo_bin("cowork")
        .expect("binary should build")
        .current_dir(&dirs.project)
        .env("HOME", &dirs.home)
        .args(["doctor", "--model", "gemma3:12b", "--host", &host])
        .assert()
        .code(10);
    let output = assert.get_output().stdout.clone();
    let value = parse_json_output(&output);

    handle.join().expect("server should join");
    assert_eq!(value["status"], "error");
    assert_eq!(value["checks"][4]["status"], "ok");
    assert_eq!(value["checks"][5]["name"], "probe_output_shape_valid");
    assert_eq!(value["checks"][5]["status"], "error");
}

fn parse_json_output(stdout: &[u8]) -> Value {
    serde_json::from_slice(stdout).expect("stdout should be json")
}

struct TestDirs {
    project: PathBuf,
    home: PathBuf,
}

fn test_dirs() -> TestDirs {
    let root = std::env::temp_dir().join(format!(
        "cowork-doctor-cli-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos()
    ));
    let project = root.join("project");
    let home = root.join("home");

    fs::create_dir_all(&project).expect("project dir should create");
    fs::create_dir_all(&home).expect("home dir should create");

    TestDirs { project, home }
}

fn spawn_server(response: String) -> (String, thread::JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let address = listener.local_addr().expect("listener should have addr");

    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("server should accept");
        let request = read_request(&mut stream);

        stream
            .write_all(response.as_bytes())
            .expect("response should write");
        stream.flush().expect("response should flush");

        request
    });

    (format!("http://{address}"), handle)
}

fn read_request(stream: &mut TcpStream) -> Vec<u8> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("timeout should set");

    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    let mut body_start = None;
    let mut body_len = 0_usize;

    loop {
        let read = stream.read(&mut buffer).expect("request should read");
        if read == 0 {
            break;
        }

        request.extend_from_slice(&buffer[..read]);

        if body_start.is_none()
            && let Some(index) = find_bytes(&request, b"\r\n\r\n")
        {
            body_start = Some(index + 4);
            body_len = parse_content_length(&request[..index + 4]);
        }

        if let Some(body_start) = body_start
            && request.len() >= body_start + body_len
        {
            break;
        }
    }

    request
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn parse_content_length(headers: &[u8]) -> usize {
    let headers = String::from_utf8(headers.to_vec()).expect("headers should be utf-8");

    headers
        .lines()
        .find_map(|line| {
            let value = line.strip_prefix("content-length: ")?;
            value.parse::<usize>().ok()
        })
        .unwrap_or(0)
}

fn ok_response(body: &str) -> String {
    http_response(200, "OK", body)
}

fn http_response(status: u16, status_text: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} {status_text}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn response_envelope(response: &str) -> String {
    json!({ "response": response }).to_string()
}

fn unused_host() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let address = listener.local_addr().expect("listener should have addr");
    drop(listener);

    format!("http://{address}")
}
