use cargo_stranding_again::{app, infra::logging};

fn main() {
    logging::init();
    tracing::info!("starting windowed game");
    app::run();
}
