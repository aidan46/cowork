//! CLI tests for `cowork init --write`.
#![allow(clippy::missing_errors_doc, reason = "test helpers stay local")]
#![allow(clippy::missing_panics_doc, reason = "test asserts and fixtures")]
#![allow(clippy::expect_used, reason = "integration test helpers stay direct")]
#![allow(clippy::unwrap_used, reason = "integration test helpers stay direct")]

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use assert_cmd::Command;
use serde_json::Value;

const CODEX_START: &str = "<!-- cowork:init:start agent=codex -->";
const CODEX_END: &str = "<!-- cowork:init:end agent=codex -->";
const CLAUDE_START: &str = "<!-- cowork:init:start agent=claude -->";
const CLAUDE_END: &str = "<!-- cowork:init:end agent=claude -->";

#[test]
fn codex_write_creates_agents_file_and_stays_idempotent() {
    let dirs = test_dirs();

    run_init_write(&dirs, "codex");
    let first =
        fs::read_to_string(dirs.project.join("AGENTS.md")).expect("codex target should write");

    assert!(first.contains(CODEX_START));
    assert!(first.contains(CODEX_END));
    assert!(first.contains("# cowork rules for Codex"));
    assert!(first.contains("cowork ask"));
    assert!(!dirs.project.join("CLAUDE.md").exists());

    run_init_write(&dirs, "codex");
    let second = fs::read_to_string(dirs.project.join("AGENTS.md"))
        .expect("codex target should stay readable");

    assert_eq!(first, second);
    assert_eq!(first.matches(CODEX_START).count(), 1);
    assert_eq!(first.matches(CODEX_END).count(), 1);
}

#[test]
fn claude_write_creates_claude_file() {
    let dirs = test_dirs();

    run_init_write(&dirs, "claude");
    let content =
        fs::read_to_string(dirs.project.join("CLAUDE.md")).expect("claude target should write");

    assert!(content.contains(CLAUDE_START));
    assert!(content.contains(CLAUDE_END));
    assert!(content.contains("# cowork rules for Claude"));
    assert!(content.contains("cowork doctor"));
    assert!(!dirs.project.join("AGENTS.md").exists());
}

#[test]
fn codex_write_replaces_existing_managed_block_only() {
    let dirs = test_dirs();
    let agents = dirs.project.join("AGENTS.md");

    fs::write(
        &agents,
        format!("before\n\n{CODEX_START}\nold rules\n{CODEX_END}\n\nafter\n"),
    )
    .expect("seed file should write");

    run_init_write(&dirs, "codex");
    let content = fs::read_to_string(&agents).expect("updated file should read");

    assert!(content.starts_with("before\n\n"));
    assert!(content.ends_with("\n\nafter\n"));
    assert!(!content.contains("old rules"));
    assert_eq!(content.matches(CODEX_START).count(), 1);
    assert_eq!(content.matches(CODEX_END).count(), 1);
}

#[test]
fn claude_write_preserves_user_content_before_and_after_block() {
    let dirs = test_dirs();
    let claude = dirs.project.join("CLAUDE.md");
    let prefix = "# user notes\n\nKeep this.\n\n";
    let suffix = "\n\n## tail\nKeep this too.\n";

    fs::write(
        &claude,
        format!("{prefix}{CLAUDE_START}\nold block\n{CLAUDE_END}{suffix}"),
    )
    .expect("seed file should write");

    run_init_write(&dirs, "claude");
    let content = fs::read_to_string(&claude).expect("updated file should read");

    assert!(content.starts_with(prefix));
    assert!(content.ends_with(suffix));
    assert!(!content.contains("old block"));
    assert!(content.contains("# cowork rules for Claude"));
}

#[test]
fn init_bad_flag_returns_invalid_arguments() {
    let dirs = test_dirs();

    let assert = Command::cargo_bin("cowork")
        .expect("binary should build")
        .current_dir(&dirs.project)
        .env("HOME", &dirs.home)
        .args(["init", "codex", "--bogus"])
        .assert()
        .code(1);
    let output = assert.get_output();
    let value = parse_json_output(&output.stdout);

    assert!(output.stderr.is_empty());
    assert_eq!(value["command"], "cli");
    assert_eq!(value["status"], "error");
    assert_eq!(value["error"]["code"], "INVALID_ARGUMENTS");
    assert_eq!(value["error"]["hint"], "Use `cowork --help`.");
    assert!(
        value["error"]["message"]
            .as_str()
            .expect("message should be string")
            .contains("--bogus")
    );
}

fn run_init_write(dirs: &TestDirs, agent: &str) {
    let assert = Command::cargo_bin("cowork")
        .expect("binary should build")
        .current_dir(&dirs.project)
        .env("HOME", &dirs.home)
        .args(["init", agent, "--write"])
        .assert()
        .success();
    let output = assert.get_output();

    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

fn parse_json_output(stdout: &[u8]) -> Value {
    serde_json::from_slice(stdout).expect("stdout should be json")
}

struct TestDirs {
    project: PathBuf,
    home: PathBuf,
}

fn test_dirs() -> TestDirs {
    let root = std::env::temp_dir().join(format!(
        "cowork-init-write-cli-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos()
    ));
    let project = root.join("project");
    let home = root.join("home");

    fs::create_dir_all(&project).expect("project dir should create");
    fs::create_dir_all(&home).expect("home dir should create");

    TestDirs { project, home }
}
