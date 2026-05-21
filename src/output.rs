mod ask;
mod doctor;
mod init;
mod locate;

const SCHEMA_VERSION: &str = "1.0";
const ASK_COMMAND: &str = "ask";
const CLI_COMMAND: &str = "cli";
const DOCTOR_COMMAND: &str = "doctor";
const INIT_COMMAND: &str = "init";
const LOCATE_COMMAND: &str = "locate";
const STATUS_OK: &str = "ok";
const STATUS_ERROR: &str = "error";
const CODEX_INIT_RULES: &str = r#"# cowork rules for Codex

- Use `cowork ask` before reading more than 3 files or any large file.
- Ask narrow questions, example: `cowork ask --paths src/cli.rs src/lib.rs --question "How does init print flow work?"`
- Ask narrow questions, example: `cowork ask --paths src/output.rs --question "Where are init rules defined?"`
- Do not use `cowork ask` for whole-repo summaries, final authority, or write plans without code evidence.
- If setup fails, run `cowork doctor`.
- Inspect `next_reads` yourself before acting.
- Treat local model output as lead, not authority.
"#;
const CLAUDE_INIT_RULES: &str = r#"# cowork rules for Claude

- Use `cowork ask` before reading more than 3 files or any large file.
- Ask narrow questions, example: `cowork ask --paths src/cli.rs src/lib.rs --question "How does init print flow work?"`
- Ask narrow questions, example: `cowork ask --paths src/output.rs --question "Where are init rules defined?"`
- Do not use `cowork ask` for whole-repo summaries, final authority, or write plans without code evidence.
- If setup fails, run `cowork doctor`.
- Inspect `next_reads` yourself before acting.
- Treat local model output as lead, not authority.
"#;

pub(crate) use ask::parse_ask_output;
pub(crate) use doctor::{DoctorCheck, DoctorOutput, parse_doctor_probe};
pub(crate) use init::{init_target_file, render_init_rules, update_init_managed_block};
pub(crate) use locate::parse_locate_output;

#[must_use]
pub(crate) fn cli_command() -> &'static str {
    CLI_COMMAND
}

#[must_use]
pub(crate) fn init_command() -> &'static str {
    INIT_COMMAND
}
