use bevy::prelude::*;
use bevy_ecs::prelude::{ChildSpawner, With};

mod layout;
mod view_model;

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

pub(crate) fn sync_render(world: &mut World) {
    let rendered = {
        let mut query = world.query_filtered::<Entity, With<RenderedFrameEntity>>();
        query.iter(world).collect::<Vec<_>>()
    };
    for entity in rendered {
        world.entity_mut(entity).despawn();
    }

    update_camera(world);
    let camera = *world.resource::<Camera>();

    draw_viewport_background(world, camera);
    draw_map(world, camera);

    let loose_cargo = LooseCargoRender::all_from_world(world);
    let actor_cargo = ActorCargoRender::all_from_world(world);
    draw_loose_cargo(world, &loose_cargo, camera);
    draw_porters(world, camera, &actor_cargo);
    draw_player(world, camera, &actor_cargo);
    draw_viewport_frame(world, camera);
    draw_ui(world, camera);
}

fn update_camera(world: &mut World) {
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

fn draw_viewport_background(world: &mut World, camera: Camera) {
    spawn_rect(
        world,
        VIEWPORT_X - 4.0,
        VIEWPORT_Y - 4.0,
        viewport_width(camera) + 8.0,
        viewport_height(camera) + 8.0,
        0.0,
        color_rgba(8, 10, 12, 255),
    );
}

fn draw_viewport_frame(world: &mut World, camera: Camera) {
    draw_rect_outline(
        world,
        VIEWPORT_X - 1.0,
        VIEWPORT_Y - 1.0,
        viewport_width(camera) + 2.0,
        viewport_height(camera) + 2.0,
        2.0,
        color_rgba(215, 220, 226, 180),
    );
}

fn draw_map(world: &mut World, camera: Camera) {
    let visible_tiles = {
        let map = world.resource::<Map>();
        map.visible_tiles(camera.origin_coord(), camera.width, camera.height)
            .map(|coord| {
                let tile = map
                    .tile_at_coord(coord)
                    .expect("visible tile iteration is in bounds");
                (coord, tile)
            })
            .collect::<Vec<_>>()
    };

    for (coord, tile) in visible_tiles {
        let (px, py) = tile_to_screen(camera, coord);
        spawn_rect(
            world,
            px,
            py,
            TILE_SIZE,
            TILE_SIZE,
            1.0,
            terrain_color(tile.terrain, tile.elevation, tile.water_depth),
        );
        draw_rect_outline(
            world,
            px,
            py,
            TILE_SIZE,
            TILE_SIZE,
            1.0,
            color_rgba(0, 0, 0, 45),
        );

        spawn_text(
            world,
            &tile.elevation.to_string(),
            px + 1.5,
            py + 7.0,
            8.0,
            5.0,
            color_rgba(245, 245, 235, 175),
        );

        if tile.water_depth > 0 {
            spawn_text(
                world,
                &tile.water_depth.to_string(),
                px + 10.0,
                py + 14.0,
                8.0,
                5.0,
                color_rgba(210, 235, 255, 210),
            );
        } else if matches!(tile.terrain.glyph(), 'D' | '^') {
            spawn_text(
                world,
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

fn draw_loose_cargo(world: &mut World, cargo: &[LooseCargoRender], camera: Camera) {
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
        spawn_rect(world, cx - 5.0, cy - 5.0, 10.0, 10.0, 10.0, color);
        if let Some(outline) = outline {
            draw_rect_outline(world, cx - 6.5, cy - 6.5, 13.0, 13.0, 2.0, outline);
        }
        spawn_text(
            world,
            &format!("{:.0}", item.weight),
            cx - 4.0,
            cy + 4.0,
            10.0,
            12.0,
            Color::BLACK,
        );
        if let Some(label) = label {
            spawn_text(world, &label, px + 1.0, py - 1.0, 10.0, 12.0, Color::WHITE);
        }
    }
}

fn draw_porters(world: &mut World, camera: Camera, actor_cargo: &[ActorCargoRender]) {
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
        spawn_rect(world, px + 3.0, py + 3.0, 10.0, 10.0, 15.0, color);
        draw_rect_outline(world, px + 2.0, py + 2.0, 12.0, 12.0, 1.5, Color::WHITE);
        spawn_text(
            world,
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
        spawn_text(world, label, px + 10.0, py + 7.0, 10.0, 16.0, Color::WHITE);
        if let Some(cargo) = carried {
            draw_actor_cargo_badge(world, px, py, cargo);
        }
    }
}

fn draw_player(world: &mut World, camera: Camera, actor_cargo: &[ActorCargoRender]) {
    let player = {
        let mut query = world.query_filtered::<(
            bevy_ecs::prelude::Entity,
            &Position,
            &Stamina,
            &MovementState,
            &Momentum,
        ), bevy_ecs::prelude::With<Player>>();
        query
            .iter(world)
            .next()
            .map(|(entity, position, stamina, movement_state, _momentum)| {
                (entity, *position, *stamina, *movement_state)
            })
    };
    let Some((entity, position, stamina, movement_state)) = player else {
        return;
    };
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
        world,
        px + 2.0,
        py + 2.0,
        TILE_SIZE - 4.0,
        TILE_SIZE - 4.0,
        20.0,
        player_color,
    );
    spawn_text(
        world,
        "@",
        px + 3.0,
        py + 14.0,
        18.0,
        21.0,
        color_rgba(30, 35, 45, 255),
    );

    let stamina_ratio = stamina.current / stamina.max;
    spawn_rect(
        world,
        px,
        py - 4.0,
        TILE_SIZE,
        3.0,
        22.0,
        color_rgba(60, 20, 20, 255),
    );
    spawn_rect(
        world,
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
        draw_actor_cargo_badge(world, px, py, cargo);
    }
}

fn draw_actor_cargo_badge(world: &mut World, px: f32, py: f32, cargo: &ActorCargoRender) {
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
        world,
        px + 9.0,
        py,
        8.0,
        8.0,
        25.0,
        color_rgba(12, 14, 16, 235),
    );
    spawn_rect(world, px + 10.0, py + 1.0, 6.0, 6.0, 26.0, badge_color);
    spawn_text(world, &label, px + 10.0, py + 7.0, 8.0, 27.0, Color::BLACK);
}

fn draw_ui(world: &mut World, camera: Camera) {
    let screen = *world.resource::<GameScreen>();
    let pause_menu = *world.resource::<PauseMenuState>();
    let persistence_status = world.resource::<PersistenceStatus>().clone();
    let hud = PlayerHudSnapshot::from_world(world, camera);
    let porter_rows = PorterDebugRow::all_from_world(world);
    let inventory_entries = if screen == GameScreen::InventoryMenu {
        InventoryEntry::all_from_world(world)
    } else {
        Vec::new()
    };

    world
        .spawn((
            RenderedFrameEntity,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                ..Default::default()
            },
            GlobalZIndex(10),
        ))
        .with_children(|parent| {
            draw_hud(parent, hud, &porter_rows, camera);
            match screen {
                GameScreen::Playing => {}
                GameScreen::PauseMenu => {
                    draw_pause_menu(parent, pause_menu, &persistence_status);
                }
                GameScreen::InventoryMenu => {
                    draw_inventory_menu(parent, &inventory_entries);
                }
                GameScreen::OptionsMenu => {
                    draw_options_menu(parent);
                }
            }
        });
}

fn draw_hud(
    parent: &mut ChildSpawner,
    hud: Option<PlayerHudSnapshot>,
    porter_rows: &[PorterDebugRow],
    camera: Camera,
) {
    let Some(hud) = hud else {
        return;
    };
    let panel_x = ui_x(camera);

    parent
        .spawn((
            RenderedFrameEntity,
            Node {
                position_type: PositionType::Absolute,
                left: px(panel_x - 12.0),
                top: px(0),
                width: px(UI_PANEL_WIDTH + 24.0),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexStart,
                padding: UiRect::new(px(12), px(12), px(26), px(20)),
                row_gap: px(4),
                ..Default::default()
            },
            BackgroundColor(color_rgba(24, 27, 31, 245)),
        ))
        .with_children(|panel| {
            spawn_ui_text(panel, "Cargo Stranding Again", 26.0, Color::WHITE);
            spawn_ui_text(
                panel,
                "Bevy app + Bevy ECS simulation",
                18.0,
                Color::srgb(0.5, 0.5, 0.5),
            );
            spawn_ui_spacer(panel, 10.0);
            spawn_ui_text(panel, format!("Turn: {}", hud.turn), 18.0, Color::WHITE);
            spawn_ui_text(
                panel,
                format!("Energy time: {}", hud.timeline),
                18.0,
                Color::WHITE,
            );
            spawn_ui_text(
                panel,
                format!(
                    "Camera: {},{} | {}x{}",
                    hud.camera.x, hud.camera.y, hud.camera.width, hud.camera.height
                ),
                18.0,
                Color::WHITE,
            );
            spawn_ui_text(
                panel,
                format!("Player: {}, {}", hud.position.x, hud.position.y),
                18.0,
                Color::WHITE,
            );
            spawn_ui_text(
                panel,
                format!(
                    "Elevation: {} | water depth: {}",
                    hud.elevation, hud.water_depth
                ),
                18.0,
                Color::WHITE,
            );
            spawn_ui_text(
                panel,
                format!("Stamina: {:.1}/{:.1}", hud.stamina_current, hud.stamina_max),
                18.0,
                Color::WHITE,
            );
            spawn_ui_text(
                panel,
                format!("Movement: {}", hud.movement_mode.label()),
                18.0,
                Color::WHITE,
            );
            spawn_ui_text(
                panel,
                format!(
                    "Momentum: {:.1} {}",
                    hud.momentum_amount,
                    hud.momentum_direction
                        .map(|direction| direction.label())
                        .unwrap_or("none")
                ),
                18.0,
                Color::WHITE,
            );
            spawn_ui_text(
                panel,
                format!("Ready: {}", hud.ready_label),
                18.0,
                Color::WHITE,
            );
            spawn_ui_text(
                panel,
                format!("Cargo: {:.1}/{:.1}", hud.cargo_current, hud.cargo_max),
                18.0,
                Color::WHITE,
            );
            spawn_ui_text(
                panel,
                format!("Delivered parcels: {}", hud.delivered_parcels),
                18.0,
                Color::WHITE,
            );
            spawn_ui_spacer(panel, 14.0);

            spawn_ui_text(panel, "Porters", 22.0, Color::WHITE);
            for row in porter_rows {
                spawn_ui_text(
                    panel,
                    format!(
                        "P{}: {},{} | {} | load {:.0} | {}",
                        row.id,
                        row.position.x,
                        row.position.y,
                        row.phase_label,
                        row.load,
                        row.ready_label
                    ),
                    18.0,
                    Color::WHITE,
                );
            }
            spawn_ui_spacer(panel, 14.0);

            spawn_ui_text(panel, "Controls", 22.0, Color::WHITE);
            for line in CONTROL_HINTS {
                spawn_ui_text(panel, line, 18.0, Color::WHITE);
            }
            spawn_ui_spacer(panel, 14.0);

            spawn_ui_text(panel, "Legend", 22.0, Color::WHITE);
            for line in LEGEND_LINES {
                spawn_ui_text(panel, line, 18.0, Color::WHITE);
            }
        });
}

fn draw_pause_menu(parent: &mut ChildSpawner, menu: PauseMenuState, status: &PersistenceStatus) {
    spawn_modal(parent, 560.0, 360.0, |panel| {
        spawn_ui_text(panel, "Paused", 34.0, Color::WHITE);
        spawn_ui_spacer(panel, 18.0);
        for entry in PauseMenuEntry::ALL {
            spawn_menu_entry(panel, entry.label(), menu.selected() == entry);
        }
        if let Some(message) = &status.message {
            spawn_ui_spacer(panel, 10.0);
            spawn_ui_text(panel, message, 18.0, color_rgba(190, 204, 214, 255));
        }
    });
}

fn draw_inventory_menu(parent: &mut ChildSpawner, entries: &[InventoryEntry]) {
    spawn_modal(parent, 460.0, 360.0, |panel| {
        spawn_ui_text(panel, "Inventory", 34.0, Color::WHITE);
        spawn_ui_spacer(panel, 18.0);

        if entries.is_empty() {
            spawn_ui_text(
                panel,
                "No carried cargo.",
                22.0,
                color_rgba(210, 216, 222, 255),
            );
            return;
        }

        for entry in entries {
            spawn_menu_entry(panel, &entry.label, entry.selected);
        }
    });
}

fn draw_options_menu(parent: &mut ChildSpawner) {
    spawn_modal(parent, 420.0, 250.0, |panel| {
        spawn_ui_text(panel, "Options", 34.0, Color::WHITE);
        spawn_ui_spacer(panel, 20.0);
        spawn_ui_text(
            panel,
            "No options yet.",
            22.0,
            color_rgba(210, 216, 222, 255),
        );
    });
}

fn spawn_modal(
    parent: &mut ChildSpawner,
    width: f32,
    height: f32,
    children: impl FnOnce(&mut ChildSpawner),
) {
    parent
        .spawn((
            RenderedFrameEntity,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..Default::default()
            },
            BackgroundColor(color_rgba(0, 0, 0, 145)),
            GlobalZIndex(100),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    RenderedFrameEntity,
                    Node {
                        width: px(width),
                        height: px(height),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::FlexStart,
                        padding: UiRect::all(px(42)),
                        border: UiRect::all(px(2)),
                        row_gap: px(6),
                        ..Default::default()
                    },
                    BackgroundColor(color_rgba(30, 35, 41, 245)),
                    BorderColor::all(color_rgba(220, 226, 232, 180)),
                ))
                .with_children(children);
        });
}

fn spawn_menu_entry(parent: &mut ChildSpawner, label: &str, selected: bool) {
    let color = if selected {
        color_rgba(252, 204, 84, 255)
    } else {
        color_rgba(220, 225, 230, 255)
    };
    let marker = if selected { ">" } else { " " };
    parent
        .spawn((
            RenderedFrameEntity,
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(18),
                min_height: px(34),
                align_items: AlignItems::Center,
                ..Default::default()
            },
        ))
        .with_children(|row| {
            spawn_ui_text(row, marker, 26.0, color);
            spawn_ui_text(row, label, 26.0, color);
        });
}

fn spawn_ui_text(parent: &mut ChildSpawner, text: impl Into<String>, font_size: f32, color: Color) {
    parent.spawn((
        RenderedFrameEntity,
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(font_size),
            ..Default::default()
        },
        TextColor(color),
        Node {
            min_height: px(font_size + 4.0),
            ..Default::default()
        },
    ));
}

fn spawn_ui_spacer(parent: &mut ChildSpawner, height: f32) {
    parent.spawn((
        RenderedFrameEntity,
        Node {
            height: px(height),
            ..Default::default()
        },
    ));
}

fn draw_rect_outline(
    world: &mut World,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    thickness: f32,
    color: Color,
) {
    spawn_rect(world, x, y, width, thickness, 50.0, color);
    spawn_rect(
        world,
        x,
        y + height - thickness,
        width,
        thickness,
        50.0,
        color,
    );
    spawn_rect(world, x, y, thickness, height, 50.0, color);
    spawn_rect(
        world,
        x + width - thickness,
        y,
        thickness,
        height,
        50.0,
        color,
    );
}

fn spawn_rect(world: &mut World, x: f32, y: f32, width: f32, height: f32, z: f32, color: Color) {
    world.spawn((
        RenderedFrameEntity,
        Sprite {
            color,
            custom_size: Some(Vec2::new(width, height)),
            ..Default::default()
        },
        Transform::from_translation(screen_point_to_world(x + width / 2.0, y + height / 2.0, z)),
    ));
}

fn spawn_text(world: &mut World, text: &str, x: f32, y: f32, font_size: f32, z: f32, color: Color) {
    world.spawn((
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_sync_spawns_world_and_ui_entities() {
        let mut app = App::new();
        crate::world_setup::init_world(app.world_mut());
        app.add_systems(Update, sync_render);

        app.update();

        let mut rendered = app
            .world_mut()
            .query_filtered::<Entity, With<RenderedFrameEntity>>();
        let rendered_count = rendered.iter(app.world()).count();
        let mut sprites = app.world_mut().query::<&Sprite>();
        let sprite_count = sprites.iter(app.world()).count();
        let mut nodes = app.world_mut().query::<&Node>();
        let node_count = nodes.iter(app.world()).count();
        let mut texts = app.world_mut().query::<&Text>();
        let text_count = texts.iter(app.world()).count();

        assert!(rendered_count > 0);
        assert!(sprite_count > 0, "world scene should spawn sprite entities");
        assert!(node_count > 0, "HUD should spawn Bevy UI nodes");
        assert!(text_count > 0, "HUD should spawn Bevy UI text");
    }
}
