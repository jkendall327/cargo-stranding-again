use bevy_ecs::prelude::*;
use macroquad::prelude::*;

use crate::components::*;
use crate::map::{Map, Terrain};
use crate::render::layout::{
    tile_to_screen, viewport_height, viewport_width, VIEWPORT_X, VIEWPORT_Y,
};
use crate::render::view_model::{
    ActorCargoRender, CargoHolderKind, LooseCargoRender, LooseCargoState,
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

    let loose_cargo = LooseCargoRender::all_from_world(world);
    let actor_cargo = ActorCargoRender::all_from_world(world);
    draw_loose_cargo(&loose_cargo, camera);
    draw_porters(world, camera, &actor_cargo);
    draw_player(world, camera, &actor_cargo);
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
        } else if matches!(tile.terrain.glyph(), 'D' | '^') {
            draw_text(
                &tile.terrain.glyph().to_string(),
                px + 4.0,
                py + 12.5,
                14.0,
                Color::from_rgba(18, 18, 18, 200),
            );
        }
    }
}

fn terrain_color(terrain: Terrain, elevation: i16, water_depth: u8) -> Color {
    let [r, g, b, a] = terrain.definition().color_rgba;
    let base = Color::from_rgba(r, g, b, a);
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

fn draw_loose_cargo(cargo: &[LooseCargoRender], camera: Camera) {
    for item in cargo {
        if !camera.contains(item.position) {
            continue;
        }
        let (px, py) = tile_to_screen(camera, item.position.into());
        let cx = px + TILE_SIZE / 2.0;
        let cy = py + TILE_SIZE / 2.0;
        let (color, outline, label) = match item.state {
            LooseCargoState::Available => (Color::from_rgba(224, 154, 72, 255), None, None),
            LooseCargoState::Reserved { porter_id } => (
                Color::from_rgba(252, 204, 84, 255),
                Some(Color::from_rgba(255, 244, 176, 255)),
                porter_id.map(|id| format!("P{}", id)),
            ),
        };
        draw_rectangle(cx - 5.0, cy - 5.0, 10.0, 10.0, color);
        if let Some(outline) = outline {
            draw_rectangle_lines(cx - 6.5, cy - 6.5, 13.0, 13.0, 2.0, outline);
        }
        draw_text(
            &format!("{:.0}", item.weight),
            cx - 4.0,
            cy + 4.0,
            10.0,
            BLACK,
        );
        if let Some(label) = label {
            draw_text(&label, px + 1.0, py - 1.0, 10.0, WHITE);
        }
    }
}

fn draw_porters(world: &mut World, camera: Camera, actor_cargo: &[ActorCargoRender]) {
    let mut query = world.query::<(Entity, &Position, &Porter, &AssignedJob)>();
    let porters = query
        .iter(world)
        .map(|(entity, position, porter, job)| (entity, *position, porter.id, job.phase()))
        .collect::<Vec<_>>();
    for (entity, position, porter_id, phase) in porters {
        if !camera.contains(position) {
            continue;
        }
        let carried = actor_cargo
            .iter()
            .find(|cargo| cargo.holder == entity)
            .filter(|cargo| matches!(cargo.holder_kind, CargoHolderKind::Porter(_)));
        let (px, py) = tile_to_screen(camera, position.into());
        let color = if carried.is_some_and(|cargo| cargo.total_weight > 0.0) {
            Color::from_rgba(238, 196, 99, 255)
        } else {
            Color::from_rgba(80, 210, 170, 255)
        };
        let cx = px + TILE_SIZE / 2.0;
        let cy = py + TILE_SIZE / 2.0;
        draw_circle(cx, cy, 8.0, Color::from_rgba(12, 14, 16, 230));
        draw_circle(cx, cy, 6.0, color);
        draw_circle_lines(cx, cy, 9.5, 2.0, WHITE);
        draw_text(&format!("P{}", porter_id), px + 1.0, py + 13.0, 12.0, BLACK);

        let label = match phase {
            JobPhase::FindParcel => "?",
            JobPhase::GoToParcel => "P",
            JobPhase::GoToDepot => "D",
            JobPhase::Done => "!",
        };
        draw_text(label, px + 10.0, py + 7.0, 10.0, WHITE);
        if let Some(cargo) = carried {
            draw_actor_cargo_badge(px, py, cargo);
        }
    }
}

fn draw_player(world: &mut World, camera: Camera, actor_cargo: &[ActorCargoRender]) {
    let mut query = world
        .query_filtered::<(Entity, &Position, &Stamina, &MovementState, &Momentum), With<Player>>();
    let Some((entity, position, stamina, movement_state, _momentum)) = query.iter(world).next()
    else {
        return;
    };
    let position = *position;
    let stamina = *stamina;
    let movement_state = *movement_state;
    if !camera.contains(position) {
        return;
    }

    let (px, py) = tile_to_screen(camera, position.into());
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

    if let Some(cargo) = actor_cargo
        .iter()
        .find(|cargo| cargo.holder == entity)
        .filter(|cargo| matches!(cargo.holder_kind, CargoHolderKind::Player))
    {
        draw_actor_cargo_badge(px, py, cargo);
    }
}

fn draw_actor_cargo_badge(px: f32, py: f32, cargo: &ActorCargoRender) {
    let badge_color = if cargo.has_contained_items {
        Color::from_rgba(116, 205, 239, 255)
    } else {
        Color::from_rgba(224, 154, 72, 255)
    };
    let label = if cargo.parcel_count > 0 {
        cargo.parcel_count.to_string()
    } else {
        format!("{:.0}", cargo.total_weight)
    };
    draw_circle(px + 13.0, py + 4.0, 5.0, Color::from_rgba(12, 14, 16, 235));
    draw_circle(px + 13.0, py + 4.0, 3.8, badge_color);
    draw_text(&label, px + 10.0, py + 7.0, 8.0, BLACK);
}
