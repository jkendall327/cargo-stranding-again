use bevy_ecs::prelude::*;
use macroquad::prelude::*;

use crate::components::*;
use crate::map::Map;
use crate::resources::{
    Camera, EnergyTimeline, GameScreen, InventoryMenuState, PauseMenuEntry, PauseMenuState,
    SimulationClock, DEFAULT_CAMERA_TILE_SPAN,
};

pub const TILE_SIZE: f32 = 16.0;
const VIEWPORT_X: f32 = 24.0;
const VIEWPORT_Y: f32 = 24.0;
const UI_GAP: f32 = 28.0;
const CONTROL_HINTS: [&str; 9] = [
    "WASD / Arrows: move one tile",
    "Space / .: wait and recover stamina",
    "E: pick up loose cargo here",
    "I / Tab: open inventory",
    "Shift: cycle walk / sprint / steady",
    "Esc: pause / resume",
    "Energy advances only on valid action",
    "Water blocks movement",
    "Sprint is fast; steady saves stamina",
];
const LEGEND_LINES: [&str; 4] = [
    "@ player",
    "Green/Gold circles: porters",
    "Orange boxes: loose cargo",
    "D: depot",
];

pub fn window_conf() -> Conf {
    Conf {
        window_title: "Cargo Stranding Again".to_owned(),
        // The camera shows a configurable tile square while the debug panel
        // stays fixed to the right.
        window_width: (VIEWPORT_X * 2.0
            + DEFAULT_CAMERA_TILE_SPAN as f32 * TILE_SIZE
            + UI_GAP
            + 360.0) as i32,
        window_height: 760,
        high_dpi: true,
        ..Default::default()
    }
}

pub fn render(world: &mut World) {
    clear_background(Color::from_rgba(16, 18, 20, 255));

    update_camera(world);
    let camera = *world.resource::<Camera>();

    draw_viewport_background(camera);

    {
        let map = world.resource::<Map>();
        draw_map(map, camera);
    }

    draw_parcels(world, camera);
    draw_agents(world, camera);
    draw_player(world, camera);
    draw_viewport_frame(camera);
    draw_ui(world, camera);
    draw_game_state_overlay(world);
}

fn update_camera(world: &mut World) {
    let player_position = {
        let mut query = world.query_filtered::<&Position, With<Player>>();
        query.iter(world).next().copied()
    };
    let Some(player_position) = player_position else {
        return;
    };

    let (map_width, map_height) = {
        let map = world.resource::<Map>();
        (map.width, map.height)
    };
    world
        .resource_mut::<Camera>()
        .center_on(player_position, map_width, map_height);
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
    for y in camera.y..(camera.y + camera.height).min(map.height) {
        for x in camera.x..(camera.x + camera.width).min(map.width) {
            let terrain = map.terrain_at(x, y).expect("map iteration is in bounds");
            let (px, py) = tile_to_screen(camera, x, y);
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

fn draw_parcels(world: &mut World, camera: Camera) {
    let mut query = world.query::<(&Position, &CargoParcel, &ParcelState)>();
    for (position, parcel, state) in query.iter(world) {
        if !matches!(state, ParcelState::Loose | ParcelState::AssignedTo(_)) {
            continue;
        }
        if !camera.contains(*position) {
            continue;
        }
        let (px, py) = tile_to_screen(camera, position.x, position.y);
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

fn draw_agents(world: &mut World, camera: Camera) {
    let mut query = world.query::<(&Position, &Agent, &Cargo, &AssignedJob)>();
    for (position, agent, cargo, job) in query.iter(world) {
        if !camera.contains(*position) {
            continue;
        }
        let (px, py) = tile_to_screen(camera, position.x, position.y);
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

    let (px, py) = tile_to_screen(camera, position.x, position.y);
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

fn draw_ui(world: &mut World, camera: Camera) {
    let ui_x = ui_x(camera);
    draw_rectangle(
        ui_x - 12.0,
        0.0,
        screen_width() - ui_x + 12.0,
        screen_height(),
        Color::from_rgba(24, 27, 31, 245),
    );

    let clock = *world.resource::<SimulationClock>();
    let timeline = *world.resource::<EnergyTimeline>();
    let mut player_query = world.query_filtered::<(
        &Position,
        &Stamina,
        &Cargo,
        &MovementState,
        &Momentum,
        &ActionEnergy,
    ), With<Player>>();
    let (player_position, stamina, cargo, movement_state, momentum, player_energy) = player_query
        .iter(world)
        .next()
        .expect("player exists for UI");

    let mut y = 34.0;
    draw_text("Cargo Stranding Again", ui_x, y, 26.0, WHITE);
    y += 34.0;
    draw_text("Macroquad frame loop + Bevy ECS sim", ui_x, y, 18.0, GRAY);
    y += 38.0;

    ui_line(ui_x, &mut y, &format!("Turn: {}", clock.turn));
    ui_line(ui_x, &mut y, &format!("Energy time: {}", timeline.now));
    ui_line(
        ui_x,
        &mut y,
        &format!(
            "Camera: {},{} | {}x{}",
            camera.x, camera.y, camera.width, camera.height
        ),
    );
    ui_line(
        ui_x,
        &mut y,
        &format!("Player: {}, {}", player_position.x, player_position.y),
    );
    ui_line(
        ui_x,
        &mut y,
        &format!("Stamina: {:.1}/{:.1}", stamina.current, stamina.max),
    );
    ui_line(
        ui_x,
        &mut y,
        &format!("Movement: {}", movement_state.mode.label()),
    );
    ui_line(
        ui_x,
        &mut y,
        &format!(
            "Momentum: {:.1} {}",
            momentum.amount,
            momentum
                .direction
                .map(|direction| direction.label())
                .unwrap_or("none")
        ),
    );
    ui_line(
        ui_x,
        &mut y,
        &format!("Ready: {}", ready_label(*player_energy, timeline.now)),
    );
    ui_line(
        ui_x,
        &mut y,
        &format!("Cargo: {:.1}/{:.1}", cargo.current_weight, cargo.max_weight),
    );
    ui_line(
        ui_x,
        &mut y,
        &format!("Delivered parcels: {}", clock.delivered_parcels),
    );
    y += 18.0;

    draw_text("Porters", ui_x, y, 22.0, WHITE);
    y += 28.0;
    draw_agent_debug(world, ui_x, &mut y);
    y += 18.0;

    draw_text("Controls", ui_x, y, 22.0, WHITE);
    y += 28.0;
    ui_lines(ui_x, &mut y, &CONTROL_HINTS);
    y += 18.0;

    draw_text("Legend", ui_x, y, 22.0, WHITE);
    y += 28.0;
    ui_lines(ui_x, &mut y, &LEGEND_LINES);
}

fn draw_game_state_overlay(world: &mut World) {
    match *world.resource::<GameScreen>() {
        GameScreen::Playing => {}
        GameScreen::PauseMenu => draw_pause_menu(world.resource::<PauseMenuState>()),
        GameScreen::InventoryMenu => draw_inventory_menu(world),
        GameScreen::OptionsMenu => draw_options_menu(),
    }
}

fn draw_pause_menu(menu: &PauseMenuState) {
    draw_modal_panel(380.0, 250.0);

    let panel_x = (screen_width() - 380.0) / 2.0;
    let mut y = (screen_height() - 250.0) / 2.0 + 58.0;
    draw_text("Paused", panel_x + 42.0, y, 34.0, WHITE);
    y += 50.0;

    for entry in PauseMenuEntry::ALL {
        draw_menu_entry(panel_x + 42.0, y, entry.label(), menu.selected() == entry);
        y += 44.0;
    }
}

fn draw_inventory_menu(world: &mut World) {
    draw_modal_panel(460.0, 360.0);

    let panel_x = (screen_width() - 460.0) / 2.0;
    let mut y = (screen_height() - 360.0) / 2.0 + 58.0;
    draw_text("Inventory", panel_x + 42.0, y, 34.0, WHITE);
    y += 50.0;

    let entries = inventory_entries(world);
    if entries.is_empty() {
        draw_text(
            "No carried parcels.",
            panel_x + 42.0,
            y,
            22.0,
            Color::from_rgba(210, 216, 222, 255),
        );
        return;
    }

    let selected_index = world.resource::<InventoryMenuState>().selected_index();
    for (index, entry) in entries.iter().enumerate() {
        let label = format!("Parcel {:.0} weight", entry.weight);
        draw_menu_entry(panel_x + 42.0, y, &label, selected_index == index);
        y += 38.0;
    }
}

struct InventoryEntry {
    entity: Entity,
    weight: f32,
}

fn inventory_entries(world: &mut World) -> Vec<InventoryEntry> {
    let Some(player_entity) = player_entity(world) else {
        return Vec::new();
    };

    let mut query = world.query::<(Entity, &CargoParcel, &ParcelState)>();
    let mut entries = query
        .iter(world)
        .filter_map(|(entity, parcel, state)| {
            if *state == ParcelState::CarriedBy(player_entity) {
                Some(InventoryEntry {
                    entity,
                    weight: parcel.weight,
                })
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.entity.to_bits());
    entries
}

fn player_entity(world: &mut World) -> Option<Entity> {
    let mut query = world.query_filtered::<Entity, With<Player>>();
    query.iter(world).next()
}

fn draw_options_menu() {
    draw_modal_panel(420.0, 250.0);

    let panel_x = (screen_width() - 420.0) / 2.0;
    let mut y = (screen_height() - 250.0) / 2.0 + 58.0;
    draw_text("Options", panel_x + 42.0, y, 34.0, WHITE);
    y += 52.0;
    draw_text(
        "No options yet.",
        panel_x + 42.0,
        y,
        22.0,
        Color::from_rgba(210, 216, 222, 255),
    );
}

fn draw_modal_panel(width: f32, height: f32) {
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::from_rgba(0, 0, 0, 145),
    );

    let panel_x = (screen_width() - width) / 2.0;
    let panel_y = (screen_height() - height) / 2.0;
    draw_rectangle(
        panel_x,
        panel_y,
        width,
        height,
        Color::from_rgba(30, 35, 41, 245),
    );
    draw_rectangle_lines(
        panel_x,
        panel_y,
        width,
        height,
        2.0,
        Color::from_rgba(220, 226, 232, 180),
    );
}

fn draw_menu_entry(x: f32, y: f32, label: &str, selected: bool) {
    let color = if selected {
        Color::from_rgba(252, 204, 84, 255)
    } else {
        Color::from_rgba(220, 225, 230, 255)
    };
    let marker = if selected { ">" } else { " " };
    draw_text(marker, x, y, 26.0, color);
    draw_text(label, x + 34.0, y, 26.0, color);
}

fn draw_agent_debug(world: &mut World, ui_x: f32, y: &mut f32) {
    let timeline = world.resource::<EnergyTimeline>().now;
    let mut query = world.query::<(&Position, &Agent, &Cargo, &AssignedJob, &ActionEnergy)>();
    let mut rows = query.iter(world).collect::<Vec<_>>();
    rows.sort_by_key(|(_, agent, _, _, _)| agent.id);

    for (position, agent, cargo, job, energy) in rows {
        let phase = match job.phase {
            JobPhase::FindParcel => "finding",
            JobPhase::GoToParcel => "to parcel",
            JobPhase::GoToDepot => "to depot",
            JobPhase::Done => "done",
        };
        ui_line(
            ui_x,
            y,
            &format!(
                "P{}: {},{} | {} | load {:.0} | {}",
                agent.id,
                position.x,
                position.y,
                phase,
                cargo.current_weight,
                ready_label(*energy, timeline)
            ),
        );
    }
}

fn ready_label(energy: ActionEnergy, now: u64) -> String {
    if energy.is_ready(now) {
        "ready".to_owned()
    } else {
        format!("ready in {}", energy.ready_at - now)
    }
}

fn ui_line(ui_x: f32, y: &mut f32, text: &str) {
    draw_text(text, ui_x, *y, 18.0, Color::from_rgba(220, 225, 230, 255));
    *y += 24.0;
}

fn ui_lines(ui_x: f32, y: &mut f32, lines: &[&str]) {
    for line in lines {
        ui_line(ui_x, y, line);
    }
}

fn tile_to_screen(camera: Camera, x: i32, y: i32) -> (f32, f32) {
    (
        VIEWPORT_X + (x - camera.x) as f32 * TILE_SIZE,
        VIEWPORT_Y + (y - camera.y) as f32 * TILE_SIZE,
    )
}

fn viewport_width(camera: Camera) -> f32 {
    camera.width as f32 * TILE_SIZE
}

fn viewport_height(camera: Camera) -> f32 {
    camera.height as f32 * TILE_SIZE
}

fn ui_x(camera: Camera) -> f32 {
    VIEWPORT_X + viewport_width(camera) + UI_GAP
}
