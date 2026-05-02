use clap::{Args, Parser, Subcommand};

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
    Bless(BlessCommand),
    /// Run only the bullshit detector code audit.
    Bs(CodeAuditOpts),
}

#[derive(Args, Debug)]
pub struct BlessCommand {
    #[command(flatten)]
    pub opts: BlessOpts,

    #[command(subcommand)]
    pub command: Option<BlessSubcommand>,
}

#[derive(Subcommand, Debug)]
pub enum BlessSubcommand {
    /// Run only the bullshit detector code audit.
    Bs(CodeAuditOpts),
}

#[derive(Args, Debug)]
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

    /// Reserved for future machine-assisted suggestions.
    #[arg(long, hide = true)]
    pub llm: bool,

    /// Path to the Cargo.toml to analyze (defaults to current directory).
    #[arg(long, value_name = "PATH")]
    pub manifest_path: Option<std::path::PathBuf>,

    /// Run the bullshit detector code audit.
    #[arg(long)]
    pub audit_code: bool,

    /// Skip the default bullshit detector code audit.
    #[arg(long, hide = true)]
    pub no_audit_code: bool,

    /// Output suggestions in JSON format.
    #[arg(long)]
    pub json: bool,

    /// Exit with non-zero code when a suggestion matches the given severity level(s).
    /// Reserved severity gate. Comma-separated: low, medium, high, critical.
    #[arg(long, value_delimiter = ',', hide = true)]
    pub fail_on: Vec<String>,

    /// Analyze all workspace members instead of only the root package.
    #[arg(long, hide = true)]
    pub workspace: bool,

    /// Only analyze the specified package(s) in a workspace. Accepts package names.
    #[arg(long, value_delimiter = ',', hide = true)]
    pub package: Vec<String>,

    /// Include dev-dependencies and build-dependencies in analysis.
    #[arg(long, hide = true)]
    pub all_targets: bool,

    /// Do not fetch online data; use only the local rule cache.
    #[arg(long)]
    pub offline: bool,

    /// Path to a bless.toml policy file for custom rules and overrides.
    #[arg(long, value_name = "PATH")]
    pub policy: Option<std::path::PathBuf>,

    /// Show every bullshit detector finding instead of a concise summary.
    #[arg(long)]
    pub verbose: bool,
}

#[derive(Args, Debug)]
pub struct CodeAuditOpts {
    /// Path to the Cargo.toml whose source tree should be audited.
    #[arg(long, value_name = "PATH")]
    pub manifest_path: Option<std::path::PathBuf>,

    /// Output the bullshit audit report as JSON.
    #[arg(long)]
    pub json: bool,

    /// Audit only lines changed in `git diff HEAD`.
    #[arg(long)]
    pub diff: bool,

    /// Path to a bless.toml policy file for code-audit suppressions.
    #[arg(long, value_name = "PATH")]
    pub policy: Option<std::path::PathBuf>,

    /// Show every finding instead of a concise summary.
    #[arg(long)]
    pub verbose: bool,
}
