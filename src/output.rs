use serde::{Deserialize, Serialize};

use crate::error::AppError;

const SCHEMA_VERSION: &str = "1.0";
const COMMAND: &str = "ask";
const STATUS_OK: &str = "ok";

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct AskOutput {
    schema_version: &'static str,
    command: &'static str,
    status: &'static str,
    question: String,
    answer: AskAnswer,
    files: Vec<AskFile>,
    symbols: Vec<AskSymbol>,
    evidence: Vec<AskEvidence>,
    risks: Vec<AskRisk>,
    next_reads: Vec<AskNextRead>,
    metadata: AskMetadata,
}

impl AskOutput {
    #[must_use]
    pub(crate) fn to_json(&self) -> String {
        serde_json::to_string(self).expect("ask output should serialize")
    }
}

#[derive(Debug, Deserialize)]
struct RawAskOutput {
    question: String,
    answer: AskAnswer,
    files: Vec<AskFile>,
    symbols: Vec<AskSymbol>,
    evidence: Vec<AskEvidence>,
    risks: Vec<AskRisk>,
    next_reads: Vec<AskNextRead>,
    metadata: AskMetadata,
}

impl From<RawAskOutput> for AskOutput {
    fn from(value: RawAskOutput) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            command: COMMAND,
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
pub(crate) struct AskAnswer {
    summary: String,
    confidence: AskConfidence,
    not_found: bool,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct AskFile {
    path: String,
    included: bool,
    reason: String,
    bytes: usize,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct AskSymbol {
    name: String,
    kind: AskSymbolKind,
    path: String,
    relevance: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct AskEvidence {
    path: String,
    symbol: String,
    note: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct AskRisk {
    kind: AskRiskKind,
    message: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct AskNextRead {
    path: String,
    reason: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct AskMetadata {
    input_bytes: usize,
    duration_ms: usize,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AskConfidence {
    High,
    Medium,
    Low,
    Unknown,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AskSymbolKind {
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

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AskRiskKind {
    MissingContext,
    ModelUncertainty,
    ParseError,
    SkippedFile,
    UnsupportedFile,
    Unknown,
}

pub(crate) fn parse_ask_output(raw_json: &str) -> Result<AskOutput, AppError> {
    serde_json::from_str::<RawAskOutput>(raw_json)
        .map(AskOutput::from)
        .map_err(|error| {
            AppError::response_parse_failed(format!("failed to parse ask output: {error}"))
        })
}

#[cfg(test)]
mod tests {
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
        let value = serde_json::from_str::<serde_json::Value>(&output.to_json())
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
