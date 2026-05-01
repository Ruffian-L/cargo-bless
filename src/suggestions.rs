//! Suggestion engine — rule-based recommendations from blessed.rs mappings.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::parser::ResolvedDep;

/// A modernization suggestion for a specific dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub kind: SuggestionKind,
    pub current: String,
    pub recommended: String,
    pub reason: String,
    pub source: String,
    pub impact: Impact,
    pub confidence: Confidence,
    pub migration_risk: MigrationRisk,
    pub autofix_safety: AutofixSafety,
    pub evidence_source: EvidenceSource,
}

impl Suggestion {
    /// Whether this suggestion can be auto-applied by editing Cargo.toml only.
    /// Only suggestions explicitly marked as Cargo.toml-only are eligible.
    pub fn is_auto_fixable(&self) -> bool {
        matches!(self.autofix_safety, AutofixSafety::CargoTomlOnly)
    }
}

/// The type of suggestion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SuggestionKind {
    /// Replace with a modern alternative crate.
    ModernAlternative,
    /// Enable a built-in feature to drop a separate dependency.
    FeatureOptimization,
    /// Replace with a std equivalent (e.g., LazyLock).
    StdReplacement,
    /// Consolidate multiple crates doing the same thing.
    ComboWin,
    /// Crate is unmaintained — switch to maintained fork/successor.
    Unmaintained,
}

/// Impact level of a suggestion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Impact {
    High,
    Medium,
    Low,
}

/// Confidence level for a recommendation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

/// Estimated migration risk for applying a recommendation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MigrationRisk {
    High,
    Medium,
    Low,
}

/// Whether cargo-bless may safely apply the suggestion itself.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AutofixSafety {
    /// Safe to apply by changing Cargo.toml only.
    CargoTomlOnly,
    /// Requires source review or source edits.
    ManualOnly,
}

/// Primary evidence behind a recommendation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EvidenceSource {
    BlessedRs,
    RustSec,
    StdDocs,
    CrateDocs,
    CratesIo,
    Heuristic,
}

/// The embedded blessed.rs-based rule database.
/// Each rule maps a current pattern to a recommended modern alternative.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub pattern: String,
    pub replacement: String,
    pub kind: SuggestionKind,
    pub reason: String,
    pub source: String,
    pub condition: Option<String>,
    #[serde(default = "default_confidence")]
    pub confidence: Confidence,
    #[serde(default = "default_migration_risk")]
    pub migration_risk: MigrationRisk,
    #[serde(default = "default_autofix_safety")]
    pub autofix_safety: AutofixSafety,
    #[serde(default = "default_evidence_source")]
    pub evidence_source: EvidenceSource,
}

/// Derive impact from suggestion kind.
fn impact_for(kind: &SuggestionKind) -> Impact {
    match kind {
        SuggestionKind::Unmaintained | SuggestionKind::StdReplacement => Impact::High,
        SuggestionKind::ModernAlternative | SuggestionKind::ComboWin => Impact::Medium,
        SuggestionKind::FeatureOptimization => Impact::Low,
    }
}

fn default_confidence() -> Confidence {
    Confidence::Medium
}

fn default_migration_risk() -> MigrationRisk {
    MigrationRisk::Medium
}

fn default_autofix_safety() -> AutofixSafety {
    AutofixSafety::ManualOnly
}

fn default_evidence_source() -> EvidenceSource {
    EvidenceSource::Heuristic
}

/// Load suggestion rules, merging cached blessed.rs rules with the embedded fallback.
///
/// If `~/.cache/cargo-bless/blessed-rules.json` exists and is fresh,
/// those rules take priority. Any embedded rules whose patterns are NOT
/// covered by the blessed.rs set are appended (preserves hand-crafted
/// combo rules and custom additions).
pub fn load_rules() -> Vec<Rule> {
    let embedded: Vec<Rule> = {
        let json = include_str!("../data/suggestions.json");
        serde_json::from_str(json).expect("embedded suggestions.json should be valid")
    };

    // Try loading cached blessed.rs rules
    let cached = crate::updater::load_cached_rules();

    match cached {
        Some(mut blessed_rules) => {
            // Merge: blessed.rs rules first, then append embedded-only rules
            let blessed_patterns: std::collections::HashSet<String> =
                blessed_rules.iter().map(|r| r.pattern.clone()).collect();

            for rule in embedded {
                if !blessed_patterns.contains(&rule.pattern) {
                    blessed_rules.push(rule);
                }
            }

            blessed_rules
        }
        None => embedded,
    }
}

use std::fs;
/// Analyze resolved dependencies against the rule database.
///
/// Matching strategies:
/// - **Single-crate** rules (pattern has no `+`): fire if a direct dep has that name.
/// - **Combo** rules (pattern contains `+`): fire if ALL named crates are present
///   as direct deps.
use std::path::Path;

pub fn analyze(
    manifest_path: Option<&Path>,
    deps: &[ResolvedDep],
    rules: &[Rule],
) -> Vec<Suggestion> {
    let direct_names: HashSet<&str> = deps
        .iter()
        .filter(|d| d.is_direct)
        .map(|d| d.name.as_str())
        .collect();

    let mut suggestions = Vec::new();

    for rule in rules {
        let matched = if rule.pattern.contains('+') {
            // Combo rule: all named crates must be present
            let all_present = rule
                .pattern
                .split('+')
                .all(|name| direct_names.contains(name.trim()));

            if all_present {
                // For FeatureOptimization combo rules (like `reqwest+serde_json`),
                // check if the second crate is actually used directly in the codebase.
                // If it is, we shouldn't recommend dropping it.
                if rule.kind == SuggestionKind::FeatureOptimization {
                    let parts: Vec<&str> = rule.pattern.split('+').collect();
                    if parts.len() == 2 {
                        let extra_crate = parts[1].trim();
                        if is_crate_used_in_source(manifest_path, extra_crate) {
                            continue;
                        }
                    }
                }
                true
            } else {
                false
            }
        } else {
            // Single-crate rule: exact name match
            direct_names.contains(rule.pattern.as_str())
        };

        if matched {
            suggestions.push(Suggestion {
                kind: rule.kind.clone(),
                current: rule.pattern.clone(),
                recommended: rule.replacement.clone(),
                reason: rule.reason.clone(),
                source: rule.source.clone(),
                impact: impact_for(&rule.kind),
                confidence: rule.confidence.clone(),
                migration_risk: rule.migration_risk.clone(),
                autofix_safety: rule.autofix_safety.clone(),
                evidence_source: rule.evidence_source.clone(),
            });
        }
    }

    suggestions
}

/// Recursively scans `.rs` files in the project to determine if the crate is imported or used.
/// Checks `src`, `tests`, `benches`, and `examples` directories relative to the `manifest_path`.
fn is_crate_used_in_source(manifest_path: Option<&Path>, crate_name: &str) -> bool {
    let base_dir = manifest_path
        .and_then(|p| p.parent())
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let crate_ident = crate_name.replace('-', "_");

    // We check for some common usage patterns of the crate identifier
    let patterns = [
        format!("use {crate_ident}::"),
        format!("use {crate_ident};"),
        format!("{crate_ident}::"),
        format!("{crate_ident}!"),
    ];

    let dirs_to_check = ["src", "tests", "benches", "examples"];

    for dir_name in dirs_to_check {
        let dir_path = base_dir.join(dir_name);
        if !dir_path.exists() || !dir_path.is_dir() {
            continue;
        }

        if scan_dir_for_patterns(&dir_path, &patterns) {
            return true;
        }
    }

    false
}

fn scan_dir_for_patterns(dir: &Path, patterns: &[String]) -> bool {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return false,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if scan_dir_for_patterns(&path, patterns) {
                return true;
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            if let Ok(contents) = fs::read_to_string(&path) {
                for pattern in patterns {
                    if contents.contains(pattern) {
                        return true;
                    }
                }
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_rules() {
        let rules = load_rules();
        assert!(
            rules.len() >= 15,
            "should load at least 15 rules, got {}",
            rules.len()
        );

        // Spot-check a known rule
        let lazy = rules.iter().find(|r| r.pattern == "lazy_static").unwrap();
        assert_eq!(lazy.replacement, "std::sync::LazyLock");
        assert!(matches!(lazy.kind, SuggestionKind::StdReplacement));
    }

    #[test]
    fn test_analyze_single_crate_match() {
        let rules = load_rules();
        let deps = vec![
            ResolvedDep {
                name: "lazy_static".into(),
                version: "1.5.0".into(),
                enabled_features: vec![],
                available_features: vec![],
                source: Some("registry".into()),
                repository: None,
                is_direct: true,
            },
            ResolvedDep {
                name: "serde".into(),
                version: "1.0.0".into(),
                enabled_features: vec![],
                available_features: vec![],
                source: Some("registry".into()),
                repository: None,
                is_direct: true,
            },
        ];

        let suggestions = analyze(None, &deps, &rules);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].current, "lazy_static");
        assert_eq!(suggestions[0].recommended, "std::sync::LazyLock");
        assert_eq!(suggestions[0].impact, Impact::High);
    }

    #[test]
    fn test_analyze_combo_match() {
        let deps = vec![
            ResolvedDep {
                name: "reqwest".into(),
                version: "0.12.0".into(),
                enabled_features: vec![],
                available_features: vec![],
                source: Some("registry".into()),
                repository: None,
                is_direct: true,
            },
            ResolvedDep {
                // Use a crate name that definitely isn't used in this test file
                name: "some_unused_crate".into(),
                version: "1.0.0".into(),
                enabled_features: vec![],
                available_features: vec![],
                source: Some("registry".into()),
                repository: None,
                is_direct: true,
            },
        ];

        // Create a custom rule to avoid triggering the usage grep for real dependencies
        let custom_rule = Rule {
            pattern: "reqwest+some_unused_crate".into(),
            replacement: "reqwest with some feature".into(),
            kind: SuggestionKind::FeatureOptimization,
            reason: "".into(),
            source: "".into(),
            condition: None,
            confidence: Confidence::High,
            migration_risk: MigrationRisk::Low,
            autofix_safety: AutofixSafety::CargoTomlOnly,
            evidence_source: EvidenceSource::Heuristic,
        };

        let suggestions = analyze(None, &deps, &[custom_rule]);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].current, "reqwest+some_unused_crate");
        assert!(matches!(
            suggestions[0].kind,
            SuggestionKind::FeatureOptimization
        ));
        assert_eq!(suggestions[0].impact, Impact::Low);
    }

    #[test]
    fn test_analyze_combo_partial_no_match() {
        let rules = load_rules();
        // Only reqwest present, no serde_json — combo should NOT fire
        let deps = vec![ResolvedDep {
            name: "reqwest".into(),
            version: "0.12.0".into(),
            enabled_features: vec![],
            available_features: vec![],
            source: Some("registry".into()),
            repository: None,
            is_direct: true,
        }];

        let suggestions = analyze(None, &deps, &rules);
        assert!(
            suggestions.is_empty(),
            "combo rule should not fire with only one of the pair"
        );
    }

    #[test]
    fn test_analyze_ignores_transitive() {
        let rules = load_rules();
        let deps = vec![ResolvedDep {
            name: "lazy_static".into(),
            version: "1.5.0".into(),
            enabled_features: vec![],
            available_features: vec![],
            source: Some("registry".into()),
            repository: None,
            is_direct: false, // transitive — should be ignored
        }];

        let suggestions = analyze(None, &deps, &rules);
        assert!(
            suggestions.is_empty(),
            "transitive deps should not trigger suggestions"
        );
    }

    #[test]
    fn test_analyze_multiple_matches() {
        let rules = load_rules();
        let deps = vec![
            ResolvedDep {
                name: "lazy_static".into(),
                version: "1.5.0".into(),
                enabled_features: vec![],
                available_features: vec![],
                source: Some("registry".into()),
                repository: None,
                is_direct: true,
            },
            ResolvedDep {
                name: "structopt".into(),
                version: "0.3.0".into(),
                enabled_features: vec![],
                available_features: vec![],
                source: Some("registry".into()),
                repository: None,
                is_direct: true,
            },
            ResolvedDep {
                name: "memmap".into(),
                version: "0.7.0".into(),
                enabled_features: vec![],
                available_features: vec![],
                source: Some("registry".into()),
                repository: None,
                is_direct: true,
            },
        ];

        let suggestions = analyze(None, &deps, &rules);
        assert_eq!(suggestions.len(), 3);

        let names: Vec<&str> = suggestions.iter().map(|s| s.current.as_str()).collect();
        assert!(names.contains(&"lazy_static"));
        assert!(names.contains(&"structopt"));
        assert!(names.contains(&"memmap"));
    }

    #[test]
    fn test_analyze_clean_project() {
        let rules = load_rules();
        // Modern deps that shouldn't trigger any rules
        let deps = vec![
            ResolvedDep {
                name: "clap".into(),
                version: "4.5.0".into(),
                enabled_features: vec!["derive".into()],
                available_features: vec![],
                source: Some("registry".into()),
                repository: None,
                is_direct: true,
            },
            ResolvedDep {
                name: "serde".into(),
                version: "1.0.0".into(),
                enabled_features: vec!["derive".into()],
                available_features: vec![],
                source: Some("registry".into()),
                repository: None,
                is_direct: true,
            },
        ];

        let suggestions = analyze(None, &deps, &rules);
        assert!(
            suggestions.is_empty(),
            "modern deps should not trigger any suggestions"
        );
    }

    #[test]
    fn test_impact_derivation() {
        assert_eq!(impact_for(&SuggestionKind::Unmaintained), Impact::High);
        assert_eq!(impact_for(&SuggestionKind::StdReplacement), Impact::High);
        assert_eq!(
            impact_for(&SuggestionKind::ModernAlternative),
            Impact::Medium
        );
        assert_eq!(impact_for(&SuggestionKind::ComboWin), Impact::Medium);
        assert_eq!(
            impact_for(&SuggestionKind::FeatureOptimization),
            Impact::Low
        );
    }
}
