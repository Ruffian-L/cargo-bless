//! Output layer — renders the modernization report to the terminal
//! with colored output, emojis, and actionable links.

use std::collections::HashMap;
use std::io::Write;

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
    let mut stdout = std::io::stdout();
    let _ = render_report_to(&mut stdout, project_name, version, suggestions, intel);
}

/// Render the full modernization report to the given writer.
pub fn render_report_to(
    mut writer: impl Write,
    project_name: &str,
    version: &str,
    suggestions: &[Suggestion],
    intel: &HashMap<String, CrateIntel>,
) -> std::io::Result<()> {
    if suggestions.is_empty() {
        writeln!(
            writer,
            "{}",
            "✅ Your dependencies are already blessed! Nothing to modernize.".green()
        )?;
        return Ok(());
    }

    writeln!(
        writer,
        "{}",
        format!("🚀 Modernization report for {} v{}", project_name, version).bold()
    )?;
    writeln!(writer)?;

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
        writeln!(
            writer,
            " {} {} {} → {} ({})",
            icon,
            impact_tag,
            suggestion.current.yellow(),
            suggestion.recommended.green(),
            suggestion.reason.dimmed()
        )?;

        // Enrich with live intel if available
        // For combo rules like "reqwest+serde_json", check each crate name
        let crate_names: Vec<&str> = suggestion.current.split('+').collect();
        for crate_name in crate_names {
            if let Some(info) = intel.get(crate_name) {
                let mut enrichment = format!("   latest: v{}", info.latest_version);
                if let Some(recent) = info.recent_downloads {
                    enrichment.push_str(&format!(", {} recent downloads", format_downloads(recent)));
                }
                writeln!(writer, "   {}", enrichment.dimmed())?;
            }
        }
    }

    let high_count = suggestions
        .iter()
        .filter(|s| matches!(s.impact, Impact::High))
        .count();

    writeln!(writer)?;
    writeln!(
        writer,
        "{}",
        format!(
            "{} high-impact upgrade{} available. Run `cargo bless --fix` to apply safely.",
            high_count,
            if high_count == 1 { "" } else { "s" }
        )
        .bold()
    )?;

    Ok(())
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
    use std::collections::HashMap;

    #[test]
    fn test_format_downloads() {
        assert_eq!(format_downloads(0), "0");
        assert_eq!(format_downloads(500), "500");
        assert_eq!(format_downloads(1_500), "1.5K");
        assert_eq!(format_downloads(1_200_000), "1.2M");
        assert_eq!(format_downloads(100_000_000), "100.0M");
    }

    #[test]
    fn test_render_report_to_empty() {
        let mut buf = Vec::new();
        let intel = HashMap::new();
        render_report_to(&mut buf, "my_project", "1.0.0", &[], &intel).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("✅ Your dependencies are already blessed! Nothing to modernize."));
        assert!(!output.contains("🚀 Modernization report"));
    }

    #[test]
    fn test_render_report_to_with_suggestions_no_intel() {
        let mut buf = Vec::new();
        let intel = HashMap::new();
        let suggestions = vec![
            Suggestion {
                kind: SuggestionKind::ModernAlternative,
                current: "structopt".to_string(),
                recommended: "clap".to_string(),
                reason: "structopt is unmaintained".to_string(),
                source: "test".to_string(),
                impact: Impact::Medium,
            },
            Suggestion {
                kind: SuggestionKind::Unmaintained,
                current: "memmap".to_string(),
                recommended: "memmap2".to_string(),
                reason: "memmap is unmaintained".to_string(),
                source: "test".to_string(),
                impact: Impact::High,
            },
        ];

        render_report_to(&mut buf, "my_project", "1.0.0", &suggestions, &intel).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("🚀 Modernization report for my_project v1.0.0"));
        assert!(output.contains("structopt"));
        assert!(output.contains("clap"));
        assert!(output.contains("memmap"));
        assert!(output.contains("memmap2"));
        // 1 high-impact upgrade
        assert!(output.contains("1 high-impact upgrade available. Run `cargo bless --fix` to apply safely."));
    }

    #[test]
    fn test_render_report_to_with_intel() {
        let mut buf = Vec::new();
        let mut intel = HashMap::new();
        intel.insert("reqwest".to_string(), CrateIntel {
            name: "reqwest".to_string(),
            latest_version: "0.11.20".to_string(),
            downloads: 100000000,
            recent_downloads: Some(2500000),
            last_updated: "2023-01-01T00:00:00Z".to_string(),
            repository_url: None,
            description: None,
        });

        let suggestions = vec![
            Suggestion {
                kind: SuggestionKind::FeatureOptimization,
                current: "reqwest+serde_json".to_string(),
                recommended: "reqwest with json feature".to_string(),
                reason: "combo".to_string(),
                source: "test".to_string(),
                impact: Impact::Low,
            },
        ];

        render_report_to(&mut buf, "my_project", "1.0.0", &suggestions, &intel).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("reqwest+serde_json"));
        assert!(output.contains("latest: v0.11.20"));
        assert!(output.contains("2.5M recent downloads"));
        assert!(output.contains("0 high-impact upgrades available."));
    }
}
