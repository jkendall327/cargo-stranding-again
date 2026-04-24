use bevy_ecs::prelude::*;

use crate::components::Position;

pub const DEFAULT_CAMERA_TILE_SPAN: i32 = 31;
pub const INPUT_REPEAT_INITIAL_DELAY: f64 = 0.18;
pub const INPUT_REPEAT_INTERVAL: f64 = 0.08;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputAction {
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    Wait,
}

impl InputAction {
    pub fn to_input_state(self) -> InputState {
        match self {
            Self::MoveLeft => InputState {
                move_x: -1,
                ..Default::default()
            },
            Self::MoveRight => InputState {
                move_x: 1,
                ..Default::default()
            },
            Self::MoveUp => InputState {
                move_y: -1,
                ..Default::default()
            },
            Self::MoveDown => InputState {
                move_y: 1,
                ..Default::default()
            },
            Self::Wait => InputState {
                wait: true,
                ..Default::default()
            },
        }
    }
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct InputRepeat {
    held_action: Option<InputAction>,
    next_repeat_at: f64,
}

impl InputRepeat {
    pub fn action_for_frame(
        &mut self,
        held_action: Option<InputAction>,
        newly_pressed_action: Option<InputAction>,
        now: f64,
    ) -> Option<InputAction> {
        let Some(action) = held_action else {
            self.held_action = None;
            return None;
        };

        if newly_pressed_action == Some(action) || self.held_action != Some(action) {
            self.held_action = Some(action);
            self.next_repeat_at = now + INPUT_REPEAT_INITIAL_DELAY;
            return Some(action);
        }

        if now >= self.next_repeat_at {
            self.next_repeat_at = now + INPUT_REPEAT_INTERVAL;
            return Some(action);
        }

        None
    }
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct InputState {
    pub move_x: i32,
    pub move_y: i32,
    pub wait: bool,
}

impl InputState {
    pub fn has_action(self) -> bool {
        self.wait || self.move_x != 0 || self.move_y != 0
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct SimulationClock {
    pub turn: u64,
    pub delivered_parcels: u32,
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct TurnState {
    pub consumed: bool,
}

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

    pub fn center_on(&mut self, position: Position, map_width: i32, map_height: i32) {
        self.x = clamp_axis(position.x - self.width / 2, self.width, map_width);
        self.y = clamp_axis(position.y - self.height / 2, self.height, map_height);
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

fn clamp_axis(origin: i32, view_size: i32, map_size: i32) -> i32 {
    if view_size >= map_size {
        0
    } else {
        origin.clamp(0, map_size - view_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_centers_on_player_until_it_hits_map_edges() {
        let mut camera = Camera::square(31);

        camera.center_on(Position { x: 30, y: 20 }, 60, 40);
        assert_eq!((camera.x, camera.y), (15, 5));

        camera.center_on(Position { x: 2, y: 2 }, 60, 40);
        assert_eq!((camera.x, camera.y), (0, 0));

        camera.center_on(Position { x: 58, y: 38 }, 60, 40);
        assert_eq!((camera.x, camera.y), (29, 9));
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

    #[test]
    fn input_repeat_emits_on_press_then_after_delay() {
        let mut repeat = InputRepeat::default();

        assert_eq!(
            repeat.action_for_frame(Some(InputAction::MoveUp), Some(InputAction::MoveUp), 0.0),
            Some(InputAction::MoveUp)
        );
        assert_eq!(
            repeat.action_for_frame(Some(InputAction::MoveUp), None, 0.1),
            None
        );
        assert_eq!(
            repeat.action_for_frame(Some(InputAction::MoveUp), None, INPUT_REPEAT_INITIAL_DELAY),
            Some(InputAction::MoveUp)
        );
        assert_eq!(
            repeat.action_for_frame(
                Some(InputAction::MoveUp),
                None,
                INPUT_REPEAT_INITIAL_DELAY + INPUT_REPEAT_INTERVAL
            ),
            Some(InputAction::MoveUp)
        );
    }

    #[test]
    fn input_repeat_resets_when_key_is_released() {
        let mut repeat = InputRepeat::default();

        assert_eq!(
            repeat.action_for_frame(Some(InputAction::Wait), Some(InputAction::Wait), 0.0),
            Some(InputAction::Wait)
        );
        assert_eq!(repeat.action_for_frame(None, None, 0.05), None);
        assert_eq!(
            repeat.action_for_frame(Some(InputAction::Wait), Some(InputAction::Wait), 0.06),
            Some(InputAction::Wait)
        );
    }
}
