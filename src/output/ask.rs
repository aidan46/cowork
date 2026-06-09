use serde::{Deserialize, Serialize};

use crate::error::AppError;

use super::{
    CommandId, CommandMetadata, SCHEMA_VERSION, STATUS_OK,
    bounds::{MAX_MODEL_STRING_CHARS, NormalizationNotes, cap_rows, truncate_model_string},
};

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
    command: CommandId,
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
            let risk_keep = notes.risk_rows_to_keep(MAX_RISKS);
            notes.note_risk_cap(risks.len(), risk_keep);
            risks.truncate(risk_keep);
        }
        inject_notice_risks(&mut risks, &notes);
        risks.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        risks.dedup();

        Self {
            schema_version: SCHEMA_VERSION,
            command: CommandId::Ask,
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

/// Append CLI notice risks.
fn inject_notice_risks(risks: &mut Vec<AskRisk>, notes: &NormalizationNotes) {
    if notes.has_caps() {
        risks.push(AskRisk::output_notice(format!(
            "Output capped: {}.",
            notes.cap_summary(&CAP_FIELD_ORDER)
        )));
    }

    if notes.has_truncations() {
        risks.push(AskRisk::output_notice(format!(
            "Output truncated at {MAX_MODEL_STRING_CHARS} chars: {}.",
            notes.truncation_summary(&TRUNCATION_FIELD_ORDER)
        )));
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
mod tests;
