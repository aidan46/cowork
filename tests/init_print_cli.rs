//! CLI tests for `cowork init --print`.

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use assert_cmd::Command;

#[test]
fn codex_prints_rules_without_writing_files() {
    let dirs = test_dirs();

    let assert = Command::cargo_bin("cowork")
        .expect("binary should build")
        .current_dir(&dirs.project)
        .env("HOME", &dirs.home)
        .args(["init", "codex", "--print"])
        .assert()
        .success();
    let output = assert.get_output();
    let stdout = String::from_utf8(output.stdout.clone()).expect("stdout should be utf-8");

    assert!(output.stderr.is_empty());
    assert!(stdout.contains("# cowork rules for Codex"));
    assert!(stdout.contains("cowork ask"));
    assert!(stdout.contains("cowork doctor"));
    assert!(stdout.contains("next_reads"));
    assert!(stdout.contains("lead, not authority"));
    assert!(!dirs.project.join("AGENTS.md").exists());
    assert!(!dirs.project.join("CLAUDE.md").exists());
}

#[test]
fn claude_prints_rules_without_writing_files() {
    let dirs = test_dirs();

    let assert = Command::cargo_bin("cowork")
        .expect("binary should build")
        .current_dir(&dirs.project)
        .env("HOME", &dirs.home)
        .args(["init", "claude", "--print"])
        .assert()
        .success();
    let output = assert.get_output();
    let stdout = String::from_utf8(output.stdout.clone()).expect("stdout should be utf-8");

    assert!(output.stderr.is_empty());
    assert!(stdout.contains("# cowork rules for Claude"));
    assert!(stdout.contains("cowork ask"));
    assert!(stdout.contains("cowork doctor"));
    assert!(stdout.contains("next_reads"));
    assert!(stdout.contains("lead, not authority"));
    assert!(!dirs.project.join("AGENTS.md").exists());
    assert!(!dirs.project.join("CLAUDE.md").exists());
}

struct TestDirs {
    project: PathBuf,
    home: PathBuf,
}

fn test_dirs() -> TestDirs {
    let root = std::env::temp_dir().join(format!(
        "cowork-init-cli-{}-{}",
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
