use bevy_ecs::prelude::*;

use crate::components::{Player, Position};

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Item;

#[derive(Component, Clone, Copy, Debug)]
pub struct CargoStats {
    pub weight: f32,
    pub volume: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarrySlot {
    Back,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CarriedBy {
    pub holder: Entity,
    pub slot: CarrySlot,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Cargo {
    pub current_weight: f32,
    pub max_weight: f32,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct CargoParcel {
    pub weight: f32,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParcelState {
    Loose,
    AssignedTo(Entity),
    CarriedBy(Entity),
    Delivered,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarriedParcelEntry {
    pub entity: Entity,
    pub weight: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarriedItemEntry {
    pub entity: Entity,
    pub weight: f32,
    pub volume: f32,
    pub slot: CarrySlot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CargoError {
    MissingCargo,
    MissingItem,
    NotLoose,
    NotCarriedByHolder,
    CapacityExceeded,
}

pub fn carried_parcels(world: &mut World, holder: Entity) -> Vec<CarriedParcelEntry> {
    let mut parcel_query = world.query::<(Entity, &CargoParcel, Option<&CarriedBy>)>();
    let mut parcels = parcel_query
        .iter(world)
        .filter_map(|(entity, parcel, carried_by)| {
            if carried_by.is_some_and(|carried_by| carried_by.holder == holder) {
                Some(CarriedParcelEntry {
                    entity,
                    weight: parcel.weight,
                })
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    parcels.sort_by_key(|entry| entry.entity.to_bits());
    parcels
}

pub fn carried_parcel_count(world: &mut World, holder: Entity) -> usize {
    carried_parcels(world, holder).len()
}

pub fn player_carried_parcels(world: &mut World) -> Vec<CarriedParcelEntry> {
    let Some(player_entity) = player_entity(world) else {
        return Vec::new();
    };
    carried_parcels(world, player_entity)
}

pub fn player_carried_parcel_count(world: &mut World) -> usize {
    let Some(player_entity) = player_entity(world) else {
        return 0;
    };
    carried_parcel_count(world, player_entity)
}

pub fn cargo_load(world: &World, holder: Entity) -> Option<f32> {
    world.get::<Cargo>(holder).map(|cargo| cargo.current_weight)
}

pub fn carried_items(world: &mut World, holder: Entity) -> Vec<CarriedItemEntry> {
    let mut item_query = world.query::<(Entity, &CargoStats, &CarriedBy)>();
    let mut items = item_query
        .iter(world)
        .filter_map(|(entity, stats, carried_by)| {
            (carried_by.holder == holder).then_some(CarriedItemEntry {
                entity,
                weight: stats.weight,
                volume: stats.volume,
                slot: carried_by.slot,
            })
        })
        .collect::<Vec<_>>();
    items.sort_by_key(|entry| entry.entity.to_bits());
    items
}

pub fn derived_load(world: &mut World, holder: Entity) -> f32 {
    carried_items(world, holder)
        .iter()
        .map(|item| item.weight)
        .sum()
}

pub fn refresh_cargo_cache(world: &mut World, holder: Entity) -> bool {
    let load = derived_load(world, holder);
    let Some(mut cargo) = world.get_mut::<Cargo>(holder) else {
        return false;
    };
    cargo.current_weight = load;
    true
}

pub fn refresh_all_cargo_caches(world: &mut World) {
    let holders = {
        let mut query = world.query::<(Entity, &Cargo)>();
        query
            .iter(world)
            .map(|(entity, _)| entity)
            .collect::<Vec<_>>()
    };

    for holder in holders {
        refresh_cargo_cache(world, holder);
    }
}

pub fn can_pick_up_item(world: &mut World, holder: Entity, item: Entity) -> Result<(), CargoError> {
    let max_weight = world
        .get::<Cargo>(holder)
        .ok_or(CargoError::MissingCargo)?
        .max_weight;
    let item_weight = world
        .get::<CargoStats>(item)
        .ok_or(CargoError::MissingItem)?
        .weight;

    if !world.get::<Item>(item).is_some()
        || world.get::<CarriedBy>(item).is_some()
        || !parcel_can_be_picked_up_by(world, holder, item)
    {
        return Err(CargoError::NotLoose);
    }

    let current_load = derived_load(world, holder);
    if current_load + item_weight > max_weight {
        return Err(CargoError::CapacityExceeded);
    }

    Ok(())
}

pub fn pick_up_item(
    world: &mut World,
    holder: Entity,
    item: Entity,
    slot: CarrySlot,
) -> Result<(), CargoError> {
    can_pick_up_item(world, holder, item)?;

    world.entity_mut(item).insert(CarriedBy { holder, slot });

    if let Some(mut parcel_state) = world.get_mut::<ParcelState>(item) {
        *parcel_state = ParcelState::CarriedBy(holder);
    }

    refresh_cargo_cache(world, holder);
    Ok(())
}

pub fn drop_item(
    world: &mut World,
    holder: Entity,
    item: Entity,
    at: Position,
) -> Result<(), CargoError> {
    let carried_by = world
        .get::<CarriedBy>(item)
        .copied()
        .ok_or(CargoError::NotCarriedByHolder)?;
    if carried_by.holder != holder {
        return Err(CargoError::NotCarriedByHolder);
    }

    world.entity_mut(item).remove::<CarriedBy>();
    if let Some(mut item_position) = world.get_mut::<Position>(item) {
        *item_position = at;
    } else {
        world.entity_mut(item).insert(at);
    }
    if let Some(mut parcel_state) = world.get_mut::<ParcelState>(item) {
        *parcel_state = ParcelState::Loose;
    }

    refresh_cargo_cache(world, holder);
    Ok(())
}

pub fn drop_carried_parcel(
    world: &mut World,
    holder: Entity,
    parcel: Entity,
    at: Position,
) -> bool {
    drop_item(world, holder, parcel, at).is_ok()
}

pub fn deliver_carried_parcel(
    world: &mut World,
    holder: Entity,
    parcel: Entity,
) -> Result<(), CargoError> {
    let carried_by = world
        .get::<CarriedBy>(parcel)
        .copied()
        .ok_or(CargoError::NotCarriedByHolder)?;
    if carried_by.holder != holder {
        return Err(CargoError::NotCarriedByHolder);
    }

    world.entity_mut(parcel).remove::<CarriedBy>();
    let Some(mut parcel_state) = world.get_mut::<ParcelState>(parcel) else {
        return Err(CargoError::MissingItem);
    };
    *parcel_state = ParcelState::Delivered;
    refresh_cargo_cache(world, holder);
    Ok(())
}

pub fn drop_carried_parcels(world: &mut World, holder: Entity, at: Position) -> usize {
    let parcels = carried_parcels(world, holder);
    let mut dropped = 0;
    for parcel in parcels {
        if drop_carried_parcel(world, holder, parcel.entity, at) {
            dropped += 1;
        }
    }
    dropped
}

fn parcel_can_be_picked_up_by(world: &World, holder: Entity, item: Entity) -> bool {
    match world.get::<ParcelState>(item) {
        None => true,
        Some(ParcelState::Loose) => true,
        Some(ParcelState::AssignedTo(assigned_holder)) => *assigned_holder == holder,
        Some(ParcelState::CarriedBy(_) | ParcelState::Delivered) => false,
    }
}

fn player_entity(world: &mut World) -> Option<Entity> {
    let mut player_query = world.query_filtered::<Entity, With<Player>>();
    player_query.iter(world).next()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn_holder(world: &mut World, current_weight: f32, max_weight: f32) -> Entity {
        world
            .spawn((Cargo {
                current_weight,
                max_weight,
            },))
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
    fn derived_load_sums_carried_item_weights() {
        let mut world = World::new();
        let holder = spawn_holder(&mut world, 0.0, 40.0);
        world.spawn((
            Item,
            CargoStats {
                weight: 3.0,
                volume: 1.0,
            },
            CarriedBy {
                holder,
                slot: CarrySlot::Back,
            },
        ));
        world.spawn((
            Item,
            CargoStats {
                weight: 4.5,
                volume: 1.0,
            },
            CarriedBy {
                holder,
                slot: CarrySlot::Back,
            },
        ));

        assert_eq!(derived_load(&mut world, holder), 7.5);
    }

    #[test]
    fn pickup_succeeds_and_updates_relationships_and_cache() {
        let mut world = World::new();
        let holder = spawn_holder(&mut world, 0.0, 40.0);
        let parcel = spawn_loose_parcel(&mut world, Position { x: 1, y: 1 }, 5.0);

        pick_up_item(&mut world, holder, parcel, CarrySlot::Back).unwrap();

        assert_eq!(
            world.get::<CarriedBy>(parcel).map(|carried| carried.holder),
            Some(holder)
        );
        assert_eq!(
            *world.get::<ParcelState>(parcel).unwrap(),
            ParcelState::CarriedBy(holder)
        );
        assert_eq!(world.get::<Cargo>(holder).unwrap().current_weight, 5.0);
    }

    #[test]
    fn pickup_capacity_failure_does_not_mutate_state() {
        let mut world = World::new();
        let holder = spawn_holder(&mut world, 0.0, 4.0);
        let parcel = spawn_loose_parcel(&mut world, Position { x: 1, y: 1 }, 5.0);

        assert_eq!(
            pick_up_item(&mut world, holder, parcel, CarrySlot::Back),
            Err(CargoError::CapacityExceeded)
        );
        assert!(world.get::<CarriedBy>(parcel).is_none());
        assert_eq!(
            *world.get::<ParcelState>(parcel).unwrap(),
            ParcelState::Loose
        );
        assert_eq!(world.get::<Cargo>(holder).unwrap().current_weight, 0.0);
    }

    #[test]
    fn drop_succeeds_and_makes_item_loose_at_actor_position() {
        let mut world = World::new();
        let holder = spawn_holder(&mut world, 0.0, 40.0);
        let parcel = spawn_loose_parcel(&mut world, Position { x: 1, y: 1 }, 5.0);
        pick_up_item(&mut world, holder, parcel, CarrySlot::Back).unwrap();

        drop_item(&mut world, holder, parcel, Position { x: 2, y: 3 }).unwrap();

        assert!(world.get::<CarriedBy>(parcel).is_none());
        assert_eq!(
            *world.get::<ParcelState>(parcel).unwrap(),
            ParcelState::Loose
        );
        assert_eq!(
            *world.get::<Position>(parcel).unwrap(),
            Position { x: 2, y: 3 }
        );
        assert_eq!(world.get::<Cargo>(holder).unwrap().current_weight, 0.0);
    }

    #[test]
    fn drop_fails_when_item_is_not_carried_by_holder() {
        let mut world = World::new();
        let holder = spawn_holder(&mut world, 0.0, 40.0);
        let other_holder = spawn_holder(&mut world, 0.0, 40.0);
        let parcel = spawn_loose_parcel(&mut world, Position { x: 1, y: 1 }, 5.0);
        pick_up_item(&mut world, other_holder, parcel, CarrySlot::Back).unwrap();

        assert_eq!(
            drop_item(&mut world, holder, parcel, Position { x: 2, y: 3 }),
            Err(CargoError::NotCarriedByHolder)
        );
        assert_eq!(
            world.get::<CarriedBy>(parcel).map(|carried| carried.holder),
            Some(other_holder)
        );
    }
}
