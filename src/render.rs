use std::collections::HashSet;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_ecs::prelude::{ChildSpawner, With};

mod layout;
mod view_model;

use crate::components::*;
use crate::map::{Map, Terrain, TileCoord};
use crate::render::layout::{
    tile_to_screen, viewport_height, viewport_width, VIEWPORT_X, VIEWPORT_Y,
};
use crate::render::view_model::{
    ActorCargoRender, CargoHolderKind, InventoryEntry, LooseCargoRender, LooseCargoState,
    PlayerHudSnapshot, PorterDebugRow,
};
use crate::resources::{
    Camera, GameScreen, PauseMenuEntry, PauseMenuState, PersistenceStatus, DEFAULT_CAMERA_TILE_SPAN,
};

pub const TILE_SIZE: f32 = 16.0;
const INITIAL_WINDOW_WIDTH: f32 = 1280.0;
const INITIAL_WINDOW_HEIGHT: f32 = 720.0;
const CONTROL_HINTS: [&str; 9] = [
    "WASD / Arrows: move one tile",
    "Space / .: wait and recover stamina",
    "E: pick up loose cargo here",
    "I / Tab: open inventory",
    "Shift: cycle walk / sprint / steady",
    "Esc: pause / resume",
    "?: show this panel",
    "Energy advances only on valid action",
    "Water blocks movement",
];
const LEGEND_LINES: [&str; 5] = [
    "@ player",
    "Teal/Gold squares: porters",
    "Orange boxes: available cargo",
    "Yellow outlined boxes: reserved cargo",
    "Small badges: carried cargo",
];

#[derive(Component)]
struct ViewportBackground;

#[derive(Component, Clone, Copy)]
enum ViewportFrameSegment {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Component, Clone, Copy)]
struct MapTileSprite {
    offset_x: i32,
    offset_y: i32,
}

#[derive(Component, Clone, Copy)]
struct MapTileElevationText {
    offset_x: i32,
    offset_y: i32,
}

#[derive(Component, Clone, Copy)]
struct MapTileAuxText {
    offset_x: i32,
    offset_y: i32,
}

#[derive(Component, Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum SceneRenderKey {
    LooseCargoBox(Entity),
    LooseCargoOutline(Entity),
    LooseCargoWeight(Entity),
    LooseCargoLabel(Entity),
    PorterBody(Entity),
    PorterOutline(Entity),
    PorterId(Entity),
    PorterPhase(Entity),
    PlayerBody,
    PlayerGlyph,
    PlayerStaminaBackground,
    PlayerStaminaFill,
    CargoBadgeBackground(Entity),
    CargoBadgeFill(Entity),
    CargoBadgeText(Entity),
}

#[derive(Component)]
struct SceneRenderPart {
    key: SceneRenderKey,
}

#[derive(Component)]
pub(crate) struct HudRoot;

#[derive(Component, Clone, Copy)]
enum HudTextField {
    Turn,
    Timeline,
    Camera,
    Player,
    Terrain,
    Stamina,
    Movement,
    Momentum,
    Ready,
    Cargo,
    Delivered,
    Porters,
}

#[derive(Component)]
struct HudText {
    field: HudTextField,
}

#[derive(Component)]
struct MenuRoot;

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuUiFingerprint {
    screen: GameScreen,
    pause_selected: Option<PauseMenuEntry>,
    persistence_status: Option<String>,
    inventory_entries: Vec<(String, bool)>,
}

pub fn initial_window_width() -> u32 {
    INITIAL_WINDOW_WIDTH as u32
}

pub fn initial_window_height() -> u32 {
    INITIAL_WINDOW_HEIGHT as u32
}

pub fn color_rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::srgba_u8(r, g, b, a)
}

pub(crate) fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

pub(crate) fn spawn_scene_view(world: &mut World) {
    spawn_viewport_chrome(world);
    spawn_map_tile_pool(world);
}

pub(crate) fn sync_rendered_scene(world: &mut World) {
    update_camera(world);
    let camera = *world.resource::<Camera>();

    sync_viewport_chrome(world, camera);
    sync_map_tiles(world, camera);

    let loose_cargo = LooseCargoRender::all_from_world(world);
    let actor_cargo = ActorCargoRender::all_from_world(world);
    let mut live_parts = HashSet::new();
    sync_loose_cargo(world, &loose_cargo, camera, &mut live_parts);
    sync_porters(world, camera, &actor_cargo, &mut live_parts);
    sync_player(world, camera, &actor_cargo, &mut live_parts);
    despawn_stale_scene_parts(world, &live_parts);
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
    let (width, height) = camera_tile_dimensions(world);
    let mut camera = world.resource_mut::<Camera>();
    camera.resize_tiles(width, height);
    camera.center_on(player_position, bounds);
}

fn camera_tile_dimensions(world: &mut World) -> (i32, i32) {
    let dimensions = window_dimensions(world);
    (
        (dimensions.x / TILE_SIZE).ceil().max(1.0) as i32,
        (dimensions.y / TILE_SIZE).ceil().max(1.0) as i32,
    )
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

fn spawn_viewport_chrome(world: &mut World) {
    world.spawn((
        ViewportBackground,
        Sprite::default(),
        Transform::default(),
        Visibility::default(),
    ));
    for segment in [
        ViewportFrameSegment::Top,
        ViewportFrameSegment::Bottom,
        ViewportFrameSegment::Left,
        ViewportFrameSegment::Right,
    ] {
        world.spawn((
            segment,
            Sprite::default(),
            Transform::default(),
            Visibility::default(),
        ));
    }
}

fn sync_viewport_chrome(world: &mut World, camera: Camera) {
    let background = {
        let mut query = world.query_filtered::<Entity, With<ViewportBackground>>();
        query.iter(world).next()
    };
    if let Some(entity) = background {
        set_sprite(
            world,
            entity,
            SpriteRender {
                x: VIEWPORT_X - 4.0,
                y: VIEWPORT_Y - 4.0,
                width: viewport_width(camera) + 8.0,
                height: viewport_height(camera) + 8.0,
                z: 0.0,
                color: color_rgba(8, 10, 12, 255),
                visible: true,
            },
        );
    }

    let frame_segments = {
        let mut query = world.query::<(Entity, &ViewportFrameSegment)>();
        query
            .iter(world)
            .map(|(entity, segment)| (entity, *segment))
            .collect::<Vec<_>>()
    };

    for (entity, segment) in frame_segments {
        set_sprite(world, entity, viewport_frame_render(segment, camera));
    }
}

fn viewport_frame_render(segment: ViewportFrameSegment, camera: Camera) -> SpriteRender {
    let x = VIEWPORT_X;
    let y = VIEWPORT_Y;
    let width = viewport_width(camera);
    let height = viewport_height(camera);
    let thickness = 2.0;
    let (x, y, width, height) = match segment {
        ViewportFrameSegment::Top => (x, y, width, thickness),
        ViewportFrameSegment::Bottom => (x, y + height - thickness, width, thickness),
        ViewportFrameSegment::Left => (x, y, thickness, height),
        ViewportFrameSegment::Right => (x + width - thickness, y, thickness, height),
    };

    SpriteRender {
        x,
        y,
        width,
        height,
        z: 50.0,
        color: color_rgba(215, 220, 226, 180),
        visible: true,
    }
}

fn spawn_map_tile_pool(world: &mut World) {
    for offset_y in 0..DEFAULT_CAMERA_TILE_SPAN {
        for offset_x in 0..DEFAULT_CAMERA_TILE_SPAN {
            spawn_map_tile_parts(world, offset_x, offset_y);
        }
    }
}

fn ensure_map_tile_pool(world: &mut World, camera: Camera) {
    let existing = {
        let mut query = world.query::<&MapTileSprite>();
        query
            .iter(world)
            .map(|tile| (tile.offset_x, tile.offset_y))
            .collect::<HashSet<_>>()
    };

    for offset_y in 0..camera.height {
        for offset_x in 0..camera.width {
            if !existing.contains(&(offset_x, offset_y)) {
                spawn_map_tile_parts(world, offset_x, offset_y);
            }
        }
    }
}

fn spawn_map_tile_parts(world: &mut World, offset_x: i32, offset_y: i32) {
    world.spawn((
        MapTileSprite { offset_x, offset_y },
        Sprite::default(),
        Transform::default(),
        Visibility::default(),
    ));
    world.spawn((
        MapTileElevationText { offset_x, offset_y },
        Text2d::new(""),
        TextFont {
            font_size: FontSize::Px(8.0),
            ..Default::default()
        },
        TextColor(color_rgba(245, 245, 235, 175)),
        Transform::default(),
        Visibility::default(),
    ));
    world.spawn((
        MapTileAuxText { offset_x, offset_y },
        Text2d::new(""),
        TextFont {
            font_size: FontSize::Px(8.0),
            ..Default::default()
        },
        TextColor(color_rgba(210, 235, 255, 210)),
        Transform::default(),
        Visibility::default(),
    ));
}

fn coord_for_camera_offset(camera: Camera, offset_x: i32, offset_y: i32) -> Option<TileCoord> {
    (offset_x < camera.width && offset_y < camera.height)
        .then(|| TileCoord::new(camera.x + offset_x, camera.y + offset_y))
}

fn sync_map_tiles(world: &mut World, camera: Camera) {
    ensure_map_tile_pool(world, camera);

    let sprites = {
        let mut query = world.query::<(Entity, &MapTileSprite)>();
        query
            .iter(world)
            .map(|(entity, tile)| (entity, tile.offset_x, tile.offset_y))
            .collect::<Vec<_>>()
    };
    let elevation_text = {
        let mut query = world.query::<(Entity, &MapTileElevationText)>();
        query
            .iter(world)
            .map(|(entity, tile)| (entity, tile.offset_x, tile.offset_y))
            .collect::<Vec<_>>()
    };
    let aux_text = {
        let mut query = world.query::<(Entity, &MapTileAuxText)>();
        query
            .iter(world)
            .map(|(entity, tile)| (entity, tile.offset_x, tile.offset_y))
            .collect::<Vec<_>>()
    };

    for (entity, offset_x, offset_y) in sprites {
        let Some(coord) = coord_for_camera_offset(camera, offset_x, offset_y) else {
            set_visibility(world, entity, false);
            continue;
        };
        let tile = {
            let map = world.resource::<Map>();
            map.tile_at_coord(coord)
        };
        let Some(tile) = tile else {
            set_visibility(world, entity, false);
            continue;
        };
        let (px, py) = tile_to_screen(camera, coord);
        set_sprite(
            world,
            entity,
            SpriteRender {
                x: px,
                y: py,
                width: TILE_SIZE,
                height: TILE_SIZE,
                z: 1.0,
                color: terrain_color(tile.terrain, tile.elevation, tile.water_depth),
                visible: true,
            },
        );
    }

    for (entity, offset_x, offset_y) in elevation_text {
        let Some(coord) = coord_for_camera_offset(camera, offset_x, offset_y) else {
            set_visibility(world, entity, false);
            continue;
        };
        let tile = {
            let map = world.resource::<Map>();
            map.tile_at_coord(coord)
        };
        let Some(tile) = tile else {
            set_visibility(world, entity, false);
            continue;
        };
        let (px, py) = tile_to_screen(camera, coord);
        set_text(
            world,
            entity,
            TextRender {
                text: tile.elevation.to_string(),
                x: px + 1.5,
                y: py + 7.0,
                z: 5.0,
                color: color_rgba(245, 245, 235, 175),
                font_size: 8.0,
                visible: true,
            },
        );
    }

    for (entity, offset_x, offset_y) in aux_text {
        let Some(coord) = coord_for_camera_offset(camera, offset_x, offset_y) else {
            set_visibility(world, entity, false);
            continue;
        };
        let tile = {
            let map = world.resource::<Map>();
            map.tile_at_coord(coord)
        };
        let Some(tile) = tile else {
            set_visibility(world, entity, false);
            continue;
        };
        let (text, x, y, font_size, color) = if tile.water_depth > 0 {
            (
                tile.water_depth.to_string(),
                10.0,
                14.0,
                8.0,
                color_rgba(210, 235, 255, 210),
            )
        } else if matches!(tile.terrain.glyph(), 'D' | '^') {
            (
                tile.terrain.glyph().to_string(),
                4.0,
                12.5,
                14.0,
                color_rgba(18, 18, 18, 200),
            )
        } else {
            set_visibility(world, entity, false);
            continue;
        };
        let (px, py) = tile_to_screen(camera, coord);
        set_text(
            world,
            entity,
            TextRender {
                text,
                x: px + x,
                y: py + y,
                z: 5.0,
                color,
                font_size,
                visible: true,
            },
        );
    }
}

fn sync_loose_cargo(
    world: &mut World,
    cargo: &[LooseCargoRender],
    camera: Camera,
    live_parts: &mut HashSet<SceneRenderKey>,
) {
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
        set_keyed_sprite(
            world,
            SceneRenderKey::LooseCargoBox(item.entity),
            live_parts,
            SpriteRender {
                x: cx - 5.0,
                y: cy - 5.0,
                width: 10.0,
                height: 10.0,
                z: 10.0,
                color,
                visible: true,
            },
        );
        if let Some(outline) = outline {
            set_keyed_sprite(
                world,
                SceneRenderKey::LooseCargoOutline(item.entity),
                live_parts,
                SpriteRender {
                    x: cx - 6.5,
                    y: cy - 6.5,
                    width: 13.0,
                    height: 13.0,
                    z: 11.0,
                    color: outline,
                    visible: true,
                },
            );
        }
        set_keyed_text(
            world,
            SceneRenderKey::LooseCargoWeight(item.entity),
            live_parts,
            TextRender {
                text: format!("{:.0}", item.weight),
                x: cx - 4.0,
                y: cy + 4.0,
                z: 12.0,
                color: Color::BLACK,
                font_size: 10.0,
                visible: true,
            },
        );
        if let Some(label) = label {
            set_keyed_text(
                world,
                SceneRenderKey::LooseCargoLabel(item.entity),
                live_parts,
                TextRender {
                    text: label,
                    x: px + 1.0,
                    y: py - 1.0,
                    z: 12.0,
                    color: Color::WHITE,
                    font_size: 10.0,
                    visible: true,
                },
            );
        }
    }
}

fn sync_porters(
    world: &mut World,
    camera: Camera,
    actor_cargo: &[ActorCargoRender],
    live_parts: &mut HashSet<SceneRenderKey>,
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
        set_keyed_sprite(
            world,
            SceneRenderKey::PorterBody(entity),
            live_parts,
            SpriteRender {
                x: px + 3.0,
                y: py + 3.0,
                width: 10.0,
                height: 10.0,
                z: 15.0,
                color,
                visible: true,
            },
        );
        set_keyed_sprite(
            world,
            SceneRenderKey::PorterOutline(entity),
            live_parts,
            SpriteRender {
                x: px + 2.0,
                y: py + 2.0,
                width: 12.0,
                height: 12.0,
                z: 16.0,
                color: Color::WHITE,
                visible: true,
            },
        );
        set_keyed_text(
            world,
            SceneRenderKey::PorterId(entity),
            live_parts,
            TextRender {
                text: format!("P{porter_id}"),
                x: px + 1.0,
                y: py + 13.0,
                z: 16.0,
                color: Color::BLACK,
                font_size: 12.0,
                visible: true,
            },
        );

        let label = match phase {
            JobPhase::FindParcel => "?",
            JobPhase::GoToParcel => "P",
            JobPhase::GoToDepot => "D",
            JobPhase::Done => "!",
        };
        set_keyed_text(
            world,
            SceneRenderKey::PorterPhase(entity),
            live_parts,
            TextRender {
                text: label.to_owned(),
                x: px + 10.0,
                y: py + 7.0,
                z: 16.0,
                color: Color::WHITE,
                font_size: 10.0,
                visible: true,
            },
        );
        if let Some(cargo) = carried {
            sync_actor_cargo_badge(world, px, py, cargo, live_parts);
        }
    }
}

fn sync_player(
    world: &mut World,
    camera: Camera,
    actor_cargo: &[ActorCargoRender],
    live_parts: &mut HashSet<SceneRenderKey>,
) {
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
    set_keyed_sprite(
        world,
        SceneRenderKey::PlayerBody,
        live_parts,
        SpriteRender {
            x: px + 2.0,
            y: py + 2.0,
            width: TILE_SIZE - 4.0,
            height: TILE_SIZE - 4.0,
            z: 20.0,
            color: player_color,
            visible: true,
        },
    );
    set_keyed_text(
        world,
        SceneRenderKey::PlayerGlyph,
        live_parts,
        TextRender {
            text: "@".to_owned(),
            x: px + 3.0,
            y: py + 14.0,
            z: 21.0,
            color: color_rgba(30, 35, 45, 255),
            font_size: 18.0,
            visible: true,
        },
    );

    let stamina_ratio = stamina.current / stamina.max;
    set_keyed_sprite(
        world,
        SceneRenderKey::PlayerStaminaBackground,
        live_parts,
        SpriteRender {
            x: px,
            y: py - 4.0,
            width: TILE_SIZE,
            height: 3.0,
            z: 22.0,
            color: color_rgba(60, 20, 20, 255),
            visible: true,
        },
    );
    set_keyed_sprite(
        world,
        SceneRenderKey::PlayerStaminaFill,
        live_parts,
        SpriteRender {
            x: px,
            y: py - 4.0,
            width: TILE_SIZE * stamina_ratio,
            height: 3.0,
            z: 23.0,
            color: color_rgba(90, 220, 130, 255),
            visible: true,
        },
    );

    if let Some(cargo) = actor_cargo
        .iter()
        .find(|cargo| cargo.holder == entity)
        .filter(|cargo| matches!(cargo.holder_kind, CargoHolderKind::Player))
    {
        sync_actor_cargo_badge(world, px, py, cargo, live_parts);
    }
}

fn sync_actor_cargo_badge(
    world: &mut World,
    px: f32,
    py: f32,
    cargo: &ActorCargoRender,
    live_parts: &mut HashSet<SceneRenderKey>,
) {
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
    set_keyed_sprite(
        world,
        SceneRenderKey::CargoBadgeBackground(cargo.holder),
        live_parts,
        SpriteRender {
            x: px + 9.0,
            y: py,
            width: 8.0,
            height: 8.0,
            z: 25.0,
            color: color_rgba(12, 14, 16, 235),
            visible: true,
        },
    );
    set_keyed_sprite(
        world,
        SceneRenderKey::CargoBadgeFill(cargo.holder),
        live_parts,
        SpriteRender {
            x: px + 10.0,
            y: py + 1.0,
            width: 6.0,
            height: 6.0,
            z: 26.0,
            color: badge_color,
            visible: true,
        },
    );
    set_keyed_text(
        world,
        SceneRenderKey::CargoBadgeText(cargo.holder),
        live_parts,
        TextRender {
            text: label,
            x: px + 10.0,
            y: py + 7.0,
            z: 27.0,
            color: Color::BLACK,
            font_size: 8.0,
            visible: true,
        },
    );
}

pub(crate) fn update_hud(world: &mut World) {
    let camera = *world.resource::<Camera>();
    let Some(hud) = PlayerHudSnapshot::from_world(world, camera) else {
        return;
    };
    let porter_rows = PorterDebugRow::all_from_world(world);
    let text_entities = {
        let mut query = world.query::<(Entity, &HudText)>();
        query
            .iter(world)
            .map(|(entity, text)| (entity, text.field))
            .collect::<Vec<_>>()
    };

    for (entity, field) in text_entities {
        if let Some(mut text) = world.get_mut::<Text>(entity) {
            text.0 = hud_text(field, &hud, &porter_rows);
        }
    }
}

pub(crate) fn sync_menu_ui(world: &mut World) {
    let fingerprint = menu_fingerprint(world);
    if world.resource::<MenuUiFingerprint>() == &fingerprint {
        return;
    }

    let menu_roots = {
        let mut query = world.query_filtered::<Entity, With<MenuRoot>>();
        query.iter(world).collect::<Vec<_>>()
    };
    for entity in menu_roots {
        world.entity_mut(entity).despawn();
    }

    match fingerprint.screen {
        GameScreen::Playing => {}
        GameScreen::HelpModal => spawn_help_modal_root(world),
        GameScreen::PauseMenu => spawn_pause_menu_root(
            world,
            fingerprint.pause_selected.unwrap_or(PauseMenuEntry::Resume),
            fingerprint.persistence_status.as_deref(),
        ),
        GameScreen::InventoryMenu => {
            spawn_inventory_menu_root(world, &fingerprint.inventory_entries);
        }
        GameScreen::OptionsMenu => spawn_options_menu_root(world),
    }

    *world.resource_mut::<MenuUiFingerprint>() = fingerprint;
}

fn spawn_hud_text(parent: &mut ChildSpawner, field: HudTextField) {
    parent.spawn((
        HudText { field },
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(18.0),
            ..Default::default()
        },
        TextColor(Color::WHITE),
        Node {
            min_height: px(22.0),
            ..Default::default()
        },
    ));
}

fn hud_text(
    field: HudTextField,
    hud: &PlayerHudSnapshot,
    porter_rows: &[PorterDebugRow],
) -> String {
    match field {
        HudTextField::Turn => format!("Turn: {}", hud.turn),
        HudTextField::Timeline => format!("Energy time: {}", hud.timeline),
        HudTextField::Camera => format!(
            "Camera: {},{} | {}x{}",
            hud.camera.x, hud.camera.y, hud.camera.width, hud.camera.height
        ),
        HudTextField::Player => format!("Player: {}, {}", hud.position.x, hud.position.y),
        HudTextField::Terrain => format!(
            "Elevation: {} | water depth: {}",
            hud.elevation, hud.water_depth
        ),
        HudTextField::Stamina => {
            format!("Stamina: {:.1}/{:.1}", hud.stamina_current, hud.stamina_max)
        }
        HudTextField::Movement => format!("Movement: {}", hud.movement_mode.label()),
        HudTextField::Momentum => format!(
            "Momentum: {:.1} {}",
            hud.momentum_amount,
            hud.momentum_direction
                .map(|direction| direction.label())
                .unwrap_or("none")
        ),
        HudTextField::Ready => format!("Ready: {}", hud.ready_label),
        HudTextField::Cargo => format!("Cargo: {:.1}/{:.1}", hud.cargo_current, hud.cargo_max),
        HudTextField::Delivered => format!("Delivered parcels: {}", hud.delivered_parcels),
        HudTextField::Porters => porter_rows
            .iter()
            .map(|row| {
                format!(
                    "P{}: {},{} | {} | load {:.0} | {}",
                    row.id,
                    row.position.x,
                    row.position.y,
                    row.phase_label,
                    row.load,
                    row.ready_label
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn menu_fingerprint(world: &mut World) -> MenuUiFingerprint {
    let screen = *world.resource::<GameScreen>();
    let pause_selected =
        (screen == GameScreen::PauseMenu).then(|| world.resource::<PauseMenuState>().selected());
    let persistence_status = world.resource::<PersistenceStatus>().message.clone();
    let inventory_entries = if screen == GameScreen::InventoryMenu {
        InventoryEntry::all_from_world(world)
            .into_iter()
            .map(|entry| (entry.label, entry.selected))
            .collect()
    } else {
        Vec::new()
    };

    MenuUiFingerprint {
        screen,
        pause_selected,
        persistence_status,
        inventory_entries,
    }
}

fn spawn_help_modal_root(world: &mut World) {
    world
        .spawn(menu_root_bundle())
        .with_children(draw_help_modal);
}

fn spawn_pause_menu_root(world: &mut World, selected: PauseMenuEntry, status: Option<&str>) {
    world
        .spawn(menu_root_bundle())
        .with_children(|parent| draw_pause_menu(parent, selected, status));
}

fn spawn_inventory_menu_root(world: &mut World, entries: &[(String, bool)]) {
    world
        .spawn(menu_root_bundle())
        .with_children(|parent| draw_inventory_menu(parent, entries));
}

fn spawn_options_menu_root(world: &mut World) {
    world
        .spawn(menu_root_bundle())
        .with_children(draw_options_menu);
}

fn menu_root_bundle() -> impl Bundle {
    (
        MenuRoot,
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            width: percent(100),
            height: percent(100),
            ..Default::default()
        },
        GlobalZIndex(100),
    )
}

fn draw_help_modal(parent: &mut ChildSpawner) {
    spawn_modal(parent, 900.0, 560.0, |panel| {
        panel
            .spawn((
                HudRoot,
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::FlexStart,
                    row_gap: px(8),
                    ..Default::default()
                },
            ))
            .with_children(|panel| {
                spawn_ui_text(panel, "Cargo Stranding Again", 30.0, Color::WHITE);
                spawn_ui_text(
                    panel,
                    "Bevy app + Bevy ECS simulation",
                    16.0,
                    Color::srgb(0.5, 0.5, 0.5),
                );
                panel
                    .spawn((Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::FlexStart,
                        column_gap: px(42),
                        ..Default::default()
                    },))
                    .with_children(|columns| {
                        spawn_help_status_column(columns);
                        spawn_help_reference_column(columns);
                    });
            });
    });
}

fn spawn_help_status_column(parent: &mut ChildSpawner) {
    parent
        .spawn((Node {
            width: px(430),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexStart,
            row_gap: px(3),
            ..Default::default()
        },))
        .with_children(|panel| {
            spawn_hud_text(panel, HudTextField::Turn);
            spawn_hud_text(panel, HudTextField::Timeline);
            spawn_hud_text(panel, HudTextField::Camera);
            spawn_hud_text(panel, HudTextField::Player);
            spawn_hud_text(panel, HudTextField::Terrain);
            spawn_hud_text(panel, HudTextField::Stamina);
            spawn_hud_text(panel, HudTextField::Movement);
            spawn_hud_text(panel, HudTextField::Momentum);
            spawn_hud_text(panel, HudTextField::Ready);
            spawn_hud_text(panel, HudTextField::Cargo);
            spawn_hud_text(panel, HudTextField::Delivered);
            spawn_ui_spacer(panel, 8.0);
            spawn_ui_text(panel, "Porters", 20.0, Color::WHITE);
            spawn_hud_text(panel, HudTextField::Porters);
        });
}

fn spawn_help_reference_column(parent: &mut ChildSpawner) {
    parent
        .spawn((Node {
            width: px(330),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexStart,
            row_gap: px(3),
            ..Default::default()
        },))
        .with_children(|panel| {
            spawn_ui_text(panel, "Controls", 20.0, Color::WHITE);
            for line in CONTROL_HINTS {
                spawn_ui_text(panel, line, 16.0, Color::WHITE);
            }
            spawn_ui_spacer(panel, 10.0);
            spawn_ui_text(panel, "Legend", 20.0, Color::WHITE);
            for line in LEGEND_LINES {
                spawn_ui_text(panel, line, 16.0, Color::WHITE);
            }
        });
}

fn draw_pause_menu(parent: &mut ChildSpawner, selected: PauseMenuEntry, status: Option<&str>) {
    spawn_modal(parent, 560.0, 360.0, |panel| {
        spawn_ui_text(panel, "Paused", 34.0, Color::WHITE);
        spawn_ui_spacer(panel, 18.0);
        for entry in PauseMenuEntry::ALL {
            spawn_menu_entry(panel, entry.label(), selected == entry);
        }
        if let Some(message) = status {
            spawn_ui_spacer(panel, 10.0);
            spawn_ui_text(panel, message, 18.0, color_rgba(190, 204, 214, 255));
        }
    });
}

fn draw_inventory_menu(parent: &mut ChildSpawner, entries: &[(String, bool)]) {
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

        for (label, selected) in entries {
            spawn_menu_entry(panel, label, *selected);
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
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            column_gap: px(18),
            min_height: px(34),
            align_items: AlignItems::Center,
            ..Default::default()
        },))
        .with_children(|row| {
            spawn_ui_text(row, marker, 26.0, color);
            spawn_ui_text(row, label, 26.0, color);
        });
}

fn spawn_ui_text(parent: &mut ChildSpawner, text: impl Into<String>, font_size: f32, color: Color) {
    parent.spawn((
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
    parent.spawn((Node {
        height: px(height),
        ..Default::default()
    },));
}

struct SpriteRender {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    z: f32,
    color: Color,
    visible: bool,
}

struct TextRender {
    text: String,
    x: f32,
    y: f32,
    z: f32,
    color: Color,
    font_size: f32,
    visible: bool,
}

fn set_keyed_sprite(
    world: &mut World,
    key: SceneRenderKey,
    live_parts: &mut HashSet<SceneRenderKey>,
    render: SpriteRender,
) {
    live_parts.insert(key);
    let entity = ensure_scene_sprite(world, key);
    set_sprite(world, entity, render);
}

fn set_keyed_text(
    world: &mut World,
    key: SceneRenderKey,
    live_parts: &mut HashSet<SceneRenderKey>,
    render: TextRender,
) {
    live_parts.insert(key);
    let entity = ensure_scene_text(world, key);
    set_text(world, entity, render);
}

fn ensure_scene_sprite(world: &mut World, key: SceneRenderKey) -> Entity {
    if let Some(entity) = scene_part_entity(world, key) {
        return entity;
    }
    world
        .spawn((
            SceneRenderPart { key },
            Sprite::default(),
            Transform::default(),
            Visibility::default(),
        ))
        .id()
}

fn ensure_scene_text(world: &mut World, key: SceneRenderKey) -> Entity {
    if let Some(entity) = scene_part_entity(world, key) {
        return entity;
    }
    world
        .spawn((
            SceneRenderPart { key },
            Text2d::new(""),
            TextFont::default(),
            TextColor(Color::WHITE),
            Transform::default(),
            Visibility::default(),
        ))
        .id()
}

fn scene_part_entity(world: &mut World, key: SceneRenderKey) -> Option<Entity> {
    let mut query = world.query::<(Entity, &SceneRenderPart)>();
    query
        .iter(world)
        .find(|(_, part)| part.key == key)
        .map(|(entity, _)| entity)
}

fn set_sprite(world: &mut World, entity: Entity, render: SpriteRender) {
    let dimensions = window_dimensions(world);
    if let Some(mut sprite) = world.get_mut::<Sprite>(entity) {
        sprite.color = render.color;
        sprite.custom_size = Some(Vec2::new(render.width, render.height));
    }
    if let Some(mut transform) = world.get_mut::<Transform>(entity) {
        transform.translation = screen_point_to_world(
            dimensions,
            render.x + render.width / 2.0,
            render.y + render.height / 2.0,
            render.z,
        );
    }
    set_visibility(world, entity, render.visible);
}

fn set_text(world: &mut World, entity: Entity, render: TextRender) {
    let dimensions = window_dimensions(world);
    if let Some(mut text) = world.get_mut::<Text2d>(entity) {
        text.0 = render.text;
    }
    if let Some(mut font) = world.get_mut::<TextFont>(entity) {
        font.font_size = FontSize::Px(render.font_size);
    }
    if let Some(mut color) = world.get_mut::<TextColor>(entity) {
        color.0 = render.color;
    }
    if let Some(mut transform) = world.get_mut::<Transform>(entity) {
        transform.translation = screen_point_to_world(dimensions, render.x, render.y, render.z);
    }
    set_visibility(world, entity, render.visible);
}

fn set_visibility(world: &mut World, entity: Entity, visible: bool) {
    if let Some(mut visibility) = world.get_mut::<Visibility>(entity) {
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn despawn_stale_scene_parts(world: &mut World, live_parts: &HashSet<SceneRenderKey>) {
    let stale = {
        let mut query = world.query::<(Entity, &SceneRenderPart)>();
        query
            .iter(world)
            .filter_map(|(entity, part)| (!live_parts.contains(&part.key)).then_some(entity))
            .collect::<Vec<_>>()
    };
    for entity in stale {
        world.entity_mut(entity).despawn();
    }
}

fn window_dimensions(world: &mut World) -> Vec2 {
    let mut query = world.query_filtered::<&Window, With<PrimaryWindow>>();
    query
        .iter(world)
        .next()
        .map(|window| Vec2::new(window.resolution.width(), window.resolution.height()))
        .unwrap_or(Vec2::new(
            initial_window_width() as f32,
            initial_window_height() as f32,
        ))
}

fn screen_point_to_world(dimensions: Vec2, x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x - dimensions.x / 2.0, dimensions.y / 2.0 - y, z)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_render_systems_spawn_world_without_default_hud() {
        let mut app = App::new();
        crate::world_setup::init_world(app.world_mut());
        app.init_resource::<MenuUiFingerprint>()
            .add_systems(Startup, spawn_scene_view)
            .add_systems(
                Update,
                (sync_rendered_scene, sync_menu_ui, update_hud).chain(),
            );

        app.update();

        let mut scene_parts = app
            .world_mut()
            .query_filtered::<Entity, With<SceneRenderPart>>();
        let scene_part_count = scene_parts.iter(app.world()).count();
        let mut sprites = app.world_mut().query::<&Sprite>();
        let sprite_count = sprites.iter(app.world()).count();
        let mut nodes = app.world_mut().query::<&Node>();
        let node_count = nodes.iter(app.world()).count();

        assert!(scene_part_count > 0);
        assert!(sprite_count > 0, "world scene should spawn sprite entities");
        assert_eq!(node_count, 0, "HUD should not be visible by default");
    }

    #[test]
    fn help_modal_spawns_hud_ui_on_demand() {
        let mut app = App::new();
        crate::world_setup::init_world(app.world_mut());
        app.init_resource::<MenuUiFingerprint>()
            .add_systems(Startup, spawn_scene_view)
            .add_systems(
                Update,
                (sync_rendered_scene, sync_menu_ui, update_hud).chain(),
            );

        *app.world_mut().resource_mut::<GameScreen>() = GameScreen::HelpModal;
        app.update();

        let mut hud_roots = app.world_mut().query_filtered::<Entity, With<HudRoot>>();
        let hud_root_count = hud_roots.iter(app.world()).count();
        let mut texts = app.world_mut().query::<&Text>();
        let text_count = texts.iter(app.world()).count();

        assert_eq!(hud_root_count, 1);
        assert!(text_count > 0, "help modal should spawn HUD text");
    }
}
