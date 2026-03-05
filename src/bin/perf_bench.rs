use std::time::Instant;
use cargo_bless::intel::IntelClient;

fn main() {
    let client = IntelClient::new().unwrap();
    // Use some crates to avoid hitting limits but enough to show a difference.
    let crates = vec![
        "serde", "tokio", "anyhow", "reqwest", "clap",
        "log", "thiserror", "regex", "lazy_static", "syn",
        "quote", "proc-macro2", "unicode-ident", "libc", "cfg-if",
        "zeroize", "smallvec", "itoa", "cc", "once_cell"
    ];
    let start = Instant::now();
    let res = client.fetch_bulk_intel(&crates);
    println!("Fetched {} crates in {:?}", res.len(), start.elapsed());
}
