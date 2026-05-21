use std::{fs, io::ErrorKind as IoErrorKind, path::Path, process::ExitCode};

use crate::{
    InitAgent, InitArgs,
    error::AppError,
    output::{init_target_file, render_init_rules, update_init_managed_block},
};

pub(super) fn run_init(args: InitArgs) -> Result<ExitCode, AppError> {
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
