use serde::Serialize;

use crate::error::AppError;

use super::{SCHEMA_VERSION, SETUP_COMMAND};

#[derive(Debug, Serialize, PartialEq, Eq)]
/// Setup command output.
pub(crate) struct SetupOutput {
    /// JSON schema version.
    schema_version: &'static str,
    /// Command tag.
    command: &'static str,
    /// Output status.
    status: SetupStatus,
    /// Check list.
    checks: Vec<SetupCheck>,
    /// Model recommendation.
    recommendation: Option<SetupRecommendation>,
    /// Action list.
    actions: Vec<SetupAction>,
    /// Config plan.
    config: Option<SetupConfig>,
    /// Setup metadata.
    metadata: SetupMetadata,
}

impl SetupOutput {
    #[must_use]
    /// Build success output.
    pub(crate) fn ok(checks: Vec<SetupCheck>) -> Self {
        Self::new(SetupStatus::Ok, checks)
    }

    #[must_use]
    /// Build warning output.
    pub(crate) fn warning(checks: Vec<SetupCheck>) -> Self {
        Self::new(SetupStatus::Warning, checks)
    }

    #[must_use]
    /// Build error output.
    #[cfg(test)]
    pub(crate) fn error(checks: Vec<SetupCheck>) -> Self {
        Self::new(SetupStatus::Error, checks)
    }

    #[must_use]
    /// Set recommendation.
    pub(crate) fn with_recommendation(mut self, recommendation: SetupRecommendation) -> Self {
        self.recommendation = Some(recommendation);
        self
    }

    #[must_use]
    /// Set actions.
    pub(crate) fn with_actions(mut self, actions: Vec<SetupAction>) -> Self {
        self.actions = actions;
        self
    }

    #[must_use]
    /// Set config plan.
    pub(crate) fn with_config(mut self, config: SetupConfig) -> Self {
        self.config = Some(config);
        self
    }

    #[must_use]
    /// Set metadata.
    pub(crate) fn with_metadata(mut self, metadata: SetupMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Serialize output to JSON.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] when JSON serialization fails.
    #[allow(
        clippy::wrong_self_convention,
        reason = "final byte count mutates copy"
    )]
    pub(crate) fn to_json(mut self) -> Result<String, AppError> {
        loop {
            let json = serde_json::to_string(&self).map_err(|error| {
                AppError::response_parse_failed(format!(
                    "failed to serialize setup output: {error}"
                ))
            })?;
            let output_bytes = json.len();

            if self.metadata.output_bytes == Some(output_bytes) {
                return Ok(json);
            }

            if self.metadata.output_bytes.is_none() {
                return Ok(json);
            }

            self.metadata.set_output_bytes(output_bytes);
        }
    }

    #[must_use]
    /// Build output row.
    fn new(status: SetupStatus, checks: Vec<SetupCheck>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            command: SETUP_COMMAND,
            status,
            checks,
            recommendation: None,
            actions: Vec::new(),
            config: None,
            metadata: SetupMetadata::default(),
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
/// One setup check row.
pub(crate) struct SetupCheck {
    /// Check name.
    name: &'static str,
    /// Check status.
    status: SetupStatus,
    /// Check message.
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional fix hint.
    hint: Option<String>,
}

impl SetupCheck {
    #[must_use]
    /// Build success check.
    pub(crate) fn ok(name: &'static str, message: impl Into<String>) -> Self {
        Self::new(name, SetupStatus::Ok, message, None)
    }

    #[must_use]
    /// Build warning check.
    pub(crate) fn warning(
        name: &'static str,
        message: impl Into<String>,
        hint: Option<&str>,
    ) -> Self {
        Self::new(name, SetupStatus::Warning, message, hint)
    }

    #[must_use]
    /// Build error check.
    pub(crate) fn error(
        name: &'static str,
        message: impl Into<String>,
        hint: Option<&str>,
    ) -> Self {
        Self::new(name, SetupStatus::Error, message, hint)
    }

    #[must_use]
    /// Build skipped check.
    #[cfg(test)]
    pub(crate) fn skipped(name: &'static str, message: impl Into<String>) -> Self {
        Self::new(name, SetupStatus::Skipped, message, None)
    }

    #[must_use]
    /// Build check row.
    fn new(
        name: &'static str,
        status: SetupStatus,
        message: impl Into<String>,
        hint: Option<&str>,
    ) -> Self {
        Self {
            name,
            status,
            message: message.into(),
            hint: hint.map(str::to_string),
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
/// Setup recommendation row.
pub(crate) struct SetupRecommendation {
    /// Model name.
    model: String,
    /// Pull required.
    needs_pull: bool,
    /// Recommendation reason.
    reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Confidence tag.
    confidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Hardware class tag.
    hardware_class: Option<String>,
}

impl SetupRecommendation {
    #[must_use]
    /// Build recommendation row.
    pub(crate) fn new(
        model: impl Into<String>,
        needs_pull: bool,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            model: model.into(),
            needs_pull,
            reason: reason.into(),
            confidence: None,
            hardware_class: None,
        }
    }

    #[must_use]
    /// Set confidence tag.
    pub(crate) fn with_confidence(mut self, confidence: impl Into<String>) -> Self {
        self.confidence = Some(confidence.into());
        self
    }

    #[must_use]
    /// Set hardware class tag.
    pub(crate) fn with_hardware_class(mut self, hardware_class: impl Into<String>) -> Self {
        self.hardware_class = Some(hardware_class.into());
        self
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
/// Setup action row.
pub(crate) struct SetupAction {
    /// Action name.
    name: &'static str,
    /// Action status.
    status: SetupStatus,
    /// Action message.
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional model name.
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional path.
    path: Option<String>,
}

impl SetupAction {
    #[must_use]
    /// Build action row.
    pub(crate) fn new(name: &'static str, status: SetupStatus, message: impl Into<String>) -> Self {
        Self {
            name,
            status,
            message: message.into(),
            model: None,
            path: None,
        }
    }

    #[must_use]
    /// Set model name.
    pub(crate) fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    #[must_use]
    /// Set path.
    #[cfg(test)]
    pub(crate) fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
/// Setup config row.
pub(crate) struct SetupConfig {
    /// Config target.
    target: String,
    /// Config path.
    path: String,
    /// Write flag.
    write_requested: bool,
    /// Force flag.
    force: bool,
}

impl SetupConfig {
    #[must_use]
    /// Build config row.
    pub(crate) fn new(
        target: impl Into<String>,
        path: impl Into<String>,
        write_requested: bool,
        force: bool,
    ) -> Self {
        Self {
            target: target.into(),
            path: path.into(),
            write_requested,
            force,
        }
    }
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
/// Setup metadata row.
pub(crate) struct SetupMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Command time in ms.
    duration_ms: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Final JSON byte count.
    output_bytes: Option<usize>,
}

impl SetupMetadata {
    #[must_use]
    /// Build timed metadata.
    pub(crate) fn timed(duration_ms: usize) -> Self {
        Self {
            duration_ms: Some(duration_ms),
            output_bytes: Some(0),
        }
    }

    /// Set final JSON size.
    fn set_output_bytes(&mut self, output_bytes: usize) {
        self.output_bytes = Some(output_bytes);
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Setup state.
pub(crate) enum SetupStatus {
    /// Row passed.
    Ok,
    /// Row warns.
    Warning,
    /// Row failed.
    Error,
    /// Row skipped.
    Skipped,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::missing_errors_doc, reason = "test helpers stay local")]
    #![allow(clippy::missing_panics_doc, reason = "test asserts stay local")]

    use serde_json::json;

    use super::{
        SetupAction, SetupCheck, SetupConfig, SetupMetadata, SetupOutput, SetupRecommendation,
        SetupStatus,
    };

    #[test]
    fn setup_output_serializes_fixed_top_level_fields() {
        let json = SetupOutput::ok(Vec::new())
            .to_json()
            .expect("output should serialize");

        assert_eq!(
            json,
            concat!(
                "{\"schema_version\":\"1.0\",\"command\":\"setup\",\"status\":\"ok\",",
                "\"checks\":[],\"recommendation\":null,\"actions\":[],",
                "\"config\":null,\"metadata\":{}}"
            )
        );
    }

    #[test]
    fn setup_output_serializes_happy_path_rows() {
        let json = SetupOutput::ok(vec![SetupCheck::ok(
            "ollama_detected",
            "Found Ollama on localhost.",
        )])
        .with_recommendation(
            SetupRecommendation::new("gemma3:12b", true, "Fits local hardware.")
                .with_confidence("high")
                .with_hardware_class("16gb_gpu"),
        )
        .with_actions(vec![
            SetupAction::new(
                "config_write_skipped",
                SetupStatus::Skipped,
                "Dry run kept config unchanged.",
            )
            .with_model("gemma3:12b")
            .with_path("/tmp/cowork.toml"),
        ])
        .with_config(SetupConfig::new("user", "/tmp/cowork.toml", false, false))
        .with_metadata(SetupMetadata::timed(12))
        .to_json()
        .expect("output should serialize");
        let value = serde_json::from_str::<serde_json::Value>(&json).expect("json should parse");

        assert_eq!(value["schema_version"], "1.0");
        assert_eq!(value["command"], "setup");
        assert_eq!(value["status"], "ok");
        assert_eq!(
            value["checks"],
            json!([{
                "name": "ollama_detected",
                "status": "ok",
                "message": "Found Ollama on localhost."
            }])
        );
        assert_eq!(
            value["recommendation"],
            json!({
                "model": "gemma3:12b",
                "needs_pull": true,
                "reason": "Fits local hardware.",
                "confidence": "high",
                "hardware_class": "16gb_gpu"
            })
        );
        assert_eq!(
            value["actions"],
            json!([{
                "name": "config_write_skipped",
                "status": "skipped",
                "message": "Dry run kept config unchanged.",
                "model": "gemma3:12b",
                "path": "/tmp/cowork.toml"
            }])
        );
        assert_eq!(
            value["config"],
            json!({
                "target": "user",
                "path": "/tmp/cowork.toml",
                "write_requested": false,
                "force": false
            })
        );
        assert_eq!(value["metadata"]["duration_ms"], 12);
        assert!(value["metadata"]["output_bytes"].as_u64().unwrap_or(0) > 0);
    }

    #[test]
    fn warning_output_serializes_warning_status() {
        let value = serde_json::from_str::<serde_json::Value>(
            &SetupOutput::warning(vec![SetupCheck::warning(
                "models_listed",
                "No installed models found.",
                Some("Run pull later."),
            )])
            .to_json()
            .expect("output should serialize"),
        )
        .expect("json should parse");

        assert_eq!(value["status"], "warning");
    }

    #[test]
    fn skipped_check_serializes_skipped_status() {
        let value = serde_json::from_str::<serde_json::Value>(
            &SetupOutput::ok(vec![SetupCheck::skipped(
                "model_pull_skipped",
                "Pull disabled.",
            )])
            .to_json()
            .expect("output should serialize"),
        )
        .expect("json should parse");

        assert_eq!(value["checks"][0]["status"], "skipped");
    }

    #[test]
    fn optional_fields_are_omitted_when_none() {
        let value = serde_json::from_str::<serde_json::Value>(
            &SetupOutput::ok(vec![SetupCheck::ok(
                "doctor_probe_passed",
                "Probe returned ok.",
            )])
            .with_recommendation(SetupRecommendation::new(
                "gemma3:12b",
                false,
                "Already installed.",
            ))
            .with_actions(vec![SetupAction::new(
                "models_listed",
                SetupStatus::Ok,
                "Listed installed models.",
            )])
            .to_json()
            .expect("output should serialize"),
        )
        .expect("json should parse");

        assert!(value["checks"][0].get("hint").is_none());
        assert!(value["recommendation"].get("confidence").is_none());
        assert!(value["recommendation"].get("hardware_class").is_none());
        assert!(value["actions"][0].get("model").is_none());
        assert!(value["actions"][0].get("path").is_none());
        assert!(value["metadata"].get("duration_ms").is_none());
        assert!(value["metadata"].get("output_bytes").is_none());
    }

    #[test]
    fn output_byte_metadata_updates_after_final_serialization() {
        let json = SetupOutput::error(vec![SetupCheck::error(
            "ollama_detected",
            "Ollama missing.",
            Some("Install Ollama first."),
        )])
        .with_metadata(SetupMetadata::timed(25))
        .to_json()
        .expect("output should serialize");
        let value = serde_json::from_str::<serde_json::Value>(&json).expect("json should parse");

        assert_eq!(value["metadata"]["duration_ms"], 25);
        assert_eq!(value["metadata"]["output_bytes"], json.len());
    }
}
