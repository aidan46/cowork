use std::{env, path::Path, process::ExitCode, time::Instant};

use crate::{
    AskArgs,
    config::resolve_ask_config,
    error::AppError,
    files::{collect_ask_candidates, load_ask_files},
    model, output,
    prompt::render_ask_prompt,
};

/// Run `ask` and print JSON.
///
/// # Errors
///
/// Returns [`AppError`] on file, config, model, or JSON output failure.
pub(super) fn run_ask(args: AskArgs) -> Result<ExitCode, AppError> {
    let ask_json = run_ask_json(args)?;
    println!("{ask_json}");

    Ok(ExitCode::SUCCESS)
}

/// Run `ask` in cwd and return JSON.
///
/// # Errors
///
/// Returns [`AppError`] on cwd lookup, file load, config, model, or JSON output failure.
fn run_ask_json(args: AskArgs) -> Result<String, AppError> {
    let project_dir = env::current_dir().map_err(|error| {
        AppError::invalid_arguments(format!("failed to resolve current dir: {error}"))
    })?;
    let home_dir = env::var_os("HOME").map(std::path::PathBuf::from);

    run_ask_json_in(args, &project_dir, home_dir.as_deref())
}

/// Run `ask` with explicit dirs.
///
/// # Errors
///
/// Returns [`AppError`] on file load, config, model, parse, or JSON output failure.
pub(crate) fn run_ask_json_in(
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
        fail_on_missing,
        no_fail_on_missing,
    } = args;
    let fail_on_missing = fail_on_missing || !no_fail_on_missing;
    let candidate_paths =
        collect_ask_candidates(&paths, recursive, &include, &exclude, fail_on_missing)?;
    let loaded_files = load_ask_files(&candidate_paths, max_bytes)?;
    let prompt = render_ask_prompt(&question, &loaded_files);
    let config = resolve_ask_config(project_dir, home_dir, model, host)?;
    let model = config.model.as_deref().ok_or_else(|| {
        AppError::invalid_arguments("`--model` required, no default model configured yet")
    })?;
    let started = Instant::now();
    let raw_output = model::request_generate(&config.host, model, &prompt)?;
    let duration_ms = usize::try_from(started.elapsed().as_millis()).unwrap_or(usize::MAX);
    let metadata = output::CommandMetadata::new(loaded_files.total_bytes, duration_ms);
    let output = output::parse_ask_output(&raw_output, metadata)?;

    output.into_json()
}
