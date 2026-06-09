use std::{
    env,
    path::{Path, PathBuf},
    process::ExitCode,
    time::Instant,
};

use crate::{
    SetupArgs,
    config::{AskConfigWrite, ResolvedAskConfig, resolve_ask_config, write_ask_config},
    error::AppError,
    model::{self, DoctorProbeErrorKind, OllamaErrorKind},
    output::{
        SetupAction, SetupCheck, SetupConfig, SetupMetadata, SetupOutput, SetupRecommendation,
        SetupStatus,
    },
    recommend::{self, HardwareFacts},
};

/// Setup mutation options.
struct SetupOptions {
    /// Model came from CLI.
    cli_model: bool,
    /// Pull missing model.
    pull_requested: bool,
    /// Write chosen config.
    write_requested: bool,
    /// Use project config target.
    project_target: bool,
    /// Replace conflicting ask values.
    force: bool,
}

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
    let SetupArgs {
        model,
        host,
        pull,
        write_config,
        user: _,
        project,
        force,
    } = args;
    let cli_model = model.is_some();
    let config = resolve_ask_config(&project_dir, home_dir.as_deref(), model, host)?;
    let facts = recommend::collect_hardware_facts();

    render_setup_json(
        config,
        &project_dir,
        home_dir.as_deref(),
        &facts,
        SetupOptions {
            cli_model,
            pull_requested: pull,
            write_requested: write_config,
            project_target: project,
            force,
        },
    )
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
    let SetupArgs {
        model,
        host,
        pull,
        write_config,
        user: _,
        project,
        force,
    } = args;
    let cli_model = model.is_some();
    let config = resolve_ask_config(project_dir, home_dir, model, host)?;

    render_setup_json(
        config,
        project_dir,
        home_dir,
        facts,
        SetupOptions {
            cli_model,
            pull_requested: pull,
            write_requested: write_config,
            project_target: project,
            force,
        },
    )
}

/// Render setup from resolved config.
///
/// # Errors
///
/// Returns `AppError` on JSON output failure.
fn render_setup_json(
    config: ResolvedAskConfig,
    project_dir: &Path,
    home_dir: Option<&Path>,
    facts: &HardwareFacts,
    options: SetupOptions,
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
    let (chosen_model, reason, confidence, hardware_class, selection_message, mut needs_pull) =
        match config.model.as_deref() {
            Some(model) if options.cli_model => (
                model,
                "Selected by `--model`.",
                None,
                None,
                format!("Selected model `{model}` from `--model`."),
                !installed_models.iter().any(|installed| installed == model),
            ),
            Some(model) => (
                model,
                "Selected from resolved config.",
                None,
                None,
                format!("Selected configured model `{model}`."),
                !installed_models.iter().any(|installed| installed == model),
            ),
            None => (
                recommendation.model(),
                recommendation.why(),
                Some(recommendation.confidence_tag()),
                Some(recommendation.hardware_class_tag()),
                format!("Recommended model `{}`.", recommendation.model()),
                recommendation.needs_pull(),
            ),
        };
    let (pull_status, pull_message, pull_failed) = if !needs_pull {
        (
            SetupStatus::Skipped,
            "Chosen model is already installed.".to_string(),
            false,
        )
    } else if !options.pull_requested {
        (
            SetupStatus::Skipped,
            "Pull not requested; chosen model remains missing.".to_string(),
            false,
        )
    } else {
        match model::request_ollama_pull(&config.host, chosen_model) {
            Ok(()) => {
                needs_pull = false;
                (
                    SetupStatus::Ok,
                    format!("Pulled chosen model `{chosen_model}`."),
                    false,
                )
            }
            Err(error) => (
                SetupStatus::Error,
                format!(
                    "Could not pull chosen model `{chosen_model}`: {}.",
                    ollama_error_tag(error.kind)
                ),
                true,
            ),
        }
    };

    let mut recommendation_row = SetupRecommendation::new(chosen_model, needs_pull, reason);
    if let Some(confidence) = confidence {
        recommendation_row = recommendation_row.with_confidence(confidence);
    }
    if let Some(hardware_class) = hardware_class {
        recommendation_row = recommendation_row.with_hardware_class(hardware_class);
    }

    let (config_target, config_path) =
        config_target_path(project_dir, home_dir, options.project_target);
    let config_path_text = config_path
        .as_deref()
        .map_or_else(String::new, |path| path.display().to_string());
    let (config_status, config_message, config_failed) = if !options.write_requested {
        (
            SetupStatus::Skipped,
            "Config write not requested.".to_string(),
            false,
        )
    } else if let Some(path) = config_path.as_deref() {
        match write_ask_config(path, chosen_model, &config.host, options.force) {
            Ok(AskConfigWrite::Written) => (
                SetupStatus::Ok,
                "Wrote chosen model and host to config.".to_string(),
                false,
            ),
            Ok(AskConfigWrite::Unchanged) => (
                SetupStatus::Skipped,
                "Config already contains chosen model and host.".to_string(),
                false,
            ),
            Err(error) => (
                SetupStatus::Error,
                format!("Could not write config: {error}."),
                true,
            ),
        }
    } else {
        (
            SetupStatus::Error,
            "Could not write config: HOME is not set.".to_string(),
            true,
        )
    };

    let (probe_status, probe_message, probe_failed) =
        probe_chosen_model(&config.host, chosen_model, needs_pull, pull_failed);

    let output = if pull_failed || config_failed || probe_failed {
        SetupOutput::error(checks)
    } else {
        match status {
            SetupStatus::Ok => SetupOutput::ok(checks),
            SetupStatus::Warning | SetupStatus::Error | SetupStatus::Skipped => {
                SetupOutput::warning(checks)
            }
        }
    }
    .with_recommendation(recommendation_row)
    .with_actions(vec![
        SetupAction::new(
            "models_listed",
            status,
            action_models_listed_message(status, installed_models.len()),
        ),
        SetupAction::new("model_selected", SetupStatus::Ok, selection_message)
            .with_model(chosen_model),
        SetupAction::new("model_pull", pull_status, pull_message).with_model(chosen_model),
        SetupAction::new("config_write", config_status, config_message)
            .with_model(chosen_model)
            .with_path(&config_path_text),
        SetupAction::new("model_probe", probe_status, probe_message).with_model(chosen_model),
    ])
    .with_config(SetupConfig::new(
        config_target,
        config_path_text,
        options.write_requested,
        options.force,
    ))
    .with_metadata(SetupMetadata::timed(duration_ms(started)));

    output.to_json()
}

/// Probe available chosen model.
fn probe_chosen_model(
    host: &str,
    chosen_model: &str,
    needs_pull: bool,
    pull_failed: bool,
) -> (SetupStatus, String, bool) {
    if pull_failed {
        return (
            SetupStatus::Skipped,
            "Probe skipped; model pull failed.".to_string(),
            false,
        );
    }

    if needs_pull {
        return (
            SetupStatus::Skipped,
            "Probe skipped; chosen model remains missing.".to_string(),
            false,
        );
    }

    let raw_probe = match model::request_doctor_probe(host, chosen_model) {
        Ok(raw_probe) => raw_probe,
        Err(error) => {
            return (
                SetupStatus::Error,
                format!(
                    "Could not probe chosen model `{chosen_model}`: {}.",
                    probe_request_error_tag(error.kind)
                ),
                true,
            );
        }
    };

    if crate::output::parse_doctor_probe(&raw_probe).is_err() {
        return (
            SetupStatus::Error,
            format!("Could not probe chosen model `{chosen_model}`: invalid_output_shape."),
            true,
        );
    }

    (
        SetupStatus::Ok,
        "Chosen model probe passed.".to_string(),
        false,
    )
}

/// Stable probe request error tag.
const fn probe_request_error_tag(kind: DoctorProbeErrorKind) -> &'static str {
    match kind {
        DoctorProbeErrorKind::BadHost => "bad_host",
        DoctorProbeErrorKind::UnreachableHost => "unreachable_host",
        DoctorProbeErrorKind::ProbeRequestFailed => "request_failed",
        DoctorProbeErrorKind::InvalidProbeJson => "invalid_response_envelope",
    }
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
    let status = match kind {
        OllamaErrorKind::BadHost | OllamaErrorKind::UnreachableHost => SetupStatus::Warning,
        OllamaErrorKind::RequestFailed | OllamaErrorKind::InvalidJson => SetupStatus::Error,
    };

    (status, ollama_error_tag(kind))
}

/// Stable Ollama error tag.
const fn ollama_error_tag(kind: OllamaErrorKind) -> &'static str {
    match kind {
        OllamaErrorKind::BadHost => "bad_host",
        OllamaErrorKind::UnreachableHost => "unreachable_host",
        OllamaErrorKind::RequestFailed => "request_failed",
        OllamaErrorKind::InvalidJson => "invalid_json",
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

/// Resolve config write target and path.
fn config_target_path(
    project_dir: &Path,
    home_dir: Option<&Path>,
    project_target: bool,
) -> (&'static str, Option<PathBuf>) {
    if project_target {
        ("project", Some(project_dir.join("cowork.toml")))
    } else {
        (
            "user",
            home_dir.map(|home| home.join(".cowork/config.toml")),
        )
    }
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
        let (host, handle) = spawn_server_sequence(vec![
            ok_tags(&["qwen2.5-coder:7b"]),
            ok_probe_response(r#"{"ok":true}"#),
        ]);
        let args = setup_args(Some(host));

        let output = run_setup_json_in(args, &dirs.project, Some(&dirs.home), &cpu_standard())
            .expect("setup should pass");
        let requests = handle.join().expect("server should join");
        let value = parse_json(&output);

        assert!(requests[0].starts_with(b"GET /api/tags HTTP/1.1\r\n"));
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
        let (host, handle) = spawn_server_sequence(vec![
            ok_tags(&["qwen2.5-coder:3b", "qwen2.5-coder:7b"]),
            ok_probe_response(r#"{"ok":true}"#),
        ]);

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
    fn configured_model_wins_over_recommendation() {
        let dirs = test_dirs("config-model");
        let (host, handle) = spawn_server(ok_tags(&["qwen2.5-coder:7b"]));
        write_config(
            &dirs.project.join("cowork.toml"),
            "[ask]\nmodel = \"gemma3:12b\"\n",
        );

        let output = run_setup_json_in(
            setup_args(Some(host)),
            &dirs.project,
            Some(&dirs.home),
            &cpu_standard(),
        )
        .expect("setup should pass");
        let value = parse_json(&output);

        handle.join().expect("server should join");
        assert_eq!(value["recommendation"]["model"], "gemma3:12b");
        assert_eq!(value["recommendation"]["needs_pull"], true);
        assert_eq!(
            value["recommendation"]["reason"],
            "Selected from resolved config."
        );
        assert_eq!(value["actions"][1]["name"], "model_selected");
        assert_eq!(value["actions"][1]["model"], "gemma3:12b");
    }

    #[test]
    fn cli_model_wins_over_configured_model_without_pull() {
        let dirs = test_dirs("cli-model");
        let (host, handle) = spawn_server(ok_tags(&[]));
        write_config(
            &dirs.project.join("cowork.toml"),
            "[ask]\nmodel = \"qwen2.5-coder:3b\"\n",
        );

        let output = run_setup_json_in(
            setup_args_with(Some(host), Some("gemma3:12b"), false),
            &dirs.project,
            Some(&dirs.home),
            &cpu_standard(),
        )
        .expect("setup should pass");
        let request = handle.join().expect("server should join");
        let request_text = String::from_utf8(request).expect("request should be utf-8");
        let value = parse_json(&output);

        assert!(request_text.starts_with("GET /api/tags HTTP/1.1\r\n"));
        assert_eq!(value["recommendation"]["model"], "gemma3:12b");
        assert_eq!(value["recommendation"]["needs_pull"], true);
        assert_eq!(value["recommendation"]["reason"], "Selected by `--model`.");
        assert_eq!(value["actions"][2]["name"], "model_pull");
        assert_eq!(value["actions"][2]["status"], "skipped");
        assert_eq!(value["actions"][4]["name"], "model_probe");
        assert_eq!(value["actions"][4]["status"], "skipped");
        assert_eq!(
            value["actions"][4]["message"],
            "Probe skipped; chosen model remains missing."
        );
    }

    #[test]
    fn missing_chosen_model_with_pull_sends_one_exact_pull_request() {
        let dirs = test_dirs("pull-missing");
        let (host, handle) = spawn_server_sequence(vec![
            ok_tags(&["gemma3:12b-latest"]),
            ok_response(r#"{"status":"success"}"#),
            ok_probe_response(r#"{"ok":true}"#),
        ]);

        let output = run_setup_json_in(
            setup_args_with(Some(host), Some("gemma3:12b"), true),
            &dirs.project,
            Some(&dirs.home),
            &cpu_standard(),
        )
        .expect("setup should pass");
        let requests = handle.join().expect("server should join");
        let value = parse_json(&output);

        assert_eq!(requests.len(), 3);
        assert!(requests[0].starts_with(b"GET /api/tags HTTP/1.1\r\n"));
        assert!(requests[1].starts_with(b"POST /api/pull HTTP/1.1\r\n"));
        assert!(requests[2].starts_with(b"POST /api/generate HTTP/1.1\r\n"));
        assert_eq!(
            serde_json::from_slice::<Value>(request_body(&requests[1]))
                .expect("pull body should be json"),
            json!({ "model": "gemma3:12b", "stream": false })
        );
        assert_eq!(value["recommendation"]["needs_pull"], false);
        assert_eq!(value["actions"][2]["name"], "model_pull");
        assert_eq!(value["actions"][2]["status"], "ok");
        assert_eq!(value["actions"][4]["name"], "model_probe");
        assert_eq!(value["actions"][4]["status"], "ok");
    }

    #[test]
    fn installed_chosen_model_with_pull_skips_pull_request() {
        let dirs = test_dirs("pull-installed");
        let (host, handle) = spawn_server_sequence(vec![
            ok_tags(&["gemma3:12b"]),
            ok_probe_response(r#"{"ok":true}"#),
        ]);

        let output = run_setup_json_in(
            setup_args_with(Some(host), Some("gemma3:12b"), true),
            &dirs.project,
            Some(&dirs.home),
            &cpu_standard(),
        )
        .expect("setup should pass");
        let requests = handle.join().expect("server should join");
        let value = parse_json(&output);

        assert_eq!(requests.len(), 2);
        assert!(requests[0].starts_with(b"GET /api/tags HTTP/1.1\r\n"));
        assert!(requests[1].starts_with(b"POST /api/generate HTTP/1.1\r\n"));
        assert_eq!(
            serde_json::from_slice::<Value>(request_body(&requests[1]))
                .expect("probe body should be json"),
            json!({
                "model": "gemma3:12b",
                "prompt": "Return strict JSON only with exact shape {\"ok\":true}.",
                "stream": false,
                "format": "json",
                "options": { "temperature": 0 }
            })
        );
        assert_eq!(value["recommendation"]["needs_pull"], false);
        assert_eq!(value["actions"][2]["status"], "skipped");
        assert_eq!(
            value["actions"][2]["message"],
            "Chosen model is already installed."
        );
        assert_eq!(value["actions"][4]["status"], "ok");
    }

    #[test]
    fn pull_failure_returns_deterministic_setup_json() {
        let dirs = test_dirs("pull-failure");
        let (host, handle) = spawn_server_sequence(vec![ok_tags(&[]), error_response(500, "boom")]);

        let output = run_setup_json_in(
            setup_args_with(Some(host), Some("gemma3:12b"), true),
            &dirs.project,
            Some(&dirs.home),
            &cpu_standard(),
        )
        .expect("setup should return json");
        let requests = handle.join().expect("server should join");
        let value = parse_json(&output);

        assert_eq!(requests.len(), 2);
        assert_eq!(value["status"], "error");
        assert_eq!(value["recommendation"]["model"], "gemma3:12b");
        assert_eq!(value["recommendation"]["needs_pull"], true);
        assert_eq!(value["actions"][2]["name"], "model_pull");
        assert_eq!(value["actions"][2]["status"], "error");
        assert_eq!(
            value["actions"][2]["message"],
            "Could not pull chosen model `gemma3:12b`: request_failed."
        );
        assert_eq!(value["actions"][4]["name"], "model_probe");
        assert_eq!(value["actions"][4]["status"], "skipped");
        assert_eq!(
            value["actions"][4]["message"],
            "Probe skipped; model pull failed."
        );
    }

    #[test]
    fn probe_request_failure_returns_stable_error() {
        let dirs = test_dirs("probe-request-failure");
        let (host, handle) = spawn_server_sequence(vec![
            ok_tags(&["gemma3:12b"]),
            error_response(500, "variable transport detail"),
        ]);

        let output = run_setup_json_in(
            setup_args_with(Some(host), Some("gemma3:12b"), false),
            &dirs.project,
            Some(&dirs.home),
            &cpu_standard(),
        )
        .expect("setup should return json");
        let requests = handle.join().expect("server should join");
        let value = parse_json(&output);

        assert_eq!(requests.len(), 2);
        assert_eq!(value["status"], "error");
        assert_eq!(value["actions"][4]["name"], "model_probe");
        assert_eq!(value["actions"][4]["status"], "error");
        assert_eq!(
            value["actions"][4]["message"],
            "Could not probe chosen model `gemma3:12b`: request_failed."
        );
    }

    #[test]
    fn invalid_probe_envelope_returns_stable_error() {
        let dirs = test_dirs("probe-envelope-failure");
        let (host, handle) = spawn_server_sequence(vec![
            ok_tags(&["gemma3:12b"]),
            ok_response(r#"{"done":true}"#),
        ]);

        let output = run_setup_json_in(
            setup_args_with(Some(host), Some("gemma3:12b"), false),
            &dirs.project,
            Some(&dirs.home),
            &cpu_standard(),
        )
        .expect("setup should return json");
        handle.join().expect("server should join");
        let value = parse_json(&output);

        assert_eq!(value["status"], "error");
        assert_eq!(
            value["actions"][4]["message"],
            "Could not probe chosen model `gemma3:12b`: invalid_response_envelope."
        );
    }

    #[test]
    fn invalid_probe_output_shape_returns_stable_error() {
        let dirs = test_dirs("probe-shape-failure");
        let (host, handle) = spawn_server_sequence(vec![
            ok_tags(&["gemma3:12b"]),
            ok_probe_response(r#"{"ready":true}"#),
        ]);

        let output = run_setup_json_in(
            setup_args_with(Some(host), Some("gemma3:12b"), false),
            &dirs.project,
            Some(&dirs.home),
            &cpu_standard(),
        )
        .expect("setup should return json");
        handle.join().expect("server should join");
        let value = parse_json(&output);

        assert_eq!(value["status"], "error");
        assert_eq!(
            value["actions"][4]["message"],
            "Could not probe chosen model `gemma3:12b`: invalid_output_shape."
        );
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
        assert_eq!(value["actions"][2]["name"], "model_pull");
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
        assert_eq!(value["actions"][3]["name"], "config_write");
        assert_eq!(value["actions"][3]["status"], "skipped");
        assert_eq!(value["config"]["target"], "user");
        assert_eq!(value["config"]["write_requested"], false);
        assert_eq!(value["config"]["force"], false);
    }

    #[test]
    fn write_config_defaults_to_user_target() {
        let dirs = test_dirs("write-user");
        let (host, handle) = spawn_server(ok_tags(&[]));
        let args = setup_args_with_config(
            Some(host.clone()),
            Some("gemma3:12b"),
            false,
            true,
            false,
            false,
        );

        let output = run_setup_json_in(args, &dirs.project, Some(&dirs.home), &cpu_standard())
            .expect("setup should return json");
        handle.join().expect("server should join");
        let value = parse_json(&output);
        let path = dirs.home.join(".cowork/config.toml");
        let written = fs::read_to_string(&path).expect("user config should read");

        assert_eq!(value["status"], "ok");
        assert_eq!(value["actions"][3]["name"], "config_write");
        assert_eq!(value["actions"][3]["status"], "ok");
        assert_eq!(value["actions"][3]["path"], path.display().to_string());
        assert_eq!(value["config"]["target"], "user");
        assert_eq!(value["config"]["write_requested"], true);
        assert_eq!(
            written,
            format!("[ask]\nmodel = \"gemma3:12b\"\nhost = \"{host}\"\n")
        );
    }

    #[test]
    fn write_config_project_target_preserves_unrelated_toml() {
        let dirs = test_dirs("write-project");
        let (host, handle) = spawn_server(ok_tags(&[]));
        let path = dirs.project.join("cowork.toml");
        write_config(&path, "title = \"keep\"\n\n[other]\nenabled = true\n");
        let args = setup_args_with_config(
            Some(host.clone()),
            Some("gemma3:12b"),
            false,
            true,
            true,
            false,
        );

        let output = run_setup_json_in(args, &dirs.project, Some(&dirs.home), &cpu_standard())
            .expect("setup should return json");
        handle.join().expect("server should join");
        let value = parse_json(&output);
        let written = fs::read_to_string(&path).expect("project config should read");

        assert_eq!(value["actions"][3]["status"], "ok");
        assert_eq!(value["config"]["target"], "project");
        assert!(written.starts_with("title = \"keep\"\n\n[other]\nenabled = true\n\n"));
        assert!(written.ends_with(&format!(
            "[ask]\nmodel = \"gemma3:12b\"\nhost = \"{host}\"\n"
        )));
    }

    #[test]
    fn config_conflict_reports_error_without_mutation() {
        let dirs = test_dirs("write-conflict");
        let (host, handle) = spawn_server_sequence(vec![
            ok_tags(&["new-model"]),
            ok_probe_response(r#"{"ok":true}"#),
        ]);
        let path = dirs.project.join("cowork.toml");
        let original = "[ask]\nmodel = \"old-model\"\nhost = \"http://old\"\n";
        write_config(&path, original);
        let args = setup_args_with_config(Some(host), Some("new-model"), false, true, true, false);

        let output = run_setup_json_in(args, &dirs.project, Some(&dirs.home), &cpu_standard())
            .expect("setup should return json");
        let requests = handle.join().expect("server should join");
        let value = parse_json(&output);

        assert_eq!(requests.len(), 2);
        assert_eq!(value["status"], "error");
        assert_eq!(value["actions"][3]["status"], "error");
        assert_eq!(
            value["actions"][3]["message"],
            "Could not write config: existing `[ask].model` differs; use `--force` to replace it."
        );
        assert_eq!(
            fs::read_to_string(path).expect("config should read"),
            original
        );
        assert_eq!(value["actions"][4]["name"], "model_probe");
        assert_eq!(value["actions"][4]["status"], "ok");
    }

    #[test]
    fn force_replaces_project_ask_values() {
        let dirs = test_dirs("write-force");
        let (host, handle) = spawn_server(ok_tags(&[]));
        let path = dirs.project.join("cowork.toml");
        write_config(
            &path,
            "title = \"keep\"\n\n[ask]\nmodel = \"old\"\nhost = \"http://old\"\n",
        );
        let args = setup_args_with_config(
            Some(host.clone()),
            Some("new-model"),
            false,
            true,
            true,
            true,
        );

        let output = run_setup_json_in(args, &dirs.project, Some(&dirs.home), &cpu_standard())
            .expect("setup should return json");
        handle.join().expect("server should join");
        let value = parse_json(&output);
        let written = fs::read_to_string(path).expect("config should read");

        assert_eq!(value["status"], "ok");
        assert_eq!(value["actions"][3]["status"], "ok");
        assert_eq!(value["config"]["force"], true);
        assert_eq!(
            written,
            format!("title = \"keep\"\n\n[ask]\nmodel = \"new-model\"\nhost = \"{host}\"\n")
        );
    }

    #[test]
    fn equal_project_ask_values_skip_rewrite() {
        let dirs = test_dirs("write-equal");
        let (host, handle) = spawn_server_sequence(vec![
            ok_tags(&["gemma3:12b"]),
            ok_probe_response(r#"{"ok":true}"#),
        ]);
        let path = dirs.project.join("cowork.toml");
        let original = format!("[ask]\nmodel = \"gemma3:12b\"\nhost = \"{host}\"\n");
        write_config(&path, &original);
        let args = setup_args_with_config(None, None, false, true, true, false);

        let output = run_setup_json_in(args, &dirs.project, Some(&dirs.home), &cpu_standard())
            .expect("setup should return json");
        handle.join().expect("server should join");
        let value = parse_json(&output);

        assert_eq!(value["actions"][3]["status"], "skipped");
        assert_eq!(
            value["actions"][3]["message"],
            "Config already contains chosen model and host."
        );
        assert_eq!(
            fs::read_to_string(path).expect("config should read"),
            original
        );
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
        setup_args_with(host, None, false)
    }

    fn setup_args_with(host: Option<String>, model: Option<&str>, pull: bool) -> SetupArgs {
        setup_args_with_config(host, model, pull, false, false, false)
    }

    fn setup_args_with_config(
        host: Option<String>,
        model: Option<&str>,
        pull: bool,
        write_config: bool,
        project: bool,
        force: bool,
    ) -> SetupArgs {
        SetupArgs {
            model: model.map(str::to_string),
            host,
            pull,
            write_config,
            user: false,
            project,
            force,
        }
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

    fn request_body(request: &[u8]) -> &[u8] {
        let body_start = find_bytes(request, b"\r\n\r\n").expect("request should have body");

        &request[body_start + 4..]
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

    fn ok_tags(models: &[&str]) -> String {
        let models = models
            .iter()
            .map(|name| json!({ "name": name }))
            .collect::<Vec<_>>();

        ok_response(&json!({ "models": models }).to_string())
    }

    fn ok_probe_response(model_output: &str) -> String {
        ok_response(&json!({ "response": model_output }).to_string())
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
