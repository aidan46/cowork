use serde::{Deserialize, Serialize};

use crate::error::AppError;

use super::{ASK_COMMAND, CommandMetadata, SCHEMA_VERSION, STATUS_OK};

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
    metadata: CommandMetadata,
}

impl AskOutput {
    /// Serialize output to JSON.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] when JSON serialization fails.
    pub(crate) fn into_json(mut self) -> Result<String, AppError> {
        loop {
            let json = serde_json::to_string(&self).map_err(|error| {
                AppError::response_parse_failed(format!("failed to serialize ask output: {error}"))
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
}

impl AskOutput {
    /// Add fixed fields, normalize order.
    fn from_raw(value: RawAskOutput, metadata: CommandMetadata) -> Self {
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
            command: ASK_COMMAND,
            status: STATUS_OK,
            question: value.question,
            answer: value.answer,
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

impl AskFile {
    /// Build stable sort key.
    fn sort_key(&self) -> (&str, u8, &str, usize) {
        (
            self.path.as_str(),
            if self.included { 0 } else { 1 },
            self.reason.as_str(),
            self.bytes,
        )
    }
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

impl AskSymbol {
    /// Build stable sort key.
    fn sort_key(&self) -> (&str, &str, &str, &str) {
        (
            self.path.as_str(),
            self.name.as_str(),
            self.kind.as_str(),
            self.relevance.as_str(),
        )
    }
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

impl AskEvidence {
    /// Build stable sort key.
    fn sort_key(&self) -> (&str, &str, &str) {
        (self.path.as_str(), self.symbol.as_str(), self.note.as_str())
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
/// One risk row.
struct AskRisk {
    /// Risk kind.
    kind: AskRiskKind,
    /// Risk message.
    message: String,
}

impl AskRisk {
    /// Build stable sort key.
    fn sort_key(&self) -> (&str, &str) {
        (self.kind.as_str(), self.message.as_str())
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
/// One next-read row.
struct AskNextRead {
    /// File path.
    path: String,
    /// Read reason.
    reason: String,
}

impl AskNextRead {
    /// Build stable sort key.
    fn sort_key(&self) -> (&str, &str) {
        (self.path.as_str(), self.reason.as_str())
    }
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

impl AskSymbolKind {
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

impl AskRiskKind {
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

/// Parse ask output JSON.
///
/// # Errors
///
/// Returns [`AppError`] when the JSON is invalid or required fields do not match schema.
pub(crate) fn parse_ask_output(
    raw_json: &str,
    metadata: CommandMetadata,
) -> Result<AskOutput, AppError> {
    serde_json::from_str::<RawAskOutput>(raw_json)
        .map(|value| AskOutput::from_raw(value, metadata))
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
    use crate::{error::AppError, output::CommandMetadata};

    #[test]
    fn valid_model_json_parses_to_typed_success_output() {
        let output =
            parse_ask_output(&valid_model_json(), metadata()).expect("output should parse");

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
        assert_eq!(output.metadata.duration_ms, 980);
        assert_eq!(output.metadata.output_bytes, 0);
    }

    #[test]
    fn parsed_output_sorts_and_dedupes_rows() {
        let output = parse_ask_output(&unsorted_duplicate_model_json(), metadata())
            .expect("output should parse");

        assert_eq!(
            serde_json::to_value(&output.files).expect("files should serialize"),
            json!([
                {
                    "path": "src/a.rs",
                    "included": true,
                    "reason": "Contains auth flow.",
                    "bytes": 10
                },
                {
                    "path": "src/a.rs",
                    "included": true,
                    "reason": "Contains auth flow.",
                    "bytes": 30
                },
                {
                    "path": "src/z.rs",
                    "included": false,
                    "reason": "Skipped binary snapshot.",
                    "bytes": 12
                }
            ])
        );
        assert_eq!(
            serde_json::to_value(&output.symbols).expect("symbols should serialize"),
            json!([
                {
                    "name": "AuthState",
                    "kind": "type",
                    "path": "src/a.rs",
                    "relevance": "Owns auth state."
                },
                {
                    "name": "authenticate",
                    "kind": "function",
                    "path": "src/a.rs",
                    "relevance": "Runs auth checks."
                },
                {
                    "name": "snapshot_auth",
                    "kind": "function",
                    "path": "src/z.rs",
                    "relevance": "Exports auth snapshot."
                }
            ])
        );
        assert_eq!(
            serde_json::to_value(&output.evidence).expect("evidence should serialize"),
            json!([
                {
                    "path": "src/a.rs",
                    "symbol": "AuthState",
                    "note": "Holds request auth state."
                },
                {
                    "path": "src/a.rs",
                    "symbol": "authenticate",
                    "note": "Rejects bad tokens."
                },
                {
                    "path": "src/z.rs",
                    "symbol": "snapshot_auth",
                    "note": "Writes auth snapshot."
                }
            ])
        );
        assert_eq!(
            serde_json::to_value(&output.risks).expect("risks should serialize"),
            json!([
                {
                    "kind": "missing_context",
                    "message": "Tests missing."
                },
                {
                    "kind": "unsupported_file",
                    "message": "Binary snapshot skipped."
                }
            ])
        );
        assert_eq!(
            serde_json::to_value(&output.next_reads).expect("next reads should serialize"),
            json!([
                {
                    "path": "src/a/tests.rs",
                    "reason": "Likely covers auth edges."
                },
                {
                    "path": "src/z/tests.rs",
                    "reason": "Likely covers snapshot edges."
                }
            ])
        );
    }

    #[test]
    fn serialized_json_uses_cli_owned_metadata() {
        let output =
            parse_ask_output(&valid_model_json(), metadata()).expect("output should parse");
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
    fn serialized_json_normalizes_row_order_and_dedupes() {
        let output = parse_ask_output(&unsorted_duplicate_model_json(), metadata())
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
                "command": "ask",
                "status": "ok",
                "question": "Where does auth state live?",
                "answer": {
                    "summary": "Auth state lives in src/a.rs.",
                    "confidence": "medium",
                    "not_found": false
                },
                "files": [
                    {
                        "path": "src/a.rs",
                        "included": true,
                        "reason": "Contains auth flow.",
                        "bytes": 10
                    },
                    {
                        "path": "src/a.rs",
                        "included": true,
                        "reason": "Contains auth flow.",
                        "bytes": 30
                    },
                    {
                        "path": "src/z.rs",
                        "included": false,
                        "reason": "Skipped binary snapshot.",
                        "bytes": 12
                    }
                ],
                "symbols": [
                    {
                        "name": "AuthState",
                        "kind": "type",
                        "path": "src/a.rs",
                        "relevance": "Owns auth state."
                    },
                    {
                        "name": "authenticate",
                        "kind": "function",
                        "path": "src/a.rs",
                        "relevance": "Runs auth checks."
                    },
                    {
                        "name": "snapshot_auth",
                        "kind": "function",
                        "path": "src/z.rs",
                        "relevance": "Exports auth snapshot."
                    }
                ],
                "evidence": [
                    {
                        "path": "src/a.rs",
                        "symbol": "AuthState",
                        "note": "Holds request auth state."
                    },
                    {
                        "path": "src/a.rs",
                        "symbol": "authenticate",
                        "note": "Rejects bad tokens."
                    },
                    {
                        "path": "src/z.rs",
                        "symbol": "snapshot_auth",
                        "note": "Writes auth snapshot."
                    }
                ],
                "risks": [
                    {
                        "kind": "missing_context",
                        "message": "Tests missing."
                    },
                    {
                        "kind": "unsupported_file",
                        "message": "Binary snapshot skipped."
                    }
                ],
                "next_reads": [
                    {
                        "path": "src/a/tests.rs",
                        "reason": "Likely covers auth edges."
                    },
                    {
                        "path": "src/z/tests.rs",
                        "reason": "Likely covers snapshot edges."
                    }
                ]
            })
        );

        assert_eq!(metadata["input_bytes"], 12420);
        assert_eq!(metadata["duration_ms"], 980);
        assert!(metadata["output_bytes"].as_u64().unwrap_or(0) > 0);
    }

    #[test]
    fn serialized_json_keeps_fixed_top_level_fields() {
        let output =
            parse_ask_output(&valid_model_json(), metadata()).expect("output should parse");
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
        let error = parse_ask_output(
            r#"{
                "question":"q",
                "answer":{"summary":"s","confidence":"high","not_found":false},
                "files":[],
                "symbols":[],
                "evidence":[],
                "risks":[]
            }"#,
            metadata(),
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
                "next_reads":[]
            }"#,
            metadata(),
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
            ]
        })
        .to_string()
    }

    fn unsorted_duplicate_model_json() -> String {
        json!({
            "question": "Where does auth state live?",
            "answer": {
                "summary": "Auth state lives in src/a.rs.",
                "confidence": "medium",
                "not_found": false
            },
            "files": [
                {
                    "path": "src/z.rs",
                    "included": false,
                    "reason": "Skipped binary snapshot.",
                    "bytes": 12
                },
                {
                    "path": "src/a.rs",
                    "included": true,
                    "reason": "Contains auth flow.",
                    "bytes": 30
                },
                {
                    "path": "src/a.rs",
                    "included": true,
                    "reason": "Contains auth flow.",
                    "bytes": 10
                },
                {
                    "path": "src/a.rs",
                    "included": true,
                    "reason": "Contains auth flow.",
                    "bytes": 30
                }
            ],
            "symbols": [
                {
                    "name": "snapshot_auth",
                    "kind": "function",
                    "path": "src/z.rs",
                    "relevance": "Exports auth snapshot."
                },
                {
                    "name": "authenticate",
                    "kind": "function",
                    "path": "src/a.rs",
                    "relevance": "Runs auth checks."
                },
                {
                    "name": "AuthState",
                    "kind": "type",
                    "path": "src/a.rs",
                    "relevance": "Owns auth state."
                },
                {
                    "name": "authenticate",
                    "kind": "function",
                    "path": "src/a.rs",
                    "relevance": "Runs auth checks."
                }
            ],
            "evidence": [
                {
                    "path": "src/z.rs",
                    "symbol": "snapshot_auth",
                    "note": "Writes auth snapshot."
                },
                {
                    "path": "src/a.rs",
                    "symbol": "authenticate",
                    "note": "Rejects bad tokens."
                },
                {
                    "path": "src/a.rs",
                    "symbol": "AuthState",
                    "note": "Holds request auth state."
                },
                {
                    "path": "src/a.rs",
                    "symbol": "authenticate",
                    "note": "Rejects bad tokens."
                }
            ],
            "risks": [
                {
                    "kind": "unsupported_file",
                    "message": "Binary snapshot skipped."
                },
                {
                    "kind": "missing_context",
                    "message": "Tests missing."
                },
                {
                    "kind": "missing_context",
                    "message": "Tests missing."
                }
            ],
            "next_reads": [
                {
                    "path": "src/z/tests.rs",
                    "reason": "Likely covers snapshot edges."
                },
                {
                    "path": "src/a/tests.rs",
                    "reason": "Likely covers auth edges."
                },
                {
                    "path": "src/a/tests.rs",
                    "reason": "Likely covers auth edges."
                }
            ]
        })
        .to_string()
    }

    fn metadata() -> CommandMetadata {
        CommandMetadata::new(12420, 980)
    }
}
