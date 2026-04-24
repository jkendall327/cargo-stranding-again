use bevy_ecs::prelude::*;

use crate::components::*;
use crate::map::Map;
use crate::resources::{InputState, SimulationClock};

type AgentJobItem<'a> = (
    Entity,
    &'a mut Position,
    &'a mut Velocity,
    &'a mut Cargo,
    &'a mut AssignedJob,
    &'a mut StepCooldown,
);

pub fn tick_clock(mut clock: ResMut<SimulationClock>) {
    clock.turn += 1;
}

pub fn tick_cooldowns(mut query: Query<&mut StepCooldown>) {
    for mut cooldown in &mut query {
        cooldown.frames = cooldown.frames.saturating_sub(1);
    }
}

pub fn player_movement(
    input: Res<InputState>,
    map: Res<Map>,
    mut query: Query<(&mut Position, &mut Velocity, &mut Stamina, &Cargo), With<Player>>,
) {
    let map = &*map;
    let Ok((mut position, mut velocity, mut stamina, cargo)) = query.get_single_mut() else {
        return;
    };

    velocity.dx = 0;
    velocity.dy = 0;

    if input.move_x == 0 && input.move_y == 0 {
        stamina.current = (stamina.current + 0.35).min(stamina.max);
        return;
    }

    let next_x = position.x + input.move_x;
    let next_y = position.y + input.move_y;
    let Some(terrain) = map.terrain_at(next_x, next_y) else {
        return;
    };
    if !terrain.passable() {
        return;
    }

    let load_factor = 1.0 + cargo.current_weight / cargo.max_weight.max(1.0);
    let stamina_cost = terrain.movement_cost() * load_factor;
    if stamina.current < stamina_cost {
        stamina.current = (stamina.current + 0.1).min(stamina.max);
        return;
    }

    position.x = next_x;
    position.y = next_y;
    velocity.dx = input.move_x;
    velocity.dy = input.move_y;
    stamina.current -= stamina_cost;
}

pub fn assign_agent_jobs(
    mut parcels: Query<(Entity, &mut ParcelState), With<CargoParcel>>,
    mut agents: Query<(Entity, &mut AssignedJob), With<Agent>>,
) {
    for (agent_entity, mut job) in &mut agents {
        if job.parcel.is_some() && job.phase != JobPhase::Done {
            continue;
        }

        if let Some((parcel_entity, mut state)) = parcels
            .iter_mut()
            .find(|(_, state)| matches!(**state, ParcelState::Loose))
        {
            *state = ParcelState::AssignedTo(agent_entity);
            job.parcel = Some(parcel_entity);
            job.phase = JobPhase::GoToParcel;
        } else {
            job.parcel = None;
            job.phase = JobPhase::FindParcel;
        }
    }
}

pub fn agent_jobs(
    map: Res<Map>,
    mut clock: ResMut<SimulationClock>,
    mut agents: Query<AgentJobItem, With<Agent>>,
    mut parcels: Query<(&Position, &CargoParcel, &mut ParcelState), Without<Agent>>,
) {
    let map = &*map;
    for (agent_entity, mut position, mut velocity, mut cargo, mut job, mut cooldown) in &mut agents
    {
        velocity.dx = 0;
        velocity.dy = 0;

        if cooldown.frames > 0 {
            continue;
        }

        let Some(parcel_entity) = job.parcel else {
            continue;
        };
        let Ok((parcel_position, parcel, mut parcel_state)) = parcels.get_mut(parcel_entity) else {
            job.phase = JobPhase::FindParcel;
            job.parcel = None;
            continue;
        };

        match job.phase {
            JobPhase::FindParcel | JobPhase::Done => {}
            JobPhase::GoToParcel => {
                if *parcel_state != ParcelState::AssignedTo(agent_entity) {
                    job.phase = JobPhase::FindParcel;
                    job.parcel = None;
                    continue;
                }

                if position.x == parcel_position.x && position.y == parcel_position.y {
                    *parcel_state = ParcelState::CarriedBy(agent_entity);
                    cargo.current_weight += parcel.weight;
                    job.phase = JobPhase::GoToDepot;
                    cooldown.frames = 10;
                    continue;
                }

                let moved = greedy_step(map, &mut position, parcel_position.x, parcel_position.y);
                velocity.dx = moved.0;
                velocity.dy = moved.1;
                cooldown.frames = step_delay(map, position.x, position.y);
            }
            JobPhase::GoToDepot => {
                if position.x == map.depot.0 && position.y == map.depot.1 {
                    *parcel_state = ParcelState::Delivered;
                    cargo.current_weight = (cargo.current_weight - parcel.weight).max(0.0);
                    clock.delivered_parcels += 1;
                    job.phase = JobPhase::Done;
                    job.parcel = None;
                    cooldown.frames = 18;
                    continue;
                }

                let moved = greedy_step(map, &mut position, map.depot.0, map.depot.1);
                velocity.dx = moved.0;
                velocity.dy = moved.1;
                cooldown.frames = step_delay(map, position.x, position.y);
            }
        }
    }
}

fn greedy_step(map: &Map, position: &mut Position, target_x: i32, target_y: i32) -> (i32, i32) {
    let dx = (target_x - position.x).signum();
    let dy = (target_y - position.y).signum();
    let candidates = if (target_x - position.x).abs() >= (target_y - position.y).abs() {
        [(dx, 0), (0, dy), (0, -dy), (-dx, 0)]
    } else {
        [(0, dy), (dx, 0), (-dx, 0), (0, -dy)]
    };

    for (step_x, step_y) in candidates {
        if step_x == 0 && step_y == 0 {
            continue;
        }
        let next_x = position.x + step_x;
        let next_y = position.y + step_y;
        if map.is_passable(next_x, next_y) {
            position.x = next_x;
            position.y = next_y;
            return (step_x, step_y);
        }
    }

    (0, 0)
}

fn step_delay(map: &Map, x: i32, y: i32) -> u32 {
    let cost = map
        .terrain_at(x, y)
        .map_or(1.0, |terrain| terrain.movement_cost());
    (6.0 + cost * 5.0) as u32
}
