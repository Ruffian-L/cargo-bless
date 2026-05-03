# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo check                          # type-check without compiling
cargo clippy -- -D warnings          # lint (CI enforces warnings as errors)
cargo test                           # all unit + integration tests
cargo test <test_name>               # run a single test by name
cargo test -- --ignored              # run live network tests (hits crates.io / GitHub)
cargo build                          # debug build
cargo install --path . --debug       # install locally (smoke test: cargo bless --help)
```

Minimum supported Rust version: **1.80** (required for `std::sync::LazyLock`). CI runs both `stable` and `1.80`.

There is also a helper binary `update-suggestions` (`src/bin/update_suggestions.rs`) for regenerating `data/suggestions.json` from blessed.rs. It is not part of the regular dev workflow.

## Architecture

`cargo-bless` is a Cargo subcommand binary (`cargo bless`). `src/main.rs` is the entry point — it parses the CLI, validates flag combinations, and orchestrates calls into the library crate (`cargo_bless`, `src/lib.rs`).

### Module responsibilities

| Module | Role |
|--------|------|
| `cli` | Clap structs — `BlessOpts`, `CodeAuditOpts`. Flag validation lives in `main.rs`, not here. |
| `parser` | Calls `cargo_metadata` to produce a resolved `Vec<ResolvedDep>` (direct vs. transitive, enabled features). Entry points: `get_deps` (single crate) and `get_package_snapshots` (workspace). |
| `suggestions` | Rule engine. Loads `data/suggestions.json` (embedded at compile time) merged with an optional cached blessed.rs update (embedded patterns take precedence). Matches direct deps only — single-crate exact match or multi-crate combo rules (`pattern = "a+b"`). |
| `policy` | Parses `bless.toml` (`[packages.<name>].suppress`, `ignore_packages`, `max_suggestions`, `[code_audit].ignore_paths/kinds`). Applied after suggestion analysis. |
| `intel` | Optional live crates.io + GitHub metadata. Non-fatal — silently skips on failure. Disk cache at `~/.cache/cargo-bless/<crate>.json` with 1-hour TTL. Skipped when `--offline` or `[settings].offline = true`. |
| `fix` | Applies `AutofixSafety::CargoTomlOnly` suggestions by editing `Cargo.toml` with `toml_edit` (comment-preserving). Writes a `.bak` backup first. Never touches `.rs` files. |
| `updater` | Fetches blessed.rs JSON and converts it to `Rule` objects, stored in the XDG cache dir (1-week TTL). Triggered by `--update-rules`. |
| `code_audit` | Static "bullshit detector" — tree-sitter + regex scan of `.rs` files for patterns like `UnwrapAbuse`, `ArcAbuse`, `SleepAbuse`, etc. `scan_project` for full tree, `scan_git_diff` for `--diff` mode (changed lines only). |
| `output` | Renders human-readable terminal output and the unified JSON schema (`JsonReportUnified`). |
| `feedback` | `--feedback` mode — aggregate stats + code-audit hotspots, no crate names, no network. |

### Typical `cargo bless` flow

1. Parse CLI → validate flag combinations (mutually exclusive flags enforced in `main.rs`).
2. `parser::get_package_snapshots` — one `cargo metadata` call; resolves root or workspace members.
3. `suggestions::load_rules` (embedded + optional cache) → `analyze` / `analyze_for_package` → `policy::apply_policy`.
4. If not offline and suggestions exist: `intel::IntelClient::fetch_bulk_intel`.
5. `output::render_packages_modernization`.
6. If `--audit-code`: `code_audit::scan_project` → `output::render_code_audit_report`.
7. If `--fix`: `fix::apply` per package.
8. `--fail-on` exit-code check last.

`cargo bless bs` (or `cargo bless … bs`) skips steps 2-4 and runs only `code_audit`. `--diff` mode in `bs` limits the scan to lines changed in `git diff HEAD`.

### Rule data

`data/suggestions.json` is the authoritative embedded rule database — compiled into the binary via `include_str!`. Adding or changing rules: edit this file directly. The `update-suggestions` binary appends new blessed.rs-derived rules for patterns not already present locally (never overwrites).

### Policy file (`bless.toml`)

Auto-discovered next to `Cargo.toml` (or explicit via `--policy`). Supports per-package suppress/pin, global `ignore_packages`, `max_suggestions`, `[code_audit].ignore_kinds`, and `[settings].offline`.

### `--fix` safety contract

Only suggestions with `"autofix_safety": "CargoTomlOnly"` in `data/suggestions.json` are auto-applied. The fix layer writes `<manifest>.toml.bak` before any edit and never modifies Rust source files.
