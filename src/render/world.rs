use bevy_ecs::prelude::*;
use macroquad::prelude::*;

use crate::components::*;
use crate::map::{Map, Terrain};
use crate::render::layout::{
    tile_to_screen, viewport_height, viewport_width, VIEWPORT_X, VIEWPORT_Y,
};
use crate::render::TILE_SIZE;
use crate::resources::Camera;

pub(super) fn update_camera(world: &mut World) {
    let player_position = {
        let mut query = world.query_filtered::<&Position, With<Player>>();
        query.iter(world).next().copied()
    };
    let Some(player_position) = player_position else {
        return;
    };

    let bounds = {
        let map = world.resource::<Map>();
        map.bounds()
    };
    world
        .resource_mut::<Camera>()
        .center_on(player_position, bounds);
}

pub(super) fn draw_world(world: &mut World, camera: Camera) {
    draw_viewport_background(camera);

    {
        let map = world.resource::<Map>();
        draw_map(map, camera);
    }

    draw_parcels(world, camera);
    draw_porters(world, camera);
    draw_player(world, camera);
    draw_viewport_frame(camera);
}

fn draw_viewport_background(camera: Camera) {
    draw_rectangle(
        VIEWPORT_X - 4.0,
        VIEWPORT_Y - 4.0,
        viewport_width(camera) + 8.0,
        viewport_height(camera) + 8.0,
        Color::from_rgba(8, 10, 12, 255),
    );
}

fn draw_viewport_frame(camera: Camera) {
    draw_rectangle_lines(
        VIEWPORT_X - 1.0,
        VIEWPORT_Y - 1.0,
        viewport_width(camera) + 2.0,
        viewport_height(camera) + 2.0,
        2.0,
        Color::from_rgba(215, 220, 226, 180),
    );
}

fn draw_map(map: &Map, camera: Camera) {
    for coord in map.visible_tiles(camera.origin_coord(), camera.width, camera.height) {
        let tile = map
            .tile_at_coord(coord)
            .expect("visible tile iteration is in bounds");
        let (px, py) = tile_to_screen(camera, coord);
        draw_rectangle(
            px,
            py,
            TILE_SIZE,
            TILE_SIZE,
            terrain_color(tile.terrain, tile.elevation, tile.water_depth),
        );
        draw_rectangle_lines(
            px,
            py,
            TILE_SIZE,
            TILE_SIZE,
            1.0,
            Color::from_rgba(0, 0, 0, 45),
        );

        draw_text(
            &tile.elevation.to_string(),
            px + 1.5,
            py + 7.0,
            8.0,
            Color::from_rgba(245, 245, 235, 175),
        );

        if tile.water_depth > 0 {
            draw_text(
                &tile.water_depth.to_string(),
                px + 10.0,
                py + 14.0,
                8.0,
                Color::from_rgba(210, 235, 255, 210),
            );
        } else if matches!(terrain_glyph(tile.terrain), "D" | "^") {
            draw_text(
                terrain_glyph(tile.terrain),
                px + 4.0,
                py + 12.5,
                14.0,
                Color::from_rgba(18, 18, 18, 200),
            );
        }
    }
}

fn terrain_color(terrain: Terrain, elevation: i16, water_depth: u8) -> Color {
    let base = match terrain {
        Terrain::Grass => Color::from_rgba(64, 128, 72, 255),
        Terrain::Mud => Color::from_rgba(104, 75, 48, 255),
        Terrain::Rock => Color::from_rgba(92, 96, 100, 255),
        Terrain::Water => Color::from_rgba(34, 92, 138, 255),
        Terrain::Road => Color::from_rgba(150, 126, 78, 255),
        Terrain::Depot => Color::from_rgba(214, 174, 68, 255),
    };
    if terrain == Terrain::Water {
        shade_color(base, -(f32::from(water_depth) * 0.09))
    } else {
        shade_color(base, (f32::from(elevation) - 4.5) * 0.045)
    }
}

fn shade_color(color: Color, amount: f32) -> Color {
    Color {
        r: (color.r + amount).clamp(0.0, 1.0),
        g: (color.g + amount).clamp(0.0, 1.0),
        b: (color.b + amount).clamp(0.0, 1.0),
        a: color.a,
    }
}

fn terrain_glyph(terrain: Terrain) -> &'static str {
    match terrain {
        Terrain::Grass => ".",
        Terrain::Mud => "~",
        Terrain::Rock => "^",
        Terrain::Water => "w",
        Terrain::Road => "=",
        Terrain::Depot => "D",
    }
}

fn draw_parcels(world: &mut World, camera: Camera) {
    let mut query = world.query::<(&Position, &CargoParcel, &ParcelState)>();
    for (position, parcel, state) in query.iter(world) {
        if !matches!(state, ParcelState::Loose | ParcelState::AssignedTo(_)) {
            continue;
        }
        if !camera.contains(*position) {
            continue;
        }
        let (px, py) = tile_to_screen(camera, (*position).into());
        let cx = px + TILE_SIZE / 2.0;
        let cy = py + TILE_SIZE / 2.0;
        let color = if matches!(state, ParcelState::AssignedTo(_)) {
            Color::from_rgba(252, 204, 84, 255)
        } else {
            Color::from_rgba(224, 154, 72, 255)
        };
        draw_rectangle(cx - 5.0, cy - 5.0, 10.0, 10.0, color);
        draw_text(
            &format!("{:.0}", parcel.weight),
            cx - 4.0,
            cy + 4.0,
            10.0,
            BLACK,
        );
    }
}

fn draw_porters(world: &mut World, camera: Camera) {
    let mut query = world.query::<(&Position, &Porter, &Cargo, &AssignedJob)>();
    for (position, porter, cargo, job) in query.iter(world) {
        if !camera.contains(*position) {
            continue;
        }
        let (px, py) = tile_to_screen(camera, (*position).into());
        let color = if cargo.current_weight > 0.0 {
            Color::from_rgba(238, 196, 99, 255)
        } else {
            Color::from_rgba(80, 210, 170, 255)
        };
        let cx = px + TILE_SIZE / 2.0;
        let cy = py + TILE_SIZE / 2.0;
        draw_circle(cx, cy, 8.0, Color::from_rgba(12, 14, 16, 230));
        draw_circle(cx, cy, 6.0, color);
        draw_circle_lines(cx, cy, 9.5, 2.0, WHITE);
        draw_text(&format!("P{}", porter.id), px + 1.0, py + 13.0, 12.0, BLACK);

        let label = match job.phase {
            JobPhase::FindParcel => "?",
            JobPhase::GoToParcel => "P",
            JobPhase::GoToDepot => "D",
            JobPhase::Done => "!",
        };
        draw_text(label, px + 10.0, py + 7.0, 10.0, WHITE);
    }
}

fn draw_player(world: &mut World, camera: Camera) {
    let mut query = world
        .query_filtered::<(&Position, &Stamina, &Cargo, &MovementState, &Momentum), With<Player>>();
    let Some((position, stamina, cargo, movement_state, _momentum)) = query.iter(world).next()
    else {
        return;
    };
    if !camera.contains(*position) {
        return;
    }

    let (px, py) = tile_to_screen(camera, (*position).into());
    let player_color = match movement_state.mode {
        crate::movement::MovementMode::Walking => Color::from_rgba(235, 235, 246, 255),
        crate::movement::MovementMode::Sprinting => Color::from_rgba(250, 218, 108, 255),
        crate::movement::MovementMode::Steady => Color::from_rgba(117, 205, 188, 255),
    };
    draw_rectangle(
        px + 2.0,
        py + 2.0,
        TILE_SIZE - 4.0,
        TILE_SIZE - 4.0,
        player_color,
    );
    draw_text(
        "@",
        px + 3.0,
        py + 14.0,
        18.0,
        Color::from_rgba(30, 35, 45, 255),
    );

    let stamina_ratio = stamina.current / stamina.max;
    draw_rectangle(
        px,
        py - 4.0,
        TILE_SIZE,
        3.0,
        Color::from_rgba(60, 20, 20, 255),
    );
    draw_rectangle(
        px,
        py - 4.0,
        TILE_SIZE * stamina_ratio,
        3.0,
        Color::from_rgba(90, 220, 130, 255),
    );

    if cargo.current_weight > 0.0 {
        draw_circle(
            px + 13.0,
            py + 4.0,
            3.0,
            Color::from_rgba(224, 154, 72, 255),
        );
    }
}
