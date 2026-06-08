use serde::{Deserialize, Serialize};

use crate::error::AppError;

use super::{
    BRIEF_COMMAND, CommandMetadata, SCHEMA_VERSION, STATUS_OK,
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
/// Max key points per file.
const MAX_KEY_POINTS: usize = 20;
/// Cap notice field order.
const CAP_FIELD_ORDER: [&str; 6] = [
    "files",
    "files.key_points",
    "symbols",
    "evidence",
    "risks",
    "next_reads",
];
/// Truncation notice field order.
const TRUNCATION_FIELD_ORDER: [&str; 13] = [
    "brief.summary",
    "files.path",
    "files.role",
    "files.key_points",
    "symbols.name",
    "symbols.path",
    "symbols.responsibility",
    "evidence.path",
    "evidence.symbol",
    "evidence.note",
    "risks.message",
    "next_reads.path",
    "next_reads.reason",
];

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
    fn from_raw(mut value: RawBriefOutput, goal: &str, metadata: CommandMetadata) -> Self {
        let mut notes = NormalizationNotes::default();

        value.brief.normalize(&mut notes);
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

impl BriefSummary {
    /// Truncate model string fields.
    fn normalize(&mut self, notes: &mut NormalizationNotes) {
        truncate_model_string(&mut self.summary, "brief.summary", notes);
    }
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
    /// Truncate model string fields.
    fn normalize(&mut self, notes: &mut NormalizationNotes) {
        truncate_model_string(&mut self.path, "files.path", notes);
        truncate_model_string(&mut self.role, "files.role", notes);
        self.key_points
            .iter_mut()
            .for_each(|point| truncate_model_string(point, "files.key_points", notes));
        cap_rows(
            &mut self.key_points,
            "files.key_points",
            MAX_KEY_POINTS,
            notes,
        );
    }

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
    /// Truncate model string fields.
    fn normalize(&mut self, notes: &mut NormalizationNotes) {
        truncate_model_string(&mut self.name, "symbols.name", notes);
        truncate_model_string(&mut self.path, "symbols.path", notes);
        truncate_model_string(&mut self.responsibility, "symbols.responsibility", notes);
    }

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
    /// Truncate model string fields.
    fn normalize(&mut self, notes: &mut NormalizationNotes) {
        truncate_model_string(&mut self.path, "evidence.path", notes);
        truncate_model_string(&mut self.symbol, "evidence.symbol", notes);
        truncate_model_string(&mut self.note, "evidence.note", notes);
    }

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
    /// Truncate model string fields.
    fn normalize(&mut self, notes: &mut NormalizationNotes) {
        truncate_model_string(&mut self.message, "risks.message", notes);
    }

    /// Build output notice row.
    fn output_notice(message: String) -> Self {
        Self {
            kind: BriefRiskKind::Unknown,
            message,
        }
    }

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
    /// Truncate model string fields.
    fn normalize(&mut self, notes: &mut NormalizationNotes) {
        truncate_model_string(&mut self.path, "next_reads.path", notes);
        truncate_model_string(&mut self.reason, "next_reads.reason", notes);
    }

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

/// Append CLI notice risks.
fn inject_notice_risks(risks: &mut Vec<BriefRisk>, notes: &NormalizationNotes) {
    if notes.has_caps() {
        risks.push(BriefRisk::output_notice(format!(
            "Output capped: {}.",
            notes.cap_summary(&CAP_FIELD_ORDER)
        )));
    }

    if notes.has_truncations() {
        risks.push(BriefRisk::output_notice(format!(
            "Output truncated at {MAX_MODEL_STRING_CHARS} chars: {}.",
            notes.truncation_summary(&TRUNCATION_FIELD_ORDER)
        )));
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
mod tests;
