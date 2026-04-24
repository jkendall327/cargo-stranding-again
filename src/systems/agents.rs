use bevy_ecs::prelude::*;

use crate::components::*;
use crate::energy::{ActionEnergy, DEFAULT_ACTION_ENERGY_COST, PICKUP_ENERGY_COST};
use crate::map::Map;
use crate::movement::{resolve_movement, CargoLoad, MovementRequest};
use crate::resources::{Direction, EnergyTimeline, SimulationClock};

type AgentJobItem<'a> = (
    Entity,
    &'a mut Position,
    &'a mut Velocity,
    &'a mut Cargo,
    &'a mut AssignedJob,
    &'a mut ActionEnergy,
);

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
    timeline: Res<EnergyTimeline>,
    mut clock: ResMut<SimulationClock>,
    mut agents: Query<AgentJobItem, With<Agent>>,
    mut parcels: Query<(&Position, &CargoParcel, &mut ParcelState), Without<Agent>>,
) {
    let map = &*map;
    let now = timeline.now;
    for (agent_entity, mut position, mut velocity, mut cargo, mut job, mut energy) in &mut agents {
        velocity.dx = 0;
        velocity.dy = 0;

        for _ in 0..128 {
            if !energy.is_ready(now) {
                break;
            }

            let Some(parcel_entity) = job.parcel else {
                break;
            };
            let Ok((parcel_position, parcel, mut parcel_state)) = parcels.get_mut(parcel_entity)
            else {
                job.phase = JobPhase::FindParcel;
                job.parcel = None;
                break;
            };

            match job.phase {
                JobPhase::FindParcel | JobPhase::Done => break,
                JobPhase::GoToParcel => {
                    if *parcel_state != ParcelState::AssignedTo(agent_entity) {
                        job.phase = JobPhase::FindParcel;
                        job.parcel = None;
                        break;
                    }

                    if position.x == parcel_position.x && position.y == parcel_position.y {
                        *parcel_state = ParcelState::CarriedBy(agent_entity);
                        cargo.current_weight += parcel.weight;
                        job.phase = JobPhase::GoToDepot;
                        energy.spend(now, PICKUP_ENERGY_COST);
                        continue;
                    }

                    if let Some(moved) = greedy_step(
                        map,
                        agent_entity,
                        &mut position,
                        cargo.current_weight,
                        cargo.max_weight,
                        *parcel_position,
                    ) {
                        velocity.dx = moved.actual_delta.0;
                        velocity.dy = moved.actual_delta.1;
                        energy.spend(now, moved.energy_cost);
                    } else {
                        energy.spend(now, DEFAULT_ACTION_ENERGY_COST);
                    }
                }
                JobPhase::GoToDepot => {
                    if position.x == map.depot.0 && position.y == map.depot.1 {
                        *parcel_state = ParcelState::Delivered;
                        cargo.current_weight = (cargo.current_weight - parcel.weight).max(0.0);
                        clock.delivered_parcels += 1;
                        job.phase = JobPhase::Done;
                        job.parcel = None;
                        energy.spend(now, PICKUP_ENERGY_COST);
                        continue;
                    }

                    if let Some(moved) = greedy_step(
                        map,
                        agent_entity,
                        &mut position,
                        cargo.current_weight,
                        cargo.max_weight,
                        Position {
                            x: map.depot.0,
                            y: map.depot.1,
                        },
                    ) {
                        velocity.dx = moved.actual_delta.0;
                        velocity.dy = moved.actual_delta.1;
                        energy.spend(now, moved.energy_cost);
                    } else {
                        energy.spend(now, DEFAULT_ACTION_ENERGY_COST);
                    }
                }
            }
        }
    }
}

fn greedy_step(
    map: &Map,
    entity: Entity,
    position: &mut Position,
    current_weight: f32,
    max_weight: f32,
    target: Position,
) -> Option<crate::movement::MovementResult> {
    let dx = (target.x - position.x).signum();
    let dy = (target.y - position.y).signum();
    let candidates = if (target.x - position.x).abs() >= (target.y - position.y).abs() {
        [(dx, 0), (0, dy), (0, -dy), (-dx, 0)]
    } else {
        [(0, dy), (dx, 0), (-dx, 0), (0, -dy)]
    };

    for (step_x, step_y) in candidates {
        if step_x == 0 && step_y == 0 {
            continue;
        }
        let Some(direction) = direction_from_delta(step_x, step_y) else {
            continue;
        };
        let mut request = MovementRequest::walking(*position, direction);
        request.entity = Some(entity);
        request.cargo = CargoLoad {
            current_weight,
            max_weight,
        };

        let outcome = resolve_movement(map, request);
        if let Some(result) = outcome.moved() {
            position.x = result.target.x;
            position.y = result.target.y;
            return Some(result);
        }
    }

    None
}

fn direction_from_delta(dx: i32, dy: i32) -> Option<Direction> {
    match (dx, dy) {
        (-1, 0) => Some(Direction::West),
        (1, 0) => Some(Direction::East),
        (0, -1) => Some(Direction::North),
        (0, 1) => Some(Direction::South),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn_test_agent(world: &mut World, id: usize, position: Position) {
        world.spawn((
            Agent { id },
            position,
            Velocity::default(),
            Cargo {
                current_weight: 0.0,
                max_weight: 35.0,
            },
            AssignedJob {
                phase: JobPhase::FindParcel,
                parcel: None,
            },
            ActionEnergy::default(),
        ));
    }

    fn spawn_test_parcel(world: &mut World, position: Position) {
        world.spawn((position, CargoParcel { weight: 5.0 }, ParcelState::Loose));
    }

    #[test]
    fn agents_reserve_distinct_loose_parcels() {
        let mut world = World::new();
        spawn_test_agent(&mut world, 0, Position { x: 0, y: 0 });
        spawn_test_agent(&mut world, 1, Position { x: 1, y: 0 });
        spawn_test_parcel(&mut world, Position { x: 2, y: 0 });
        spawn_test_parcel(&mut world, Position { x: 3, y: 0 });

        let mut schedule = Schedule::default();
        schedule.add_systems(assign_agent_jobs);
        schedule.run(&mut world);

        let mut job_query = world.query::<&AssignedJob>();
        let assigned_jobs = job_query
            .iter(&world)
            .filter(|job| matches!(job.phase, JobPhase::GoToParcel) && job.parcel.is_some())
            .count();
        assert_eq!(assigned_jobs, 2);

        let mut parcel_query = world.query::<&ParcelState>();
        let reserved_parcels = parcel_query
            .iter(&world)
            .filter(|state| matches!(state, ParcelState::AssignedTo(_)))
            .count();
        assert_eq!(reserved_parcels, 2);
    }

    #[test]
    fn agent_picks_up_and_delivers_parcel_to_depot() {
        let mut world = World::new();
        let map = Map::generate();
        let depot = map.depot;
        world.insert_resource(map);
        world.insert_resource(SimulationClock {
            turn: 0,
            delivered_parcels: 0,
        });
        world.insert_resource(EnergyTimeline::default());
        spawn_test_agent(
            &mut world,
            0,
            Position {
                x: depot.0,
                y: depot.1,
            },
        );
        spawn_test_parcel(
            &mut world,
            Position {
                x: depot.0,
                y: depot.1,
            },
        );

        let mut schedule = Schedule::default();
        schedule.add_systems((assign_agent_jobs, agent_jobs));
        for _ in 0..12 {
            schedule.run(&mut world);
            world.resource_mut::<EnergyTimeline>().now += 100;
        }

        let clock = world.resource::<SimulationClock>();
        assert_eq!(clock.delivered_parcels, 1);

        let mut parcel_query = world.query::<&ParcelState>();
        let delivered_parcels = parcel_query
            .iter(&world)
            .filter(|state| matches!(state, ParcelState::Delivered))
            .count();
        assert_eq!(delivered_parcels, 1);

        let mut cargo_query = world.query_filtered::<&Cargo, With<Agent>>();
        let empty_agents = cargo_query
            .iter(&world)
            .filter(|cargo| cargo.current_weight == 0.0)
            .count();
        assert_eq!(empty_agents, 1);
    }
}
