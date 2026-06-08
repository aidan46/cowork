#![allow(clippy::missing_errors_doc, reason = "test helpers stay local")]
#![allow(clippy::missing_panics_doc, reason = "test asserts and fixtures")]

use serde_json::json;

use super::*;
use crate::{error::AppError, output::CommandMetadata};

#[test]
fn valid_model_json_parses_to_typed_success_output() {
    let output = parse_locate_output(&valid_model_json(), metadata()).expect("output should parse");

    assert_eq!(output.matches.len(), 2);
    assert_eq!(output.next_reads.len(), 1);
    assert_eq!(output.risks.len(), 1);
    assert_eq!(output.metadata.input_bytes, 12420);
    assert_eq!(output.metadata.duration_ms, 980);
    assert_eq!(output.metadata.output_bytes, 0);
}

#[test]
fn parsed_output_sorts_and_dedupes_rows() {
    let output = parse_locate_output(&unsorted_duplicate_model_json(), metadata())
        .expect("output should parse");

    assert_eq!(
        serde_json::to_value(&output.matches).expect("matches should serialize"),
        json!([
            {
                "path": "src/cli.rs",
                "symbol": "Command",
                "kind": "type",
                "reason": "Defines CLI command variants.",
                "confidence": "high"
            },
            {
                "path": "src/commands.rs",
                "reason": "Dispatches parsed subcommands.",
                "confidence": "medium"
            }
        ])
    );
    assert_eq!(
        serde_json::to_value(&output.next_reads).expect("next reads should serialize"),
        json!([
            {
                "path": "src/commands/ask.rs",
                "reason": "Shows current command run flow."
            },
            {
                "path": "src/output.rs",
                "reason": "Defines shared output metadata."
            }
        ])
    );
}

#[test]
fn serialized_json_uses_cli_owned_metadata() {
    let output = parse_locate_output(&valid_model_json(), metadata()).expect("output should parse");
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
fn serialized_json_keeps_fixed_top_level_fields() {
    let output = parse_locate_output(&valid_model_json(), metadata()).expect("output should parse");
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
            "command": "locate",
            "status": "ok",
            "matches": [
                {
                    "path": "src/cli.rs",
                    "symbol": "Command",
                    "kind": "type",
                    "reason": "Defines CLI command variants.",
                    "confidence": "high"
                },
                {
                    "path": "src/commands.rs",
                    "reason": "Dispatches parsed subcommands.",
                    "confidence": "medium"
                }
            ],
            "next_reads": [
                {
                    "path": "src/commands/ask.rs",
                    "reason": "Shows current command run flow."
                }
            ],
            "risks": [
                {
                    "kind": "missing_context",
                    "message": "Loaded files do not include all command modules."
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
fn parsed_output_caps_arrays_and_injects_cap_risk() {
    let output =
        parse_locate_output(&capped_model_json(), metadata()).expect("output should parse");

    assert_eq!(output.matches.len(), 80);
    assert_eq!(output.next_reads.len(), 20);
    assert_eq!(output.risks.len(), 20);
    assert!(output.risks.iter().any(|risk| {
        risk == &LocateRisk {
            kind: LocateRiskKind::Unknown,
            message:
                "Output capped: matches 85->80, risks kept 19 of 25 model rows, next_reads 25->20."
                    .to_string(),
        }
    }));
}

#[test]
fn serialized_json_truncates_long_strings_and_injects_truncation_risk() {
    let output =
        parse_locate_output(&truncated_model_json(), metadata()).expect("output should parse");
    let value = serde_json::from_str::<serde_json::Value>(
        &output.into_json().expect("output should serialize"),
    )
    .expect("json should parse");

    assert_eq!(
        value["matches"][0]["path"]
            .as_str()
            .map(|path| path.chars().count()),
        Some(1200)
    );
    assert!(
        value["matches"][0]["path"]
            .as_str()
            .is_some_and(|path| path.ends_with(" [truncated]"))
    );
    assert!(
        value["matches"][0]["symbol"]
            .as_str()
            .is_some_and(|symbol| symbol.ends_with(" [truncated]"))
    );
    assert_eq!(
        value["risks"][1],
        json!({
            "kind": "unknown",
            "message": "Output truncated at 1200 chars: matches.path x1, matches.symbol x1, matches.reason x1, next_reads.path x1, next_reads.reason x1, risks.message x1."
        })
    );
}

#[test]
fn serialized_json_adds_both_notices_deterministically_and_stays_stable() {
    let json = parse_locate_output(&bounded_model_json(), metadata())
        .expect("output should parse")
        .into_json()
        .expect("output should serialize");
    let json_again = parse_locate_output(&bounded_model_json(), metadata())
        .expect("output should parse")
        .into_json()
        .expect("output should serialize");
    let value = serde_json::from_str::<serde_json::Value>(&json).expect("json should parse");
    let risks = value["risks"].as_array().expect("risks should be array");

    assert_eq!(json, json_again);
    assert_eq!(value["metadata"]["output_bytes"], json.len());
    assert_eq!(risks.len(), 20);
    assert_eq!(
        risks[18],
        json!({
            "kind": "unknown",
            "message": "Output capped: matches 85->80, risks kept 18 of 25 model rows, next_reads 25->20."
        })
    );
    assert_eq!(
        risks[19],
        json!({
            "kind": "unknown",
            "message": "Output truncated at 1200 chars: matches.path x1, matches.symbol x1, matches.reason x1, next_reads.path x1, next_reads.reason x1, risks.message x1."
        })
    );
}

#[test]
fn missing_required_field_maps_to_response_parse_failed() {
    let error = parse_locate_output(
        r#"{
            "matches":[],
            "risks":[]
        }"#,
        metadata(),
    )
    .expect_err("parse should fail");

    assert!(matches!(error, AppError::ResponseParseFailed { .. }));
}

#[test]
fn bad_enum_value_maps_to_response_parse_failed() {
    let error = parse_locate_output(
        r#"{
            "matches":[
                {
                    "path":"src/lib.rs",
                    "reason":"reason",
                    "confidence":"certain"
                }
            ],
            "next_reads":[],
            "risks":[]
        }"#,
        metadata(),
    )
    .expect_err("parse should fail");

    assert!(matches!(error, AppError::ResponseParseFailed { .. }));
}

fn valid_model_json() -> String {
    json!({
        "matches": [
            {
                "path": "src/commands.rs",
                "reason": "Dispatches parsed subcommands.",
                "confidence": "medium"
            },
            {
                "path": "src/cli.rs",
                "symbol": "Command",
                "kind": "type",
                "reason": "Defines CLI command variants.",
                "confidence": "high"
            }
        ],
        "next_reads": [
            {
                "path": "src/commands/ask.rs",
                "reason": "Shows current command run flow."
            }
        ],
        "risks": [
            {
                "kind": "missing_context",
                "message": "Loaded files do not include all command modules."
            }
        ]
    })
    .to_string()
}

fn unsorted_duplicate_model_json() -> String {
    json!({
        "matches": [
            {
                "path": "src/commands.rs",
                "reason": "Dispatches parsed subcommands.",
                "confidence": "medium"
            },
            {
                "path": "src/cli.rs",
                "symbol": "Command",
                "kind": "type",
                "reason": "Defines CLI command variants.",
                "confidence": "high"
            },
            {
                "path": "src/cli.rs",
                "symbol": "Command",
                "kind": "type",
                "reason": "Defines CLI command variants.",
                "confidence": "high"
            }
        ],
        "next_reads": [
            {
                "path": "src/output.rs",
                "reason": "Defines shared output metadata."
            },
            {
                "path": "src/commands/ask.rs",
                "reason": "Shows current command run flow."
            },
            {
                "path": "src/output.rs",
                "reason": "Defines shared output metadata."
            }
        ],
        "risks": [
            {
                "kind": "missing_context",
                "message": "Loaded files do not include all command modules."
            },
            {
                "kind": "missing_context",
                "message": "Loaded files do not include all command modules."
            }
        ]
    })
    .to_string()
}

fn capped_model_json() -> String {
    let matches = (0..85)
        .map(|index| {
            json!({
                "path": format!("src/file-{index:02}.rs"),
                "symbol": format!("symbol_{index:02}"),
                "kind": "function",
                "reason": format!("Reason {index:02}."),
                "confidence": if index % 2 == 0 { "high" } else { "medium" }
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
    let risks = (0..25)
        .map(|index| {
            json!({
                "kind": "missing_context",
                "message": format!("Risk {index:02}.")
            })
        })
        .collect::<Vec<_>>();

    json!({
        "matches": matches,
        "next_reads": next_reads,
        "risks": risks
    })
    .to_string()
}

fn truncated_model_json() -> String {
    json!({
        "matches": [
            {
                "path": long_string(1300),
                "symbol": long_string(1301),
                "kind": "function",
                "reason": long_string(1302),
                "confidence": "high"
            }
        ],
        "next_reads": [
            {
                "path": long_string(1303),
                "reason": long_string(1304)
            }
        ],
        "risks": [
            {
                "kind": "missing_context",
                "message": long_string(1305)
            }
        ]
    })
    .to_string()
}

fn bounded_model_json() -> String {
    let mut value = serde_json::from_str::<serde_json::Value>(&capped_model_json())
        .expect("capped json should parse");

    value["matches"][0]["path"] = json!(long_string(1300));
    value["matches"][0]["symbol"] = json!(long_string(1301));
    value["matches"][0]["reason"] = json!(long_string(1302));
    value["next_reads"][0]["path"] = json!(long_string(1303));
    value["next_reads"][0]["reason"] = json!(long_string(1304));
    value["risks"][0]["message"] = json!(long_string(1305));

    value.to_string()
}

fn long_string(len: usize) -> String {
    "x".repeat(len)
}

fn metadata() -> CommandMetadata {
    CommandMetadata::new(12420, 980)
}
