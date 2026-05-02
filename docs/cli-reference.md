# CLI reference

Invoked as `cargo bless …` once `cargo-bless` is on your `PATH` (`cargo install cargo-bless`).

## Subcommands

| Invocation | Meaning |
|------------|---------|
| `cargo bless` | Dependency scan + report; optional `--audit-code`, `--fix`, etc. |
| `cargo bless bs` | Code audit only (same flags as standalone audit: `--diff`, `--json`, `--manifest-path`, `--policy`, `--verbose`). |
| `cargo bless … bs …` | Same as nested `BlessSubcommand::Bs` (audit-only shortcut). |

## `cargo bless` flags

| Flag | Description |
|------|-------------|
| `--manifest-path=PATH` | Root `Cargo.toml` to analyze (default: current crate). |
| `--offline` | Skip crates.io/GitHub fetches for live intel (rules + cache still work). |
| `--json` | Emit machine-readable JSON (suggestions and optional code audit); cannot combine with `--fix`. |
| `--fix` | Apply Cargo.toml-only autofixes; creates `Cargo.toml.bak`. |
| `--dry-run` | With `--fix`, print diff only (no writes). |
| `--update-rules` | Download latest blessed-derived rules into the cache |
| `--audit-code` | Include code audit section in the main run (heavy on large trees). |
| `--no-audit-code` | Hidden noop for compatibility |
| `--verbose` | Dump every code-audit finding instead of trimmed summary |
| `--policy=PATH` | Use this `bless.toml` instead of auto-discovered next to manifest |
| `--feedback` | Print paste-safe stats block only (counts + hotspots); no network; excludes `--fix`/`--json`/`--dry-run`/`--audit-code`; honors `--manifest-path` and `--policy` |

Reserved / hidden (fail at runtime until implemented):

- `--fail-on`, `--workspace`, `--package`, `--all-targets`, `--llm`

### `cargo bless bs` flags

Same as structured `CodeAuditOpts`: `--manifest-path`, `--policy`, `--json`, `--diff`, `--verbose`.

## Policy file (`bless.toml`)

See repository README → **Policy File** section. Applies to both modernization and code-audit ignores.
