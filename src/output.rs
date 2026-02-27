//! Output layer — renders the modernization report to the terminal
//! with colored output, emojis, and actionable links.

use std::collections::HashMap;

use colored::*;

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
                    enrichment.push_str(&format!(", {} recent downloads", format_downloads(recent)));
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
