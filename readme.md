# cargo-bless

A Cargo subcommand that checks your dependencies against [blessed.rs](https://blessed.rs/) recommendations and suggests modern alternatives.

## What it does

- Scans your `Cargo.toml` dependency tree (direct + transitive, with features)
- Matches against a built-in rule database sourced from blessed.rs
- Detects single-crate replacements _and_ combo optimizations (e.g. dropping `serde_json` when `reqwest` has the `json` feature)
- Fetches live metadata from crates.io (latest version, downloads) and GitHub (last push, archived status)
- Optionally applies safe fixes to your `Cargo.toml` with `--fix` (preview first with `--dry-run`)

## Installation

From crates.io (once published):

```sh
cargo install cargo-bless
```

From source:

```sh
git clone https://github.com/Ruffian-L/cargo-bless
cd cargo-bless
cargo install --path .
```

## Usage

```sh
cargo bless                  # scan and report
cargo bless --fix --dry-run  # preview changes without writing
cargo bless --fix            # apply changes (creates .bak backup)
cargo bless --update-rules   # fetch latest rules from blessed.rs
```

## Example

```
$ cargo bless

🔥 cargo-bless v0.1.0

📋 Scanning dependencies...

📦 Direct dependencies (16)
  • reqwest 0.12.28 [json, default-tls, ...]
  • serde_json 1.0.149 [default, ...]
  ...

Found 16 direct deps, 317 total.

🌐 Fetching live intelligence...

🚀 Modernization report for my-project v0.1.0

 • [LOW] reqwest+serde_json → reqwest with "json" feature
      latest: v0.13.2, 64.6M recent downloads

0 high-impact upgrades available.
```

```
$ cargo bless --fix --dry-run

🔍 Dry-run: the following changes would be made:

--- Cargo.toml (original)
+++ Cargo.toml (modified)

- serde_json = "1"

Changes that would be applied:
  ✓ Removed `serde_json`, enabled `json` feature on `reqwest`
```

## Built-in rules

| Pattern | Suggestion | Kind | Impact |
|---------|-----------|------|--------|
| `lazy_static` | `std::sync::LazyLock` | StdReplacement | High |
| `once_cell` | `std::sync::LazyLock` / `OnceLock` | StdReplacement | High |
| `memmap` | `memmap2` | Unmaintained | High |
| `failure` | `anyhow` + `thiserror` | Unmaintained | High |
| `iron` | `axum` | Unmaintained | High |
| `structopt` | `clap v4 (derive)` | ModernAlternative | Medium |
| `actix-web` | `axum` | ModernAlternative | Medium |
| `log` | `tracing` | ModernAlternative | Medium |
| `chrono` | `time` | ModernAlternative | Medium |
| `env_logger` | `tracing-subscriber` | ModernAlternative | Medium |
| `reqwest` + `serde_json` | `reqwest` with `json` feature | FeatureOptimization | Low |
| `tokio` + `async-std` | `tokio` only | ComboWin | Medium |
| `log` + `env_logger` | `tracing` + `tracing-subscriber` | ComboWin | Medium |
| `warp` | `axum` | ModernAlternative | Medium |
| `rocket` | `axum` | ModernAlternative | Medium |

Rules are embedded at compile time from `data/suggestions.json`. PRs to add more are welcome.

## How --fix works

Only some suggestion types are auto-fixable (the ones that only need `Cargo.toml` changes):

- **StdReplacement** — removes the dep (you still need to update your source code)
- **Unmaintained** — renames the dep key to the maintained fork
- **FeatureOptimization** — removes the extra dep and enables the feature on the main dep

`ModernAlternative` and `ComboWin` are reported but not auto-fixed, since they require source code changes.

Before any edit, `--fix` creates a `Cargo.toml.bak` backup and runs `cargo update` afterward.

## How it works

1. `cargo_metadata` parses the full resolved dependency tree with features
2. Rules from `data/suggestions.json` are matched against direct deps (single-crate and combo patterns)
3. `crates_io_api::SyncClient` fetches live metadata (cached to `~/.cache/cargo-bless/` with 1-hour TTL)
4. `octocrab` checks GitHub for `pushed_at`, `archived`, and star count
5. `toml_edit` applies fixes while preserving comments and formatting

Network calls are non-fatal — if you're offline, the rule-based report still works.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.