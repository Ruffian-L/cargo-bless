//! Self-updating rules — fetches the latest blessed.rs crate recommendations
//! and converts them into cargo-bless suggestion rules.
//!
//! blessed.rs publishes structured data at:
//! https://raw.githubusercontent.com/nicoburns/blessed-rs/main/data/crates.json

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::suggestions::Rule;

const BLESSED_URL: &str =
    "https://raw.githubusercontent.com/nicoburns/blessed-rs/main/data/crates.json";
const CACHE_TTL_SECS: u64 = 7 * 24 * 3600; // 1 week

// ── blessed.rs JSON types ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct BlessedData {
    crate_groups: Vec<CrateGroup>,
}

#[derive(Debug, Deserialize)]
struct CrateGroup {
    #[allow(dead_code)]
    name: Option<String>,
    subgroups: Vec<Subgroup>,
}

#[derive(Debug, Deserialize)]
struct Subgroup {
    #[allow(dead_code)]
    name: Option<String>,
    purposes: Vec<Purpose>,
}

#[derive(Debug, Deserialize)]
struct Purpose {
    name: String,
    notes: Option<String>,
    recommendations: Vec<Recommendation>,
}

#[derive(Debug, Deserialize)]
struct Recommendation {
    name: String,
    notes: Option<String>,
}

/// Cached rules wrapper with timestamp.
#[derive(Debug, Serialize, Deserialize)]
struct CachedRules {
    rules: Vec<Rule>,
    fetched_at: u64,
}

impl CachedRules {
    fn is_fresh(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(self.fetched_at) < CACHE_TTL_SECS
    }
}

// ── Public API ───────────────────────────────────────────────────────

/// Force-fetch the latest blessed.rs data and update the cached rules.
pub fn update_rules() -> Result<Vec<Rule>> {
    println!("📡 Fetching latest blessed.rs recommendations...");

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(concat!("cargo-bless/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("failed to build HTTP client")?;

    let response = client
        .get(BLESSED_URL)
        .send()
        .context("failed to fetch blessed.rs data")?;

    let data: BlessedData = response.json().context("failed to parse blessed.rs JSON")?;

    let rules = convert_to_rules(&data);
    println!("✅ Generated {} rules from blessed.rs", rules.len());

    // Cache to disk when a user-specific cache directory is available.
    if let Some(cache_path) = get_cache_path() {
        if let Some(parent) = cache_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let cached = CachedRules {
            rules: rules.clone(),
            fetched_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        if let Ok(json) = serde_json::to_string_pretty(&cached) {
            let _ = fs::write(&cache_path, json);
            println!("💾 Cached to {}", cache_path.display());
        }
    }

    Ok(rules)
}

/// Load cached blessed.rs rules if they exist and are fresh.
pub fn load_cached_rules() -> Option<Vec<Rule>> {
    let cache_path = get_cache_path()?;
    let contents = fs::read_to_string(&cache_path).ok()?;
    let cached: CachedRules = serde_json::from_str(&contents).ok()?;

    if cached.is_fresh() {
        Some(cached.rules)
    } else {
        None
    }
}

/// Get the cache file path.
fn get_cache_path() -> Option<PathBuf> {
    ProjectDirs::from("rs", "", "cargo-bless")
        .map(|dirs| dirs.cache_dir().join("blessed-rules.json"))
}

// ── Converter ────────────────────────────────────────────────────────

/// Strip simple HTML tags from blessed.rs notes (links, line breaks).
fn strip_html(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

/// Convert blessed.rs data into cargo-bless rules.
///
/// For each purpose with 2+ recommendations, the first is "preferred"
/// and the rest become migration rule patterns pointing to it.
fn convert_to_rules(data: &BlessedData) -> Vec<Rule> {
    let mut rules = Vec::new();

    for group in &data.crate_groups {
        for sub in &group.subgroups {
            for purpose in &sub.purposes {
                if purpose.recommendations.len() < 2 {
                    continue;
                }

                let preferred = &purpose.recommendations[0];
                let purpose_clean = strip_html(purpose.notes.as_deref().unwrap_or(""));

                for alt in &purpose.recommendations[1..] {
                    let alt_clean = strip_html(alt.notes.as_deref().unwrap_or(""));

                    // Only generate a rule if there's a clear migration signal.
                    // Without this filter, co-equal alternatives (e.g. insta vs
                    // cargo-nextest, tokio vs crossbeam-channel) create false positives.
                    if !has_migration_signal(&purpose_clean, &alt_clean) {
                        continue;
                    }

                    let kind = infer_kind(&purpose_clean, &alt_clean);
                    let reason = build_reason(&purpose_clean, &alt_clean, &purpose.name);

                    rules.push(Rule {
                        pattern: alt.name.clone(),
                        replacement: preferred.name.clone(),
                        kind,
                        reason,
                        source: "blessed.rs".to_string(),
                        condition: None,
                        confidence: crate::suggestions::Confidence::Medium,
                        migration_risk: crate::suggestions::MigrationRisk::Medium,
                        autofix_safety: crate::suggestions::AutofixSafety::ManualOnly,
                        evidence_source: crate::suggestions::EvidenceSource::BlessedRs,
                    });
                }
            }
        }
    }

    rules
}

/// Check if the notes contain a clear signal that the alternative should
/// be migrated away from (rather than just being a co-equal option).
///
/// Tuned against [blessed.rs](https://blessed.rs/) / `crates.json` wording: bare
/// **`simpler`** is *not* enough (e.g. flume describes itself as "simpler than
/// crossbeam-channel" but should not flip direction; color-eyre says anyhow is
/// "**simpler**" for other use cases but is not deprecating color-eyre).
fn has_migration_signal(purpose_notes: &str, alt_notes: &str) -> bool {
    let combined = format!("{purpose_notes} {alt_notes}").to_lowercase();

    const CORE: &[&str] = &[
        "unmaintained",
        "deprecated",
        "superseded",
        "archived",
        "legacy",
        "maintenance mode",
        "inactive",
        "no longer maintained",
        "obsolete",
        "older crate",
        "an older",
        "older and",
        "less convenient",
        "included in standard library",
        "included in std",
        "adopted into",
        "not recommended",
    ];

    if CORE.iter().any(|s| combined.contains(s)) {
        return true;
    }

    // Blessed often positions one crate as today's default (e.g. tracing for logging).
    if combined.contains("go-to") || combined.contains("now the ") {
        return true;
    }

    // "simpler" only with enough context to avoid spurious direction flips.
    if combined.contains("simpler") {
        return combined.contains("older")
            || combined.contains("games")
            || combined.contains("2d ")
            || combined.contains("verbosity");
    }

    false
}

/// Infer the suggestion kind from notes.
fn infer_kind(purpose_notes: &str, alt_notes: &str) -> crate::suggestions::SuggestionKind {
    use crate::suggestions::SuggestionKind;

    let combined = format!("{} {}", purpose_notes, alt_notes).to_lowercase();

    if combined.contains("standard library") || combined.contains("included in std") {
        SuggestionKind::StdReplacement
    } else if combined.contains("unmaintained")
        || combined.contains("superseded")
        || combined.contains("deprecated")
        || combined.contains("archived")
    {
        SuggestionKind::Unmaintained
    } else {
        SuggestionKind::ModernAlternative
    }
}

/// Build a human-readable reason string.
fn build_reason(purpose_notes: &str, alt_notes: &str, purpose_name: &str) -> String {
    // Prefer alt-specific notes, fall back to purpose notes
    let note = if !alt_notes.is_empty() && alt_notes.len() > 10 {
        alt_notes
    } else if !purpose_notes.is_empty() {
        purpose_notes
    } else {
        ""
    };

    // Notes are already stripped of HTML in convert_to_rules.
    let clean = note.trim();

    if clean.is_empty() {
        format!(
            "blessed.rs recommends a different crate for: {}",
            purpose_name
        )
    } else if clean.len() > 120 {
        format!("{}...", &clean[..117])
    } else {
        clean.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html() {
        assert_eq!(
            strip_html("This is <a href=\"foo\">included in the standard library</a>"),
            "This is included in the standard library"
        );
        assert_eq!(strip_html("No tags here"), "No tags here");
        assert_eq!(strip_html(""), "");
    }

    #[test]
    fn test_infer_kind_std() {
        let kind = infer_kind("included in the standard library", "");
        assert_eq!(kind, crate::suggestions::SuggestionKind::StdReplacement);
    }

    #[test]
    fn test_infer_kind_unmaintained() {
        let kind = infer_kind("", "This crate is unmaintained");
        assert_eq!(kind, crate::suggestions::SuggestionKind::Unmaintained);
    }

    #[test]
    fn test_infer_kind_older() {
        let kind = infer_kind("", "Older crate. API is less convenient.");
        assert_eq!(kind, crate::suggestions::SuggestionKind::ModernAlternative);
    }

    #[test]
    fn test_infer_kind_default() {
        let kind = infer_kind("", "A simpler alternative");
        assert_eq!(kind, crate::suggestions::SuggestionKind::ModernAlternative);
    }

    #[test]
    fn test_migration_signal_rejects_flume_spurious_simpler() {
        let alt = "Smaller and simpler than crossbeam-channel and almost as fast";
        assert!(!has_migration_signal("", alt));
    }

    #[test]
    fn test_migration_signal_rejects_color_eyre_otherwise_simpler() {
        let alt = "A fork of anyhow that gives you more control over the format of the generated error messages. Recommended if you intend to present error messages to end users. Otherwise anyhow is simpler.";
        assert!(!has_migration_signal("", alt));
    }

    #[test]
    fn test_migration_signal_accepts_ggez_games_context() {
        assert!(has_migration_signal(
            "",
            "A simpler option for 2d games only."
        ));
    }

    #[test]
    fn test_convert_minimal() {
        let data = BlessedData {
            crate_groups: vec![CrateGroup {
                name: Some("Test".into()),
                subgroups: vec![Subgroup {
                    name: Some("Test".into()),
                    purposes: vec![Purpose {
                        name: "Logging".into(),
                        notes: None,
                        recommendations: vec![
                            Recommendation {
                                name: "tracing".into(),
                                notes: Some("Modern structured logging".into()),
                            },
                            Recommendation {
                                name: "log".into(),
                                notes: Some("Older and simpler".into()),
                            },
                        ],
                    }],
                }],
            }],
        };

        let rules = convert_to_rules(&data);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "log");
        assert_eq!(rules[0].replacement, "tracing");
    }

    #[test]
    fn test_convert_skips_single_rec() {
        let data = BlessedData {
            crate_groups: vec![CrateGroup {
                name: None,
                subgroups: vec![Subgroup {
                    name: None,
                    purposes: vec![Purpose {
                        name: "Temp files".into(),
                        notes: None,
                        recommendations: vec![Recommendation {
                            name: "tempfile".into(),
                            notes: None,
                        }],
                    }],
                }],
            }],
        };

        let rules = convert_to_rules(&data);
        assert!(rules.is_empty());
    }

    #[test]
    fn test_convert_multi_alternatives_no_signal() {
        // Co-equal alternatives without migration signals should NOT generate rules
        let data = BlessedData {
            crate_groups: vec![CrateGroup {
                name: None,
                subgroups: vec![Subgroup {
                    name: None,
                    purposes: vec![Purpose {
                        name: "Arrays".into(),
                        notes: None,
                        recommendations: vec![
                            Recommendation {
                                name: "arrayvec".into(),
                                notes: None,
                            },
                            Recommendation {
                                name: "smallvec".into(),
                                notes: None,
                            },
                            Recommendation {
                                name: "tinyvec".into(),
                                notes: None,
                            },
                        ],
                    }],
                }],
            }],
        };

        let rules = convert_to_rules(&data);
        assert!(
            rules.is_empty(),
            "co-equal options without migration signals should not generate rules"
        );
    }

    #[test]
    fn test_convert_with_migration_signal() {
        let data = BlessedData {
            crate_groups: vec![CrateGroup {
                name: None,
                subgroups: vec![Subgroup {
                    name: None,
                    purposes: vec![Purpose {
                        name: "Logging".into(),
                        notes: None,
                        recommendations: vec![
                            Recommendation {
                                name: "tracing".into(),
                                notes: Some("The modern choice".into()),
                            },
                            Recommendation {
                                name: "log".into(),
                                notes: Some("An older and simpler crate".into()),
                            },
                        ],
                    }],
                }],
            }],
        };

        let rules = convert_to_rules(&data);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "log");
        assert_eq!(rules[0].replacement, "tracing");
    }

    /// Live network test — run with `cargo test -- --ignored`
    #[test]
    #[ignore]
    fn test_live_update() {
        let rules = update_rules().expect("should fetch and convert");
        assert!(
            rules.len() >= 3,
            "expected a small set of high-confidence blessed migration rows, got {}",
            rules.len()
        );
        println!("Generated {} rules from live blessed.rs", rules.len());
        for rule in &rules {
            println!(
                "  {} → {} ({})",
                rule.pattern, rule.replacement, rule.reason
            );
        }
    }
}
