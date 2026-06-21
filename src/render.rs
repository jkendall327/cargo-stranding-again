use bevy::prelude::*;
use bevy_ecs::prelude::{With, World as SimWorld};

mod layout;
mod view_model;

use crate::app::Game;
use crate::components::*;
use crate::map::{Map, Terrain, TileCoord};
use crate::render::layout::{
    tile_to_screen, ui_x, viewport_height, viewport_width, VIEWPORT_X, VIEWPORT_Y,
};
use crate::render::view_model::{
    ActorCargoRender, CargoHolderKind, InventoryEntry, LooseCargoRender, LooseCargoState,
    PlayerHudSnapshot, PorterDebugRow,
};
use crate::resources::{
    Camera, GameScreen, PauseMenuEntry, PauseMenuState, PersistenceStatus, DEFAULT_CAMERA_TILE_SPAN,
};

pub const TILE_SIZE: f32 = 16.0;
const WINDOW_HEIGHT: f32 = 1160.0;
const UI_PANEL_WIDTH: f32 = 1220.0;
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
    "Teal/Gold squares: porters",
    "Orange boxes: available cargo",
    "Yellow outlined boxes: reserved cargo",
    "Small badges: carried cargo",
];

#[derive(Component)]
pub(crate) struct RenderedFrameEntity;

pub fn initial_window_width() -> u32 {
    layout::window_width_for_camera(DEFAULT_CAMERA_TILE_SPAN) as u32
}

pub fn initial_window_height() -> u32 {
    WINDOW_HEIGHT as u32
}

pub fn color_rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::srgba_u8(r, g, b, a)
}

pub(crate) fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

pub(crate) fn sync_render(
    mut commands: Commands,
    mut game: ResMut<Game>,
    rendered: Query<Entity, With<RenderedFrameEntity>>,
) {
    for entity in &rendered {
        commands.entity(entity).despawn();
    }

    update_camera(game.world_mut());
    let camera = *game.world().resource::<Camera>();

    draw_viewport_background(&mut commands, camera);
    draw_map(&mut commands, game.world(), camera);

    let loose_cargo = LooseCargoRender::all_from_world(game.world_mut());
    let actor_cargo = ActorCargoRender::all_from_world(game.world_mut());
    draw_loose_cargo(&mut commands, &loose_cargo, camera);
    draw_porters(&mut commands, game.world_mut(), camera, &actor_cargo);
    draw_player(&mut commands, game.world_mut(), camera, &actor_cargo);
    draw_viewport_frame(&mut commands, camera);
    draw_hud(&mut commands, game.world_mut(), camera);
    draw_game_state_overlay(&mut commands, game.world_mut());
}

fn update_camera(world: &mut SimWorld) {
    let player_position = {
        let mut query = world.query_filtered::<&Position, bevy_ecs::prelude::With<Player>>();
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

fn draw_viewport_background(commands: &mut Commands, camera: Camera) {
    spawn_rect(
        commands,
        VIEWPORT_X - 4.0,
        VIEWPORT_Y - 4.0,
        viewport_width(camera) + 8.0,
        viewport_height(camera) + 8.0,
        0.0,
        color_rgba(8, 10, 12, 255),
    );
}

fn draw_viewport_frame(commands: &mut Commands, camera: Camera) {
    draw_rect_outline(
        commands,
        VIEWPORT_X - 1.0,
        VIEWPORT_Y - 1.0,
        viewport_width(camera) + 2.0,
        viewport_height(camera) + 2.0,
        2.0,
        color_rgba(215, 220, 226, 180),
    );
}

fn draw_map(commands: &mut Commands, world: &SimWorld, camera: Camera) {
    let map = world.resource::<Map>();
    for coord in map.visible_tiles(camera.origin_coord(), camera.width, camera.height) {
        let tile = map
            .tile_at_coord(coord)
            .expect("visible tile iteration is in bounds");
        let (px, py) = tile_to_screen(camera, coord);
        spawn_rect(
            commands,
            px,
            py,
            TILE_SIZE,
            TILE_SIZE,
            1.0,
            terrain_color(tile.terrain, tile.elevation, tile.water_depth),
        );
        draw_rect_outline(
            commands,
            px,
            py,
            TILE_SIZE,
            TILE_SIZE,
            1.0,
            color_rgba(0, 0, 0, 45),
        );

        spawn_text(
            commands,
            &tile.elevation.to_string(),
            px + 1.5,
            py + 7.0,
            8.0,
            5.0,
            color_rgba(245, 245, 235, 175),
        );

        if tile.water_depth > 0 {
            spawn_text(
                commands,
                &tile.water_depth.to_string(),
                px + 10.0,
                py + 14.0,
                8.0,
                5.0,
                color_rgba(210, 235, 255, 210),
            );
        } else if matches!(tile.terrain.glyph(), 'D' | '^') {
            spawn_text(
                commands,
                &tile.terrain.glyph().to_string(),
                px + 4.0,
                py + 12.5,
                14.0,
                5.0,
                color_rgba(18, 18, 18, 200),
            );
        }
    }
}

fn terrain_color(terrain: Terrain, elevation: i16, water_depth: u8) -> Color {
    let [r, g, b, a] = terrain.definition().color_rgba;
    let base = color_rgba(r, g, b, a);
    if terrain == Terrain::Water {
        shade_color(base, -(f32::from(water_depth) * 0.09))
    } else {
        shade_color(base, (f32::from(elevation) - 4.5) * 0.045)
    }
}

fn shade_color(color: Color, amount: f32) -> Color {
    let Srgba {
        red,
        green,
        blue,
        alpha,
    } = color.to_srgba();
    Color::srgba(
        (red + amount).clamp(0.0, 1.0),
        (green + amount).clamp(0.0, 1.0),
        (blue + amount).clamp(0.0, 1.0),
        alpha,
    )
}

fn draw_loose_cargo(commands: &mut Commands, cargo: &[LooseCargoRender], camera: Camera) {
    for item in cargo {
        if !camera.contains(item.position) {
            continue;
        }
        let (px, py) = tile_to_screen(camera, TileCoord::from(item.position));
        let cx = px + TILE_SIZE / 2.0;
        let cy = py + TILE_SIZE / 2.0;
        let (color, outline, label) = match item.state {
            LooseCargoState::Available => (color_rgba(224, 154, 72, 255), None, None),
            LooseCargoState::Reserved { porter_id } => (
                color_rgba(252, 204, 84, 255),
                Some(color_rgba(255, 244, 176, 255)),
                porter_id.map(|id| format!("P{id}")),
            ),
        };
        spawn_rect(commands, cx - 5.0, cy - 5.0, 10.0, 10.0, 10.0, color);
        if let Some(outline) = outline {
            draw_rect_outline(commands, cx - 6.5, cy - 6.5, 13.0, 13.0, 2.0, outline);
        }
        spawn_text(
            commands,
            &format!("{:.0}", item.weight),
            cx - 4.0,
            cy + 4.0,
            10.0,
            12.0,
            Color::BLACK,
        );
        if let Some(label) = label {
            spawn_text(
                commands,
                &label,
                px + 1.0,
                py - 1.0,
                10.0,
                12.0,
                Color::WHITE,
            );
        }
    }
}

fn draw_porters(
    commands: &mut Commands,
    world: &mut SimWorld,
    camera: Camera,
    actor_cargo: &[ActorCargoRender],
) {
    let mut query = world.query::<(bevy_ecs::prelude::Entity, &Position, &Porter, &AssignedJob)>();
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
        let (px, py) = tile_to_screen(camera, TileCoord::from(position));
        let color = if carried.is_some_and(|cargo| cargo.total_weight > 0.0) {
            color_rgba(238, 196, 99, 255)
        } else {
            color_rgba(80, 210, 170, 255)
        };
        spawn_rect(commands, px + 3.0, py + 3.0, 10.0, 10.0, 15.0, color);
        draw_rect_outline(commands, px + 2.0, py + 2.0, 12.0, 12.0, 1.5, Color::WHITE);
        spawn_text(
            commands,
            &format!("P{porter_id}"),
            px + 1.0,
            py + 13.0,
            12.0,
            16.0,
            Color::BLACK,
        );

        let label = match phase {
            JobPhase::FindParcel => "?",
            JobPhase::GoToParcel => "P",
            JobPhase::GoToDepot => "D",
            JobPhase::Done => "!",
        };
        spawn_text(
            commands,
            label,
            px + 10.0,
            py + 7.0,
            10.0,
            16.0,
            Color::WHITE,
        );
        if let Some(cargo) = carried {
            draw_actor_cargo_badge(commands, px, py, cargo);
        }
    }
}

fn draw_player(
    commands: &mut Commands,
    world: &mut SimWorld,
    camera: Camera,
    actor_cargo: &[ActorCargoRender],
) {
    let mut query = world.query_filtered::<(
        bevy_ecs::prelude::Entity,
        &Position,
        &Stamina,
        &MovementState,
        &Momentum,
    ), bevy_ecs::prelude::With<Player>>();
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

    let (px, py) = tile_to_screen(camera, TileCoord::from(position));
    let player_color = match movement_state.mode {
        crate::movement::MovementMode::Walking => color_rgba(235, 235, 246, 255),
        crate::movement::MovementMode::Sprinting => color_rgba(250, 218, 108, 255),
        crate::movement::MovementMode::Steady => color_rgba(117, 205, 188, 255),
    };
    spawn_rect(
        commands,
        px + 2.0,
        py + 2.0,
        TILE_SIZE - 4.0,
        TILE_SIZE - 4.0,
        20.0,
        player_color,
    );
    spawn_text(
        commands,
        "@",
        px + 3.0,
        py + 14.0,
        18.0,
        21.0,
        color_rgba(30, 35, 45, 255),
    );

    let stamina_ratio = stamina.current / stamina.max;
    spawn_rect(
        commands,
        px,
        py - 4.0,
        TILE_SIZE,
        3.0,
        22.0,
        color_rgba(60, 20, 20, 255),
    );
    spawn_rect(
        commands,
        px,
        py - 4.0,
        TILE_SIZE * stamina_ratio,
        3.0,
        23.0,
        color_rgba(90, 220, 130, 255),
    );

    if let Some(cargo) = actor_cargo
        .iter()
        .find(|cargo| cargo.holder == entity)
        .filter(|cargo| matches!(cargo.holder_kind, CargoHolderKind::Player))
    {
        draw_actor_cargo_badge(commands, px, py, cargo);
    }
}

fn draw_actor_cargo_badge(commands: &mut Commands, px: f32, py: f32, cargo: &ActorCargoRender) {
    let badge_color = if cargo.has_contained_items {
        color_rgba(116, 205, 239, 255)
    } else {
        color_rgba(224, 154, 72, 255)
    };
    let label = if cargo.parcel_count > 0 {
        cargo.parcel_count.to_string()
    } else {
        format!("{:.0}", cargo.total_weight)
    };
    spawn_rect(
        commands,
        px + 9.0,
        py,
        8.0,
        8.0,
        25.0,
        color_rgba(12, 14, 16, 235),
    );
    spawn_rect(commands, px + 10.0, py + 1.0, 6.0, 6.0, 26.0, badge_color);
    spawn_text(
        commands,
        &label,
        px + 10.0,
        py + 7.0,
        8.0,
        27.0,
        Color::BLACK,
    );
}

fn draw_hud(commands: &mut Commands, world: &mut SimWorld, camera: Camera) {
    let Some(hud) = PlayerHudSnapshot::from_world(world, camera) else {
        return;
    };
    let porter_rows = PorterDebugRow::all_from_world(world);
    let panel_x = ui_x(camera);
    spawn_rect(
        commands,
        panel_x - 12.0,
        0.0,
        UI_PANEL_WIDTH + 24.0,
        WINDOW_HEIGHT,
        30.0,
        color_rgba(24, 27, 31, 245),
    );

    let mut y = 34.0;
    hud_text(
        commands,
        panel_x,
        &mut y,
        "Cargo Stranding Again",
        26.0,
        Color::WHITE,
    );
    hud_text(
        commands,
        panel_x,
        &mut y,
        "Bevy app + Bevy ECS simulation",
        18.0,
        Color::srgb(0.5, 0.5, 0.5),
    );
    y += 14.0;
    hud_text(
        commands,
        panel_x,
        &mut y,
        &format!("Turn: {}", hud.turn),
        18.0,
        Color::WHITE,
    );
    hud_text(
        commands,
        panel_x,
        &mut y,
        &format!("Energy time: {}", hud.timeline),
        18.0,
        Color::WHITE,
    );
    hud_text(
        commands,
        panel_x,
        &mut y,
        &format!(
            "Camera: {},{} | {}x{}",
            hud.camera.x, hud.camera.y, hud.camera.width, hud.camera.height
        ),
        18.0,
        Color::WHITE,
    );
    hud_text(
        commands,
        panel_x,
        &mut y,
        &format!("Player: {}, {}", hud.position.x, hud.position.y),
        18.0,
        Color::WHITE,
    );
    hud_text(
        commands,
        panel_x,
        &mut y,
        &format!(
            "Elevation: {} | water depth: {}",
            hud.elevation, hud.water_depth
        ),
        18.0,
        Color::WHITE,
    );
    hud_text(
        commands,
        panel_x,
        &mut y,
        &format!("Stamina: {:.1}/{:.1}", hud.stamina_current, hud.stamina_max),
        18.0,
        Color::WHITE,
    );
    hud_text(
        commands,
        panel_x,
        &mut y,
        &format!("Movement: {}", hud.movement_mode.label()),
        18.0,
        Color::WHITE,
    );
    hud_text(
        commands,
        panel_x,
        &mut y,
        &format!(
            "Momentum: {:.1} {}",
            hud.momentum_amount,
            hud.momentum_direction
                .map(|direction| direction.label())
                .unwrap_or("none")
        ),
        18.0,
        Color::WHITE,
    );
    hud_text(
        commands,
        panel_x,
        &mut y,
        &format!("Ready: {}", hud.ready_label),
        18.0,
        Color::WHITE,
    );
    hud_text(
        commands,
        panel_x,
        &mut y,
        &format!("Cargo: {:.1}/{:.1}", hud.cargo_current, hud.cargo_max),
        18.0,
        Color::WHITE,
    );
    hud_text(
        commands,
        panel_x,
        &mut y,
        &format!("Delivered parcels: {}", hud.delivered_parcels),
        18.0,
        Color::WHITE,
    );
    y += 18.0;

    hud_text(commands, panel_x, &mut y, "Porters", 22.0, Color::WHITE);
    for row in porter_rows {
        hud_text(
            commands,
            panel_x,
            &mut y,
            &format!(
                "P{}: {},{} | {} | load {:.0} | {}",
                row.id, row.position.x, row.position.y, row.phase_label, row.load, row.ready_label
            ),
            18.0,
            Color::WHITE,
        );
    }
    y += 18.0;

    hud_text(commands, panel_x, &mut y, "Controls", 22.0, Color::WHITE);
    for line in CONTROL_HINTS {
        hud_text(commands, panel_x, &mut y, line, 18.0, Color::WHITE);
    }
    y += 18.0;

    hud_text(commands, panel_x, &mut y, "Legend", 22.0, Color::WHITE);
    for line in LEGEND_LINES {
        hud_text(commands, panel_x, &mut y, line, 18.0, Color::WHITE);
    }
}

fn hud_text(
    commands: &mut Commands,
    x: f32,
    y: &mut f32,
    text: &str,
    font_size: f32,
    color: Color,
) {
    spawn_text(commands, text, x, *y, font_size, 40.0, color);
    *y += font_size + 6.0;
}

fn draw_game_state_overlay(commands: &mut Commands, world: &mut SimWorld) {
    match *world.resource::<GameScreen>() {
        GameScreen::Playing => {}
        GameScreen::PauseMenu => draw_pause_menu(
            commands,
            world.resource::<PauseMenuState>(),
            world.resource::<PersistenceStatus>(),
        ),
        GameScreen::InventoryMenu => draw_inventory_menu(commands, world),
        GameScreen::OptionsMenu => draw_options_menu(commands),
    }
}

fn draw_pause_menu(commands: &mut Commands, menu: &PauseMenuState, status: &PersistenceStatus) {
    draw_modal_panel(commands, 560.0, 360.0);
    let panel_x = (initial_window_width() as f32 - 560.0) / 2.0;
    let mut y = (WINDOW_HEIGHT - 360.0) / 2.0 + 58.0;
    spawn_text(
        commands,
        "Paused",
        panel_x + 42.0,
        y,
        34.0,
        80.0,
        Color::WHITE,
    );
    y += 50.0;

    for entry in PauseMenuEntry::ALL {
        draw_menu_entry(
            commands,
            panel_x + 42.0,
            y,
            entry.label(),
            menu.selected() == entry,
        );
        y += 44.0;
    }

    if let Some(message) = &status.message {
        y += 14.0;
        spawn_text(
            commands,
            message,
            panel_x + 42.0,
            y,
            18.0,
            80.0,
            color_rgba(190, 204, 214, 255),
        );
    }
}

fn draw_inventory_menu(commands: &mut Commands, world: &mut SimWorld) {
    draw_modal_panel(commands, 460.0, 360.0);
    let panel_x = (initial_window_width() as f32 - 460.0) / 2.0;
    let mut y = (WINDOW_HEIGHT - 360.0) / 2.0 + 58.0;
    spawn_text(
        commands,
        "Inventory",
        panel_x + 42.0,
        y,
        34.0,
        80.0,
        Color::WHITE,
    );
    y += 50.0;

    let entries = InventoryEntry::all_from_world(world);
    if entries.is_empty() {
        spawn_text(
            commands,
            "No carried cargo.",
            panel_x + 42.0,
            y,
            22.0,
            80.0,
            color_rgba(210, 216, 222, 255),
        );
        return;
    }

    for entry in entries {
        draw_menu_entry(commands, panel_x + 42.0, y, &entry.label, entry.selected);
        y += 38.0;
    }
}

fn draw_options_menu(commands: &mut Commands) {
    draw_modal_panel(commands, 420.0, 250.0);
    let panel_x = (initial_window_width() as f32 - 420.0) / 2.0;
    let mut y = (WINDOW_HEIGHT - 250.0) / 2.0 + 58.0;
    spawn_text(
        commands,
        "Options",
        panel_x + 42.0,
        y,
        34.0,
        80.0,
        Color::WHITE,
    );
    y += 52.0;
    spawn_text(
        commands,
        "No options yet.",
        panel_x + 42.0,
        y,
        22.0,
        80.0,
        color_rgba(210, 216, 222, 255),
    );
}

fn draw_modal_panel(commands: &mut Commands, width: f32, height: f32) {
    spawn_rect(
        commands,
        0.0,
        0.0,
        initial_window_width() as f32,
        WINDOW_HEIGHT,
        70.0,
        color_rgba(0, 0, 0, 145),
    );
    let panel_x = (initial_window_width() as f32 - width) / 2.0;
    let panel_y = (WINDOW_HEIGHT - height) / 2.0;
    spawn_rect(
        commands,
        panel_x,
        panel_y,
        width,
        height,
        71.0,
        color_rgba(30, 35, 41, 245),
    );
    draw_rect_outline(
        commands,
        panel_x,
        panel_y,
        width,
        height,
        2.0,
        color_rgba(220, 226, 232, 180),
    );
}

fn draw_menu_entry(commands: &mut Commands, x: f32, y: f32, label: &str, selected: bool) {
    let color = if selected {
        color_rgba(252, 204, 84, 255)
    } else {
        color_rgba(220, 225, 230, 255)
    };
    let marker = if selected { ">" } else { " " };
    spawn_text(commands, marker, x, y, 26.0, 82.0, color);
    spawn_text(commands, label, x + 34.0, y, 26.0, 82.0, color);
}

fn draw_rect_outline(
    commands: &mut Commands,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    thickness: f32,
    color: Color,
) {
    spawn_rect(commands, x, y, width, thickness, 50.0, color);
    spawn_rect(
        commands,
        x,
        y + height - thickness,
        width,
        thickness,
        50.0,
        color,
    );
    spawn_rect(commands, x, y, thickness, height, 50.0, color);
    spawn_rect(
        commands,
        x + width - thickness,
        y,
        thickness,
        height,
        50.0,
        color,
    );
}

fn spawn_rect(
    commands: &mut Commands,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    z: f32,
    color: Color,
) {
    commands.spawn((
        RenderedFrameEntity,
        Sprite {
            color,
            custom_size: Some(Vec2::new(width, height)),
            ..Default::default()
        },
        Transform::from_translation(screen_point_to_world(x + width / 2.0, y + height / 2.0, z)),
    ));
}

fn spawn_text(
    commands: &mut Commands,
    text: &str,
    x: f32,
    y: f32,
    font_size: f32,
    z: f32,
    color: Color,
) {
    commands.spawn((
        RenderedFrameEntity,
        Text2d::new(text),
        TextFont {
            font_size: FontSize::Px(font_size),
            ..Default::default()
        },
        TextColor(color),
        Transform::from_translation(screen_point_to_world(x, y, z)),
    ));
}

fn screen_point_to_world(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(
        x - initial_window_width() as f32 / 2.0,
        WINDOW_HEIGHT / 2.0 - y,
        z,
    )
}
