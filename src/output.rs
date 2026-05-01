//! Output layer — renders the modernization report to the terminal
//! with colored output, emojis, and actionable links.

use std::collections::HashMap;

use colored::*;
use serde::Serialize;

use crate::code_audit::{kind_label, CodeAuditReport};
use crate::intel::CrateIntel;
use crate::suggestions::{Impact, Suggestion, SuggestionKind};

/// Render the full modernization report to stdout.
///
/// `intel` provides optional live metadata for enriching suggestions
/// with version info, downloads, and freshness (can be empty).
pub fn render_report(
    project_name: &str,
    version: &str,
    suggestions: &[Suggestion],
    intel: &HashMap<String, CrateIntel>,
) {
    if suggestions.is_empty() {
        println!(
            "{}",
            "✅ Your dependencies are already blessed! Nothing to modernize.".green()
        );
        return;
    }

    println!(
        "{}",
        format!("🚀 Modernization report for {} v{}", project_name, version).bold()
    );
    println!();

    for suggestion in suggestions {
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

        // Base suggestion line
        println!(
            " {} {} {} → {} ({})",
            icon,
            impact_tag,
            suggestion.current.yellow(),
            suggestion.recommended.green(),
            suggestion.reason.dimmed()
        );

        // Enrich with live intel if available
        // For combo rules like "reqwest+serde_json", check each crate name
        let crate_names: Vec<&str> = suggestion.current.split('+').collect();
        for crate_name in crate_names {
            if let Some(info) = intel.get(crate_name) {
                let mut enrichment = format!("   latest: v{}", info.latest_version);
                if let Some(recent) = info.recent_downloads {
                    enrichment
                        .push_str(&format!(", {} recent downloads", format_downloads(recent)));
                }
                println!("   {}", enrichment.dimmed());
            }
        }
    }

    let high_count = suggestions
        .iter()
        .filter(|s| matches!(s.impact, Impact::High))
        .count();

    println!();
    println!(
        "{}",
        format!(
            "{} high-impact upgrade{} available. Run `cargo bless --fix` to apply safely.",
            high_count,
            if high_count == 1 { "" } else { "s" }
        )
        .bold()
    );
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
pub struct JsonReport<'a> {
    pub dependency_suggestions: &'a [Suggestion],
    pub code_audit: Option<&'a CodeAuditReport>,
}

/// Render a unified JSON report for machine consumption.
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
