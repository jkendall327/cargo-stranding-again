use bevy_ecs::prelude::*;
use macroquad::prelude::*;

use crate::render::view_model::InventoryEntry;
use crate::resources::{GameScreen, PauseMenuEntry, PauseMenuState, PersistenceStatus};

pub(super) fn draw_game_state_overlay(world: &mut World) {
    match *world.resource::<GameScreen>() {
        GameScreen::Playing => {}
        GameScreen::PauseMenu => draw_pause_menu(
            world.resource::<PauseMenuState>(),
            world.resource::<PersistenceStatus>(),
        ),
        GameScreen::InventoryMenu => draw_inventory_menu(world),
        GameScreen::OptionsMenu => draw_options_menu(),
    }
}

fn draw_pause_menu(menu: &PauseMenuState, status: &PersistenceStatus) {
    draw_modal_panel(560.0, 360.0);

    let panel_x = (screen_width() - 560.0) / 2.0;
    let mut y = (screen_height() - 360.0) / 2.0 + 58.0;
    draw_text("Paused", panel_x + 42.0, y, 34.0, WHITE);
    y += 50.0;

    for entry in PauseMenuEntry::ALL {
        draw_menu_entry(panel_x + 42.0, y, entry.label(), menu.selected() == entry);
        y += 44.0;
    }

    if let Some(message) = &status.message {
        y += 14.0;
        draw_text(
            message,
            panel_x + 42.0,
            y,
            18.0,
            Color::from_rgba(190, 204, 214, 255),
        );
    }
}

fn draw_inventory_menu(world: &mut World) {
    draw_modal_panel(460.0, 360.0);

    let panel_x = (screen_width() - 460.0) / 2.0;
    let mut y = (screen_height() - 360.0) / 2.0 + 58.0;
    draw_text("Inventory", panel_x + 42.0, y, 34.0, WHITE);
    y += 50.0;

    let entries = InventoryEntry::all_from_world(world);
    if entries.is_empty() {
        draw_text(
            "No carried cargo.",
            panel_x + 42.0,
            y,
            22.0,
            Color::from_rgba(210, 216, 222, 255),
        );
        return;
    }

    for entry in entries {
        draw_menu_entry(panel_x + 42.0, y, &entry.label, entry.selected);
        y += 38.0;
    }
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
