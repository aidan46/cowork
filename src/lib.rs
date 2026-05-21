//! `cowork` CLI library.
//!
//! Thin entry layer for CLI parse and exit flow.

/// CLI types.
pub mod cli;
mod config;
mod error;
mod files;
mod model;
mod output;
mod prompt;

use std::{env, fs, io::ErrorKind as IoErrorKind, path::Path, process::ExitCode};

use clap::{Parser, error::ErrorKind};

pub use cli::{AskArgs, Cli, Command, DoctorArgs, InitAgent, InitArgs, InitModeArgs};
use config::resolve_ask_config;
use error::{AppError, DoctorExit};
use files::{collect_ask_candidates, load_ask_files};
use model::DoctorProbeErrorKind;
use output::{
    DoctorCheck, DoctorOutput, init_target_file, render_init_rules, update_init_managed_block,
};
use prompt::render_ask_prompt;

/// Run `cowork`.
#[must_use]
pub fn run() -> ExitCode {
    match try_run() {
        Ok(code) => code,
        Err(error) => {
            println!("{}", error.to_json());
            error.exit_code()
        }
    }
}

fn try_run() -> Result<ExitCode, AppError> {
    let cli = parse_cli()?;

    match cli.command {
        Command::Ask(args) => run_ask(args),
        Command::Doctor(args) => run_doctor(args),
        Command::Init(args) => run_init(args),
    }
}

fn parse_cli() -> Result<Cli, AppError> {
    Cli::try_parse().map_err(|error| match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => error.exit(),
        _ => AppError::invalid_arguments(error.to_string()),
    })
}

fn run_ask(args: AskArgs) -> Result<ExitCode, AppError> {
    let ask_json = run_ask_json(args)?;
    println!("{ask_json}");

    Ok(ExitCode::SUCCESS)
}

fn run_ask_json(args: AskArgs) -> Result<String, AppError> {
    let project_dir = env::current_dir().map_err(|error| {
        AppError::invalid_arguments(format!("failed to resolve current dir: {error}"))
    })?;
    let home_dir = env::var_os("HOME").map(std::path::PathBuf::from);

    run_ask_json_in(args, &project_dir, home_dir.as_deref())
}

fn run_ask_json_in(
    args: AskArgs,
    project_dir: &Path,
    home_dir: Option<&Path>,
) -> Result<String, AppError> {
    let AskArgs {
        paths,
        question,
        model,
        host,
        max_bytes,
        recursive,
        include,
        exclude,
        fail_on_missing: _,
    } = args;
    let candidate_paths = collect_ask_candidates(&paths, recursive, &include, &exclude)?;
    let loaded_files = load_ask_files(&candidate_paths, max_bytes)?;
    let prompt = render_ask_prompt(&question, &loaded_files);
    let config = resolve_ask_config(project_dir, home_dir, model, host)?;
    let model = config.model.as_deref().ok_or_else(|| {
        AppError::invalid_arguments("`--model` required, no default model configured yet")
    })?;
    let raw_output = model::request_generate(&config.host, model, &prompt)?;
    let output = output::parse_ask_output(&raw_output)?;

    Ok(output.to_json())
}

fn run_doctor(args: DoctorArgs) -> Result<ExitCode, AppError> {
    let result = run_doctor_json(args)?;
    println!("{}", result.json);

    Ok(result.exit_code)
}

fn run_init(args: InitArgs) -> Result<ExitCode, AppError> {
    let (agent, mode) = match args.agent {
        InitAgent::Codex(mode) => ("codex", mode),
        InitAgent::Claude(mode) => ("claude", mode),
    };

    if mode.print {
        println!("{}", render_init_rules(agent));
        return Ok(ExitCode::SUCCESS);
    }

    let target = Path::new(init_target_file(agent));
    let current = match fs::read_to_string(target) {
        Ok(content) => content,
        Err(error) if error.kind() == IoErrorKind::NotFound => String::new(),
        Err(error) => return Err(AppError::init_file_update(target, error.to_string())),
    };
    let next = update_init_managed_block(agent, &current)
        .map_err(|message| AppError::init_file_update(target, message))?;

    fs::write(target, next)
        .map_err(|error| AppError::init_file_update(target, error.to_string()))?;

    Ok(ExitCode::SUCCESS)
}

fn run_doctor_json(args: DoctorArgs) -> Result<DoctorRunResult, AppError> {
    let project_dir = env::current_dir().map_err(|error| {
        AppError::invalid_arguments(format!("failed to resolve current dir: {error}"))
    })?;
    let home_dir = env::var_os("HOME").map(std::path::PathBuf::from);

    Ok(run_doctor_json_in(args, &project_dir, home_dir.as_deref()))
}

fn run_doctor_json_in(
    args: DoctorArgs,
    project_dir: &Path,
    home_dir: Option<&Path>,
) -> DoctorRunResult {
    let DoctorArgs { model, host } = args;
    let mut checks = Vec::new();
    let config = match resolve_ask_config(project_dir, home_dir, model, host) {
        Ok(config) => config,
        Err(error) => {
            checks.push(DoctorCheck::error(
                "config_files_loaded",
                error.to_string(),
                Some("Fix config TOML, then run `cowork doctor` again."),
            ));
            push_skipped_doctor_checks(
                &mut checks,
                "config load failed, later checks skipped.",
                &[
                    "effective_model_chosen",
                    "host_url_parsed",
                    "generate_endpoint_reachable",
                    "tiny_json_probe_succeeds",
                    "probe_output_shape_valid",
                ],
            );

            return DoctorRunResult::error(checks, DoctorExit::InvalidConfig);
        }
    };

    checks.push(DoctorCheck::ok(
        "config_files_loaded",
        format_loaded_config_message(&config.loaded_files),
    ));

    let model = match config.model.as_deref() {
        Some(model) => {
            checks.push(DoctorCheck::ok(
                "effective_model_chosen",
                format!("Using model `{model}`."),
            ));
            model
        }
        None => {
            checks.push(DoctorCheck::error(
                "effective_model_chosen",
                "No model found from CLI or `[ask].model` config.",
                Some("Pass `--model` or set `[ask].model` in config."),
            ));
            push_skipped_doctor_checks(
                &mut checks,
                "model missing, network checks skipped.",
                &[
                    "host_url_parsed",
                    "generate_endpoint_reachable",
                    "tiny_json_probe_succeeds",
                    "probe_output_shape_valid",
                ],
            );

            return DoctorRunResult::error(checks, DoctorExit::MissingModel);
        }
    };

    if let Err(error) = model::validate_generate_host(&config.host) {
        checks.push(DoctorCheck::error(
            "host_url_parsed",
            error.message,
            Some("Pass `--host http://localhost:11434` or fix `[ask].host`."),
        ));
        push_skipped_doctor_checks(
            &mut checks,
            "host invalid, later checks skipped.",
            &[
                "generate_endpoint_reachable",
                "tiny_json_probe_succeeds",
                "probe_output_shape_valid",
            ],
        );

        return DoctorRunResult::error(checks, DoctorExit::BadHost);
    }

    checks.push(DoctorCheck::ok(
        "host_url_parsed",
        format!("Host URL parsed: `{}`.", config.host),
    ));

    let raw_probe = match model::request_doctor_probe(&config.host, model) {
        Ok(raw_probe) => {
            checks.push(DoctorCheck::ok(
                "generate_endpoint_reachable",
                "Reached `/api/generate`.",
            ));
            checks.push(DoctorCheck::ok(
                "tiny_json_probe_succeeds",
                "Probe request returned JSON text.",
            ));
            raw_probe
        }
        Err(error) => {
            return doctor_probe_error_result(checks, error);
        }
    };

    if let Err(message) = output::parse_doctor_probe(&raw_probe) {
        checks.push(DoctorCheck::error(
            "probe_output_shape_valid",
            message,
            Some("Model must return exact JSON shape `{ \"ok\": true }`."),
        ));

        return DoctorRunResult::error(checks, DoctorExit::InvalidProbeJson);
    }

    checks.push(DoctorCheck::ok(
        "probe_output_shape_valid",
        "Probe JSON matched `{ \"ok\": true }`.",
    ));

    DoctorRunResult::ok(checks)
}

fn doctor_probe_error_result(
    mut checks: Vec<DoctorCheck>,
    error: model::DoctorProbeError,
) -> DoctorRunResult {
    match error.kind {
        DoctorProbeErrorKind::BadHost => {
            checks.push(DoctorCheck::error(
                "generate_endpoint_reachable",
                error.message,
                Some("Pass `--host http://localhost:11434` or fix `[ask].host`."),
            ));
            push_skipped_doctor_checks(
                &mut checks,
                "probe skipped after bad host.",
                &["tiny_json_probe_succeeds", "probe_output_shape_valid"],
            );

            DoctorRunResult::error(checks, DoctorExit::BadHost)
        }
        DoctorProbeErrorKind::UnreachableHost => {
            checks.push(DoctorCheck::error(
                "generate_endpoint_reachable",
                error.message,
                Some("Start Ollama or check host, port, and firewall."),
            ));
            push_skipped_doctor_checks(
                &mut checks,
                "host unreachable, later checks skipped.",
                &["tiny_json_probe_succeeds", "probe_output_shape_valid"],
            );

            DoctorRunResult::error(checks, DoctorExit::UnreachableHost)
        }
        DoctorProbeErrorKind::ProbeRequestFailed => {
            checks.push(DoctorCheck::ok(
                "generate_endpoint_reachable",
                "Reached `/api/generate`.",
            ));
            checks.push(DoctorCheck::error(
                "tiny_json_probe_succeeds",
                error.message,
                Some("Check model exists and endpoint accepts Ollama generate requests."),
            ));
            checks.push(DoctorCheck::skipped(
                "probe_output_shape_valid",
                "probe request failed, shape check skipped.",
            ));

            DoctorRunResult::error(checks, DoctorExit::ProbeRequestFailed)
        }
        DoctorProbeErrorKind::InvalidProbeJson => {
            checks.push(DoctorCheck::ok(
                "generate_endpoint_reachable",
                "Reached `/api/generate`.",
            ));
            checks.push(DoctorCheck::error(
                "tiny_json_probe_succeeds",
                error.message,
                Some("Endpoint must return valid Ollama JSON envelope."),
            ));
            checks.push(DoctorCheck::skipped(
                "probe_output_shape_valid",
                "probe JSON invalid, shape check skipped.",
            ));

            DoctorRunResult::error(checks, DoctorExit::InvalidProbeJson)
        }
    }
}

fn push_skipped_doctor_checks(
    checks: &mut Vec<DoctorCheck>,
    message: &str,
    names: &[&'static str],
) {
    checks.extend(
        names
            .iter()
            .copied()
            .map(|name| DoctorCheck::skipped(name, message)),
    );
}

fn format_loaded_config_message(loaded_files: &[std::path::PathBuf]) -> String {
    match loaded_files {
        [] => "Loaded 0 config files, using built-in defaults.".to_string(),
        [path] => format!("Loaded 1 config file: {}.", path.display()),
        paths => format!(
            "Loaded {} config files: {}.",
            paths.len(),
            paths
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

struct DoctorRunResult {
    json: String,
    exit_code: ExitCode,
}

impl DoctorRunResult {
    fn ok(checks: Vec<DoctorCheck>) -> Self {
        Self {
            json: DoctorOutput::ok(checks).to_json(),
            exit_code: DoctorExit::Ok.exit_code(),
        }
    }

    fn error(checks: Vec<DoctorCheck>, exit: DoctorExit) -> Self {
        Self {
            json: DoctorOutput::error(checks).to_json(),
            exit_code: exit.exit_code(),
        }
    }
}

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
