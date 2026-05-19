//! `cowork` CLI library.
//!
//! Thin entry layer for CLI parse and exit flow.

/// CLI types.
pub mod cli;
mod error;
mod files;
mod model;
mod output;
mod prompt;

use std::process::ExitCode;

use clap::{Parser, error::ErrorKind};

pub use cli::{AskArgs, Cli, Command};
use error::AppError;
use files::{collect_ask_candidates, load_ask_files};
use prompt::render_ask_prompt;

/// Run `cowork`.
#[must_use]
pub fn run() -> ExitCode {
    match try_run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            println!("{}", error.to_json());
            error.exit_code()
        }
    }
}

fn try_run() -> Result<(), AppError> {
    let cli = parse_cli()?;

    match cli.command {
        Command::Ask(args) => run_ask(args),
    }
}

fn parse_cli() -> Result<Cli, AppError> {
    Cli::try_parse().map_err(|error| match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => error.exit(),
        _ => AppError::invalid_arguments(error.to_string()),
    })
}

fn run_ask(args: AskArgs) -> Result<(), AppError> {
    let candidate_paths =
        collect_ask_candidates(&args.paths, args.recursive, &args.include, &args.exclude)?;
    let loaded_files = load_ask_files(&candidate_paths, args.max_bytes)?;
    let _prompt = render_ask_prompt(&args.question, &loaded_files);
    let _ = model::request_generate;
    let _ = output::parse_ask_output;
    let _ = output::AskOutput::to_json;

    Err(AppError::AskNotImplemented)
}
