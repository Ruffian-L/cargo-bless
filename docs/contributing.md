# Contributing

## Build & test

```sh
cargo build
cargo fmt --check
cargo clippy -- -D warnings   # optional but recommended before a PR
cargo test
```

## Rules data

Suggestions ship from `data/suggestions.json`. Maintainer tooling (`update-suggestions` binary / `cargo bless --update-rules`) can refresh from blessed.rs; PRs that hand-edit JSON should keep `confidence`, `migration_risk`, and `autofix_safety` honest.

## Documentation & releases

1. **Before** tagging or `cargo publish`: update `README.md`, this `docs/` tree as needed, and `changelog.md`.
2. Bump `version` in `Cargo.toml`; commit with a clear message.
3. Open PR to `main` (branch protections may require signed commits — check repo rules).
4. After merge: tag `vX.Y.Z` on the release commit when it matches what you publish.
5. `cargo publish` (requires `cargo login`).

User-agent strings for outbound HTTP now follow `cargo-bless/` + `CARGO_PKG_VERSION` (`intel`, `updater`).
