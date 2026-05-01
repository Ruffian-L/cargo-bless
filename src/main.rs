use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Parser;
use colored::*;

mod cli;

fn main() -> Result<()> {
    let args = cli::Cli::parse();

    match args.command {
        cli::Commands::Bless(opts) => {
            let manifest = opts.manifest_path.as_deref();
            let run_code_audit = !opts.no_audit_code || opts.audit_code;
            let policy = load_policy(opts.policy.as_deref(), manifest);
            let code_audit_config = cargo_bless::code_audit::config_from_policy(policy.as_ref());

            if opts.json {
                let deps = cargo_bless::parser::get_deps(manifest)?;
                let rules = cargo_bless::suggestions::load_rules();
                let suggestions = cargo_bless::suggestions::analyze(manifest, &deps, &rules);
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
            if opts.llm {
                println!(
                    "{}",
                    "LLM-powered suggestions are not yet implemented. Stay tuned!".yellow()
                );
                println!();
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
            let suggestions = cargo_bless::suggestions::analyze(manifest, &deps, &rules);

            // Live intelligence: fetch metadata for flagged deps (non-fatal)
            let intel = if !opts.offline && !suggestions.is_empty() {
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
            cargo_bless::output::render_report(
                &project_name,
                &project_version,
                &suggestions,
                &intel,
            );

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
        cli::Commands::Bs(opts) => {
            let manifest = opts.manifest_path.as_deref();
            let policy = load_policy(opts.policy.as_deref(), manifest);
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
    }
}

fn load_policy(
    explicit_path: Option<&Path>,
    manifest_path: Option<&Path>,
) -> Option<cargo_bless::policy::Policy> {
    let path = explicit_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_policy_path(manifest_path));
    cargo_bless::policy::load_policy(&path)
}

fn default_policy_path(manifest_path: Option<&Path>) -> PathBuf {
    manifest_path
        .and_then(Path::parent)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join("bless.toml")
}
