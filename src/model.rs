use std::time::Duration;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Send one Ollama generate req, return raw model text.
pub(crate) fn request_generate(host: &str, model: &str, prompt: &str) -> Result<String, AppError> {
    #[derive(Serialize)]
    struct GenerateRequest<'a> {
        model: &'a str,
        prompt: &'a str,
        stream: bool,
        format: &'a str,
        options: GenerateOptions,
    }

    #[derive(Serialize)]
    struct GenerateOptions {
        temperature: u8,
    }

    #[derive(Deserialize)]
    struct GenerateResponse {
        response: String,
    }

    let client = Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|error| {
            AppError::model_request_failed(format!("failed to build model client: {error}"))
        })?;

    let response = client
        .post(format!("{}/api/generate", host.trim_end_matches('/')))
        .json(&GenerateRequest {
            model,
            prompt,
            stream: false,
            format: "json",
            options: GenerateOptions { temperature: 0 },
        })
        .send()
        .map_err(|error| {
            AppError::model_request_failed(format!("failed to send model request: {error}"))
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

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        thread,
        time::Duration,
    };

    use serde_json::json;

    use super::request_generate;
    use crate::error::AppError;

    #[test]
    fn request_body_keeps_stream_format_and_temperature() {
        let (host, handle) = spawn_server(ok_response(&response_envelope(r#"{"answer":"ok"}"#)));

        let result =
            request_generate(&host, "gemma3:12b", "find answer").expect("request should parse");
        let request = handle.join().expect("server should join");
        let request_text = String::from_utf8(request).expect("request should be utf-8");

        assert_eq!(result, r#"{"answer":"ok"}"#);
        assert!(request_text.starts_with("POST /api/generate HTTP/1.1\r\n"));

        let body = request_body(request_text.as_bytes());
        let value =
            serde_json::from_slice::<serde_json::Value>(body).expect("request body should be json");

        assert_eq!(
            value,
            json!({
                "model": "gemma3:12b",
                "prompt": "find answer",
                "stream": false,
                "format": "json",
                "options": {
                    "temperature": 0
                }
            })
        );
    }

    #[test]
    fn non_2xx_maps_to_model_request_failed() {
        let (host, handle) = spawn_server(error_response(500, "boom"));

        let error =
            request_generate(&host, "gemma3:12b", "find answer").expect_err("request should fail");

        handle.join().expect("server should join");
        assert!(matches!(error, AppError::ModelRequestFailed { .. }));
    }

    #[test]
    fn bad_response_envelope_maps_to_response_parse_failed() {
        let (host, handle) = spawn_server(ok_response(r#"{"done":true}"#));

        let error =
            request_generate(&host, "gemma3:12b", "find answer").expect_err("parse should fail");

        handle.join().expect("server should join");
        assert!(matches!(error, AppError::ResponseParseFailed { .. }));
    }

    #[test]
    fn valid_envelope_returns_raw_model_text() {
        let (host, handle) = spawn_server(ok_response(&response_envelope(r#"{"answer":"ok"}"#)));

        let result =
            request_generate(&host, "gemma3:12b", "find answer").expect("request should pass");

        handle.join().expect("server should join");
        assert_eq!(result, r#"{"answer":"ok"}"#);
    }

    fn spawn_server(response: String) -> (String, thread::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let address = listener.local_addr().expect("listener should have addr");

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("server should accept");
            let request = read_request(&mut stream);

            stream
                .write_all(response.as_bytes())
                .expect("response should write");
            stream.flush().expect("response should flush");

            request
        });

        (format!("http://{address}"), handle)
    }

    fn read_request(stream: &mut TcpStream) -> Vec<u8> {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("timeout should set");

        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        let mut body_start = None;
        let mut body_len = 0_usize;

        loop {
            let read = stream.read(&mut buffer).expect("request should read");
            if read == 0 {
                break;
            }

            request.extend_from_slice(&buffer[..read]);

            if body_start.is_none()
                && let Some(index) = find_bytes(&request, b"\r\n\r\n")
            {
                body_start = Some(index + 4);
                body_len = parse_content_length(&request[..index + 4]);
            }

            if let Some(body_start) = body_start
                && request.len() >= body_start + body_len
            {
                break;
            }
        }

        request
    }

    fn request_body(request: &[u8]) -> &[u8] {
        let body_start = find_bytes(request, b"\r\n\r\n").expect("request should have body");

        &request[body_start + 4..]
    }

    fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    fn parse_content_length(headers: &[u8]) -> usize {
        let headers = String::from_utf8(headers.to_vec()).expect("headers should be utf-8");

        headers
            .lines()
            .find_map(|line| {
                let value = line.strip_prefix("content-length: ")?;
                value.parse::<usize>().ok()
            })
            .unwrap_or(0)
    }

    fn ok_response(body: &str) -> String {
        http_response(200, "OK", body)
    }

    fn error_response(status: u16, body: &str) -> String {
        http_response(status, "ERR", body)
    }

    fn http_response(status: u16, status_text: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status} {status_text}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn response_envelope(response: &str) -> String {
        json!({ "response": response }).to_string()
    }
}
