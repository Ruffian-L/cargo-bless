//! Fix layer — applies suggestions by editing Cargo.toml (preserving comments)
//! and optionally running `cargo update`.

use anyhow::Result;
use std::path::Path;

use crate::suggestions::Suggestion;

/// Apply the given suggestions to the Cargo.toml at `manifest_path`.
///
/// - Creates a `.bak` backup before any edits.
/// - Uses `toml_edit` to preserve comments and formatting.
/// - If `dry_run` is true, prints the diff but writes nothing.
pub fn apply(
    _suggestions: &[Suggestion],
    _manifest_path: &Path,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        // TODO: Show diff of proposed changes without writing
        println!("🔍 Dry-run: the following changes would be made:");
        todo!("Dry-run diff rendering not yet implemented");
    }

    // TODO: Backup Cargo.toml → Cargo.toml.bak
    // TODO: Parse with toml_edit::DocumentMut
    // TODO: For each suggestion, modify the [dependencies] table
    // TODO: Write edited TOML back
    // TODO: Run `cargo update` via std::process::Command

    todo!("Fix mode not yet implemented")
}
