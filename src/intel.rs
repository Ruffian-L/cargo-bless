//! Live intelligence layer — fetches metadata from crates.io and GitHub
//! to assess freshness, popularity, and maintenance status.
//!
//! All network operations are **non-fatal**: failures are logged and the
//! tool continues with whatever data it has.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

const USER_AGENT: &str = "cargo-bless/0.1.0 (https://github.com/Ruffian-L/cargo-bless)";
const CACHE_TTL_SECS: u64 = 3600; // 1 hour

// ── Public types ─────────────────────────────────────────────────────

/// Live metadata for a single crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateIntel {
    pub name: String,
    pub latest_version: String,
    pub downloads: u64,
    pub recent_downloads: Option<u64>,
    pub last_updated: String,
    pub repository_url: Option<String>,
    pub description: Option<String>,
}

/// GitHub repository activity summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubActivity {
    pub last_push: String,
    pub stars: u64,
    pub is_archived: bool,
    pub open_issues: u64,
}

/// Cache wrapper that tracks when data was fetched.
#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry<T> {
    data: T,
    fetched_at: u64,
}

impl<T> CacheEntry<T> {
    fn is_fresh(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(self.fetched_at) < CACHE_TTL_SECS
    }

    fn new(data: T) -> Self {
        let fetched_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self { data, fetched_at }
    }
}

// ── IntelClient ──────────────────────────────────────────────────────

/// Client for fetching live dependency intelligence.
pub struct IntelClient {
    client: crates_io_api::SyncClient,
    cache_dir: PathBuf,
}

impl IntelClient {
    /// Create a new IntelClient with crates.io API access and disk cache.
    pub fn new() -> Result<Self> {
        // Use 1 req/sec rate limit per crates.io policy, but wait within threads.
        let client = crates_io_api::SyncClient::new(USER_AGENT, Duration::from_secs(1))
            .context("failed to create crates.io client")?;

        let cache_dir = ProjectDirs::from("rs", "", "cargo-bless")
            .map(|dirs| dirs.cache_dir().to_path_buf())
            .unwrap_or_else(|| {
                let mut fallback = std::env::temp_dir();
                fallback.push("cargo-bless-cache");
                fallback
            });

        fs::create_dir_all(&cache_dir).context("failed to create cache directory")?;

        Ok(Self { client, cache_dir })
    }

    /// Fetch live intel for a crate. Checks disk cache first (1hr TTL).
    pub fn fetch_crate_intel(&self, name: &str) -> Result<CrateIntel> {
        // Check cache
        let cache_path = self.cache_dir.join(format!("{}.json", name));
        if let Ok(contents) = fs::read_to_string(&cache_path) {
            if let Ok(entry) = serde_json::from_str::<CacheEntry<CrateIntel>>(&contents) {
                if entry.is_fresh() {
                    return Ok(entry.data);
                }
            }
        }

        // Cache miss or stale — fetch from crates.io
        let response = self
            .client
            .get_crate(name)
            .with_context(|| format!("failed to fetch crate info for '{}'", name))?;

        let crate_data = &response.crate_data;
        let latest_version = response
            .versions
            .first()
            .map(|v| v.num.clone())
            .unwrap_or_else(|| crate_data.max_version.clone());

        let intel = CrateIntel {
            name: name.to_string(),
            latest_version,
            downloads: crate_data.downloads,
            recent_downloads: crate_data.recent_downloads,
            last_updated: crate_data.updated_at.to_rfc3339(),
            repository_url: crate_data.repository.clone(),
            description: crate_data.description.clone(),
        };

        // Write to cache (best-effort)
        let entry = CacheEntry::new(intel.clone());
        if let Ok(json) = serde_json::to_string_pretty(&entry) {
            let _ = fs::write(&cache_path, json);
        }

        Ok(intel)
    }

    /// Fetch GitHub activity for a repository URL.
    /// Returns None if the URL is not a GitHub URL or if the fetch fails.
    pub fn fetch_github_activity(&self, repo_url: &str) -> Option<GitHubActivity> {
        let (owner, repo) = parse_github_url(repo_url)?;

        // Use a small tokio runtime for the async octocrab call
        let rt = tokio::runtime::Runtime::new().ok()?;
        rt.block_on(async {
            let octo = octocrab::Octocrab::builder().build().ok()?;

            let repo_info = octo.repos(&owner, &repo).get().await.ok()?;

            Some(GitHubActivity {
                last_push: repo_info
                    .pushed_at
                    .map(|d| d.to_rfc3339())
                    .unwrap_or_else(|| "unknown".into()),
                stars: repo_info.stargazers_count.unwrap_or(0) as u64,
                is_archived: repo_info.archived.unwrap_or(false),
                open_issues: repo_info.open_issues_count.unwrap_or(0) as u64,
            })
        })
    }

    /// Fetch intel for all unique crate names, returning what we can get.
    /// Failures for individual crates are silently skipped.
    pub fn fetch_bulk_intel(&self, crate_names: &[&str]) -> HashMap<String, CrateIntel> {
        use rayon::prelude::*;

        crate_names
            .par_iter()
            .filter_map(|&name| {
                self.fetch_crate_intel(name)
                    .ok()
                    .map(|info| (name.to_string(), info))
            })
            .collect()
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Parse a GitHub URL into (owner, repo).
/// Supports: https://github.com/owner/repo, https://github.com/owner/repo.git,
/// https://github.com/owner/repo/tree/main, etc.
pub fn parse_github_url(url: &str) -> Option<(String, String)> {
    let url = url.trim().trim_end_matches('/');

    // Find the github.com part
    let after_github = if let Some(pos) = url.find("github.com/") {
        &url[pos + "github.com/".len()..]
    } else {
        return None;
    };

    let parts: Vec<&str> = after_github.splitn(3, '/').collect();
    if parts.len() < 2 {
        return None;
    }

    let owner = parts[0].to_string();
    let repo = parts[1].trim_end_matches(".git").to_string();

    if owner.is_empty() || repo.is_empty() {
        return None;
    }

    Some((owner, repo))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_github_url_basic() {
        let result = parse_github_url("https://github.com/serde-rs/serde");
        assert_eq!(result, Some(("serde-rs".into(), "serde".into())));
    }

    #[test]
    fn test_parse_github_url_with_git_suffix() {
        let result = parse_github_url("https://github.com/tokio-rs/tokio.git");
        assert_eq!(result, Some(("tokio-rs".into(), "tokio".into())));
    }

    #[test]
    fn test_parse_github_url_with_path() {
        let result = parse_github_url("https://github.com/dtolnay/anyhow/tree/main");
        assert_eq!(result, Some(("dtolnay".into(), "anyhow".into())));
    }

    #[test]
    fn test_parse_github_url_trailing_slash() {
        let result = parse_github_url("https://github.com/clap-rs/clap/");
        assert_eq!(result, Some(("clap-rs".into(), "clap".into())));
    }

    #[test]
    fn test_parse_github_url_not_github() {
        assert!(parse_github_url("https://gitlab.com/foo/bar").is_none());
        assert!(parse_github_url("https://crates.io/crates/serde").is_none());
    }

    #[test]
    fn test_parse_github_url_too_short() {
        assert!(parse_github_url("https://github.com/just-user").is_none());
        assert!(parse_github_url("https://github.com/").is_none());
    }

    #[test]
    fn test_cache_entry_fresh() {
        let entry = CacheEntry::new("some data".to_string());
        assert!(entry.is_fresh());
    }

    #[test]
    fn test_cache_entry_stale() {
        let entry = CacheEntry {
            data: "old data".to_string(),
            fetched_at: 0, // epoch = definitely stale
        };
        assert!(!entry.is_fresh());
    }

    #[test]
    fn test_cache_entry_roundtrip() {
        let intel = CrateIntel {
            name: "serde".into(),
            latest_version: "1.0.228".into(),
            downloads: 100_000_000,
            recent_downloads: Some(5_000_000),
            last_updated: "2026-01-15T12:00:00Z".into(),
            repository_url: Some("https://github.com/serde-rs/serde".into()),
            description: Some("A serialization framework".into()),
        };
        let entry = CacheEntry::new(intel);
        let json = serde_json::to_string(&entry).unwrap();
        let roundtrip: CacheEntry<CrateIntel> = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.data.name, "serde");
        assert_eq!(roundtrip.data.downloads, 100_000_000);
        assert!(roundtrip.is_fresh());
    }

    #[test]
    fn test_cache_disk_write_and_read() {
        let tmp = TempDir::new().unwrap();
        let cache_path = tmp.path().join("test_crate.json");

        let intel = CrateIntel {
            name: "test_crate".into(),
            latest_version: "0.1.0".into(),
            downloads: 42,
            recent_downloads: None,
            last_updated: "2026-02-27T00:00:00Z".into(),
            repository_url: None,
            description: None,
        };

        // Write
        let entry = CacheEntry::new(intel);
        let json = serde_json::to_string_pretty(&entry).unwrap();
        fs::write(&cache_path, &json).unwrap();

        // Read back
        let contents = fs::read_to_string(&cache_path).unwrap();
        let loaded: CacheEntry<CrateIntel> = serde_json::from_str(&contents).unwrap();
        assert_eq!(loaded.data.name, "test_crate");
        assert!(loaded.is_fresh());
    }

    /// Live network test — run with `cargo test -- --ignored`
    #[test]
    #[ignore]
    fn test_live_fetch_serde() {
        let client = IntelClient::new().expect("client should init");
        let intel = client
            .fetch_crate_intel("serde")
            .expect("should fetch serde");
        assert_eq!(intel.name, "serde");
        assert!(intel.downloads > 0);
        println!(
            "serde: v{}, {} downloads",
            intel.latest_version, intel.downloads
        );
    }

    /// Live GitHub test — run with `cargo test -- --ignored`
    #[test]
    #[ignore]
    fn test_live_github_serde() {
        let client = IntelClient::new().expect("client should init");
        let activity = client
            .fetch_github_activity("https://github.com/serde-rs/serde")
            .expect("should get activity");
        assert!(activity.stars > 0);
        println!(
            "serde: {} stars, archived={}",
            activity.stars, activity.is_archived
        );
    }
}
