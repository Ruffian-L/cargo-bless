use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Parser;
use colored::*;

mod cli;

fn main() -> Result<()> {
    let args = cli::Cli::parse();

    match args.command {
        cli::Commands::Bless(command) => match command.command {
            Some(cli::BlessSubcommand::Bs(opts)) => run_code_audit_command(opts),
            None => run_bless_command(command.opts),
        },
        cli::Commands::Bs(opts) => run_code_audit_command(opts),
    }
}

fn use_tagged_suggestions(opts: &cli::BlessOpts) -> bool {
    opts.workspace || !opts.package.is_empty()
}


fn parse_fail_on_levels(
    raw: &[String],
) -> Result<Option<HashSet<cargo_bless::suggestions::Impact>>> {
    if raw.is_empty() {
        return Ok(None);
    }
    let mut out = HashSet::new();
    for part in raw {
        let p = part.trim().to_ascii_lowercase();
        if p.is_empty() {
            continue;
        }
        match p.as_str() {
            "low" => {
                out.insert(cargo_bless::suggestions::Impact::Low);
            }
            "medium" => {
                out.insert(cargo_bless::suggestions::Impact::Medium);
            }
            "high" => {
                out.insert(cargo_bless::suggestions::Impact::High);
            }
            // Reserved alias: dependency suggestions only map to Low/Medium/High today.
            "critical" => {
                out.insert(cargo_bless::suggestions::Impact::High);
            }
            other => {
                bail!(
                    "unknown --fail-on level {:?} (expected low, medium, high, or critical)",
                    other
                );
            }
        }
    }
    if out.is_empty() {
        return Ok(None);
    }
    Ok(Some(out))
}

fn dependency_fail_triggered(
    suggestions: &[cargo_bless::suggestions::Suggestion],
    levels: &HashSet<cargo_bless::suggestions::Impact>,
) -> bool {
    suggestions.iter().any(|s| levels.contains(&s.impact))
}

fn maybe_fail_on_exit(
    suggestions: &[cargo_bless::suggestions::Suggestion],
    levels: &Option<HashSet<cargo_bless::suggestions::Impact>>,
) -> Result<()> {
    if let Some(set) = levels {
        if dependency_fail_triggered(suggestions, set) {
            bail!("exiting with non-zero status: at least one dependency suggestion matched --fail-on");
        }
    }
    Ok(())
}

fn run_bless_command(opts: cli::BlessOpts) -> Result<()> {
    if opts.init_ci {
        return run_init_ci(opts.manifest_path.as_deref());
    }
    if opts.init_hooks {
        return run_init_hooks(opts.manifest_path.as_deref());
    }
    if let Some(ref pattern) = opts.explain {
        return run_explain(pattern);
    }
    reject_invalid_flag_combinations(&opts)?;
    reject_unfinished_flags(&opts)?;
    if opts.feedback {
        return run_feedback_command(opts);
    }

    let manifest = opts.manifest_path.as_deref();
    let policy = load_policy(opts.policy.as_deref(), manifest)?;

    let effective_fail_on: Vec<String> = if opts.fail_on.is_empty() {
        policy.as_ref()
            .and_then(|p| p.fail_on.clone())
            .unwrap_or_default()
    } else {
        opts.fail_on.clone()
    };
    let fail_levels = parse_fail_on_levels(&effective_fail_on)?;

    if opts.summary {
        return run_summary_mode(&opts, &fail_levels, policy.as_ref());
    }

    let run_code_audit = opts.audit_code;
    let code_audit_config = cargo_bless::code_audit::config_from_policy(policy.as_ref());
    let tagged = use_tagged_suggestions(&opts);
    let effective_all_targets = opts.all_targets
        || policy.as_ref().is_some_and(|p| p.settings.all_targets);
    let snapshots = cargo_bless::parser::get_package_snapshots(
        manifest,
        opts.workspace,
        &opts.package,
        effective_all_targets,
    )?;
    let rules = cargo_bless::suggestions::load_rules();

    let mut per_pkg_suggestions: Vec<Vec<cargo_bless::suggestions::Suggestion>> = Vec::new();
    for snap in &snapshots {
        let mpath = snap.manifest_path.as_path();
        let raw = if tagged {
            cargo_bless::suggestions::analyze_for_package(
                Some(mpath),
                &snap.deps,
                &rules,
                Some(snap.name.as_str()),
            )
        } else {
            cargo_bless::suggestions::analyze(Some(mpath), &snap.deps, &rules)
        };
        per_pkg_suggestions.push(apply_policy(raw, policy.as_ref()));
    }

    let all_suggestions: Vec<cargo_bless::suggestions::Suggestion> =
        per_pkg_suggestions.iter().flatten().cloned().collect();

    let packages_for_json: Vec<cargo_bless::output::JsonPackageOutput<'_>> = snapshots
        .iter()
        .zip(per_pkg_suggestions.iter())
        .map(|(snap, sug)| cargo_bless::output::JsonPackageOutput {
            name: snap.name.as_str(),
            version: snap.version.as_str(),
            manifest_path: snap.manifest_path.display().to_string(),
            dependency_suggestions: sug.as_slice(),
        })
        .collect();

    if opts.json {
        let merged_audit = if run_code_audit {
            let reports: Vec<_> = snapshots
                .iter()
                .map(|s| {
                    cargo_bless::code_audit::scan_project(
                        Some(s.manifest_path.as_path()),
                        &code_audit_config,
                    )
                })
                .collect::<Result<_, _>>()?;
            Some(cargo_bless::code_audit::merge_reports(reports))
        } else {
            None
        };
        let advisories_for_json = if !opts.offline
            && !policy.as_ref().is_some_and(|p| p.settings.offline)
            && !opts.no_advisories
        {
            let crates: Vec<&str> = snapshots
                .iter()
                .flat_map(|s| s.deps.iter())
                .filter(|d| d.is_direct)
                .map(|d| d.name.as_str())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            cargo_bless::advisories::fetch_advisories_batch(&crates)
        } else {
            Vec::new()
        };
        let report = cargo_bless::output::JsonReportUnified {
            cargo_bless_version: env!("CARGO_PKG_VERSION"),
            workspace_scan: opts.workspace || snapshots.len() > 1,
            packages: packages_for_json,
            code_audit: merged_audit.as_ref(),
            hardcoded_values: None,
            security_advisories: advisories_for_json,
        };
        cargo_bless::output::render_unified_json(report);
        maybe_fail_on_exit(&all_suggestions, &fail_levels)?;
        return Ok(());
    }

    println!("🙏 cargo-bless v{}", env!("CARGO_PKG_VERSION"));
    println!();

    if opts.update_rules {
        cargo_bless::updater::update_rules()?;
        println!();
        println!("Rules updated. Run `cargo bless` to use them.");
        return Ok(());
    }

    if opts.fix {
        if opts.dry_run {
            println!(
                "🔍 Dry-run — previewing Cargo.toml edits only (no writes, no `cargo update`)"
            );
            println!(
                "{}",
                "   `--fix` never modifies Rust sources; manifests may get a `.toml.bak` when applied."
                    .dimmed()
            );
        } else {
            println!(
                "{}",
                "🔧 Applying Cargo.toml autofixes (Rust source is never touched)".bold()
            );
        }
        println!();
    }

    println!("📋 Scanning dependencies...");
    println!();

    if snapshots.len() == 1 {
        let snap = &snapshots[0];
        let deps = &snap.deps;
        let direct: Vec<_> = deps.iter().filter(|d| d.is_direct).collect();
        let transitive: Vec<_> = deps.iter().filter(|d| !d.is_direct).collect();

        println!(
            "{}",
            format!("📦 Direct dependencies ({})", direct.len()).bold()
        );
        for dep in &direct {
            let features_str = if dep.enabled_features.is_empty() {
                String::new()
            } else {
                format!(" [{}]", dep.enabled_features.join(", "))
            };
            println!(
                "  {} {} {}{}",
                "•".green(),
                dep.name.bold(),
                dep.version.dimmed(),
                features_str.dimmed()
            );
        }

        println!();
        println!(
            "{}",
            format!("📎 Transitive dependencies ({})", transitive.len()).dimmed()
        );

        println!();
        println!(
            "{}",
            format!("Found {} direct deps, {} total.", direct.len(), deps.len()).bold()
        );
    } else {
        for snap in &snapshots {
            let direct = snap.deps.iter().filter(|d| d.is_direct).count();
            let total = snap.deps.len();
            println!(
                "  {} {} v{} — {} direct, {} total ({})",
                "•".green(),
                snap.name.bold(),
                snap.version.dimmed(),
                direct,
                total,
                snap.manifest_path.display().to_string().dimmed()
            );
        }
        println!();
        let total_direct: usize = snapshots
            .iter()
            .map(|s| s.deps.iter().filter(|d| d.is_direct).count())
            .sum();
        let total_all: usize = snapshots.iter().map(|s| s.deps.len()).sum();
        println!(
            "{}",
            format!(
                "Workspace: {} members · {} direct deps (sum) · {} resolved rows (sum).",
                snapshots.len(),
                total_direct,
                total_all
            )
            .bold()
        );
    }

    println!();
    let effective_offline = opts.offline
        || policy
            .as_ref()
            .is_some_and(|policy| policy.settings.offline);
    let intel = if !effective_offline && !all_suggestions.is_empty() {
        let crate_names: Vec<&str> = all_suggestions
            .iter()
            .flat_map(|s| s.current.split('+'))
            .collect();

        match cargo_bless::intel::IntelClient::new() {
            Ok(client) => {
                println!("{}", "🌐 Fetching live intelligence...".dimmed());
                let result = client.fetch_bulk_intel(&crate_names);
                if result.is_empty() && !crate_names.is_empty() {
                    println!(
                        "{}",
                        "⚠️  Live data unavailable (offline or rate-limited)"
                            .yellow()
                            .dimmed()
                    );
                }
                println!();
                result
            }
            Err(_) => {
                println!(
                    "{}",
                    "⚠️  Could not initialize live intelligence (continuing without)"
                        .yellow()
                        .dimmed()
                );
                println!();
                HashMap::new()
            }
        }
    } else {
        HashMap::new()
    };

    let views: Vec<cargo_bless::output::PackageSuggestionView<'_>> = snapshots
        .iter()
        .zip(per_pkg_suggestions.iter())
        .map(|(snap, sug)| cargo_bless::output::PackageSuggestionView {
            name: snap.name.as_str(),
            version: snap.version.as_str(),
            manifest_path: snap.manifest_path.as_path(),
            suggestions: sug.as_slice(),
        })
        .collect();

    cargo_bless::output::render_packages_modernization(&views, &intel);

    // Security advisory check (osv.dev) — one batch call for all direct deps
    if !effective_offline && !opts.no_advisories {
        let direct_crates: Vec<&str> = {
            let mut seen = std::collections::HashSet::new();
            snapshots
                .iter()
                .flat_map(|s| s.deps.iter())
                .filter(|d| d.is_direct)
                .map(|d| d.name.as_str())
                .filter(|n| seen.insert(*n))
                .collect()
        };
        if !direct_crates.is_empty() {
            let hits = cargo_bless::advisories::fetch_advisories_batch(&direct_crates);
            cargo_bless::output::render_advisories(&hits);
        }
    }

    if run_code_audit {
        let reports: Vec<_> = snapshots
            .iter()
            .map(|s| {
                cargo_bless::code_audit::scan_project(
                    Some(s.manifest_path.as_path()),
                    &code_audit_config,
                )
            })
            .collect::<Result<_, _>>()?;
        let merged = cargo_bless::code_audit::merge_reports(reports);
        cargo_bless::output::render_code_audit_report(&merged, opts.verbose);
    }

    if opts.fix && !all_suggestions.is_empty() {
        println!();
        for snap in &snapshots {
            let pkg_sugs: Vec<cargo_bless::suggestions::Suggestion> =
                if use_tagged_suggestions(&opts) {
                    all_suggestions
                        .iter()
                        .filter(|s| s.package.as_deref() == Some(snap.name.as_str()))
                        .cloned()
                        .collect()
                } else {
                    all_suggestions.clone()
                };
            if pkg_sugs.is_empty() {
                continue;
            }
            println!(
                "{}",
                format!(
                    "── Autofix: {} ({}) ──",
                    snap.name,
                    snap.manifest_path.display()
                )
                .dimmed()
            );
            cargo_bless::fix::apply(&pkg_sugs, &snap.manifest_path, opts.dry_run)?;
        }
    }

    maybe_fail_on_exit(&all_suggestions, &fail_levels)?;

    Ok(())
}

fn run_summary_mode(
    opts: &cli::BlessOpts,
    fail_levels: &Option<HashSet<cargo_bless::suggestions::Impact>>,
    policy: Option<&cargo_bless::policy::Policy>,
) -> Result<()> {
    let manifest = opts.manifest_path.as_deref();
    let tagged = use_tagged_suggestions(opts);
    let effective_all_targets = opts.all_targets
        || policy.is_some_and(|p| p.settings.all_targets);
    let snapshots = cargo_bless::parser::get_package_snapshots(
        manifest,
        opts.workspace,
        &opts.package,
        effective_all_targets,
    )?;
    let rules = cargo_bless::suggestions::load_rules();

    let mut all = Vec::new();
    for snap in &snapshots {
        let mpath = snap.manifest_path.as_path();
        let raw = if tagged {
            cargo_bless::suggestions::analyze_for_package(
                Some(mpath),
                &snap.deps,
                &rules,
                Some(snap.name.as_str()),
            )
        } else {
            cargo_bless::suggestions::analyze(Some(mpath), &snap.deps, &rules)
        };
        all.extend(apply_policy(raw, policy));
    }

    let scan_stats: Vec<(&str, usize, usize)> = snapshots
        .iter()
        .map(|s| {
            (
                s.name.as_str(),
                s.deps.iter().filter(|d| d.is_direct).count(),
                s.deps.len(),
            )
        })
        .collect();

    println!("🙏 cargo-bless v{}", env!("CARGO_PKG_VERSION"));
    println!();
    cargo_bless::output::render_summary(&scan_stats, &all);
    println!();

    maybe_fail_on_exit(&all, fail_levels)?;
    Ok(())
}

fn run_feedback_command(opts: cli::BlessOpts) -> Result<()> {
    let manifest = opts.manifest_path.as_deref();
    let policy = load_policy(opts.policy.as_deref(), manifest)?;
    let code_audit_config = cargo_bless::code_audit::config_from_policy(policy.as_ref());

    let deps = cargo_bless::parser::get_deps(manifest)?;
    let direct_count = deps.iter().filter(|d| d.is_direct).count();

    let rules = cargo_bless::suggestions::load_rules();
    let suggestions = apply_policy(
        cargo_bless::suggestions::analyze(manifest, &deps, &rules),
        policy.as_ref(),
    );

    let report = cargo_bless::code_audit::scan_project(manifest, &code_audit_config)?;

    cargo_bless::feedback::emit_feedback_stdout(
        env!("CARGO_PKG_VERSION"),
        manifest,
        direct_count,
        deps.len(),
        &suggestions,
        &report,
    )
}

fn run_code_audit_command(opts: cli::CodeAuditOpts) -> Result<()> {
    let manifest = opts.manifest_path.as_deref();
    let policy = load_policy(opts.policy.as_deref(), manifest)?;
    let code_audit_config = cargo_bless::code_audit::config_from_policy(policy.as_ref());
    let report = if opts.diff {
        cargo_bless::code_audit::scan_git_diff(manifest, &code_audit_config)?
    } else {
        cargo_bless::code_audit::scan_project(manifest, &code_audit_config)?
    };

    let bs_hits = if opts.hardcoded {
        let root = manifest
            .and_then(Path::parent)
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        cargo_bless::bs_detector::scan_dir(root)
    } else {
        Vec::new()
    };

    if opts.sarif {
        cargo_bless::output::render_sarif(&report);
    } else if opts.json {
        let unified = cargo_bless::output::JsonReportUnified {
            cargo_bless_version: env!("CARGO_PKG_VERSION"),
            workspace_scan: false,
            packages: Vec::new(),
            code_audit: Some(&report),
            hardcoded_values: if opts.hardcoded { Some(&bs_hits) } else { None },
            security_advisories: Vec::new(),
        };
        cargo_bless::output::render_unified_json(unified);
    } else {
        println!("🙏 cargo-bless v{}", env!("CARGO_PKG_VERSION"));
        cargo_bless::output::render_code_audit_report(&report, opts.verbose);
        if opts.hardcoded {
            println!();
            cargo_bless::bs_detector::render_bs_hits(&bs_hits);
        }
    }

    if opts.fix {
        apply_code_audit_fixes(&report, opts.dry_run)?;
    }

    if let Some(threshold) = opts.fail_on_confidence {
        let gated_count = report
            .alerts
            .iter()
            .filter(|a| a.confidence as f64 >= threshold)
            .count();
        if gated_count > 0 {
            eprintln!(
                "cargo-bless: {} finding(s) at confidence >= {:.2}; exiting with non-zero status.",
                gated_count, threshold
            );
            std::process::exit(1);
        }
    }

    Ok(())
}

fn load_policy(
    explicit_path: Option<&Path>,
    manifest_path: Option<&Path>,
) -> Result<Option<cargo_bless::policy::Policy>> {
    match explicit_path {
        Some(path) => cargo_bless::policy::try_load_policy(path).map(Some),
        None => {
            let path = default_policy_path(manifest_path);
            if path.exists() {
                cargo_bless::policy::try_load_policy(&path).map(Some)
            } else {
                Ok(None)
            }
        }
    }
}

fn default_policy_path(manifest_path: Option<&Path>) -> PathBuf {
    manifest_path
        .and_then(Path::parent)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join("bless.toml")
}

const INIT_CI_WORKFLOW: &str = r#"name: cargo-bless

on:
  push:
    branches: [main]
  pull_request:

jobs:
  bless:
    runs-on: ubuntu-latest
    permissions:
      security-events: write  # required for upload-sarif

    steps:
      - uses: actions/checkout@v4

      - name: Set up Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Cache Rust dependencies
        uses: Swatinem/rust-cache@v2

      - name: Install cargo-bless
        run: cargo install cargo-bless --locked

      - name: Dependency modernization check
        run: cargo bless --fail-on high --offline

      - name: Code audit (SARIF)
        run: cargo bless bs --sarif > bless-audit.sarif
        continue-on-error: true

      - name: Upload SARIF to GitHub code scanning
        uses: github/codeql-action/upload-sarif@v3
        if: always()
        with:
          sarif_file: bless-audit.sarif
"#;

fn run_init_ci(manifest_path: Option<&Path>) -> Result<()> {
    let base = manifest_path
        .and_then(Path::parent)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let workflow_dir = base.join(".github").join("workflows");
    let workflow_path = workflow_dir.join("bless.yml");

    if workflow_path.exists() {
        bail!(
            "{} already exists — delete it first to regenerate.",
            workflow_path.display()
        );
    }

    std::fs::create_dir_all(&workflow_dir)
        .with_context(|| format!("failed to create {}", workflow_dir.display()))?;
    std::fs::write(&workflow_path, INIT_CI_WORKFLOW)
        .with_context(|| format!("failed to write {}", workflow_path.display()))?;

    println!("🙏 Created {}", workflow_path.display());
    println!("   Commit and push to enable cargo-bless CI.");
    println!();
    println!("   The workflow:");
    println!("     • Gates merges when any dep suggestion has --fail-on high impact");
    println!("     • Uploads code-audit findings as SARIF (GitHub code scanning / PR annotations)");
    println!();
    println!("   Tip: add fail_on = [\"high\"] to bless.toml to enforce the gate without repeating the flag.");

    Ok(())
}

fn run_init_hooks(manifest_path: Option<&Path>) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let base = manifest_path
        .and_then(Path::parent)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let git_root_output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(base)
        .output()
        .context("failed to run git rev-parse (is this a git repository?)")?;

    if !git_root_output.status.success() {
        bail!("not inside a git repository — cannot install a pre-commit hook");
    }

    let git_root = Path::new(std::str::from_utf8(&git_root_output.stdout)?.trim());
    let hooks_dir = git_root.join(".git").join("hooks");
    let hook_path = hooks_dir.join("pre-commit");

    if hook_path.exists() {
        bail!(
            "{} already exists — delete it first to regenerate.",
            hook_path.display()
        );
    }

    std::fs::create_dir_all(&hooks_dir)
        .with_context(|| format!("failed to create {}", hooks_dir.display()))?;

    let hook_script = "#!/bin/sh\ncargo bless bs --fail-on-confidence 0.8\n";
    std::fs::write(&hook_path, hook_script)
        .with_context(|| format!("failed to write {}", hook_path.display()))?;
    std::fs::set_permissions(&hook_path, std::fs::Permissions::from_mode(0o755))
        .with_context(|| format!("failed to chmod {}", hook_path.display()))?;

    println!("🙏 Created {}", hook_path.display());
    println!("   The hook runs `cargo bless bs --fail-on-confidence 0.8` before each commit.");
    println!();
    println!("   Tip: adjust the threshold or add --policy to suppress known findings.");

    Ok(())
}

fn run_explain(pattern: &str) -> Result<()> {
    use colored::Colorize;

    let rules = cargo_bless::suggestions::load_rules();
    let pattern_lower = pattern.to_lowercase();
    let matches: Vec<_> = rules
        .iter()
        .filter(|r| r.pattern.to_lowercase().contains(&pattern_lower))
        .collect();

    if matches.is_empty() {
        bail!(
            "No rule found for '{pattern}'. Run `cargo bless` on a project to see active suggestions."
        );
    }

    println!("🙏 cargo-bless — explain: {}", pattern.bold());

    for rule in matches {
        println!();
        println!("  {:<16} {}", "Pattern:".bold(), rule.pattern);
        println!("  {:<16} {}", "Replace with:".bold(), rule.replacement);
        println!("  {:<16} {:?}", "Kind:".bold(), rule.kind);
        println!("  {:<16} {:?}", "Confidence:".bold(), rule.confidence);
        println!("  {:<16} {:?}", "Migration risk:".bold(), rule.migration_risk);
        if let Some(ref cond) = rule.condition {
            println!("  {:<16} {}", "Condition:".bold(), cond);
        }
        println!("  {:<16} {}", "Source:".bold(), rule.source);
        println!();
        println!("  {}", "Why:".bold());
        for line in textwrap_rule(&rule.reason) {
            println!("    {line}");
        }
    }

    Ok(())
}

fn textwrap_rule(s: &str) -> Vec<String> {
    const WIDTH: usize = 72;
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in s.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + 1 + word.len() <= WIDTH {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current.clone());
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn apply_code_audit_fixes(
    report: &cargo_bless::code_audit::CodeAuditReport,
    dry_run: bool,
) -> Result<()> {
    use std::collections::BTreeSet;

    let files_to_fix: BTreeSet<&std::path::PathBuf> = report
        .alerts
        .iter()
        .filter(|a| a.kind == cargo_bless::code_audit::BullshitKind::UnwrapAbuse)
        .map(|a| &a.file)
        .collect();

    if files_to_fix.is_empty() {
        return Ok(());
    }

    let mut total_replacements = 0usize;

    for file in &files_to_fix {
        let contents = std::fs::read_to_string(file)
            .with_context(|| format!("failed to read {}", file.display()))?;

        let count = contents.matches(".unwrap()").count();
        if count == 0 {
            continue;
        }

        if dry_run {
            println!(
                "   {} {} .unwrap() → .expect(\"TODO: handle this\") in {}",
                "would replace".dimmed(),
                count,
                file.display()
            );
        } else {
            let backup = file.with_extension("rs.bak");
            std::fs::write(&backup, &contents)
                .with_context(|| format!("failed to write backup {}", backup.display()))?;

            let modified = contents.replace(".unwrap()", ".expect(\"TODO: handle this\")");
            std::fs::write(file, &modified)
                .with_context(|| format!("failed to write {}", file.display()))?;
        }

        total_replacements += count;
    }

    if dry_run {
        println!(
            "🔍 Dry-run: {} .unwrap() call(s) across {} file(s) would be replaced (no files written).",
            total_replacements,
            files_to_fix.len()
        );
        println!("   Remove --dry-run to apply the fixes.");
    } else {
        println!(
            "🙏 Fixed {} .unwrap() call(s) across {} file(s). Backups written as *.rs.bak.",
            total_replacements,
            files_to_fix.len()
        );
        println!("   Review each .expect() and replace the TODO with a real reason.");
    }

    Ok(())
}

fn apply_policy(
    suggestions: Vec<cargo_bless::suggestions::Suggestion>,
    policy: Option<&cargo_bless::policy::Policy>,
) -> Vec<cargo_bless::suggestions::Suggestion> {
    match policy {
        Some(policy) => cargo_bless::policy::apply_policy(suggestions, policy),
        None => suggestions,
    }
}

fn reject_invalid_flag_combinations(opts: &cli::BlessOpts) -> Result<()> {
    if opts.feedback {
        if opts.fix {
            bail!("--feedback cannot be combined with --fix");
        }
        if opts.dry_run {
            bail!("--feedback cannot be combined with --dry-run");
        }
        if opts.json {
            bail!("--feedback cannot be combined with --json");
        }
        if opts.update_rules {
            bail!("--feedback cannot be combined with --update-rules");
        }
        if opts.summary {
            bail!("--feedback cannot be combined with --summary");
        }
        if opts.audit_code {
            bail!("--feedback always includes the code audit; do not combine with --audit-code");
        }
        if opts.workspace || !opts.package.is_empty() {
            bail!("--feedback analyzes the workspace root crate only — omit `--workspace` and `--package`");
        }
    }
    if opts.summary {
        if opts.json {
            bail!("--summary cannot be combined with --json");
        }
        if opts.fix {
            bail!("--summary cannot be combined with --fix");
        }
        if opts.update_rules {
            bail!("--summary cannot be combined with --update-rules");
        }
        if opts.audit_code {
            bail!("--summary cannot be combined with --audit-code");
        }
        if opts.dry_run {
            bail!("--summary cannot be combined with --dry-run (use `--fix --dry-run` on a normal run)");
        }
    }
    if opts.dry_run && !opts.fix {
        bail!("--dry-run requires --fix");
    }
    if opts.json && opts.fix {
        bail!("--json cannot be combined with --fix");
    }
    if opts.json && opts.update_rules {
        bail!("--json cannot be combined with --update-rules");
    }

    Ok(())
}

fn reject_unfinished_flags(opts: &cli::BlessOpts) -> Result<()> {
    if opts.llm {
        bail!("--llm is not implemented in cargo-bless yet");
    }

    Ok(())
}
