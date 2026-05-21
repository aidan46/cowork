//! Black-box CLI tests for `cowork locate`.

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
    let file = write_temp_file(&dirs.project, "input.rs", "fn answer() -> u8 { 42 }\n");
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
