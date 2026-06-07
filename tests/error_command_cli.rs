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
