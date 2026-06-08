//! Black-box CLI tests for `cowork locate`.
#![allow(clippy::missing_errors_doc, reason = "test helpers stay local")]
#![allow(clippy::missing_panics_doc, reason = "test asserts and fixtures")]
#![allow(clippy::expect_used, reason = "integration test helpers stay direct")]
#![allow(clippy::unwrap_used, reason = "integration test helpers stay direct")]

use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use assert_cmd::cargo::CommandCargoExt;
use serde_json::{Value, json};

#[test]
fn config_backed_success_returns_ok_json() {
    let dirs = test_dirs("config-success");
    let content = "fn answer() -> u8 { 42 }\n";
    let file = write_temp_file(&dirs.project, "input.rs", content);
    let (host, handle) = spawn_server(ok_response(&response_envelope(&valid_model_json())));

    write_config(
        &dirs.project.join("cowork.toml"),
        &format!("[ask]\nmodel = \"gemma3:12b\"\nhost = \"{host}\"\n"),
    );

    let output = run_locate(
        &dirs.project,
        &dirs.home,
        &[arg_path(&file), arg_thing("CLI parser")],
    );

    handle.join().expect("server should join");
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let json = parse_stdout(&output.stdout);
    assert_eq!(json["schema_version"], "1.0");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["command"], "locate");
    assert_eq!(json["matches"][0]["path"], "src/cli.rs");
    assert_eq!(json["next_reads"][0]["path"], "src/cli.rs");
    assert_eq!(json["risks"][0]["kind"], "missing_context");
    assert_eq!(json["metadata"]["input_bytes"], content.len());
    assert!(json["metadata"]["duration_ms"].as_u64().is_some());
    assert!(json["metadata"]["output_bytes"].as_u64().unwrap_or(0) > 0);
    assert!(json["metadata"].get("compression_ratio").is_none());
}

#[test]
fn bounded_success_sets_output_bytes_after_normalization() {
    let dirs = test_dirs("bounded-success");
    let content = "fn answer() -> u8 { 42 }\n";
    let file = write_temp_file(&dirs.project, "input.rs", content);
    let (host, handle) = spawn_server(ok_response(&response_envelope(&bounded_model_json())));

    write_config(
        &dirs.project.join("cowork.toml"),
        &format!("[ask]\nmodel = \"gemma3:12b\"\nhost = \"{host}\"\n"),
    );

    let output = run_locate(
        &dirs.project,
        &dirs.home,
        &[arg_path(&file), arg_thing("CLI parser")],
    );

    handle.join().expect("server should join");
    assert_eq!(output.status.code(), Some(0));

    let json = parse_stdout(&output.stdout);
    let risks = json["risks"].as_array().expect("risks should be array");
    let stdout_json = String::from_utf8(output.stdout.clone()).expect("stdout should be utf-8");

    assert_eq!(
        json["metadata"]["output_bytes"],
        stdout_json.trim_end_matches('\n').len()
    );
    assert_eq!(risks.len(), 20);
    assert_eq!(risks[18]["kind"], "unknown");
    assert_eq!(risks[19]["kind"], "unknown");
}

#[test]
fn strict_missing_path_returns_missing_path_error() {
    let dirs = test_dirs("strict-missing");
    let missing = dirs.project.join("missing.rs");

    let output = run_locate(
        &dirs.project,
        &dirs.home,
        &[arg_path(&missing), arg_thing("CLI parser")],
    );

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());

    let json = parse_stdout(&output.stdout);
    assert_eq!(json["command"], "locate");
    assert_eq!(json["status"], "error");
    assert_eq!(json["error"]["code"], "MISSING_PATH");
}

#[test]
fn no_fail_on_missing_mixed_paths_reach_runtime_flow() {
    let dirs = test_dirs("mixed-paths");
    let file = write_temp_file(&dirs.project, "input.rs", "fn main() {}\n");
    let missing = dirs.project.join("missing.rs");

    let output = run_locate(
        &dirs.project,
        &dirs.home,
        &[
            arg_path(&file),
            arg_path(&missing),
            arg_thing("CLI parser"),
            "--no-fail-on-missing".to_string(),
        ],
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());

    let json = parse_stdout(&output.stdout);
    assert_eq!(json["command"], "locate");
    assert_eq!(json["status"], "error");
    assert_eq!(json["error"]["code"], "INVALID_ARGUMENTS");
}

#[test]
fn no_fail_on_missing_all_missing_paths_return_no_input_files() {
    let dirs = test_dirs("all-missing");
    let missing = dirs.project.join("missing.rs");

    let output = run_locate(
        &dirs.project,
        &dirs.home,
        &[
            arg_path(&missing),
            arg_thing("CLI parser"),
            "--no-fail-on-missing".to_string(),
            "--model".to_string(),
            "gemma3:12b".to_string(),
            "--host".to_string(),
            "http://127.0.0.1:1".to_string(),
        ],
    );

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());

    let json = parse_stdout(&output.stdout);
    assert_eq!(json["command"], "locate");
    assert_eq!(json["status"], "error");
    assert_eq!(json["error"]["code"], "NO_INPUT_FILES");
}

fn run_locate(project_dir: &Path, home_dir: &Path, args: &[String]) -> std::process::Output {
    Command::cargo_bin("cowork")
        .expect("binary should build")
        .current_dir(project_dir)
        .env("HOME", home_dir)
        .args(["locate"])
        .args(args)
        .output()
        .expect("command should run")
}

fn parse_stdout(stdout: &[u8]) -> Value {
    serde_json::from_slice(stdout).expect("stdout should be valid json")
}

fn arg_path(path: &Path) -> String {
    format!("--paths={}", path.to_string_lossy())
}

fn arg_thing(thing: &str) -> String {
    format!("--thing={thing}")
}

fn write_temp_file(project_dir: &Path, name: &str, content: &str) -> PathBuf {
    let path = project_dir.join(name);
    fs::write(&path, content).expect("file should write");
    path
}

struct TestDirs {
    project: PathBuf,
    home: PathBuf,
}

fn test_dirs(label: &str) -> TestDirs {
    let root = std::env::temp_dir().join(format!(
        "cowork-locate-cli-test-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos()
    ));
    let project = root.join("project");
    let home = root.join("home");

    fs::create_dir_all(&project).expect("project dir should create");
    fs::create_dir_all(home.join(".cowork")).expect("home config dir should create");

    TestDirs { project, home }
}

fn write_config(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("config parent should create");
    }
    fs::write(path, content).expect("config should write");
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

fn valid_model_json() -> String {
    json!({
        "matches": [
            {
                "path": "src/cli.rs",
                "symbol": "Command",
                "kind": "type",
                "reason": "Defines CLI command variants.",
                "confidence": "high"
            },
            {
                "path": "input.rs",
                "reason": "Loaded file may define the requested item.",
                "confidence": "medium"
            }
        ],
        "next_reads": [
            {
                "path": "src/cli.rs",
                "reason": "Contains subcommand args."
            }
        ],
        "risks": [
            {
                "kind": "missing_context",
                "message": "Only one file was loaded."
            }
        ]
    })
    .to_string()
}

fn bounded_model_json() -> String {
    let matches = (0..85)
        .map(|index| {
            json!({
                "path": if index == 0 { long_string(1300) } else { format!("src/file-{index:02}.rs") },
                "symbol": if index == 0 { long_string(1301) } else { format!("symbol_{index:02}") },
                "kind": "function",
                "reason": if index == 0 { long_string(1302) } else { format!("Reason {index:02}.") },
                "confidence": if index % 2 == 0 { "high" } else { "medium" }
            })
        })
        .collect::<Vec<_>>();
    let next_reads = (0..25)
        .map(|index| {
            json!({
                "path": if index == 0 { long_string(1303) } else { format!("src/next-{index:02}.rs") },
                "reason": if index == 0 { long_string(1304) } else { format!("Next read {index:02}.") }
            })
        })
        .collect::<Vec<_>>();
    let risks = (0..25)
        .map(|index| {
            json!({
                "kind": "missing_context",
                "message": if index == 0 { long_string(1305) } else { format!("Risk {index:02}.") }
            })
        })
        .collect::<Vec<_>>();

    json!({
        "matches": matches,
        "next_reads": next_reads,
        "risks": risks
    })
    .to_string()
}

fn long_string(len: usize) -> String {
    "x".repeat(len)
}
