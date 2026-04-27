use crate::map::TileCoord;
use crate::render::TILE_SIZE;
use crate::resources::Camera;

pub(super) const VIEWPORT_X: f32 = 24.0;
pub(super) const VIEWPORT_Y: f32 = 24.0;
pub(super) const UI_GAP: f32 = 28.0;
pub(super) const UI_PANEL_WIDTH: f32 = 1220.0;

/// Initial window width that fits the map viewport and the right-side debug UI.
pub(super) fn window_width_for_camera(tile_span: i32) -> i32 {
    (VIEWPORT_X * 2.0 + tile_span as f32 * TILE_SIZE + UI_GAP + UI_PANEL_WIDTH) as i32
}

pub(super) fn tile_to_screen(camera: Camera, coord: TileCoord) -> (f32, f32) {
    (
        VIEWPORT_X + (coord.x - camera.x) as f32 * TILE_SIZE,
        VIEWPORT_Y + (coord.y - camera.y) as f32 * TILE_SIZE,
    )
}

pub(super) fn viewport_width(camera: Camera) -> f32 {
    camera.width as f32 * TILE_SIZE
}

pub(super) fn viewport_height(camera: Camera) -> f32 {
    camera.height as f32 * TILE_SIZE
}

pub(super) fn ui_x(camera: Camera) -> f32 {
    VIEWPORT_X + viewport_width(camera) + UI_GAP
}
