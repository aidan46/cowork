use serde::Serialize;

/// Ask output parse.
mod ask;
/// Doctor output parse.
mod doctor;
/// Init text render.
mod init;
/// Locate output parse.
mod locate;

/// Shared schema version.
const SCHEMA_VERSION: &str = "1.0";
/// `ask` command tag.
const ASK_COMMAND: &str = "ask";
/// CLI command tag.
const CLI_COMMAND: &str = "cli";
/// `doctor` command tag.
const DOCTOR_COMMAND: &str = "doctor";
/// `init` command tag.
const INIT_COMMAND: &str = "init";
/// `locate` command tag.
const LOCATE_COMMAND: &str = "locate";
/// Success status tag.
const STATUS_OK: &str = "ok";
/// Error status tag.
const STATUS_ERROR: &str = "error";

#[derive(Debug, Serialize, PartialEq, Eq)]
/// CLI-owned command metadata.
pub(crate) struct CommandMetadata {
    /// Input byte count.
    input_bytes: usize,
    /// Final JSON byte count.
    output_bytes: usize,
    /// Model time in ms.
    duration_ms: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Input to output ratio.
    compression_ratio: Option<String>,
}

impl CommandMetadata {
    /// Build metadata before final JSON size known.
    pub(crate) fn new(input_bytes: usize, duration_ms: usize) -> Self {
        Self {
            input_bytes,
            output_bytes: 0,
            duration_ms,
            compression_ratio: None,
        }
    }

    /// Update final JSON byte count.
    fn set_output_bytes(&mut self, output_bytes: usize) {
        self.output_bytes = output_bytes;
        self.compression_ratio = (output_bytes > 0 && self.input_bytes > output_bytes)
            .then(|| format!("{:.2}", self.input_bytes as f64 / output_bytes as f64));
    }
}
/// Codex init rules block.
const CODEX_INIT_RULES: &str = r#"# cowork rules for Codex

- Use `cowork ask` before reading more than 3 files or any large file.
- Ask narrow questions, example: `cowork ask --paths src/cli.rs src/lib.rs --question "How does init print flow work?"`
- Ask narrow questions, example: `cowork ask --paths src/output.rs --question "Where are init rules defined?"`
- Do not use `cowork ask` for whole-repo summaries, final authority, or write plans without code evidence.
- If setup fails, run `cowork doctor`.
- Inspect `next_reads` yourself before acting.
- Treat local model output as lead, not authority.
"#;
/// Claude init rules block.
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
/// Return CLI command tag.
pub(crate) fn cli_command() -> &'static str {
    CLI_COMMAND
}

#[must_use]
/// Return init command tag.
pub(crate) fn init_command() -> &'static str {
    INIT_COMMAND
}
