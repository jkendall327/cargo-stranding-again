use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

use crate::cargo::{
    Cargo, CargoError, CargoParcel, CargoStats, CargoTarget, CarriedBy, CarrySlot, ContainedIn,
    Container, Item, ParcelState,
};
use crate::components::{ActionEnergy, AssignedJob, JobPhase, Player, Porter, Position};
use crate::energy::ITEM_ACTION_ENERGY_COST;
use crate::resources::{DeliveryStats, EnergyTimeline, InventoryMenuState};

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
/// Request to carry a loose or actor-assigned item in a slot.
pub struct PickUpRequest {
    pub actor: Entity,
    pub item: Entity,
    pub target: CargoTarget,
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
    Option<&'a ContainedIn>,
    Option<&'a ParcelState>,
);

#[derive(SystemParam)]
pub struct PickupCargoQueries<'w, 's> {
    cargo: Query<'w, 's, &'static Cargo>,
    items: Query<'w, 's, ItemState<'static>>,
    direct_carries: Query<'w, 's, (Entity, &'static CargoStats, &'static CarriedBy)>,
    containers: Query<'w, 's, (&'static Container, &'static CarriedBy)>,
    contained_items: Query<'w, 's, (&'static CargoStats, &'static ContainedIn)>,
}

struct PickupScratch {
    occupied_slots: HashSet<(Entity, CarrySlot)>,
    actor_loads: HashMap<Entity, f32>,
    container_loads: HashMap<Entity, ContainerLoad>,
}

/// Resolves pickup requests into entity relationship changes.
///
/// Callers choose the intended target; this system owns the legality checks and
/// structural mutation so player, inventory, and autonomous actors share rules.
pub fn resolve_pickup_requests(
    mut commands: Commands,
    mut pickup_requests: MessageReader<PickUpRequest>,
    mut changed: MessageWriter<CargoChanged>,
    mut results: MessageWriter<CargoActionResult>,
    queries: PickupCargoQueries,
) {
    let mut scratch = PickupScratch {
        occupied_slots: occupied_slots_from_query(&queries.direct_carries),
        actor_loads: actor_loads_from_query(&queries.direct_carries, &queries.contained_items),
        container_loads: container_loads_from_query(&queries.contained_items),
    };
    for request in pickup_requests.read() {
        let result = validate_pickup(&queries, &scratch, request);
        if result.is_ok() {
            match request.target {
                CargoTarget::Slot(slot) => {
                    commands.entity(request.item).insert(CarriedBy {
                        holder: request.actor,
                        slot,
                    });
                    scratch.occupied_slots.insert((request.actor, slot));
                }
                CargoTarget::Container(container) => {
                    commands
                        .entity(request.item)
                        .insert(ContainedIn { container });
                }
            }
            let (_, stats, _, _, _) = queries
                .items
                .get(request.item)
                .expect("validated pickup item should remain queryable");
            let stats = stats.expect("validated pickup item should have cargo stats");
            *scratch.actor_loads.entry(request.actor).or_default() += stats.weight;
            if let CargoTarget::Container(container) = request.target {
                let load = scratch.container_loads.entry(container).or_default();
                load.weight += stats.weight;
                load.volume += stats.volume;
            }
            if has_parcel_state(&queries.items, request.item) {
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
}

/// Resolves drop requests into entity relationship changes.
pub fn resolve_drop_requests(
    mut commands: Commands,
    mut drop_requests: MessageReader<DropRequest>,
    mut changed: MessageWriter<CargoChanged>,
    mut results: MessageWriter<CargoActionResult>,
    cargo: Query<&Cargo>,
    items: Query<ItemState>,
    containers: Query<&CarriedBy, With<Container>>,
) {
    for request in drop_requests.read() {
        let result = validate_drop(&cargo, &items, &containers, request.actor, request.item);
        if result.is_ok() {
            commands
                .entity(request.item)
                .remove::<CarriedBy>()
                .remove::<ContainedIn>()
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
}

/// Resolves delivery requests into entity relationship changes.
pub fn resolve_delivery_requests(
    mut commands: Commands,
    mut deliver_requests: MessageReader<DeliverRequest>,
    mut changed: MessageWriter<CargoChanged>,
    mut results: MessageWriter<CargoActionResult>,
    cargo: Query<&Cargo>,
    items: Query<ItemState>,
    containers: Query<&CarriedBy, With<Container>>,
) {
    for request in deliver_requests.read() {
        let result = validate_delivery(&cargo, &items, &containers, request.actor, request.item);
        if result.is_ok() {
            commands
                .entity(request.item)
                .remove::<CarriedBy>()
                .remove::<ContainedIn>()
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
    direct_carries: Query<(Entity, &CargoStats, &CarriedBy)>,
    contained_items: Query<(&CargoStats, &ContainedIn)>,
    mut cargo: Query<&mut Cargo>,
) {
    for event in changed.read() {
        let load = derived_load_from_query(&direct_carries, &contained_items, event.actor);
        if let Ok(mut cargo) = cargo.get_mut(event.actor) {
            cargo.current_weight = load;
        }
    }
}

pub fn spend_energy_for_successful_cargo_actions(
    timeline: Res<EnergyTimeline>,
    mut results: MessageReader<CargoActionResult>,
    mut energy: Query<&mut ActionEnergy>,
) {
    for event in results.read() {
        if event.result.is_ok() {
            if let Ok(mut energy) = energy.get_mut(event.actor) {
                energy.spend(timeline.now, ITEM_ACTION_ENERGY_COST);
            }
        }
    }
}

pub fn update_porter_jobs_from_cargo_results(
    mut results: MessageReader<CargoActionResult>,
    mut jobs: Query<&mut AssignedJob, With<Porter>>,
    mut delivery_stats: ResMut<DeliveryStats>,
) {
    for event in results.read() {
        if event.result.is_ok() {
            handle_successful_job_result(event, &mut jobs, &mut delivery_stats);
        }
    }
}

pub fn clear_failed_porter_cargo_jobs(
    mut results: MessageReader<CargoActionResult>,
    mut jobs: Query<&mut AssignedJob, With<Porter>>,
) {
    for event in results.read() {
        if event.result.is_err() && jobs.get_mut(event.actor).is_ok() {
            clear_failed_porter_job(event, &mut jobs);
        }
    }
}

pub fn clamp_inventory_after_cargo_drop(
    mut results: MessageReader<CargoActionResult>,
    players: Query<(), With<Player>>,
    carried_parcels: Query<(Option<&CarriedBy>, Option<&ContainedIn>), With<CargoParcel>>,
    containers: Query<&CarriedBy, With<Container>>,
    mut inventory_menu: ResMut<InventoryMenuState>,
) {
    for event in results.read() {
        if event.result.is_ok() {
            clamp_player_inventory_after_drop(
                event,
                &players,
                &carried_parcels,
                &containers,
                &mut inventory_menu,
            );
        }
    }
}

pub fn log_failed_cargo_actions(mut results: MessageReader<CargoActionResult>) {
    for event in results.read() {
        if let Err(error) = event.result {
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
    queries: &PickupCargoQueries,
    scratch: &PickupScratch,
    request: &PickUpRequest,
) -> Result<(), CargoError> {
    let cargo = queries
        .cargo
        .get(request.actor)
        .map_err(|_| CargoError::MissingCargo)?;
    let (item_marker, stats, carried_by, contained_in, parcel_state) = queries
        .items
        .get(request.item)
        .map_err(|_| CargoError::MissingItem)?;
    let stats = stats.ok_or(CargoError::MissingItem)?;

    if item_marker.is_none()
        || carried_by.is_some()
        || contained_in.is_some()
        || !parcel_can_be_picked_up_by(parcel_state, request.actor)
    {
        return Err(CargoError::NotLoose);
    }

    match request.target {
        CargoTarget::Slot(slot) => {
            if scratch.occupied_slots.contains(&(request.actor, slot)) {
                return Err(CargoError::SlotOccupied);
            }
        }
        CargoTarget::Container(container_entity) => {
            let (container, carried_by) = queries
                .containers
                .get(container_entity)
                .map_err(|_| CargoError::MissingContainer)?;
            if carried_by.holder != request.actor {
                return Err(CargoError::NotCarriedByHolder);
            }
            let load = scratch
                .container_loads
                .get(&container_entity)
                .copied()
                .unwrap_or_default();
            if load.weight + stats.weight > container.weight_capacity
                || load.volume + stats.volume > container.volume_capacity
            {
                return Err(CargoError::ContainerCapacityExceeded);
            }
        }
    }

    let current_load = scratch
        .actor_loads
        .get(&request.actor)
        .copied()
        .unwrap_or_default();
    if current_load + stats.weight > cargo.max_weight {
        return Err(CargoError::CapacityExceeded);
    }

    Ok(())
}

fn validate_drop(
    cargo: &Query<&Cargo>,
    items: &Query<ItemState>,
    containers: &Query<&CarriedBy, With<Container>>,
    actor: Entity,
    item: Entity,
) -> Result<(), CargoError> {
    cargo.get(actor).map_err(|_| CargoError::MissingCargo)?;
    let (item_marker, stats, carried_by, contained_in, _) =
        items.get(item).map_err(|_| CargoError::MissingItem)?;
    if item_marker.is_none() || stats.is_none() {
        return Err(CargoError::MissingItem);
    }
    if carried_by.is_some_and(|carried_by| carried_by.holder == actor) {
        return Ok(());
    }
    if let Some(contained_in) = contained_in {
        if containers
            .get(contained_in.container)
            .is_ok_and(|carried_by| carried_by.holder == actor)
        {
            return Ok(());
        }
    }
    Err(CargoError::NotCarriedByHolder)
}

fn validate_delivery(
    cargo: &Query<&Cargo>,
    items: &Query<ItemState>,
    containers: &Query<&CarriedBy, With<Container>>,
    actor: Entity,
    item: Entity,
) -> Result<(), CargoError> {
    cargo.get(actor).map_err(|_| CargoError::MissingCargo)?;
    let (item_marker, stats, carried_by, contained_in, parcel_state) =
        items.get(item).map_err(|_| CargoError::MissingItem)?;
    if item_marker.is_none() || stats.is_none() || parcel_state.is_none() {
        return Err(CargoError::MissingItem);
    }
    if carried_by.is_some_and(|carried_by| carried_by.holder == actor) {
        return Ok(());
    }
    if let Some(contained_in) = contained_in {
        if containers
            .get(contained_in.container)
            .is_ok_and(|carried_by| carried_by.holder == actor)
        {
            return Ok(());
        }
    }
    Err(CargoError::NotCarriedByHolder)
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
        .is_ok_and(|(_, _, _, _, parcel_state)| parcel_state.is_some())
}

fn derived_load_from_query(
    direct_carries: &Query<(Entity, &CargoStats, &CarriedBy)>,
    contained_items: &Query<(&CargoStats, &ContainedIn)>,
    actor: Entity,
) -> f32 {
    let container_loads = container_loads_from_query(contained_items);
    direct_carries
        .iter()
        .filter_map(|(entity, stats, carried_by)| {
            (carried_by.holder == actor).then_some(
                stats.weight
                    + container_loads
                        .get(&entity)
                        .map(|load| load.weight)
                        .unwrap_or_default(),
            )
        })
        .sum()
}

fn occupied_slots_from_query(
    direct_carries: &Query<(Entity, &CargoStats, &CarriedBy)>,
) -> HashSet<(Entity, CarrySlot)> {
    direct_carries
        .iter()
        .map(|(_, _, carried_by)| (carried_by.holder, carried_by.slot))
        .collect()
}

#[derive(Clone, Copy, Debug, Default)]
struct ContainerLoad {
    weight: f32,
    volume: f32,
}

fn actor_loads_from_query(
    direct_carries: &Query<(Entity, &CargoStats, &CarriedBy)>,
    contained_items: &Query<(&CargoStats, &ContainedIn)>,
) -> HashMap<Entity, f32> {
    let container_loads = container_loads_from_query(contained_items);
    let mut actor_loads = HashMap::<Entity, f32>::new();
    for (entity, stats, carried_by) in direct_carries.iter() {
        let load = stats.weight
            + container_loads
                .get(&entity)
                .map(|load| load.weight)
                .unwrap_or_default();
        *actor_loads.entry(carried_by.holder).or_default() += load;
    }
    actor_loads
}

fn container_loads_from_query(
    contained_items: &Query<(&CargoStats, &ContainedIn)>,
) -> HashMap<Entity, ContainerLoad> {
    let mut loads = HashMap::<Entity, ContainerLoad>::new();
    for (stats, contained_in) in contained_items.iter() {
        let load = loads.entry(contained_in.container).or_default();
        load.weight += stats.weight;
        load.volume += stats.volume;
    }
    loads
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
    carried_parcels: &Query<(Option<&CarriedBy>, Option<&ContainedIn>), With<CargoParcel>>,
    containers: &Query<&CarriedBy, With<Container>>,
    inventory_menu: &mut InventoryMenuState,
) {
    if event.action != CargoAction::Drop || players.get(event.actor).is_err() {
        return;
    }

    let carried_count = carried_parcels
        .iter()
        .filter(|(carried_by, contained_in)| {
            parcel_carried_by_actor(*carried_by, *contained_in, containers, event.actor)
        })
        .count();
    inventory_menu.clamp_to_item_count(carried_count);
}

fn parcel_carried_by_actor(
    carried_by: Option<&CarriedBy>,
    contained_in: Option<&ContainedIn>,
    containers: &Query<&CarriedBy, With<Container>>,
    actor: Entity,
) -> bool {
    carried_by.is_some_and(|carried_by| carried_by.holder == actor)
        || contained_in.is_some_and(|contained_in| {
            containers
                .get(contained_in.container)
                .is_ok_and(|carried_by| carried_by.holder == actor)
        })
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
                resolve_pickup_requests,
                resolve_drop_requests,
                resolve_delivery_requests,
                ApplyDeferred,
                refresh_changed_cargo_caches,
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

    fn spawn_carried_container(
        world: &mut World,
        actor: Entity,
        weight_capacity: f32,
        volume_capacity: f32,
    ) -> Entity {
        world
            .spawn((
                Item,
                CargoStats {
                    weight: 2.0,
                    volume: 3.0,
                },
                Container {
                    weight_capacity,
                    volume_capacity,
                },
                CarriedBy {
                    holder: actor,
                    slot: CarrySlot::Back,
                },
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
                target: CargoTarget::Slot(CarrySlot::Back),
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
                target: CargoTarget::Slot(CarrySlot::Back),
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
    fn occupied_slot_pickup_fails_without_mutation_or_energy_spend() {
        let mut world = World::new();
        init_cargo_resources(&mut world);
        let actor = spawn_actor(&mut world, 40.0);
        let carried = spawn_loose_parcel(&mut world, Position { x: 1, y: 1 }, 5.0);
        world.entity_mut(carried).insert((
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
        let waiting = spawn_loose_parcel(&mut world, Position { x: 2, y: 1 }, 5.0);
        world
            .resource_mut::<Messages<PickUpRequest>>()
            .write(PickUpRequest {
                actor,
                item: waiting,
                target: CargoTarget::Slot(CarrySlot::Back),
            });

        cargo_schedule().run(&mut world);

        assert!(world.get::<CarriedBy>(waiting).is_none());
        assert_eq!(
            world.get::<ParcelState>(waiting).copied(),
            Some(ParcelState::Loose)
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
            0
        );
    }

    #[test]
    fn same_schedule_pickups_cannot_share_a_slot() {
        let mut world = World::new();
        init_cargo_resources(&mut world);
        let actor = spawn_actor(&mut world, 40.0);
        let first = spawn_loose_parcel(&mut world, Position { x: 1, y: 1 }, 5.0);
        let second = spawn_loose_parcel(&mut world, Position { x: 2, y: 1 }, 5.0);
        {
            let mut pickup_requests = world.resource_mut::<Messages<PickUpRequest>>();
            pickup_requests.write(PickUpRequest {
                actor,
                item: first,
                target: CargoTarget::Slot(CarrySlot::Back),
            });
            pickup_requests.write(PickUpRequest {
                actor,
                item: second,
                target: CargoTarget::Slot(CarrySlot::Back),
            });
        }

        cargo_schedule().run(&mut world);

        let carried_count = [first, second]
            .into_iter()
            .filter(|item| world.get::<CarriedBy>(*item).is_some())
            .count();
        assert_eq!(carried_count, 1);
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
    fn pickup_into_carried_container_succeeds_and_uses_derived_load() {
        let mut world = World::new();
        init_cargo_resources(&mut world);
        let actor = spawn_actor(&mut world, 40.0);
        let container = spawn_carried_container(&mut world, actor, 20.0, 8.0);
        let parcel = spawn_loose_parcel(&mut world, Position { x: 1, y: 1 }, 5.0);
        world
            .resource_mut::<Messages<PickUpRequest>>()
            .write(PickUpRequest {
                actor,
                item: parcel,
                target: CargoTarget::Container(container),
            });

        cargo_schedule().run(&mut world);

        assert!(world.get::<CarriedBy>(parcel).is_none());
        assert_eq!(
            world
                .get::<ContainedIn>(parcel)
                .map(|contained_in| contained_in.container),
            Some(container)
        );
        assert_eq!(
            world
                .get::<Cargo>(actor)
                .expect("actor should keep cargo")
                .current_weight,
            7.0
        );
    }

    #[test]
    fn pickup_into_full_container_fails_without_mutation_or_energy_spend() {
        let mut world = World::new();
        init_cargo_resources(&mut world);
        let actor = spawn_actor(&mut world, 40.0);
        let container = spawn_carried_container(&mut world, actor, 4.0, 8.0);
        let parcel = spawn_loose_parcel(&mut world, Position { x: 1, y: 1 }, 5.0);
        world
            .resource_mut::<Messages<PickUpRequest>>()
            .write(PickUpRequest {
                actor,
                item: parcel,
                target: CargoTarget::Container(container),
            });

        cargo_schedule().run(&mut world);

        assert!(world.get::<ContainedIn>(parcel).is_none());
        assert_eq!(
            world.get::<ParcelState>(parcel).copied(),
            Some(ParcelState::Loose)
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
