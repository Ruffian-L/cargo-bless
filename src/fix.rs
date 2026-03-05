//! Fix layer — applies auto-fixable suggestions by editing Cargo.toml
//! using `toml_edit` to preserve comments and formatting.
//!
//! Safety guardrails:
//! - `.bak` backup before any writes  
//! - `--dry-run` previews the diff without touching files
//! - Only direct dependency edits (never transitive)
//! - Only auto-fixable suggestion types (StdReplacement, Unmaintained)

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use colored::*;
use toml_edit::DocumentMut;

use crate::suggestions::{Suggestion, SuggestionKind};

/// Result summary of a fix operation.
pub struct FixResult {
    pub applied: Vec<String>,
    pub skipped: Vec<String>,
}

/// Apply auto-fixable suggestions to the Cargo.toml at `manifest_path`.
///
/// - Only processes suggestions where `is_auto_fixable()` is true.
/// - Creates a `.bak` backup before any edits.
/// - Uses `toml_edit` to preserve comments and formatting.
/// - If `dry_run` is true, prints the diff but writes nothing.
pub fn apply(
    suggestions: &[Suggestion],
    manifest_path: &Path,
    dry_run: bool,
) -> Result<FixResult> {
    let fixable: Vec<&Suggestion> = suggestions.iter().filter(|s| s.is_auto_fixable()).collect();

    if fixable.is_empty() {
        println!(
            "{}",
            "ℹ️  No auto-fixable suggestions found. Manual changes recommended above."
                .dimmed()
        );
        return Ok(FixResult {
            applied: vec![],
            skipped: suggestions.iter().map(|s| s.current.clone()).collect(),
        });
    }

    let original = fs::read_to_string(manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;

    let mut doc: DocumentMut = original
        .parse()
        .with_context(|| format!("failed to parse {} as TOML", manifest_path.display()))?;

    let mut applied = Vec::new();
    let mut skipped = Vec::new();

    for suggestion in &fixable {
        match apply_single(&mut doc, suggestion) {
            Ok(desc) => applied.push(desc),
            Err(e) => {
                skipped.push(format!("{}: {}", suggestion.current, e));
            }
        }
    }

    // Also note non-fixable suggestions as skipped
    for suggestion in suggestions {
        if !suggestion.is_auto_fixable() {
            skipped.push(format!("{} (requires source code changes)", suggestion.current));
        }
    }

    let edited = doc.to_string();

    if dry_run {
        println!("🔍 {}", "Dry-run: the following changes would be made:".bold());
        println!();
        print_diff(&original, &edited);

        if !applied.is_empty() {
            println!();
            println!("{}", "Changes that would be applied:".bold());
            for desc in &applied {
                println!("  {} {}", "✓".green(), desc);
            }
        }

        if !skipped.is_empty() {
            println!();
            println!("{}", "Skipped (manual action needed):".dimmed());
            for desc in &skipped {
                println!("  {} {}", "–".dimmed(), desc.dimmed());
            }
        }
    } else {
        // Create backup
        let backup_path = manifest_path.with_extension("toml.bak");
        fs::copy(manifest_path, &backup_path).with_context(|| {
            format!(
                "failed to create backup at {}",
                backup_path.display()
            )
        })?;
        println!(
            "📋 Backup saved to {}",
            backup_path.display().to_string().dimmed()
        );

        // Write edited TOML
        fs::write(manifest_path, &edited)
            .with_context(|| format!("failed to write {}", manifest_path.display()))?;

        // Run cargo update
        println!("{}", "📦 Running cargo update...".dimmed());
        let status = Command::new("cargo")
            .arg("update")
            .current_dir(
                manifest_path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or_else(|| Path::new(".")),
            )
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("{}", "✅ cargo update completed successfully.".green());
            }
            Ok(s) => {
                println!(
                    "{}",
                    format!("⚠️  cargo update exited with: {}", s).yellow()
                );
            }
            Err(e) => {
                println!(
                    "{}",
                    format!("⚠️  Failed to run cargo update: {}", e).yellow()
                );
            }
        }

        println!();
        if !applied.is_empty() {
            println!("{}", "Applied fixes:".bold().green());
            for desc in &applied {
                println!("  {} {}", "✓".green(), desc);
            }
        }

        if !skipped.is_empty() {
            println!();
            println!("{}", "Skipped (manual action needed):".dimmed());
            for desc in &skipped {
                println!("  {} {}", "–".dimmed(), desc.dimmed());
            }
        }
    }

    Ok(FixResult { applied, skipped })
}

/// Apply a single suggestion to the TOML document.
/// Returns a description of what was done on success.
fn apply_single(doc: &mut DocumentMut, suggestion: &Suggestion) -> Result<String> {
    match suggestion.kind {
        SuggestionKind::StdReplacement => apply_remove(doc, &suggestion.current, &suggestion.recommended),
        SuggestionKind::Unmaintained => apply_rename(doc, &suggestion.current, &suggestion.recommended),
        // FeatureOptimization was removed from being auto-fixable because it breaks projects
        // that still directly use the removed crate elsewhere.
        _ => anyhow::bail!("not auto-fixable"),
    }
}

/// Remove a dependency (StdReplacement: crate replaced by std).
fn apply_remove(doc: &mut DocumentMut, crate_name: &str, replacement: &str) -> Result<String> {
    let deps = doc
        .get_mut("dependencies")
        .and_then(|d| d.as_table_like_mut())
        .ok_or_else(|| anyhow::anyhow!("no [dependencies] table found"))?;

    if deps.remove(crate_name).is_some() {
        Ok(format!(
            "Removed `{}` (use {} instead)",
            crate_name, replacement
        ))
    } else {
        anyhow::bail!("`{}` not found in [dependencies]", crate_name)
    }
}

/// Rename a dependency (Unmaintained: swap to maintained fork).
fn apply_rename(doc: &mut DocumentMut, old_name: &str, new_name: &str) -> Result<String> {
    let deps = doc
        .get_mut("dependencies")
        .and_then(|d| d.as_table_like_mut())
        .ok_or_else(|| anyhow::anyhow!("no [dependencies] table found"))?;

    // Get the old entry's value
    let old_item = deps
        .remove(old_name)
        .ok_or_else(|| anyhow::anyhow!("`{}` not found in [dependencies]", old_name))?;

    // Insert with new name, preserving the version/features config
    deps.insert(new_name, old_item);

    Ok(format!("Renamed `{}` → `{}`", old_name, new_name))
}

/// Print a simple line-by-line diff between old and new content.
fn print_diff(old: &str, new: &str) {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    // Simple diff: show removed and added lines
    let mut shown_header = false;

    for line in &old_lines {
        if !new_lines.contains(line) {
            if !shown_header {
                println!("{}", "--- Cargo.toml (original)".dimmed());
                println!("{}", "+++ Cargo.toml (modified)".dimmed());
                println!();
                shown_header = true;
            }
            println!("{}", format!("- {}", line).red());
        }
    }

    for line in &new_lines {
        if !old_lines.contains(line) {
            if !shown_header {
                println!("{}", "--- Cargo.toml (original)".dimmed());
                println!("{}", "+++ Cargo.toml (modified)".dimmed());
                println!();
                shown_header = true;
            }
            println!("{}", format!("+ {}", line).green());
        }
    }

    if !shown_header {
        println!("{}", "  (no changes)".dimmed());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::suggestions::{Impact, SuggestionKind};
    use tempfile::TempDir;

    fn make_suggestion(kind: SuggestionKind, current: &str, recommended: &str) -> Suggestion {
        Suggestion {
            kind: kind.clone(),
            current: current.into(),
            recommended: recommended.into(),
            reason: "test reason".into(),
            source: "test".into(),
            impact: match kind {
                SuggestionKind::Unmaintained | SuggestionKind::StdReplacement => Impact::High,
                SuggestionKind::ModernAlternative | SuggestionKind::ComboWin => Impact::Medium,
                SuggestionKind::FeatureOptimization => Impact::Low,
            },
        }
    }

    #[test]
    fn test_remove_dep() {
        let toml = r#"
[package]
name = "test-project"
version = "0.1.0"

[dependencies]
lazy_static = "1.5"
serde = "1.0"
"#;
        let mut doc: DocumentMut = toml.parse().unwrap();
        let result = apply_remove(&mut doc, "lazy_static", "std::sync::LazyLock").unwrap();

        assert!(result.contains("Removed `lazy_static`"));
        let edited = doc.to_string();
        assert!(!edited.contains("lazy_static"));
        assert!(edited.contains("serde")); // other deps untouched
    }

    #[test]
    fn test_rename_dep() {
        let toml = r#"
[package]
name = "test-project"
version = "0.1.0"

[dependencies]
memmap = "0.7"
serde = "1.0"
"#;
        let mut doc: DocumentMut = toml.parse().unwrap();
        let result = apply_rename(&mut doc, "memmap", "memmap2").unwrap();

        assert!(result.contains("Renamed `memmap` → `memmap2`"));
        let edited = doc.to_string();
        assert!(!edited.contains("memmap ="));
        assert!(edited.contains("memmap2"));
        assert!(edited.contains("serde")); // other deps untouched
    }

    #[test]
    fn test_remove_nonexistent_dep() {
        let toml = r#"
[package]
name = "test-project"

[dependencies]
serde = "1.0"
"#;
        let mut doc: DocumentMut = toml.parse().unwrap();
        let result = apply_remove(&mut doc, "nonexistent", "something");
        assert!(result.is_err());
    }

    #[test]
    fn test_dry_run_does_not_write() {
        let tmp = TempDir::new().unwrap();
        let manifest = tmp.path().join("Cargo.toml");
        let toml_content = r#"
[package]
name = "test-project"
version = "0.1.0"

[dependencies]
lazy_static = "1.5"
"#;
        fs::write(&manifest, toml_content).unwrap();

        let suggestions = vec![make_suggestion(
            SuggestionKind::StdReplacement,
            "lazy_static",
            "std::sync::LazyLock",
        )];

        let result = apply(&suggestions, &manifest, true).unwrap();
        assert_eq!(result.applied.len(), 1);

        // File should be unchanged
        let after = fs::read_to_string(&manifest).unwrap();
        assert_eq!(after, toml_content);

        // No backup should exist
        assert!(!tmp.path().join("Cargo.toml.bak").exists());
    }

    #[test]
    fn test_full_apply_creates_backup() {
        let tmp = TempDir::new().unwrap();
        let manifest = tmp.path().join("Cargo.toml");
        let toml_content = r#"[package]
name = "test-project"
version = "0.1.0"

[dependencies]
lazy_static = "1.5"
serde = "1.0"
"#;
        fs::write(&manifest, toml_content).unwrap();

        let suggestions = vec![make_suggestion(
            SuggestionKind::StdReplacement,
            "lazy_static",
            "std::sync::LazyLock",
        )];

        let result = apply(&suggestions, &manifest, false).unwrap();
        assert_eq!(result.applied.len(), 1);

        // Backup should exist with original content
        let backup = tmp.path().join("Cargo.toml.bak");
        assert!(backup.exists());
        let backup_content = fs::read_to_string(&backup).unwrap();
        assert_eq!(backup_content, toml_content);

        // File should be modified
        let after = fs::read_to_string(&manifest).unwrap();
        assert!(!after.contains("lazy_static"));
        assert!(after.contains("serde")); // untouched
    }

    #[test]
    fn test_no_fixable_suggestions() {
        let tmp = TempDir::new().unwrap();
        let manifest = tmp.path().join("Cargo.toml");
        fs::write(&manifest, "[package]\nname = \"test\"\n[dependencies]\n").unwrap();

        let suggestions = vec![make_suggestion(
            SuggestionKind::ModernAlternative,
            "structopt",
            "clap v4",
        )];

        let result = apply(&suggestions, &manifest, true).unwrap();
        assert!(result.applied.is_empty());
        assert_eq!(result.skipped.len(), 1);
    }
}
