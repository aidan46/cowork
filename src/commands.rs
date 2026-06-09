use std::process::ExitCode;

use clap::{Parser, error::ErrorKind};

use crate::{Cli, Command, error::AppError, output::CommandId};

/// Ask command run flow.
mod ask;
/// Brief command run flow.
mod brief;
/// Doctor command run flow.
mod doctor;
/// Init command run flow.
mod init;
/// Locate command run flow.
mod locate;
/// Setup command run flow.
mod setup;

#[cfg(test)]
pub(crate) use ask::run_ask_json_in;
#[cfg(test)]
pub(crate) use brief::run_brief_json_in;

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

/// Dispatch parsed subcommand.
///
/// # Errors
///
/// Returns [`AppError`] when CLI parsing fails or a subcommand returns one.
fn try_run() -> Result<ExitCode, AppError> {
    let cli = parse_cli()?;

    match cli.command {
        Command::Ask(args) => {
            ask::run_ask(args).map_err(|error| error.with_command(CommandId::Ask))
        }
        Command::Brief(args) => {
            brief::run_brief(args).map_err(|error| error.with_command(CommandId::Brief))
        }
        Command::Locate(args) => {
            locate::run_locate(args).map_err(|error| error.with_command(CommandId::Locate))
        }
        Command::Doctor(args) => {
            doctor::run_doctor(args).map_err(|error| error.with_command(CommandId::Doctor))
        }
        Command::Setup(args) => {
            setup::run_setup(args).map_err(|error| error.with_command(CommandId::Setup))
        }
        Command::Init(args) => {
            init::run_init(args).map_err(|error| error.with_command(CommandId::Init))
        }
    }
}

/// Parse CLI args or map parse errors.
///
/// # Errors
///
/// Returns [`AppError`] when CLI args are invalid.
fn parse_cli() -> Result<Cli, AppError> {
    Cli::try_parse().map_err(|error| match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => error.exit(),
        _ => AppError::invalid_arguments(error.to_string()),
    })
}
