//! CLI tests for `cowork doctor`.
#![allow(clippy::missing_errors_doc, reason = "test helpers stay local")]
#![allow(clippy::missing_panics_doc, reason = "test asserts and fixtures")]
#![allow(clippy::expect_used, reason = "integration test helpers stay direct")]
#![allow(clippy::unwrap_used, reason = "integration test helpers stay direct")]

use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use assert_cmd::Command;
use serde_json::{Value, json};

#[test]
fn doctor_exits_zero_when_all_checks_pass() {
    let dirs = test_dirs();
    let (host, handle) = spawn_server_sequence(vec![
        ok_tags(&["gemma3:12b"]),
        ok_response(&response_envelope(r#"{"ok":true}"#)),
    ]);

    let assert = Command::cargo_bin("cowork")
        .expect("binary should build")
        .current_dir(&dirs.project)
        .env("HOME", &dirs.home)
        .args(["doctor", "--model", "gemma3:12b", "--host", &host])
        .assert()
        .success();
    assert!(assert.get_output().stderr.is_empty());
    let output = assert.get_output().stdout.clone();
    let value = parse_json_output(&output);
    let requests = handle.join().expect("server should join");
    let tags_request = String::from_utf8(requests[0].clone()).expect("request should be utf-8");
    let probe_request = String::from_utf8(requests[1].clone()).expect("request should be utf-8");

    assert!(tags_request.starts_with("GET /api/tags HTTP/1.1\r\n"));
    assert!(probe_request.starts_with("POST /api/generate HTTP/1.1\r\n"));
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
                    "name": "installed_models_listed",
                    "status": "ok",
                    "message": "Listed 1 installed Ollama models."
                },
                {
                    "name": "effective_model_installed",
                    "status": "ok",
                    "message": "Effective model `gemma3:12b` is installed."
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
    let (host, stop, handle) = spawn_no_request_server();

    let assert = Command::cargo_bin("cowork")
        .expect("binary should build")
        .current_dir(&dirs.project)
        .env("HOME", &dirs.home)
        .args(["doctor", "--host", &host])
        .assert()
        .code(7);
    let output = assert.get_output().stdout.clone();
    let value = parse_json_output(&output);

    assert_eq!(value["status"], "error");
    assert_eq!(value["checks"][1]["name"], "effective_model_chosen");
    assert_eq!(value["checks"][1]["status"], "error");
    assert_eq!(
        value["checks"][1]["hint"],
        "Run `cowork setup --write-config`."
    );
    assert_eq!(value["checks"][2]["status"], "skipped");
    assert_eq!(value["checks"][7]["status"], "skipped");
    stop.send(()).expect("server should stop");
    assert!(!handle.join().expect("server should join"));
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
    assert_eq!(value["checks"][3]["name"], "installed_models_listed");
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
    assert_eq!(value["checks"][3]["name"], "installed_models_listed");
    assert_eq!(value["checks"][3]["status"], "error");
    assert_eq!(value["checks"][4]["status"], "skipped");
    assert_eq!(value["checks"][4]["name"], "effective_model_installed");
    assert_eq!(value["checks"][7]["status"], "skipped");
}

#[test]
fn doctor_bad_probe_json_returns_exit_code_ten() {
    let dirs = test_dirs();
    let (host, handle) = spawn_server_sequence(vec![
        ok_tags(&["gemma3:12b"]),
        ok_response(&response_envelope(r#"{"ready":true}"#)),
    ]);

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
    assert_eq!(value["checks"][6]["status"], "ok");
    assert_eq!(value["checks"][7]["name"], "probe_output_shape_valid");
    assert_eq!(value["checks"][7]["status"], "error");
}

#[test]
fn doctor_empty_tags_reports_zero_and_skips_probe() {
    let dirs = test_dirs();
    let (host, handle) = spawn_server(ok_tags(&[]));

    let assert = Command::cargo_bin("cowork")
        .expect("binary should build")
        .current_dir(&dirs.project)
        .env("HOME", &dirs.home)
        .args(["doctor", "--model", "gemma3:12b", "--host", &host])
        .assert()
        .code(9);
    let value = parse_json_output(&assert.get_output().stdout);
    let request = handle.join().expect("server should join");
    let request = String::from_utf8(request).expect("request should be utf-8");

    assert!(request.starts_with("GET /api/tags HTTP/1.1\r\n"));
    assert_eq!(value["checks"][3]["name"], "installed_models_listed");
    assert_eq!(
        value["checks"][3]["message"],
        "Listed 0 installed Ollama models."
    );
    assert_eq!(value["checks"][4]["name"], "effective_model_installed");
    assert_eq!(value["checks"][4]["status"], "error");
    assert_eq!(value["checks"][4]["hint"], "Run `cowork setup --pull`.");
    assert_eq!(value["checks"][5]["status"], "skipped");
    assert_eq!(value["checks"][7]["status"], "skipped");
}

#[test]
fn doctor_requires_exact_installed_model_name() {
    let dirs = test_dirs();
    let (host, handle) = spawn_server(ok_tags(&["gemma3:12b-q4_K_M", "qwen3:8b"]));

    let assert = Command::cargo_bin("cowork")
        .expect("binary should build")
        .current_dir(&dirs.project)
        .env("HOME", &dirs.home)
        .args(["doctor", "--model", "gemma3:12b", "--host", &host])
        .assert()
        .code(9);
    let value = parse_json_output(&assert.get_output().stdout);

    handle.join().expect("server should join");
    assert_eq!(
        value["checks"][3]["message"],
        "Listed 2 installed Ollama models."
    );
    assert_eq!(
        value["checks"][4]["message"],
        "Effective model `gemma3:12b` is not installed."
    );
    assert_eq!(value["checks"][5]["name"], "generate_endpoint_reachable");
    assert_eq!(value["checks"][5]["status"], "skipped");
}

#[test]
fn doctor_tags_request_failure_returns_exit_code_nine() {
    let dirs = test_dirs();
    let (host, handle) = spawn_server(error_response(500, r#"{"error":"failed"}"#));

    let assert = Command::cargo_bin("cowork")
        .expect("binary should build")
        .current_dir(&dirs.project)
        .env("HOME", &dirs.home)
        .args(["doctor", "--model", "gemma3:12b", "--host", &host])
        .assert()
        .code(9);
    let value = parse_json_output(&assert.get_output().stdout);

    handle.join().expect("server should join");
    assert_eq!(value["checks"][3]["status"], "error");
    assert_eq!(
        value["checks"][3]["message"],
        "Could not list installed Ollama models: request_failed."
    );
    assert_eq!(value["checks"][4]["name"], "effective_model_installed");
    assert_eq!(value["checks"][4]["status"], "skipped");
    assert_eq!(value["checks"][7]["status"], "skipped");
}

#[test]
fn doctor_invalid_tags_json_returns_exit_code_ten() {
    let dirs = test_dirs();
    let (host, handle) = spawn_server(ok_response(r#"{"models":null}"#));

    let assert = Command::cargo_bin("cowork")
        .expect("binary should build")
        .current_dir(&dirs.project)
        .env("HOME", &dirs.home)
        .args(["doctor", "--model", "gemma3:12b", "--host", &host])
        .assert()
        .code(10);
    let value = parse_json_output(&assert.get_output().stdout);

    handle.join().expect("server should join");
    assert_eq!(value["checks"][3]["status"], "error");
    assert_eq!(
        value["checks"][3]["message"],
        "Could not list installed Ollama models: invalid_json."
    );
    assert_eq!(value["checks"][4]["status"], "skipped");
    assert_eq!(value["checks"][7]["status"], "skipped");
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

fn spawn_server_sequence(responses: Vec<String>) -> (String, thread::JoinHandle<Vec<Vec<u8>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let address = listener.local_addr().expect("listener should have addr");

    let handle = thread::spawn(move || {
        responses
            .into_iter()
            .map(|response| {
                let (mut stream, _) = listener.accept().expect("server should accept");
                let request = read_request(&mut stream);

                stream
                    .write_all(response.as_bytes())
                    .expect("response should write");
                stream.flush().expect("response should flush");

                request
            })
            .collect()
    });

    (format!("http://{address}"), handle)
}

fn spawn_no_request_server() -> (String, mpsc::Sender<()>, thread::JoinHandle<bool>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let address = listener.local_addr().expect("listener should have addr");
    listener
        .set_nonblocking(true)
        .expect("listener should be nonblocking");
    let (stop_tx, stop_rx) = mpsc::channel();

    let handle = thread::spawn(move || {
        loop {
            match listener.accept() {
                Ok((_stream, _)) => return true,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if stop_rx.try_recv().is_ok() {
                        return false;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("server accept failed: {error}"),
            }
        }
    });

    (format!("http://{address}"), stop_tx, handle)
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

fn ok_tags(models: &[&str]) -> String {
    let models = models
        .iter()
        .map(|name| json!({ "name": name }))
        .collect::<Vec<_>>();

    ok_response(&json!({ "models": models }).to_string())
}

fn error_response(status: u16, body: &str) -> String {
    http_response(status, "ERROR", body)
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
