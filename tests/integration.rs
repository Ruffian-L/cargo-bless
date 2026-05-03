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
        .stdout(predicate::str::contains("Dry-run"));
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
fn test_planned_flags_remain_hidden_from_help() {
    cargo_bless_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--all-targets"))
        .stdout(predicate::str::contains("--llm").not());
}

#[test]
fn test_fail_on_unknown_level_exits_nonzero() {
    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bless").arg("--fail-on=bogus");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("unknown --fail-on level"));
}

#[test]
fn test_fail_on_high_exits_nonzero_with_suggestions() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");
    let manifest = tmp.path().join("Cargo.toml");

    fs::write(
        &manifest,
        r#"[package]
name = "fail-on-high"
version = "0.1.0"
edition = "2021"

[dependencies]
lazy_static = "1"
"#,
    )
    .expect("write Cargo.toml");

    fs::create_dir_all(tmp.path().join("src")).expect("create src");
    fs::write(tmp.path().join("src/main.rs"), "fn main() {}").expect("write main.rs");

    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bless")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--offline")
        .arg("--fail-on=high");
    cmd.assert().failure().stderr(predicate::str::contains(
        "exiting with non-zero status: at least one dependency suggestion matched --fail-on",
    ));
}

#[test]
fn test_policy_fail_on_gates_without_cli_flag() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");
    let manifest = tmp.path().join("Cargo.toml");
    let policy_path = tmp.path().join("bless.toml");

    fs::write(
        &manifest,
        r#"[package]
name = "gate-test"
version = "0.1.0"
edition = "2021"

[dependencies]
lazy_static = "1"
"#,
    )
    .expect("write Cargo.toml");
    fs::create_dir_all(tmp.path().join("src")).expect("create src");
    fs::write(tmp.path().join("src/main.rs"), "fn main() {}").expect("write main.rs");

    fs::write(&policy_path, "fail_on = [\"high\"]\n").expect("write bless.toml");

    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bless")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--policy")
        .arg(&policy_path)
        .arg("--offline");

    cmd.assert().failure().stderr(predicate::str::contains(
        "exiting with non-zero status: at least one dependency suggestion matched --fail-on",
    ));
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
fn test_feedback_prints_block() {
    cargo_bless_cmd()
        .arg("--feedback")
        .assert()
        .success()
        .stdout(predicate::str::contains("cargo-bless feedback block"))
        .stdout(predicate::str::contains("version:"))
        .stdout(predicate::str::contains("direct_deps:"))
        .stdout(predicate::str::contains("code_audit_findings:"))
        .stdout(predicate::str::contains("top_hotspots:"));
}

#[test]
fn test_feedback_with_fix_exits_nonzero() {
    cargo_bless_cmd()
        .arg("--feedback")
        .arg("--fix")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "--feedback cannot be combined with --fix",
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
fn test_summary_prints_heading_and_counts() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");
    let manifest = tmp.path().join("Cargo.toml");

    fs::write(
        &manifest,
        r#"[package]
name = "summary-test"
version = "0.1.0"
edition = "2021"

[dependencies]
lazy_static = "1"
"#,
    )
    .expect("write Cargo.toml");

    fs::create_dir_all(tmp.path().join("src")).expect("create src");
    fs::write(tmp.path().join("src/main.rs"), "fn main() {}\n").expect("write main.rs");

    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bless")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--offline")
        .arg("--summary");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Summary"))
        .stdout(predicate::str::contains("Suggestions after policy"));
}

#[test]
fn test_workspace_flag_lists_members_and_lazy_static_hit() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");

    fs::write(
        tmp.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/alpha", "crates/bravo"]
resolver = "2"
"#,
    )
    .expect("write workspace Cargo.toml");

    for (rel, deps) in [
        (
            "crates/alpha",
            r#"lazy_static = "1"

"#,
        ),
        ("crates/bravo", ""),
    ] {
        let dir = tmp.path().join(rel);
        fs::create_dir_all(dir.join("src")).expect("create pkg src");
        fs::write(
            dir.join("Cargo.toml"),
            format!(
                r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[dependencies]
{deps}"#,
                rel.trim_start_matches("crates/"),
                deps = deps,
            ),
        )
        .expect("write member Cargo.toml");
        fs::write(dir.join("src/lib.rs"), "").expect("write lib.rs");
    }

    let root_manifest = tmp.path().join("Cargo.toml");
    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bless")
        .arg("--manifest-path")
        .arg(&root_manifest)
        .arg("--workspace")
        .arg("--offline");

    cmd.assert().success().stdout(
        predicate::str::contains("Workspace: 2 members")
            .and(predicate::str::contains("lazy_static")),
    );
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
    assert!(stdout.contains("\"cargo_bless_version\""));
    assert!(stdout.contains("\"workspace_scan\""));
    assert!(stdout.contains("\"packages\""));
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
    assert!(stdout.contains("\"cargo_bless_version\""));
    assert!(stdout.contains("\"packages\""));
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

#[test]
fn test_package_flag_filters_workspace_member() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");

    fs::write(
        tmp.path().join("Cargo.toml"),
        r#"[workspace]
members = ["alpha", "bravo"]
resolver = "2"
"#,
    )
    .expect("write root Cargo.toml");

    // alpha has lazy_static (should trigger a suggestion)
    let alpha = tmp.path().join("alpha");
    fs::create_dir_all(alpha.join("src")).expect("create alpha/src");
    fs::write(
        alpha.join("Cargo.toml"),
        r#"[package]
name = "alpha"
version = "0.1.0"
edition = "2021"

[dependencies]
lazy_static = "1"
"#,
    )
    .expect("write alpha Cargo.toml");
    fs::write(alpha.join("src/lib.rs"), "").expect("write alpha lib.rs");

    // bravo has no outdated deps
    let bravo = tmp.path().join("bravo");
    fs::create_dir_all(bravo.join("src")).expect("create bravo/src");
    fs::write(
        bravo.join("Cargo.toml"),
        r#"[package]
name = "bravo"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
    )
    .expect("write bravo Cargo.toml");
    fs::write(bravo.join("src/lib.rs"), "").expect("write bravo lib.rs");

    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bless")
        .arg("--manifest-path")
        .arg(tmp.path().join("Cargo.toml"))
        .arg("--package=alpha")
        .arg("--offline");

    let output = cmd.output().expect("run cargo-bless --package alpha");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "should exit 0: {}", stdout);
    assert!(
        stdout.contains("lazy_static"),
        "alpha's lazy_static should appear: {}",
        stdout
    );
}

#[test]
fn test_explicit_policy_flag_suppresses_suggestion() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");
    let manifest = tmp.path().join("Cargo.toml");
    let policy_path = tmp.path().join("bless.toml");

    fs::write(
        &manifest,
        r#"[package]
name = "policy-test"
version = "0.1.0"
edition = "2021"

[dependencies]
lazy_static = "1"
"#,
    )
    .expect("write Cargo.toml");

    fs::create_dir_all(tmp.path().join("src")).expect("create src");
    fs::write(tmp.path().join("src/main.rs"), "fn main() {}").expect("write main.rs");

    fs::write(
        &policy_path,
        r#"ignore_packages = ["lazy_static"]
"#,
    )
    .expect("write bless.toml");

    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bless")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--policy")
        .arg(&policy_path)
        .arg("--offline");

    let output = cmd.output().expect("run cargo-bless --policy");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "should exit 0 even when suggestions exist: {}",
        stdout
    );
    assert!(
        stdout.contains("already blessed"),
        "policy should suppress the lazy_static suggestion: {}",
        stdout
    );
}

#[test]
fn test_bs_hardcoded_flag_reports_hardcoded_values() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");
    let manifest = tmp.path().join("Cargo.toml");

    fs::write(
        &manifest,
        r#"[package]
name = "hardcoded-test"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
    )
    .expect("write Cargo.toml");

    fs::create_dir_all(tmp.path().join("src")).expect("create src");
    fs::write(
        tmp.path().join("src/main.rs"),
        "fn main() { let ip = \"192.168.1.100\"; let _ = ip; }\n",
    )
    .expect("write main.rs");

    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bs")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--hardcoded");

    let output = cmd.output().expect("run cargo-bless bs --hardcoded");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "should exit 0: {}", stdout);
    assert!(
        stdout.contains("192.168.1.100"),
        "hardcoded IP should be detected: {}",
        stdout
    );
}

#[test]
fn test_bs_sarif_flag_outputs_valid_sarif() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");
    let manifest = tmp.path().join("Cargo.toml");

    fs::write(
        &manifest,
        r#"[package]
name = "sarif-test"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
    )
    .expect("write Cargo.toml");

    fs::create_dir_all(tmp.path().join("src")).expect("create src");
    fs::write(
        tmp.path().join("src/main.rs"),
        "fn main() { let x: Option<u32> = None; let _ = x.unwrap(); }\n",
    )
    .expect("write main.rs");

    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bs")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--sarif");

    let output = cmd.output().expect("run cargo-bless bs --sarif");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "should exit 0: {}", stdout);

    let sarif: serde_json::Value =
        serde_json::from_str(&stdout).expect("--sarif output must be valid JSON");

    assert_eq!(
        sarif["version"].as_str().unwrap_or(""),
        "2.1.0",
        "SARIF version must be 2.1.0"
    );
    assert!(
        sarif["runs"].is_array(),
        "SARIF must have a 'runs' array: {}",
        stdout
    );
    let driver_name = &sarif["runs"][0]["tool"]["driver"]["name"];
    assert_eq!(
        driver_name.as_str().unwrap_or(""),
        "cargo-bless",
        "driver name must be cargo-bless"
    );
}

#[test]
fn test_bs_min_confidence_in_policy_suppresses_low_confidence_findings() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");
    let manifest = tmp.path().join("Cargo.toml");

    fs::write(
        &manifest,
        r#"[package]
name = "min-conf-test"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
    )
    .expect("write Cargo.toml");

    fs::create_dir_all(tmp.path().join("src")).expect("create src");
    // unwrap() triggers UnwrapAbuse at ~0.80 confidence; setting threshold to 0.99 should suppress it
    fs::write(
        tmp.path().join("src/main.rs"),
        "fn main() { let x: Option<u32> = None; let _ = x.unwrap(); }\n",
    )
    .expect("write main.rs");

    let policy_path = tmp.path().join("bless.toml");
    fs::write(&policy_path, "[settings]\nmin_confidence = 0.99\n").expect("write bless.toml");

    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bs")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--policy")
        .arg(&policy_path);

    let output = cmd.output().expect("run cargo-bless bs with min_confidence policy");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "should exit 0: {}", stdout);
    assert!(
        !stdout.contains("UnwrapAbuse") && !stdout.contains("unwrap"),
        "high threshold should suppress unwrap findings: {}",
        stdout
    );
}

#[test]
fn test_bs_fail_on_confidence_exits_nonzero_when_findings_present() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");
    let manifest = tmp.path().join("Cargo.toml");

    fs::write(
        &manifest,
        r#"[package]
name = "fail-conf-test"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
    )
    .expect("write Cargo.toml");

    fs::create_dir_all(tmp.path().join("src")).expect("create src");
    fs::write(
        tmp.path().join("src/main.rs"),
        "fn main() { let x: Option<u32> = None; let _ = x.unwrap(); }\n",
    )
    .expect("write main.rs");

    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bs")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--fail-on-confidence")
        .arg("0.5");

    let output = cmd.output().expect("run cargo-bless bs --fail-on-confidence");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "should exit non-zero when findings meet confidence threshold"
    );
    assert!(
        stderr.contains("exiting with non-zero status"),
        "stderr should mention exit reason: {}",
        stderr
    );
}

#[test]
fn test_init_ci_creates_workflow_file() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("temp dir");
    let manifest = tmp.path().join("Cargo.toml");

    fs::write(
        &manifest,
        r#"[package]
name = "init-ci-test"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
    )
    .expect("write Cargo.toml");

    // First run — should create the file
    let mut cmd = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd.arg("bless")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--init-ci");

    let output = cmd.output().expect("run cargo bless --init-ci");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "first run should exit 0: {}", stdout);

    let workflow_path = tmp.path().join(".github").join("workflows").join("bless.yml");
    assert!(workflow_path.exists(), ".github/workflows/bless.yml should be created");

    let contents = fs::read_to_string(&workflow_path).expect("read workflow file");
    assert!(
        contents.contains("upload-sarif"),
        "workflow should include SARIF upload step"
    );
    assert!(
        contents.contains("--fail-on high"),
        "workflow should include --fail-on high"
    );

    // Second run — should fail because file already exists
    let mut cmd2 = Command::cargo_bin("cargo-bless").expect("binary should exist");
    cmd2.arg("bless")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--init-ci");

    let output2 = cmd2.output().expect("run cargo bless --init-ci again");
    assert!(
        !output2.status.success(),
        "second run should exit non-zero (file already exists)"
    );
    let stderr2 = String::from_utf8_lossy(&output2.stderr);
    assert!(
        stderr2.contains("already exists"),
        "error should mention file already exists: {}",
        stderr2
    );
}
