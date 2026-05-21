use serde::{Deserialize, Serialize};

use crate::error::AppError;

use super::{LOCATE_COMMAND, SCHEMA_VERSION, STATUS_OK};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct LocateOutput {
    schema_version: &'static str,
    command: &'static str,
    status: &'static str,
    matches: Vec<LocateMatch>,
    next_reads: Vec<LocateNextRead>,
    risks: Vec<LocateRisk>,
}

impl LocateOutput {
    #[must_use]
    pub(crate) fn to_json(&self) -> String {
        serde_json::to_string(self).expect("locate output should serialize")
    }
}

#[derive(Debug, Deserialize)]
struct RawLocateOutput {
    matches: Vec<LocateMatch>,
    next_reads: Vec<LocateNextRead>,
    risks: Vec<LocateRisk>,
}

impl From<RawLocateOutput> for LocateOutput {
    fn from(value: RawLocateOutput) -> Self {
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
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
struct LocateMatch {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<LocateSymbolKind>,
    reason: String,
    confidence: LocateConfidence,
}

impl LocateMatch {
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
struct LocateNextRead {
    path: String,
    reason: String,
}

impl LocateNextRead {
    fn sort_key(&self) -> (&str, &str) {
        (self.path.as_str(), self.reason.as_str())
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
struct LocateRisk {
    kind: LocateRiskKind,
    message: String,
}

impl LocateRisk {
    fn sort_key(&self) -> (&str, &str) {
        (self.kind.as_str(), self.message.as_str())
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum LocateConfidence {
    High,
    Medium,
    Low,
    Unknown,
}

impl LocateConfidence {
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
enum LocateSymbolKind {
    Function,
    Type,
    Trait,
    Impl,
    Module,
    Constant,
    Variable,
    Route,
    Component,
    Test,
    Unknown,
}

impl LocateSymbolKind {
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
enum LocateRiskKind {
    MissingContext,
    ModelUncertainty,
    ParseError,
    SkippedFile,
    UnsupportedFile,
    Unknown,
}

impl LocateRiskKind {
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

pub(crate) fn parse_locate_output(raw_json: &str) -> Result<LocateOutput, AppError> {
    serde_json::from_str::<RawLocateOutput>(raw_json)
        .map(LocateOutput::from)
        .map_err(|error| {
            AppError::response_parse_failed(format!("failed to parse locate output: {error}"))
        })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::parse_locate_output;
    use crate::error::AppError;

    #[test]
    fn valid_model_json_parses_to_typed_success_output() {
        let output = parse_locate_output(&valid_model_json()).expect("output should parse");

        assert_eq!(output.matches.len(), 2);
        assert_eq!(output.next_reads.len(), 1);
        assert_eq!(output.risks.len(), 1);
    }

    #[test]
    fn serialized_json_keeps_fixed_top_level_fields() {
        let output = parse_locate_output(&valid_model_json()).expect("output should parse");
        let value = serde_json::from_str::<serde_json::Value>(&output.to_json())
            .expect("json should parse");

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
    }

    #[test]
    fn missing_required_field_maps_to_response_parse_failed() {
        let error = parse_locate_output(
            r#"{
                "matches":[],
                "risks":[]
            }"#,
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
}
