use std::{io, path::Path, process::ExitCode};

use serde_json::{Map, Value};
use thiserror::Error;

use crate::output::CommandId;

#[derive(Debug, Error)]
/// App error surface.
pub enum AppError {
    #[error("{message}")]
    /// Invalid CLI or config input.
    InvalidArguments {
        /// Error message.
        message: String,
    },
    #[error("{source}")]
    /// Subcommand tag wraps source error.
    CommandContext {
        /// Command tag.
        command: CommandId,
        /// Wrapped source error.
        source: Box<Self>,
    },
    #[error("path does not exist: {path}")]
    /// Input path missing.
    MissingPath {
        /// Missing path text.
        path: String,
    },
    #[error("`--recursive` required for directory path: {path}")]
    /// Dir input needs `--recursive`.
    DirectoryRequiresRecursive {
        /// Dir path text.
        path: String,
    },
    #[error("failed to read file: {path}: {message}")]
    /// File read failed.
    FileRead {
        /// File path text.
        path: String,
        /// IO error text.
        message: String,
    },
    #[error("failed to update init file: {path}: {message}")]
    /// Init file update failed.
    InitFileUpdate {
        /// Target path text.
        path: String,
        /// Update error text.
        message: String,
    },
    #[error("no readable UTF-8 text input files found")]
    /// No text input files survived load.
    NoInputFiles,
    #[error("input bytes exceed `--max-bytes`: {actual_bytes} > {max_bytes}")]
    /// Loaded bytes exceed cap.
    MaxBytesExceeded {
        /// Configured byte cap.
        max_bytes: usize,
        /// Actual loaded bytes.
        actual_bytes: usize,
    },
    #[error("{message}")]
    /// Model request failed.
    ModelRequestFailed {
        /// Error message.
        message: String,
    },
    #[error("{message}")]
    /// Model JSON parse failed.
    ResponseParseFailed {
        /// Error message.
        message: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// `doctor` exit buckets.
pub enum DoctorExit {
    /// Doctor checks passed.
    Ok,
    /// Config load failed.
    InvalidConfig,
    /// Host URL invalid.
    BadHost,
    /// Model missing.
    MissingModel,
    /// Host not reachable.
    UnreachableHost,
    /// Probe request failed.
    ProbeRequestFailed,
    /// Probe JSON invalid.
    InvalidProbeJson,
}

impl AppError {
    #[must_use]
    /// Build invalid-arguments error.
    pub fn invalid_arguments(message: impl Into<String>) -> Self {
        Self::InvalidArguments {
            message: message.into(),
        }
    }

    #[must_use]
    /// Wrap error with subcommand tag.
    pub(crate) fn with_command(self, command: CommandId) -> Self {
        Self::CommandContext {
            command,
            source: Box::new(self),
        }
    }

    #[must_use]
    /// Build missing-path error.
    pub fn missing_path(path: &Path) -> Self {
        Self::MissingPath {
            path: path.display().to_string(),
        }
    }

    #[must_use]
    /// Build missing-recursive error.
    pub fn directory_requires_recursive(path: &Path) -> Self {
        Self::DirectoryRequiresRecursive {
            path: path.display().to_string(),
        }
    }

    #[must_use]
    /// Build file-read error.
    pub fn file_read(path: &Path, error: &io::Error) -> Self {
        Self::FileRead {
            path: path.display().to_string(),
            message: error.to_string(),
        }
    }

    #[must_use]
    /// Build init-file-update error.
    pub fn init_file_update(path: &Path, message: impl Into<String>) -> Self {
        Self::InitFileUpdate {
            path: path.display().to_string(),
            message: message.into(),
        }
    }

    #[must_use]
    /// Build no-input-files error.
    pub fn no_input_files() -> Self {
        Self::NoInputFiles
    }

    #[must_use]
    /// Build max-bytes error.
    pub fn max_bytes_exceeded(max_bytes: usize, actual_bytes: usize) -> Self {
        Self::MaxBytesExceeded {
            max_bytes,
            actual_bytes,
        }
    }

    #[must_use]
    /// Build model-request error.
    pub fn model_request_failed(message: impl Into<String>) -> Self {
        Self::ModelRequestFailed {
            message: message.into(),
        }
    }

    #[must_use]
    /// Build response-parse error.
    pub fn response_parse_failed(message: impl Into<String>) -> Self {
        Self::ResponseParseFailed {
            message: message.into(),
        }
    }

    #[must_use]
    /// Return process exit code.
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::InvalidArguments { .. } => ExitCode::from(1),
            Self::CommandContext { source, .. } => source.exit_code(),
            Self::MissingPath { .. }
            | Self::DirectoryRequiresRecursive { .. }
            | Self::FileRead { .. }
            | Self::InitFileUpdate { .. }
            | Self::NoInputFiles => ExitCode::from(2),
            Self::MaxBytesExceeded { .. } => ExitCode::from(3),
            Self::ModelRequestFailed { .. } => ExitCode::from(4),
            Self::ResponseParseFailed { .. } => ExitCode::from(5),
        }
    }

    #[must_use]
    /// Serialize JSON error output.
    pub fn to_json(&self) -> String {
        let mut error = Map::with_capacity(3);
        error.insert(String::from("code"), Value::from(self.code()));
        error.insert(String::from("message"), Value::from(self.to_string()));
        if let Some(hint) = self.hint() {
            error.insert(String::from("hint"), Value::from(hint));
        }

        let mut root = Map::with_capacity(4);
        root.insert(String::from("schema_version"), Value::from("1.0"));
        root.insert(
            String::from("command"),
            Value::from(self.command().as_str()),
        );
        root.insert(String::from("status"), Value::from("error"));
        root.insert(String::from("error"), Value::Object(error));

        Value::Object(root).to_string()
    }

    #[must_use]
    /// Return stable error code.
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidArguments { .. } => "INVALID_ARGUMENTS",
            Self::CommandContext { source, .. } => source.code(),
            Self::MissingPath { .. } => "MISSING_PATH",
            Self::DirectoryRequiresRecursive { .. } => "DIRECTORY_REQUIRES_RECURSIVE",
            Self::FileRead { .. } => "FILE_READ_FAILED",
            Self::InitFileUpdate { .. } => "INIT_FILE_UPDATE_FAILED",
            Self::NoInputFiles => "NO_INPUT_FILES",
            Self::MaxBytesExceeded { .. } => "MAX_BYTES_EXCEEDED",
            Self::ModelRequestFailed { .. } => "MODEL_REQUEST_FAILED",
            Self::ResponseParseFailed { .. } => "RESPONSE_PARSE_FAILED",
        }
    }

    #[must_use]
    /// Return command tag for error.
    fn command(&self) -> CommandId {
        match self {
            Self::InvalidArguments { .. } => CommandId::Cli,
            Self::CommandContext { command, .. } => *command,
            Self::InitFileUpdate { .. } => CommandId::Init,
            Self::MissingPath { .. }
            | Self::DirectoryRequiresRecursive { .. }
            | Self::FileRead { .. }
            | Self::NoInputFiles
            | Self::MaxBytesExceeded { .. }
            | Self::ModelRequestFailed { .. }
            | Self::ResponseParseFailed { .. } => CommandId::Ask,
        }
    }

    #[must_use]
    /// Return optional fix hint.
    fn hint(&self) -> Option<&'static str> {
        match self {
            Self::InvalidArguments { .. } => Some("Use `cowork --help`."),
            Self::CommandContext { source, .. } => source.hint(),
            Self::MissingPath { .. } => Some("Check path spelling and current dir."),
            Self::DirectoryRequiresRecursive { .. } => {
                Some("Pass `--recursive` for directory inputs.")
            }
            Self::FileRead { .. } => Some("Check file exists and is readable."),
            Self::InitFileUpdate { .. } => {
                Some("Check target file perms and managed block markers.")
            }
            Self::NoInputFiles => Some("Provide at least one readable UTF-8 text file."),
            Self::MaxBytesExceeded { .. } => Some("Pass larger `--max-bytes` or fewer files."),
            Self::ModelRequestFailed { .. } => Some("Check Ollama server, model, and `--host`."),
            Self::ResponseParseFailed { .. } => Some("Check Ollama response and ask JSON schema."),
        }
    }
}

impl DoctorExit {
    #[must_use]
    /// Return doctor exit code.
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

#[cfg(test)]
mod tests {
    #![allow(clippy::missing_errors_doc, reason = "test helpers stay local")]
    #![allow(clippy::missing_panics_doc, reason = "test asserts and fixtures")]

    use std::process::ExitCode;

    use serde_json::json;

    use super::{AppError, DoctorExit};
    use crate::output::CommandId;

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
    fn command_context_preserves_source_contract() {
        let error = AppError::max_bytes_exceeded(1, 2).with_command(CommandId::Locate);
        let value =
            serde_json::from_str::<serde_json::Value>(&error.to_json()).expect("json should parse");

        assert_eq!(error.to_string(), "input bytes exceed `--max-bytes`: 2 > 1");
        assert_eq!(error.code(), "MAX_BYTES_EXCEEDED");
        assert_eq!(
            error.hint(),
            Some("Pass larger `--max-bytes` or fewer files.")
        );
        assert_eq!(error.exit_code(), ExitCode::from(3));
        assert_eq!(value["command"], "locate");
        assert_eq!(value["error"]["code"], "MAX_BYTES_EXCEEDED");
        assert_eq!(
            value["error"]["message"],
            "input bytes exceed `--max-bytes`: 2 > 1"
        );
        assert_eq!(
            value["error"]["hint"],
            "Pass larger `--max-bytes` or fewer files."
        );
    }

    #[test]
    fn command_context_tags_stay_exact_at_json_boundary() {
        let cases = [
            (CommandId::Ask, "ask"),
            (CommandId::Brief, "brief"),
            (CommandId::Locate, "locate"),
            (CommandId::Doctor, "doctor"),
            (CommandId::Setup, "setup"),
            (CommandId::Init, "init"),
        ];

        for (command, expected) in cases {
            let error = AppError::model_request_failed("down").with_command(command);
            let value = serde_json::from_str::<serde_json::Value>(&error.to_json())
                .expect("json should parse");

            assert_eq!(value["command"], expected);
        }
    }

    #[test]
    fn missing_path_maps_to_exit_code_two() {
        assert_eq!(
            AppError::missing_path(std::path::Path::new("nope")).exit_code(),
            ExitCode::from(2)
        );
    }

    #[test]
    fn no_input_files_contract_stays_exact() {
        let error = AppError::no_input_files();
        let value =
            serde_json::from_str::<serde_json::Value>(&error.to_json()).expect("json should parse");

        assert_eq!(
            error.to_string(),
            "no readable UTF-8 text input files found"
        );
        assert_eq!(error.code(), "NO_INPUT_FILES");
        assert_eq!(
            error.hint(),
            Some("Provide at least one readable UTF-8 text file.")
        );
        assert_eq!(error.exit_code(), ExitCode::from(2));
        assert_eq!(value["command"], "ask");
        assert_eq!(value["error"]["code"], "NO_INPUT_FILES");
        assert_eq!(
            value["error"]["message"],
            "no readable UTF-8 text input files found"
        );
        assert_eq!(
            value["error"]["hint"],
            "Provide at least one readable UTF-8 text file."
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
