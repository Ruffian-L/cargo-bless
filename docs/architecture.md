# Architecture overview

`cargo-bless` is a normal Cargo subcommand binary (`cargo bless`). The library crate (`cargo_bless`) holds the pipeline; `src/main.rs` wires CLI → library.

## Module map (matches docs.rs)

| Module | Role |
|--------|------|
| `parser` | `cargo_metadata` — resolved dependency tree, direct vs transitive, features |
| `suggestions` | Embedded `data/suggestions.json` rules; optionally augmented by fresh blessed-rs cache (**local patterns override cache** — same precedence as `update-suggestions`). Match deps and output `Suggestion` values |
| `policy` | Optional `bless.toml` — ignore packages, limits, code-audit filters |
| `intel` | Optional crates.io + GitHub metadata (cached under `~/.cache/cargo-bless/`); skipped when `--offline` or `[settings].offline` |
| `feedback` | `--feedback` — aggregate counts + code-audit “hotspots” without listing crate names or hitting the network |
| `output` | Human-readable reports, JSON helpers, code-audit summary |
| `fix` | `toml_edit` — apply `Cargo.toml`-only autofixes; backup + optional `cargo update` |
| `updater` | `cargo bless --update-rules` — refresh rules from blessed.rs JSON |
| `code_audit` | Static “bullshit detector” pass over Rust sources under `src/`, `tests/`, `examples/`, `benches/` |

## Typical `cargo bless` flow

1. Parse CLI (`src/cli.rs`) → validate flag combinations (`main.rs`).
2. Resolve deps (`parser`).
3. Load rules (`suggestions::load_rules`) → analyze (`suggestions::analyze`) → apply policy (`policy`).
4. If not `--offline` / not policy-offline and there are suggestions, bulk-fetch intel (`intel`).
5. Print modernization report (`output::render_report`).
6. If `--audit-code`, scan sources (`code_audit::scan_project`) and print findings (`output::render_code_audit_report`).
7. If `--fix`, apply Toml edits (`fix::apply`).

`cargo bless bs` / `cargo bless … bs …` skips dependency linting and runs only `code_audit` (optionally `--diff` for changed lines vs `HEAD`).

## Data flow boundaries

- **Trust metadata** on suggestions (`impact`, `confidence`, `migration_risk`, `autofix_safety`, `evidence_source`) is rule-driven; `--fix` only applies suggestions marked Cargo.toml-safe.
- **Code audit** is advisory; it does not modify files.
- **`--feedback`** always runs suggestion analysis + full code audit for the chosen manifest but does **not** perform crates.io/GitHub intellect fetches — intended for voluntary, low-leak summaries.
