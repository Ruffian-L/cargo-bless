//! Security advisory integration via the osv.dev batch API.
//!
//! Checks all direct dependencies against the OSV database (which includes
//! RustSec advisories) in a single HTTP call. Non-fatal: all errors return
//! an empty result so the main analysis always completes.

use std::time::Duration;

use serde::{Deserialize, Serialize};

const OSV_BATCH_URL: &str = "https://api.osv.dev/v1/querybatch";
const USER_AGENT: &str = concat!(
    "cargo-bless/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/Ruffian-L/cargo-bless)"
);

/// A single security advisory for a crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Advisory {
    pub id: String,
    pub summary: String,
    pub aliases: Vec<String>,
}

impl Advisory {
    /// Return the first CVE alias if one is present.
    pub fn cve(&self) -> Option<&str> {
        self.aliases
            .iter()
            .find(|a| a.starts_with("CVE-"))
            .map(String::as_str)
    }
}

/// All advisories found for a specific crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateAdvisories {
    pub crate_name: String,
    pub advisories: Vec<Advisory>,
}

// ── osv.dev wire types ───────────────────────────────────────────────────────

#[derive(Serialize)]
struct OsvBatchRequest {
    queries: Vec<OsvPackageQuery>,
}

#[derive(Serialize)]
struct OsvPackageQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    package: OsvPackage,
}

#[derive(Serialize)]
struct OsvPackage {
    name: String,
    ecosystem: &'static str,
}

#[derive(Deserialize, Default)]
struct OsvBatchResponse {
    #[serde(default)]
    results: Vec<OsvResultItem>,
}

#[derive(Deserialize, Default)]
struct OsvResultItem {
    #[serde(default)]
    vulns: Vec<OsvVuln>,
}

#[derive(Deserialize)]
struct OsvVuln {
    id: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
}

/// Fetch all historical advisories for `crate_names` in one batch request.
///
/// This name-only API is retained for compatibility. Call
/// [`fetch_advisories_batch_for_versions`] when checking resolved dependencies;
/// without versions, OSV returns advisories that may not affect the installed
/// release.
/// Returns only crates that have at least one advisory.
/// All network or parse errors are swallowed — returns `[]` on failure.
pub fn fetch_advisories_batch(crate_names: &[&str]) -> Vec<CrateAdvisories> {
    let queries = crate_names
        .iter()
        .map(|name| OsvPackageQuery {
            version: None,
            package: OsvPackage {
                name: name.to_string(),
                ecosystem: "crates.io",
            },
        })
        .collect();

    fetch_advisories(queries)
}

/// Fetch advisories that affect the exact resolved crate versions.
///
/// Each tuple is `(crate_name, resolved_version)`. OSV uses the version to
/// filter out historical advisories that do not affect the installed release.
pub fn fetch_advisories_batch_for_versions(
    crate_versions: &[(&str, &str)],
) -> Vec<CrateAdvisories> {
    fetch_advisories(versioned_queries(crate_versions))
}

fn versioned_queries(crate_versions: &[(&str, &str)]) -> Vec<OsvPackageQuery> {
    crate_versions
        .iter()
        .map(|(name, version)| OsvPackageQuery {
            version: Some(version.to_string()),
            package: OsvPackage {
                name: name.to_string(),
                ecosystem: "crates.io",
            },
        })
        .collect()
}

fn fetch_advisories(queries: Vec<OsvPackageQuery>) -> Vec<CrateAdvisories> {
    if queries.is_empty() {
        return Vec::new();
    }

    let client = match reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let request = OsvBatchRequest { queries };

    let batch: OsvBatchResponse = match client
        .post(OSV_BATCH_URL)
        .json(&request)
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.json())
    {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };

    request
        .queries
        .iter()
        .zip(batch.results.iter())
        .filter(|(_, result)| !result.vulns.is_empty())
        .map(|(query, result)| CrateAdvisories {
            crate_name: query.package.name.clone(),
            advisories: result
                .vulns
                .iter()
                .map(|v| Advisory {
                    id: v.id.clone(),
                    summary: v
                        .summary
                        .clone()
                        .unwrap_or_else(|| "No details available.".into()),
                    aliases: v.aliases.clone(),
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versioned_queries_serialize_the_exact_version() {
        let request = OsvBatchRequest {
            queries: versioned_queries(&[("anyhow", "1.0.103")]),
        };

        let json = serde_json::to_value(request).expect("serialize OSV request");
        assert_eq!(json["queries"][0]["version"], "1.0.103");
        assert_eq!(json["queries"][0]["package"]["name"], "anyhow");
        assert_eq!(json["queries"][0]["package"]["ecosystem"], "crates.io");
    }

    #[test]
    fn compatibility_queries_omit_the_version() {
        let request = OsvBatchRequest {
            queries: vec![OsvPackageQuery {
                version: None,
                package: OsvPackage {
                    name: "anyhow".to_string(),
                    ecosystem: "crates.io",
                },
            }],
        };

        let json = serde_json::to_value(request).expect("serialize OSV request");
        assert!(json["queries"][0].get("version").is_none());
    }
}
