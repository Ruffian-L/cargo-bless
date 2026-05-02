use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
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

fn load_snapshots(opts: &cli::BlessOpts) -> Result<Vec<cargo_bless::parser::PackageResult>> {
    let manifest = opts.manifest_path.as_deref();
    cargo_bless::parser::get_package_snapshots(manifest, opts.workspace, &opts.package)
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
    reject_invalid_flag_combinations(&opts)?;
    reject_unfinished_flags(&opts)?;
    if opts.feedback {
        return run_feedback_command(opts);
    }

    let fail_levels = parse_fail_on_levels(&opts.fail_on)?;

    if opts.summary {
        return run_summary_mode(&opts, &fail_levels);
    }

    let manifest = opts.manifest_path.as_deref();
    let run_code_audit = opts.audit_code;
    let policy = load_policy(opts.policy.as_deref(), manifest)?;
    let code_audit_config = cargo_bless::code_audit::config_from_policy(policy.as_ref());
    let tagged = use_tagged_suggestions(&opts);
    let snapshots = load_snapshots(&opts)?;
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
        let report = cargo_bless::output::JsonReportUnified {
            cargo_bless_version: env!("CARGO_PKG_VERSION"),
            workspace_scan: opts.workspace || snapshots.len() > 1,
            packages: packages_for_json,
            code_audit: merged_audit.as_ref(),
        };
        cargo_bless::output::render_unified_json(report);
        maybe_fail_on_exit(&all_suggestions, &fail_levels)?;
        return Ok(());
    }

    println!("🔥 cargo-bless v{}", env!("CARGO_PKG_VERSION"));
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
) -> Result<()> {
    let manifest = opts.manifest_path.as_deref();
    let policy = load_policy(opts.policy.as_deref(), manifest)?;
    let tagged = use_tagged_suggestions(opts);
    let snapshots = load_snapshots(opts)?;
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
        all.extend(apply_policy(raw, policy.as_ref()));
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

    println!("🔥 cargo-bless v{}", env!("CARGO_PKG_VERSION"));
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

    if opts.json {
        let unified = cargo_bless::output::JsonReportUnified {
            cargo_bless_version: env!("CARGO_PKG_VERSION"),
            workspace_scan: false,
            packages: Vec::new(),
            code_audit: Some(&report),
        };
        cargo_bless::output::render_unified_json(unified);
    } else {
        println!("🔥 cargo-bless v{}", env!("CARGO_PKG_VERSION"));
        cargo_bless::output::render_code_audit_report(&report, opts.verbose);
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
    if opts.all_targets {
        bail!("--all-targets is not implemented in cargo-bless yet");
    }

    Ok(())
}
