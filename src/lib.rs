//! `cowork` CLI library.
//!
//! Thin entry layer for CLI parse and exit flow.

/// CLI types.
pub mod cli;

use std::process::ExitCode;

use clap::Parser;
use serde::Serialize;
use thiserror::Error;

pub use cli::{AskArgs, Cli, Command};

#[derive(Debug, Error)]
enum AppError {
    #[error("`cowork ask` not implemented yet")]
    AskNotImplemented,
}

#[derive(Debug, Serialize)]
struct ErrorResponse<'a> {
    schema_version: &'a str,
    command: &'a str,
    status: &'a str,
    error: ErrorBody<'a>,
}

#[derive(Debug, Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: String,
}

/// Run `cowork`.
#[must_use]
pub fn run() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Command::Ask(args) => run_ask(args),
    }
}

fn run_ask(_args: AskArgs) -> ExitCode {
    let response = ErrorResponse {
        schema_version: "1.0",
        command: "ask",
        status: "error",
        error: ErrorBody {
            code: "NOT_IMPLEMENTED",
            message: AppError::AskNotImplemented.to_string(),
        },
    };

    println!(
        "{}",
        serde_json::to_string(&response).expect("error response should serialize")
    );

    ExitCode::from(1)
}
