//! Policy layer — parses bless.toml for custom rules, overrides, and enforcement settings.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Top-level policy configuration loaded from bless.toml.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Policy {
    /// Custom suggestion rules to add or override defaults.
    #[serde(default)]
    pub rules: Vec<PolicyRule>,

    /// Packages to exclude from analysis entirely.
    #[serde(default)]
    pub ignore_packages: Vec<String>,

    /// Override the default fail-on severity thresholds.
    #[serde(default)]
    pub fail_on: Option<Vec<String>>,

    /// Per-package overrides (e.g., pin a version, suppress specific rules).
    #[serde(default)]
    pub packages: HashMap<String, PackagePolicy>,

    /// Global settings.
    #[serde(default)]
    pub settings: PolicySettings,

    /// Bullshit detector code-audit suppressions.
    #[serde(default)]
    pub code_audit: CodeAuditPolicy,
}

/// A custom rule from bless.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    /// Crate name or combo pattern (e.g., "reqwest+serde_json").
    pub pattern: String,

    /// Recommended replacement.
    pub replacement: String,

    /// Reason for the suggestion.
    pub reason: String,

    /// Kind of suggestion. Defaults to "modern_alternative" if omitted.
    #[serde(default = "default_rule_kind")]
    pub kind: String,

    /// Optional condition (e.g., "version < 0.12").
    pub condition: Option<String>,
}

fn default_rule_kind() -> String {
    "modern_alternative".to_string()
}

/// Per-package policy overrides.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PackagePolicy {
    /// Suppress all suggestions for this package.
    #[serde(default)]
    pub suppress: bool,

    /// Pin to a specific version (prevents upgrade suggestions).
    pub pin_version: Option<String>,

    /// Custom reason for keeping the current dependency.
    pub keep_reason: Option<String>,
}

/// Global policy settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PolicySettings {
    /// Whether to run in offline mode by default.
    #[serde(default)]
    pub offline: bool,

    /// Whether to include dev-dependencies in analysis by default.
    #[serde(default)]
    pub all_targets: bool,

    /// Maximum number of suggestions to show per run (0 = unlimited).
    #[serde(default)]
    pub max_suggestions: usize,

    /// Confidence threshold for LLM-powered suggestions (0.0–1.0).
    #[serde(default)]
    pub min_confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodeAuditPolicy {
    /// Suppress findings in paths containing any of these strings.
    #[serde(default)]
    pub ignore_paths: Vec<String>,

    /// Suppress findings with these kind names, e.g. "UnwrapAbuse".
    #[serde(default)]
    pub ignore_kinds: Vec<String>,
}

/// Load policy from a bless.toml file at the given path.
/// Returns None if the file does not exist or cannot be parsed.
pub fn load_policy(path: &Path) -> Option<Policy> {
    let content = fs::read_to_string(path).ok()?;
    let policy: Policy = toml_edit::de::from_str(&content).ok()?;
    Some(policy)
}

/// Filter suggestions based on policy rules.
/// - Removes suggestions for ignored packages.
/// - Applies per-package suppress/pin overrides.
/// - Caps total suggestions if max_suggestions is set.
pub fn apply_policy(
    suggestions: Vec<crate::suggestions::Suggestion>,
    policy: &Policy,
) -> Vec<crate::suggestions::Suggestion> {
    let mut filtered: Vec<_> = suggestions
        .into_iter()
        .filter(|s| {
            // Check ignore_packages
            if policy.ignore_packages.iter().any(|p| s.current.contains(p)) {
                return false;
            }

            // Check per-package suppress
            for pkg_name in s.current.split('+').map(|n| n.trim()) {
                if let Some(pkg_policy) = policy.packages.get(pkg_name) {
                    if pkg_policy.suppress {
                        return false;
                    }
                }
            }

            true
        })
        .collect();

    // Apply max_suggestions cap
    if policy.settings.max_suggestions > 0 {
        filtered.truncate(policy.settings.max_suggestions);
    }

    filtered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_policy_from_string() {
        let toml_content = r#"
ignore_packages = ["ignored_dep"]

[[rules]]
pattern = "old_crate"
replacement = "new_crate"
reason = "old_crate is unmaintained"
kind = "modern_alternative"

[packages.foo]
suppress = true
"#;
        let policy: Policy = toml_edit::de::from_str(toml_content).unwrap();
        assert_eq!(policy.rules.len(), 1);
        assert_eq!(policy.rules[0].pattern, "old_crate");
        assert!(policy.ignore_packages.contains(&"ignored_dep".to_string()));
        assert!(policy.packages.get("foo").unwrap().suppress);
    }

    #[test]
    fn test_apply_policy_suppress() {
        let policy = Policy {
            packages: HashMap::from_iter([(
                "lazy_static".to_string(),
                PackagePolicy {
                    suppress: true,
                    pin_version: None,
                    keep_reason: None,
                },
            )]),
            ..Default::default()
        };

        let suggestions = vec![crate::suggestions::Suggestion {
            kind: crate::suggestions::SuggestionKind::StdReplacement,
            current: "lazy_static".into(),
            recommended: "std::sync::LazyLock".into(),
            reason: "built-in since 1.80".into(),
            source: "test".into(),
            impact: crate::suggestions::Impact::High,
            confidence: crate::suggestions::Confidence::High,
            migration_risk: crate::suggestions::MigrationRisk::Low,
            autofix_safety: crate::suggestions::AutofixSafety::ManualOnly,
            evidence_source: crate::suggestions::EvidenceSource::Heuristic,
        }];

        let filtered = apply_policy(suggestions, &policy);
        assert!(
            filtered.is_empty(),
            "suppressed suggestion should be removed"
        );
    }

    #[test]
    fn test_apply_policy_max_suggestions() {
        let policy = Policy {
            settings: PolicySettings {
                max_suggestions: 2,
                ..Default::default()
            },
            ..Default::default()
        };

        let suggestions: Vec<_> = (0..5)
            .map(|i| crate::suggestions::Suggestion {
                kind: crate::suggestions::SuggestionKind::ModernAlternative,
                current: format!("dep_{}", i),
                recommended: format!("new_dep_{}", i),
                reason: "test".into(),
                source: "test".into(),
                impact: crate::suggestions::Impact::Low,
                confidence: crate::suggestions::Confidence::Medium,
                migration_risk: crate::suggestions::MigrationRisk::Medium,
                autofix_safety: crate::suggestions::AutofixSafety::ManualOnly,
                evidence_source: crate::suggestions::EvidenceSource::Heuristic,
            })
            .collect();

        let filtered = apply_policy(suggestions, &policy);
        assert_eq!(filtered.len(), 2, "should cap at max_suggestions");
    }
}
