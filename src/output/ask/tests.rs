#![allow(clippy::missing_errors_doc, reason = "test helpers stay local")]
#![allow(clippy::missing_panics_doc, reason = "test asserts and fixtures")]

use serde_json::json;

use super::*;
use crate::{error::AppError, output::CommandMetadata};

#[test]
fn valid_model_json_parses_to_typed_success_output() {
    let output = parse_ask_output(&valid_model_json(), metadata()).expect("output should parse");

    assert_eq!(output.question, "How does request authentication work?");
    assert_eq!(
        output.answer.summary,
        "Authentication is enforced in middleware."
    );
    assert_eq!(output.files.len(), 1);
    assert_eq!(output.symbols.len(), 1);
    assert_eq!(output.evidence.len(), 1);
    assert_eq!(output.risks.len(), 1);
    assert_eq!(output.next_reads.len(), 1);
    assert_eq!(output.metadata.input_bytes, 12420);
    assert_eq!(output.metadata.duration_ms, 980);
    assert_eq!(output.metadata.output_bytes, 0);
}

#[test]
fn parsed_output_sorts_and_dedupes_rows() {
    let output = parse_ask_output(&unsorted_duplicate_model_json(), metadata())
        .expect("output should parse");

    assert_eq!(
        serde_json::to_value(&output.files).expect("files should serialize"),
        json!([
            {
                "path": "src/a.rs",
                "included": true,
                "reason": "Contains auth flow.",
                "bytes": 10
            },
            {
                "path": "src/a.rs",
                "included": true,
                "reason": "Contains auth flow.",
                "bytes": 30
            },
            {
                "path": "src/z.rs",
                "included": false,
                "reason": "Skipped binary snapshot.",
                "bytes": 12
            }
        ])
    );
    assert_eq!(
        serde_json::to_value(&output.symbols).expect("symbols should serialize"),
        json!([
            {
                "name": "AuthState",
                "kind": "type",
                "path": "src/a.rs",
                "relevance": "Owns auth state."
            },
            {
                "name": "authenticate",
                "kind": "function",
                "path": "src/a.rs",
                "relevance": "Runs auth checks."
            },
            {
                "name": "snapshot_auth",
                "kind": "function",
                "path": "src/z.rs",
                "relevance": "Exports auth snapshot."
            }
        ])
    );
    assert_eq!(
        serde_json::to_value(&output.evidence).expect("evidence should serialize"),
        json!([
            {
                "path": "src/a.rs",
                "symbol": "AuthState",
                "note": "Holds request auth state."
            },
            {
                "path": "src/a.rs",
                "symbol": "authenticate",
                "note": "Rejects bad tokens."
            },
            {
                "path": "src/z.rs",
                "symbol": "snapshot_auth",
                "note": "Writes auth snapshot."
            }
        ])
    );
    assert_eq!(
        serde_json::to_value(&output.risks).expect("risks should serialize"),
        json!([
            {
                "kind": "missing_context",
                "message": "Tests missing."
            },
            {
                "kind": "unsupported_file",
                "message": "Binary snapshot skipped."
            }
        ])
    );
    assert_eq!(
        serde_json::to_value(&output.next_reads).expect("next reads should serialize"),
        json!([
            {
                "path": "src/a/tests.rs",
                "reason": "Likely covers auth edges."
            },
            {
                "path": "src/z/tests.rs",
                "reason": "Likely covers snapshot edges."
            }
        ])
    );
}

#[test]
fn serialized_json_uses_cli_owned_metadata() {
    let output = parse_ask_output(&valid_model_json(), metadata()).expect("output should parse");
    let value = serde_json::from_str::<serde_json::Value>(
        &output.into_json().expect("output should serialize"),
    )
    .expect("json should parse");

    assert_eq!(value["metadata"]["input_bytes"], 12420);
    assert_eq!(value["metadata"]["duration_ms"], 980);
    assert!(value["metadata"]["output_bytes"].as_u64().unwrap_or(0) > 0);
    assert!(
        value["metadata"]["compression_ratio"]
            .as_str()
            .and_then(|ratio| ratio.parse::<f64>().ok())
            .is_some_and(|ratio| ratio > 1.0)
    );
}

#[test]
fn serialized_json_normalizes_row_order_and_dedupes() {
    let output = parse_ask_output(&unsorted_duplicate_model_json(), metadata())
        .expect("output should parse");
    let mut value = serde_json::from_str::<serde_json::Value>(
        &output.into_json().expect("output should serialize"),
    )
    .expect("json should parse");
    let metadata = value
        .as_object_mut()
        .and_then(|root| root.remove("metadata"))
        .expect("metadata should exist");

    assert_eq!(
        value,
        json!({
            "schema_version": "1.0",
            "command": "ask",
            "status": "ok",
            "question": "Where does auth state live?",
            "answer": {
                "summary": "Auth state lives in src/a.rs.",
                "confidence": "medium",
                "not_found": false
            },
            "files": [
                {
                    "path": "src/a.rs",
                    "included": true,
                    "reason": "Contains auth flow.",
                    "bytes": 10
                },
                {
                    "path": "src/a.rs",
                    "included": true,
                    "reason": "Contains auth flow.",
                    "bytes": 30
                },
                {
                    "path": "src/z.rs",
                    "included": false,
                    "reason": "Skipped binary snapshot.",
                    "bytes": 12
                }
            ],
            "symbols": [
                {
                    "name": "AuthState",
                    "kind": "type",
                    "path": "src/a.rs",
                    "relevance": "Owns auth state."
                },
                {
                    "name": "authenticate",
                    "kind": "function",
                    "path": "src/a.rs",
                    "relevance": "Runs auth checks."
                },
                {
                    "name": "snapshot_auth",
                    "kind": "function",
                    "path": "src/z.rs",
                    "relevance": "Exports auth snapshot."
                }
            ],
            "evidence": [
                {
                    "path": "src/a.rs",
                    "symbol": "AuthState",
                    "note": "Holds request auth state."
                },
                {
                    "path": "src/a.rs",
                    "symbol": "authenticate",
                    "note": "Rejects bad tokens."
                },
                {
                    "path": "src/z.rs",
                    "symbol": "snapshot_auth",
                    "note": "Writes auth snapshot."
                }
            ],
            "risks": [
                {
                    "kind": "missing_context",
                    "message": "Tests missing."
                },
                {
                    "kind": "unsupported_file",
                    "message": "Binary snapshot skipped."
                }
            ],
            "next_reads": [
                {
                    "path": "src/a/tests.rs",
                    "reason": "Likely covers auth edges."
                },
                {
                    "path": "src/z/tests.rs",
                    "reason": "Likely covers snapshot edges."
                }
            ]
        })
    );

    assert_eq!(metadata["input_bytes"], 12420);
    assert_eq!(metadata["duration_ms"], 980);
    assert!(metadata["output_bytes"].as_u64().unwrap_or(0) > 0);
}

#[test]
fn serialized_json_keeps_fixed_top_level_fields() {
    let output = parse_ask_output(&valid_model_json(), metadata()).expect("output should parse");
    let mut value = serde_json::from_str::<serde_json::Value>(
        &output.into_json().expect("output should serialize"),
    )
    .expect("json should parse");
    let metadata = value
        .as_object_mut()
        .and_then(|root| root.remove("metadata"))
        .expect("metadata should exist");

    assert_eq!(
        value,
        json!({
            "schema_version": "1.0",
            "command": "ask",
            "status": "ok",
            "question": "How does request authentication work?",
            "answer": {
                "summary": "Authentication is enforced in middleware.",
                "confidence": "high",
                "not_found": false
            },
            "files": [
                {
                    "path": "src/auth/middleware.rs",
                    "included": true,
                    "reason": "Contains authentication middleware.",
                    "bytes": 12420
                }
            ],
            "symbols": [
                {
                    "name": "authenticate_request",
                    "kind": "function",
                    "path": "src/auth/middleware.rs",
                    "relevance": "Validates credentials and attaches user context."
                }
            ],
            "evidence": [
                {
                    "path": "src/auth/middleware.rs",
                    "symbol": "authenticate_request",
                    "note": "Requests without valid credentials return early."
                }
            ],
            "risks": [
                {
                    "kind": "missing_context",
                    "message": "Tests were not provided."
                }
            ],
            "next_reads": [
                {
                    "path": "src/auth/tests.rs",
                    "reason": "Likely contains authentication edge cases."
                }
            ]
        })
    );

    assert_eq!(metadata["input_bytes"], 12420);
    assert_eq!(metadata["duration_ms"], 980);
    assert!(metadata["output_bytes"].as_u64().unwrap_or(0) > 0);
    assert!(
        metadata["compression_ratio"]
            .as_str()
            .and_then(|ratio| ratio.parse::<f64>().ok())
            .is_some_and(|ratio| ratio > 1.0)
    );
}

#[test]
fn missing_required_field_maps_to_response_parse_failed() {
    let error = parse_ask_output(
        r#"{
            "question":"q",
            "answer":{"summary":"s","confidence":"high","not_found":false},
            "files":[],
            "symbols":[],
            "evidence":[],
            "risks":[]
        }"#,
        metadata(),
    )
    .expect_err("parse should fail");

    assert!(matches!(error, AppError::ResponseParseFailed { .. }));
}

#[test]
fn parsed_output_caps_arrays_and_injects_cap_risk() {
    let output = parse_ask_output(&capped_model_json(), metadata()).expect("output should parse");

    assert_eq!(output.files.len(), 40);
    assert_eq!(output.symbols.len(), 80);
    assert_eq!(output.evidence.len(), 80);
    assert_eq!(output.risks.len(), 20);
    assert_eq!(output.next_reads.len(), 20);
    assert!(output.risks.iter().any(|risk| {
        risk == &AskRisk {
            kind: AskRiskKind::Unknown,
            message: "Output capped: files 45->40, symbols 85->80, evidence 85->80, risks kept 19 of 25 model rows, next_reads 25->20.".to_string(),
        }
    }));
}

#[test]
fn serialized_json_truncates_long_strings_and_injects_truncation_risk() {
    let output =
        parse_ask_output(&truncated_model_json(), metadata()).expect("output should parse");
    let value = serde_json::from_str::<serde_json::Value>(
        &output.into_json().expect("output should serialize"),
    )
    .expect("json should parse");

    assert_eq!(
        value["answer"]["summary"]
            .as_str()
            .map(|summary| summary.chars().count()),
        Some(1200)
    );
    assert!(
        value["answer"]["summary"]
            .as_str()
            .is_some_and(|summary| summary.ends_with(" [truncated]"))
    );
    assert!(
        value["files"][0]["reason"]
            .as_str()
            .is_some_and(|reason| reason.ends_with(" [truncated]"))
    );
    assert!(
        value["symbols"][0]["relevance"]
            .as_str()
            .is_some_and(|relevance| relevance.ends_with(" [truncated]"))
    );
    assert!(
        value["evidence"][0]["note"]
            .as_str()
            .is_some_and(|note| note.ends_with(" [truncated]"))
    );
    assert!(
        value["risks"][0]["message"]
            .as_str()
            .is_some_and(|message| message.ends_with(" [truncated]"))
    );
    assert!(
        value["next_reads"][0]["reason"]
            .as_str()
            .is_some_and(|reason| reason.ends_with(" [truncated]"))
    );
    assert_eq!(
        value["risks"][1],
        json!({
            "kind": "unknown",
            "message": "Output truncated at 1200 chars: answer.summary x1, files.reason x1, symbols.relevance x1, evidence.note x1, risks.message x1, next_reads.reason x1."
        })
    );
}

#[test]
fn bad_enum_value_maps_to_response_parse_failed() {
    let error = parse_ask_output(
        r#"{
            "question":"q",
            "answer":{"summary":"s","confidence":"certain","not_found":false},
            "files":[],
            "symbols":[],
            "evidence":[],
            "risks":[],
            "next_reads":[]
        }"#,
        metadata(),
    )
    .expect_err("parse should fail");

    assert!(matches!(error, AppError::ResponseParseFailed { .. }));
}

fn valid_model_json() -> String {
    json!({
        "question": "How does request authentication work?",
        "answer": {
            "summary": "Authentication is enforced in middleware.",
            "confidence": "high",
            "not_found": false
        },
        "files": [
            {
                "path": "src/auth/middleware.rs",
                "included": true,
                "reason": "Contains authentication middleware.",
                "bytes": 12420
            }
        ],
        "symbols": [
            {
                "name": "authenticate_request",
                "kind": "function",
                "path": "src/auth/middleware.rs",
                "relevance": "Validates credentials and attaches user context."
            }
        ],
        "evidence": [
            {
                "path": "src/auth/middleware.rs",
                "symbol": "authenticate_request",
                "note": "Requests without valid credentials return early."
            }
        ],
        "risks": [
            {
                "kind": "missing_context",
                "message": "Tests were not provided."
            }
        ],
        "next_reads": [
            {
                "path": "src/auth/tests.rs",
                "reason": "Likely contains authentication edge cases."
            }
        ]
    })
    .to_string()
}

fn unsorted_duplicate_model_json() -> String {
    json!({
        "question": "Where does auth state live?",
        "answer": {
            "summary": "Auth state lives in src/a.rs.",
            "confidence": "medium",
            "not_found": false
        },
        "files": [
            {
                "path": "src/z.rs",
                "included": false,
                "reason": "Skipped binary snapshot.",
                "bytes": 12
            },
            {
                "path": "src/a.rs",
                "included": true,
                "reason": "Contains auth flow.",
                "bytes": 30
            },
            {
                "path": "src/a.rs",
                "included": true,
                "reason": "Contains auth flow.",
                "bytes": 10
            },
            {
                "path": "src/a.rs",
                "included": true,
                "reason": "Contains auth flow.",
                "bytes": 30
            }
        ],
        "symbols": [
            {
                "name": "snapshot_auth",
                "kind": "function",
                "path": "src/z.rs",
                "relevance": "Exports auth snapshot."
            },
            {
                "name": "authenticate",
                "kind": "function",
                "path": "src/a.rs",
                "relevance": "Runs auth checks."
            },
            {
                "name": "AuthState",
                "kind": "type",
                "path": "src/a.rs",
                "relevance": "Owns auth state."
            },
            {
                "name": "authenticate",
                "kind": "function",
                "path": "src/a.rs",
                "relevance": "Runs auth checks."
            }
        ],
        "evidence": [
            {
                "path": "src/z.rs",
                "symbol": "snapshot_auth",
                "note": "Writes auth snapshot."
            },
            {
                "path": "src/a.rs",
                "symbol": "authenticate",
                "note": "Rejects bad tokens."
            },
            {
                "path": "src/a.rs",
                "symbol": "AuthState",
                "note": "Holds request auth state."
            },
            {
                "path": "src/a.rs",
                "symbol": "authenticate",
                "note": "Rejects bad tokens."
            }
        ],
        "risks": [
            {
                "kind": "unsupported_file",
                "message": "Binary snapshot skipped."
            },
            {
                "kind": "missing_context",
                "message": "Tests missing."
            },
            {
                "kind": "missing_context",
                "message": "Tests missing."
            }
        ],
        "next_reads": [
            {
                "path": "src/z/tests.rs",
                "reason": "Likely covers snapshot edges."
            },
            {
                "path": "src/a/tests.rs",
                "reason": "Likely covers auth edges."
            },
            {
                "path": "src/a/tests.rs",
                "reason": "Likely covers auth edges."
            }
        ]
    })
    .to_string()
}

fn capped_model_json() -> String {
    let files = (0..45)
        .map(|index| {
            json!({
                "path": format!("src/file-{index:02}.rs"),
                "included": true,
                "reason": format!("Reason {index:02}."),
                "bytes": index + 1
            })
        })
        .collect::<Vec<_>>();
    let symbols = (0..85)
        .map(|index| {
            json!({
                "name": format!("symbol_{index:02}"),
                "kind": "function",
                "path": format!("src/file-{:02}.rs", index % 45),
                "relevance": format!("Relevance {index:02}.")
            })
        })
        .collect::<Vec<_>>();
    let evidence = (0..85)
        .map(|index| {
            json!({
                "path": format!("src/file-{:02}.rs", index % 45),
                "symbol": format!("symbol_{index:02}"),
                "note": format!("Evidence {index:02}.")
            })
        })
        .collect::<Vec<_>>();
    let risks = (0..25)
        .map(|index| {
            json!({
                "kind": "missing_context",
                "message": format!("Risk {index:02}.")
            })
        })
        .collect::<Vec<_>>();
    let next_reads = (0..25)
        .map(|index| {
            json!({
                "path": format!("src/next-{index:02}.rs"),
                "reason": format!("Next read {index:02}.")
            })
        })
        .collect::<Vec<_>>();

    json!({
        "question": "What matters?",
        "answer": {
            "summary": "Small answer.",
            "confidence": "medium",
            "not_found": false
        },
        "files": files,
        "symbols": symbols,
        "evidence": evidence,
        "risks": risks,
        "next_reads": next_reads
    })
    .to_string()
}

fn truncated_model_json() -> String {
    let long_text = long_string(1300);

    json!({
        "question": "What matters?",
        "answer": {
            "summary": long_text,
            "confidence": "high",
            "not_found": false
        },
        "files": [
            {
                "path": "src/auth.rs",
                "included": true,
                "reason": long_string(1301),
                "bytes": 10
            }
        ],
        "symbols": [
            {
                "name": "authenticate",
                "kind": "function",
                "path": "src/auth.rs",
                "relevance": long_string(1302)
            }
        ],
        "evidence": [
            {
                "path": "src/auth.rs",
                "symbol": "authenticate",
                "note": long_string(1303)
            }
        ],
        "risks": [
            {
                "kind": "missing_context",
                "message": long_string(1304)
            }
        ],
        "next_reads": [
            {
                "path": "src/auth/tests.rs",
                "reason": long_string(1305)
            }
        ]
    })
    .to_string()
}

fn long_string(len: usize) -> String {
    "x".repeat(len)
}

fn metadata() -> CommandMetadata {
    CommandMetadata::new(12420, 980)
}
