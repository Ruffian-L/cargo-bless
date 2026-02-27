use anyhow::Result;
use clap::Parser;

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
            } else {
                println!("📋 Generating modernization report...");
            }

            // TODO: Wire up the pipeline:
            //   1. parser::get_deps()
            //   2. intel::fetch_metadata()
            //   3. suggestions::analyze()
            //   4. output::render_report() or fix::apply()

            println!();
            println!("⚠️  Not yet implemented — scaffold only.");
            Ok(())
        }
    }
}
