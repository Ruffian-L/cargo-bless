use clap::{Parser, Subcommand};

/// Cargo subcommand wrapper — invoked as `cargo bless`.
#[derive(Parser, Debug)]
#[command(name = "cargo", bin_name = "cargo")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Bless your dependencies — modernize, optimize, and stay current.
    Bless(BlessOpts),
}

#[derive(clap::Args, Debug)]
pub struct BlessOpts {
    /// Apply suggested changes to Cargo.toml (creates .bak backup first).
    #[arg(long)]
    pub fix: bool,

    /// Preview changes without writing anything (requires --fix).
    #[arg(long)]
    pub dry_run: bool,

    /// Fetch the latest rules from blessed.rs and update the local cache.
    #[arg(long)]
    pub update_rules: bool,

    /// Enable LLM-powered suggestions via local Ollama or API.
    #[arg(long)]
    pub llm: bool,

    /// Path to the Cargo.toml to analyze (defaults to current directory).
    #[arg(long, value_name = "PATH")]
    pub manifest_path: Option<std::path::PathBuf>,
}
