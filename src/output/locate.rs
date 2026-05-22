use serde::{Deserialize, Serialize};

use crate::error::AppError;

use super::{CommandMetadata, LOCATE_COMMAND, SCHEMA_VERSION, STATUS_OK};

#[derive(Debug, Serialize, PartialEq, Eq)]
/// Locate command output.
pub(crate) struct LocateOutput {
    /// JSON schema version.
    schema_version: &'static str,
    /// Command tag.
    command: &'static str,
    /// Output status.
    status: &'static str,
    /// Candidate matches.
    matches: Vec<LocateMatch>,
    /// Suggested next reads.
    next_reads: Vec<LocateNextRead>,
    /// Risk list.
    risks: Vec<LocateRisk>,
    /// Output metadata.
    metadata: CommandMetadata,
}

impl LocateOutput {
    /// Serialize output to JSON.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] when JSON serialization fails.
    pub(crate) fn into_json(mut self) -> Result<String, AppError> {
        loop {
            let json = serde_json::to_string(&self).map_err(|error| {
                AppError::response_parse_failed(format!(
                    "failed to serialize locate output: {error}"
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
/// Raw locate output from model.
struct RawLocateOutput {
    /// Candidate matches.
    matches: Vec<LocateMatch>,
    /// Suggested next reads.
    next_reads: Vec<LocateNextRead>,
    /// Risk list.
    risks: Vec<LocateRisk>,
}

impl LocateOutput {
    /// Add fixed fields and normalize order.
    fn from_raw(value: RawLocateOutput, metadata: CommandMetadata) -> Self {
        let mut matches = value.matches;
        let mut next_reads = value.next_reads;
        let mut risks = value.risks;

        matches.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        matches.dedup();
        next_reads.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        next_reads.dedup();
        risks.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        risks.dedup();

        Self {
            schema_version: SCHEMA_VERSION,
            command: LOCATE_COMMAND,
            status: STATUS_OK,
            matches,
            next_reads,
            risks,
            metadata,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
/// One locate match row.
struct LocateMatch {
    /// File path.
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional symbol name.
    symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional symbol kind.
    kind: Option<LocateSymbolKind>,
    /// Match reason.
    reason: String,
    /// Match confidence.
    confidence: LocateConfidence,
}

impl LocateMatch {
    /// Build stable sort key.
    fn sort_key(&self) -> (u8, &str, Option<&str>, Option<&str>, &str) {
        (
            self.confidence.rank(),
            self.path.as_str(),
            self.symbol.as_deref(),
            self.kind.as_ref().map(LocateSymbolKind::as_str),
            self.reason.as_str(),
        )
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
/// One locate next-read row.
struct LocateNextRead {
    /// File path.
    path: String,
    /// Read reason.
    reason: String,
}

impl LocateNextRead {
    /// Build stable sort key.
    fn sort_key(&self) -> (&str, &str) {
        (self.path.as_str(), self.reason.as_str())
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
/// One locate risk row.
struct LocateRisk {
    /// Risk kind.
    kind: LocateRiskKind,
    /// Risk message.
    message: String,
}

impl LocateRisk {
    /// Build stable sort key.
    fn sort_key(&self) -> (&str, &str) {
        (self.kind.as_str(), self.message.as_str())
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Locate confidence level.
enum LocateConfidence {
    /// High confidence.
    High,
    /// Medium confidence.
    Medium,
    /// Low confidence.
    Low,
    /// Unknown confidence.
    Unknown,
}

impl LocateConfidence {
    /// Rank confidence for sort.
    fn rank(&self) -> u8 {
        match self {
            Self::High => 0,
            Self::Medium => 1,
            Self::Low => 2,
            Self::Unknown => 3,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Locate symbol kind.
enum LocateSymbolKind {
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

impl LocateSymbolKind {
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
/// Locate risk kind.
enum LocateRiskKind {
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

impl LocateRiskKind {
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

/// Parse locate output JSON.
///
/// # Errors
///
/// Returns [`AppError`] when the JSON is invalid or required fields do not match schema.
pub(crate) fn parse_locate_output(
    raw_json: &str,
    metadata: CommandMetadata,
) -> Result<LocateOutput, AppError> {
    serde_json::from_str::<RawLocateOutput>(raw_json)
        .map(|value| LocateOutput::from_raw(value, metadata))
        .map_err(|error| {
            AppError::response_parse_failed(format!("failed to parse locate output: {error}"))
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::missing_errors_doc, reason = "test helpers stay local")]
    #![allow(clippy::missing_panics_doc, reason = "test asserts and fixtures")]

    use serde_json::json;

    use super::parse_locate_output;
    use crate::{error::AppError, output::CommandMetadata};

    #[test]
    fn valid_model_json_parses_to_typed_success_output() {
        let output =
            parse_locate_output(&valid_model_json(), metadata()).expect("output should parse");

        assert_eq!(output.matches.len(), 2);
        assert_eq!(output.next_reads.len(), 1);
        assert_eq!(output.risks.len(), 1);
        assert_eq!(output.metadata.input_bytes, 12420);
        assert_eq!(output.metadata.duration_ms, 980);
        assert_eq!(output.metadata.output_bytes, 0);
    }

    #[test]
    fn serialized_json_uses_cli_owned_metadata() {
        let output =
            parse_locate_output(&valid_model_json(), metadata()).expect("output should parse");
        let value = serde_json::from_str::<serde_json::Value>(
            &output.into_json().expect("output should serialize"),
        )
        .expect("json should parse");

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
        let output =
            parse_locate_output(&valid_model_json(), metadata()).expect("output should parse");
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
                "command": "locate",
                "status": "ok",
                "matches": [
                    {
                        "path": "src/cli.rs",
                        "symbol": "Command",
                        "kind": "type",
                        "reason": "Defines CLI command variants.",
                        "confidence": "high"
                    },
                    {
                        "path": "src/commands.rs",
                        "reason": "Dispatches parsed subcommands.",
                        "confidence": "medium"
                    }
                ],
                "next_reads": [
                    {
                        "path": "src/commands/ask.rs",
                        "reason": "Shows current command run flow."
                    }
                ],
                "risks": [
                    {
                        "kind": "missing_context",
                        "message": "Loaded files do not include all command modules."
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
        let error = parse_locate_output(
            r#"{
                "matches":[],
                "risks":[]
            }"#,
            metadata(),
        )
        .expect_err("parse should fail");

        assert!(matches!(error, AppError::ResponseParseFailed { .. }));
    }

    #[test]
    fn bad_enum_value_maps_to_response_parse_failed() {
        let error = parse_locate_output(
            r#"{
                "matches":[
                    {
                        "path":"src/lib.rs",
                        "reason":"reason",
                        "confidence":"certain"
                    }
                ],
                "next_reads":[],
                "risks":[]
            }"#,
            metadata(),
        )
        .expect_err("parse should fail");

        assert!(matches!(error, AppError::ResponseParseFailed { .. }));
    }

    fn valid_model_json() -> String {
        json!({
            "matches": [
                {
                    "path": "src/commands.rs",
                    "reason": "Dispatches parsed subcommands.",
                    "confidence": "medium"
                },
                {
                    "path": "src/cli.rs",
                    "symbol": "Command",
                    "kind": "type",
                    "reason": "Defines CLI command variants.",
                    "confidence": "high"
                }
            ],
            "next_reads": [
                {
                    "path": "src/commands/ask.rs",
                    "reason": "Shows current command run flow."
                }
            ],
            "risks": [
                {
                    "kind": "missing_context",
                    "message": "Loaded files do not include all command modules."
                }
            ]
        })
        .to_string()
    }

    fn metadata() -> CommandMetadata {
        CommandMetadata::new(12420, 980)
    }
}
