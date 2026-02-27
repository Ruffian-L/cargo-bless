**No full-featured tool matching this exact description exists as of February 2026.**  
**It is 100% feasible to build—and would fill a clear gap for both human developers and AI-assisted coding.**  
**A solid POC could be shipped in a weekend; a polished version in weeks with community help.**  

Key points:  
- Existing tools cover pieces (version checks, security/unmaintained flags, unused deps, curated recs) but none combine Cargo.toml/lockfile scanning with smart modern alternatives, feature optimizations, or combo suggestions like “reqwest + json feature instead of serde_json”.  
- The blueprint from the conversation (parser + live data + rule-based/LLM suggestions) works perfectly with battle-tested crates.  
- Research suggests this would be a massive hit on r/rust, crates.io, and IDE integrations—exactly what fixes stale 2023/2024 AI or Stack Overflow code.  
- Evidence leans toward starting rule-based (fast & reliable) then adding optional LLM RAG for the “spicy” 2026-modern suggestions grounded in blessed.rs and lib.rs.  

**Quick start recommendation**  
Name it **cargo-bless** (ties directly to blessed.rs) or **cargo-fresh** (even though a different global tool exists). Install via `cargo install cargo-bless` once published. Run `cargo bless` for a report or `cargo bless --fix` to edit Cargo.toml safely.  

**High-level architecture**  
1. Parse → `cargo_metadata` + `cargo-lock`  
2. Fetch live intel → `crates_io_api` (or sparse index) + GitHub API via `octocrab`  
3. Suggest → hardcoded blessed.rs mappings + optional LLM context  
4. Output & fix → pretty report + `toml_edit` + `cargo update`  

A minimal viable tool would already outperform every existing option combined for the “modern/alternative/combo” use case.

---

**Comprehensive Research & Build Guide: Creating a Rust Dependency Modernization Tool (cargo-bless / cargo-fresh)**  

The Rust ecosystem in February 2026 is mature but still fragmented when it comes to dependency hygiene. Developers and AI coding assistants frequently encounter stale recommendations (lazy_static in 2026, manual serde_json with reqwest, structopt instead of clap v4 derive). No single CLI currently scans a project’s Cargo.toml + lockfile, cross-references live crates.io data, blessed.rs recommendations, lib.rs discovery signals, and GitHub activity to produce actionable “use X instead / enable this feature / drop this combo” suggestions.

**Existing Tools – Comparison Table**  

| Tool                | Primary Function                          | Strengths                              | Gaps (why it doesn’t solve the full problem) |
|---------------------|-------------------------------------------|----------------------------------------|----------------------------------------------|
| cargo-outdated     | Checks for newer versions                | Fast, respects lockfile                | Only versions; no alternatives or features   |
| cargo-audit (RustSec) | Security + unmaintained flags           | Excellent advisories with successor notes | Security-focused; misses non-vuln modern recs |
| cargo-unmaintained  | Heuristic unmaintained detection         | Automatic (no repo, archived, etc.)    | No positive “switch to X” suggestions        |
| cargo-machete      | Unused dependency detection              | Lightning fast                         | Only removal; no upgrades or alts            |
| blessed.rs         | Hand-curated “use this instead” list     | Gold standard for time/chrono, axum, etc. | Static website; no automatic project scan    |
| lib.rs             | Discovery + related crates section       | Better stats, popularity, groupings    | Manual browsing only                         |
| cargo-fresh (jenkinpan) | Global installed binary updates       | Interactive                            | Not for project dependencies                 |

None combine all layers into one automated flow. This gap is exactly why the proposed tool would be transformative.

**Detailed Build Roadmap (Step-by-Step)**  

1. **Project Setup – Cargo Subcommand**  
   Create a binary named `cargo-bless` (Cargo auto-detects any `cargo-xxx` in PATH).  
   ```bash
   cargo new cargo-bless --bin
   cd cargo-bless
   ```  
   Use `clap` (derive) or `clap-cargo` for nice `--help`, `--fix`, `--llm`, etc.  
   Example skeleton (from standard Cargo subcommand patterns and clap cargo-example):  
   ```toml
   # Cargo.toml (starter)
   [package]
   name = "cargo-bless"
   version = "0.1.0"
   edition = "2021"
   description = "Modernize your Rust dependencies with blessed.rs + live intel"

   [[bin]]
   name = "cargo-bless"
   path = "src/main.rs"

   [dependencies]
   clap = { version = "4", features = ["derive"] }
   cargo_metadata = "0.19"          # or latest
   crates_io_api = "0.12"
   toml_edit = "0.22"
   reqwest = { version = "0.12", features = ["json"] }
   serde = { version = "1", features = ["derive"] }
   colored = "2"
   octocrab = "0.43"                # GitHub API
   # optional: scraper, ollama-rs, syn, rustsec
   ```  

2. **Parser Layer (Cargo.toml + lockfile)**  
   ```rust
   use cargo_metadata::{MetadataCommand, CargoOpt};

   let metadata = MetadataCommand::new()
       .features(CargoOpt::AllFeatures)
       .exec()
       .unwrap();

   // Full resolved tree with exact versions & enabled features
   for pkg in &metadata.packages {
       if pkg.source.is_none() { continue; } // skip workspace members if wanted
       println!("{} {} (features: {:?})", pkg.name, pkg.version, pkg.features);
   }
   ```  
   This gives you everything: direct deps, transitive, features enabled, exact locked versions.

3. **Live Intelligence Layer**  
   - **crates.io**: Use `crates_io_api::SyncClient` (handles rate limits + User-Agent).  
     Endpoints (from official OpenAPI):  
     - `GET /api/v1/crates/{name}` → latest version, downloads, repository URL, description.  
     - `GET /api/v1/crates/{name}/versions` → full version list + release dates.  
     - `GET /api/v1/crates/{name}/downloads` → 90-day stats.  
     Sparse index (`https://index.crates.io`) is rate-limit free for bulk metadata.  
   - **Maintenance & Activity**: Parse repository URL from above. If GitHub, `octocrab` →  
     ```rust
     let repo = octocrab.repos(owner, repo).get().await?;
     let pushed_at = repo.pushed_at; // or /commits?per_page=1
     ```  
     Cross-reference with RustSec for unmaintained flags.  
   - **Cache aggressively** (directories crate + .cache/cargo-bless/) to respect 1 req/s.

4. **Suggestion Engine**  
   **Rule-based core** (fast, deterministic, always on):  
   Maintain a small embedded or Git-synced JSON/YAML DB populated from blessed.rs. Examples (directly from blessed.rs and the original convo):  

   | Current Crate/Pattern          | Suggested Modern Alternative       | Reason / When to Suggest                  | Source          |
   |--------------------------------|------------------------------------|-------------------------------------------|-----------------|
   | chrono v0.4                    | time v0.3 (or keep for full TZ)   | Smaller, faster compiles, no bloat if only UTC | blessed.rs     |
   | reqwest + separate serde_json  | reqwest with "json" feature       | Built-in deserialization, one fewer dep   | reqwest docs + lib.rs |
   | lazy_static                    | std::sync::LazyLock               | Stable since Rust 1.80 (2024)             | std docs       |
   | structopt                      | clap v4 with derive               | Fully integrated, actively maintained     | blessed.rs     |
   | actix-web 3 patterns           | axum                              | Minimal & ergonomic, Tokio-first          | blessed.rs     |
   | tokio + async-std              | tokio only                        | Dominant runtime ecosystem                | blessed.rs     |
   | memmap                         | memmap2                           | Unmaintained → maintained fork            | RustSec        |

   **LLM RAG mode** (optional `--llm` flag): Stuff the dep list + fresh metadata + full blessed.rs excerpts + recent Rust blog posts into context and ask a local model (ollama-rs) or API:  
   “You are a 2026 Rust expert. Given these dependencies and blessed.rs recommendations, suggest modern alternatives or feature optimizations.”  
   Grounding prevents hallucination and beats training-cutoff issues perfectly.

5. **Output & --fix Mode**  
   Beautiful report matching the example in the query (emojis, bullets, combo wins).  
   `--fix`:  
   - Use `toml_edit` to surgically edit dependencies section (preserves comments/formatting).  
   - Run `cargo update` after.  
   - Dry-run + backup first (Cargo.toml.bak).  

6. **Advanced / Future Features**  
   - Source-level API usage (syn parser to detect “you only use DateTime<Utc> from chrono → switch to time”).  
   - IDE integration (rust-analyzer diagnostic or VSCode extension).  
   - Community PRs to the migration DB (just like blessed.rs itself).  
   - Integrate cargo-audit, cargo-machete, cargo-unmaintained under one roof.  

**Tech Stack Summary Table (Minimal + Recommended)**  

| Layer              | Crate(s) Recommended                     | Why |
|--------------------|------------------------------------------|-----|
| CLI                | clap 4 (derive)                          | Standard, subcommand friendly |
| Parsing            | cargo_metadata 0.19+                     | Official, full tree + features |
| crates.io          | crates_io_api 0.12                       | Handles limits, easy structs |
| GitHub             | octocrab 0.43                            | Full REST client |
| TOML editing       | toml_edit 0.22                           | Preserves comments |
| Output             | colored / owo-colors                     | Nice terminal UI |
| Optional LLM       | ollama-rs or async-openai                | Local or API RAG |
| Scraping blessed   | scraper + reqwest (or sync Git clone)    | For auto DB updates |

**Potential Challenges & Mitigations**  
- Rate limits → cache + sparse index fallback.  
- Subjective “better” → always show source (blessed.rs link, downloads delta, last commit).  
- Breaking changes → warn + suggest `cargo update --breaking` only in dry-run.  
- Data freshness → nightly CI that updates migration DB from blessed.rs GitHub.

**Why This Would Blow Up**  
It directly solves the “stale training data” pain for AI tools and the “I copied code from 2023” pain for everyone. Combine it with rust-analyzer inline warnings and Dependabot-style PRs and the Rust ecosystem gets a massive quality-of-life upgrade.

**My take as Grok**  
Yes—this bad boy deserves to ship. The blueprint is rock-solid, the tech exists today, and the need is real. I can generate the complete initial Cargo.toml + src/main.rs skeleton (with rule DB stub and report output) right now if you say the word, or we can iterate on specific parts (LLM integration, source parsing, etc.). Ping me and we’ll build it together. The ecosystem really does need it yesterday. 🚀

**Key Citations**  
- Blessed.rs – Recommended Crate Directory (hand-curated alternatives, time vs chrono, etc.): https://blessed.rs/  
- crates.io Data Access & API (endpoints, rate limits, sparse index, OpenAPI): https://crates.io/data-access  
- Cargo Metadata Command & cargo_metadata crate (full dep tree parsing): https://doc.rust-lang.org/cargo/commands/cargo-metadata.html  
- lib.rs – Crate discovery with related sections (reqwest json feature example): https://lib.rs/crates/reqwest  
- Extending Cargo with Custom Subcommands (official pattern): https://doc.rust-lang.org/book/ch14-05-extending-cargo.html  
- crates_io_api Rust client documentation: https://docs.rs/crates_io_api  
- GitHub blessed-rs repository (curated data source): https://github.com/nicoburns/blessed-rs