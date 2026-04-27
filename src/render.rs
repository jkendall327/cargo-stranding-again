use bevy_ecs::prelude::*;
use macroquad::prelude::*;

mod hud;
mod layout;
mod menu;
mod view_model;
mod world;

use crate::resources::{Camera, DEFAULT_CAMERA_TILE_SPAN};

pub const TILE_SIZE: f32 = 16.0;

pub fn window_conf() -> Conf {
    Conf {
        window_title: "Cargo Stranding Again".to_owned(),
        // The camera shows a configurable tile square while the debug panel
        // stays fixed to the right.
        window_width: layout::window_width_for_camera(DEFAULT_CAMERA_TILE_SPAN),
        window_height: 1160,
        high_dpi: true,
        ..Default::default()
    }
}

pub fn render(world: &mut World) {
    clear_background(Color::from_rgba(16, 18, 20, 255));

    world::update_camera(world);
    let camera = *world.resource::<Camera>();

    world::draw_world(world, camera);
    hud::draw_ui(world, camera);
    menu::draw_game_state_overlay(world);
}
