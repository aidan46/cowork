use serde::{Deserialize, Serialize};

use crate::error::AppError;

use super::{BRIEF_COMMAND, CommandMetadata, SCHEMA_VERSION, STATUS_OK};

#[derive(Debug, Serialize, PartialEq, Eq)]
/// Brief command output.
pub(crate) struct BriefOutput {
    /// JSON schema version.
    schema_version: &'static str,
    /// Command tag.
    command: &'static str,
    /// Output status.
    status: &'static str,
    /// Goal text.
    goal: String,
    /// Brief summary block.
    brief: BriefSummary,
    /// File rows.
    files: Vec<BriefFile>,
    /// Symbol rows.
    symbols: Vec<BriefSymbol>,
    /// Evidence rows.
    evidence: Vec<BriefEvidence>,
    /// Risk rows.
    risks: Vec<BriefRisk>,
    /// Next-read rows.
    next_reads: Vec<BriefNextRead>,
    /// Output metadata.
    metadata: CommandMetadata,
}

impl BriefOutput {
    /// Serialize output to JSON.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] when JSON serialization fails.
    pub(crate) fn into_json(mut self) -> Result<String, AppError> {
        loop {
            let json = serde_json::to_string(&self).map_err(|error| {
                AppError::response_parse_failed(format!(
                    "failed to serialize brief output: {error}"
                ))
            })?;
            let output_bytes = json.len();

            if self.metadata.output_bytes == output_bytes {
                return Ok(json);
            }

            self.metadata.set_output_bytes(output_bytes);
        }
    }
}

#[derive(Debug, Deserialize)]
/// Raw brief output from model.
struct RawBriefOutput {
    /// Brief summary block.
    brief: BriefSummary,
    /// File rows.
    files: Vec<BriefFile>,
    /// Symbol rows.
    symbols: Vec<BriefSymbol>,
    /// Evidence rows.
    evidence: Vec<BriefEvidence>,
    /// Risk rows.
    risks: Vec<BriefRisk>,
    /// Next-read rows.
    next_reads: Vec<BriefNextRead>,
}

impl BriefOutput {
    /// Add fixed fields and normalize order.
    fn from_raw(value: RawBriefOutput, goal: &str, metadata: CommandMetadata) -> Self {
        let mut files = value.files;
        let mut symbols = value.symbols;
        let mut evidence = value.evidence;
        let mut risks = value.risks;
        let mut next_reads = value.next_reads;

        files.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        files.dedup();
        symbols.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        symbols.dedup();
        evidence.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        evidence.dedup();
        risks.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        risks.dedup();
        next_reads.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        next_reads.dedup();

        Self {
            schema_version: SCHEMA_VERSION,
            command: BRIEF_COMMAND,
            status: STATUS_OK,
            goal: goal.to_owned(),
            brief: value.brief,
            files,
            symbols,
            evidence,
            risks,
            next_reads,
            metadata,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
/// Brief summary block.
struct BriefSummary {
    /// Summary text.
    summary: String,
    /// Summary confidence.
    confidence: BriefConfidence,
    /// True when answer missing.
    not_found: bool,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
/// One brief file row.
struct BriefFile {
    /// File path.
    path: String,
    /// Why file matters.
    role: String,
    /// Key file points.
    key_points: Vec<String>,
    /// File byte count.
    bytes: usize,
}

impl BriefFile {
    /// Build stable sort key.
    fn sort_key(&self) -> (&str, &str, &[String], usize) {
        (
            self.path.as_str(),
            self.role.as_str(),
            self.key_points.as_slice(),
            self.bytes,
        )
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
/// One brief symbol row.
struct BriefSymbol {
    /// Symbol name.
    name: String,
    /// Symbol kind.
    kind: BriefSymbolKind,
    /// Symbol file path.
    path: String,
    /// Symbol job.
    responsibility: String,
}

impl BriefSymbol {
    /// Build stable sort key.
    fn sort_key(&self) -> (&str, &str, &str, &str) {
        (
            self.path.as_str(),
            self.name.as_str(),
            self.kind.as_str(),
            self.responsibility.as_str(),
        )
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
/// One brief evidence row.
struct BriefEvidence {
    /// File path.
    path: String,
    /// Related symbol.
    symbol: String,
    /// Evidence note.
    note: String,
}

impl BriefEvidence {
    /// Build stable sort key.
    fn sort_key(&self) -> (&str, &str, &str) {
        (self.path.as_str(), self.symbol.as_str(), self.note.as_str())
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
/// One brief risk row.
struct BriefRisk {
    /// Risk kind.
    kind: BriefRiskKind,
    /// Risk message.
    message: String,
}

impl BriefRisk {
    /// Build stable sort key.
    fn sort_key(&self) -> (&str, &str) {
        (self.kind.as_str(), self.message.as_str())
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
/// One brief next-read row.
struct BriefNextRead {
    /// File path.
    path: String,
    /// Read reason.
    reason: String,
}

impl BriefNextRead {
    /// Build stable sort key.
    fn sort_key(&self) -> (&str, &str) {
        (self.path.as_str(), self.reason.as_str())
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Brief confidence level.
enum BriefConfidence {
    /// High confidence.
    High,
    /// Medium confidence.
    Medium,
    /// Low confidence.
    Low,
    /// Unknown confidence.
    Unknown,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Brief symbol kind.
enum BriefSymbolKind {
    /// Function symbol.
    Function,
    /// Type symbol.
    Type,
    /// Trait symbol.
    Trait,
    /// Impl block.
    Impl,
    /// Module symbol.
    Module,
    /// Constant symbol.
    Constant,
    /// Variable symbol.
    Variable,
    /// Route symbol.
    Route,
    /// Component symbol.
    Component,
    /// Test symbol.
    Test,
    /// Unknown symbol kind.
    Unknown,
}

impl BriefSymbolKind {
    /// Return API string for kind.
    fn as_str(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Type => "type",
            Self::Trait => "trait",
            Self::Impl => "impl",
            Self::Module => "module",
            Self::Constant => "constant",
            Self::Variable => "variable",
            Self::Route => "route",
            Self::Component => "component",
            Self::Test => "test",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Brief risk kind.
enum BriefRiskKind {
    /// Missing context risk.
    MissingContext,
    /// Model uncertainty risk.
    ModelUncertainty,
    /// Parse error risk.
    ParseError,
    /// Skipped file risk.
    SkippedFile,
    /// Unsupported file risk.
    UnsupportedFile,
    /// Unknown risk.
    Unknown,
}

impl BriefRiskKind {
    /// Return API string for risk.
    fn as_str(&self) -> &'static str {
        match self {
            Self::MissingContext => "missing_context",
            Self::ModelUncertainty => "model_uncertainty",
            Self::ParseError => "parse_error",
            Self::SkippedFile => "skipped_file",
            Self::UnsupportedFile => "unsupported_file",
            Self::Unknown => "unknown",
        }
    }
}

/// Parse brief output JSON.
///
/// # Errors
///
/// Returns [`AppError`] when JSON is invalid or required fields do not match schema.
pub(crate) fn parse_brief_output(
    raw_json: &str,
    goal: &str,
    metadata: CommandMetadata,
) -> Result<BriefOutput, AppError> {
    serde_json::from_str::<RawBriefOutput>(raw_json)
        .map(|value| BriefOutput::from_raw(value, goal, metadata))
        .map_err(|error| {
            AppError::response_parse_failed(format!("failed to parse brief output: {error}"))
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::missing_errors_doc, reason = "test helpers stay local")]
    #![allow(clippy::missing_panics_doc, reason = "test asserts and fixtures")]

    use serde_json::json;

    use super::parse_brief_output;
    use crate::{error::AppError, output::CommandMetadata};

    #[test]
    fn valid_model_json_parses_to_typed_success_output() {
        let output = parse_brief_output(&valid_model_json(), "trace CLI flow", metadata())
            .expect("output should parse");

        assert_eq!(output.goal, "trace CLI flow");
        assert_eq!(output.files.len(), 2);
        assert_eq!(output.symbols.len(), 1);
        assert_eq!(output.evidence.len(), 1);
        assert_eq!(output.risks.len(), 1);
        assert_eq!(output.next_reads.len(), 1);
        assert_eq!(output.metadata.input_bytes, 12420);
        assert_eq!(output.metadata.duration_ms, 980);
        assert_eq!(output.metadata.output_bytes, 0);
    }

    #[test]
    fn serialized_json_uses_cli_owned_goal_and_metadata() {
        let output = parse_brief_output(&valid_model_json(), "trace CLI flow", metadata())
            .expect("output should parse");
        let value = serde_json::from_str::<serde_json::Value>(
            &output.into_json().expect("output should serialize"),
        )
        .expect("json should parse");

        assert_eq!(value["goal"], "trace CLI flow");
        assert_eq!(value["metadata"]["input_bytes"], 12420);
        assert_eq!(value["metadata"]["duration_ms"], 980);
        assert!(value["metadata"]["output_bytes"].as_u64().unwrap_or(0) > 0);
        assert!(
            value["metadata"]["compression_ratio"]
                .as_str()
                .and_then(|ratio| ratio.parse::<f64>().ok())
                .is_some_and(|ratio| ratio > 1.0)
        );
    }

    #[test]
    fn serialized_json_keeps_fixed_top_level_fields() {
        let output = parse_brief_output(&valid_model_json(), "trace CLI flow", metadata())
            .expect("output should parse");
        let mut value = serde_json::from_str::<serde_json::Value>(
            &output.into_json().expect("output should serialize"),
        )
        .expect("json should parse");
        let metadata = value
            .as_object_mut()
            .and_then(|root| root.remove("metadata"))
            .expect("metadata should exist");

        assert_eq!(
            value,
            json!({
                "schema_version": "1.0",
                "command": "brief",
                "status": "ok",
                "goal": "trace CLI flow",
                "brief": {
                    "summary": "CLI flow starts in main and dispatches command runner.",
                    "confidence": "high",
                    "not_found": false
                },
                "files": [
                    {
                        "path": "src/cli.rs",
                        "role": "Defines brief args.",
                        "key_points": ["Adds required goal flag."],
                        "bytes": 3200
                    },
                    {
                        "path": "src/commands.rs",
                        "role": "Dispatches brief runner.",
                        "key_points": ["Routes parsed command to module."],
                        "bytes": 620
                    }
                ],
                "symbols": [
                    {
                        "name": "run_brief",
                        "kind": "function",
                        "path": "src/commands/brief.rs",
                        "responsibility": "Runs brief command and prints JSON."
                    }
                ],
                "evidence": [
                    {
                        "path": "src/commands.rs",
                        "symbol": "try_run",
                        "note": "Match arm dispatches brief args into runner."
                    }
                ],
                "risks": [
                    {
                        "kind": "missing_context",
                        "message": "Loaded files omit output parser details."
                    }
                ],
                "next_reads": [
                    {
                        "path": "src/output/brief.rs",
                        "reason": "Shows final JSON shape."
                    }
                ]
            })
        );

        assert_eq!(metadata["input_bytes"], 12420);
        assert_eq!(metadata["duration_ms"], 980);
        assert!(metadata["output_bytes"].as_u64().unwrap_or(0) > 0);
        assert!(
            metadata["compression_ratio"]
                .as_str()
                .and_then(|ratio| ratio.parse::<f64>().ok())
                .is_some_and(|ratio| ratio > 1.0)
        );
    }

    #[test]
    fn missing_required_field_maps_to_response_parse_failed() {
        let error = parse_brief_output(
            r#"{
                "brief":{"summary":"summary","confidence":"high","not_found":false},
                "files":[],
                "symbols":[],
                "evidence":[],
                "risks":[]
            }"#,
            "trace CLI flow",
            metadata(),
        )
        .expect_err("parse should fail");

        assert!(matches!(error, AppError::ResponseParseFailed { .. }));
    }

    #[test]
    fn bad_enum_value_maps_to_response_parse_failed() {
        let error = parse_brief_output(
            r#"{
                "brief":{"summary":"summary","confidence":"certain","not_found":false},
                "files":[],
                "symbols":[],
                "evidence":[],
                "risks":[],
                "next_reads":[]
            }"#,
            "trace CLI flow",
            metadata(),
        )
        .expect_err("parse should fail");

        assert!(matches!(error, AppError::ResponseParseFailed { .. }));
    }

    fn valid_model_json() -> String {
        json!({
            "schema_version": "9.9",
            "command": "wrong",
            "status": "bad",
            "goal": "wrong goal",
            "brief": {
                "summary": "CLI flow starts in main and dispatches command runner.",
                "confidence": "high",
                "not_found": false
            },
            "files": [
                {
                    "path": "src/commands.rs",
                    "role": "Dispatches brief runner.",
                    "key_points": ["Routes parsed command to module."],
                    "bytes": 620
                },
                {
                    "path": "src/cli.rs",
                    "role": "Defines brief args.",
                    "key_points": ["Adds required goal flag."],
                    "bytes": 3200
                }
            ],
            "symbols": [
                {
                    "name": "run_brief",
                    "kind": "function",
                    "path": "src/commands/brief.rs",
                    "responsibility": "Runs brief command and prints JSON."
                }
            ],
            "evidence": [
                {
                    "path": "src/commands.rs",
                    "symbol": "try_run",
                    "note": "Match arm dispatches brief args into runner."
                }
            ],
            "risks": [
                {
                    "kind": "missing_context",
                    "message": "Loaded files omit output parser details."
                }
            ],
            "next_reads": [
                {
                    "path": "src/output/brief.rs",
                    "reason": "Shows final JSON shape."
                }
            ]
        })
        .to_string()
    }

    fn metadata() -> CommandMetadata {
        CommandMetadata::new(12420, 980)
    }
}
