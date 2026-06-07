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
mod tests {
    #![allow(clippy::missing_errors_doc, reason = "test helpers stay local")]
    #![allow(clippy::missing_panics_doc, reason = "test asserts and fixtures")]

    use clap::{CommandFactory, Parser};

    use super::{Cli, Command, InitAgent};

    fn command_help(name: &str) -> String {
        let mut command = Cli::command();
        let subcommand = command
            .find_subcommand_mut(name)
            .expect("subcommand should exist");
        let mut help = Vec::new();

        subcommand
            .write_long_help(&mut help)
            .expect("help should render");

        String::from_utf8(help).expect("help should be utf-8")
    }

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
                assert!(!args.fail_on_missing);
                assert!(!args.no_fail_on_missing);
            }
            Command::Brief(_) => panic!("expected ask command"),
            Command::Locate(_) => panic!("expected ask command"),
            Command::Doctor(_) => panic!("expected ask command"),
            Command::Init(_) => panic!("expected ask command"),
        }
    }

    #[test]
    fn ask_help_shows_missing_path_flags() {
        let help = command_help("ask");

        assert!(help.contains("--paths <PATHS>..."));
        assert!(help.contains("--question <QUESTION>"));
        assert!(help.contains("--fail-on-missing"));
        assert!(help.contains("--no-fail-on-missing"));
    }

    #[test]
    fn brief_parses_required_flags() {
        let cli = Cli::try_parse_from([
            "cowork",
            "brief",
            "--paths",
            "src",
            "Cargo.toml",
            "--goal",
            "trace CLI flow",
        ])
        .expect("brief args should parse");

        match cli.command {
            Command::Brief(args) => {
                assert_eq!(args.paths.len(), 2);
                assert_eq!(args.goal, "trace CLI flow");
                assert!(!args.fail_on_missing);
                assert!(!args.no_fail_on_missing);
            }
            Command::Ask(_) => panic!("expected brief command"),
            Command::Locate(_) => panic!("expected brief command"),
            Command::Doctor(_) => panic!("expected brief command"),
            Command::Init(_) => panic!("expected brief command"),
        }
    }

    #[test]
    fn brief_help_shows_missing_path_flags() {
        let help = command_help("brief");

        assert!(help.contains("--paths <PATHS>..."));
        assert!(help.contains("--goal <GOAL>"));
        assert!(help.contains("--fail-on-missing"));
        assert!(help.contains("--no-fail-on-missing"));
    }

    #[test]
    fn locate_parses_required_flags() {
        let cli = Cli::try_parse_from([
            "cowork",
            "locate",
            "--paths",
            "src",
            "Cargo.toml",
            "--thing",
            "CLI parser",
        ])
        .expect("locate args should parse");

        match cli.command {
            Command::Locate(args) => {
                assert_eq!(args.paths.len(), 2);
                assert_eq!(args.thing, "CLI parser");
                assert!(!args.fail_on_missing);
                assert!(!args.no_fail_on_missing);
            }
            Command::Ask(_) => panic!("expected locate command"),
            Command::Brief(_) => panic!("expected locate command"),
            Command::Doctor(_) => panic!("expected locate command"),
            Command::Init(_) => panic!("expected locate command"),
        }
    }

    #[test]
    fn locate_help_shows_missing_path_flags() {
        let help = command_help("locate");

        assert!(help.contains("--paths <PATHS>..."));
        assert!(help.contains("--thing <THING>"));
        assert!(help.contains("--fail-on-missing"));
        assert!(help.contains("--no-fail-on-missing"));
    }

    #[test]
    fn ask_parses_explicit_missing_path_flags() {
        let cli = Cli::try_parse_from([
            "cowork",
            "ask",
            "--paths",
            "src",
            "--question",
            "Where is CLI parsing defined?",
            "--fail-on-missing",
        ])
        .expect("ask args should parse");

        match cli.command {
            Command::Ask(args) => {
                assert!(args.fail_on_missing);
                assert!(!args.no_fail_on_missing);
            }
            _ => panic!("expected ask command"),
        }
    }

    #[test]
    fn ask_parses_missing_path_opt_out() {
        let cli = Cli::try_parse_from([
            "cowork",
            "ask",
            "--paths",
            "src",
            "--question",
            "Where is CLI parsing defined?",
            "--no-fail-on-missing",
        ])
        .expect("ask args should parse");

        match cli.command {
            Command::Ask(args) => {
                assert!(!args.fail_on_missing);
                assert!(args.no_fail_on_missing);
            }
            _ => panic!("expected ask command"),
        }
    }

    #[test]
    fn brief_parses_explicit_missing_path_flags() {
        let cli = Cli::try_parse_from([
            "cowork",
            "brief",
            "--paths",
            "src",
            "--goal",
            "trace CLI flow",
            "--fail-on-missing",
        ])
        .expect("brief args should parse");

        match cli.command {
            Command::Brief(args) => {
                assert!(args.fail_on_missing);
                assert!(!args.no_fail_on_missing);
            }
            _ => panic!("expected brief command"),
        }
    }

    #[test]
    fn brief_parses_missing_path_opt_out() {
        let cli = Cli::try_parse_from([
            "cowork",
            "brief",
            "--paths",
            "src",
            "--goal",
            "trace CLI flow",
            "--no-fail-on-missing",
        ])
        .expect("brief args should parse");

        match cli.command {
            Command::Brief(args) => {
                assert!(!args.fail_on_missing);
                assert!(args.no_fail_on_missing);
            }
            _ => panic!("expected brief command"),
        }
    }

    #[test]
    fn locate_parses_explicit_missing_path_flags() {
        let cli = Cli::try_parse_from([
            "cowork",
            "locate",
            "--paths",
            "src",
            "--thing",
            "CLI parser",
            "--fail-on-missing",
        ])
        .expect("locate args should parse");

        match cli.command {
            Command::Locate(args) => {
                assert!(args.fail_on_missing);
                assert!(!args.no_fail_on_missing);
            }
            _ => panic!("expected locate command"),
        }
    }

    #[test]
    fn locate_parses_missing_path_opt_out() {
        let cli = Cli::try_parse_from([
            "cowork",
            "locate",
            "--paths",
            "src",
            "--thing",
            "CLI parser",
            "--no-fail-on-missing",
        ])
        .expect("locate args should parse");

        match cli.command {
            Command::Locate(args) => {
                assert!(!args.fail_on_missing);
                assert!(args.no_fail_on_missing);
            }
            _ => panic!("expected locate command"),
        }
    }

    #[test]
    fn conflicting_missing_path_flags_fail_parse() {
        let error = Cli::try_parse_from([
            "cowork",
            "ask",
            "--paths",
            "src",
            "--question",
            "Where is CLI parsing defined?",
            "--fail-on-missing",
            "--no-fail-on-missing",
        ])
        .expect_err("conflicting flags should fail");

        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn brief_conflicting_missing_path_flags_fail_parse() {
        let error = Cli::try_parse_from([
            "cowork",
            "brief",
            "--paths",
            "src",
            "--goal",
            "trace CLI flow",
            "--fail-on-missing",
            "--no-fail-on-missing",
        ])
        .expect_err("conflicting flags should fail");

        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn locate_conflicting_missing_path_flags_fail_parse() {
        let error = Cli::try_parse_from([
            "cowork",
            "locate",
            "--paths",
            "src",
            "--thing",
            "CLI parser",
            "--fail-on-missing",
            "--no-fail-on-missing",
        ])
        .expect_err("conflicting flags should fail");

        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn doctor_parses_optional_flags() {
        let cli = Cli::try_parse_from([
            "cowork",
            "doctor",
            "--model",
            "gemma3:12b",
            "--host",
            "http://localhost:11434",
        ])
        .expect("doctor args should parse");

        match cli.command {
            Command::Doctor(args) => {
                assert_eq!(args.model.as_deref(), Some("gemma3:12b"));
                assert_eq!(args.host.as_deref(), Some("http://localhost:11434"));
            }
            Command::Ask(_) => panic!("expected doctor command"),
            Command::Brief(_) => panic!("expected doctor command"),
            Command::Locate(_) => panic!("expected doctor command"),
            Command::Init(_) => panic!("expected doctor command"),
        }
    }

    #[test]
    fn doctor_help_shows_optional_flags() {
        let mut command = Cli::command();
        let doctor = command
            .find_subcommand_mut("doctor")
            .expect("doctor subcommand should exist");
        let mut help = Vec::new();

        doctor
            .write_long_help(&mut help)
            .expect("doctor help should render");

        let help = String::from_utf8(help).expect("help should be utf-8");

        assert!(help.contains("--model <MODEL>"));
        assert!(help.contains("--host <HOST>"));
    }

    #[test]
    fn init_codex_parses_print_flag() {
        let cli = Cli::try_parse_from(["cowork", "init", "codex", "--print"])
            .expect("init args should parse");

        match cli.command {
            Command::Init(args) => match args.agent {
                InitAgent::Codex(mode) => {
                    assert!(mode.print);
                    assert!(!mode.write);
                }
                InitAgent::Claude(_) => panic!("expected codex init target"),
            },
            _ => panic!("expected init command"),
        }
    }

    #[test]
    fn init_codex_parses_write_flag() {
        let cli = Cli::try_parse_from(["cowork", "init", "codex", "--write"])
            .expect("init args should parse");

        match cli.command {
            Command::Init(args) => match args.agent {
                InitAgent::Codex(mode) => {
                    assert!(!mode.print);
                    assert!(mode.write);
                }
                InitAgent::Claude(_) => panic!("expected codex init target"),
            },
            _ => panic!("expected init command"),
        }
    }

    #[test]
    fn init_help_shows_agent_subcommands() {
        let mut command = Cli::command();
        let init = command
            .find_subcommand_mut("init")
            .expect("init subcommand should exist");
        let mut help = Vec::new();

        init.write_long_help(&mut help)
            .expect("init help should render");

        let help = String::from_utf8(help).expect("help should be utf-8");

        assert!(help.contains("codex"));
        assert!(help.contains("claude"));
    }

    #[test]
    fn init_codex_help_shows_mode_flags() {
        let mut command = Cli::command();
        let init = command
            .find_subcommand_mut("init")
            .expect("init subcommand should exist");
        let codex = init
            .find_subcommand_mut("codex")
            .expect("codex init target should exist");
        let mut help = Vec::new();

        codex
            .write_long_help(&mut help)
            .expect("codex init help should render");

        let help = String::from_utf8(help).expect("help should be utf-8");

        assert!(help.contains("--print"));
        assert!(help.contains("--write"));
    }
}
