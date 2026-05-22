use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::AppError;

use super::{ASK_COMMAND, CommandMetadata, SCHEMA_VERSION, STATUS_OK};

/// Max file rows.
const MAX_FILES: usize = 40;
/// Max symbol rows.
const MAX_SYMBOLS: usize = 80;
/// Max evidence rows.
const MAX_EVIDENCE: usize = 80;
/// Max risk rows.
const MAX_RISKS: usize = 20;
/// Max next-read rows.
const MAX_NEXT_READS: usize = 20;
/// Max model string chars.
const MAX_MODEL_STRING_CHARS: usize = 1200;
/// Truncation tail.
const TRUNCATION_MARKER: &str = " [truncated]";
/// Cap notice field order.
const CAP_FIELD_ORDER: [&str; 5] = ["files", "symbols", "evidence", "risks", "next_reads"];
/// Truncation notice field order.
const TRUNCATION_FIELD_ORDER: [&str; 6] = [
    "answer.summary",
    "files.reason",
    "symbols.relevance",
    "evidence.note",
    "risks.message",
    "next_reads.reason",
];

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
    fn from_raw(mut value: RawAskOutput, metadata: CommandMetadata) -> Self {
        let mut notes = NormalizationNotes::default();

        value.answer.normalize(&mut notes);
        value
            .files
            .iter_mut()
            .for_each(|file| file.normalize(&mut notes));
        value
            .symbols
            .iter_mut()
            .for_each(|symbol| symbol.normalize(&mut notes));
        value
            .evidence
            .iter_mut()
            .for_each(|evidence| evidence.normalize(&mut notes));
        value
            .risks
            .iter_mut()
            .for_each(|risk| risk.normalize(&mut notes));
        value
            .next_reads
            .iter_mut()
            .for_each(|next_read| next_read.normalize(&mut notes));

        let mut files = value.files;
        let mut symbols = value.symbols;
        let mut evidence = value.evidence;
        let mut risks = value.risks;
        let mut next_reads = value.next_reads;

        files.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        files.dedup();
        cap_rows(&mut files, "files", MAX_FILES, &mut notes);
        symbols.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        symbols.dedup();
        cap_rows(&mut symbols, "symbols", MAX_SYMBOLS, &mut notes);
        evidence.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        evidence.dedup();
        cap_rows(&mut evidence, "evidence", MAX_EVIDENCE, &mut notes);
        next_reads.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        next_reads.dedup();
        cap_rows(&mut next_reads, "next_reads", MAX_NEXT_READS, &mut notes);

        risks.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        risks.dedup();
        if risks.len() + notes.notice_count() > MAX_RISKS {
            let risk_keep =
                MAX_RISKS.saturating_sub(notes.notice_count() + usize::from(!notes.has_caps()));
            notes.note_risk_cap(risks.len(), risk_keep);
            risks.truncate(risk_keep);
        }
        risks.extend(notes.into_risks());
        risks.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        risks.dedup();

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

impl AskAnswer {
    /// Truncate model string fields.
    fn normalize(&mut self, notes: &mut NormalizationNotes) {
        truncate_model_string(&mut self.summary, "answer.summary", notes);
    }
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
    /// Truncate model string fields.
    fn normalize(&mut self, notes: &mut NormalizationNotes) {
        truncate_model_string(&mut self.reason, "files.reason", notes);
    }

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
    /// Truncate model string fields.
    fn normalize(&mut self, notes: &mut NormalizationNotes) {
        truncate_model_string(&mut self.relevance, "symbols.relevance", notes);
    }

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
    /// Truncate model string fields.
    fn normalize(&mut self, notes: &mut NormalizationNotes) {
        truncate_model_string(&mut self.note, "evidence.note", notes);
    }

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
    /// Truncate model string fields.
    fn normalize(&mut self, notes: &mut NormalizationNotes) {
        truncate_model_string(&mut self.message, "risks.message", notes);
    }

    /// Build output notice row.
    fn output_notice(message: String) -> Self {
        Self {
            kind: AskRiskKind::Unknown,
            message,
        }
    }

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
    /// Truncate model string fields.
    fn normalize(&mut self, notes: &mut NormalizationNotes) {
        truncate_model_string(&mut self.reason, "next_reads.reason", notes);
    }

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

/// Output change notes.
#[derive(Default)]
struct NormalizationNotes {
    /// Cap messages by field.
    caps: BTreeMap<&'static str, String>,
    /// Truncation counts by field.
    truncations: BTreeMap<&'static str, usize>,
}

impl NormalizationNotes {
    /// True when cap notice row needed.
    fn has_caps(&self) -> bool {
        !self.caps.is_empty()
    }

    /// Count injected notice rows.
    fn notice_count(&self) -> usize {
        usize::from(self.has_caps()) + usize::from(!self.truncations.is_empty())
    }

    /// Record capped array.
    fn note_cap(&mut self, field: &'static str, before: usize, after: usize) {
        if before > after {
            self.caps
                .insert(field, format!("{field} {before}->{after}"));
        }
    }

    /// Record capped risk rows.
    fn note_risk_cap(&mut self, before: usize, kept: usize) {
        if before > kept {
            self.caps
                .insert("risks", format!("risks kept {kept} of {before} model rows"));
        }
    }

    /// Record truncated string.
    fn note_truncation(&mut self, field: &'static str) {
        *self.truncations.entry(field).or_default() += 1;
    }

    /// Build output notice rows.
    fn into_risks(self) -> Vec<AskRisk> {
        let mut risks = Vec::new();

        if !self.caps.is_empty() {
            let capped_fields = CAP_FIELD_ORDER
                .iter()
                .filter_map(|field| self.caps.get(field))
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ");
            risks.push(AskRisk::output_notice(format!(
                "Output capped: {capped_fields}."
            )));
        }

        if !self.truncations.is_empty() {
            let truncated_fields = TRUNCATION_FIELD_ORDER
                .iter()
                .filter_map(|field| {
                    self.truncations
                        .get(field)
                        .map(|count| format!("{field} x{count}"))
                })
                .collect::<Vec<_>>()
                .join(", ");
            risks.push(AskRisk::output_notice(format!(
                "Output truncated at {MAX_MODEL_STRING_CHARS} chars: {truncated_fields}."
            )));
        }

        risks
    }
}

/// Cap row count, note drop.
fn cap_rows<T>(rows: &mut Vec<T>, field: &'static str, cap: usize, notes: &mut NormalizationNotes) {
    let before = rows.len();
    if before > cap {
        rows.truncate(cap);
        notes.note_cap(field, before, cap);
    }
}

/// Truncate long model string.
fn truncate_model_string(value: &mut String, field: &'static str, notes: &mut NormalizationNotes) {
    if truncate_string(value) {
        notes.note_truncation(field);
    }
}

/// Truncate string at char limit.
fn truncate_string(value: &mut String) -> bool {
    let value_chars = value.chars().count();
    if value_chars <= MAX_MODEL_STRING_CHARS {
        return false;
    }

    let keep_chars = MAX_MODEL_STRING_CHARS - TRUNCATION_MARKER.chars().count();
    let mut truncated = value.chars().take(keep_chars).collect::<String>();
    truncated.push_str(TRUNCATION_MARKER);
    *value = truncated;
    true
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
    fn parsed_output_caps_arrays_and_injects_cap_risk() {
        let output =
            parse_ask_output(&capped_model_json(), metadata()).expect("output should parse");

        assert_eq!(output.files.len(), 40);
        assert_eq!(output.symbols.len(), 80);
        assert_eq!(output.evidence.len(), 80);
        assert_eq!(output.risks.len(), 20);
        assert_eq!(output.next_reads.len(), 20);
        assert!(output.risks.iter().any(|risk| {
            risk == &super::AskRisk {
                kind: super::AskRiskKind::Unknown,
                message: "Output capped: files 45->40, symbols 85->80, evidence 85->80, risks kept 19 of 25 model rows, next_reads 25->20.".to_string(),
            }
        }));
    }

    #[test]
    fn serialized_json_truncates_long_strings_and_injects_truncation_risk() {
        let output =
            parse_ask_output(&truncated_model_json(), metadata()).expect("output should parse");
        let value = serde_json::from_str::<serde_json::Value>(
            &output.into_json().expect("output should serialize"),
        )
        .expect("json should parse");

        assert_eq!(
            value["answer"]["summary"]
                .as_str()
                .map(|summary| summary.chars().count()),
            Some(1200)
        );
        assert!(
            value["answer"]["summary"]
                .as_str()
                .is_some_and(|summary| summary.ends_with(" [truncated]"))
        );
        assert!(
            value["files"][0]["reason"]
                .as_str()
                .is_some_and(|reason| reason.ends_with(" [truncated]"))
        );
        assert!(
            value["symbols"][0]["relevance"]
                .as_str()
                .is_some_and(|relevance| relevance.ends_with(" [truncated]"))
        );
        assert!(
            value["evidence"][0]["note"]
                .as_str()
                .is_some_and(|note| note.ends_with(" [truncated]"))
        );
        assert!(
            value["risks"][0]["message"]
                .as_str()
                .is_some_and(|message| message.ends_with(" [truncated]"))
        );
        assert!(
            value["next_reads"][0]["reason"]
                .as_str()
                .is_some_and(|reason| reason.ends_with(" [truncated]"))
        );
        assert_eq!(
            value["risks"][1],
            json!({
                "kind": "unknown",
                "message": "Output truncated at 1200 chars: answer.summary x1, files.reason x1, symbols.relevance x1, evidence.note x1, risks.message x1, next_reads.reason x1."
            })
        );
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

    fn capped_model_json() -> String {
        let files = (0..45)
            .map(|index| {
                json!({
                    "path": format!("src/file-{index:02}.rs"),
                    "included": true,
                    "reason": format!("Reason {index:02}."),
                    "bytes": index + 1
                })
            })
            .collect::<Vec<_>>();
        let symbols = (0..85)
            .map(|index| {
                json!({
                    "name": format!("symbol_{index:02}"),
                    "kind": "function",
                    "path": format!("src/file-{:02}.rs", index % 45),
                    "relevance": format!("Relevance {index:02}.")
                })
            })
            .collect::<Vec<_>>();
        let evidence = (0..85)
            .map(|index| {
                json!({
                    "path": format!("src/file-{:02}.rs", index % 45),
                    "symbol": format!("symbol_{index:02}"),
                    "note": format!("Evidence {index:02}.")
                })
            })
            .collect::<Vec<_>>();
        let risks = (0..25)
            .map(|index| {
                json!({
                    "kind": "missing_context",
                    "message": format!("Risk {index:02}.")
                })
            })
            .collect::<Vec<_>>();
        let next_reads = (0..25)
            .map(|index| {
                json!({
                    "path": format!("src/next-{index:02}.rs"),
                    "reason": format!("Next read {index:02}.")
                })
            })
            .collect::<Vec<_>>();

        json!({
            "question": "What matters?",
            "answer": {
                "summary": "Small answer.",
                "confidence": "medium",
                "not_found": false
            },
            "files": files,
            "symbols": symbols,
            "evidence": evidence,
            "risks": risks,
            "next_reads": next_reads
        })
        .to_string()
    }

    fn truncated_model_json() -> String {
        let long_text = long_string(1300);

        json!({
            "question": "What matters?",
            "answer": {
                "summary": long_text,
                "confidence": "high",
                "not_found": false
            },
            "files": [
                {
                    "path": "src/auth.rs",
                    "included": true,
                    "reason": long_string(1301),
                    "bytes": 10
                }
            ],
            "symbols": [
                {
                    "name": "authenticate",
                    "kind": "function",
                    "path": "src/auth.rs",
                    "relevance": long_string(1302)
                }
            ],
            "evidence": [
                {
                    "path": "src/auth.rs",
                    "symbol": "authenticate",
                    "note": long_string(1303)
                }
            ],
            "risks": [
                {
                    "kind": "missing_context",
                    "message": long_string(1304)
                }
            ],
            "next_reads": [
                {
                    "path": "src/auth/tests.rs",
                    "reason": long_string(1305)
                }
            ]
        })
        .to_string()
    }

    fn long_string(len: usize) -> String {
        "x".repeat(len)
    }

    fn metadata() -> CommandMetadata {
        CommandMetadata::new(12420, 980)
    }
}
