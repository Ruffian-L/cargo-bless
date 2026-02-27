//! Integration tests for cargo-bless.
//!
//! Uses `assert_cmd` to run the binary and check stdout/exit codes.

#[allow(deprecated)]
use assert_cmd::Command;
use predicates::prelude::*;

fn cargo_bless_cmd() -> Command {
    #[allow(deprecated)]
    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bless");
    cmd
}

/// Running `cargo-bless bless` in our own project should succeed and print
/// the version header plus a dependency summary.
#[test]
fn test_bless_reports_deps() {
    cargo_bless_cmd()
        .assert()
        .success()
        .stdout(predicate::str::contains("cargo-bless v"))
        .stdout(predicate::str::contains("Direct dependencies"))
        .stdout(predicate::str::contains("Found"));
}

/// Running with --fix should print a not-yet-implemented message but still exit 0.
#[test]
fn test_fix_mode_not_yet_implemented() {
    cargo_bless_cmd()
        .arg("--fix")
        .assert()
        .success()
        .stdout(predicate::str::contains("Fix mode"))
        .stdout(predicate::str::contains("not yet implemented"));
}

/// Running with --help should print usage information.
#[test]
fn test_help_flag() {
    cargo_bless_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Bless your dependencies"));
}
