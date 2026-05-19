use std::{io, path::Path, process::ExitCode};

use serde::Serialize;
use thiserror::Error;

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
    #[error("`cowork ask` not implemented yet")]
    AskNotImplemented,
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
            Self::InvalidArguments { .. } | Self::AskNotImplemented => ExitCode::from(1),
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
            Self::AskNotImplemented => "ASK_NOT_IMPLEMENTED",
        }
    }

    #[must_use]
    fn command(&self) -> &'static str {
        "ask"
    }

    #[must_use]
    fn hint(&self) -> Option<&'static str> {
        match self {
            Self::InvalidArguments { .. } => Some("Use `cowork ask --help`."),
            Self::MissingPath { .. } => Some("Check path spelling and current dir."),
            Self::DirectoryRequiresRecursive { .. } => {
                Some("Pass `--recursive` for directory inputs.")
            }
            Self::FileRead { .. } => Some("Check file exists and is readable."),
            Self::MaxBytesExceeded { .. } => Some("Pass larger `--max-bytes` or fewer files."),
            Self::ModelRequestFailed { .. } => Some("Check Ollama server, model, and `--host`."),
            Self::ResponseParseFailed { .. } => Some("Check Ollama response envelope."),
            Self::AskNotImplemented => None,
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

    use super::AppError;

    #[test]
    fn invalid_arguments_maps_to_exit_code_one() {
        assert_eq!(
            AppError::invalid_arguments("bad flag").exit_code(),
            ExitCode::from(1)
        );
    }

    #[test]
    fn ask_not_implemented_serializes_as_json_error() {
        let value =
            serde_json::from_str::<serde_json::Value>(&AppError::AskNotImplemented.to_json())
                .expect("error json should parse");

        assert_eq!(
            value,
            json!({
                "schema_version": "1.0",
                "command": "ask",
                "status": "error",
                "error": {
                    "code": "ASK_NOT_IMPLEMENTED",
                    "message": "`cowork ask` not implemented yet"
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
}
