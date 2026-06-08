use std::collections::BTreeMap;

/// Max model string chars.
pub(crate) const MAX_MODEL_STRING_CHARS: usize = 1200;
/// Truncation tail.
pub(crate) const TRUNCATION_MARKER: &str = " [truncated]";

#[derive(Debug, Default, PartialEq, Eq)]
/// Output change notes.
pub(crate) struct NormalizationNotes {
    /// Cap messages by field.
    caps: BTreeMap<&'static str, CapNote>,
    /// Truncation counts by field.
    truncations: BTreeMap<&'static str, usize>,
}

impl NormalizationNotes {
    /// True when cap notice row needed.
    pub(crate) fn has_caps(&self) -> bool {
        !self.caps.is_empty()
    }

    /// True when truncation notice row needed.
    pub(crate) fn has_truncations(&self) -> bool {
        !self.truncations.is_empty()
    }

    /// Count injected notice rows.
    pub(crate) fn notice_count(&self) -> usize {
        usize::from(self.has_caps()) + usize::from(self.has_truncations())
    }

    /// Record capped array.
    pub(crate) fn note_cap(&mut self, field: &'static str, before: usize, after: usize) {
        if before > after {
            self.caps
                .entry(field)
                .or_insert_with(CapNote::default_rows)
                .note(before, after);
        }
    }

    /// Record capped risk rows.
    pub(crate) fn note_risk_cap(&mut self, before: usize, kept: usize) {
        if before > kept {
            self.caps.insert(
                "risks",
                CapNote::custom(format!("risks kept {kept} of {before} model rows")),
            );
        }
    }

    /// Record truncated string.
    pub(crate) fn note_truncation(&mut self, field: &'static str) {
        *self.truncations.entry(field).or_default() += 1;
    }

    /// Format capped field summary in fixed order.
    pub(crate) fn cap_summary(&self, order: &[&str]) -> String {
        order
            .iter()
            .filter_map(|field| self.caps.get(field).map(|note| note.message(field)))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Format truncated field summary in fixed order.
    pub(crate) fn truncation_summary(&self, order: &[&str]) -> String {
        order
            .iter()
            .filter_map(|field| {
                self.truncations
                    .get(field)
                    .map(|count| format!("{field} x{count}"))
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Count model risks to keep before notice injection.
    pub(crate) fn risk_rows_to_keep(&self, cap: usize) -> usize {
        cap.saturating_sub(self.notice_count() + usize::from(!self.has_caps()))
    }
}

#[derive(Debug, PartialEq, Eq)]
/// Cap note state.
enum CapNote {
    /// Aggregated row caps.
    RowCounts {
        /// Total rows before cap.
        before_total: usize,
        /// Total rows kept.
        after_total: usize,
        /// Count capped items.
        hits: usize,
    },
    /// Fixed cap message.
    Custom(String),
}

impl CapNote {
    /// Build empty row-cap note.
    fn default_rows() -> Self {
        Self::RowCounts {
            before_total: 0,
            after_total: 0,
            hits: 0,
        }
    }

    /// Build custom note.
    fn custom(message: String) -> Self {
        Self::Custom(message)
    }

    /// Add capped rows.
    fn note(&mut self, before: usize, after: usize) {
        if let Self::RowCounts {
            before_total,
            after_total,
            hits,
        } = self
        {
            *before_total += before;
            *after_total += after;
            *hits += 1;
        }
    }

    /// Format cap note.
    fn message(&self, field: &str) -> String {
        match self {
            Self::RowCounts {
                before_total,
                after_total,
                hits,
            } if *hits == 1 => format!("{field} {before_total}->{after_total}"),
            Self::RowCounts {
                before_total,
                after_total,
                hits,
            } => format!("{field} kept {after_total} of {before_total} rows across {hits} items"),
            Self::Custom(message) => message.clone(),
        }
    }
}

/// Cap row count, note drop.
pub(crate) fn cap_rows<T>(
    rows: &mut Vec<T>,
    field: &'static str,
    cap: usize,
    notes: &mut NormalizationNotes,
) {
    let before = rows.len();
    if before > cap {
        rows.truncate(cap);
        notes.note_cap(field, before, cap);
    }
}

/// Truncate long model string.
pub(crate) fn truncate_model_string(
    value: &mut String,
    field: &'static str,
    notes: &mut NormalizationNotes,
) {
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

#[cfg(test)]
mod tests {
    #![allow(clippy::missing_panics_doc, reason = "test asserts stay local")]

    use super::{
        MAX_MODEL_STRING_CHARS, NormalizationNotes, TRUNCATION_MARKER, truncate_model_string,
    };

    #[test]
    fn char_safe_truncation_keeps_valid_utf8() {
        let mut value = "🙂".repeat(MAX_MODEL_STRING_CHARS + 50);
        let mut notes = NormalizationNotes::default();

        truncate_model_string(&mut value, "field", &mut notes);

        assert_eq!(value.chars().count(), MAX_MODEL_STRING_CHARS);
        assert!(value.ends_with(TRUNCATION_MARKER));
        assert_eq!(notes.truncation_summary(&["field"]), "field x1");
    }

    #[test]
    fn strings_at_cap_stay_unchanged() {
        let mut value = "x".repeat(MAX_MODEL_STRING_CHARS);
        let mut notes = NormalizationNotes::default();

        truncate_model_string(&mut value, "field", &mut notes);

        assert_eq!(value, "x".repeat(MAX_MODEL_STRING_CHARS));
        assert!(!notes.has_truncations());
    }

    #[test]
    fn strings_over_cap_get_marker_and_count() {
        let mut value = "x".repeat(MAX_MODEL_STRING_CHARS + 1);
        let mut notes = NormalizationNotes::default();

        truncate_model_string(&mut value, "field", &mut notes);

        assert_eq!(value.chars().count(), MAX_MODEL_STRING_CHARS);
        assert!(value.ends_with(TRUNCATION_MARKER));
        assert_eq!(notes.truncation_summary(&["field"]), "field x1");
    }
}
