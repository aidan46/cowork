use serde::{Serialize, Serializer};

/// Ask output parse.
mod ask;
/// Shared output bounds.
mod bounds;
/// Brief output parse.
mod brief;
/// Doctor output parse.
mod doctor;
/// Init text render.
mod init;
/// Locate output parse.
mod locate;
/// Setup output parse.
mod setup;

/// Shared schema version.
const SCHEMA_VERSION: &str = "1.0";
/// Success status tag.
const STATUS_OK: &str = "ok";
/// Error status tag.
const STATUS_ERROR: &str = "error";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Closed command identifier.
pub(crate) enum CommandId {
    /// CLI parser.
    Cli,
    /// Ask command.
    Ask,
    /// Brief command.
    Brief,
    /// Locate command.
    Locate,
    /// Doctor command.
    Doctor,
    /// Setup command.
    Setup,
    /// Init command.
    Init,
}

impl CommandId {
    #[must_use]
    /// Return stable command tag.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::Ask => "ask",
            Self::Brief => "brief",
            Self::Locate => "locate",
            Self::Doctor => "doctor",
            Self::Setup => "setup",
            Self::Init => "init",
        }
    }
}

impl Serialize for CommandId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

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

- Use `cowork locate` first when unsure which files matter.
- Use `cowork ask` before reading more than 3 files or any large file.
- Ask narrow questions, example: `cowork ask --paths src/output.rs --question \"Where are init rules defined?\"`
- Use `cowork brief` when cloud context would otherwise need raw files.
- Inspect `evidence` and `next_reads` before edits.
- Do not use `cowork ask` for whole-repo summaries, final authority, or write plans without code evidence.
- If setup fails, run `cowork doctor`.
- Treat local model output as lead, not authority.
"#;
/// Claude init rules block.
const CLAUDE_INIT_RULES: &str = r#"# cowork rules for Claude

- Use `cowork locate` first when unsure which files matter.
- Use `cowork ask` before reading more than 3 files or any large file.
- Ask narrow questions, example: `cowork ask --paths src/output.rs --question \"Where are init rules defined?\"`
- Use `cowork brief` when cloud context would otherwise need raw files.
- Inspect `evidence` and `next_reads` before edits.
- Do not use `cowork ask` for whole-repo summaries, final authority, or write plans without code evidence.
- If setup fails, run `cowork doctor`.
- Treat local model output as lead, not authority.
"#;

pub(crate) use ask::parse_ask_output;
pub(crate) use brief::parse_brief_output;
pub(crate) use doctor::{DoctorCheck, DoctorOutput, parse_doctor_probe};
pub(crate) use init::{init_target_file, render_init_rules, update_init_managed_block};
pub(crate) use locate::parse_locate_output;
pub(crate) use setup::{
    SetupAction, SetupCheck, SetupConfig, SetupMetadata, SetupOutput, SetupRecommendation,
    SetupStatus,
};

#[cfg(test)]
mod tests {
    #![allow(clippy::missing_panics_doc, reason = "test asserts")]

    use super::CommandId;

    #[test]
    fn command_ids_serialize_to_exact_tags() {
        let cases = [
            (CommandId::Cli, "\"cli\""),
            (CommandId::Ask, "\"ask\""),
            (CommandId::Brief, "\"brief\""),
            (CommandId::Locate, "\"locate\""),
            (CommandId::Doctor, "\"doctor\""),
            (CommandId::Setup, "\"setup\""),
            (CommandId::Init, "\"init\""),
        ];

        for (command, expected) in cases {
            assert_eq!(
                serde_json::to_string(&command).expect("command should serialize"),
                expected
            );
        }
    }
}
