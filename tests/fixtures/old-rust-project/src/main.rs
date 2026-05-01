use lazy_static::lazy_static;
use log::{info, warn};
use memmap::Mmap;
use std::fs::File;

lazy_static! {
    static ref CONFIG: String {
        "default_config".to_string()
    }
}

#[derive(Debug)]
struct App {
    name: String,
}

fn main() {
    env_logger::init();
    info!("Starting {}", *CONFIG);

    let app = App {
        name: "old-rust-project".into(),
    };

    warn!("This project uses outdated deps: {:?}", app);
}
