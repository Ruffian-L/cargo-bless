//! Privacy-safe `--feedback` output for voluntary issue reports.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;

use crate::code_audit::{BullshitAlert, CodeAuditReport};
use crate::suggestions::{Impact, Suggestion};
use anyhow::Result;

const HOTSPOT_LIMIT: usize = 5;

pub fn emit_feedback_stdout(
    version: &str,
    manifest: Option<&Path>,
    direct_deps: usize,
    total_deps: usize,
    suggestions: &[Suggestion],
    code_audit: &CodeAuditReport,
) -> Result<()> {
    let high_impact = suggestions
        .iter()
        .filter(|s| matches!(s.impact, Impact::High))
        .count();
    let project_root = manifest
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let hotspots = top_hotspots(code_audit, &project_root, HOTSPOT_LIMIT);

    println!("cargo-bless feedback block");
    println!("version: {version}");
    println!("direct_deps: {direct_deps}");
    println!("total_deps: {total_deps}");
    println!("suggestions: {}", suggestions.len());
    println!("high_impact: {high_impact}");
    println!("code_audit_findings: {}", code_audit.alerts.len());
    println!("top_hotspots:");
    if hotspots.is_empty() {
        println!("  - (none)");
    } else {
        for h in hotspots {
            let shown = h.strip_prefix("./").unwrap_or(&h);
            println!("  - {shown}");
        }
    }

    Ok(())
}

fn hotspot_key(alert: &BullshitAlert, project_root: &Path) -> String {
    let rel = display_path_under_root(&alert.file, project_root);
    if let Some(name) = extract_rust_fn_name(&alert.context_snippet) {
        format!("{}::{}", rel, name)
    } else {
        format!("{}:{}", rel, alert.line)
    }
}

fn display_path_under_root(path: &Path, project_root: &Path) -> String {
    if let Ok(stripped) = path.strip_prefix(project_root) {
        return normalize_path_sep(stripped);
    }
    match (path.canonicalize(), project_root.canonicalize()) {
        (Ok(pa), Ok(pr)) => {
            if let Ok(stripped) = pa.strip_prefix(&pr) {
                normalize_path_sep(stripped)
            } else {
                normalize_path_sep(path)
            }
        }
        _ => normalize_path_sep(path),
    }
}

fn normalize_path_sep(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// Best-effort: pull `fn foo` name from snippet or declaration line (including `async fn`).
fn extract_rust_fn_name(snippet: &str) -> Option<&str> {
    static FN_RE: OnceLock<Regex> = OnceLock::new();
    let re = FN_RE.get_or_init(|| Regex::new(r"\bfn\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap());
    re.captures(snippet)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
}

/// Rank hotspot locations by summed alert severity (higher first), then alphabetically for stability.
fn top_hotspots(report: &CodeAuditReport, project_root: &Path, limit: usize) -> Vec<String> {
    let mut scores: HashMap<String, f32> = HashMap::new();
    for alert in &report.alerts {
        let key = hotspot_key(alert, project_root);
        *scores.entry(key).or_insert(0.0) += alert.severity;
    }
    let mut items: Vec<(String, f32)> = scores.into_iter().collect();
    items.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    items.into_iter().take(limit).map(|(k, _)| k).collect()
}

#[cfg(test)]
mod tests {
    use super::extract_rust_fn_name;

    #[test]
    fn extracts_fn_name_after_async() {
        assert_eq!(
            extract_rust_fn_name("pub async fn run_simulation() {}"),
            Some("run_simulation")
        );
    }

    #[test]
    fn extracts_fn_name_plain() {
        assert_eq!(
            extract_rust_fn_name("fn apply_forces() {"),
            Some("apply_forces")
        );
    }
}
