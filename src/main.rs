mod components;
mod map;
mod render;
mod resources;
mod systems;

use bevy_ecs::prelude::*;
use macroquad::prelude::*;

use components::*;
use map::Map;
use render::window_conf;
use resources::{InputState, SimulationClock, TurnState};

#[macroquad::main(window_conf)]
async fn main() {
    let mut world = World::new();
    init_world(&mut world);

    let mut player_schedule = Schedule::default();
    player_schedule.add_systems(systems::player_movement);

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

        // Bevy ECS owns simulation state, but the sim is turn-based: first the
        // player action is resolved, then NPC jobs and the clock advance only
        // if that action actually consumed a turn.
        if world.resource::<InputState>().has_action() {
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
    world.insert_resource(InputState::default());
    world.insert_resource(TurnState::default());
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
    let mut input = world.resource_mut::<InputState>();
    *input = InputState::default();

    if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
        input.move_x = -1;
    } else if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
        input.move_x = 1;
    } else if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
        input.move_y = -1;
    } else if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
        input.move_y = 1;
    } else if is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Period) {
        input.wait = true;
    }
}
