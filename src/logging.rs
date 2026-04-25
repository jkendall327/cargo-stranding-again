use std::io;

use tracing_subscriber::EnvFilter;

pub fn init() {
    init_with_default("cargo_stranding_again=info,headless=info");
}

pub fn init_headless() {
    init_with_default("cargo_stranding_again=warn,headless=warn");
}

fn init_with_default(default_filter: &str) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(io::stderr)
        .try_init();
}
