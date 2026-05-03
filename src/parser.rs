//! Parser layer — extracts the full dependency tree from Cargo.toml / Cargo.lock
//! using `cargo_metadata` for feature-aware resolution.

use anyhow::{bail, Result};
use cargo_metadata::{CargoOpt, DependencyKind, MetadataCommand, Node, Package, PackageId, Resolve};
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

/// One Cargo package in the workspace (or the resolved root crate) plus its dependency tree.
///
/// Produced before suggestion analysis runs; see Phase 3 workspace design (`docs/`).
#[derive(Debug, Clone)]
pub struct PackageResult {
    pub name: String,
    pub version: String,
    pub manifest_path: PathBuf,
    pub deps: Vec<ResolvedDep>,
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

/// Parse the dependency tree for the project at `manifest_path` (resolved root crate only).
///
/// **Key change**: uses `resolve.nodes[].features` for the **actual enabled features**
/// rather than `pkg.features.keys()` which only lists declared/available features.
pub fn get_deps(manifest_path: Option<&Path>) -> Result<Vec<ResolvedDep>> {
    let snapshots =
        fetch_metadata_and_snapshots(manifest_path, SnapshotMode::RootOnly, false)?;
    snapshots
        .into_iter()
        .next()
        .map(|p| p.deps)
        .ok_or_else(|| anyhow::anyhow!("No dependency snapshot produced"))
}

/// Load dependency snapshots for workspace members according to Cargo metadata.
///
/// - **Root-only**: `workspace_all_members` false and `package_filters` empty — only `[package]` at the resolved root (`resolve.root`).
/// - **All members**: `workspace_all_members` true — every entry in `metadata.workspace_members`.
/// - **Filtered**: non-empty `package_filters` — members whose names match case-insensitively (comma-separated CLI values become one slice).
///
/// `all_targets` widens what counts as a "direct" dep to include `[dev-dependencies]` and
/// `[build-dependencies]` in addition to normal `[dependencies]`.
pub fn get_package_snapshots(
    manifest_path: Option<&Path>,
    workspace_all_members: bool,
    package_filters: &[String],
    all_targets: bool,
) -> Result<Vec<PackageResult>> {
    fetch_metadata_and_snapshots(
        manifest_path,
        SnapshotMode::Custom {
            workspace_all_members,
            filters: package_filters,
        },
        all_targets,
    )
}

#[derive(Clone, Copy, Debug)]
enum SnapshotMode<'a> {
    RootOnly,
    Custom {
        workspace_all_members: bool,
        filters: &'a [String],
    },
}

fn fetch_metadata_and_snapshots(
    manifest_path: Option<&Path>,
    mode: SnapshotMode<'_>,
    all_targets: bool,
) -> Result<Vec<PackageResult>> {
    let mut cmd = metadata_command(manifest_path);
    cmd.features(CargoOpt::AllFeatures);

    let metadata = cmd.exec()?;
    let resolve = metadata
        .resolve
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No dependency resolution found"))?;

    let node_map = build_node_lookup(resolve);
    let pkg_map = build_pkg_lookup(&metadata);

    let workspace_packages_ordered: Vec<&Package> = metadata
        .workspace_members
        .iter()
        .filter_map(|id| metadata.packages.iter().find(|p| &p.id == id))
        .collect();

    let resolve_workspace_root_pkg = || -> Result<&Package> {
        let root_id = resolve
            .root
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No root package in resolve"))?;
        pkg_map
            .get(&root_id.to_string())
            .ok_or_else(|| anyhow::anyhow!("Root package not in cargo metadata packages list"))
    };

    let targets = match mode {
        SnapshotMode::RootOnly => vec![resolve_workspace_root_pkg()?],
        SnapshotMode::Custom {
            workspace_all_members,
            filters,
        } => {
            if workspace_all_members && filters.is_empty() {
                workspace_packages_ordered
            } else if !filters.is_empty() {
                let wanted_set: HashSet<String> = filters
                    .iter()
                    .map(|s| s.trim().to_ascii_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect();

                let mut matched: Vec<&Package> = workspace_packages_ordered
                    .iter()
                    .copied()
                    .filter(|p| wanted_set.contains(&p.name.to_ascii_lowercase()))
                    .collect();

                matched.sort_by(|a, b| a.name.cmp(&b.name));

                let mut missing = Vec::new();
                for w in &wanted_set {
                    if !matched
                        .iter()
                        .any(|p| p.name.eq_ignore_ascii_case(w.as_str()))
                    {
                        missing.push(w.clone());
                    }
                }

                if !missing.is_empty() {
                    bail!(
                        "no workspace package(s) matching {:?} — available: {}",
                        missing,
                        workspace_packages_ordered
                            .iter()
                            .map(|p| p.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }

                matched
            } else {
                vec![resolve_workspace_root_pkg()?]
            }
        }
    };

    let mut out = Vec::with_capacity(targets.len());
    for pkg in targets {
        let deps = resolve_deps_for_root(&pkg.id, resolve, &pkg_map, &node_map, all_targets)?;
        out.push(PackageResult {
            name: pkg.name.to_string(),
            version: pkg.version.to_string(),
            manifest_path: PathBuf::from(pkg.manifest_path.as_str()),
            deps,
        });
    }

    Ok(out)
}

/// Resolve flattened dependency list relative to `root_pkg_id`'s subtree.
///
/// When `all_targets` is false (the default), only `[dependencies]` entries are treated as
/// direct. When true, `[dev-dependencies]` and `[build-dependencies]` are also considered direct.
fn resolve_deps_for_root(
    root_pkg_id: &PackageId,
    resolve: &Resolve,
    pkg_map: &HashMap<String, Package>,
    node_map: &HashMap<String, Node>,
    all_targets: bool,
) -> Result<Vec<ResolvedDep>> {
    let root_node = node_map
        .get(&root_pkg_id.to_string())
        .ok_or_else(|| anyhow::anyhow!("Root node missing in resolve.nodes"))?;

    let direct_dep_ids: HashSet<String> = root_node
        .deps
        .iter()
        .filter(|d| {
            all_targets
                || d.dep_kinds
                    .iter()
                    .any(|k| k.kind == DependencyKind::Normal)
        })
        .map(|d| d.pkg.to_string())
        .collect();

    let mut deps = Vec::new();

    for node in &resolve.nodes {
        if node.id == *root_pkg_id {
            continue;
        }

        let pkg = match pkg_map.get(&node.id.to_string()) {
            Some(p) => p,
            None => continue,
        };

        let is_direct = direct_dep_ids.contains(&node.id.to_string());

        let enabled_features: Vec<String> = node
            .features
            .iter()
            .map(|s| s.as_str().to_string())
            .collect();

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
