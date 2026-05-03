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
    /// Apply suggested changes to Cargo.toml only (`*.toml.bak` backup before write; never touches `.rs`).
    #[arg(long)]
    pub fix: bool,

    /// With `--fix`, print the unified diff and planned actions without writing or running `cargo update`.
    #[arg(long)]
    pub dry_run: bool,

    /// Print a privacy-safe summary block suitable for pasting into issue reports (no paths to source snippets).
    #[arg(long)]
    pub feedback: bool,

    /// Short paste-friendly dependency summary: counts, impacts, and deduped suggestion patterns (not JSON; omits live intel).
    #[arg(long)]
    pub summary: bool,
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

    /// Exit non-zero when any dependency suggestion matches one of these impacts (comma-separated: low, medium, high, critical).
    /// `critical` is treated as **high** until a separate code-audit gate exists.
    #[arg(long, value_delimiter = ',')]
    pub fail_on: Vec<String>,

    /// Analyze every workspace member’s `Cargo.toml` (one `cargo metadata` call).
    #[arg(long)]
    pub workspace: bool,

    /// Only analyze these workspace member package names (comma-separated). Implies member selection; combine with `--workspace` to avoid ambiguity on some layouts.
    #[arg(long, value_delimiter = ',')]
    pub package: Vec<String>,
    /// Include dev-dependencies and build-dependencies in analysis.
    #[arg(long)]
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

    /// Write a starter GitHub Actions workflow to .github/workflows/bless.yml and exit.
    #[arg(long)]
    pub init_ci: bool,

    /// Write a pre-commit git hook that runs the code audit before each commit.
    #[arg(long)]
    pub init_hooks: bool,

    /// Show full details for a suggestion pattern (e.g. --explain lazy_static).
    #[arg(long, value_name = "PATTERN")]
    pub explain: Option<String>,
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

    /// Also scan for hardcoded values: magic numbers, API keys, IPs, URLs, credentials.
    #[arg(long)]
    pub hardcoded: bool,

    /// Output findings as SARIF 2.1.0 JSON (for GitHub code-scanning / PR annotations).
    #[arg(long)]
    pub sarif: bool,

    /// Exit non-zero if any finding has confidence >= this value (0.0–1.0).
    #[arg(long, value_name = "FLOAT")]
    pub fail_on_confidence: Option<f64>,

    /// Auto-fix safe findings: replaces .unwrap() with .expect("TODO") (writes *.rs.bak backups).
    #[arg(long)]
    pub fix: bool,

    /// With `--fix`, show what would be changed without writing files or backups.
    #[arg(long)]
    pub dry_run: bool,
}
