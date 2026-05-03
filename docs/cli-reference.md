# CLI reference

Invoked as `cargo bless …` once `cargo-bless` is on your `PATH` (`cargo install cargo-bless`).

## Subcommands

| Invocation | Meaning |
|------------|---------|
| `cargo bless` | Dependency scan + optional code audit (`--audit-code`), JSON, workspace flags, autofix (`--fix`). |
| `cargo bless bs` | Code audit only (same structured flags as `CodeAuditOpts`: `--diff`, `--json`, `--manifest-path`, `--policy`, `--verbose`). |
| `cargo bless … bs …` | Nested `BlessSubcommand::Bs` — audit-only shortcut. |

## Paste-friendly vs machine output

| Flag | Shows | Typical use |
|------|-------|--------------|
| *(default)* | Full report + optional live intel | Day-to-day triage |
| `--feedback` | Version, dep totals, suggestion counts, code-audit hotspot list | Paste into GitHub issues — **never** emits your graph’s crate listing or full suggestion rationale |
| `--summary` | Per-member dep counts + impact tallies + short pattern bullets | Paste into chats (skips intel fetch entirely) |
| `--json` | Structured JSON (**`cargo_bless_version`**, **`workspace_scan`**, **`packages[]`**, **`code_audit`**) | Scripts / CI |

**Mutual exclusion:** **`--feedback`**, **`--summary`**, **`--json`**, **`--fix`**, **`--audit-code`** combinations are constrained as in `cargo bless --help` (e.g. **`--feedback`** forbids **`--workspace`/`--package`**; **`--summary`** forbids **`--audit-code`**).

## `cargo bless` dependency flags

| Flag | Description |
|------|-------------|
| `--manifest-path=PATH` | Workspace root or package `Cargo.toml` (defaults to cwd). Virtual `[workspace]` roots work with **`--workspace`**. |
| `--offline` | Skip crates.io / GitHub intelligence fetches (`bless.toml` **`[settings].offline`** also forces offline). Rules + caches still apply. |
| `--json` | Print unified JSON; cannot combine with **`--fix`**, **`--update-rules`**, **`--feedback`**, **`--summary`**. |
| `--fix` | Apply **Cargo.toml-only** autofixes to each selected member manifest (writes **`Cargo.toml.toml.bak`**, runs **`cargo update`** when not **`--dry-run`**). Never touches **`.rs`**. |
| `--dry-run` | With **`--fix`**, print diff/plan without writing files. Requires **`--fix`**. |
| `--feedback` | Privacy-minded aggregate feedback block (**root crate only**). |
| `--summary` | Short dependency summary (**no intel**). Cannot combine with **`--audit-code`**. |
| `--fail-on=L[,…]` | Exit **non-zero** if any retained suggestion’s **`impact`** is in **`{low, medium, high, critical}`**. **`critical`** is currently an alias for **high** (dependency tier only). |
| `--workspace` | Analyze **every** `workspace.members` entry once. |
| `--package=P[,…]` | Restrict to workspace members by **`[package].name`** (comma-separated). |
| `--audit-code` | Run the detector on each scanned member manifest and merge results (sums file counts / alerts). |
| `--update-rules` | Refresh blessed-derived rules in the cache. |
| `--policy=PATH` | Explicit **`bless.toml`** (otherwise auto-discovered next to the manifest directory). |
| `--verbose` | Dump every bullshit finding (text mode).

### Planned / stubbed flags

Declared but still rejected at runtime (see **`--help`**): **`--all-targets`**, **`--llm`**.

Hidden compatibility noop: **`--no-audit-code`**.

### JSON shape (**0.2+**)

Top-level fields:

- **`cargo_bless_version`**: string (`env!("CARGO_PKG_VERSION")` of the binary).
- **`workspace_scan`**: `true` when **`--workspace`** was used or multiple members were analyzed.
- **`packages`**: array of `{ name, version, manifest_path, dependency_suggestions }` where **`dependency_suggestions`** is the list of **`Suggestion`** structs (optional JSON field **`package`** names the workspace member when present).
- **`code_audit`**: `null` unless **`--audit-code`** (bless) populated the merged report, or **`cargo bless bs --json`** with code-audit output.

`cargo bless bs --json` uses **`packages: []`** plus **`code_audit`**.

### `cargo bless bs` flags

Same as **`CodeAuditOpts`** above: **`--manifest-path`**, **`--policy`**, **`--json`**, **`--diff`**, **`--verbose`**.

## Policy file (`bless.toml`)

See repository README → **Policy File** section. Applies to both modernization filtering and bullshit detector ignores.
