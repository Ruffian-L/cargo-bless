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

            if opts.fix {
                if opts.dry_run {
                    println!("🔍 Dry-run mode — previewing changes (no files will be modified)");
                } else {
                    println!("🔧 Fix mode — applying safe changes");
                }
                println!();
                println!("⚠️  Fix mode not yet implemented — coming in Phase 4.");
                return Ok(());
            }

            println!("📋 Scanning dependencies...");
            println!();

            // Parse the dep tree
            let manifest = opts.manifest_path.as_deref();
            let deps = cargo_bless::parser::get_deps(manifest)?;
            let (project_name, project_version) =
                cargo_bless::parser::get_project_info(manifest)?;

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
                format!(
                    "Found {} direct deps, {} total.",
                    direct.len(),
                    deps.len()
                )
                .bold()
            );

            // Suggestion engine: load rules → analyze → report
            println!();
            let rules = cargo_bless::suggestions::load_rules();
            let suggestions = cargo_bless::suggestions::analyze(&deps, &rules);
            cargo_bless::output::render_report(&project_name, &project_version, &suggestions);

            // TODO Phase 3: intel::fetch_metadata() enrichment

            Ok(())
        }
    }
}
