//! Black-box CLI tests for command-aware JSON errors.
#![allow(clippy::missing_errors_doc, reason = "test helpers stay local")]
#![allow(clippy::missing_panics_doc, reason = "test asserts and fixtures")]
#![allow(clippy::expect_used, reason = "integration test helpers stay direct")]

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use assert_cmd::cargo::CommandCargoExt;
use serde_json::Value;

#[test]
fn locate_runtime_error_reports_locate_command() {
    let dirs = test_dirs("locate-runtime-error");
    let file = write_temp_file(&dirs.project, "input.rs", "fn main() {}\n");

    let output = run_command(
        &dirs.project,
        &dirs.home,
        &["locate", &arg_path(&file), "--thing=CLI parser"],
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());

    let json = parse_stdout(&output.stdout);
    assert_eq!(json["command"], "locate");
    assert_eq!(json["status"], "error");
    assert_eq!(json["error"]["code"], "INVALID_ARGUMENTS");
}

#[test]
fn brief_runtime_error_reports_brief_command() {
    let dirs = test_dirs("brief-runtime-error");
    let file = write_temp_file(&dirs.project, "input.rs", "fn main() {}\n");

    let output = run_command(
        &dirs.project,
        &dirs.home,
        &["brief", &arg_path(&file), "--goal=Summarize parser"],
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());

    let json = parse_stdout(&output.stdout);
    assert_eq!(json["command"], "brief");
    assert_eq!(json["status"], "error");
    assert_eq!(json["error"]["code"], "INVALID_ARGUMENTS");
}

#[test]
fn malformed_cli_still_reports_cli_command() {
    let dirs = test_dirs("malformed-cli");

    let output = run_command(&dirs.project, &dirs.home, &["brief", "--bogus"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());

    let json = parse_stdout(&output.stdout);
    assert_eq!(json["command"], "cli");
    assert_eq!(json["status"], "error");
    assert_eq!(json["error"]["code"], "INVALID_ARGUMENTS");
}

#[test]
fn brief_strict_missing_path_reports_brief_command() {
    let dirs = test_dirs("brief-strict-missing");
    let missing = dirs.project.join("missing.rs");

    let output = run_command(
        &dirs.project,
        &dirs.home,
        &["brief", &arg_path(&missing), "--goal=Summarize parser"],
    );

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());

    let json = parse_stdout(&output.stdout);
    assert_eq!(json["command"], "brief");
    assert_eq!(json["status"], "error");
    assert_eq!(json["error"]["code"], "MISSING_PATH");
}

#[test]
fn brief_no_fail_on_missing_mixed_paths_reach_runtime_flow() {
    let dirs = test_dirs("brief-mixed-paths");
    let file = write_temp_file(&dirs.project, "input.rs", "fn main() {}\n");
    let missing = dirs.project.join("missing.rs");

    let output = run_command(
        &dirs.project,
        &dirs.home,
        &[
            "brief",
            &arg_path(&file),
            &arg_path(&missing),
            "--goal=Summarize parser",
            "--no-fail-on-missing",
        ],
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());

    let json = parse_stdout(&output.stdout);
    assert_eq!(json["command"], "brief");
    assert_eq!(json["status"], "error");
    assert_eq!(json["error"]["code"], "INVALID_ARGUMENTS");
}

#[test]
fn brief_no_fail_on_missing_all_missing_paths_return_no_input_files() {
    let dirs = test_dirs("brief-all-missing");
    let missing = dirs.project.join("missing.rs");

    let output = run_command(
        &dirs.project,
        &dirs.home,
        &[
            "brief",
            &arg_path(&missing),
            "--goal=Summarize parser",
            "--no-fail-on-missing",
            "--model=gemma3:12b",
            "--host=http://127.0.0.1:1",
        ],
    );

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());

    let json = parse_stdout(&output.stdout);
    assert_eq!(json["command"], "brief");
    assert_eq!(json["status"], "error");
    assert_eq!(json["error"]["code"], "NO_INPUT_FILES");
}

#[test]
fn conflicting_missing_path_flags_report_cli_command() {
    let dirs = test_dirs("conflicting-missing-path-flags");
    let file = write_temp_file(&dirs.project, "input.rs", "fn main() {}\n");

    let output = run_command(
        &dirs.project,
        &dirs.home,
        &[
            "ask",
            &arg_path(&file),
            "--question=explain",
            "--fail-on-missing",
            "--no-fail-on-missing",
        ],
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());

    let json = parse_stdout(&output.stdout);
    assert_eq!(json["command"], "cli");
    assert_eq!(json["status"], "error");
    assert_eq!(json["error"]["code"], "INVALID_ARGUMENTS");
}

fn run_command(project_dir: &Path, home_dir: &Path, args: &[&str]) -> std::process::Output {
    Command::cargo_bin("cowork")
        .expect("binary should build")
        .current_dir(project_dir)
        .env("HOME", home_dir)
        .args(args)
        .output()
        .expect("command should run")
}

fn parse_stdout(stdout: &[u8]) -> Value {
    serde_json::from_slice(stdout).expect("stdout should be valid json")
}

fn arg_path(path: &Path) -> String {
    format!("--paths={}", path.to_string_lossy())
}

fn write_temp_file(project_dir: &Path, name: &str, content: &str) -> PathBuf {
    let path = project_dir.join(name);
    fs::write(&path, content).expect("file should write");
    path
}

struct TestDirs {
    project: PathBuf,
    home: PathBuf,
}

fn test_dirs(label: &str) -> TestDirs {
    let root = std::env::temp_dir().join(format!(
        "cowork-error-command-cli-test-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos()
    ));
    let project = root.join("project");
    let home = root.join("home");

    fs::create_dir_all(&project).expect("project dir should create");
    fs::create_dir_all(home.join(".cowork")).expect("home config dir should create");

    TestDirs { project, home }
}
