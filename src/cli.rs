use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueHint};

#[derive(Debug, Parser)]
#[command(name = "cowork")]
#[command(about = "Local AI coworker CLI for coding agents", long_about = None)]
#[command(subcommand_required = true, arg_required_else_help = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(arg_required_else_help = true)]
    Ask(AskArgs),
}

#[derive(Debug, Args)]
pub struct AskArgs {
    #[arg(long, required = true, num_args = 1.., value_name = "PATHS", value_hint = ValueHint::AnyPath)]
    pub paths: Vec<PathBuf>,

    #[arg(long, required = true, value_name = "QUESTION")]
    pub question: String,

    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,

    #[arg(long, value_name = "HOST")]
    pub host: Option<String>,

    #[arg(long, value_name = "BYTES")]
    pub max_bytes: Option<usize>,

    #[arg(long)]
    pub recursive: bool,

    #[arg(long, value_name = "GLOB")]
    pub include: Vec<String>,

    #[arg(long, value_name = "GLOB")]
    pub exclude: Vec<String>,

    #[arg(long)]
    pub fail_on_missing: bool,
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::{Cli, Command};

    #[test]
    fn ask_parses_required_flags() {
        let cli = Cli::try_parse_from([
            "cowork",
            "ask",
            "--paths",
            "src",
            "Cargo.toml",
            "--question",
            "Where is CLI parsing defined?",
        ])
        .expect("ask args should parse");

        match cli.command {
            Command::Ask(args) => {
                assert_eq!(args.paths.len(), 2);
                assert_eq!(args.question, "Where is CLI parsing defined?");
            }
        }
    }

    #[test]
    fn ask_help_shows_required_flags() {
        let mut command = Cli::command();
        let ask = command
            .find_subcommand_mut("ask")
            .expect("ask subcommand should exist");
        let mut help = Vec::new();

        ask.write_long_help(&mut help)
            .expect("ask help should render");

        let help = String::from_utf8(help).expect("help should be utf-8");

        assert!(help.contains("--paths <PATHS>..."));
        assert!(help.contains("--question <QUESTION>"));
    }
}
