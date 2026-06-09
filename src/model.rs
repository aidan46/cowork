use std::{error::Error as _, time::Duration};

use reqwest::{Url, blocking::Client};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// Setup-facing Ollama operations.
mod ollama;

#[allow(unused_imports)]
pub(crate) use ollama::{
    OllamaError, OllamaErrorKind, OllamaModel, request_ollama_pull, request_ollama_tags,
};

/// HTTP connect timeout.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// HTTP request timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);
/// Tiny doctor probe prompt.
const DOCTOR_PROMPT: &str = r#"Return strict JSON only with exact shape {"ok":true}."#;

#[derive(Debug, PartialEq, Eq)]
/// Doctor probe error kind.
pub(crate) enum DoctorProbeErrorKind {
    /// Host URL invalid.
    BadHost,
    /// Host not reachable.
    UnreachableHost,
    /// Probe request failed.
    ProbeRequestFailed,
    /// Probe JSON invalid.
    InvalidProbeJson,
}

#[derive(Debug, PartialEq, Eq)]
/// Doctor probe error payload.
pub(crate) struct DoctorProbeError {
    /// Error kind.
    pub(crate) kind: DoctorProbeErrorKind,
    /// Error message.
    pub(crate) message: String,
}

#[derive(Serialize)]
/// Ollama generate request body.
struct GenerateRequest<'a> {
    /// Model name.
    model: &'a str,
    /// Prompt text.
    prompt: &'a str,
    /// Stream flag.
    stream: bool,
    /// Output format.
    format: &'a str,
    /// Generate options.
    options: GenerateOptions,
}

#[derive(Serialize)]
/// Ollama generate options.
struct GenerateOptions {
    /// Sampling temperature.
    temperature: u8,
}

#[derive(Deserialize)]
/// Ollama generate response envelope.
struct GenerateResponse {
    /// Raw model response text.
    response: String,
}

/// Send one Ollama generate req, return raw model text.
///
/// # Errors
///
/// Returns [`AppError`] on host, HTTP, or response JSON failure.
pub(crate) fn request_generate(host: &str, model: &str, prompt: &str) -> Result<String, AppError> {
    let client = Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|error| {
            AppError::model_request_failed(format_reqwest_error(
                "failed to build model client",
                &error,
            ))
        })?;
    let endpoint = build_generate_url(host).map_err(AppError::model_request_failed)?;

    let response = client
        .post(endpoint)
        .json(&GenerateRequest {
            model,
            prompt,
            stream: false,
            format: "json",
            options: GenerateOptions { temperature: 0 },
        })
        .send()
        .map_err(|error| {
            AppError::model_request_failed(format_reqwest_error(
                "failed to send model request",
                &error,
            ))
        })?;

    if !response.status().is_success() {
        return Err(AppError::model_request_failed(format!(
            "model request failed with status {}",
            response.status().as_u16()
        )));
    }

    response
        .json::<GenerateResponse>()
        .map(|envelope| envelope.response)
        .map_err(|error| {
            AppError::response_parse_failed(format!("failed to parse model response: {error}"))
        })
}

/// Validate generate host URL.
///
/// # Errors
///
/// Returns [`DoctorProbeError`] when the host URL is invalid.
pub(crate) fn validate_generate_host(host: &str) -> Result<(), DoctorProbeError> {
    build_generate_url(host)
        .map(|_| ())
        .map_err(DoctorProbeError::bad_host)
}

/// Send doctor probe request.
///
/// # Errors
///
/// Returns [`DoctorProbeError`] on host, HTTP, or response JSON failure.
pub(crate) fn request_doctor_probe(host: &str, model: &str) -> Result<String, DoctorProbeError> {
    let client = Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|error| {
            DoctorProbeError::probe_request_failed(format_reqwest_error(
                "failed to build doctor client",
                &error,
            ))
        })?;
    let endpoint = build_generate_url(host).map_err(DoctorProbeError::bad_host)?;

    let response = client
        .post(endpoint)
        .json(&GenerateRequest {
            model,
            prompt: DOCTOR_PROMPT,
            stream: false,
            format: "json",
            options: GenerateOptions { temperature: 0 },
        })
        .send()
        .map_err(|error| {
            if error.is_connect() || error.is_timeout() {
                DoctorProbeError::unreachable_host(format_reqwest_error(
                    "failed to reach /api/generate",
                    &error,
                ))
            } else {
                DoctorProbeError::probe_request_failed(format_reqwest_error(
                    "doctor probe request failed",
                    &error,
                ))
            }
        })?;

    if !response.status().is_success() {
        return Err(DoctorProbeError::probe_request_failed(format!(
            "doctor probe failed with status {}",
            response.status().as_u16()
        )));
    }

    response
        .json::<GenerateResponse>()
        .map(|envelope| envelope.response)
        .map_err(|error| {
            DoctorProbeError::invalid_probe_json(format!(
                "failed to parse doctor probe response: {error}"
            ))
        })
}

impl DoctorProbeError {
    /// Build bad-host error.
    fn bad_host(message: impl Into<String>) -> Self {
        Self {
            kind: DoctorProbeErrorKind::BadHost,
            message: message.into(),
        }
    }

    /// Build unreachable-host error.
    fn unreachable_host(message: impl Into<String>) -> Self {
        Self {
            kind: DoctorProbeErrorKind::UnreachableHost,
            message: message.into(),
        }
    }

    /// Build probe-request error.
    fn probe_request_failed(message: impl Into<String>) -> Self {
        Self {
            kind: DoctorProbeErrorKind::ProbeRequestFailed,
            message: message.into(),
        }
    }

    /// Build invalid-probe-json error.
    fn invalid_probe_json(message: impl Into<String>) -> Self {
        Self {
            kind: DoctorProbeErrorKind::InvalidProbeJson,
            message: message.into(),
        }
    }
}

/// Build `/api/generate` URL.
///
/// # Errors
///
/// Returns a string error when the host URL cannot be parsed or joined.
fn build_generate_url(host: &str) -> Result<Url, String> {
    build_api_url(host, "api/generate")
}

/// Build Ollama API URL.
///
/// # Errors
///
/// Returns a string error when the host URL cannot be parsed or joined.
fn build_api_url(host: &str, api_path: &str) -> Result<Url, String> {
    let mut base = host.trim().to_string();
    if !base.ends_with('/') {
        base.push('/');
    }

    let base =
        Url::parse(&base).map_err(|error| format!("failed to parse host URL `{host}`: {error}"))?;

    base.join(api_path)
        .map_err(|error| format!("failed to build `/{api_path}` URL from `{host}`: {error}"))
}

/// Flatten reqwest error chain.
fn format_reqwest_error(prefix: &str, error: &reqwest::Error) -> String {
    let mut message = format!("{prefix}: {error}");
    let mut source = error.source();

    while let Some(next) = source {
        message.push_str(": ");
        message.push_str(&next.to_string());
        source = next.source();
    }

    message
}

#[cfg(test)]
mod tests;
