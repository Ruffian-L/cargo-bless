# cargo-bless

A Cargo subcommand that checks your dependencies against [blessed.rs](https://blessed.rs/) recommendations and suggests modern alternatives.

<div align="center">

![cargo-bless — bless your dependency tree](https://raw.githubusercontent.com/Ruffian-L/cargo-bless/main/docs/images/readme-banner.png)

[![Crates.io](https://img.shields.io/crates/v/cargo-bless.svg?style=for-the-badge)](https://crates.io/crates/cargo-bless)
[![docs.rs](https://img.shields.io/docsrs/cargo-bless?style=for-the-badge)](https://docs.rs/cargo-bless)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue?style=for-the-badge)](LICENSE-MIT)
[![Rust MSRV](https://img.shields.io/badge/MSRV-1.80-orange?style=for-the-badge)](https://github.com/Ruffian-L/cargo-bless/blob/main/Cargo.toml#L5)

<sub>Powered by **[blessed.rs](https://blessed.rs/)** curated paths · optional crates.io + GitHub intel · optional **Cargo.toml‑only** autofix</sub>

</div>

**On crates.io:** [cargo-bless](https://crates.io/crates/cargo-bless) · **Generated API docs:** [docs.rs/cargo-bless](https://docs.rs/cargo-bless) · **Changelog:** [changelog.md](https://github.com/Ruffian-L/cargo-bless/blob/main/changelog.md) · **Repo docs:** [`docs/`](https://github.com/Ruffian-L/cargo-bless/tree/main/docs)

`cargo-bless` checks whether your Rust dependency tree is modern, boring, and defensible.

## At a glance

<div align="center">

![Pipeline concept: lockfile + rules → report](https://raw.githubusercontent.com/Ruffian-L/cargo-bless/main/docs/images/pipeline-overview.png)

</div>

```mermaid
flowchart LR
  A["Cargo.toml + lock"] --> B["cargo metadata\nresolved features"]
  B --> C["Built-in rules\n(blessed.rs lineage)"]
  C --> D["Suggestions\n+ trust metadata"]
  D --> E["TTY report or JSON"]
  E --> F{{"Extras"}}
  F --> G["Live intel\ncrates.io / GitHub"]
  F --> H["--fix\nCargo.toml only"]
  F --> I["--audit-code\nstatic scan"]
```

### Which command should I run?

```mermaid
flowchart TD
  Start([You are here]) --> Q{Need machine output?}
  Q -->|yes| J["cargo bless --json\n(+ --fail-on=… in CI)"]
  Q -->|no| R{Paste into GitHub issue?}
  R -->|yes| F["cargo bless --feedback\n(root crate)"]
  R -->|no| S{Want a one-screen roll-up?}
  S -->|yes| Sm["cargo bless --summary"]
  S -->|no| D["cargo bless\n(default full report)"]
```

### Release framing (semver)

| Version | What it represented |
|---------|---------------------|
| **0.1.0** | Birth |
| **0.1.1–0.1.3** | Rapid hardening |
| **0.1.4** | First “people might actually try this” slice — think *how does a stranger feel after running this once?* |
| **0.1.7** | Rule merges fixed (embedded suggestions win over blessed cache/tooling); selective blessed cherry-picks; CI `--feedback` smoke |
| **0.1.8** | Blessed.rs ingest: tightened migration cues, HTML-stripped notes, regression tests aligned with upstream `crates.json` wording |
| **0.2.0** | **`--workspace` / `--package`**, **`--summary`**, **`--fail-on`**, JSON grouped **per package**, virtual-workspace-safe metadata parsing, clearer **`--fix`** messaging (Cargo.toml-only) |

**Likely near-term forks:**

- **0.2.x** — `bless.toml` / severity gates refinement, `--all-targets`, cache-first polish.

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

### First run (copy-paste)

```sh
cd /path/to/your/crate
cargo bless --offline           # no network: rules + local cache only — good “what is this?” probe
cargo bless --summary --offline # shortest story: counts + pattern bullets
cargo bless --feedback          # issue template: stats + code-audit hotspots (no dep names listed)
```

If the report looks sane, drop `--offline` to light up crates.io / GitHub intel on the colorful default run.

## Usage

```sh
cargo bless                  # scan and report (root package)
cargo bless --workspace       # every workspace member (virtual workspace manifests OK)
cargo bless --package=foo,bar # only listed member packages (comma-separated)
cargo bless bs               # run only the bullshit detector code audit
cargo bless bs --diff        # audit only lines changed since HEAD
cargo bless --feedback       # paste-safe issue block (counts + code-audit hotspots; root crate only)
cargo bless --summary        # paste-friendly dependency roll-up (counts + patterns; no live intel fetch)
cargo bless --fail-on=high   # exit non-zero if any suggestion matches listed impact(s)
cargo bless --fix --dry-run  # preview Cargo.toml diff only (no writes)
cargo bless --fix            # apply Cargo.toml autofixes (`*.toml.bak`; never touches `.rs`)
cargo bless --update-rules   # fetch latest rules from blessed.rs
cargo bless --json           # structured JSON (`packages`, optional `code_audit`)
cargo bless --offline        # skip crates.io/GitHub intel; rules + cache still apply
cargo bless --audit-code     # include code audit in the main dependency run
```

### Multi-crate workspaces

```mermaid
flowchart TB
  W["Workspace Cargo.toml<br/>[workspace] members"]
  W --> P1["crates/foo"]
  W --> P2["crates/bar"]
  subgraph scan ["One cargo metadata resolve"]
    P1 --> R["Per-member suggestions"]
    P2 --> R
    R --> F["Optional --fix per Cargo.toml"]
  end
```

```text
$ cargo bless --workspace --offline

🙏 cargo-bless v0.2.0

📋 Scanning dependencies…

  • api v0.3.0 — 11 direct, 189 total (crates/api/Cargo.toml)
  • worker v0.1.0 — 6 direct, 155 total (crates/worker/Cargo.toml)

Workspace: 2 members · 17 direct deps (sum) · 344 resolved rows (sum).

📦 api v0.3.0 (crates/api/Cargo.toml)

 • [HIGH] lazy_static → std::sync::LazyLock
   …

📦 worker v0.1.0 (crates/worker/Cargo.toml)

 • [MED] log → tracing
   …
```

### CLI Flags

| Flag | Description |
|------|-------------|
| `--fix` | Apply **Cargo.toml-only** autofixes (`*.toml.bak` on write); never edits Rust sources |
| `--dry-run` | With `--fix`, prints the unified diff/plan — no files written, no `cargo update` |
| `--audit-code` | Bullshit detector pass merged across selected packages (sums files + alerts) |
| `--diff` | With `cargo bless bs`, audit only changed lines from `git diff HEAD` |
| `--verbose` | Show every code-audit finding instead of the trimmed summary |
| `--json` | Machine JSON (`cargo_bless_version`, `workspace_scan`, `packages[]`, `code_audit`) |
| `--offline` | Skip crates.io/GitHub intel; rules + embedded data still apply |
| `--policy=PATH` | Use custom `bless.toml` policy file |
| `--update-rules` | Fetch latest blessed-derived rules into the cache |
| `--manifest-path=PATH` | Workspace or package `Cargo.toml` (defaults to current directory) |
| `--feedback` | Issue/discord block: aggregates + hotspots; **root crate only** (no `--workspace`/`--package`) |
| `--summary` | Short dep summary + pattern bullets; skips live intel; mutually exclusive with `--json`/`--fix`/`--feedback`/`--audit-code` |
| `--fail-on=l,m,h,c` | Fail CI when any suggestion’s **impact** matches (comma-separated; `critical` aliases **high** for deps today) |
| `--workspace` | Analyze all `[workspace].members` with one `cargo metadata` call |
| `--package=NAMES` | Member filter (comma-separated names) |

### Picking an output mode

| Mode | Best for |
|------|----------|
| Default `cargo bless` | Full dependency report + optional crates.io/GitHub intel |
| **`--feedback`** | GitHub issues / Discord — aggregates + code-audit hotspots (**root crate only**) |
| **`--summary`** | Quick roll-up — counts + deduped “`crate → suggestion`” lines **without** fetching live intel |
| **`--json`** | CI / automation — stable JSON with **`packages[].dependency_suggestions`** and nullable **`code_audit`** |

**`--feedback`**, **`--summary`**, and **`--json`** are mutually exclusive. **`--feedback`** also rejects **`--workspace`** / **`--package`**.

### Pasteable feedback (`--feedback`)

Tried cargo-bless on a non-trivial tree? Paste **`cargo bless --feedback`** into an issue. It prints aggregate counts plus coarsely-ranked source locations (`path::fn`); it does **not** print your dependency crate list or full suggestion text. **No network** (skips live intel); still runs the local code audit. **`--manifest-path`** and **`--policy`** work as usual.

Example shape:

```
cargo-bless feedback block
version: 0.2.0
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

## Examples & terminal gallery

Synthetic screenshots below are trimmed for readability; your tree will differ.

### `cargo bless --summary` (paste-friendly roll-up)

```
🙏 cargo-bless v0.2.0

📊 Summary — scanned 1 workspace member
   • my-crate — 42 direct deps, 580 total in resolve

Suggestions after policy: 7
By impact — high: 3, medium: 3, low: 1

Top patterns:
   • serde_derive → serde with "derive" feature
   • tracing-subscriber+parking_lot → tracing-subscriber without parking_lot
   …

`--fix` changes Cargo.toml entries only — never Rust source.
```

### Default run + `--audit-code` (color in a real terminal)

```
$ cargo bless --audit-code

🙏 cargo-bless v0.2.0

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
   …
      latest: v0.13.2, 64.6M recent downloads

(This sample shows only a `[LOW]` impact row — real trees often surface `[HIGH]` items too.)

🧨 Bullshit detector code audit
Scanned 8 Rust files.
🚨 Bullshit detected: 2 findings

 • unwrap abuse src/main.rs:14:35
   Fix: Propagate the error with ?, add context, or handle the failure explicitly.
```

### `--fix --dry-run` (diff only — no writes)

```
$ cargo bless --fix --dry-run

🔍 Dry-run — previewing Cargo.toml edits only (no writes, no cargo update)

🔍 Dry-run: the following changes would be made:

--- Cargo.toml (original)
+++ Cargo.toml (modified)

- serde_json = "1"

Changes that would be applied:
  ✓ Removed `serde_json`, enabled `json` feature on `reqwest`
```

### `--json` (CI shape, v0.2+ — truncated)

```json
{
  "cargo_bless_version": "0.2.0",
  "workspace_scan": false,
  "packages": [
    {
      "name": "my-crate",
      "version": "0.5.1",
      "manifest_path": "/tmp/demo/Cargo.toml",
      "dependency_suggestions": [
        {
          "kind": "StdReplacement",
          "current": "lazy_static",
          "recommended": "std::sync::LazyLock",
          "impact": "High",
          "confidence": "High",
          "migration_risk": "Low",
          "autofix_safety": "ManualOnly",
          "reason": "…"
        }
      ]
    }
  ],
  "code_audit": null
}
```

### GitHub Actions: fail builds on **high**-impact suggestions

```yaml
jobs:
  bless:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Install cargo-bless
        run: cargo install cargo-bless
      - name: Lint dependency choices
        run: cargo bless --offline --fail-on=high
```

Combine with **`--json`** in a dedicated job if you want to upload artifacts rather than stare at ANSI colors.

### Before / after Cargo.toml (**autofix** pattern)

**Before**

```toml
[dependencies]
reqwest = { version = "0.12", features = ["json"] }
serde_json = "1"
```

**After** (conceptual — `cargo bless --fix`)

```toml
[dependencies]
reqwest = { version = "0.12", features = ["json"] }
# serde_json dropped — responses use reqwest's json path
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

## Extended documentation

These files also live under `docs/` in the repository (links work from GitHub and crates.io):

- [Documentation index](https://github.com/Ruffian-L/cargo-bless/tree/main/docs) — `docs/README.md`
- [Architecture](https://github.com/Ruffian-L/cargo-bless/blob/main/docs/architecture.md) — module map and pipeline
- [CLI reference](https://github.com/Ruffian-L/cargo-bless/blob/main/docs/cli-reference.md) — flags and subcommands
- [Contributing](https://github.com/Ruffian-L/cargo-bless/blob/main/docs/contributing.md) — build, test, release checklist

## License

MIT -- see [LICENSE-MIT](LICENSE-MIT).
