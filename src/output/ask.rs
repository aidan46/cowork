use serde::{Deserialize, Serialize};

use crate::error::AppError;

use super::{ASK_COMMAND, SCHEMA_VERSION, STATUS_OK};

#[derive(Debug, Serialize, PartialEq, Eq)]
/// Ask command output.
pub(crate) struct AskOutput {
    /// JSON schema version.
    schema_version: &'static str,
    /// Command tag.
    command: &'static str,
    /// Output status.
    status: &'static str,
    /// Asked question.
    question: String,
    /// Answer body.
    answer: AskAnswer,
    /// File evidence list.
    files: Vec<AskFile>,
    /// Symbol evidence list.
    symbols: Vec<AskSymbol>,
    /// Evidence notes.
    evidence: Vec<AskEvidence>,
    /// Risk list.
    risks: Vec<AskRisk>,
    /// Suggested next reads.
    next_reads: Vec<AskNextRead>,
    /// Output metadata.
    metadata: AskMetadata,
}

impl AskOutput {
    /// Serialize output to JSON.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] when JSON serialization fails.
    pub(crate) fn to_json(&self) -> Result<String, AppError> {
        serde_json::to_string(self).map_err(|error| {
            AppError::response_parse_failed(format!("failed to serialize ask output: {error}"))
        })
    }
}

#[derive(Debug, Deserialize)]
/// Raw ask output from model.
struct RawAskOutput {
    /// Asked question.
    question: String,
    /// Answer body.
    answer: AskAnswer,
    /// File evidence list.
    files: Vec<AskFile>,
    /// Symbol evidence list.
    symbols: Vec<AskSymbol>,
    /// Evidence notes.
    evidence: Vec<AskEvidence>,
    /// Risk list.
    risks: Vec<AskRisk>,
    /// Suggested next reads.
    next_reads: Vec<AskNextRead>,
    /// Output metadata.
    metadata: AskMetadata,
}

impl From<RawAskOutput> for AskOutput {
    /// Add fixed top-level fields.
    fn from(value: RawAskOutput) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            command: ASK_COMMAND,
            status: STATUS_OK,
            question: value.question,
            answer: value.answer,
            files: value.files,
            symbols: value.symbols,
            evidence: value.evidence,
            risks: value.risks,
            next_reads: value.next_reads,
            metadata: value.metadata,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
/// Ask answer body.
pub(crate) struct AskAnswer {
    /// Short answer text.
    summary: String,
    /// Answer confidence.
    confidence: AskConfidence,
    /// True when evidence missing.
    not_found: bool,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
/// One file evidence row.
struct AskFile {
    /// File path.
    path: String,
    /// True when file was included.
    included: bool,
    /// Inclusion reason.
    reason: String,
    /// File byte count.
    bytes: usize,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
/// One symbol evidence row.
struct AskSymbol {
    /// Symbol name.
    name: String,
    /// Symbol kind.
    kind: AskSymbolKind,
    /// Source path.
    path: String,
    /// Relevance note.
    relevance: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
/// One evidence note.
struct AskEvidence {
    /// Source path.
    path: String,
    /// Source symbol.
    symbol: String,
    /// Evidence note.
    note: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
/// One risk row.
struct AskRisk {
    /// Risk kind.
    kind: AskRiskKind,
    /// Risk message.
    message: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
/// One next-read row.
struct AskNextRead {
    /// File path.
    path: String,
    /// Read reason.
    reason: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
/// Ask output metadata.
struct AskMetadata {
    /// Input byte count.
    input_bytes: usize,
    /// Model time in ms.
    duration_ms: usize,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Ask confidence level.
enum AskConfidence {
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
/// Ask symbol kind.
enum AskSymbolKind {
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

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Ask risk kind.
enum AskRiskKind {
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

/// Parse ask output JSON.
///
/// # Errors
///
/// Returns [`AppError`] when the JSON is invalid or required fields do not match schema.
pub(crate) fn parse_ask_output(raw_json: &str) -> Result<AskOutput, AppError> {
    serde_json::from_str::<RawAskOutput>(raw_json)
        .map(AskOutput::from)
        .map_err(|error| {
            AppError::response_parse_failed(format!("failed to parse ask output: {error}"))
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::missing_errors_doc, reason = "test helpers stay local")]
    #![allow(clippy::missing_panics_doc, reason = "test asserts and fixtures")]

    use serde_json::json;

    use super::parse_ask_output;
    use crate::error::AppError;

    #[test]
    fn valid_model_json_parses_to_typed_success_output() {
        let output = parse_ask_output(&valid_model_json()).expect("output should parse");

        assert_eq!(output.question, "How does request authentication work?");
        assert_eq!(
            output.answer.summary,
            "Authentication is enforced in middleware."
        );
        assert_eq!(output.files.len(), 1);
        assert_eq!(output.symbols.len(), 1);
        assert_eq!(output.evidence.len(), 1);
        assert_eq!(output.risks.len(), 1);
        assert_eq!(output.next_reads.len(), 1);
        assert_eq!(output.metadata.input_bytes, 12420);
    }

    #[test]
    fn serialized_json_keeps_fixed_top_level_fields() {
        let output = parse_ask_output(&valid_model_json()).expect("output should parse");
        let value = serde_json::from_str::<serde_json::Value>(
            &output.to_json().expect("output should serialize"),
        )
        .expect("json should parse");

        assert_eq!(
            value,
            json!({
                "schema_version": "1.0",
                "command": "ask",
                "status": "ok",
                "question": "How does request authentication work?",
                "answer": {
                    "summary": "Authentication is enforced in middleware.",
                    "confidence": "high",
                    "not_found": false
                },
                "files": [
                    {
                        "path": "src/auth/middleware.rs",
                        "included": true,
                        "reason": "Contains authentication middleware.",
                        "bytes": 12420
                    }
                ],
                "symbols": [
                    {
                        "name": "authenticate_request",
                        "kind": "function",
                        "path": "src/auth/middleware.rs",
                        "relevance": "Validates credentials and attaches user context."
                    }
                ],
                "evidence": [
                    {
                        "path": "src/auth/middleware.rs",
                        "symbol": "authenticate_request",
                        "note": "Requests without valid credentials return early."
                    }
                ],
                "risks": [
                    {
                        "kind": "missing_context",
                        "message": "Tests were not provided."
                    }
                ],
                "next_reads": [
                    {
                        "path": "src/auth/tests.rs",
                        "reason": "Likely contains authentication edge cases."
                    }
                ],
                "metadata": {
                    "input_bytes": 12420,
                    "duration_ms": 980
                }
            })
        );
    }

    #[test]
    fn missing_required_field_maps_to_response_parse_failed() {
        let error = parse_ask_output(
            r#"{
                "question":"q",
                "answer":{"summary":"s","confidence":"high","not_found":false},
                "files":[],
                "symbols":[],
                "evidence":[],
                "risks":[],
                "metadata":{"input_bytes":1,"duration_ms":2}
            }"#,
        )
        .expect_err("parse should fail");

        assert!(matches!(error, AppError::ResponseParseFailed { .. }));
    }

    #[test]
    fn bad_enum_value_maps_to_response_parse_failed() {
        let error = parse_ask_output(
            r#"{
                "question":"q",
                "answer":{"summary":"s","confidence":"certain","not_found":false},
                "files":[],
                "symbols":[],
                "evidence":[],
                "risks":[],
                "next_reads":[],
                "metadata":{"input_bytes":1,"duration_ms":2}
            }"#,
        )
        .expect_err("parse should fail");

        assert!(matches!(error, AppError::ResponseParseFailed { .. }));
    }

    fn valid_model_json() -> String {
        json!({
            "question": "How does request authentication work?",
            "answer": {
                "summary": "Authentication is enforced in middleware.",
                "confidence": "high",
                "not_found": false
            },
            "files": [
                {
                    "path": "src/auth/middleware.rs",
                    "included": true,
                    "reason": "Contains authentication middleware.",
                    "bytes": 12420
                }
            ],
            "symbols": [
                {
                    "name": "authenticate_request",
                    "kind": "function",
                    "path": "src/auth/middleware.rs",
                    "relevance": "Validates credentials and attaches user context."
                }
            ],
            "evidence": [
                {
                    "path": "src/auth/middleware.rs",
                    "symbol": "authenticate_request",
                    "note": "Requests without valid credentials return early."
                }
            ],
            "risks": [
                {
                    "kind": "missing_context",
                    "message": "Tests were not provided."
                }
            ],
            "next_reads": [
                {
                    "path": "src/auth/tests.rs",
                    "reason": "Likely contains authentication edge cases."
                }
            ],
            "metadata": {
                "input_bytes": 12420,
                "duration_ms": 980
            }
        })
        .to_string()
    }
}
