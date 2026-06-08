use std::{env, path::Path, process::ExitCode, time::Instant};

use crate::{
    SetupArgs,
    config::{ResolvedAskConfig, resolve_ask_config},
    error::AppError,
    model::{self, OllamaErrorKind},
    output::{
        SetupAction, SetupCheck, SetupConfig, SetupMetadata, SetupOutput, SetupRecommendation,
        SetupStatus,
    },
    recommend::{self, HardwareFacts},
};

/// Run setup and print JSON.
///
/// # Errors
///
/// Returns `AppError` on config or JSON output failure.
pub(super) fn run_setup(args: SetupArgs) -> Result<ExitCode, AppError> {
    let setup_json = run_setup_json(args)?;
    println!("{setup_json}");

    Ok(ExitCode::SUCCESS)
}

/// Run setup in cwd and return JSON.
///
/// # Errors
///
/// Returns `AppError` on cwd lookup, config, or JSON output failure.
fn run_setup_json(args: SetupArgs) -> Result<String, AppError> {
    let project_dir = env::current_dir().map_err(|error| {
        AppError::invalid_arguments(format!("failed to resolve current dir: {error}"))
    })?;
    let home_dir = env::var_os("HOME").map(std::path::PathBuf::from);
    let SetupArgs { host } = args;
    let config = resolve_ask_config(&project_dir, home_dir.as_deref(), None, host)?;
    let facts = recommend::collect_hardware_facts();

    render_setup_json(config, home_dir.as_deref(), &facts)
}

/// Run setup with explicit dirs and facts.
///
/// # Errors
///
/// Returns `AppError` on config or JSON output failure.
#[cfg(test)]
pub(crate) fn run_setup_json_in(
    args: SetupArgs,
    project_dir: &Path,
    home_dir: Option<&Path>,
    facts: &HardwareFacts,
) -> Result<String, AppError> {
    let SetupArgs { host } = args;
    let config = resolve_ask_config(project_dir, home_dir, None, host)?;

    render_setup_json(config, home_dir, facts)
}

/// Render setup from resolved config.
///
/// # Errors
///
/// Returns `AppError` on JSON output failure.
fn render_setup_json(
    config: ResolvedAskConfig,
    home_dir: Option<&Path>,
    facts: &HardwareFacts,
) -> Result<String, AppError> {
    let started = Instant::now();
    let mut checks = vec![
        SetupCheck::ok(
            "config_files_loaded",
            format_loaded_config_message(&config.loaded_files),
        ),
        SetupCheck::ok(
            "host_resolved",
            format!("Using Ollama host `{}`.", config.host),
        ),
    ];

    let (status, installed_models) = match model::request_ollama_tags(&config.host) {
        Ok(models) => {
            let names = models
                .into_iter()
                .map(|model| model.name)
                .collect::<Vec<_>>();
            checks.push(SetupCheck::ok(
                "models_listed",
                format!("Listed {} installed Ollama models.", names.len()),
            ));
            (SetupStatus::Ok, names)
        }
        Err(error) => {
            let (status, tag) = listing_failure(error.kind);
            let message = format!("Could not list Ollama models: {tag}.");
            let hint = Some("Start Ollama or check `--host`.");
            let check = match status {
                SetupStatus::Warning => SetupCheck::warning("models_listed", message, hint),
                SetupStatus::Error => SetupCheck::error("models_listed", message, hint),
                SetupStatus::Ok | SetupStatus::Skipped => {
                    SetupCheck::warning("models_listed", message, hint)
                }
            };
            checks.push(check);
            (status, Vec::new())
        }
    };

    let recommendation = recommend::recommend_model(facts, &installed_models);
    let output = match status {
        SetupStatus::Ok => SetupOutput::ok(checks),
        SetupStatus::Warning | SetupStatus::Error | SetupStatus::Skipped => {
            SetupOutput::warning(checks)
        }
    }
    .with_recommendation(
        SetupRecommendation::new(
            recommendation.model(),
            recommendation.needs_pull(),
            recommendation.why(),
        )
        .with_confidence(recommendation.confidence_tag())
        .with_hardware_class(recommendation.hardware_class_tag()),
    )
    .with_actions(vec![
        SetupAction::new(
            "models_listed",
            status,
            action_models_listed_message(status, installed_models.len()),
        ),
        SetupAction::new(
            "model_recommended",
            SetupStatus::Ok,
            format!("Recommended model `{}`.", recommendation.model()),
        )
        .with_model(recommendation.model()),
        SetupAction::new(
            "model_pull_skipped",
            SetupStatus::Skipped,
            "Dry run kept models unchanged.",
        )
        .with_model(recommendation.model()),
    ])
    .with_config(SetupConfig::new(
        "user",
        default_user_config_path(home_dir),
        false,
        false,
    ))
    .with_metadata(SetupMetadata::timed(duration_ms(started)));

    output.to_json()
}

/// Format loaded config summary.
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

/// Map Ollama error to action status and stable tag.
const fn listing_failure(kind: OllamaErrorKind) -> (SetupStatus, &'static str) {
    match kind {
        OllamaErrorKind::BadHost => (SetupStatus::Warning, "bad_host"),
        OllamaErrorKind::UnreachableHost => (SetupStatus::Warning, "unreachable_host"),
        OllamaErrorKind::RequestFailed => (SetupStatus::Error, "request_failed"),
        OllamaErrorKind::InvalidJson => (SetupStatus::Error, "invalid_json"),
    }
}

/// Build action message.
fn action_models_listed_message(status: SetupStatus, count: usize) -> String {
    match status {
        SetupStatus::Ok => format!("Listed {count} installed Ollama models."),
        SetupStatus::Warning => "Model listing unavailable, used empty installed list.".to_string(),
        SetupStatus::Error => "Model listing failed, used empty installed list.".to_string(),
        SetupStatus::Skipped => "Model listing skipped, used empty installed list.".to_string(),
    }
}

/// Default config plan path.
fn default_user_config_path(home_dir: Option<&Path>) -> String {
    home_dir
        .map(|home| home.join(".cowork/config.toml").display().to_string())
        .unwrap_or_default()
}

/// Elapsed ms, saturated.
fn duration_ms(started: Instant) -> usize {
    usize::try_from(started.elapsed().as_millis()).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::missing_errors_doc, reason = "test helpers stay local")]
    #![allow(clippy::missing_panics_doc, reason = "test asserts and fixtures")]

    use std::{
        fs,
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        path::{Path, PathBuf},
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use serde_json::{Value, json};

    use super::{HardwareFacts, SetupArgs, run_setup_json_in};
    use crate::recommend::classify_hardware;

    #[test]
    fn cli_host_override_reaches_tags() {
        let dirs = test_dirs("cli-host");
        let (host, handle) = spawn_server(ok_tags(&["qwen2.5-coder:7b"]));
        let args = setup_args(Some(host));

        let output = run_setup_json_in(args, &dirs.project, Some(&dirs.home), &cpu_standard())
            .expect("setup should pass");
        let request = handle.join().expect("server should join");
        let request_text = String::from_utf8(request).expect("request should be utf-8");
        let value = parse_json(&output);

        assert!(request_text.starts_with("GET /api/tags HTTP/1.1\r\n"));
        assert_eq!(value["status"], "ok");
    }

    #[test]
    fn config_host_used_when_cli_host_absent() {
        let dirs = test_dirs("config-host");
        let (host, handle) = spawn_server(ok_tags(&[]));
        write_config(
            &dirs.project.join("cowork.toml"),
            &format!("[ask]\nhost = \"{host}\"\n"),
        );

        let output = run_setup_json_in(
            setup_args(None),
            &dirs.project,
            Some(&dirs.home),
            &cpu_standard(),
        )
        .expect("setup should pass");
        let request = handle.join().expect("server should join");
        let request_text = String::from_utf8(request).expect("request should be utf-8");
        let value = parse_json(&output);

        assert!(request_text.starts_with("GET /api/tags HTTP/1.1\r\n"));
        assert_eq!(value["checks"][1]["name"], "host_resolved");
        assert_eq!(
            value["checks"][1]["message"],
            format!("Using Ollama host `{host}`.")
        );
    }

    #[test]
    fn empty_tags_returns_builtin_recommendation_needing_pull() {
        let dirs = test_dirs("empty-tags");
        let (host, handle) = spawn_server(ok_tags(&[]));

        let output = run_setup_json_in(
            setup_args(Some(host)),
            &dirs.project,
            Some(&dirs.home),
            &cpu_standard(),
        )
        .expect("setup should pass");
        let value = parse_json(&output);

        handle.join().expect("server should join");
        assert_eq!(value["status"], "ok");
        assert_eq!(value["recommendation"]["model"], "qwen2.5-coder:7b");
        assert_eq!(value["recommendation"]["needs_pull"], true);
        assert_eq!(value["recommendation"]["confidence"], "medium");
        assert_eq!(value["recommendation"]["hardware_class"], "cpu_standard");
    }

    #[test]
    fn acceptable_installed_model_wins_without_pull() {
        let dirs = test_dirs("installed-wins");
        let (host, handle) = spawn_server(ok_tags(&["qwen2.5-coder:3b", "qwen2.5-coder:7b"]));

        let output = run_setup_json_in(
            setup_args(Some(host)),
            &dirs.project,
            Some(&dirs.home),
            &cpu_standard(),
        )
        .expect("setup should pass");
        let value = parse_json(&output);

        handle.join().expect("server should join");
        assert_eq!(value["recommendation"]["model"], "qwen2.5-coder:7b");
        assert_eq!(value["recommendation"]["needs_pull"], false);
    }

    #[test]
    fn tags_failure_returns_warning_and_fallback_recommendation() {
        let dirs = test_dirs("tags-failure");
        let (host, handle) = spawn_server(error_response(500, "boom"));

        let output = run_setup_json_in(
            setup_args(Some(host)),
            &dirs.project,
            Some(&dirs.home),
            &cpu_standard(),
        )
        .expect("setup should pass");
        let value = parse_json(&output);

        handle.join().expect("server should join");
        assert_eq!(value["status"], "warning");
        assert_eq!(value["checks"][2]["name"], "models_listed");
        assert_eq!(value["checks"][2]["status"], "error");
        assert_eq!(
            value["checks"][2]["message"],
            "Could not list Ollama models: request_failed."
        );
        assert_eq!(value["recommendation"]["model"], "qwen2.5-coder:7b");
        assert_eq!(value["recommendation"]["needs_pull"], true);
        assert_eq!(value["actions"][0]["status"], "error");
        assert_eq!(value["actions"][2]["name"], "model_pull_skipped");
        assert_eq!(value["actions"][2]["status"], "skipped");
    }

    #[test]
    fn unreachable_tags_returns_warning_and_fallback_recommendation() {
        let dirs = test_dirs("tags-unreachable");
        let host = unused_host();

        let output = run_setup_json_in(
            setup_args(Some(host)),
            &dirs.project,
            Some(&dirs.home),
            &cpu_standard(),
        )
        .expect("setup should pass");
        let value = parse_json(&output);

        assert_eq!(value["status"], "warning");
        assert_eq!(value["checks"][2]["status"], "warning");
        assert_eq!(
            value["checks"][2]["message"],
            "Could not list Ollama models: unreachable_host."
        );
        assert_eq!(value["recommendation"]["model"], "qwen2.5-coder:7b");
        assert_eq!(value["recommendation"]["needs_pull"], true);
        assert_eq!(value["actions"][0]["status"], "warning");
    }

    #[test]
    fn setup_never_calls_pull() {
        let dirs = test_dirs("no-pull");
        let (host, handle) = spawn_server(ok_tags(&[]));

        run_setup_json_in(
            setup_args(Some(host)),
            &dirs.project,
            Some(&dirs.home),
            &cpu_standard(),
        )
        .expect("setup should pass");
        let request = handle.join().expect("server should join");
        let request_text = String::from_utf8(request).expect("request should be utf-8");

        assert!(request_text.starts_with("GET /api/tags HTTP/1.1\r\n"));
        assert!(!request_text.contains("/api/pull"));
    }

    #[test]
    fn setup_never_writes_config() {
        let dirs = test_dirs("no-write");
        let project_config = dirs.project.join("cowork.toml");
        let user_config = dirs.home.join(".cowork/config.toml");
        write_config(&project_config, "[ask]\nhost = \"://bad\"\n");
        let before_project = fs::read_to_string(&project_config).expect("config should read");
        let before_user = fs::read_to_string(&user_config).ok();

        let output = run_setup_json_in(
            setup_args(None),
            &dirs.project,
            Some(&dirs.home),
            &cpu_standard(),
        )
        .expect("setup should pass");
        let value = parse_json(&output);

        assert_eq!(value["status"], "warning");
        assert_eq!(
            fs::read_to_string(&project_config).expect("config should read"),
            before_project
        );
        assert_eq!(fs::read_to_string(&user_config).ok(), before_user);
    }

    #[test]
    fn setup_output_contains_final_nonzero_output_bytes() {
        let dirs = test_dirs("output-bytes");
        let (host, handle) = spawn_server(ok_tags(&[]));

        let output = run_setup_json_in(
            setup_args(Some(host)),
            &dirs.project,
            Some(&dirs.home),
            &cpu_standard(),
        )
        .expect("setup should pass");
        let value = parse_json(&output);

        handle.join().expect("server should join");
        assert_eq!(value["metadata"]["output_bytes"], output.len());
        assert!(value["metadata"]["output_bytes"].as_u64().unwrap_or(0) > 0);
    }

    fn cpu_standard() -> HardwareFacts {
        let facts = HardwareFacts::new("linux", "x86_64", Some(16 * 1024 * 1024 * 1024), None);

        assert_eq!(classify_hardware(&facts).as_str(), "cpu_standard");
        facts
    }

    fn setup_args(host: Option<String>) -> SetupArgs {
        SetupArgs { host }
    }

    fn parse_json(output: &str) -> Value {
        serde_json::from_str(output).expect("output should be json")
    }

    struct TestDirs {
        project: PathBuf,
        home: PathBuf,
    }

    fn test_dirs(label: &str) -> TestDirs {
        let root = std::env::temp_dir().join(format!(
            "cowork-setup-{label}-{}-{}",
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

    fn unused_host() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let address = listener.local_addr().expect("listener should have addr");
        drop(listener);

        format!("http://{address}")
    }

    fn read_request(stream: &mut TcpStream) -> Vec<u8> {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("timeout should set");

        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];

        loop {
            let read = stream.read(&mut buffer).expect("request should read");
            if read == 0 {
                break;
            }

            request.extend_from_slice(&buffer[..read]);

            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }

        request
    }

    fn ok_tags(models: &[&str]) -> String {
        let models = models
            .iter()
            .map(|name| json!({ "name": name }))
            .collect::<Vec<_>>();

        ok_response(&json!({ "models": models }).to_string())
    }

    fn ok_response(body: &str) -> String {
        http_response(200, "OK", body)
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
}
