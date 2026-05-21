//! `cowork` CLI library.
//!
//! Thin entry layer for CLI parse and exit flow.

/// CLI types.
pub mod cli;
mod commands;
mod config;
mod error;
mod files;
mod model;
mod output;
mod prompt;

pub use cli::{AskArgs, Cli, Command, DoctorArgs, InitAgent, InitArgs, InitModeArgs, LocateArgs};
pub use commands::run;

#[cfg(test)]
pub(crate) use commands::run_ask_json_in;

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        path::{Path, PathBuf},
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;

    use super::{AskArgs, run_ask_json_in};
    use crate::error::AppError;

    #[test]
    fn missing_model_maps_to_invalid_arguments() {
        let dirs = test_dirs();
        let file = write_temp_file("fn main() {}\n");
        let args = ask_args(vec![file], None, None);

        let error = run_ask_json_in(args, &dirs.project, Some(&dirs.home))
            .expect_err("missing model should fail");

        assert!(matches!(error, AppError::InvalidArguments { .. }));
    }

    #[test]
    fn explicit_host_override_reaches_model_client_path() {
        let dirs = test_dirs();
        let file = write_temp_file("fn main() {}\n");
        let (host, handle) = spawn_server(ok_response(&response_envelope(&valid_model_json())));
        let args = ask_args(vec![file], Some("gemma3:12b"), Some(host));

        let output =
            run_ask_json_in(args, &dirs.project, Some(&dirs.home)).expect("ask should pass");
        let request = handle.join().expect("server should join");
        let request_text = String::from_utf8(request).expect("request should be utf-8");
        let value =
            serde_json::from_str::<serde_json::Value>(&output).expect("output should be json");

        assert!(request_text.starts_with("POST /api/generate HTTP/1.1\r\n"));
        assert_eq!(value["status"], "ok");
    }

    #[test]
    fn happy_path_returns_success_json() {
        let dirs = test_dirs();
        let file = write_temp_file("fn main() {}\n");
        let (host, handle) = spawn_server(ok_response(&response_envelope(&valid_model_json())));
        let args = ask_args(vec![file], Some("gemma3:12b"), Some(host));

        let output =
            run_ask_json_in(args, &dirs.project, Some(&dirs.home)).expect("ask should pass");
        let value =
            serde_json::from_str::<serde_json::Value>(&output).expect("output should be json");

        handle.join().expect("server should join");
        assert_eq!(value["status"], "ok");
    }

    #[test]
    fn bad_model_json_maps_to_response_parse_failed() {
        let dirs = test_dirs();
        let file = write_temp_file("fn main() {}\n");
        let (host, handle) = spawn_server(ok_response(&response_envelope("not json")));
        let args = ask_args(vec![file], Some("gemma3:12b"), Some(host));

        let error = run_ask_json_in(args, &dirs.project, Some(&dirs.home))
            .expect_err("bad model json should fail");

        handle.join().expect("server should join");
        assert!(matches!(error, AppError::ResponseParseFailed { .. }));
    }

    #[test]
    fn config_backed_model_removes_missing_model_error_path() {
        let dirs = test_dirs();
        let file = write_temp_file("fn main() {}\n");
        let (host, handle) = spawn_server(ok_response(&response_envelope(&valid_model_json())));
        write_config(
            &dirs.project.join("cowork.toml"),
            &format!("[ask]\nmodel = \"gemma3:12b\"\nhost = \"{host}\"\n"),
        );
        let args = ask_args(vec![file], None, None);

        let output = run_ask_json_in(args, &dirs.project, Some(&dirs.home))
            .expect("config-backed model should pass");
        let value =
            serde_json::from_str::<serde_json::Value>(&output).expect("output should be json");

        handle.join().expect("server should join");
        assert_eq!(value["status"], "ok");
    }

    fn ask_args(paths: Vec<PathBuf>, model: Option<&str>, host: Option<String>) -> AskArgs {
        AskArgs {
            paths,
            question: "What does this file do?".to_string(),
            model: model.map(str::to_string),
            host,
            max_bytes: None,
            recursive: false,
            include: Vec::new(),
            exclude: Vec::new(),
            fail_on_missing: false,
        }
    }

    fn write_temp_file(content: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "cowork-ask-live-path-{}-{}.rs",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should move forward")
                .as_nanos()
        ));

        fs::write(&path, content).expect("temp file should write");
        path
    }

    struct TestDirs {
        project: PathBuf,
        home: PathBuf,
    }

    fn test_dirs() -> TestDirs {
        let root = std::env::temp_dir().join(format!(
            "cowork-ask-config-{}-{}",
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

    fn write_config(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dir should create");
        }

        fs::write(path, contents).expect("config should write");
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
            "question": "What does this file do?",
            "answer": {
                "summary": "It defines a small function.",
                "confidence": "high",
                "not_found": false
            },
            "files": [
                {
                    "path": "tmp.rs",
                    "included": true,
                    "reason": "Input file.",
                    "bytes": 13
                }
            ],
            "symbols": [],
            "evidence": [],
            "risks": [],
            "next_reads": [],
            "metadata": {
                "input_bytes": 13,
                "duration_ms": 4
            }
        })
        .to_string()
    }
}
