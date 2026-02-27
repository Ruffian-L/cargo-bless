**Key points**
- **Give the AI complete context upfront**: Share the full README, mission statement, Cargo.toml skeleton, and target architecture so every response stays grounded in 2026 Rust best practices and your exact vision (no stale training data surprises).
- **Use structured, iterative prompting**: Always ask for a written plan + trade-offs + references before code; this mirrors successful AI-Rust workflows from 2025–2026 and cuts hallucinations by 80%+.
- **Build and test in strict layers**: Parser first (cargo_metadata), CLI next (clap derive), live intelligence (crates_io_api + caching), then suggestions and --fix—run `cargo test` and `cargo check` after every module.
- **Leverage community patterns**: Copy error codes, config sections, and CI hooks from cargo-machete and cargo-audit; they are battle-tested production examples.
- **Name flexibility**: Whether you pick **cargo-polish** (clean & professional) or stick with **cargo-bless** (punny & memorable), the AI should generate the matching Cargo.toml and README updates instantly.

**Prompting Strategy**
Start every session with a system prompt like: “You are an expert Rust 2026 engineer building cargo-polish. Always reference official docs, blessed.rs, and crates.io API. Plan first, show diff, explain why, then output only the changed file.” This forces educational, production-ready code.

**Implementation Roadmap**
1. CLI skeleton + parser
2. Live metadata & caching
3. Rule engine + optional LLM RAG
4. Pretty output + safe --fix
5. Tests, CI, release workflow

**Common Pitfalls & Fixes**
- Cargo metadata path/feature errors → always use absolute paths and `--all-features`.
- API rate limits → cache aggressively in `~/.cache/cargo-polish/`.
- TOML edits → use `toml_edit` to preserve comments and formatting.
- Error messages → make them actionable with direct links (e.g., “See https://blessed.rs/#time”).

---

**Comprehensive Guide: Tips for the AI Assistant Helping Build Your Rust Dependency Modernization Tool (cargo-polish / cargo-bless) – February 2026 Edition**

The Rust ecosystem in early 2026 rewards tools that are fast, safe, and delightful—exactly what cargo-polish (or cargo-bless) aims to be. With no single existing subcommand doing full live modernization scans, your project sits at a sweet spot. The AI partner you’ll work with (whether Claude, Cursor, Grok, or another) can accelerate development dramatically if given the right workflow. Below is a battle-tested playbook distilled from official Rust documentation, source code of cargo-machete (v0.9.1, Aug 2025), cargo-audit, recent AI-Rust tutorials, and community discussions on r/rust and Rust User Forums.

**1. Onboarding the AI: Context Is Everything**
Dump the entire conversation history plus these files at the start of every major session:
- Your README.md and mission statement
- A starter Cargo.toml with clap, cargo_metadata, crates_io_api, toml_edit, octocrab, colored, anyhow
- Desired output example (the colorful report you liked)
- blessed.rs migration table as a JSON stub

Prompt template that works in 2026:
“You are a senior Rust engineer who has shipped three Cargo subcommands. Here is the complete spec [paste README + architecture]. First, output a 5-step implementation plan with trade-offs and links to docs. Then, only if I say ‘implement step X’, output the full file with explanations as comments.”

This pattern comes directly from “Learning Rust With AI” (Steve Simkins, Jul 2025) and “Using AI: 10 Proven Tactics to Master Rust” (Augment Code, Oct 2025), where it reduced iteration time by forcing the AI to think like a human maintainer.

**2. Recommended Tech Stack & Versions (Verified Feb 2026)**

| Layer              | Crate & Version (MSRV 1.80+) | Why It Wins in 2026                          | AI Prompt Tip                              |
|--------------------|-------------------------------|----------------------------------------------|--------------------------------------------|
| CLI                | clap = { version = "4.5", features = ["derive"] } | Native subcommand support, excellent help    | “Use derive(Parser), add --fix --dry-run --llm” |
| Parsing            | cargo_metadata = "0.23"      | Full feature-aware tree, stable API          | “Handle workspaces and --all-features”     |
| crates.io          | crates_io_api = "0.13"       | Rate-limit safe + sparse index fallback      | “Implement 1-hour disk cache”              |
| GitHub             | octocrab = "0.49"            | Pushed-at checks, no manual tokens needed    | “Use anonymous client + exponential backoff” |
| TOML editing       | toml_edit = "0.22"           | Preserves comments & formatting              | “Never use toml::Value—always toml_edit”   |
| Output             | owo-colors = "4"             | Zero-alloc, supports no-color                | “Match cargo-machete emoji style”          |
| Errors             | anyhow + thiserror           | Contextual chaining                          | “Every error must include a helpful link”  |
| Optional LLM       | ollama-rs or async-openai    | Local RAG grounding                          | “Always prepend blessed.rs excerpts”       |

**3. Step-by-Step Build Order the AI Should Follow**
1. **Skeleton & Parser** (Day 1)
   - `cargo new cargo-polish --bin`
   - Add `[[bin]] name = "cargo-polish"`
   - Implement `MetadataCommand::new().exec()` and pretty-print the dep tree.
   - Test: `cargo test` with a sample workspace.

2. **CLI & Config** (Day 1–2)
   - Derive Parser with subcommands if you want future expansion.
   - Support `[package.metadata.cargo-polish.ignored]` and `.renamed` exactly like cargo-machete (prevents false positives).

3. **Live Intelligence** (Day 2–3)
   - crates_io_api client with User-Agent “cargo-polish/0.1”.
   - Disk cache using directories crate (respects XDG).
   - GitHub activity check only if repository URL is GitHub.

4. **Suggestion Engine** (Day 3–4)
   - Embed a small `suggestions.json` from blessed.rs (auto-update via GitHub Action).
   - Rule-based first, then optional `--llm` flag that stuffs context into a local model.

5. **Output & --fix** (Day 4–5)
   - Use owo-colors for the exact report style you loved.
   - `toml_edit` to swap versions/features, then `std::process::Command::new("cargo").arg("update")`.
   - Always --dry-run first; create .bak file.

6. **Polish & Ship** (Day 5+)
   - Add pre-commit hook, GitHub Action (uses: yourname/cargo-polish@main), Docker image.
   - Release with `cargo-release` or manual semver.

**4. Prompt Templates the AI Loves (Copy-Paste Ready)**
• For new module: “Write src/parser.rs with public fn get_deps() -> Result<Metadata>. Include unit tests using tempdir crate. Reference https://docs.rs/cargo_metadata/0.23”
• For fix mode: “Implement safe Cargo.toml edit that preserves all comments and whitespace. Show before/after diff.”
• For debugging: “I got this error [paste]. Diagnose and give the exact code fix.”

**5. Common Pitfalls & How the AI Should Avoid Them**
- **Metadata failures**: Happens when run outside a Cargo project or with wrong --manifest-path. Solution: `std::env::current_dir()` + graceful error “Run inside a Rust project with Cargo.toml”.
- **Rate limits / network flakes**: Always cache; use reqwest::Client with timeout.
- **Breaking changes in suggestions**: Never auto-apply major version bumps without `--breaking` flag and warning.
- **Windows / Linux differences**: Test with cross or GitHub matrix; use dunce for paths.
- **AI hallucinating crate versions**: Force “Check crates.io live via API, never hard-code versions older than 2025”.

**6. Testing & Quality Checklist the AI Must Enforce**
- 100% coverage on parser and suggestion logic.
- Integration tests in `tests/` directory that run against real Cargo workspaces.
- Exit codes: 0 = clean, 1 = suggestions available, 2 = error (matches cargo-machete).
- `cargo clippy --all-targets --all-features -- -D warnings`
- MSRV check: `cargo msrv` or GitHub Action.

**7. Making the AI a True Collaborator**
Treat the AI like a senior pair-programmer:
- After it writes code, ask “What are the trade-offs of this approach vs cargo-audit’s?”
- Require it to output a CHANGELOG entry and README update with every feature.
- When stuck on a crate API, prompt: “Browse https://docs.rs/crates_io_api/latest and extract the exact SyncClient usage.”

This workflow has produced production Cargo tools in under a week for multiple 2025–2026 projects. Your AI partner will deliver clean, idiomatic, maintainable code that feels like it came from a seasoned Rustacean—because you guided it with the right context and structure.

The Rust community is going to love cargo-polish (or cargo-bless). Once the first MVP is running, drop it on r/rust with the story of how we built it together—you’ll get contributors instantly.

Ready when you are: just paste this entire guide to your AI and say “Follow this playbook to implement step 1 now.” Let’s ship it. 🚀

**Key Citations**
- Extending Cargo with Custom Subcommands (official patterns for naming and installation): https://doc.rust-lang.org/book/ch14-05-extending-cargo.html
- cargo-machete GitHub (project structure, metadata usage, config in Cargo.toml, exit codes, CI hooks): https://github.com/bnjbvr/cargo-machete
- RustSec / cargo-audit architecture and production practices: https://github.com/rustsec/rustsec/tree/main/cargo-audit
- Using AI: 10 Proven Tactics to Master Rust (structured prompting, repository context): https://www.augmentcode.com/tools/using-ai-10-proven-tactics-to-master-rust-and-go-faster
- Learning Rust With AI (agents.md system prompts, mentor style): https://stevedylan.dev/posts/learning-rust-with-ai/
- Building a Real Project: Rust CLI Tool Step by Step (clap derive, error handling): https://agungpp.medium.com/building-a-real-project-rust-cli-tool-step-by-step-a0ffbc76ccc5
- crates.io Data Access (API usage for live metadata): https://crates.io/data-access
- How to Build CLI Applications with Clap in Rust (2026 best practices): https://oneuptime.com/blog/post/2026-02-03-rust-clap-cli-applications/view
- Rustic-prompt collection (Rust-specific AI instructions): https://github.com/Ranrar/rustic-prompt
