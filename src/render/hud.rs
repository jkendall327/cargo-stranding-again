use bevy_ecs::prelude::*;
use macroquad::prelude::*;

use crate::render::layout::ui_x;
use crate::render::view_model::{PlayerHudSnapshot, PorterDebugRow};
use crate::resources::Camera;

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
const LEGEND_LINES: [&str; 5] = [
    "@ player",
    "Green/Gold circles: porters",
    "Orange boxes: available cargo",
    "Yellow outlined boxes: reserved cargo",
    "Small badges: carried cargo",
];

pub(super) fn draw_ui(world: &mut World, camera: Camera) {
    let hud = PlayerHudSnapshot::from_world(world, camera).expect("player exists for UI");
    let porter_rows = PorterDebugRow::all_from_world(world);
    let ui_x = ui_x(camera);
    draw_rectangle(
        ui_x - 12.0,
        0.0,
        screen_width() - ui_x + 12.0,
        screen_height(),
        Color::from_rgba(24, 27, 31, 245),
    );

    let mut y = 34.0;
    draw_text("Cargo Stranding Again", ui_x, y, 26.0, WHITE);
    y += 34.0;
    draw_text("Macroquad frame loop + Bevy ECS sim", ui_x, y, 18.0, GRAY);
    y += 38.0;

    ui_line(ui_x, &mut y, &format!("Turn: {}", hud.turn));
    ui_line(ui_x, &mut y, &format!("Energy time: {}", hud.timeline));
    ui_line(
        ui_x,
        &mut y,
        &format!(
            "Camera: {},{} | {}x{}",
            hud.camera.x, hud.camera.y, hud.camera.width, hud.camera.height
        ),
    );
    ui_line(
        ui_x,
        &mut y,
        &format!("Player: {}, {}", hud.position.x, hud.position.y),
    );
    ui_line(
        ui_x,
        &mut y,
        &format!(
            "Elevation: {} | water depth: {}",
            hud.elevation, hud.water_depth
        ),
    );
    ui_line(
        ui_x,
        &mut y,
        &format!("Stamina: {:.1}/{:.1}", hud.stamina_current, hud.stamina_max),
    );
    ui_line(
        ui_x,
        &mut y,
        &format!("Movement: {}", hud.movement_mode.label()),
    );
    ui_line(
        ui_x,
        &mut y,
        &format!(
            "Momentum: {:.1} {}",
            hud.momentum_amount,
            hud.momentum_direction
                .map(|direction| direction.label())
                .unwrap_or("none")
        ),
    );
    ui_line(ui_x, &mut y, &format!("Ready: {}", hud.ready_label));
    ui_line(
        ui_x,
        &mut y,
        &format!("Cargo: {:.1}/{:.1}", hud.cargo_current, hud.cargo_max),
    );
    ui_line(
        ui_x,
        &mut y,
        &format!("Delivered parcels: {}", hud.delivered_parcels),
    );
    y += 18.0;

    draw_text("Porters", ui_x, y, 22.0, WHITE);
    y += 28.0;
    draw_porter_debug_rows(&porter_rows, ui_x, &mut y);
    y += 18.0;

    draw_text("Controls", ui_x, y, 22.0, WHITE);
    y += 28.0;
    ui_lines(ui_x, &mut y, &CONTROL_HINTS);
    y += 18.0;

    draw_text("Legend", ui_x, y, 22.0, WHITE);
    y += 28.0;
    ui_lines(ui_x, &mut y, &LEGEND_LINES);
}

fn draw_porter_debug_rows(rows: &[PorterDebugRow], ui_x: f32, y: &mut f32) {
    for row in rows {
        ui_line(
            ui_x,
            y,
            &format!(
                "P{}: {},{} | {} | load {:.0} | {}",
                row.id, row.position.x, row.position.y, row.phase_label, row.load, row.ready_label
            ),
        );
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
