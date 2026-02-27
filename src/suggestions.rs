//! Suggestion engine — rule-based recommendations from blessed.rs mappings
//! with optional LLM RAG grounding for context-aware 2026 advice.

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
}

/// Derive impact from suggestion kind.
fn impact_for(kind: &SuggestionKind) -> Impact {
    match kind {
        SuggestionKind::Unmaintained | SuggestionKind::StdReplacement => Impact::High,
        SuggestionKind::ModernAlternative | SuggestionKind::ComboWin => Impact::Medium,
        SuggestionKind::FeatureOptimization => Impact::Low,
    }
}

/// Load the built-in suggestion rules from the embedded JSON database.
pub fn load_rules() -> Vec<Rule> {
    let json = include_str!("../data/suggestions.json");
    serde_json::from_str(json).expect("embedded suggestions.json should be valid")
}

/// Analyze resolved dependencies against the rule database.
///
/// Matching strategies:
/// - **Single-crate** rules (pattern has no `+`): fire if a direct dep has that name.
/// - **Combo** rules (pattern contains `+`): fire if ALL named crates are present
///   as direct deps.
pub fn analyze(deps: &[ResolvedDep], rules: &[Rule]) -> Vec<Suggestion> {
    let direct_names: HashSet<&str> = deps
        .iter()
        .filter(|d| d.is_direct)
        .map(|d| d.name.as_str())
        .collect();

    let mut suggestions = Vec::new();

    for rule in rules {
        let matched = if rule.pattern.contains('+') {
            // Combo rule: all named crates must be present
            rule.pattern
                .split('+')
                .all(|name| direct_names.contains(name.trim()))
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
            });
        }
    }

    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_rules() {
        let rules = load_rules();
        assert_eq!(rules.len(), 7, "should load all 7 starter rules");

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
                features: vec![],
                source: Some("registry".into()),
                repository: None,
                is_direct: true,
            },
            ResolvedDep {
                name: "serde".into(),
                version: "1.0.0".into(),
                features: vec![],
                source: Some("registry".into()),
                repository: None,
                is_direct: true,
            },
        ];

        let suggestions = analyze(&deps, &rules);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].current, "lazy_static");
        assert_eq!(suggestions[0].recommended, "std::sync::LazyLock");
        assert_eq!(suggestions[0].impact, Impact::High);
    }

    #[test]
    fn test_analyze_combo_match() {
        let rules = load_rules();
        let deps = vec![
            ResolvedDep {
                name: "reqwest".into(),
                version: "0.12.0".into(),
                features: vec![],
                source: Some("registry".into()),
                repository: None,
                is_direct: true,
            },
            ResolvedDep {
                name: "serde_json".into(),
                version: "1.0.0".into(),
                features: vec![],
                source: Some("registry".into()),
                repository: None,
                is_direct: true,
            },
        ];

        let suggestions = analyze(&deps, &rules);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].current, "reqwest+serde_json");
        assert!(matches!(suggestions[0].kind, SuggestionKind::FeatureOptimization));
        assert_eq!(suggestions[0].impact, Impact::Low);
    }

    #[test]
    fn test_analyze_combo_partial_no_match() {
        let rules = load_rules();
        // Only reqwest present, no serde_json — combo should NOT fire
        let deps = vec![ResolvedDep {
            name: "reqwest".into(),
            version: "0.12.0".into(),
            features: vec![],
            source: Some("registry".into()),
            repository: None,
            is_direct: true,
        }];

        let suggestions = analyze(&deps, &rules);
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
            features: vec![],
            source: Some("registry".into()),
            repository: None,
            is_direct: false, // transitive — should be ignored
        }];

        let suggestions = analyze(&deps, &rules);
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
                features: vec![],
                source: Some("registry".into()),
                repository: None,
                is_direct: true,
            },
            ResolvedDep {
                name: "structopt".into(),
                version: "0.3.0".into(),
                features: vec![],
                source: Some("registry".into()),
                repository: None,
                is_direct: true,
            },
            ResolvedDep {
                name: "memmap".into(),
                version: "0.7.0".into(),
                features: vec![],
                source: Some("registry".into()),
                repository: None,
                is_direct: true,
            },
        ];

        let suggestions = analyze(&deps, &rules);
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
                features: vec!["derive".into()],
                source: Some("registry".into()),
                repository: None,
                is_direct: true,
            },
            ResolvedDep {
                name: "serde".into(),
                version: "1.0.0".into(),
                features: vec!["derive".into()],
                source: Some("registry".into()),
                repository: None,
                is_direct: true,
            },
        ];

        let suggestions = analyze(&deps, &rules);
        assert!(suggestions.is_empty(), "modern deps should not trigger any suggestions");
    }

    #[test]
    fn test_impact_derivation() {
        assert_eq!(impact_for(&SuggestionKind::Unmaintained), Impact::High);
        assert_eq!(impact_for(&SuggestionKind::StdReplacement), Impact::High);
        assert_eq!(impact_for(&SuggestionKind::ModernAlternative), Impact::Medium);
        assert_eq!(impact_for(&SuggestionKind::ComboWin), Impact::Medium);
        assert_eq!(impact_for(&SuggestionKind::FeatureOptimization), Impact::Low);
    }
}
