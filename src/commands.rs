use std::process::ExitCode;

use clap::{Parser, error::ErrorKind};

use crate::{Cli, Command, error::AppError};

mod ask;
mod doctor;
mod init;
mod locate;

#[cfg(test)]
pub(crate) use ask::run_ask_json_in;

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
        Command::Ask(args) => ask::run_ask(args),
        Command::Locate(args) => locate::run_locate(args),
        Command::Doctor(args) => doctor::run_doctor(args),
        Command::Init(args) => init::run_init(args),
    }
}

fn parse_cli() -> Result<Cli, AppError> {
    Cli::try_parse().map_err(|error| match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => error.exit(),
        _ => AppError::invalid_arguments(error.to_string()),
    })
}
