//! Standalone binary for CI: fetches blessed.rs rules and merges them
//! into data/suggestions.json, preserving hand-crafted rules.
//!
//! Usage: cargo run --bin update-suggestions

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

fn main() -> Result<()> {
    println!("Fetching blessed.rs rules...");

    // Fetch live rules via the updater module
    let blessed_rules =
        cargo_bless::updater::update_rules().context("failed to fetch blessed.rs rules")?;

    // Load existing hand-crafted rules from data/suggestions.json
    let data_path = Path::new("data/suggestions.json");
    let existing: Vec<cargo_bless::suggestions::Rule> = if data_path.exists() {
        let json = fs::read_to_string(data_path).context("failed to read data/suggestions.json")?;
        serde_json::from_str(&json).context("failed to parse data/suggestions.json")?
    } else {
        Vec::new()
    };

    // Merge: handcrafted rules stay authoritative; blessed.rs fills in only
    // patterns we do not already define locally (avoid clobbering tuned copy).
    let hand_patterns: HashSet<String> = existing.iter().map(|r| r.pattern.clone()).collect();

    let mut merged = existing;
    let mut from_blessed = 0usize;
    for rule in blessed_rules {
        if !hand_patterns.contains(&rule.pattern) {
            merged.push(rule);
            from_blessed += 1;
        }
    }

    println!(
        "Merged: {} hand-authored + {} new from blessed.rs = {} total",
        merged.len() - from_blessed,
        from_blessed,
        merged.len()
    );

    // Write back
    let json = serde_json::to_string_pretty(&merged).context("failed to serialize merged rules")?;
    fs::write(data_path, format!("{json}\n")).context("failed to write data/suggestions.json")?;

    println!("Wrote {}", data_path.display());
    Ok(())
}
