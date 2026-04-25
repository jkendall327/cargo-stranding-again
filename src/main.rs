use cargo_stranding_again::{app::Game, logging, render::window_conf};

#[macroquad::main(window_conf)]
async fn main() {
    logging::init();
    tracing::info!("starting windowed game");
    Game::new().run().await;
}
