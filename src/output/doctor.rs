use serde::{Deserialize, Serialize};

use super::{DOCTOR_COMMAND, SCHEMA_VERSION, STATUS_ERROR, STATUS_OK};

#[derive(Debug, Serialize, PartialEq, Eq)]
/// Doctor command output.
pub(crate) struct DoctorOutput {
    /// JSON schema version.
    schema_version: &'static str,
    /// Command tag.
    command: &'static str,
    /// Output status.
    status: &'static str,
    /// Check list.
    checks: Vec<DoctorCheck>,
}

impl DoctorOutput {
    #[must_use]
    /// Build success output.
    pub(crate) fn ok(checks: Vec<DoctorCheck>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            command: DOCTOR_COMMAND,
            status: STATUS_OK,
            checks,
        }
    }

    #[must_use]
    /// Build error output.
    pub(crate) fn error(checks: Vec<DoctorCheck>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            command: DOCTOR_COMMAND,
            status: STATUS_ERROR,
            checks,
        }
    }

    #[must_use]
    /// Serialize output to JSON.
    pub(crate) fn to_json(&self) -> String {
        serde_json::to_string(self).expect("doctor output should serialize")
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
/// One doctor check row.
pub(crate) struct DoctorCheck {
    /// Check name.
    name: &'static str,
    /// Check status.
    status: DoctorCheckStatus,
    /// Check message.
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional fix hint.
    hint: Option<String>,
}

impl DoctorCheck {
    #[must_use]
    /// Build success check.
    pub(crate) fn ok(name: &'static str, message: impl Into<String>) -> Self {
        Self {
            name,
            status: DoctorCheckStatus::Ok,
            message: message.into(),
            hint: None,
        }
    }

    #[must_use]
    /// Build error check.
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
    /// Build skipped check.
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
/// Doctor check state.
enum DoctorCheckStatus {
    /// Check passed.
    Ok,
    /// Check failed.
    Error,
    /// Check skipped.
    Skipped,
}

#[derive(Debug, Deserialize)]
/// Raw doctor probe JSON.
struct RawDoctorProbe {
    /// Probe success flag.
    ok: bool,
}

/// Parse doctor probe JSON.
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

    use super::{DoctorCheck, DoctorOutput, parse_doctor_probe};

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
}
