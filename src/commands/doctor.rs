use std::{env, path::Path, process::ExitCode};

use crate::{
    DoctorArgs,
    config::resolve_ask_config,
    error::{AppError, DoctorExit},
    model::{self, DoctorProbeErrorKind, OllamaErrorKind},
    output::{DoctorCheck, DoctorOutput},
};

/// Run `doctor` and print JSON.
///
/// # Errors
///
/// Returns [`AppError`] on cwd lookup or JSON output failure.
pub(super) fn run_doctor(args: DoctorArgs) -> Result<ExitCode, AppError> {
    let result = run_doctor_json(args)?;
    println!("{}", result.json);

    Ok(result.exit_code)
}

/// Run `doctor` in cwd and return JSON result.
///
/// # Errors
///
/// Returns [`AppError`] on cwd lookup or JSON output failure.
fn run_doctor_json(args: DoctorArgs) -> Result<DoctorRunResult, AppError> {
    let project_dir = env::current_dir().map_err(|error| {
        AppError::invalid_arguments(format!("failed to resolve current dir: {error}"))
    })?;
    let home_dir = env::var_os("HOME").map(std::path::PathBuf::from);

    run_doctor_json_in(args, &project_dir, home_dir.as_deref())
}

/// Run `doctor` with explicit dirs.
///
/// # Errors
///
/// Returns [`AppError`] when doctor JSON output serialization fails.
fn run_doctor_json_in(
    args: DoctorArgs,
    project_dir: &Path,
    home_dir: Option<&Path>,
) -> Result<DoctorRunResult, AppError> {
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
                    "installed_models_listed",
                    "effective_model_installed",
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
                Some("Run `cowork setup --write-config`."),
            ));
            push_skipped_doctor_checks(
                &mut checks,
                "model missing, network checks skipped.",
                &[
                    "host_url_parsed",
                    "installed_models_listed",
                    "effective_model_installed",
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
                "installed_models_listed",
                "effective_model_installed",
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

    let installed_models = match model::request_ollama_tags(&config.host) {
        Ok(models) => models,
        Err(error) => return doctor_tags_error_result(checks, error),
    };
    checks.push(DoctorCheck::ok(
        "installed_models_listed",
        format!("Listed {} installed Ollama models.", installed_models.len()),
    ));

    if !installed_models
        .iter()
        .any(|installed| installed.name == model)
    {
        checks.push(DoctorCheck::error(
            "effective_model_installed",
            format!("Effective model `{model}` is not installed."),
            Some("Run `cowork setup --pull`."),
        ));
        push_skipped_doctor_checks(
            &mut checks,
            "effective model not installed, probe checks skipped.",
            &[
                "generate_endpoint_reachable",
                "tiny_json_probe_succeeds",
                "probe_output_shape_valid",
            ],
        );

        return DoctorRunResult::error(checks, DoctorExit::ProbeRequestFailed);
    }

    checks.push(DoctorCheck::ok(
        "effective_model_installed",
        format!("Effective model `{model}` is installed."),
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

    if let Err(message) = crate::output::parse_doctor_probe(&raw_probe) {
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

/// Map tags error into checks and exit.
///
/// # Errors
///
/// Returns [`AppError`] when doctor JSON output serialization fails.
fn doctor_tags_error_result(
    mut checks: Vec<DoctorCheck>,
    error: model::OllamaError,
) -> Result<DoctorRunResult, AppError> {
    let (tag, hint, exit) = match error.kind {
        OllamaErrorKind::BadHost => (
            "bad_host",
            "Pass `--host http://localhost:11434` or fix `[ask].host`.",
            DoctorExit::BadHost,
        ),
        OllamaErrorKind::UnreachableHost => (
            "unreachable_host",
            "Start Ollama or check host, port, and firewall.",
            DoctorExit::UnreachableHost,
        ),
        OllamaErrorKind::RequestFailed => (
            "request_failed",
            "Check Ollama `/api/tags` and host configuration.",
            DoctorExit::ProbeRequestFailed,
        ),
        OllamaErrorKind::InvalidJson => (
            "invalid_json",
            "Ollama `/api/tags` must return valid JSON.",
            DoctorExit::InvalidProbeJson,
        ),
    };

    checks.push(DoctorCheck::error(
        "installed_models_listed",
        format!("Could not list installed Ollama models: {tag}."),
        Some(hint),
    ));
    push_skipped_doctor_checks(
        &mut checks,
        "model listing failed, later checks skipped.",
        &[
            "effective_model_installed",
            "generate_endpoint_reachable",
            "tiny_json_probe_succeeds",
            "probe_output_shape_valid",
        ],
    );

    DoctorRunResult::error(checks, exit)
}

/// Map probe error into checks and exit.
///
/// # Errors
///
/// Returns [`AppError`] when doctor JSON output serialization fails.
fn doctor_probe_error_result(
    mut checks: Vec<DoctorCheck>,
    error: model::DoctorProbeError,
) -> Result<DoctorRunResult, AppError> {
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

/// Push skipped doctor checks.
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

/// Doctor JSON plus exit code.
struct DoctorRunResult {
    /// Serialized JSON output.
    json: String,
    /// Exit code to return.
    exit_code: ExitCode,
}

impl DoctorRunResult {
    /// Build success result.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] when JSON serialization fails.
    fn ok(checks: Vec<DoctorCheck>) -> Result<Self, AppError> {
        Ok(Self {
            json: DoctorOutput::ok(checks).to_json()?,
            exit_code: DoctorExit::Ok.exit_code(),
        })
    }

    /// Build error result.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] when JSON serialization fails.
    fn error(checks: Vec<DoctorCheck>, exit: DoctorExit) -> Result<Self, AppError> {
        Ok(Self {
            json: DoctorOutput::error(checks).to_json()?,
            exit_code: exit.exit_code(),
        })
    }
}
