//! Parser layer — extracts the full dependency tree from Cargo.toml / Cargo.lock
//! using `cargo_metadata` for feature-aware resolution.

use anyhow::Result;
use cargo_metadata::{CargoOpt, MetadataCommand};
use std::fmt;
use std::path::Path;

/// A resolved dependency with its name, version, and enabled features.
#[derive(Debug, Clone)]
pub struct ResolvedDep {
    pub name: String,
    pub version: String,
    pub features: Vec<String>,
    pub source: Option<String>,
    pub repository: Option<String>,
    pub is_direct: bool,
}

impl fmt::Display for ResolvedDep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tag = if self.is_direct { "direct" } else { "transitive" };
        write!(f, "{} v{} ({})", self.name, self.version, tag)?;
        if !self.features.is_empty() {
            write!(f, " [{}]", self.features.join(", "))?;
        }
        Ok(())
    }
}

/// Parse the dependency tree for the project at `manifest_path`.
/// If `manifest_path` is None, uses the current directory.
pub fn get_deps(manifest_path: Option<&Path>) -> Result<Vec<ResolvedDep>> {
    let mut cmd = MetadataCommand::new();
    cmd.features(CargoOpt::AllFeatures);

    if let Some(path) = manifest_path {
        cmd.manifest_path(path);
    }

    let metadata = cmd.exec()?;
    let resolve = metadata
        .resolve
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No dependency resolution found"))?;

    // Collect root/direct dependency names for tagging
    let direct_dep_ids: std::collections::HashSet<_> = resolve
        .root
        .as_ref()
        .and_then(|root_id| {
            resolve
                .nodes
                .iter()
                .find(|n| &n.id == root_id)
                .map(|n| n.deps.iter().map(|d| d.pkg.clone()).collect())
        })
        .unwrap_or_default();

    let mut deps = Vec::new();

    for pkg in &metadata.packages {
        // Skip the workspace root itself
        if pkg.source.is_none() {
            continue;
        }

        deps.push(ResolvedDep {
            name: pkg.name.clone(),
            version: pkg.version.to_string(),
            features: pkg.features.keys().cloned().collect(),
            source: pkg.source.as_ref().map(|s| s.repr.clone()),
            repository: pkg.repository.clone(),
            is_direct: direct_dep_ids.contains(&pkg.id),
        });
    }

    Ok(deps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolvedep_debug() {
        let dep = ResolvedDep {
            name: "serde".into(),
            version: "1.0.0".into(),
            features: vec!["derive".into()],
            source: Some("registry+https://github.com/rust-lang/crates.io-index".into()),
            repository: Some("https://github.com/serde-rs/serde".into()),
            is_direct: true,
        };
        assert!(format!("{:?}", dep).contains("serde"));
    }

    #[test]
    fn test_resolvedep_display() {
        let dep = ResolvedDep {
            name: "clap".into(),
            version: "4.5.0".into(),
            features: vec!["derive".into(), "std".into()],
            source: Some("registry+https://github.com/rust-lang/crates.io-index".into()),
            repository: None,
            is_direct: true,
        };
        let display = format!("{}", dep);
        assert!(display.contains("clap"));
        assert!(display.contains("4.5.0"));
        assert!(display.contains("direct"));
        assert!(display.contains("[derive, std]"));
    }

    #[test]
    fn test_resolvedep_display_transitive_no_features() {
        let dep = ResolvedDep {
            name: "unicode-ident".into(),
            version: "1.0.0".into(),
            features: vec![],
            source: Some("registry+https://github.com/rust-lang/crates.io-index".into()),
            repository: None,
            is_direct: false,
        };
        let display = format!("{}", dep);
        assert!(display.contains("transitive"));
        assert!(!display.contains('['));
    }

    #[test]
    fn test_get_deps_self() {
        // Parse our own Cargo.toml as a self-test
        let deps = get_deps(None).expect("should parse own project");
        assert!(!deps.is_empty(), "should find at least one dependency");

        // We know clap and serde are direct deps
        let names: Vec<&str> = deps.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"clap"), "clap should be in deps");
        assert!(names.contains(&"serde"), "serde should be in deps");

        // At least some should be marked as direct
        let direct_count = deps.iter().filter(|d| d.is_direct).count();
        assert!(direct_count > 0, "should have direct dependencies");
    }
}
