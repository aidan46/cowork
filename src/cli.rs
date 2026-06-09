use std::path::PathBuf;

use clap::{ArgGroup, Args, Parser, Subcommand, ValueHint};

/// Parsed `cowork` args.
#[derive(Debug, Parser)]
#[command(name = "cowork")]
#[command(about = "Local AI coworker CLI for coding agents", long_about = None)]
#[command(subcommand_required = true, arg_required_else_help = true)]
pub struct Cli {
    /// Subcommand to run.
    #[command(subcommand)]
    pub command: Command,
}

/// `cowork` subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Ask about file context.
    #[command(arg_required_else_help = true)]
    Ask(AskArgs),
    /// Build compact file brief.
    #[command(arg_required_else_help = true)]
    Brief(BriefArgs),
    /// Locate likely files and symbols.
    #[command(arg_required_else_help = true)]
    Locate(LocateArgs),
    /// Check local setup for `ask`.
    Doctor(DoctorArgs),
    /// Dry-run local setup discovery.
    Setup(SetupArgs),
    /// Print agent install rules.
    #[command(arg_required_else_help = true)]
    Init(InitArgs),
}

/// Args for `cowork ask`.
#[derive(Debug, Args)]
pub struct AskArgs {
    /// Files or dirs to inspect.
    #[arg(long, required = true, num_args = 1.., value_name = "PATHS", value_hint = ValueHint::AnyPath)]
    pub paths: Vec<PathBuf>,

    /// Narrow question to answer.
    #[arg(long, required = true, value_name = "QUESTION")]
    pub question: String,

    /// Model override.
    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,

    /// Host override.
    #[arg(long, value_name = "HOST")]
    pub host: Option<String>,

    /// Max input bytes.
    #[arg(long, value_name = "BYTES")]
    pub max_bytes: Option<usize>,

    /// Recurse into dirs.
    #[arg(long)]
    pub recursive: bool,

    /// Include glob.
    #[arg(long, value_name = "GLOB")]
    pub include: Vec<String>,

    /// Exclude glob.
    #[arg(long, value_name = "GLOB")]
    pub exclude: Vec<String>,

    /// Fail on missing path.
    #[arg(long, conflicts_with = "no_fail_on_missing")]
    pub fail_on_missing: bool,

    /// Skip missing paths.
    #[arg(long, conflicts_with = "fail_on_missing")]
    pub no_fail_on_missing: bool,
}

/// Args for `cowork brief`.
#[derive(Debug, Args)]
pub struct BriefArgs {
    /// Files or dirs to inspect.
    #[arg(long, required = true, num_args = 1.., value_name = "PATHS", value_hint = ValueHint::AnyPath)]
    pub paths: Vec<PathBuf>,

    /// Goal to brief for.
    #[arg(long, required = true, value_name = "GOAL")]
    pub goal: String,

    /// Model override.
    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,

    /// Host override.
    #[arg(long, value_name = "HOST")]
    pub host: Option<String>,

    /// Max input bytes.
    #[arg(long, value_name = "BYTES")]
    pub max_bytes: Option<usize>,

    /// Recurse into dirs.
    #[arg(long)]
    pub recursive: bool,

    /// Include glob.
    #[arg(long, value_name = "GLOB")]
    pub include: Vec<String>,

    /// Exclude glob.
    #[arg(long, value_name = "GLOB")]
    pub exclude: Vec<String>,

    /// Fail on missing path.
    #[arg(long, conflicts_with = "no_fail_on_missing")]
    pub fail_on_missing: bool,

    /// Skip missing paths.
    #[arg(long, conflicts_with = "fail_on_missing")]
    pub no_fail_on_missing: bool,
}

/// Args for `cowork locate`.
#[derive(Debug, Args)]
pub struct LocateArgs {
    /// Files or dirs to inspect.
    #[arg(long, required = true, num_args = 1.., value_name = "PATHS", value_hint = ValueHint::AnyPath)]
    pub paths: Vec<PathBuf>,

    /// Thing to locate.
    #[arg(long, required = true, value_name = "THING")]
    pub thing: String,

    /// Model override.
    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,

    /// Host override.
    #[arg(long, value_name = "HOST")]
    pub host: Option<String>,

    /// Max input bytes.
    #[arg(long, value_name = "BYTES")]
    pub max_bytes: Option<usize>,

    /// Recurse into dirs.
    #[arg(long)]
    pub recursive: bool,

    /// Include glob.
    #[arg(long, value_name = "GLOB")]
    pub include: Vec<String>,

    /// Exclude glob.
    #[arg(long, value_name = "GLOB")]
    pub exclude: Vec<String>,

    /// Fail on missing path.
    #[arg(long, conflicts_with = "no_fail_on_missing")]
    pub fail_on_missing: bool,

    /// Skip missing paths.
    #[arg(long, conflicts_with = "fail_on_missing")]
    pub no_fail_on_missing: bool,
}

/// Args for `cowork doctor`.
#[derive(Debug, Args)]
pub struct DoctorArgs {
    /// Model override.
    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,

    /// Host override.
    #[arg(long, value_name = "HOST")]
    pub host: Option<String>,
}

/// Args for `cowork setup`.
#[derive(Debug, Args)]
pub struct SetupArgs {
    /// Model override.
    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,

    /// Host override.
    #[arg(long, value_name = "HOST")]
    pub host: Option<String>,

    /// Pull chosen model when missing.
    #[arg(long)]
    pub pull: bool,
}

/// Args for `cowork init`.
#[derive(Debug, Args)]
pub struct InitArgs {
    /// Agent target.
    #[command(subcommand)]
    pub agent: InitAgent,
}

/// Agent targets for `cowork init`.
#[derive(Debug, Subcommand)]
pub enum InitAgent {
    /// Print or write rules for Codex.
    Codex(InitModeArgs),
    /// Print or write rules for Claude.
    Claude(InitModeArgs),
}

/// Mode for `cowork init`.
#[derive(Clone, Copy, Debug, Args)]
#[command(group(ArgGroup::new("init_mode").required(true).args(["print", "write"])))]
pub struct InitModeArgs {
    /// Print rules only.
    #[arg(long)]
    pub print: bool,

    /// Write managed block to target file.
    #[arg(long)]
    pub write: bool,
}

#[cfg(test)]
mod tests;
