//! Live intelligence layer — fetches metadata from crates.io and GitHub
//! to assess freshness, popularity, and maintenance status.

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Live metadata for a single crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateIntel {
    pub name: String,
    pub latest_version: String,
    pub downloads: u64,
    pub recent_downloads: Option<u64>,
    pub last_updated: String,
    pub repository_url: Option<String>,
    pub last_commit: Option<String>,
    pub is_unmaintained: bool,
}

/// Fetch live metadata from crates.io for the given crate name.
pub async fn fetch_crate_intel(_name: &str) -> Result<CrateIntel> {
    // TODO: Use crates_io_api::AsyncClient with User-Agent "cargo-bless/0.1"
    // TODO: Implement disk cache in ~/.cache/cargo-bless/ (1-hour TTL)
    // TODO: Sparse index fallback for rate-limit safety

    todo!("crates.io API integration not yet implemented")
}

/// Fetch GitHub activity (last push, stars, archived status) for a repo URL.
pub async fn fetch_github_activity(_repo_url: &str) -> Result<Option<GitHubActivity>> {
    // TODO: Use octocrab anonymous client with exponential backoff
    // TODO: Parse owner/repo from URL

    todo!("GitHub API integration not yet implemented")
}

/// GitHub repository activity summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubActivity {
    pub last_push: String,
    pub stars: u64,
    pub is_archived: bool,
    pub open_issues: u64,
}
