# Phase 3: Workspace Support Architecture

Design notes for a future **`--workspace` / `--package`** release (targeted at **v0.2.x**).

**See also:** [Documentation index](./README.md) · [Architecture overview](./architecture.md) · [Root README](https://github.com/Ruffian-L/cargo-bless#readme)

---

## Problem

cargo-bless currently analyzes one Cargo.toml at a time. Real Rust projects use workspaces with multiple packages, each with their own dependency trees. A single `cargo bless` run should be able to audit the entire workspace.

## Goals

1. `--workspace` flag: analyze all workspace members and aggregate results
2. `--package NAME`: analyze only specific workspace member(s)
3. Per-package suggestion output so users know which package has which issue
4. Fix mode that can target individual member manifests

## Non-Goals (Phase 3)

- Virtual manifest editing (workspace-level `[dependencies]`) — too complex, low value
- Cross-package deduplication analysis — Phase 4 material

---

## Design

### New Struct: `PackageResult`

```rust
// src/parser.rs — new public struct

/// Analysis result for one workspace member.
pub struct PackageResult {
    /// Package name (from Cargo.toml [package].name)
    pub name: String,
    /// Package version
    pub version: String,
    /// Path to this package's Cargo.toml
    pub manifest_path: std::path::PathBuf,
    /// Resolved dependencies for this package
    pub deps: Vec<ResolvedDep>,
    /// Suggestions specific to this package
    pub suggestions: Vec<Suggestion>,
}
```

### New Function: `get_workspace_deps`

```rust
// src/parser.rs — new public function

/// Parse all workspace members and return per-package results.
///
/// If `package_filter` is Some, only analyze matching packages.
/// If the project is not a workspace (single package), returns one entry.
pub fn get_workspace_deps(
    manifest_path: Option<&Path>,
    package_filter: Option<&[String]>,
) -> Result<Vec<PackageResult>> { ... }
```

#### Algorithm

1. Run `cargo metadata` once for the entire workspace (same as current `get_deps`)
2. Call `metadata.workspace_packages()` to get all member packages
3. For each member package:
   a. Build its own resolve tree using the shared `Resolve` from step 1
   b. Compute direct vs transitive deps relative to THIS member's root node
   c. Run suggestion analysis against this member's deps
4. Return vector of `PackageResult`, one per member

#### Key Insight

`cargo metadata` returns ONE `Resolve` covering the entire workspace. Each workspace member has its own entry in `resolve.nodes`. The "direct dependencies" for a member are the nodes listed in that member's `Node.deps`. We reuse the same `pkg_map` and `node_map` across all members — no repeated metadata calls.

### Changes to `get_deps`

Current `get_deps` becomes an internal helper renamed to `resolve_deps_for_package`:

```rust
/// Resolve dependencies for ONE package given shared metadata structures.
fn resolve_deps_for_package(
    pkg: &Package,
    node_map: &HashMap<String, Node>,
    pkg_map: &HashMap<String, Package>,
) -> Vec<ResolvedDep> { ... }
```

### Changes to `analyze` in suggestions.rs

Add an overload that takes a package name for context:

```rust
/// Analyze deps with package context (for workspace output).
pub fn analyze_with_context(
    manifest_path: Option<&Path>,
    deps: &[ResolvedDep],
    rules: &[Rule],
    package_name: &str,
) -> Vec<Suggestion> { ... }
```

This is identical to `analyze` but attaches `package_name` to each suggestion for display purposes. (May require adding an optional `package: Option<String>` field to `Suggestion`.)

### Changes to main.rs Pipeline

```rust
// Pseudocode for the new pipeline

let results = if opts.workspace {
    // Analyze all workspace members
    let results = cargo_bless::parser::get_workspace_deps(manifest, None)?;
    
    // Load policy once
    let policy = load_policy_or_default(&opts.policy);
    
    // Apply suggestions per-package
    for result in &mut results {
        let rules = cargo_bless::suggestions::load_rules();
        result.suggestions = cargo_bless::suggestions::analyze_with_context(
            Some(&result.manifest_path),
            &result.deps,
            &rules,
            &result.name,
        );
        result.suggestions = cargo_bless::policy::apply_policy(result.suggestions.clone(), &policy);
    }
    
    results
} else {
    // Single package (current behavior)
    let deps = cargo_bless::parser::get_deps(manifest)?;
    let rules = cargo_bless::suggestions::load_rules();
    let suggestions = cargo_bless::suggestions::analyze(manifest, &deps, &rules);
    
    vec![PackageResult {
        name: project_name,
        version: project_version,
        manifest_path: ...,
        deps,
        suggestions,
    }]
};

// Render per-package results
for result in &results {
    render_package_report(result, &intel, opts.json);
}
```

### Output Format Changes

**Text mode:** Add package header before each section.
```
📦 workspace-member-a v0.1.0 (Cargo.toml)
  └─ lazy_static → std::sync::LazyLock [HIGH]

📦 workspace-member-b v0.2.0 (crates/b/Cargo.toml)
  └─ memmap → memmap2 [HIGH]
```

**JSON mode:** Wrap in array with package context.
```json
{
  "workspace": {
    "members": [
      {
        "name": "member-a",
        "version": "0.1.0",
        "manifest_path": "Cargo.toml",
        "suggestions": [...]
      }
    ]
  }
}
```

---

## Fix Mode for Workspaces

When `--fix` is used with `--workspace`:
1. Iterate each package result
2. Apply fixable suggestions to that member's Cargo.toml
3. Create per-package backups (e.g., `crates/b/Cargo.toml.bak`)
4. Run `cargo update` once at the workspace root

## Risks and Mitigations

| Risk | Mitigation |
|------|-----------|
| Large workspaces slow down analysis | Reuse single metadata call; O(n) in total deps, not per-package |
| Shared deps counted multiple times | Accept — each package's view is independent. Dedup is Phase 4. |
| Virtual manifests have no root resolve | `workspace_packages()` handles this; skip virtual manifest entries |

## Acceptance Criteria

1. `cargo bless --workspace` on a multi-package workspace shows per-package results
2. `cargo bless --package member-a` analyzes only that member
3. `--fix --workspace` applies fixes to all affected member manifests
4. JSON output includes package context
5. Single-package projects work identically to current behavior (no regression)


## ## Implementation Notes (from ripgrep test)

### Real-World Validation

Tested against ripgrep v15.1.0 which has:
- 11 workspace members (`crates/globset`, `crates/grep`, `crates/cli`, etc.)
- 13 direct deps in root, 47 transitive
- Current cargo-bless only analyzes the ROOT package

**What Phase 3 must capture:**
- Each member has its own dependency set (e.g., `crates/cli` may have `clap` while root doesn't)
- Shared transitive deps should appear in each member's view independently
- Fix mode needs to target the correct member's Cargo.toml

### cargo_metadata API Details

```rust
// From cargo_metadata docs:
metadata.workspace_packages()  // Returns Vec<&Package> for all workspace members

// Each Package has:
pkg.name          // Package name
pkg.version       // Version string  
pkg.manifest_path // Path to this package's Cargo.toml
pkg.id            // PackageId (unique identifier)

// The Resolve contains ALL nodes across the workspace:
resolve.nodes     // Vec<Node> - every package in the dependency graph
resolve.root      // Option<PackageId> - root of the workspace (may be virtual manifest)
```

### Migration Path for get_deps()

1. Keep `get_deps()` as-is for backward compatibility (single package mode)
2. Add `get_workspace_deps()` that calls cargo_metadata once and iterates members
3. Refactor internal logic to share the metadata call between both functions

### Output Example for Workspace Mode

```
📦 workspace-member-a v0.1.0 (crates/a/Cargo.toml)
  └─ lazy_static → std::sync::LazyLock [HIGH]

📦 workspace-member-b v0.2.0 (crates/b/Cargo.toml)  
  └─ memmap → memmap2 [HIGH]

📦 workspace-root v1.0.0 (Cargo.toml)
  └─ log → tracing [MED]

Summary: 3 suggestions across 3 packages
```
