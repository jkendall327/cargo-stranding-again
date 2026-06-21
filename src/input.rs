use bevy::input::ButtonInput;
use bevy::prelude::KeyCode;
use bevy_ecs::prelude::*;

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
                    keys: vec![KeyCode::ArrowLeft, KeyCode::KeyA],
                    action: PlayerAction::Move(Direction::West),
                },
                KeyBinding {
                    keys: vec![KeyCode::ArrowRight, KeyCode::KeyD],
                    action: PlayerAction::Move(Direction::East),
                },
                KeyBinding {
                    keys: vec![KeyCode::ArrowUp, KeyCode::KeyW],
                    action: PlayerAction::Move(Direction::North),
                },
                KeyBinding {
                    keys: vec![KeyCode::ArrowDown, KeyCode::KeyS],
                    action: PlayerAction::Move(Direction::South),
                },
                KeyBinding {
                    keys: vec![KeyCode::Space, KeyCode::Period],
                    action: PlayerAction::Wait,
                },
                KeyBinding {
                    keys: vec![KeyCode::KeyI, KeyCode::Tab],
                    action: PlayerAction::OpenInventory,
                },
                KeyBinding {
                    keys: vec![KeyCode::KeyE],
                    action: PlayerAction::PickUp,
                },
                KeyBinding {
                    keys: vec![KeyCode::ShiftLeft, KeyCode::ShiftRight],
                    action: PlayerAction::CycleMovementMode,
                },
            ],
            menu: vec![
                KeyBinding {
                    keys: vec![KeyCode::Escape],
                    action: MenuAction::Cancel,
                },
                KeyBinding {
                    keys: vec![KeyCode::ArrowUp, KeyCode::KeyW],
                    action: MenuAction::MoveSelectionUp,
                },
                KeyBinding {
                    keys: vec![KeyCode::ArrowDown, KeyCode::KeyS],
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

pub fn copy_to_ecs(world: &mut World, keyboard: &ButtonInput<KeyCode>, now_seconds: f64) {
    let game_screen = *world.resource::<GameScreen>();
    let menu_action = current_menu_action(game_screen, world.resource::<KeyBindings>(), keyboard);
    *world.resource_mut::<MenuInputState>() = MenuInputState {
        action: menu_action,
    };

    if game_screen.allows_simulation() && menu_action.is_none() {
        let keybindings = world.resource::<KeyBindings>();
        let held_action = current_held_action(keybindings, keyboard);
        let newly_pressed_action = current_pressed_action(keybindings, keyboard);
        let action = world.resource_mut::<InputRepeat>().action_for_frame(
            held_action,
            newly_pressed_action,
            now_seconds,
        );

        *world.resource_mut::<PlayerIntent>() = PlayerIntent { action };
    } else {
        world.resource_mut::<InputRepeat>().reset();
        *world.resource_mut::<PlayerIntent>() = PlayerIntent::default();
    }
}

fn current_menu_action(
    game_screen: GameScreen,
    keybindings: &KeyBindings,
    keyboard: &ButtonInput<KeyCode>,
) -> Option<MenuAction> {
    let action = pressed_action(&keybindings.menu, keyboard)?;
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

fn current_held_action(
    keybindings: &KeyBindings,
    keyboard: &ButtonInput<KeyCode>,
) -> Option<PlayerAction> {
    held_action(&keybindings.gameplay, keyboard)
}

fn current_pressed_action(
    keybindings: &KeyBindings,
    keyboard: &ButtonInput<KeyCode>,
) -> Option<PlayerAction> {
    pressed_action(&keybindings.gameplay, keyboard)
}

fn held_action<T: Copy>(bindings: &[KeyBinding<T>], keyboard: &ButtonInput<KeyCode>) -> Option<T> {
    bindings
        .iter()
        .find(|binding| binding.keys.iter().any(|key| keyboard.pressed(*key)))
        .map(|binding| binding.action)
}

fn pressed_action<T: Copy>(
    bindings: &[KeyBinding<T>],
    keyboard: &ButtonInput<KeyCode>,
) -> Option<T> {
    bindings
        .iter()
        .find(|binding| binding.keys.iter().any(|key| keyboard.just_pressed(*key)))
        .map(|binding| binding.action)
}
