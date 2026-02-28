use std::collections::HashMap;

use anyhow::Result;
use clap::Parser;
use colored::*;

mod cli;

fn main() -> Result<()> {
    let args = cli::Cli::parse();

    match args.command {
        cli::Commands::Bless(opts) => {
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

            // Parse the dep tree (single metadata call)
            let manifest = opts.manifest_path.as_deref();
            let metadata = cargo_bless::parser::get_metadata(manifest)?;
            let deps = cargo_bless::parser::get_deps(&metadata)?;
            let (project_name, project_version) = cargo_bless::parser::get_project_info(&metadata)?;

            let direct: Vec<_> = deps.iter().filter(|d| d.is_direct).collect();
            let transitive: Vec<_> = deps.iter().filter(|d| !d.is_direct).collect();

            // Print direct dependencies
            println!(
                "{}",
                format!("📦 Direct dependencies ({})", direct.len()).bold()
            );
            for dep in &direct {
                let features_str = if dep.features.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", dep.features.join(", "))
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
            let suggestions = cargo_bless::suggestions::analyze(&deps, &rules);

            // Live intelligence: fetch metadata for flagged deps (non-fatal)
            let intel = if !suggestions.is_empty() {
                // Collect unique crate names from suggestions
                let crate_names: Vec<&str> = suggestions
                    .iter()
                    .flat_map(|s| s.current.split('+'))
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
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

            // Apply fixes if --fix was passed
            if opts.fix && !suggestions.is_empty() {
                println!();
                let manifest = opts
                    .manifest_path
                    .clone()
                    .unwrap_or_else(|| std::path::PathBuf::from("Cargo.toml"));
                cargo_bless::fix::apply(&suggestions, &manifest, opts.dry_run)?;
            }

            Ok(())
        }
    }
}
