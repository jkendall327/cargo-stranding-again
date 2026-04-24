use bevy_ecs::prelude::*;
use macroquad::prelude::*;

use crate::components::*;
use crate::map::Map;
use crate::resources::SimulationClock;

pub const TILE_SIZE: f32 = 16.0;
const UI_X: f32 = 980.0;

pub fn window_conf() -> Conf {
    Conf {
        window_title: "Cargo Stranding Again".to_owned(),
        // The map is 60x40 tiles at 16px, so 960x640. Leave enough room for
        // the right-hand debug panel by default instead of requiring resize.
        window_width: 1360,
        window_height: 760,
        high_dpi: true,
        ..Default::default()
    }
}

pub fn render(world: &mut World) {
    clear_background(Color::from_rgba(16, 18, 20, 255));

    let map = world.resource::<Map>();
    draw_map(map);

    draw_parcels(world);
    draw_agents(world);
    draw_player(world);
    draw_ui(world);
}

fn draw_map(map: &Map) {
    for y in 0..map.height {
        for x in 0..map.width {
            let terrain = map.terrain_at(x, y).expect("map iteration is in bounds");
            let px = x as f32 * TILE_SIZE;
            let py = y as f32 * TILE_SIZE;
            draw_rectangle(px, py, TILE_SIZE, TILE_SIZE, terrain.color());
            draw_rectangle_lines(
                px,
                py,
                TILE_SIZE,
                TILE_SIZE,
                1.0,
                Color::from_rgba(0, 0, 0, 45),
            );

            if matches!(terrain.glyph(), "D" | "w" | "^") {
                draw_text(
                    terrain.glyph(),
                    px + 4.0,
                    py + 12.5,
                    14.0,
                    Color::from_rgba(18, 18, 18, 200),
                );
            }
        }
    }
}

fn draw_parcels(world: &mut World) {
    let mut query = world.query::<(&Position, &CargoParcel, &ParcelState)>();
    for (position, parcel, state) in query.iter(world) {
        if !matches!(state, ParcelState::Loose | ParcelState::AssignedTo(_)) {
            continue;
        }
        let cx = position.x as f32 * TILE_SIZE + TILE_SIZE / 2.0;
        let cy = position.y as f32 * TILE_SIZE + TILE_SIZE / 2.0;
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

fn draw_agents(world: &mut World) {
    let mut query = world.query::<(&Position, &Agent, &Cargo, &AssignedJob)>();
    for (position, agent, cargo, job) in query.iter(world) {
        let px = position.x as f32 * TILE_SIZE;
        let py = position.y as f32 * TILE_SIZE;
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
        draw_text(&format!("P{}", agent.id), px + 1.0, py + 13.0, 12.0, BLACK);

        let label = match job.phase {
            JobPhase::FindParcel => "?",
            JobPhase::GoToParcel => "P",
            JobPhase::GoToDepot => "D",
            JobPhase::Done => "!",
        };
        draw_text(label, px + 10.0, py + 7.0, 10.0, WHITE);
    }
}

fn draw_player(world: &mut World) {
    let mut query = world.query_filtered::<(&Position, &Stamina, &Cargo), With<Player>>();
    let Some((position, stamina, cargo)) = query.iter(world).next() else {
        return;
    };

    let px = position.x as f32 * TILE_SIZE;
    let py = position.y as f32 * TILE_SIZE;
    draw_rectangle(
        px + 2.0,
        py + 2.0,
        TILE_SIZE - 4.0,
        TILE_SIZE - 4.0,
        Color::from_rgba(235, 235, 246, 255),
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

fn draw_ui(world: &mut World) {
    draw_rectangle(
        UI_X - 12.0,
        0.0,
        screen_width() - UI_X + 12.0,
        screen_height(),
        Color::from_rgba(24, 27, 31, 245),
    );

    let clock = *world.resource::<SimulationClock>();
    let mut player_query = world.query_filtered::<(&Position, &Stamina, &Cargo), With<Player>>();
    let (player_position, stamina, cargo) = player_query
        .iter(world)
        .next()
        .expect("player exists for UI");

    let mut y = 34.0;
    draw_text("Cargo Stranding Again", UI_X, y, 26.0, WHITE);
    y += 34.0;
    draw_text("Macroquad frame loop + Bevy ECS sim", UI_X, y, 18.0, GRAY);
    y += 38.0;

    ui_line(&mut y, &format!("Turn: {}", clock.turn));
    ui_line(
        &mut y,
        &format!("Player: {}, {}", player_position.x, player_position.y),
    );
    ui_line(
        &mut y,
        &format!("Stamina: {:.1}/{:.1}", stamina.current, stamina.max),
    );
    ui_line(
        &mut y,
        &format!("Cargo: {:.1}/{:.1}", cargo.current_weight, cargo.max_weight),
    );
    ui_line(
        &mut y,
        &format!("Delivered parcels: {}", clock.delivered_parcels),
    );
    y += 18.0;

    draw_text("Porters", UI_X, y, 22.0, WHITE);
    y += 28.0;
    draw_agent_debug(world, &mut y);
    y += 18.0;

    draw_text("Controls", UI_X, y, 22.0, WHITE);
    y += 28.0;
    ui_line(&mut y, "WASD / Arrows: move one tile");
    ui_line(&mut y, "Space / .: wait and recover stamina");
    ui_line(&mut y, "Turns advance only on valid action");
    ui_line(&mut y, "Water blocks movement");
    ui_line(&mut y, "Mud/Rock cost more stamina");
    ui_line(&mut y, "Roads are cheap traversal");
    y += 18.0;

    draw_text("Legend", UI_X, y, 22.0, WHITE);
    y += 28.0;
    ui_line(&mut y, "@ player");
    ui_line(&mut y, "Green/Gold circles: porters");
    ui_line(&mut y, "Orange boxes: loose cargo");
    ui_line(&mut y, "D: depot");
}

fn draw_agent_debug(world: &mut World, y: &mut f32) {
    let mut query = world.query::<(&Position, &Agent, &Cargo, &AssignedJob, &StepCooldown)>();
    let mut rows = query.iter(world).collect::<Vec<_>>();
    rows.sort_by_key(|(_, agent, _, _, _)| agent.id);

    for (position, agent, cargo, job, cooldown) in rows {
        let phase = match job.phase {
            JobPhase::FindParcel => "finding",
            JobPhase::GoToParcel => "to parcel",
            JobPhase::GoToDepot => "to depot",
            JobPhase::Done => "done",
        };
        ui_line(
            y,
            &format!(
                "P{}: {},{} | {} | load {:.0} | cd {}",
                agent.id, position.x, position.y, phase, cargo.current_weight, cooldown.frames
            ),
        );
    }
}

fn ui_line(y: &mut f32, text: &str) {
    draw_text(text, UI_X, *y, 18.0, Color::from_rgba(220, 225, 230, 255));
    *y += 24.0;
}
