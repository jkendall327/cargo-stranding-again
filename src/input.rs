use bevy_ecs::prelude::*;
use macroquad::prelude::*;

use crate::resources::{
    Direction, GameScreen, InputRepeat, MenuAction, MenuInputState, PlayerAction, PlayerIntent,
};

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
                    keys: vec![KeyCode::I, KeyCode::Tab],
                    action: PlayerAction::OpenInventory,
                },
                KeyBinding {
                    keys: vec![KeyCode::E],
                    action: PlayerAction::PickUp,
                },
                KeyBinding {
                    keys: vec![KeyCode::LeftShift, KeyCode::RightShift],
                    action: PlayerAction::CycleMovementMode,
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

pub fn copy_to_ecs(world: &mut World) {
    let game_screen = *world.resource::<GameScreen>();
    let menu_action = current_menu_action(game_screen, world.resource::<KeyBindings>());
    *world.resource_mut::<MenuInputState>() = MenuInputState {
        action: menu_action,
    };

    if game_screen.allows_simulation() && menu_action.is_none() {
        let keybindings = world.resource::<KeyBindings>();
        let held_action = current_held_action(keybindings);
        let newly_pressed_action = current_pressed_action(keybindings);
        let action = world.resource_mut::<InputRepeat>().action_for_frame(
            held_action,
            newly_pressed_action,
            get_time(),
        );

        *world.resource_mut::<PlayerIntent>() = PlayerIntent { action };
    } else {
        world.resource_mut::<InputRepeat>().reset();
        *world.resource_mut::<PlayerIntent>() = PlayerIntent::default();
    }
}

fn current_menu_action(game_screen: GameScreen, keybindings: &KeyBindings) -> Option<MenuAction> {
    let action = pressed_action(&keybindings.menu)?;
    if action == MenuAction::Cancel {
        return Some(action);
    }

    match game_screen {
        GameScreen::Playing => None,
        GameScreen::PauseMenu | GameScreen::InventoryMenu => Some(action),
        GameScreen::OptionsMenu => {
            if action == MenuAction::Cancel {
                Some(action)
            } else {
                None
            }
        }
    }
}

fn current_held_action(keybindings: &KeyBindings) -> Option<PlayerAction> {
    held_action(&keybindings.gameplay)
}

fn current_pressed_action(keybindings: &KeyBindings) -> Option<PlayerAction> {
    pressed_action(&keybindings.gameplay)
}

fn held_action<T: Copy>(bindings: &[KeyBinding<T>]) -> Option<T> {
    bindings
        .iter()
        .find(|binding| binding.keys.iter().copied().any(is_key_down))
        .map(|binding| binding.action)
}

fn pressed_action<T: Copy>(bindings: &[KeyBinding<T>]) -> Option<T> {
    bindings
        .iter()
        .find(|binding| binding.keys.iter().copied().any(is_key_pressed))
        .map(|binding| binding.action)
}
