//! Integration tests for cargo-bless.
//!
//! Uses `assert_cmd` to run the binary and check stdout/exit codes.

#![allow(deprecated)]

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

/// Running with --fix --dry-run should analyze and show fix preview.
#[test]
fn test_fix_dry_run() {
    cargo_bless_cmd()
        .arg("--fix")
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry-run mode"));
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

#[test]
fn test_unfinished_flags_are_hidden_from_help() {
    cargo_bless_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--fail-on").not())
        .stdout(predicate::str::contains("--workspace").not())
        .stdout(predicate::str::contains("--package").not())
        .stdout(predicate::str::contains("--all-targets").not())
        .stdout(predicate::str::contains("--llm").not());
}

#[test]
fn test_unfinished_flag_exits_nonzero() {
    cargo_bless_cmd()
        .arg("--fail-on=high")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--fail-on is not implemented"));
}

#[test]
fn test_dry_run_without_fix_exits_nonzero() {
    cargo_bless_cmd()
        .arg("--dry-run")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--dry-run requires --fix"));
}

#[test]
fn test_json_fix_combination_exits_nonzero() {
    cargo_bless_cmd()
        .arg("--json")
        .arg("--fix")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "--json cannot be combined with --fix",
        ));
}

#[test]
fn test_json_update_rules_combination_exits_nonzero() {
    cargo_bless_cmd()
        .arg("--json")
        .arg("--update-rules")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "--json cannot be combined with --update-rules",
        ));
}

#[test]
fn test_explicit_missing_policy_exits_nonzero() {
    cargo_bless_cmd()
        .arg("--policy")
        .arg("missing-bless.toml")
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing-bless.toml"));
}

#[test]
fn test_invalid_default_policy_exits_nonzero() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");
    let manifest = tmp.path().join("Cargo.toml");
    fs::write(
        &manifest,
        r#"[package]
name = "invalid-policy"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
    )
    .expect("write Cargo.toml");
    fs::create_dir_all(tmp.path().join("src")).expect("create src");
    fs::write(tmp.path().join("src/main.rs"), "fn main() {}\n").expect("write main.rs");
    fs::write(tmp.path().join("bless.toml"), "not valid toml =").expect("write bless.toml");

    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bless")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--offline");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("TOML"));
}

// ── Real-world project tests ────────────────────────────────────────

/// End-to-end test: create a temp Rust project with known outdated deps,
/// run cargo-bless, and verify it detects them all.
#[test]
fn test_real_project_with_outdated_deps() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");
    let manifest = tmp.path().join("Cargo.toml");

    // Write a Cargo.toml with deps we know our rules catch
    fs::write(
        &manifest,
        r#"[package]
name = "test-outdated"
version = "0.1.0"
edition = "2021"

[dependencies]
lazy_static = "1"
structopt = "0.3"
memmap = "0.7"
"#,
    )
    .expect("write Cargo.toml");

    // Create minimal src so cargo metadata works
    fs::create_dir_all(tmp.path().join("src")).expect("create src");
    fs::write(tmp.path().join("src/main.rs"), "fn main() {}").expect("write main.rs");

    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bless")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--offline"); // skip network for determinism

    let output = cmd.output().expect("run cargo-bless");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "cargo-bless should exit 0: {}",
        stdout
    );
    assert!(stdout.contains("lazy_static"), "should detect lazy_static");
    assert!(stdout.contains("structopt"), "should detect structopt");
    assert!(stdout.contains("memmap"), "should detect memmap");
}

/// Verify --fix --dry-run on a project with auto-fixable deps shows the diff.
#[test]
fn test_fix_dry_run_on_outdated_project() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");
    let manifest = tmp.path().join("Cargo.toml");

    fs::write(
        &manifest,
        r#"[package]
name = "test-fixable"
version = "0.1.0"
edition = "2021"

[dependencies]
reqwest = { path = "crates/reqwest" }
serde_json = { path = "crates/serde_json" }
serde = { path = "crates/serde" }
serde_derive = { path = "crates/serde_derive" }
"#,
    )
    .expect("write Cargo.toml");

    fs::create_dir_all(tmp.path().join("src")).expect("create src");
    fs::write(tmp.path().join("src/main.rs"), "fn main() {}").expect("write main.rs");
    for (name, extra) in [
        ("reqwest", "[features]\njson = []\n"),
        ("serde", "[features]\nderive = []\n"),
        ("serde_json", ""),
        ("serde_derive", ""),
    ] {
        let crate_dir = tmp.path().join("crates").join(name);
        fs::create_dir_all(crate_dir.join("src")).expect("create path crate src");
        fs::write(crate_dir.join("src/lib.rs"), "").expect("write path crate lib");
        fs::write(
            crate_dir.join("Cargo.toml"),
            format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n{extra}"
            ),
        )
        .expect("write path crate manifest");
    }

    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bless")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--fix")
        .arg("--dry-run")
        .arg("--offline");

    let output = cmd.output().expect("run cargo-bless");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "should exit 0: {}", stdout);
    assert!(stdout.contains("Dry-run"), "should show dry-run header");
    assert!(
        stdout.contains("serde_json"),
        "should list serde_json for feature optimization"
    );
    assert!(
        stdout.contains("serde_derive"),
        "should list serde_derive for feature optimization"
    );
    // serde should NOT be in the diff — it's modern
    let diff_section: &str = stdout.split("Dry-run").nth(1).unwrap_or("");
    assert!(
        !diff_section.contains("- serde"),
        "serde should not be removed"
    );
}

#[test]
fn test_bless_skips_code_audit_by_default() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");
    let manifest = tmp.path().join("Cargo.toml");

    fs::write(
        &manifest,
        r#"[package]
name = "test-bs"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
    )
    .expect("write Cargo.toml");

    fs::create_dir_all(tmp.path().join("src")).expect("create src");
    fs::write(
        tmp.path().join("src/main.rs"),
        r#"fn main() {
    let value = std::env::var("NOPE").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    println!("{}", value);
}
"#,
    )
    .expect("write main.rs");

    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bless")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--offline");

    let output = cmd.output().expect("run cargo-bless");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "should exit 0: {}", stdout);
    assert!(!stdout.contains("Bullshit detector code audit"));
    assert!(!stdout.contains("unwrap abuse"));
    assert!(!stdout.contains("sleep abuse"));
}

#[test]
fn test_audit_code_flag_runs_code_audit() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");
    let manifest = tmp.path().join("Cargo.toml");

    fs::write(
        &manifest,
        r#"[package]
name = "test-audit-code"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
    )
    .expect("write Cargo.toml");

    fs::create_dir_all(tmp.path().join("src")).expect("create src");
    fs::write(
        tmp.path().join("src/main.rs"),
        r#"fn main() {
    let value = std::env::var("NOPE").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    println!("{}", value);
}
"#,
    )
    .expect("write main.rs");

    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bless")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--offline")
        .arg("--audit-code");

    let output = cmd.output().expect("run cargo-bless");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "should exit 0: {}", stdout);
    assert!(stdout.contains("Bullshit detector code audit"));
    assert!(stdout.contains("unwrap abuse"));
    assert!(stdout.contains("sleep abuse"));
}

#[test]
fn test_no_audit_code_remains_accepted_as_noop() {
    let output = cargo_bless_cmd()
        .arg("--offline")
        .arg("--no-audit-code")
        .output()
        .expect("run cargo-bless");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "should exit 0: {}", stdout);
    assert!(!stdout.contains("Bullshit detector code audit"));
}

#[test]
fn test_direct_bs_subcommand_runs_code_audit_only() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");
    let manifest = tmp.path().join("Cargo.toml");

    fs::write(
        &manifest,
        r#"[package]
name = "test-bs-only"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
    )
    .expect("write Cargo.toml");

    fs::create_dir_all(tmp.path().join("src")).expect("create src");
    fs::write(
        tmp.path().join("src/lib.rs"),
        "pub fn bad() { thing().unwrap(); }\n",
    )
    .expect("write lib.rs");

    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bs").arg("--manifest-path").arg(&manifest);

    let output = cmd.output().expect("run cargo-bless bs");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "should exit 0: {}", stdout);
    assert!(stdout.contains("Bullshit detector code audit"));
    assert!(stdout.contains("unwrap abuse"));
    assert!(!stdout.contains("Direct dependencies"));
}

#[test]
fn test_cargo_style_bs_subcommand_runs_code_audit_only() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");
    let manifest = tmp.path().join("Cargo.toml");

    fs::write(
        &manifest,
        r#"[package]
name = "test-cargo-style-bs"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
    )
    .expect("write Cargo.toml");

    fs::create_dir_all(tmp.path().join("src")).expect("create src");
    fs::write(
        tmp.path().join("src/lib.rs"),
        "pub fn bad() { thing().unwrap(); }\n",
    )
    .expect("write lib.rs");

    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bless")
        .arg("bs")
        .arg("--manifest-path")
        .arg(&manifest);

    let output = cmd.output().expect("run cargo-bless bless bs");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "should exit 0: {}", stdout);
    assert!(stdout.contains("Bullshit detector code audit"));
    assert!(stdout.contains("unwrap abuse"));
    assert!(!stdout.contains("Direct dependencies"));
}

#[test]
fn test_json_contains_dependency_section_and_null_code_audit_by_default() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");
    let manifest = tmp.path().join("Cargo.toml");

    fs::write(
        &manifest,
        r#"[package]
name = "test-json"
version = "0.1.0"
edition = "2021"

[dependencies]
lazy_static = "1"
"#,
    )
    .expect("write Cargo.toml");

    fs::create_dir_all(tmp.path().join("src")).expect("create src");
    fs::write(
        tmp.path().join("src/main.rs"),
        "fn main() { thing().unwrap(); }\n",
    )
    .expect("write main.rs");

    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bless")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--offline")
        .arg("--json");

    let output = cmd.output().expect("run cargo-bless json");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "should exit 0: {}", stdout);
    assert!(stdout.contains("\"dependency_suggestions\""));
    assert!(stdout.contains("\"code_audit\": null"));
    assert!(stdout.contains("lazy_static"));
    assert!(!stdout.contains("UnwrapAbuse"));
}

#[test]
fn test_json_with_audit_code_contains_code_audit_section() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");
    let manifest = tmp.path().join("Cargo.toml");

    fs::write(
        &manifest,
        r#"[package]
name = "test-json-audit"
version = "0.1.0"
edition = "2021"

[dependencies]
lazy_static = "1"
"#,
    )
    .expect("write Cargo.toml");

    fs::create_dir_all(tmp.path().join("src")).expect("create src");
    fs::write(
        tmp.path().join("src/main.rs"),
        "fn main() { thing().unwrap(); }\n",
    )
    .expect("write main.rs");

    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bless")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--offline")
        .arg("--json")
        .arg("--audit-code");

    let output = cmd.output().expect("run cargo-bless json");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "should exit 0: {}", stdout);
    assert!(stdout.contains("\"dependency_suggestions\""));
    assert!(stdout.contains("\"code_audit\""));
    assert!(stdout.contains("lazy_static"));
    assert!(stdout.contains("UnwrapAbuse"));
}

#[test]
fn test_code_audit_summary_hides_extra_findings_without_verbose() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");
    let manifest = tmp.path().join("Cargo.toml");
    fs::write(
        &manifest,
        r#"[package]
name = "test-summary"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
    )
    .expect("write Cargo.toml");
    fs::create_dir_all(tmp.path().join("src")).expect("create src");
    fs::write(
        tmp.path().join("src/main.rs"),
        "fn main() {\nthing().unwrap();\nthing().unwrap();\nthing().unwrap();\nthing().unwrap();\nthing().unwrap();\nthing().unwrap();\n}\n",
    )
    .expect("write main.rs");

    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bs").arg("--manifest-path").arg(&manifest);
    let output = cmd.output().expect("run cargo-bless bs");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "should exit 0: {}", stdout);
    assert!(stdout.contains("Showing top 5"));

    let mut verbose = Command::cargo_bin("cargo-bless").expect("binary should exist");
    verbose
        .arg("bs")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--verbose");
    let output = verbose.output().expect("run cargo-bless bs --verbose");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "should exit 0: {}", stdout);
    assert!(!stdout.contains("Showing top 5"));
}

#[test]
fn test_code_audit_policy_suppresses_findings() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");
    let manifest = tmp.path().join("Cargo.toml");
    let policy = tmp.path().join("bless.toml");
    fs::write(
        &manifest,
        r#"[package]
name = "test-policy"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
    )
    .expect("write Cargo.toml");
    fs::write(
        &policy,
        r#"[code_audit]
ignore_kinds = ["UnwrapAbuse"]
"#,
    )
    .expect("write bless.toml");
    fs::create_dir_all(tmp.path().join("src")).expect("create src");
    fs::write(
        tmp.path().join("src/main.rs"),
        "fn main() { thing().unwrap(); }\n",
    )
    .expect("write main.rs");

    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bs")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--policy")
        .arg(&policy);
    let output = cmd.output().expect("run cargo-bless bs");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "should exit 0: {}", stdout);
    assert!(stdout.contains("No bullshit detected"));
}

#[test]
fn test_bs_diff_only_reports_changed_lines() {
    use std::fs;
    use std::process::Command as StdCommand;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");
    let manifest = tmp.path().join("Cargo.toml");
    let src = tmp.path().join("src/main.rs");

    fs::write(
        &manifest,
        r#"[package]
name = "test-diff"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
    )
    .expect("write Cargo.toml");
    fs::create_dir_all(tmp.path().join("src")).expect("create src");
    fs::write(&src, "fn main() {\n    println!(\"clean\");\n}\n").expect("write main.rs");

    assert!(StdCommand::new("git")
        .arg("init")
        .current_dir(tmp.path())
        .status()
        .expect("git init")
        .success());
    assert!(StdCommand::new("git")
        .args(["add", "."])
        .current_dir(tmp.path())
        .status()
        .expect("git add")
        .success());
    assert!(StdCommand::new("git")
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-m",
            "initial",
        ])
        .current_dir(tmp.path())
        .status()
        .expect("git commit")
        .success());

    fs::write(
        &src,
        "fn main() {\n    println!(\"clean\");\n    thing().unwrap();\n}\n",
    )
    .expect("modify main.rs");

    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bs")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--diff");
    let output = cmd.output().expect("run cargo-bless bs --diff");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "should exit 0: {}", stdout);
    assert!(stdout.contains("unwrap abuse"));
}
