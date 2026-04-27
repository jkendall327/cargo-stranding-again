use bevy_ecs::prelude::*;

use crate::ai::pathing::first_step_toward;
use crate::cargo::{
    derived_load, Cargo, CargoParcel, CargoTarget, CarriedBy, CarrySlot, ContainedIn, Container,
    ParcelDelivery,
};
use crate::components::*;
use crate::energy::{ActionEnergy, DEFAULT_ACTION_ENERGY_COST};
use crate::map::{Map, TileCoord};
use crate::movement::{resolve_movement, CargoLoad, MovementRequest};
use crate::resources::EnergyTimeline;
use crate::systems::{DeliverRequest, PickUpRequest};

type ParcelCarryState<'a> = (
    &'a ParcelDelivery,
    Option<&'a Position>,
    Option<&'a CarriedBy>,
    Option<&'a ContainedIn>,
);
type AssignableParcelDelivery<'a> = (
    Entity,
    &'a mut ParcelDelivery,
    Option<&'a Position>,
    Option<&'a CarriedBy>,
    Option<&'a ContainedIn>,
);

pub fn update_porter_action_interest(
    mut commands: Commands,
    parcels: Query<ParcelCarryState, With<CargoParcel>>,
    porters: Query<(Entity, &AssignedJob), With<Porter>>,
) {
    let has_loose_parcel = parcels
        .iter()
        .any(|(delivery, position, carried_by, contained_in)| {
            matches!(*delivery, ParcelDelivery::Available)
                && position.is_some()
                && carried_by.is_none()
                && contained_in.is_none()
        });

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
    mut parcels: Query<AssignableParcelDelivery, With<CargoParcel>>,
    mut porters: Query<(Entity, &mut AssignedJob), With<Porter>>,
) {
    for (porter_entity, mut job) in &mut porters {
        if job.parcel.is_some() && job.phase != JobPhase::Done {
            continue;
        }

        if let Some((parcel_entity, mut delivery, _, _, _)) =
            parcels
                .iter_mut()
                .find(|(_, delivery, position, carried_by, contained_in)| {
                    matches!(**delivery, ParcelDelivery::Available)
                        && position.is_some()
                        && carried_by.is_none()
                        && contained_in.is_none()
                })
        {
            *delivery = ParcelDelivery::ReservedBy(porter_entity);
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
        match snapshot.job.phase {
            JobPhase::FindParcel | JobPhase::Done => {}
            JobPhase::GoToParcel => {
                let Some((parcel_position, parcel_state)) = parcel_snapshot(world, parcel_entity)
                else {
                    clear_porter_job(world, porter_entity);
                    continue;
                };

                if parcel_state != ParcelDelivery::ReservedBy(porter_entity) {
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

fn parcel_snapshot(world: &mut World, parcel_entity: Entity) -> Option<(Position, ParcelDelivery)> {
    let mut query = world.query::<(&Position, &ParcelDelivery, &CargoParcel)>();
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

    let cargo_load = CargoLoad {
        current_weight: current_load,
        max_weight: cargo.max_weight,
    };
    let Some(direction) = first_step_toward(map, porter_entity, *position, target, cargo_load)
    else {
        energy.spend(now, DEFAULT_ACTION_ENERGY_COST);
        return;
    };

    let mut request = MovementRequest::walking(*position, direction);
    request.entity = Some(porter_entity);
    request.cargo = cargo_load;

    if let Some(moved) = resolve_movement(map, request).moved() {
        position.x = moved.target.x;
        position.y = moved.target.y;
        velocity.dx = moved.actual_delta.0;
        velocity.dy = moved.actual_delta.1;
        energy.spend(now, moved.energy_cost);
    } else {
        energy.spend(now, DEFAULT_ACTION_ENERGY_COST);
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
        resolve_delivery_requests, resolve_drop_requests, resolve_pickup_requests,
        spend_energy_for_successful_cargo_actions, update_porter_jobs_from_cargo_results,
    };
    use bevy_ecs::schedule::ApplyDeferred;

    fn init_cargo_message_resources(world: &mut World) {
        world.insert_resource(crate::resources::InventoryMenuState::default());
        crate::messages::init_simulation_messages(world);
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
                crate::messages::maintain_cargo_messages,
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
            CargoParcel,
            ParcelDelivery::Available,
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

        let mut parcel_query = world.query::<&ParcelDelivery>();
        let reserved_parcels = parcel_query
            .iter(&world)
            .filter(|state| matches!(state, ParcelDelivery::ReservedBy(_)))
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

        let mut parcel_query = world.query::<&ParcelDelivery>();
        let delivered_parcels = parcel_query
            .iter(&world)
            .filter(|state| matches!(state, ParcelDelivery::Delivered))
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
