use std::{path::Path, process::ExitCode};

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
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::InvalidArguments { .. } | Self::AskNotImplemented => ExitCode::from(1),
            Self::MissingPath { .. } | Self::DirectoryRequiresRecursive { .. } => ExitCode::from(2),
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
}
