# cargo-bless

A Cargo subcommand that checks your dependencies against [blessed.rs](https://blessed.rs/) recommendations and suggests modern alternatives.

API reference: [cargo-bless on docs.rs](https://docs.rs/cargo-bless).

`cargo-bless` checks whether your Rust dependency tree is modern, boring, and defensible.

### Release framing (semver)

| Version | What it represented |
|---------|---------------------|
| **0.1.0** | Birth |
| **0.1.1–0.1.3** | Rapid hardening |
| **0.1.4** | First “people might actually try this” slice — think *how does a stranger feel after running this once?* |

**Likely near-term forks:**

- **0.1.5** — “stranger trust” polish: clearer copy, `--feedback` pasteables, tighter `--fix` disclaimers.
- **0.2.0** — Policy / config maturity: fuller `bless.toml` fields, failure gates, workspace coverage, offline-first behavior.

## What it does

- Scans your `Cargo.toml` dependency tree (direct + transitive, with features)
- Matches against a built-in rule database sourced from blessed.rs
- Detects single-crate replacements _and_ combo optimizations (e.g. dropping `serde_json` when `reqwest` has the `json` feature)
- Optionally runs a built-in bullshit detector code audit for suspicious Rust complexity patterns
- Fetches live metadata from crates.io (latest version, downloads) and GitHub (last push, archived status)
- Optionally applies safe fixes to your `Cargo.toml` with `--fix` (preview first with `--dry-run`)

## What cargo-bless is not

- It is not a replacement for `cargo audit`, `cargo deny`, or license/security policy tooling.
- It is not automatic truth. Recommendations include confidence, migration risk, autofix safety, and evidence source.
- It is not a source rewriter. `--fix` only applies rules marked as safe Cargo.toml-only edits.
- It is not a command to blindly run in production without reading the report.

## Installation

From crates.io:

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
cargo bless bs               # run only the bullshit detector code audit
cargo bless bs --diff        # audit only lines changed since HEAD
cargo bless --feedback       # print a privacy-safe block for issue reports / Discord
cargo bless --fix --dry-run  # preview changes without writing
cargo bless --fix            # apply changes (creates .bak backup)
cargo bless --update-rules   # fetch latest rules from blessed.rs
cargo bless --json           # output suggestions as JSON
cargo bless --offline        # skip network calls, use local cache only
cargo bless --audit-code     # include code audit in the main report
```

### CLI Flags

| Flag | Description |
|------|-------------|
| `--fix` | Apply auto-fixable suggestions to Cargo.toml |
| `--dry-run` | Preview changes without writing (use with `--fix`) |
| `--audit-code` | Include the bullshit detector code audit in the main report |
| `--diff` | With `cargo bless bs`, audit only changed lines from `git diff HEAD` |
| `--verbose` | Show every code-audit finding instead of the top findings summary |
| `--json` | Output suggestions as JSON array (for CI/pipelines) |
| `--offline` | Skip crates.io/GitHub fetches, use local cache only |
| `--policy=PATH` | Use custom bless.toml policy file |
| `--update-rules` | Fetch latest rules from blessed.rs |
| `--manifest-path=PATH` | Path to Cargo.toml (defaults to current directory) |
| `--feedback` | Emit version, dep counts, suggestion counts, and top code-audit hotspots (paste into issues; no telemetry) |

### Pasteable feedback (`--feedback`)

Tried cargo-bless on a non-trivial tree? Paste the output of **`cargo bless --feedback`** into a GitHub issue. It prints aggregate counts plus coarsely-ranked source locations (`path::fn` where we can infer a function); it does **not** list crate names from your dependency graph or cargo-bless’s suggestion text — no telemetry runs.

Example shape:

```
cargo-bless feedback block
version: 0.1.5
direct_deps: 46
total_deps: 624
suggestions: 2
high_impact: 1
code_audit_findings: 401
top_hotspots:
  - src/main.rs::run_simulation
  - src/main.rs:apply_forces
```

### Policy File (bless.toml)

Drop a `bless.toml` next to your `Cargo.toml` to customize behavior:

```toml
# Ignore specific packages
ignore_packages = ["internal-crate"]

# Per-package overrides
[packages.lazy_static]
suppress = true
keep_reason = "We use lazy_static for cross-crate compatibility"

# Global settings
[settings]
offline = true
max_suggestions = 10

[code_audit]
ignore_paths = ["src/generated", "tests/fixtures"]
ignore_kinds = ["UnwrapAbuse"]
```

Or pass a custom path: `cargo bless --policy=custom-bless.toml`

## Example

```
$ cargo bless --audit-code

🔥 cargo-bless v0.1.5

📋 Scanning dependencies...

📦 Direct dependencies (16)
  • reqwest 0.12.28 [json, default-tls, ...]
  • serde_json 1.0.149 [default, ...]
  ...

Found 16 direct deps, 317 total.

🌐 Fetching live intelligence...

🚀 Modernization report for my-project v0.1.0

 • [LOW] reqwest+serde_json → reqwest with "json" feature
   [HIGH confidence] [LOW risk] [autofix: Cargo.toml-only] evidence: crate docs
   reqwest can deserialize JSON directly when its json feature is enabled; cargo-bless only suggests this when serde_json is not used directly in source
      latest: v0.13.2, 64.6M recent downloads

0 high-impact upgrades available.

🧨 Bullshit detector code audit
Scanned 8 Rust files.
🚨 Bullshit detected: 2 findings
unwrap abuse: 1, fake complexity: 1

 • unwrap abuse src/main.rs:14:35
   unwrap() is a runtime trap dressed up as confidence.
   Fix: Propagate the error with ?, add context, or handle the failure explicitly.
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

Each rule carries trust metadata:

- `impact`: how important the dependency choice may be.
- `confidence`: how strong the recommendation is.
- `migration_risk`: how likely the change is to require careful review.
- `autofix_safety`: whether `cargo bless --fix` may edit `Cargo.toml`.
- `evidence_source`: where the recommendation is grounded.

| Pattern | Suggestion | Impact | Confidence | Risk | Autofix |
|---------|------------|--------|------------|------|---------|
| `lazy_static` | `std::sync::LazyLock` | High | High | Low | Manual |
| `once_cell` | `std::sync::LazyLock` / `OnceLock` | High | High | Low | Manual |
| `memmap` | `memmap2` | High | High | Medium | Manual |
| `failure` | `anyhow` + `thiserror` | High | High | Medium | Manual |
| `iron` | `axum` | High | High | High | Manual |
| `structopt` | `clap v4 (derive)` | Medium | High | Medium | Manual |
| `log` | `tracing` | Medium | Medium | Medium | Manual |
| `chrono` | consider `time` | Medium | Low | Medium | Manual |
| `reqwest` + `serde_json` | `reqwest` with `json` feature | Low | High | Low | Cargo.toml-only |
| `serde_derive` | `serde` with `derive` feature | Low | High | Low | Cargo.toml-only |
| `clap` + `clap_derive` | `clap` with `derive` feature | Low | High | Low | Cargo.toml-only |

Rules are embedded at compile time from `data/suggestions.json`. PRs to add more are welcome.

## How --fix works

Only suggestions marked `autofix_safety = "CargoTomlOnly"` are auto-fixable.

`StdReplacement`, `Unmaintained`, `ModernAlternative`, and `ComboWin` are reported but not auto-fixed by default, since they usually require source code changes or architectural judgment.

Code-audit findings are advisory in this release. `--fix` only edits dependency declarations in `Cargo.toml`; it never rewrites Rust source files.

Before any write, `--fix` creates a `Cargo.toml.bak` backup and runs `cargo update` afterward.

## How it works

1. `cargo_metadata` parses the full resolved dependency tree with features
2. Rules from `data/suggestions.json` are matched against direct deps (single-crate and combo patterns)
3. `crates_io_api::SyncClient` fetches live metadata (cached to `~/.cache/cargo-bless/` with 1-hour TTL)
4. `reqwest` checks GitHub for `pushed_at`, `archived`, and star count
5. With `--audit-code` or `cargo bless bs`, the bullshit detector scans Rust files under `src`, `tests`, `examples`, and `benches` for static complexity patterns
6. `toml_edit` applies fixes while preserving comments and formatting

Network calls are non-fatal — if you're offline, the rule-based report still works.

## License

MIT -- see [LICENSE-MIT](LICENSE-MIT).
