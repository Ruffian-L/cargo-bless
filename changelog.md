# Changelog

All notable changes to `cargo-bless` are logged here.

## 0.1.6 (2026-05-02)

- Published to [crates.io/cargo-bless/0.1.6](https://crates.io/crates/cargo-bless/0.1.6). GitHub: [PR #28](https://github.com/Ruffian-L/cargo-bless/pull/28).
- Documentation: expanded root README (working links for crates.io/GitHub, roadmap through 0.1.6). New **`docs/`** index — [README](https://github.com/Ruffian-L/cargo-bless/blob/main/docs/README.md), [architecture](https://github.com/Ruffian-L/cargo-bless/blob/main/docs/architecture.md), [CLI reference](https://github.com/Ruffian-L/cargo-bless/blob/main/docs/cli-reference.md), [contributing](https://github.com/Ruffian-L/cargo-bless/blob/main/docs/contributing.md). Phase 3 workspace design cross-linked from `docs/`.
- HTTP `User-Agent` for crates.io / blessed.rs fetches now uses `concat!(…, env!("CARGO_PKG_VERSION"))` so it tracks the running crate version.

## 0.1.5 (2026-05-02)

- Published to [crates.io](https://crates.io/crates/cargo-bless/0.1.5). GitHub: [PR #26](https://github.com/Ruffian-L/cargo-bless/pull/26) (squash-merged to `main`).
- Added `cargo bless --feedback`: prints a voluntary, paste-friendly summary block (version, dep counts, suggestion counts, high-impact count, code-audit totals, ranked hotspots) without network calls — useful for GitHub issues without telemetry.
- Relaxed wording for the `chrono` ↔ `time` rule in `data/suggestions.json`: frames `time` as something to evaluate, explicitly not a mandate to replace `chrono`.
- Documented release framing (0.1.4 as first “strangers might try it” milestone), roadmap sketch for **0.1.5** (trust polish) vs **0.2.0** (policy boundary), docs.rs link, and README section for `--feedback`.

## Earlier releases

- **0.1.4**: First crates.io milestone suitable for brave early adopters; docs.rs publishes module breakdown (parser, policy, suggestions, intel, fix, output, updater, code audit).
