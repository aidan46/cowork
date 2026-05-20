use std::{io, path::Path, process::ExitCode};

use serde::Serialize;
use thiserror::Error;

use crate::output::cli_command;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{message}")]
    InvalidArguments { message: String },
    #[error("path does not exist: {path}")]
    MissingPath { path: String },
    #[error("`--recursive` required for directory path: {path}")]
    DirectoryRequiresRecursive { path: String },
    #[error("failed to read file: {path}: {message}")]
    FileRead { path: String, message: String },
    #[error("input bytes exceed `--max-bytes`: {actual_bytes} > {max_bytes}")]
    MaxBytesExceeded {
        max_bytes: usize,
        actual_bytes: usize,
    },
    #[error("{message}")]
    ModelRequestFailed { message: String },
    #[error("{message}")]
    ResponseParseFailed { message: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoctorExit {
    Ok,
    InvalidConfig,
    BadHost,
    MissingModel,
    UnreachableHost,
    ProbeRequestFailed,
    InvalidProbeJson,
}

impl AppError {
    #[must_use]
    pub fn invalid_arguments(message: impl Into<String>) -> Self {
        Self::InvalidArguments {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn missing_path(path: &Path) -> Self {
        Self::MissingPath {
            path: path.display().to_string(),
        }
    }

    #[must_use]
    pub fn directory_requires_recursive(path: &Path) -> Self {
        Self::DirectoryRequiresRecursive {
            path: path.display().to_string(),
        }
    }

    #[must_use]
    pub fn file_read(path: &Path, error: &io::Error) -> Self {
        Self::FileRead {
            path: path.display().to_string(),
            message: error.to_string(),
        }
    }

    #[must_use]
    pub fn max_bytes_exceeded(max_bytes: usize, actual_bytes: usize) -> Self {
        Self::MaxBytesExceeded {
            max_bytes,
            actual_bytes,
        }
    }

    #[must_use]
    pub fn model_request_failed(message: impl Into<String>) -> Self {
        Self::ModelRequestFailed {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn response_parse_failed(message: impl Into<String>) -> Self {
        Self::ResponseParseFailed {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::InvalidArguments { .. } => ExitCode::from(1),
            Self::MissingPath { .. }
            | Self::DirectoryRequiresRecursive { .. }
            | Self::FileRead { .. } => ExitCode::from(2),
            Self::MaxBytesExceeded { .. } => ExitCode::from(3),
            Self::ModelRequestFailed { .. } => ExitCode::from(4),
            Self::ResponseParseFailed { .. } => ExitCode::from(5),
        }
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(&ErrorResponse::from(self)).expect("error response should serialize")
    }

    #[must_use]
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidArguments { .. } => "INVALID_ARGUMENTS",
            Self::MissingPath { .. } => "MISSING_PATH",
            Self::DirectoryRequiresRecursive { .. } => "DIRECTORY_REQUIRES_RECURSIVE",
            Self::FileRead { .. } => "FILE_READ_FAILED",
            Self::MaxBytesExceeded { .. } => "MAX_BYTES_EXCEEDED",
            Self::ModelRequestFailed { .. } => "MODEL_REQUEST_FAILED",
            Self::ResponseParseFailed { .. } => "RESPONSE_PARSE_FAILED",
        }
    }

    #[must_use]
    fn command(&self) -> &'static str {
        match self {
            Self::InvalidArguments { .. } => cli_command(),
            Self::MissingPath { .. }
            | Self::DirectoryRequiresRecursive { .. }
            | Self::FileRead { .. }
            | Self::MaxBytesExceeded { .. }
            | Self::ModelRequestFailed { .. }
            | Self::ResponseParseFailed { .. } => "ask",
        }
    }

    #[must_use]
    fn hint(&self) -> Option<&'static str> {
        match self {
            Self::InvalidArguments { .. } => Some("Use `cowork --help`."),
            Self::MissingPath { .. } => Some("Check path spelling and current dir."),
            Self::DirectoryRequiresRecursive { .. } => {
                Some("Pass `--recursive` for directory inputs.")
            }
            Self::FileRead { .. } => Some("Check file exists and is readable."),
            Self::MaxBytesExceeded { .. } => Some("Pass larger `--max-bytes` or fewer files."),
            Self::ModelRequestFailed { .. } => Some("Check Ollama server, model, and `--host`."),
            Self::ResponseParseFailed { .. } => Some("Check Ollama response and ask JSON schema."),
        }
    }
}

impl DoctorExit {
    #[must_use]
    pub fn exit_code(self) -> ExitCode {
        match self {
            Self::Ok => ExitCode::SUCCESS,
            Self::InvalidConfig => ExitCode::from(1),
            Self::BadHost => ExitCode::from(6),
            Self::MissingModel => ExitCode::from(7),
            Self::UnreachableHost => ExitCode::from(8),
            Self::ProbeRequestFailed => ExitCode::from(9),
            Self::InvalidProbeJson => ExitCode::from(10),
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorResponse<'a> {
    schema_version: &'a str,
    command: &'a str,
    status: &'a str,
    error: ErrorBody<'a>,
}

#[derive(Debug, Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<&'a str>,
}

impl<'a> From<&'a AppError> for ErrorResponse<'a> {
    fn from(error: &'a AppError) -> Self {
        Self {
            schema_version: "1.0",
            command: error.command(),
            status: "error",
            error: ErrorBody {
                code: error.code(),
                message: error.to_string(),
                hint: error.hint(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::process::ExitCode;

    use serde_json::json;

    use super::{AppError, DoctorExit};

    #[test]
    fn invalid_arguments_maps_to_exit_code_one() {
        assert_eq!(
            AppError::invalid_arguments("bad flag").exit_code(),
            ExitCode::from(1)
        );
    }

    #[test]
    fn invalid_arguments_serialize_as_cli_error() {
        let value = serde_json::from_str::<serde_json::Value>(
            &AppError::invalid_arguments("bad flag").to_json(),
        )
        .expect("error json should parse");

        assert_eq!(
            value,
            json!({
                "schema_version": "1.0",
                "command": "cli",
                "status": "error",
                "error": {
                    "code": "INVALID_ARGUMENTS",
                    "message": "bad flag",
                    "hint": "Use `cowork --help`."
                }
            })
        );
    }

    #[test]
    fn missing_path_maps_to_exit_code_two() {
        assert_eq!(
            AppError::missing_path(std::path::Path::new("nope")).exit_code(),
            ExitCode::from(2)
        );
    }

    #[test]
    fn dir_requires_recursive_serializes_as_json_error() {
        let value = serde_json::from_str::<serde_json::Value>(
            &AppError::directory_requires_recursive(std::path::Path::new("src")).to_json(),
        )
        .expect("error json should parse");

        assert_eq!(
            value,
            json!({
                "schema_version": "1.0",
                "command": "ask",
                "status": "error",
                "error": {
                    "code": "DIRECTORY_REQUIRES_RECURSIVE",
                    "message": "`--recursive` required for directory path: src",
                    "hint": "Pass `--recursive` for directory inputs."
                }
            })
        );
    }

    #[test]
    fn max_bytes_exceeded_maps_to_exit_code_three() {
        assert_eq!(
            AppError::max_bytes_exceeded(1, 2).exit_code(),
            ExitCode::from(3)
        );
    }

    #[test]
    fn max_bytes_exceeded_serializes_as_json_error() {
        let value = serde_json::from_str::<serde_json::Value>(
            &AppError::max_bytes_exceeded(1, 2).to_json(),
        )
        .expect("error json should parse");

        assert_eq!(
            value,
            json!({
                "schema_version": "1.0",
                "command": "ask",
                "status": "error",
                "error": {
                    "code": "MAX_BYTES_EXCEEDED",
                    "message": "input bytes exceed `--max-bytes`: 2 > 1",
                    "hint": "Pass larger `--max-bytes` or fewer files."
                }
            })
        );
    }

    #[test]
    fn model_request_failed_maps_to_exit_code_four() {
        assert_eq!(
            AppError::model_request_failed("down").exit_code(),
            ExitCode::from(4)
        );
    }

    #[test]
    fn response_parse_failed_maps_to_exit_code_five() {
        assert_eq!(
            AppError::response_parse_failed("bad json").exit_code(),
            ExitCode::from(5)
        );
    }

    #[test]
    fn doctor_bad_host_maps_to_exit_code_six() {
        assert_eq!(DoctorExit::BadHost.exit_code(), ExitCode::from(6));
    }

    #[test]
    fn doctor_missing_model_maps_to_exit_code_seven() {
        assert_eq!(DoctorExit::MissingModel.exit_code(), ExitCode::from(7));
    }

    #[test]
    fn doctor_unreachable_host_maps_to_exit_code_eight() {
        assert_eq!(DoctorExit::UnreachableHost.exit_code(), ExitCode::from(8));
    }

    #[test]
    fn doctor_invalid_probe_json_maps_to_exit_code_ten() {
        assert_eq!(DoctorExit::InvalidProbeJson.exit_code(), ExitCode::from(10));
    }
}
