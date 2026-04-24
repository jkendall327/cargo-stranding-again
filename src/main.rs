mod components;
mod input;
mod map;
mod render;
mod resources;
mod systems;

use bevy_ecs::prelude::*;
use input::{KeyBinding, KeyBindings};
use macroquad::prelude::*;

use components::*;
use map::Map;
use render::window_conf;
use resources::{
    Camera, GameScreen, InputRepeat, MenuAction, MenuInputState, PauseMenuState, PlayerAction,
    PlayerIntent, SimulationClock, TurnState,
};

#[macroquad::main(window_conf)]
async fn main() {
    let mut world = World::new();
    init_world(&mut world);

    let mut player_schedule = Schedule::default();
    player_schedule.add_systems(systems::player_movement);

    let mut menu_schedule = Schedule::default();
    menu_schedule.add_systems(systems::menu_navigation);

    let mut simulation_schedule = Schedule::default();
    simulation_schedule.add_systems((
        systems::tick_clock,
        systems::tick_cooldowns,
        systems::assign_agent_jobs,
        systems::agent_jobs,
    ));

    loop {
        // Macroquad owns the outer async frame loop and immediate-mode input.
        // Each frame we copy only the compact input intent into an ECS resource.
        copy_input_to_ecs(&mut world);
        menu_schedule.run(&mut world);

        // Bevy ECS owns simulation state, but the sim is turn-based: first the
        // player action is resolved, then NPC jobs and the clock advance only
        // if that action actually consumed a turn.
        if world.resource::<GameScreen>().allows_simulation()
            && world.resource::<PlayerIntent>().has_action()
        {
            player_schedule.run(&mut world);
            if world.resource::<TurnState>().consumed {
                simulation_schedule.run(&mut world);
            }
        }

        // Rendering is deliberately a plain Macroquad function that manually
        // queries ECS state. This keeps drawing separate from deterministic sim.
        render::render(&mut world);
        next_frame().await;
    }
}

fn init_world(world: &mut World) {
    world.insert_resource(Map::generate());
    world.insert_resource(GameScreen::default());
    world.insert_resource(PlayerIntent::default());
    world.insert_resource(MenuInputState::default());
    world.insert_resource(InputRepeat::default());
    world.insert_resource(KeyBindings::default());
    world.insert_resource(PauseMenuState::default());
    world.insert_resource(TurnState::default());
    world.insert_resource(Camera::default());
    world.insert_resource(SimulationClock {
        turn: 0,
        delivered_parcels: 0,
    });

    world.spawn((
        Player,
        Position { x: 6, y: 6 },
        Velocity::default(),
        Cargo {
            current_weight: 12.0,
            max_weight: 40.0,
        },
        Stamina {
            current: 35.0,
            max: 35.0,
        },
    ));

    for (id, (x, y)) in [(0, (41, 30)), (1, (52, 26))] {
        world.spawn((
            Agent { id },
            Position { x, y },
            Velocity::default(),
            Cargo {
                current_weight: 0.0,
                max_weight: 35.0,
            },
            AssignedJob {
                phase: JobPhase::FindParcel,
                parcel: None,
            },
            StepCooldown::default(),
        ));
    }

    for (x, y, weight) in [
        (8, 8, 6.0),
        (18, 15, 9.0),
        (26, 33, 5.0),
        (36, 9, 8.0),
        (55, 19, 7.0),
    ] {
        world.spawn((
            Position { x, y },
            CargoParcel { weight },
            ParcelState::Loose,
        ));
    }
}

fn copy_input_to_ecs(world: &mut World) {
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
        GameScreen::Playing | GameScreen::OptionsMenu => None,
        GameScreen::PauseMenu => Some(action),
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
