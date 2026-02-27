//! Output layer — renders the modernization report to the terminal
//! with colored output, emojis, and actionable links.

use colored::*;

use crate::suggestions::{Impact, Suggestion, SuggestionKind};

/// Render the full modernization report to stdout.
pub fn render_report(project_name: &str, version: &str, suggestions: &[Suggestion]) {
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

        println!(
            " {} {} {} → {} ({})",
            icon,
            impact_tag,
            suggestion.current.yellow(),
            suggestion.recommended.green(),
            suggestion.reason.dimmed()
        );
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
