# Contributing

## Build & test

```sh
cargo build
cargo fmt --check
cargo clippy -- -D warnings   # optional but recommended before a PR
cargo test
```

## Rules data

Suggestions ship from `data/suggestions.json`. Run **`cargo run --bin update-suggestions`** (or `.github/workflows/update-rules.yml`) to pull [blessed.rs](https://blessed.rs/) and **merge in new crate patterns**.

**Merge policy:** local rules **win pattern conflicts**. Blessed-derived rows are appended only when the pattern is not already defined in `data/suggestions.json` — so curated copy (e.g. `lazy_static` → `LazyLock`) is never overwritten. After a merge, skim the tail of the JSON and drop any row that contradicts its own rationale (upstream data quirks happen).

Embedded metadata (`confidence`, `migration_risk`, `autofix_safety`) should stay honest on hand-edited rows.

## Documentation & releases

1. **Before** tagging or `cargo publish`: update `README.md`, this `docs/` tree as needed, and `changelog.md`.
2. Bump `version` in `Cargo.toml`; commit with a clear message.
3. Open PR to `main` (branch protections may require signed commits — check repo rules).
4. After merge: tag `vX.Y.Z` on the release commit when it matches what you publish.
5. `cargo publish` (requires `cargo login`).

User-agent strings for outbound HTTP now follow `cargo-bless/` + `CARGO_PKG_VERSION` (`intel`, `updater`).
