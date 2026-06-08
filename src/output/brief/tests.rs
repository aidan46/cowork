#![allow(clippy::missing_errors_doc, reason = "test helpers stay local")]
#![allow(clippy::missing_panics_doc, reason = "test asserts and fixtures")]

use serde_json::json;

use super::*;
use crate::{error::AppError, output::CommandMetadata};

#[test]
fn valid_model_json_parses_to_typed_success_output() {
    let output = parse_brief_output(&valid_model_json(), "trace CLI flow", metadata())
        .expect("output should parse");

    assert_eq!(output.goal, "trace CLI flow");
    assert_eq!(output.files.len(), 2);
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
    let output = parse_brief_output(
        &unsorted_duplicate_model_json(),
        "trace CLI flow",
        metadata(),
    )
    .expect("output should parse");

    assert_eq!(
        serde_json::to_value(&output.files).expect("files should serialize"),
        json!([
            {
                "path": "src/cli.rs",
                "role": "Defines brief args.",
                "key_points": ["Adds required goal flag."],
                "bytes": 3200
            },
            {
                "path": "src/commands.rs",
                "role": "Dispatches brief runner.",
                "key_points": ["Routes parsed command to module."],
                "bytes": 620
            }
        ])
    );
    assert_eq!(
        serde_json::to_value(&output.symbols).expect("symbols should serialize"),
        json!([
            {
                "name": "Command",
                "kind": "type",
                "path": "src/cli.rs",
                "responsibility": "Defines CLI command enum."
            },
            {
                "name": "run_brief",
                "kind": "function",
                "path": "src/commands/brief.rs",
                "responsibility": "Runs brief command and prints JSON."
            }
        ])
    );
    assert_eq!(
        serde_json::to_value(&output.evidence).expect("evidence should serialize"),
        json!([
            {
                "path": "src/cli.rs",
                "symbol": "Command",
                "note": "CLI enum contains brief variant."
            },
            {
                "path": "src/commands.rs",
                "symbol": "try_run",
                "note": "Match arm dispatches brief args into runner."
            }
        ])
    );
    assert_eq!(
        serde_json::to_value(&output.next_reads).expect("next reads should serialize"),
        json!([
            {
                "path": "src/output.rs",
                "reason": "Defines shared output metadata."
            },
            {
                "path": "src/output/brief.rs",
                "reason": "Shows final JSON shape."
            }
        ])
    );
}

#[test]
fn serialized_json_uses_cli_owned_goal_and_metadata() {
    let output = parse_brief_output(&valid_model_json(), "trace CLI flow", metadata())
        .expect("output should parse");
    let value = serde_json::from_str::<serde_json::Value>(
        &output.into_json().expect("output should serialize"),
    )
    .expect("json should parse");

    assert_eq!(value["goal"], "trace CLI flow");
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
    let output = parse_brief_output(&valid_model_json(), "trace CLI flow", metadata())
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
            "command": "brief",
            "status": "ok",
            "goal": "trace CLI flow",
            "brief": {
                "summary": "CLI flow starts in main and dispatches command runner.",
                "confidence": "high",
                "not_found": false
            },
            "files": [
                {
                    "path": "src/cli.rs",
                    "role": "Defines brief args.",
                    "key_points": ["Adds required goal flag."],
                    "bytes": 3200
                },
                {
                    "path": "src/commands.rs",
                    "role": "Dispatches brief runner.",
                    "key_points": ["Routes parsed command to module."],
                    "bytes": 620
                }
            ],
            "symbols": [
                {
                    "name": "run_brief",
                    "kind": "function",
                    "path": "src/commands/brief.rs",
                    "responsibility": "Runs brief command and prints JSON."
                }
            ],
            "evidence": [
                {
                    "path": "src/commands.rs",
                    "symbol": "try_run",
                    "note": "Match arm dispatches brief args into runner."
                }
            ],
            "risks": [
                {
                    "kind": "missing_context",
                    "message": "Loaded files omit output parser details."
                }
            ],
            "next_reads": [
                {
                    "path": "src/output/brief.rs",
                    "reason": "Shows final JSON shape."
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
fn parsed_output_caps_arrays_and_key_points_and_injects_cap_risk() {
    let output = parse_brief_output(&capped_model_json(), "trace CLI flow", metadata())
        .expect("output should parse");

    assert_eq!(output.files.len(), 40);
    assert_eq!(output.files[0].key_points.len(), 20);
    assert_eq!(output.symbols.len(), 80);
    assert_eq!(output.evidence.len(), 80);
    assert_eq!(output.risks.len(), 20);
    assert_eq!(output.next_reads.len(), 20);
    assert!(output.risks.iter().any(|risk| {
        risk == &BriefRisk {
            kind: BriefRiskKind::Unknown,
            message: "Output capped: files 45->40, files.key_points 25->20, symbols 85->80, evidence 85->80, risks kept 19 of 25 model rows, next_reads 25->20.".to_string(),
        }
    }));
}

#[test]
fn serialized_json_truncates_long_strings_and_nested_key_points() {
    let output = parse_brief_output(&truncated_model_json(), "trace CLI flow", metadata())
        .expect("output should parse");
    let value = serde_json::from_str::<serde_json::Value>(
        &output.into_json().expect("output should serialize"),
    )
    .expect("json should parse");

    assert_eq!(
        value["brief"]["summary"]
            .as_str()
            .map(|summary| summary.chars().count()),
        Some(1200)
    );
    assert!(
        value["brief"]["summary"]
            .as_str()
            .is_some_and(|summary| summary.ends_with(" [truncated]"))
    );
    assert!(
        value["files"][0]["key_points"][0]
            .as_str()
            .is_some_and(|point| point.ends_with(" [truncated]"))
    );
    assert_eq!(
        value["risks"][1],
        json!({
            "kind": "unknown",
            "message": "Output truncated at 1200 chars: brief.summary x1, files.path x1, files.role x1, files.key_points x2, symbols.name x1, symbols.path x1, symbols.responsibility x1, evidence.path x1, evidence.symbol x1, evidence.note x1, risks.message x1, next_reads.path x1, next_reads.reason x1."
        })
    );
}

#[test]
fn serialized_json_adds_both_notices_deterministically_and_stays_stable() {
    let json = parse_brief_output(&bounded_model_json(), "trace CLI flow", metadata())
        .expect("output should parse")
        .into_json()
        .expect("output should serialize");
    let json_again = parse_brief_output(&bounded_model_json(), "trace CLI flow", metadata())
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
            "message": "Output capped: files 45->40, files.key_points 25->20, symbols 85->80, evidence 85->80, risks kept 18 of 25 model rows, next_reads 25->20."
        })
    );
    assert_eq!(
        risks[19],
        json!({
            "kind": "unknown",
            "message": "Output truncated at 1200 chars: brief.summary x1, files.path x1, files.role x1, files.key_points x2, symbols.name x1, symbols.path x1, symbols.responsibility x1, evidence.path x1, evidence.symbol x1, evidence.note x1, risks.message x1, next_reads.path x1, next_reads.reason x1."
        })
    );
}

#[test]
fn missing_required_field_maps_to_response_parse_failed() {
    let error = parse_brief_output(
        r#"{
            "brief":{"summary":"summary","confidence":"high","not_found":false},
            "files":[],
            "symbols":[],
            "evidence":[],
            "risks":[]
        }"#,
        "trace CLI flow",
        metadata(),
    )
    .expect_err("parse should fail");

    assert!(matches!(error, AppError::ResponseParseFailed { .. }));
}

#[test]
fn bad_enum_value_maps_to_response_parse_failed() {
    let error = parse_brief_output(
        r#"{
            "brief":{"summary":"summary","confidence":"certain","not_found":false},
            "files":[],
            "symbols":[],
            "evidence":[],
            "risks":[],
            "next_reads":[]
        }"#,
        "trace CLI flow",
        metadata(),
    )
    .expect_err("parse should fail");

    assert!(matches!(error, AppError::ResponseParseFailed { .. }));
}

fn valid_model_json() -> String {
    json!({
        "schema_version": "9.9",
        "command": "wrong",
        "status": "bad",
        "goal": "wrong goal",
        "brief": {
            "summary": "CLI flow starts in main and dispatches command runner.",
            "confidence": "high",
            "not_found": false
        },
        "files": [
            {
                "path": "src/commands.rs",
                "role": "Dispatches brief runner.",
                "key_points": ["Routes parsed command to module."],
                "bytes": 620
            },
            {
                "path": "src/cli.rs",
                "role": "Defines brief args.",
                "key_points": ["Adds required goal flag."],
                "bytes": 3200
            }
        ],
        "symbols": [
            {
                "name": "run_brief",
                "kind": "function",
                "path": "src/commands/brief.rs",
                "responsibility": "Runs brief command and prints JSON."
            }
        ],
        "evidence": [
            {
                "path": "src/commands.rs",
                "symbol": "try_run",
                "note": "Match arm dispatches brief args into runner."
            }
        ],
        "risks": [
            {
                "kind": "missing_context",
                "message": "Loaded files omit output parser details."
            }
        ],
        "next_reads": [
            {
                "path": "src/output/brief.rs",
                "reason": "Shows final JSON shape."
            }
        ]
    })
    .to_string()
}

fn unsorted_duplicate_model_json() -> String {
    json!({
        "brief": {
            "summary": "CLI flow starts in main and dispatches command runner.",
            "confidence": "high",
            "not_found": false
        },
        "files": [
            {
                "path": "src/commands.rs",
                "role": "Dispatches brief runner.",
                "key_points": ["Routes parsed command to module."],
                "bytes": 620
            },
            {
                "path": "src/cli.rs",
                "role": "Defines brief args.",
                "key_points": ["Adds required goal flag."],
                "bytes": 3200
            },
            {
                "path": "src/cli.rs",
                "role": "Defines brief args.",
                "key_points": ["Adds required goal flag."],
                "bytes": 3200
            }
        ],
        "symbols": [
            {
                "name": "run_brief",
                "kind": "function",
                "path": "src/commands/brief.rs",
                "responsibility": "Runs brief command and prints JSON."
            },
            {
                "name": "Command",
                "kind": "type",
                "path": "src/cli.rs",
                "responsibility": "Defines CLI command enum."
            },
            {
                "name": "run_brief",
                "kind": "function",
                "path": "src/commands/brief.rs",
                "responsibility": "Runs brief command and prints JSON."
            }
        ],
        "evidence": [
            {
                "path": "src/commands.rs",
                "symbol": "try_run",
                "note": "Match arm dispatches brief args into runner."
            },
            {
                "path": "src/cli.rs",
                "symbol": "Command",
                "note": "CLI enum contains brief variant."
            },
            {
                "path": "src/commands.rs",
                "symbol": "try_run",
                "note": "Match arm dispatches brief args into runner."
            }
        ],
        "risks": [
            {
                "kind": "missing_context",
                "message": "Loaded files omit output parser details."
            },
            {
                "kind": "missing_context",
                "message": "Loaded files omit output parser details."
            }
        ],
        "next_reads": [
            {
                "path": "src/output.rs",
                "reason": "Defines shared output metadata."
            },
            {
                "path": "src/output/brief.rs",
                "reason": "Shows final JSON shape."
            },
            {
                "path": "src/output.rs",
                "reason": "Defines shared output metadata."
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
                "role": format!("Role {index:02}."),
                "key_points": if index == 0 {
                    (0..25)
                        .map(|point| format!("Point {point:02}."))
                        .collect::<Vec<_>>()
                } else {
                    vec![format!("Point {index:02}.")]
                },
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
                "responsibility": format!("Responsibility {index:02}.")
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
        "brief": {
            "summary": "CLI flow starts in main and dispatches command runner.",
            "confidence": "high",
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
    json!({
        "brief": {
            "summary": long_string(1300),
            "confidence": "high",
            "not_found": false
        },
        "files": [
            {
                "path": long_string(1301),
                "role": long_string(1302),
                "key_points": [long_string(1303), long_string(1304)],
                "bytes": 1
            }
        ],
        "symbols": [
            {
                "name": long_string(1305),
                "kind": "function",
                "path": long_string(1306),
                "responsibility": long_string(1307)
            }
        ],
        "evidence": [
            {
                "path": long_string(1308),
                "symbol": long_string(1309),
                "note": long_string(1310)
            }
        ],
        "risks": [
            {
                "kind": "missing_context",
                "message": long_string(1311)
            }
        ],
        "next_reads": [
            {
                "path": long_string(1312),
                "reason": long_string(1313)
            }
        ]
    })
    .to_string()
}

fn bounded_model_json() -> String {
    let mut value = serde_json::from_str::<serde_json::Value>(&capped_model_json())
        .expect("capped json should parse");

    value["brief"]["summary"] = json!(long_string(1300));
    value["files"][0]["path"] = json!(long_string(1301));
    value["files"][0]["role"] = json!(long_string(1302));
    value["files"][0]["key_points"][0] = json!(long_string(1303));
    value["files"][0]["key_points"][1] = json!(long_string(1304));
    value["symbols"][0]["name"] = json!(long_string(1305));
    value["symbols"][0]["path"] = json!(long_string(1306));
    value["symbols"][0]["responsibility"] = json!(long_string(1307));
    value["evidence"][0]["path"] = json!(long_string(1308));
    value["evidence"][0]["symbol"] = json!(long_string(1309));
    value["evidence"][0]["note"] = json!(long_string(1310));
    value["risks"][0]["message"] = json!(long_string(1311));
    value["next_reads"][0]["path"] = json!(long_string(1312));
    value["next_reads"][0]["reason"] = json!(long_string(1313));

    value.to_string()
}

fn long_string(len: usize) -> String {
    "x".repeat(len)
}

fn metadata() -> CommandMetadata {
    CommandMetadata::new(12420, 980)
}
