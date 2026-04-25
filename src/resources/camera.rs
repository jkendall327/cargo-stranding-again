use bevy_ecs::prelude::*;

use crate::components::Position;
use crate::map::{MapBounds, TileCoord};

pub const DEFAULT_CAMERA_TILE_SPAN: i32 = 31;

#[derive(Resource, Clone, Copy, Debug)]
pub struct Camera {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Camera {
    pub fn square(tile_span: i32) -> Self {
        let tile_span = tile_span.max(1);
        Self {
            x: 0,
            y: 0,
            width: tile_span,
            height: tile_span,
        }
    }

    /// Centers the camera on an ECS position while clamping to finite map bounds.
    pub fn center_on(&mut self, position: Position, bounds: MapBounds) {
        let coord = TileCoord::from(position);
        self.x = clamp_axis(
            coord.x - self.width / 2,
            self.width,
            bounds.min_x,
            bounds.max_x(),
        );
        self.y = clamp_axis(
            coord.y - self.height / 2,
            self.height,
            bounds.min_y,
            bounds.max_y(),
        );
    }

    /// Returns the camera's top-left tile as a world tile coordinate.
    pub fn origin_coord(self) -> TileCoord {
        TileCoord::new(self.x, self.y)
    }

    pub fn contains(self, position: Position) -> bool {
        position.x >= self.x
            && position.y >= self.y
            && position.x < self.x + self.width
            && position.y < self.y + self.height
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::square(DEFAULT_CAMERA_TILE_SPAN)
    }
}

fn clamp_axis(origin: i32, view_size: i32, min: i32, max: i32) -> i32 {
    let map_size = max - min;
    if view_size >= map_size {
        min
    } else {
        origin.clamp(min, max - view_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_centers_on_player_until_it_hits_map_edges() {
        let mut camera = Camera::square(31);

        camera.center_on(Position { x: 30, y: 20 }, MapBounds::new(0, 0, 60, 40));
        assert_eq!((camera.x, camera.y), (15, 5));

        camera.center_on(Position { x: 2, y: 2 }, MapBounds::new(0, 0, 60, 40));
        assert_eq!((camera.x, camera.y), (0, 0));

        camera.center_on(Position { x: 58, y: 38 }, MapBounds::new(0, 0, 60, 40));
        assert_eq!((camera.x, camera.y), (29, 9));

        camera.center_on(
            Position { x: -14, y: -14 },
            MapBounds::new(-16, -16, 76, 56),
        );
        assert_eq!((camera.x, camera.y), (-16, -16));
    }

    #[test]
    fn camera_contains_positions_inside_visible_tiles() {
        let camera = Camera {
            x: 10,
            y: 5,
            width: 31,
            height: 31,
        };

        assert!(camera.contains(Position { x: 10, y: 5 }));
        assert!(camera.contains(Position { x: 40, y: 35 }));
        assert!(!camera.contains(Position { x: 41, y: 35 }));
        assert!(!camera.contains(Position { x: 40, y: 36 }));
    }
}
