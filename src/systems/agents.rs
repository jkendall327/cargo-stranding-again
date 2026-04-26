use bevy_ecs::prelude::*;

use crate::cargo::{
    derived_load, Cargo, CargoParcel, CargoTarget, CarriedBy, CarrySlot, Container, ParcelState,
};
use crate::components::*;
use crate::energy::{ActionEnergy, DEFAULT_ACTION_ENERGY_COST};
use crate::map::{Map, TileCoord};
use crate::movement::{resolve_movement, CargoLoad, MovementRequest};
use crate::resources::{Direction, EnergyTimeline};
use crate::systems::{DeliverRequest, PickUpRequest};

pub fn update_porter_action_interest(
    mut commands: Commands,
    parcels: Query<&ParcelState, With<CargoParcel>>,
    porters: Query<(Entity, &AssignedJob), With<Porter>>,
) {
    let has_loose_parcel = parcels
        .iter()
        .any(|state| matches!(*state, ParcelState::Loose));

    for (porter_entity, job) in &porters {
        let has_active_job =
            job.parcel.is_some() && !matches!(job.phase, JobPhase::FindParcel | JobPhase::Done);
        if has_loose_parcel || has_active_job {
            commands.entity(porter_entity).insert(WantsAction);
        } else {
            commands.entity(porter_entity).remove::<WantsAction>();
        }
    }
}

pub fn assign_porter_jobs(
    mut parcels: Query<(Entity, &mut ParcelState), With<CargoParcel>>,
    mut porters: Query<(Entity, &mut AssignedJob), With<Porter>>,
) {
    for (porter_entity, mut job) in &mut porters {
        if job.parcel.is_some() && job.phase != JobPhase::Done {
            continue;
        }

        if let Some((parcel_entity, mut state)) = parcels
            .iter_mut()
            .find(|(_, state)| matches!(**state, ParcelState::Loose))
        {
            *state = ParcelState::AssignedTo(porter_entity);
            job.parcel = Some(parcel_entity);
            job.phase = JobPhase::GoToParcel;
        } else {
            job.parcel = None;
            job.phase = JobPhase::FindParcel;
        }
    }
}

pub fn porter_jobs(world: &mut World) {
    let map = world.resource::<Map>().clone();
    let now = world.resource::<EnergyTimeline>().now;
    let porter_entities = {
        let mut query = world.query_filtered::<Entity, (With<Porter>, With<WantsAction>)>();
        query.iter(world).collect::<Vec<_>>()
    };

    for porter_entity in porter_entities {
        let Some(snapshot) = ready_porter_snapshot(world, porter_entity, now) else {
            continue;
        };

        let Some(parcel_entity) = snapshot.job.parcel else {
            continue;
        };
        let Some((parcel_position, parcel_state)) = parcel_snapshot(world, parcel_entity) else {
            clear_porter_job(world, porter_entity);
            continue;
        };

        match snapshot.job.phase {
            JobPhase::FindParcel | JobPhase::Done => {}
            JobPhase::GoToParcel => {
                if parcel_state != ParcelState::AssignedTo(porter_entity) {
                    clear_porter_job(world, porter_entity);
                    continue;
                }

                if snapshot.position == parcel_position {
                    let target = carried_container_target(world, porter_entity);
                    world
                        .resource_mut::<Messages<PickUpRequest>>()
                        .write(PickUpRequest {
                            actor: porter_entity,
                            item: parcel_entity,
                            target,
                        });
                    tracing::debug!(
                        porter = ?porter_entity,
                        item = ?parcel_entity,
                        "porter pickup requested"
                    );
                    continue;
                }

                move_porter_toward(world, &map, porter_entity, parcel_position, now);
            }
            JobPhase::GoToDepot => {
                let depot = map.depot_coord();
                if TileCoord::from(snapshot.position) == depot {
                    world
                        .resource_mut::<Messages<DeliverRequest>>()
                        .write(DeliverRequest {
                            actor: porter_entity,
                            item: parcel_entity,
                        });
                    tracing::info!(
                        porter = ?porter_entity,
                        item = ?parcel_entity,
                        "porter delivery requested"
                    );
                    continue;
                }

                move_porter_toward(world, &map, porter_entity, Position::from(depot), now);
            }
        }
    }
}

fn carried_container_target(world: &mut World, actor: Entity) -> CargoTarget {
    let mut query = world.query_filtered::<(Entity, &CarriedBy), With<Container>>();
    query
        .iter(world)
        .filter_map(|(entity, carried_by)| (carried_by.holder == actor).then_some(entity))
        .min_by_key(|entity| entity.to_bits())
        .map_or(CargoTarget::Slot(CarrySlot::Back), CargoTarget::Container)
}

#[derive(Clone, Copy, Debug)]
struct PorterSnapshot {
    position: Position,
    job: AssignedJob,
}

fn ready_porter_snapshot(
    world: &mut World,
    porter_entity: Entity,
    now: u64,
) -> Option<PorterSnapshot> {
    let mut query = world.query::<(
        &Position,
        &mut Velocity,
        &AssignedJob,
        &ActionEnergy,
        &Porter,
    )>();
    let (position, mut velocity, job, energy, _) = query.get_mut(world, porter_entity).ok()?;
    if !energy.is_ready(now) {
        return None;
    }
    velocity.dx = 0;
    velocity.dy = 0;
    Some(PorterSnapshot {
        position: *position,
        job: *job,
    })
}

fn parcel_snapshot(world: &mut World, parcel_entity: Entity) -> Option<(Position, ParcelState)> {
    let mut query = world.query::<(&Position, &ParcelState, &CargoParcel)>();
    let (position, state, _) = query.get(world, parcel_entity).ok()?;
    Some((*position, *state))
}

fn clear_porter_job(world: &mut World, porter_entity: Entity) {
    set_porter_job(world, porter_entity, JobPhase::FindParcel, None);
}

fn set_porter_job(
    world: &mut World,
    porter_entity: Entity,
    phase: JobPhase,
    parcel: Option<Entity>,
) {
    if let Some(mut job) = world.get_mut::<AssignedJob>(porter_entity) {
        job.phase = phase;
        job.parcel = parcel;
    }
}

fn move_porter_toward(
    world: &mut World,
    map: &Map,
    porter_entity: Entity,
    target: Position,
    now: u64,
) {
    let current_load = derived_load(world, porter_entity);
    let mut query = world.query::<(&mut Position, &mut Velocity, &Cargo, &mut ActionEnergy)>();
    let Ok((mut position, mut velocity, cargo, mut energy)) = query.get_mut(world, porter_entity)
    else {
        return;
    };

    if let Some(moved) = greedy_step(
        map,
        porter_entity,
        &mut position,
        current_load,
        cargo.max_weight,
        target,
    ) {
        velocity.dx = moved.actual_delta.0;
        velocity.dy = moved.actual_delta.1;
        energy.spend(now, moved.energy_cost);
    } else {
        energy.spend(now, DEFAULT_ACTION_ENERGY_COST);
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
    use crate::cargo::{CargoStats, Item};
    use crate::energy::ITEM_ACTION_ENERGY_COST;
    use crate::resources::DeliveryStats;
    use crate::systems::{
        clamp_inventory_after_cargo_drop, clear_failed_porter_cargo_jobs, log_failed_cargo_actions,
        maintain_cargo_messages, resolve_delivery_requests, resolve_drop_requests,
        resolve_pickup_requests, spend_energy_for_successful_cargo_actions,
        update_porter_jobs_from_cargo_results, CargoActionResult, DeliverRequest, DropRequest,
        PickUpRequest,
    };
    use bevy_ecs::schedule::ApplyDeferred;

    fn init_cargo_message_resources(world: &mut World) {
        world.insert_resource(crate::resources::InventoryMenuState::default());
        world.init_resource::<Messages<PickUpRequest>>();
        world.init_resource::<Messages<DropRequest>>();
        world.init_resource::<Messages<DeliverRequest>>();
        world.init_resource::<Messages<CargoActionResult>>();
    }

    fn porter_job_schedule() -> Schedule {
        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                update_porter_action_interest,
                assign_porter_jobs,
                porter_jobs,
                resolve_pickup_requests,
                resolve_drop_requests,
                resolve_delivery_requests,
                ApplyDeferred,
                spend_energy_for_successful_cargo_actions,
                update_porter_jobs_from_cargo_results,
                clear_failed_porter_cargo_jobs,
                clamp_inventory_after_cargo_drop,
                log_failed_cargo_actions,
                maintain_cargo_messages,
            )
                .chain(),
        );
        schedule
    }

    fn spawn_test_porter(world: &mut World, id: usize, position: Position) {
        let porter = world
            .spawn((
                Actor,
                AutonomousActor,
                WantsAction,
                Porter { id },
                position,
                Velocity::default(),
                Cargo { max_weight: 35.0 },
                AssignedJob {
                    phase: JobPhase::FindParcel,
                    parcel: None,
                },
                ActionEnergy::default(),
            ))
            .id();
        world.spawn((
            Item,
            CargoStats {
                weight: 2.0,
                volume: 3.0,
            },
            Container {
                volume_capacity: 10.0,
                weight_capacity: 20.0,
            },
            CarriedBy {
                holder: porter,
                slot: CarrySlot::Back,
            },
        ));
    }

    fn spawn_test_parcel(world: &mut World, position: Position) {
        world.spawn((
            position,
            Item,
            CargoStats {
                weight: 5.0,
                volume: 1.0,
            },
            CargoParcel { weight: 5.0 },
            ParcelState::Loose,
        ));
    }

    #[test]
    fn porters_reserve_distinct_loose_parcels() {
        let mut world = World::new();
        spawn_test_porter(&mut world, 0, Position { x: 0, y: 0 });
        spawn_test_porter(&mut world, 1, Position { x: 1, y: 0 });
        spawn_test_parcel(&mut world, Position { x: 2, y: 0 });
        spawn_test_parcel(&mut world, Position { x: 3, y: 0 });

        let mut schedule = Schedule::default();
        schedule.add_systems(assign_porter_jobs);
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
    fn porter_picks_up_and_delivers_parcel_to_depot() {
        let mut world = World::new();
        let map = Map::generate();
        let depot = map.depot_coord();
        world.insert_resource(map);
        world.insert_resource(crate::resources::SimulationClock { turn: 0 });
        world.insert_resource(DeliveryStats::default());
        world.insert_resource(EnergyTimeline::default());
        init_cargo_message_resources(&mut world);
        spawn_test_porter(
            &mut world,
            0,
            Position {
                x: depot.x,
                y: depot.y,
            },
        );
        spawn_test_parcel(
            &mut world,
            Position {
                x: depot.x,
                y: depot.y,
            },
        );

        let mut schedule = porter_job_schedule();
        for _ in 0..12 {
            schedule.run(&mut world);
            world.resource_mut::<EnergyTimeline>().now += 100;
        }

        let delivery_stats = world.resource::<DeliveryStats>();
        assert_eq!(delivery_stats.delivered_parcels, 1);

        let mut parcel_query = world.query::<&ParcelState>();
        let delivered_parcels = parcel_query
            .iter(&world)
            .filter(|state| matches!(state, ParcelState::Delivered))
            .count();
        assert_eq!(delivered_parcels, 1);

        let mut cargo_query = world.query_filtered::<Entity, With<Porter>>();
        let porter = cargo_query
            .single(&world)
            .expect("test setup should leave exactly one porter");
        assert_eq!(derived_load(&mut world, porter), 2.0);
    }

    #[test]
    fn ready_porter_takes_only_one_job_action_per_schedule_run() {
        let mut world = World::new();
        let map = Map::generate();
        let depot = map.depot_coord();
        world.insert_resource(map);
        world.insert_resource(DeliveryStats::default());
        world.insert_resource(EnergyTimeline::default());
        init_cargo_message_resources(&mut world);
        spawn_test_porter(
            &mut world,
            0,
            Position {
                x: depot.x,
                y: depot.y,
            },
        );
        spawn_test_parcel(
            &mut world,
            Position {
                x: depot.x,
                y: depot.y,
            },
        );

        let mut schedule = porter_job_schedule();
        schedule.run(&mut world);

        assert_eq!(world.resource::<DeliveryStats>().delivered_parcels, 0);

        let mut parcel_query = world.query::<&ParcelState>();
        assert!(parcel_query
            .iter(&world)
            .any(|state| matches!(state, ParcelState::CarriedBy(_))));

        let mut carried_query = world.query::<&CarriedBy>();
        assert_eq!(carried_query.iter(&world).count(), 1);

        let mut porter_query =
            world.query_filtered::<(&AssignedJob, &ActionEnergy), With<Porter>>();
        let (job, energy) = porter_query
            .single(&world)
            .expect("test setup should leave exactly one porter");
        assert!(matches!(job.phase, JobPhase::GoToDepot));
        assert_eq!(energy.ready_at, u64::from(ITEM_ACTION_ENERGY_COST));
    }
}
