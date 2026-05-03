# Architecture overview

`cargo-bless` is a normal Cargo subcommand binary (`cargo bless`). The library crate (`cargo_bless`) holds the pipeline; `src/main.rs` wires CLI → library.

## Module map (matches docs.rs)

| Module | Role |
|--------|------|
| `parser` | `cargo_metadata` — resolved dependency tree, direct vs transitive, features |
| `suggestions` | Embedded `data/suggestions.json` rules (50 patterns); optionally augmented by fresh blessed-rs cache (**local patterns override cache**). Match deps and output `Suggestion` values |
| `policy` | Optional `bless.toml` — ignore packages, limits, code-audit filters |
| `intel` | Optional crates.io + GitHub metadata (cached under `~/.cache/cargo-bless/`); skipped when `--offline` or `[settings].offline` |
| `advisories` | Optional osv.dev batch advisory lookup for direct deps; skipped when `--offline` or `--no-advisories`; non-fatal |
| `feedback` | `--feedback` — aggregate counts + code-audit "hotspots" without listing crate names or hitting the network |
| `output` | Human-readable reports, JSON helpers (`JsonReportUnified`), code-audit summary, advisory rendering |
| `fix` | `toml_edit` — apply `Cargo.toml`-only autofixes; backup + optional `cargo update` |
| `updater` | `cargo bless --update-rules` — refresh rules from blessed.rs JSON |
| `code_audit` | Static bullshit-detector pass; scans `src/` by default; `#[test]`/`#[cfg(test)]` blocks masked via tree-sitter; opt in to `tests/`/`examples/`/`benches/` with `--include-tests` |
| `bs_detector` | Hardcoded-value scanner (`--hardcoded`): magic numbers, API keys, IPs, URLs, credentials |

## Typical `cargo bless` flow

1. Parse CLI (`src/cli.rs`) → validate flag combinations (`main.rs`).
2. Resolve deps (`parser`).
3. Load rules (`suggestions::load_rules`) → analyze (`suggestions::analyze`) → apply policy (`policy`).
4. If not `--offline` / not policy-offline and there are direct deps, bulk-fetch security advisories (`advisories::fetch_advisories_batch`); non-fatal.
5. If not `--offline` and there are suggestions, bulk-fetch live intel (`intel`).
6. Print modernization report (`output::render_report`).
7. If `--audit-code`, scan sources (`code_audit::scan_project`) and print findings (`output::render_code_audit_report`).
8. If `--fix`, apply Toml edits (`fix::apply`).

`cargo bless bs` / `cargo bless … bs …` skips dependency linting and runs only `code_audit` (optionally `--diff` for changed lines vs `HEAD`, `--fix` for `.unwrap()` → `.expect()`, `--sarif` for GitHub code scanning).

## Data flow boundaries

- **Trust metadata** on suggestions (`impact`, `confidence`, `migration_risk`, `autofix_safety`, `evidence_source`) is rule-driven; `--fix` only applies suggestions marked Cargo.toml-safe.
- **Code audit** scans `src/` only by default; uses tree-sitter-rust to parse and mask string literals, comments, and test blocks before running line-pattern detectors.
- **`cargo bless bs --fix`** is the only operation that modifies `.rs` files; it only touches `UnwrapAbuse` findings and writes `*.rs.bak` backups.
- **`--feedback`** always runs suggestion analysis + full code audit for the chosen manifest but does **not** perform crates.io/GitHub/osv.dev fetches — intended for voluntary, low-leak summaries.
