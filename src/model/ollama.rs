use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use super::{CONNECT_TIMEOUT, REQUEST_TIMEOUT, build_api_url, format_reqwest_error};

#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
/// Ollama model list item.
pub(crate) struct OllamaModel {
    /// Model name.
    pub(crate) name: String,
    /// Model size in bytes.
    pub(crate) size: Option<u64>,
    /// Model family.
    pub(crate) family: Option<String>,
    /// Parameter size label.
    pub(crate) parameter_size: Option<String>,
    /// Quantization label.
    pub(crate) quantization_level: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
/// Ollama request error kind.
pub(crate) enum OllamaErrorKind {
    /// Host URL invalid.
    BadHost,
    /// Host not reachable.
    UnreachableHost,
    /// Request failed.
    RequestFailed,
    /// Response JSON invalid.
    InvalidJson,
}

#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
/// Ollama request error payload.
pub(crate) struct OllamaError {
    /// Error kind.
    pub(crate) kind: OllamaErrorKind,
    /// Error message.
    pub(crate) message: String,
}

#[allow(dead_code)]
#[derive(Serialize)]
/// Ollama pull request body.
struct PullRequest<'a> {
    /// Model name.
    model: &'a str,
    /// Stream flag.
    stream: bool,
}

#[allow(dead_code)]
#[derive(Deserialize)]
/// Ollama tags response envelope.
struct TagsResponse {
    /// Listed models.
    models: Vec<TagModelResponse>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
/// Ollama tag response item.
struct TagModelResponse {
    /// Model name.
    name: String,
    /// Model size in bytes.
    #[serde(default)]
    size: Option<u64>,
    /// Model details.
    #[serde(default)]
    details: Option<TagModelDetailsResponse>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
/// Ollama tag details.
struct TagModelDetailsResponse {
    /// Model family.
    #[serde(default)]
    family: Option<String>,
    /// Parameter size label.
    #[serde(default)]
    parameter_size: Option<String>,
    /// Quantization label.
    #[serde(default)]
    quantization_level: Option<String>,
}

#[allow(dead_code)]
/// List local Ollama models.
///
/// # Errors
///
/// Returns [`OllamaError`] on host, HTTP, or response JSON failure.
pub(crate) fn request_ollama_tags(host: &str) -> Result<Vec<OllamaModel>, OllamaError> {
    let client = Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|error| {
            OllamaError::request_failed(format_reqwest_error(
                "failed to build model client",
                &error,
            ))
        })?;
    let endpoint = build_api_url(host, "api/tags").map_err(OllamaError::bad_host)?;

    let response = client.get(endpoint).send().map_err(|error| {
        if error.is_connect() || error.is_timeout() {
            OllamaError::unreachable_host(format_reqwest_error("failed to reach /api/tags", &error))
        } else {
            OllamaError::request_failed(format_reqwest_error("tags request failed", &error))
        }
    })?;

    if !response.status().is_success() {
        return Err(OllamaError::request_failed(format!(
            "tags request failed with status {}",
            response.status().as_u16()
        )));
    }

    response
        .json::<TagsResponse>()
        .map(|envelope| {
            envelope
                .models
                .into_iter()
                .map(|model| OllamaModel {
                    name: model.name,
                    size: model.size,
                    family: model
                        .details
                        .as_ref()
                        .and_then(|details| details.family.clone()),
                    parameter_size: model
                        .details
                        .as_ref()
                        .and_then(|details| details.parameter_size.clone()),
                    quantization_level: model
                        .details
                        .as_ref()
                        .and_then(|details| details.quantization_level.clone()),
                })
                .collect()
        })
        .map_err(|error| {
            OllamaError::invalid_json(format!("failed to parse tags response: {error}"))
        })
}

#[allow(dead_code)]
/// Pull Ollama model.
///
/// # Errors
///
/// Returns [`OllamaError`] on host, HTTP, or response JSON failure.
pub(crate) fn request_ollama_pull(host: &str, model: &str) -> Result<(), OllamaError> {
    let client = Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|error| {
            OllamaError::request_failed(format_reqwest_error(
                "failed to build model client",
                &error,
            ))
        })?;
    let endpoint = build_api_url(host, "api/pull").map_err(OllamaError::bad_host)?;

    let response = client
        .post(endpoint)
        .json(&PullRequest {
            model,
            stream: false,
        })
        .send()
        .map_err(|error| {
            if error.is_connect() || error.is_timeout() {
                OllamaError::unreachable_host(format_reqwest_error(
                    "failed to reach /api/pull",
                    &error,
                ))
            } else {
                OllamaError::request_failed(format_reqwest_error("pull request failed", &error))
            }
        })?;

    if !response.status().is_success() {
        return Err(OllamaError::request_failed(format!(
            "pull request failed with status {}",
            response.status().as_u16()
        )));
    }

    response
        .json::<serde_json::Value>()
        .map(|_| ())
        .map_err(|error| {
            OllamaError::invalid_json(format!("failed to parse pull response: {error}"))
        })
}

#[allow(dead_code)]
impl OllamaError {
    /// Build bad-host error.
    fn bad_host(message: impl Into<String>) -> Self {
        Self {
            kind: OllamaErrorKind::BadHost,
            message: message.into(),
        }
    }

    /// Build unreachable-host error.
    fn unreachable_host(message: impl Into<String>) -> Self {
        Self {
            kind: OllamaErrorKind::UnreachableHost,
            message: message.into(),
        }
    }

    /// Build request-failed error.
    fn request_failed(message: impl Into<String>) -> Self {
        Self {
            kind: OllamaErrorKind::RequestFailed,
            message: message.into(),
        }
    }

    /// Build invalid-json error.
    fn invalid_json(message: impl Into<String>) -> Self {
        Self {
            kind: OllamaErrorKind::InvalidJson,
            message: message.into(),
        }
    }
}
