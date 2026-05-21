use serde::{Deserialize, Serialize};

use super::{DOCTOR_COMMAND, SCHEMA_VERSION, STATUS_ERROR, STATUS_OK};

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
enum DoctorCheckStatus {
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
