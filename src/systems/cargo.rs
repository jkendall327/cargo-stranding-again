use bevy_ecs::prelude::*;

use crate::cargo::{
    Cargo, CargoError, CargoParcel, CargoStats, CarriedBy, CarrySlot, Item, ParcelState,
};
use crate::components::{ActionEnergy, AssignedJob, JobPhase, Player, Porter, Position};
use crate::energy::ITEM_ACTION_ENERGY_COST;
use crate::resources::{DeliveryStats, EnergyTimeline, InventoryMenuState};

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
/// Request to carry a loose or actor-assigned item in a slot.
pub struct PickUpRequest {
    pub actor: Entity,
    pub item: Entity,
    pub slot: CarrySlot,
}

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
/// Request to place a carried item at a world position.
pub struct DropRequest {
    pub actor: Entity,
    pub item: Entity,
    pub at: Position,
}

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
/// Request to mark a carried parcel as delivered.
pub struct DeliverRequest {
    pub actor: Entity,
    pub item: Entity,
}

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
/// Marks an actor whose cached cargo load should be derived again.
pub struct CargoChanged {
    pub actor: Entity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// The cargo mutation kind reported after request resolution.
pub enum CargoAction {
    PickUp,
    Drop,
    Deliver,
}

#[derive(Message, Clone, Copy, Debug, PartialEq)]
/// Result message consumed by cross-cutting maintenance such as energy and jobs.
pub struct CargoActionResult {
    pub actor: Entity,
    pub item: Entity,
    pub action: CargoAction,
    pub result: Result<(), CargoError>,
}

type ItemState<'a> = (
    Option<&'a Item>,
    Option<&'a CargoStats>,
    Option<&'a CarriedBy>,
    Option<&'a ParcelState>,
);

/// Resolves cargo requests into entity relationship changes.
///
/// Callers choose the intended target; this system owns the legality checks and
/// structural mutation so player, inventory, and autonomous actors share rules.
#[allow(clippy::too_many_arguments)]
pub fn resolve_cargo_requests(
    mut commands: Commands,
    mut pickup_requests: MessageReader<PickUpRequest>,
    mut drop_requests: MessageReader<DropRequest>,
    mut deliver_requests: MessageReader<DeliverRequest>,
    mut changed: MessageWriter<CargoChanged>,
    mut results: MessageWriter<CargoActionResult>,
    cargo: Query<&Cargo>,
    items: Query<ItemState>,
    carried_items: Query<(&CargoStats, &CarriedBy)>,
) {
    for request in pickup_requests.read() {
        let result = validate_pickup(&cargo, &items, &carried_items, request.actor, request.item);
        if result.is_ok() {
            commands.entity(request.item).insert(CarriedBy {
                holder: request.actor,
                slot: request.slot,
            });
            if has_parcel_state(&items, request.item) {
                commands
                    .entity(request.item)
                    .insert(ParcelState::CarriedBy(request.actor));
            }
            changed.write(CargoChanged {
                actor: request.actor,
            });
        }
        results.write(CargoActionResult {
            actor: request.actor,
            item: request.item,
            action: CargoAction::PickUp,
            result,
        });
    }

    for request in drop_requests.read() {
        let result = validate_drop(&cargo, &items, request.actor, request.item);
        if result.is_ok() {
            commands
                .entity(request.item)
                .remove::<CarriedBy>()
                .insert(request.at);
            if has_parcel_state(&items, request.item) {
                commands.entity(request.item).insert(ParcelState::Loose);
            }
            changed.write(CargoChanged {
                actor: request.actor,
            });
        }
        results.write(CargoActionResult {
            actor: request.actor,
            item: request.item,
            action: CargoAction::Drop,
            result,
        });
    }

    for request in deliver_requests.read() {
        let result = validate_delivery(&cargo, &items, request.actor, request.item);
        if result.is_ok() {
            commands
                .entity(request.item)
                .remove::<CarriedBy>()
                .insert(ParcelState::Delivered);
            changed.write(CargoChanged {
                actor: request.actor,
            });
        }
        results.write(CargoActionResult {
            actor: request.actor,
            item: request.item,
            action: CargoAction::Deliver,
            result,
        });
    }
}

pub fn refresh_changed_cargo_caches(
    mut changed: MessageReader<CargoChanged>,
    carried_items: Query<(&CargoStats, &CarriedBy)>,
    mut cargo: Query<&mut Cargo>,
) {
    for event in changed.read() {
        let load = derived_load_from_query(&carried_items, event.actor);
        if let Ok(mut cargo) = cargo.get_mut(event.actor) {
            cargo.current_weight = load;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_cargo_action_results(
    timeline: Res<EnergyTimeline>,
    mut results: MessageReader<CargoActionResult>,
    mut energy: Query<&mut ActionEnergy>,
    mut jobs: Query<&mut AssignedJob, With<Porter>>,
    players: Query<(), With<Player>>,
    carried_parcels: Query<&CarriedBy, With<CargoParcel>>,
    mut inventory_menu: ResMut<InventoryMenuState>,
    mut delivery_stats: ResMut<DeliveryStats>,
) {
    for event in results.read() {
        match event.result {
            Ok(()) => {
                if let Ok(mut energy) = energy.get_mut(event.actor) {
                    energy.spend(timeline.now, ITEM_ACTION_ENERGY_COST);
                }
                handle_successful_job_result(event, &mut jobs, &mut delivery_stats);
                clamp_player_inventory_after_drop(
                    event,
                    &players,
                    &carried_parcels,
                    &mut inventory_menu,
                );
            }
            Err(error) => {
                if jobs.get_mut(event.actor).is_ok() {
                    clear_failed_porter_job(event, &mut jobs);
                }
                tracing::debug!(
                    actor = ?event.actor,
                    item = ?event.item,
                    action = ?event.action,
                    ?error,
                    "cargo action failed"
                );
            }
        }
    }
}

pub fn maintain_cargo_messages(
    mut pickup_requests: ResMut<Messages<PickUpRequest>>,
    mut drop_requests: ResMut<Messages<DropRequest>>,
    mut deliver_requests: ResMut<Messages<DeliverRequest>>,
    mut changed: ResMut<Messages<CargoChanged>>,
    mut results: ResMut<Messages<CargoActionResult>>,
) {
    pickup_requests.update();
    drop_requests.update();
    deliver_requests.update();
    changed.update();
    results.update();
}

fn validate_pickup(
    cargo: &Query<&Cargo>,
    items: &Query<ItemState>,
    carried_items: &Query<(&CargoStats, &CarriedBy)>,
    actor: Entity,
    item: Entity,
) -> Result<(), CargoError> {
    let cargo = cargo.get(actor).map_err(|_| CargoError::MissingCargo)?;
    let (item_marker, stats, carried_by, parcel_state) =
        items.get(item).map_err(|_| CargoError::MissingItem)?;
    let stats = stats.ok_or(CargoError::MissingItem)?;

    if item_marker.is_none()
        || carried_by.is_some()
        || !parcel_can_be_picked_up_by(parcel_state, actor)
    {
        return Err(CargoError::NotLoose);
    }

    let current_load = derived_load_from_query(carried_items, actor);
    if current_load + stats.weight > cargo.max_weight {
        return Err(CargoError::CapacityExceeded);
    }

    Ok(())
}

fn validate_drop(
    cargo: &Query<&Cargo>,
    items: &Query<ItemState>,
    actor: Entity,
    item: Entity,
) -> Result<(), CargoError> {
    cargo.get(actor).map_err(|_| CargoError::MissingCargo)?;
    let (item_marker, stats, carried_by, _) =
        items.get(item).map_err(|_| CargoError::MissingItem)?;
    if item_marker.is_none() || stats.is_none() {
        return Err(CargoError::MissingItem);
    }
    let carried_by = carried_by.ok_or(CargoError::NotCarriedByHolder)?;
    if carried_by.holder != actor {
        return Err(CargoError::NotCarriedByHolder);
    }
    Ok(())
}

fn validate_delivery(
    cargo: &Query<&Cargo>,
    items: &Query<ItemState>,
    actor: Entity,
    item: Entity,
) -> Result<(), CargoError> {
    cargo.get(actor).map_err(|_| CargoError::MissingCargo)?;
    let (item_marker, stats, carried_by, parcel_state) =
        items.get(item).map_err(|_| CargoError::MissingItem)?;
    if item_marker.is_none() || stats.is_none() || parcel_state.is_none() {
        return Err(CargoError::MissingItem);
    }
    let carried_by = carried_by.ok_or(CargoError::NotCarriedByHolder)?;
    if carried_by.holder != actor {
        return Err(CargoError::NotCarriedByHolder);
    }
    Ok(())
}

fn parcel_can_be_picked_up_by(parcel_state: Option<&ParcelState>, actor: Entity) -> bool {
    match parcel_state {
        None => true,
        Some(ParcelState::Loose) => true,
        Some(ParcelState::AssignedTo(assigned_actor)) => *assigned_actor == actor,
        Some(ParcelState::CarriedBy(_) | ParcelState::Delivered) => false,
    }
}

fn has_parcel_state(items: &Query<ItemState>, item: Entity) -> bool {
    items
        .get(item)
        .is_ok_and(|(_, _, _, parcel_state)| parcel_state.is_some())
}

fn derived_load_from_query(carried_items: &Query<(&CargoStats, &CarriedBy)>, actor: Entity) -> f32 {
    carried_items
        .iter()
        .filter_map(|(stats, carried_by)| (carried_by.holder == actor).then_some(stats.weight))
        .sum()
}

fn handle_successful_job_result(
    event: &CargoActionResult,
    jobs: &mut Query<&mut AssignedJob, With<Porter>>,
    delivery_stats: &mut DeliveryStats,
) {
    let Ok(mut job) = jobs.get_mut(event.actor) else {
        return;
    };

    if job.parcel != Some(event.item) {
        return;
    }

    match event.action {
        CargoAction::PickUp => {
            job.phase = JobPhase::GoToDepot;
        }
        CargoAction::Deliver => {
            job.phase = JobPhase::Done;
            job.parcel = None;
            delivery_stats.delivered_parcels += 1;
        }
        CargoAction::Drop => {}
    }
}

fn clear_failed_porter_job(
    event: &CargoActionResult,
    jobs: &mut Query<&mut AssignedJob, With<Porter>>,
) {
    let Ok(mut job) = jobs.get_mut(event.actor) else {
        return;
    };

    if job.parcel == Some(event.item) {
        job.phase = JobPhase::FindParcel;
        job.parcel = None;
    }
}

fn clamp_player_inventory_after_drop(
    event: &CargoActionResult,
    players: &Query<(), With<Player>>,
    carried_parcels: &Query<&CarriedBy, With<CargoParcel>>,
    inventory_menu: &mut InventoryMenuState,
) {
    if event.action != CargoAction::Drop || players.get(event.actor).is_err() {
        return;
    }

    let carried_count = carried_parcels
        .iter()
        .filter(|carried_by| carried_by.holder == event.actor)
        .count();
    inventory_menu.clamp_to_item_count(carried_count);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::ApplyDeferred;

    fn init_cargo_resources(world: &mut World) {
        world.insert_resource(EnergyTimeline::default());
        world.insert_resource(InventoryMenuState::default());
        world.insert_resource(DeliveryStats::default());
        world.init_resource::<Messages<PickUpRequest>>();
        world.init_resource::<Messages<DropRequest>>();
        world.init_resource::<Messages<DeliverRequest>>();
        world.init_resource::<Messages<CargoChanged>>();
        world.init_resource::<Messages<CargoActionResult>>();
    }

    fn cargo_schedule() -> Schedule {
        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                resolve_cargo_requests,
                ApplyDeferred,
                refresh_changed_cargo_caches,
                handle_cargo_action_results,
                maintain_cargo_messages,
            )
                .chain(),
        );
        schedule
    }

    fn spawn_actor(world: &mut World, max_weight: f32) -> Entity {
        world
            .spawn((
                Cargo {
                    current_weight: 0.0,
                    max_weight,
                },
                ActionEnergy::default(),
            ))
            .id()
    }

    fn spawn_loose_parcel(world: &mut World, position: Position, weight: f32) -> Entity {
        world
            .spawn((
                position,
                Item,
                CargoStats {
                    weight,
                    volume: 1.0,
                },
                CargoParcel { weight },
                ParcelState::Loose,
            ))
            .id()
    }

    #[test]
    fn pickup_succeeds_and_spends_energy_after_cache_refresh() {
        let mut world = World::new();
        init_cargo_resources(&mut world);
        let actor = spawn_actor(&mut world, 40.0);
        let parcel = spawn_loose_parcel(&mut world, Position { x: 1, y: 1 }, 5.0);
        world
            .resource_mut::<Messages<PickUpRequest>>()
            .write(PickUpRequest {
                actor,
                item: parcel,
                slot: CarrySlot::Back,
            });

        cargo_schedule().run(&mut world);

        assert_eq!(
            world.get::<CarriedBy>(parcel).map(|c| c.holder),
            Some(actor)
        );
        assert_eq!(
            world.get::<ParcelState>(parcel).copied(),
            Some(ParcelState::CarriedBy(actor))
        );
        assert_eq!(
            world
                .get::<Cargo>(actor)
                .expect("actor should keep cargo")
                .current_weight,
            5.0
        );
        assert_eq!(
            world
                .get::<ActionEnergy>(actor)
                .expect("actor should keep energy")
                .ready_at,
            u64::from(ITEM_ACTION_ENERGY_COST)
        );
    }

    #[test]
    fn oversized_pickup_fails_without_mutation_or_energy_spend() {
        let mut world = World::new();
        init_cargo_resources(&mut world);
        let actor = spawn_actor(&mut world, 4.0);
        let parcel = spawn_loose_parcel(&mut world, Position { x: 1, y: 1 }, 5.0);
        world
            .resource_mut::<Messages<PickUpRequest>>()
            .write(PickUpRequest {
                actor,
                item: parcel,
                slot: CarrySlot::Back,
            });

        cargo_schedule().run(&mut world);

        assert!(world.get::<CarriedBy>(parcel).is_none());
        assert_eq!(
            world.get::<ParcelState>(parcel).copied(),
            Some(ParcelState::Loose)
        );
        assert_eq!(
            world
                .get::<Cargo>(actor)
                .expect("actor should keep cargo")
                .current_weight,
            0.0
        );
        assert_eq!(
            world
                .get::<ActionEnergy>(actor)
                .expect("actor should keep energy")
                .ready_at,
            0
        );
    }

    #[test]
    fn drop_succeeds_and_refreshes_cache() {
        let mut world = World::new();
        init_cargo_resources(&mut world);
        let actor = spawn_actor(&mut world, 40.0);
        let parcel = spawn_loose_parcel(&mut world, Position { x: 1, y: 1 }, 5.0);
        world.entity_mut(parcel).insert((
            CarriedBy {
                holder: actor,
                slot: CarrySlot::Back,
            },
            ParcelState::CarriedBy(actor),
        ));
        world
            .get_mut::<Cargo>(actor)
            .expect("actor should keep cargo")
            .current_weight = 5.0;
        world
            .resource_mut::<Messages<DropRequest>>()
            .write(DropRequest {
                actor,
                item: parcel,
                at: Position { x: 2, y: 3 },
            });

        cargo_schedule().run(&mut world);

        assert!(world.get::<CarriedBy>(parcel).is_none());
        assert_eq!(
            world.get::<Position>(parcel).copied(),
            Some(Position { x: 2, y: 3 })
        );
        assert_eq!(
            world.get::<ParcelState>(parcel).copied(),
            Some(ParcelState::Loose)
        );
        assert_eq!(
            world
                .get::<Cargo>(actor)
                .expect("actor should keep cargo")
                .current_weight,
            0.0
        );
        assert_eq!(
            world
                .get::<ActionEnergy>(actor)
                .expect("actor should keep energy")
                .ready_at,
            u64::from(ITEM_ACTION_ENERGY_COST)
        );
    }

    #[test]
    fn delivery_succeeds_and_records_delivery() {
        let mut world = World::new();
        init_cargo_resources(&mut world);
        let actor = world
            .spawn((
                Porter { id: 0 },
                Cargo {
                    current_weight: 5.0,
                    max_weight: 40.0,
                },
                AssignedJob {
                    phase: JobPhase::GoToDepot,
                    parcel: None,
                },
                ActionEnergy::default(),
            ))
            .id();
        let parcel = spawn_loose_parcel(&mut world, Position { x: 1, y: 1 }, 5.0);
        world.entity_mut(actor).insert(AssignedJob {
            phase: JobPhase::GoToDepot,
            parcel: Some(parcel),
        });
        world.entity_mut(parcel).insert((
            CarriedBy {
                holder: actor,
                slot: CarrySlot::Back,
            },
            ParcelState::CarriedBy(actor),
        ));
        world
            .resource_mut::<Messages<DeliverRequest>>()
            .write(DeliverRequest {
                actor,
                item: parcel,
            });

        cargo_schedule().run(&mut world);

        assert!(world.get::<CarriedBy>(parcel).is_none());
        assert_eq!(
            world.get::<ParcelState>(parcel).copied(),
            Some(ParcelState::Delivered)
        );
        assert_eq!(
            world
                .get::<Cargo>(actor)
                .expect("actor should keep cargo")
                .current_weight,
            0.0
        );
        assert_eq!(
            world
                .get::<ActionEnergy>(actor)
                .expect("actor should keep energy")
                .ready_at,
            u64::from(ITEM_ACTION_ENERGY_COST)
        );
        assert_eq!(world.resource::<DeliveryStats>().delivered_parcels, 1);
    }
}
