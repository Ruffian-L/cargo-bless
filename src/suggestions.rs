//! Suggestion engine — rule-based recommendations from blessed.rs mappings
//! with optional LLM RAG grounding for context-aware 2026 advice.

use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Load the built-in suggestion rules.
pub fn load_rules() -> Vec<Rule> {
    // TODO: Load from embedded suggestions.json or YAML
    // Starter rules from blessed.rs (hardcoded for scaffold):
    vec![]
}

/// Analyze resolved dependencies against the rule database.
pub fn analyze(
    _deps: &[crate::parser::ResolvedDep],
    _rules: &[Rule],
) -> Vec<Suggestion> {
    // TODO: Match deps against rules, considering features and versions
    // TODO: Check for combo patterns (e.g., reqwest + serde_json)
    // TODO: Check for std replacements based on MSRV

    vec![]
}
