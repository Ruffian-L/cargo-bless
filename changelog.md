# Changelog

All notable changes to `cargo-bless` are logged here.

## 0.2.1 (2026-05-02)

- **`--all-targets`:** Widens "direct dependency" to include `[dev-dependencies]` and `[build-dependencies]`. Without the flag, only `[dependencies]` are analyzed (previous behaviour silently included dev-deps; now opt-in). Removes the "not implemented" guard and exposes the flag in `--help`.
- **`cargo bless bs --hardcoded`:** Wires the previously-dormant `bs_detector` module into the CLI. Scans for hardcoded values — magic numbers, API keys, IPs, URLs, file paths, credentials, timeouts. Findings appear in terminal output and in a `hardcoded_values` key in `--json` output.
- **`ArcAbuse` detector:** `code_audit` now emits `ArcAbuse` findings (0.62 confidence) for `Arc<String>`, `Arc<Vec<…>>`, and `Arc<Box<…>>` — value types needlessly wrapped in shared ownership.
- **Integration tests:** Added `test_package_flag_filters_workspace_member`, `test_explicit_policy_flag_suppresses_suggestion`, `test_bs_hardcoded_flag_reports_hardcoded_values`.

## 0.2.0 (2026-05-02)

- **`--workspace` / `--package`:** Scan every `workspace.members` crate (single `cargo metadata`); **`--package`** filters by **`[package].name`**; per-member suggestions + autofix loops each member **`Cargo.toml`**. Virtual workspace roots tolerate missing **`resolve.root`** when iterating members.
- **`--summary`:** Concise dependency roll-up (**counts**, **impact tallies**, **deduped `crate → reco` bullets**); skips crates.io/GitHub intel. Compared with **`--feedback`** vs **`--json`** in `README.md` and `docs/cli-reference.md`.
- **`--fail-on`:** Exit non-zero when any retained suggestion’s **impact** matches **`low`** / **`medium`** / **`high`** / **`critical`** (**critical aliases high** for dependency tier until code-audit gating arrives).
- **JSON breaking layout:** **`cargo_bless_version`**, **`workspace_scan`**, **`packages[]`**, per-package **`dependency_suggestions`**, nullable **`code_audit`**; **`cargo bless bs --json`** uses **`packages: []`** + audit blob.
- **Fix trust copy:** **`--dry-run`** / **`--fix`** Cargo.toml-only messaging across README + `main` banners + **`fix::apply`** stderr + modernization footer.
- **README visuals:** shields + hero/pipeline PNGs (under **`docs/images/`**, readme links use **raw.githubusercontent.com** so [crates.io](https://crates.io/) README renders images), Mermaid pipeline & command-picker flows, workspace mock output, **`--json`** / **`--fail-on`** Action snippet, autofix **`Cargo.toml`** before/after.

## 0.1.8 (2026-05-02)

- Published to [crates.io/cargo-bless/0.1.8](https://crates.io/crates/cargo-bless/0.1.8). GitHub: [PR #33](https://github.com/Ruffian-L/cargo-bless/pull/33).
- Recalibrated blessed migration cues against upstream [`data/crates.json`](https://raw.githubusercontent.com/nicoburns/blessed-rs/main/data/crates.json) and Firecrawl snapshots of [`blessed.rs`](https://blessed.rs/) / [`blessed.rs/crates`](https://blessed.rs/crates). Filters out bogus rows driven only by “simpler” / loose “prefer” matches (e.g. flume vs crossbeam-channel, color-eyre vs anyhow tails). Keeps explicit **older**/deprecation wording, **go-to** / “now the …” modernization copy, and niche **games**/2d + simpler notes.
- Converter **strips HTML** from blessed notes before classification; reasons use that cleaned text.

## 0.1.7 (2026-05-02)

- Published to [crates.io/cargo-bless/0.1.7](https://crates.io/crates/cargo-bless/0.1.7). GitHub: [PR #30](https://github.com/Ruffian-L/cargo-bless/pull/30).
- **Rule merging (tooling + runtime):** `data/suggestions.json` patterns are **authoritative**. `cargo run --bin update-suggestions` and `suggestions::load_rules()` append blessed-derived rows only when the pattern is absent locally — fixes curated rules being overwritten when the blessed converter is conservative or the cache briefly contains a worse row (e.g. `lazy_static` → `once_cell`).
- **Rules data:** Merged one new blessed row (`ggez` → `bevy`); dropped `color-eyre` and `flume` suggestions that were misleading or self-contradictory from the live blessed fetch.
- **CI / docs:** Smoke run for `cargo bless --feedback`; `update-rules` workflow comment clarifies merge policy; `docs/contributing.md` documents review after rule merges.

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
