//! Parser layer — extracts the full dependency tree from Cargo.toml / Cargo.lock
//! using `cargo_metadata` for feature-aware resolution.

use anyhow::Result;
use cargo_metadata::{CargoOpt, MetadataCommand, Node, Package, Resolve};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

/// A resolved dependency with its name, version, and enabled features.
#[derive(Debug, Clone)]
pub struct ResolvedDep {
    pub name: String,
    pub version: String,
    /// Features that are **actually enabled** in the resolved build plan.
    pub enabled_features: Vec<String>,
    /// All features **declared** by the crate (available but not necessarily enabled).
    pub available_features: Vec<String>,
    pub source: Option<String>,
    pub repository: Option<String>,
    pub is_direct: bool,
}

impl fmt::Display for ResolvedDep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tag = if self.is_direct {
            "direct"
        } else {
            "transitive"
        };
        write!(f, "{} v{} ({})", self.name, self.version, tag)?;
        if !self.enabled_features.is_empty() {
            write!(f, " [enabled: {}]", self.enabled_features.join(", "))?;
        }
        if !self.available_features.is_empty() && self.available_features != self.enabled_features {
            write!(f, " (available: {})", self.available_features.join(", "))?;
        }
        Ok(())
    }
}

/// Get the root project's name and version from Cargo metadata.
pub fn get_project_info(manifest_path: Option<&Path>) -> Result<(String, String)> {
    let cmd = metadata_command(manifest_path);
    let metadata = cmd.exec()?;
    let root_id = metadata
        .resolve
        .as_ref()
        .and_then(|r| r.root.as_ref())
        .ok_or_else(|| anyhow::anyhow!("No root package found"))?;
    let root_pkg = metadata
        .packages
        .iter()
        .find(|p| &p.id == root_id)
        .ok_or_else(|| anyhow::anyhow!("Root package not in packages list"))?;
    Ok((root_pkg.name.to_string(), root_pkg.version.to_string()))
}

/// Build a lookup from package ID to the resolved Node (which carries enabled features).
fn build_node_lookup(resolve: &Resolve) -> HashMap<String, Node> {
    resolve
        .nodes
        .iter()
        .map(|n| (n.id.to_string(), n.clone()))
        .collect()
}

/// Build a map from package ID to package reference.
fn build_pkg_lookup(metadata: &cargo_metadata::Metadata) -> HashMap<String, Package> {
    metadata
        .packages
        .iter()
        .map(|p| (p.id.to_string(), p.clone()))
        .collect()
}

/// Parse the dependency tree for the project at `manifest_path`.
///
/// **Key change**: uses `resolve.nodes[].features` for the **actual enabled features**
/// rather than `pkg.features.keys()` which only lists declared/available features.
pub fn get_deps(manifest_path: Option<&Path>) -> Result<Vec<ResolvedDep>> {
    let mut cmd = metadata_command(manifest_path);
    cmd.features(CargoOpt::AllFeatures);

    let metadata = cmd.exec()?;
    let resolve = metadata
        .resolve
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No dependency resolution found"))?;

    // Build node lookup for resolved (enabled) features
    let node_map = build_node_lookup(resolve);
    let pkg_map = build_pkg_lookup(&metadata);

    // Collect root/direct dependency names for tagging
    let root_id = resolve
        .root
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No root package in resolve"))?;

    // Find the root node to get its direct dependencies
    let root_node = node_map
        .get(&root_id.to_string())
        .ok_or_else(|| anyhow::anyhow!("Root node not found in resolve nodes"))?;

    // Direct dependency IDs are those listed as deps of the root node
    let direct_dep_ids: HashSet<String> =
        root_node.deps.iter().map(|d| d.pkg.to_string()).collect();

    let mut deps = Vec::new();

    for node in &resolve.nodes {
        // Skip the root package itself
        if node.id == *root_id {
            continue;
        }

        let pkg = match pkg_map.get(&node.id.to_string()) {
            Some(p) => p,
            None => continue, // not a real crate (e.g. virtual manifest)
        };

        let is_direct = direct_dep_ids.contains(&node.id.to_string());

        // Enabled features come from the resolved node (what's actually turned on)
        let enabled_features: Vec<String> = node
            .features
            .iter()
            .map(|s| s.as_str().to_string())
            .collect();

        // Available features come from the package manifest (what's declared)
        let available_features: Vec<String> = pkg.features.keys().map(|s| s.to_string()).collect();

        deps.push(ResolvedDep {
            name: pkg.name.to_string(),
            version: pkg.version.to_string(),
            enabled_features,
            available_features,
            source: pkg.source.as_ref().map(|s| s.to_string()),
            repository: pkg.repository.clone(),
            is_direct,
        });
    }

    Ok(deps)
}

fn metadata_command(manifest_path: Option<&Path>) -> MetadataCommand {
    let mut cmd = MetadataCommand::new();

    if let Some(path) = manifest_path {
        cmd.manifest_path(path);
    }

    if lockfile_path(manifest_path).is_file() {
        cmd.other_options(vec!["--locked".to_string()]);
    }

    cmd
}

fn lockfile_path(manifest_path: Option<&Path>) -> PathBuf {
    manifest_path
        .and_then(Path::parent)
        .map(|path| path.join("Cargo.lock"))
        .unwrap_or_else(|| PathBuf::from("Cargo.lock"))
}
