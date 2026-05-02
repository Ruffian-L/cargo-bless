use std::collections::HashMap;
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

fn run_bless_command(opts: cli::BlessOpts) -> Result<()> {
    reject_invalid_flag_combinations(&opts)?;
    reject_unfinished_flags(&opts)?;
    if opts.feedback {
        return run_feedback_command(opts);
    }
    let manifest = opts.manifest_path.as_deref();
    let run_code_audit = opts.audit_code;
    let policy = load_policy(opts.policy.as_deref(), manifest)?;
    let code_audit_config = cargo_bless::code_audit::config_from_policy(policy.as_ref());

    if opts.json {
        let deps = cargo_bless::parser::get_deps(manifest)?;
        let rules = cargo_bless::suggestions::load_rules();
        let suggestions = apply_policy(
            cargo_bless::suggestions::analyze(manifest, &deps, &rules),
            policy.as_ref(),
        );
        let code_audit = if run_code_audit {
            Some(cargo_bless::code_audit::scan_project(
                manifest,
                &code_audit_config,
            )?)
        } else {
            None
        };
        cargo_bless::output::render_json_report(&suggestions, code_audit.as_ref());
        return Ok(());
    }

    println!("🔥 cargo-bless v{}", env!("CARGO_PKG_VERSION"));
    println!();

    // Handle --update-rules before the main pipeline
    if opts.update_rules {
        cargo_bless::updater::update_rules()?;
        println!();
        println!("Rules updated. Run `cargo bless` to use them.");
        return Ok(());
    }
    if opts.fix {
        if opts.dry_run {
            println!("🔍 Dry-run mode — previewing changes (no files will be modified)");
        } else {
            println!("🔧 Fix mode — applying safe changes");
        }
        println!();
    }

    println!("📋 Scanning dependencies...");
    println!();

    // Parse the dep tree
    let deps = cargo_bless::parser::get_deps(manifest)?;
    let (project_name, project_version) = cargo_bless::parser::get_project_info(manifest)?;

    let direct: Vec<_> = deps.iter().filter(|d| d.is_direct).collect();
    let transitive: Vec<_> = deps.iter().filter(|d| !d.is_direct).collect();

    // Print direct dependencies
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

    // Suggestion engine: load rules → analyze
    println!();
    let rules = cargo_bless::suggestions::load_rules();
    let suggestions = apply_policy(
        cargo_bless::suggestions::analyze(manifest, &deps, &rules),
        policy.as_ref(),
    );

    // Live intelligence: fetch metadata for flagged deps (non-fatal)
    let effective_offline = opts.offline
        || policy
            .as_ref()
            .is_some_and(|policy| policy.settings.offline);
    let intel = if !effective_offline && !suggestions.is_empty() {
        // Collect unique crate names from suggestions
        let crate_names: Vec<&str> = suggestions
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

    // Render the report
    cargo_bless::output::render_report(&project_name, &project_version, &suggestions, &intel);

    if run_code_audit {
        let report = cargo_bless::code_audit::scan_project(manifest, &code_audit_config)?;
        cargo_bless::output::render_code_audit_report(&report, opts.verbose);
    }

    // Apply fixes if --fix was passed
    if opts.fix && !suggestions.is_empty() {
        println!();
        let manifest = opts
            .manifest_path
            .unwrap_or_else(|| std::path::PathBuf::from("Cargo.toml"));
        cargo_bless::fix::apply(&suggestions, &manifest, opts.dry_run)?;
    }

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
        cargo_bless::output::render_json_report(&[], Some(&report));
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
        if opts.audit_code {
            bail!("--feedback always includes the code audit; do not combine with --audit-code");
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
    if !opts.fail_on.is_empty() {
        bail!("--fail-on is not implemented in cargo-bless yet");
    }
    if opts.workspace {
        bail!("--workspace is not implemented in cargo-bless yet");
    }
    if !opts.package.is_empty() {
        bail!("--package is not implemented in cargo-bless yet");
    }
    if opts.all_targets {
        bail!("--all-targets is not implemented in cargo-bless yet");
    }

    Ok(())
}
