use cargo_stranding_again::{app::Game, render::window_conf};

#[macroquad::main(window_conf)]
async fn main() {
    Game::new().run().await;
}
