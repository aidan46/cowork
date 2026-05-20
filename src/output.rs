use serde::{Deserialize, Serialize};

use crate::error::AppError;

const SCHEMA_VERSION: &str = "1.0";
const ASK_COMMAND: &str = "ask";
const CLI_COMMAND: &str = "cli";
const DOCTOR_COMMAND: &str = "doctor";
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

#[must_use]
pub(crate) fn cli_command() -> &'static str {
    CLI_COMMAND
}

#[must_use]
pub(crate) fn render_init_rules(agent: &str) -> &'static str {
    match agent {
        "codex" => CODEX_INIT_RULES,
        "claude" => CLAUDE_INIT_RULES,
        _ => unreachable!("unsupported init agent"),
    }
}

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

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct DoctorOutput {
    schema_version: &'static str,
    command: &'static str,
    status: &'static str,
    checks: Vec<DoctorCheck>,
}

impl DoctorOutput {
    #[must_use]
    pub(crate) fn ok(checks: Vec<DoctorCheck>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            command: DOCTOR_COMMAND,
            status: STATUS_OK,
            checks,
        }
    }

    #[must_use]
    pub(crate) fn error(checks: Vec<DoctorCheck>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            command: DOCTOR_COMMAND,
            status: STATUS_ERROR,
            checks,
        }
    }

    #[must_use]
    pub(crate) fn to_json(&self) -> String {
        serde_json::to_string(self).expect("doctor output should serialize")
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct DoctorCheck {
    name: &'static str,
    status: DoctorCheckStatus,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<String>,
}

impl DoctorCheck {
    #[must_use]
    pub(crate) fn ok(name: &'static str, message: impl Into<String>) -> Self {
        Self {
            name,
            status: DoctorCheckStatus::Ok,
            message: message.into(),
            hint: None,
        }
    }

    #[must_use]
    pub(crate) fn error(
        name: &'static str,
        message: impl Into<String>,
        hint: Option<&str>,
    ) -> Self {
        Self {
            name,
            status: DoctorCheckStatus::Error,
            message: message.into(),
            hint: hint.map(str::to_string),
        }
    }

    #[must_use]
    pub(crate) fn skipped(name: &'static str, message: impl Into<String>) -> Self {
        Self {
            name,
            status: DoctorCheckStatus::Skipped,
            message: message.into(),
            hint: None,
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DoctorCheckStatus {
    Ok,
    Error,
    Skipped,
}

#[derive(Debug, Deserialize)]
struct RawDoctorProbe {
    ok: bool,
}

pub(crate) fn parse_doctor_probe(raw_json: &str) -> Result<(), String> {
    let probe = serde_json::from_str::<RawDoctorProbe>(raw_json)
        .map_err(|error| format!("failed to parse doctor probe JSON: {error}"))?;

    if probe.ok {
        return Ok(());
    }

    Err("doctor probe JSON missing `ok = true`".to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        DoctorCheck, DoctorOutput, parse_ask_output, parse_doctor_probe, render_init_rules,
    };
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

    #[test]
    fn doctor_output_serializes_fixed_top_level_fields() {
        let output = DoctorOutput::ok(vec![
            DoctorCheck::ok("config_files_loaded", "Loaded 1 config file."),
            DoctorCheck::ok("effective_model_chosen", "Using model `gemma3:12b`."),
        ]);
        let value = serde_json::from_str::<serde_json::Value>(&output.to_json())
            .expect("json should parse");

        assert_eq!(
            value,
            json!({
                "schema_version": "1.0",
                "command": "doctor",
                "status": "ok",
                "checks": [
                    {
                        "name": "config_files_loaded",
                        "status": "ok",
                        "message": "Loaded 1 config file."
                    },
                    {
                        "name": "effective_model_chosen",
                        "status": "ok",
                        "message": "Using model `gemma3:12b`."
                    }
                ]
            })
        );
    }

    #[test]
    fn doctor_probe_parser_requires_ok_true() {
        let error = parse_doctor_probe(r#"{"ready":true}"#).expect_err("probe should fail");

        assert!(error.contains("failed to parse doctor probe JSON"));
    }

    #[test]
    fn codex_init_rules_keep_required_markers() {
        let rules = render_init_rules("codex");

        assert!(rules.contains("# cowork rules for Codex"));
        assert!(rules.contains("cowork ask"));
        assert!(rules.contains("cowork doctor"));
        assert!(rules.contains("next_reads"));
        assert!(rules.contains("lead, not authority"));
    }

    #[test]
    fn claude_init_rules_keep_required_markers() {
        let rules = render_init_rules("claude");

        assert!(rules.contains("# cowork rules for Claude"));
        assert!(rules.contains("cowork ask"));
        assert!(rules.contains("cowork doctor"));
        assert!(rules.contains("next_reads"));
        assert!(rules.contains("lead, not authority"));
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
