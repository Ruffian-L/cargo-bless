# CLI reference

Invoked as `cargo bless …` once `cargo-bless` is on your `PATH` (`cargo install cargo-bless`).

## Subcommands

| Invocation | Meaning |
|------------|---------|
| `cargo bless` | Dependency scan + optional code audit (`--audit-code`), JSON, workspace flags, autofix (`--fix`). |
| `cargo bless bs` | Code audit only (`--diff`, `--fix`, `--json`, `--sarif`, `--manifest-path`, `--policy`, `--verbose`). |
| `cargo bless … bs …` | Nested `BlessSubcommand::Bs` — audit-only shortcut. |

## Paste-friendly vs machine output

| Flag | Shows | Typical use |
|------|-------|--------------|
| *(default)* | Full report + optional live intel | Day-to-day triage |
| `--feedback` | Version, dep totals, suggestion counts, code-audit hotspot list | Paste into GitHub issues — **never** emits your graph's crate listing or full suggestion rationale |
| `--summary` | Per-member dep counts + impact tallies + short pattern bullets | Paste into chats (skips intel fetch entirely) |
| `--json` | Structured JSON (**`cargo_bless_version`**, **`workspace_scan`**, **`packages[]`**, **`code_audit`**, **`security_advisories`**) | Scripts / CI |

**Mutual exclusion:** **`--feedback`**, **`--summary`**, **`--json`**, **`--fix`**, **`--audit-code`** combinations are constrained as in `cargo bless --help` (e.g. **`--feedback`** forbids **`--workspace`/`--package`**; **`--summary`** forbids **`--audit-code`**).

## `cargo bless` flags

| Flag | Description |
|------|-------------|
| `--manifest-path=PATH` | Workspace root or package `Cargo.toml` (defaults to cwd). Virtual `[workspace]` roots work with **`--workspace`**. |
| `--offline` | Skip crates.io / GitHub / osv.dev intelligence fetches. Rules + caches still apply. Also settable via `bless.toml` `[settings].offline`. |
| `--no-advisories` | Skip the osv.dev advisory check even when online. |
| `--json` | Print unified JSON; cannot combine with **`--fix`**, **`--update-rules`**, **`--feedback`**, **`--summary`**. |
| `--fix` | Apply **Cargo.toml-only** autofixes to each selected member manifest (writes **`Cargo.toml.bak`**, runs **`cargo update`** when not **`--dry-run`**). Never touches **`.rs`**. |
| `--dry-run` | With **`--fix`**, print diff/plan without writing files. Requires **`--fix`**. |
| `--feedback` | Privacy-minded aggregate feedback block (**root crate only**). |
| `--summary` | Short dependency summary (**no intel**). Cannot combine with **`--audit-code`**. |
| `--fail-on=L[,…]` | Exit **non-zero** if any retained suggestion's **`impact`** is in **`{low, medium, high, critical}`**. **`critical`** is currently an alias for **high** (dependency tier only). |
| `--workspace` | Analyze **every** `workspace.members` entry once. |
| `--package=P[,…]` | Restrict to workspace members by **`[package].name`** (comma-separated). |
| `--audit-code` | Run the detector on each scanned member manifest and merge results (sums file counts / alerts). |
| `--all-targets` | Include `[dev-dependencies]` and `[build-dependencies]` in analysis (default: normal deps only). Also settable via `bless.toml` `[settings].all_targets = true`. |
| `--update-rules` | Refresh blessed-derived rules in the cache. |
| `--init-ci` | Write a starter GitHub Actions workflow to `.github/workflows/bless.yml` and exit. |
| `--init-hooks` | Write `.git/hooks/pre-commit` that runs `cargo bless bs --fail-on-confidence 0.8` before each commit, and exit. |
| `--explain=PATTERN` | Show full details for a suggestion rule — kind, confidence, migration risk, reason, source. Fuzzy-matches on pattern name. Exits non-zero if no rule is found. |
| `--policy=PATH` | Explicit **`bless.toml`** (otherwise auto-discovered next to the manifest directory). |
| `--verbose` | Dump every bullshit finding (text mode). |

### JSON shape (0.3+)

Top-level fields:

- **`cargo_bless_version`**: string (`env!("CARGO_PKG_VERSION")` of the binary).
- **`workspace_scan`**: `true` when **`--workspace`** was used or multiple members were analyzed.
- **`packages`**: array of `{ name, version, manifest_path, dependency_suggestions }` where **`dependency_suggestions`** is the list of **`Suggestion`** structs.
- **`code_audit`**: `null` unless **`--audit-code`** (bless) populated the merged report, or **`cargo bless bs --json`** with code-audit output.
- **`security_advisories`**: array of `{ crate_name, advisories: [{ id, summary, aliases }] }`. Omitted from JSON output when empty.

`cargo bless bs --json` uses **`packages: []`** plus **`code_audit`**.

## `cargo bless bs` flags

| Flag | Description |
|------|-------------|
| `--manifest-path=PATH` | Path to the `Cargo.toml` whose source tree should be audited. |
| `--policy=PATH` | Explicit `bless.toml` for code-audit suppressions. |
| `--json` | Output findings as unified JSON (`code_audit` key). |
| `--sarif` | Output findings as SARIF 2.1.0 JSON (for `upload-sarif` in GitHub Actions). |
| `--diff` | Audit only lines changed in `git diff HEAD`. |
| `--fix` | Auto-replace `.unwrap()` → `.expect("TODO: handle this")` across all flagged files. Writes `*.rs.bak` backups. |
| `--dry-run` | With **`--fix`**, preview what would change without writing any files. |
| `--include-tests` | Also scan `tests/`, `examples/`, and `benches/` (default: `src/` only). |
| `--hardcoded` | Also scan for hardcoded values: magic numbers, API keys, IPs, URLs, credentials. |
| `--fail-on-confidence=FLOAT` | Exit non-zero if any finding has confidence ≥ this value (0.0–1.0). |
| `--verbose` | Show every finding instead of a concise summary. |

## Policy file (`bless.toml`)

See repository README → **Policy File** section. Applies to both modernization filtering and bullshit detector ignores.

Relevant `[code_audit]` fields:

```toml
[code_audit]
ignore_paths = ["src/generated"]
ignore_kinds = ["UnwrapAbuse"]
include_tests = true   # equivalent to --include-tests on the CLI
```
