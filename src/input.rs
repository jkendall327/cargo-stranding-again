use bevy_ecs::prelude::*;
use macroquad::input::KeyCode;

use crate::resources::{Direction, MenuAction, PlayerAction};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyBinding<T> {
    pub keys: Vec<KeyCode>,
    pub action: T,
}

#[derive(Resource, Clone, Debug, Eq, PartialEq)]
pub struct KeyBindings {
    pub gameplay: Vec<KeyBinding<PlayerAction>>,
    pub menu: Vec<KeyBinding<MenuAction>>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            gameplay: vec![
                KeyBinding {
                    keys: vec![KeyCode::Left, KeyCode::A],
                    action: PlayerAction::Move(Direction::West),
                },
                KeyBinding {
                    keys: vec![KeyCode::Right, KeyCode::D],
                    action: PlayerAction::Move(Direction::East),
                },
                KeyBinding {
                    keys: vec![KeyCode::Up, KeyCode::W],
                    action: PlayerAction::Move(Direction::North),
                },
                KeyBinding {
                    keys: vec![KeyCode::Down, KeyCode::S],
                    action: PlayerAction::Move(Direction::South),
                },
                KeyBinding {
                    keys: vec![KeyCode::Space, KeyCode::Period],
                    action: PlayerAction::Wait,
                },
                KeyBinding {
                    keys: vec![KeyCode::E],
                    action: PlayerAction::PickUp,
                },
                KeyBinding {
                    keys: vec![KeyCode::LeftShift, KeyCode::RightShift],
                    action: PlayerAction::ToggleSprint,
                },
            ],
            menu: vec![
                KeyBinding {
                    keys: vec![KeyCode::Escape],
                    action: MenuAction::Cancel,
                },
                KeyBinding {
                    keys: vec![KeyCode::Up, KeyCode::W],
                    action: MenuAction::MoveSelectionUp,
                },
                KeyBinding {
                    keys: vec![KeyCode::Down, KeyCode::S],
                    action: MenuAction::MoveSelectionDown,
                },
                KeyBinding {
                    keys: vec![KeyCode::Enter, KeyCode::Space],
                    action: MenuAction::Confirm,
                },
            ],
        }
    }
}
