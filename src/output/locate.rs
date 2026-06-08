use serde::{Deserialize, Serialize};

use crate::error::AppError;

use super::{
    CommandId, CommandMetadata, SCHEMA_VERSION, STATUS_OK,
    bounds::{MAX_MODEL_STRING_CHARS, NormalizationNotes, cap_rows, truncate_model_string},
};

/// Max match rows.
const MAX_MATCHES: usize = 80;
/// Max risk rows.
const MAX_RISKS: usize = 20;
/// Max next-read rows.
const MAX_NEXT_READS: usize = 20;
/// Cap notice field order.
const CAP_FIELD_ORDER: [&str; 3] = ["matches", "risks", "next_reads"];
/// Truncation notice field order.
const TRUNCATION_FIELD_ORDER: [&str; 6] = [
    "matches.path",
    "matches.symbol",
    "matches.reason",
    "next_reads.path",
    "next_reads.reason",
    "risks.message",
];

#[derive(Debug, Serialize, PartialEq, Eq)]
/// Locate command output.
pub(crate) struct LocateOutput {
    /// JSON schema version.
    schema_version: &'static str,
    /// Command tag.
    command: CommandId,
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
    fn from_raw(mut value: RawLocateOutput, metadata: CommandMetadata) -> Self {
        let mut notes = NormalizationNotes::default();

        value
            .matches
            .iter_mut()
            .for_each(|item| item.normalize(&mut notes));
        value
            .next_reads
            .iter_mut()
            .for_each(|item| item.normalize(&mut notes));
        value
            .risks
            .iter_mut()
            .for_each(|risk| risk.normalize(&mut notes));

        let mut matches = value.matches;
        let mut next_reads = value.next_reads;
        let mut risks = value.risks;

        matches.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        matches.dedup();
        cap_rows(&mut matches, "matches", MAX_MATCHES, &mut notes);
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
            command: CommandId::Locate,
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
    /// Truncate model string fields.
    fn normalize(&mut self, notes: &mut NormalizationNotes) {
        truncate_model_string(&mut self.path, "matches.path", notes);
        if let Some(symbol) = &mut self.symbol {
            truncate_model_string(symbol, "matches.symbol", notes);
        }
        truncate_model_string(&mut self.reason, "matches.reason", notes);
    }

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
/// One locate risk row.
struct LocateRisk {
    /// Risk kind.
    kind: LocateRiskKind,
    /// Risk message.
    message: String,
}

impl LocateRisk {
    /// Truncate model string fields.
    fn normalize(&mut self, notes: &mut NormalizationNotes) {
        truncate_model_string(&mut self.message, "risks.message", notes);
    }

    /// Build output notice row.
    fn output_notice(message: String) -> Self {
        Self {
            kind: LocateRiskKind::Unknown,
            message,
        }
    }

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

/// Append CLI notice risks.
fn inject_notice_risks(risks: &mut Vec<LocateRisk>, notes: &NormalizationNotes) {
    if notes.has_caps() {
        risks.push(LocateRisk::output_notice(format!(
            "Output capped: {}.",
            notes.cap_summary(&CAP_FIELD_ORDER)
        )));
    }

    if notes.has_truncations() {
        risks.push(LocateRisk::output_notice(format!(
            "Output truncated at {MAX_MODEL_STRING_CHARS} chars: {}.",
            notes.truncation_summary(&TRUNCATION_FIELD_ORDER)
        )));
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
mod tests;
