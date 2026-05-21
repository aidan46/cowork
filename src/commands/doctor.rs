use std::{env, path::Path, process::ExitCode};

use crate::{
    DoctorArgs,
    config::resolve_ask_config,
    error::{AppError, DoctorExit},
    model::{self, DoctorProbeErrorKind},
    output::{DoctorCheck, DoctorOutput},
};

/// Run `doctor` and print JSON.
pub(super) fn run_doctor(args: DoctorArgs) -> Result<ExitCode, AppError> {
    let result = run_doctor_json(args)?;
    println!("{}", result.json);

    Ok(result.exit_code)
}

/// Run `doctor` in cwd and return JSON result.
fn run_doctor_json(args: DoctorArgs) -> Result<DoctorRunResult, AppError> {
    let project_dir = env::current_dir().map_err(|error| {
        AppError::invalid_arguments(format!("failed to resolve current dir: {error}"))
    })?;
    let home_dir = env::var_os("HOME").map(std::path::PathBuf::from);

    Ok(run_doctor_json_in(args, &project_dir, home_dir.as_deref()))
}

/// Run `doctor` with explicit dirs.
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

/// Map probe error into checks and exit.
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
    fn ok(checks: Vec<DoctorCheck>) -> Self {
        Self {
            json: DoctorOutput::ok(checks).to_json(),
            exit_code: DoctorExit::Ok.exit_code(),
        }
    }

    /// Build error result.
    fn error(checks: Vec<DoctorCheck>, exit: DoctorExit) -> Self {
        Self {
            json: DoctorOutput::error(checks).to_json(),
            exit_code: exit.exit_code(),
        }
    }
}
