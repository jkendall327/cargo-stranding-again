mod app;
mod components;
mod energy;
mod input;
mod map;
mod movement;
mod render;
mod resources;
mod systems;

use render::window_conf;

#[macroquad::main(window_conf)]
async fn main() {
    app::Game::new().run().await;
}
