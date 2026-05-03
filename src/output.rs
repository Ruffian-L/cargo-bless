//! Output layer — renders the modernization report to the terminal
//! with colored output, emojis, and actionable links.

use std::collections::HashMap;
use std::path::Path;

use colored::*;
use serde::Serialize;

use crate::code_audit::{kind_label, CodeAuditReport};
use crate::intel::CrateIntel;
use crate::suggestions::{AutofixSafety, Confidence, Impact, MigrationRisk, SuggestionKind};
use crate::suggestions::{EvidenceSource, Suggestion};

fn print_suggestion_detail(suggestion: &Suggestion, intel: &HashMap<String, CrateIntel>) {
    let icon = match suggestion.kind {
        SuggestionKind::ModernAlternative => "•",
        SuggestionKind::FeatureOptimization => "•",
        SuggestionKind::StdReplacement => "•",
        SuggestionKind::ComboWin => "•",
        SuggestionKind::Unmaintained => "⚠️",
    };

    let impact_tag = match suggestion.impact {
        Impact::High => "[HIGH]".red().bold(),
        Impact::Medium => "[MED]".yellow().bold(),
        Impact::Low => "[LOW]".dimmed(),
    };
    let confidence_tag = match suggestion.confidence {
        Confidence::High => "[HIGH confidence]".green().bold(),
        Confidence::Medium => "[MED confidence]".yellow(),
        Confidence::Low => "[LOW confidence]".red(),
    };
    let risk_tag = match suggestion.migration_risk {
        MigrationRisk::High => "[HIGH risk]".red().bold(),
        MigrationRisk::Medium => "[MED risk]".yellow(),
        MigrationRisk::Low => "[LOW risk]".green(),
    };
    let autofix_tag = match suggestion.autofix_safety {
        AutofixSafety::CargoTomlOnly => "[autofix: Cargo.toml-only]".green(),
        AutofixSafety::ManualOnly => "[autofix: manual]".dimmed(),
    };
    let verb = match suggestion.confidence {
        Confidence::High => "→",
        Confidence::Medium | Confidence::Low => "→ consider",
    };

    println!(
        " {} {} {} {} {}",
        icon,
        impact_tag,
        suggestion.current.yellow(),
        verb,
        suggestion.recommended.green(),
    );
    println!(
        "   {} {} {} {}",
        confidence_tag,
        risk_tag,
        autofix_tag,
        format!("evidence: {}", evidence_label(&suggestion.evidence_source)).dimmed()
    );
    println!("   {}", suggestion.reason.dimmed());

    let crate_names: Vec<&str> = suggestion.current.split('+').collect();
    for crate_name in crate_names {
        if let Some(info) = intel.get(crate_name) {
            let mut enrichment = format!("   latest: v{}", info.latest_version);
            if let Some(recent) = info.recent_downloads {
                enrichment.push_str(&format!(", {} recent downloads", format_downloads(recent)));
            }
            println!("   {}", enrichment.dimmed());
        }
    }
}

/// One workspace member’s dependency suggestions shown in plain text reports.
#[derive(Clone, Copy, Debug)]
pub struct PackageSuggestionView<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub manifest_path: &'a Path,
    pub suggestions: &'a [Suggestion],
}

/// Render modernization output for one or many packages (`--workspace` / `--package` use multi headers).
fn use_multi_headers(packages: &[PackageSuggestionView<'_>]) -> bool {
    if packages.len() > 1 {
        return true;
    }
    packages
        .first()
        .is_some_and(|p| p.suggestions.iter().any(|s| s.package.is_some()))
}

/// Single-root modernization report (`manifest_path` is only useful in grouped layouts).
pub fn render_report(
    project_name: &str,
    version: &str,
    suggestions: &[Suggestion],
    intel: &HashMap<String, CrateIntel>,
) {
    render_packages_modernization(
        &[PackageSuggestionView {
            name: project_name,
            version,
            manifest_path: Path::new("Cargo.toml"),
            suggestions,
        }],
        intel,
    );
}

pub fn render_packages_modernization(
    packages: &[PackageSuggestionView<'_>],
    intel: &HashMap<String, CrateIntel>,
) {
    let all_empty = packages.iter().all(|p| p.suggestions.is_empty());

    if all_empty {
        println!(
            "{}",
            "✅ Your dependencies are already blessed! Nothing to modernize.".green()
        );
        return;
    }

    let multi = use_multi_headers(packages);

    if !multi {
        let p = &packages[0];
        println!(
            "{}",
            format!("🚀 Modernization report for {} v{}", p.name, p.version).bold()
        );
        println!();
        for suggestion in p.suggestions {
            print_suggestion_detail(suggestion, intel);
        }
    } else {
        for p in packages {
            if p.suggestions.is_empty() {
                continue;
            }
            println!(
                "{}",
                format!(
                    "📦 {} v{} ({})",
                    p.name,
                    p.version,
                    p.manifest_path.display()
                )
                .bold()
            );
            println!();
            for suggestion in p.suggestions {
                print_suggestion_detail(suggestion, intel);
            }
            println!();
        }
    }

    let high_count = packages
        .iter()
        .flat_map(|p| p.suggestions)
        .filter(|s| matches!(s.impact, Impact::High))
        .count();

    println!();
    println!(
        "{}",
        format!(
            "{} high-impact upgrade{} available. `--fix` only edits Cargo.toml (never Rust source). Run `cargo bless --fix --dry-run` to preview.",
            high_count,
            if high_count == 1 { "" } else { "s" }
        )
        .bold()
    );
}

fn suggestion_kind_slug(kind: &SuggestionKind) -> &'static str {
    match kind {
        SuggestionKind::ModernAlternative => "modern_alternative",
        SuggestionKind::FeatureOptimization => "feature_opt",
        SuggestionKind::StdReplacement => "std_replace",
        SuggestionKind::ComboWin => "combo_win",
        SuggestionKind::Unmaintained => "unmaintained",
    }
}

/// Paste-friendly condensed output (`--summary`).
pub fn render_summary(scan_stats: &[(&str, usize, usize)], suggestions: &[Suggestion]) {
    let pkg_ct = scan_stats.len();
    println!(
        "{}",
        format!(
            "📊 Summary — scanned {} workspace member{}",
            pkg_ct,
            if pkg_ct == 1 { "" } else { "s" }
        )
        .bold()
    );
    for (name, direct_ct, total_ct) in scan_stats {
        println!(
            "   • {} — {} direct deps, {} total in resolve",
            name.bold(),
            direct_ct,
            total_ct
        );
    }

    let mut hi = 0usize;
    let mut med = 0usize;
    let mut low = 0usize;
    for s in suggestions {
        match s.impact {
            Impact::High => hi += 1,
            Impact::Medium => med += 1,
            Impact::Low => low += 1,
        }
    }

    println!();
    println!("Suggestions after policy: {}", suggestions.len());
    println!("By impact — high: {hi}, medium: {med}, low: {low}");

    let mut kind_counts = HashMap::<&'static str, usize>::new();
    for s in suggestions {
        *kind_counts
            .entry(suggestion_kind_slug(&s.kind))
            .or_default() += 1;
    }
    let mut kind_pairs: Vec<_> = kind_counts.into_iter().collect();
    kind_pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    if !kind_pairs.is_empty() {
        println!(
            "By kind — {}",
            kind_pairs
                .iter()
                .map(|(k, c)| format!("{k}: {c}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    println!();
    println!("{}", "Top patterns:".bold());
    let mut patterns: Vec<String> = suggestions
        .iter()
        .map(|s| format!("{} → {}", s.current, s.recommended))
        .collect();
    patterns.sort();
    patterns.dedup();
    const MAX: usize = 14usize;
    for line in patterns.iter().take(MAX) {
        println!("   • {}", line);
    }
    if patterns.len() > MAX {
        println!("   … and {} more", patterns.len() - MAX);
    }

    println!();
    println!(
        "{}",
        "`--fix` changes Cargo.toml entries only — never Rust source. Check `autofix_safety` on each suggestion."
            .dimmed()
    );
}

fn evidence_label(source: &EvidenceSource) -> &'static str {
    match source {
        EvidenceSource::BlessedRs => "blessed.rs",
        EvidenceSource::RustSec => "RustSec",
        EvidenceSource::StdDocs => "std docs",
        EvidenceSource::CrateDocs => "crate docs",
        EvidenceSource::CratesIo => "crates.io",
        EvidenceSource::Heuristic => "heuristic",
    }
}

pub fn render_code_audit_report(report: &CodeAuditReport, verbose: bool) {
    println!();
    println!("{}", "🧨 Bullshit detector code audit".bold());
    println!(
        "{}",
        format!(
            "Scanned {} Rust file{}.",
            report.files_scanned,
            if report.files_scanned == 1 { "" } else { "s" }
        )
        .dimmed()
    );

    if report.is_clean() {
        println!("{}", "✅ No bullshit detected in Rust source.".green());
        return;
    }

    println!(
        "{}",
        format!(
            "🚨 Bullshit detected: {} finding{} · heat {:.1}",
            report.alerts.len(),
            if report.alerts.len() == 1 { "" } else { "s" },
            report.alerts.iter().map(|a| a.severity).sum::<f32>() * 10.0
        )
        .red()
        .bold()
    );

    let mut counts = HashMap::<&'static str, usize>::new();
    for alert in &report.alerts {
        *counts.entry(kind_label(alert.kind)).or_default() += 1;
    }
    let mut counts: Vec<_> = counts.into_iter().collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let summary = counts
        .iter()
        .map(|(kind, count)| format!("{kind}: {count}"))
        .collect::<Vec<_>>()
        .join(", ");
    println!("{}", summary.dimmed());
    println!();

    let shown = if verbose {
        report.alerts.len()
    } else {
        report.alerts.len().min(5)
    };

    for alert in report.alerts.iter().take(shown) {
        println!(
            " {} {} {}:{}:{}",
            "•".red(),
            kind_label(alert.kind).yellow().bold(),
            alert.file.display().to_string().dimmed(),
            alert.line,
            alert.column
        );
        println!("   {}", alert.why_bs);
        println!("   {}", format!("Fix: {}", alert.suggestion).green());
        if !alert.context_snippet.is_empty() {
            println!("   {}", alert.context_snippet.dimmed());
        }
    }

    if !verbose && report.alerts.len() > shown {
        println!();
        println!(
            "{}",
            format!(
                "Showing top {shown}. Run with --verbose for all {} findings, or --json for machine output.",
                report.alerts.len()
            )
            .dimmed()
        );
    }
}

/// Format download counts in a human-readable way (e.g., "1.2M", "456K").
fn format_downloads(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_downloads() {
        assert_eq!(format_downloads(0), "0");
        assert_eq!(format_downloads(500), "500");
        assert_eq!(format_downloads(1_500), "1.5K");
        assert_eq!(format_downloads(1_200_000), "1.2M");
        assert_eq!(format_downloads(100_000_000), "100.0M");
    }
}

#[derive(Serialize)]
pub struct JsonPackageOutput<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub manifest_path: String,
    pub dependency_suggestions: &'a [Suggestion],
}

/// Machine-readable report: `cargo_bless_version`, `workspace_scan`, `packages`, optional `code_audit`.
#[derive(Serialize)]
pub struct JsonReportUnified<'a> {
    pub cargo_bless_version: &'a str,
    pub workspace_scan: bool,
    pub packages: Vec<JsonPackageOutput<'a>>,
    pub code_audit: Option<&'a CodeAuditReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hardcoded_values: Option<&'a [crate::bs_detector::BSHit]>,
}

pub fn render_unified_json(report: JsonReportUnified<'_>) {
    match serde_json::to_string_pretty(&report) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("cargo-bless: failed to serialize JSON output: {}", e),
    }
}

/// Legacy shape for compatibility (flat `dependency_suggestions` at top level).
#[derive(Serialize)]
pub struct JsonReport<'a> {
    pub dependency_suggestions: &'a [Suggestion],
    pub code_audit: Option<&'a CodeAuditReport>,
}

/// Legacy narrow JSON (**`dependency_suggestions`** at top level) for crates embedding v0.1 output.
pub fn render_json_report(suggestions: &[Suggestion], code_audit: Option<&CodeAuditReport>) {
    let report = JsonReport {
        dependency_suggestions: suggestions,
        code_audit,
    };
    match serde_json::to_string_pretty(&report) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("cargo-bless: failed to serialize JSON output: {}", e),
    }
}

/// Render suggestions as a JSON array to stdout.
/// Kept for library callers that rely on the old narrow JSON shape.
pub fn render_json(suggestions: &[Suggestion]) {
    match serde_json::to_string_pretty(suggestions) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("cargo-bless: failed to serialize JSON output: {}", e),
    }
}
