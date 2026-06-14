#![allow(clippy::missing_errors_doc, reason = "test helpers stay local")]
#![allow(clippy::missing_panics_doc, reason = "integration tests assert hard")]
#![allow(clippy::expect_used, reason = "integration test helpers stay direct")]
//! Version flag integration tests.

use assert_cmd::Command;

#[test]
fn version_flag_prints_package_metadata() {
    let assert = Command::cargo_bin("cowork")
        .expect("binary should build")
        .arg("--version")
        .assert()
        .success();

    assert
        .stdout(format!("cowork {}\n", env!("CARGO_PKG_VERSION")))
        .stderr("");
}
