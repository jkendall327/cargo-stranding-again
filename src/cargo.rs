use bevy_ecs::prelude::*;

use crate::components::Player;

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Item;

#[derive(Component, Clone, Copy, Debug)]
pub struct CargoStats {
    pub weight: f32,
    pub volume: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CarrySlot {
    /// Primary back carry point for loose parcels and future backpacks.
    Back,
    /// Body-worn starter load that leaves the back slot available for parcels.
    Chest,
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
    SlotOccupied,
    CapacityExceeded,
}

/// Lists parcel cargo currently carried by a holder, independent of slot.
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

    fn spawn_loose_parcel(world: &mut World, weight: f32) -> Entity {
        world
            .spawn((
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
                slot: CarrySlot::Chest,
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
    fn carried_parcels_lists_relationships_and_refreshes_cache() {
        let mut world = World::new();
        let holder = spawn_holder(&mut world, 0.0, 40.0);
        let parcel = spawn_loose_parcel(&mut world, 5.0);
        world.entity_mut(parcel).insert((
            CarriedBy {
                holder,
                slot: CarrySlot::Back,
            },
            ParcelState::CarriedBy(holder),
        ));

        let carried = carried_parcels(&mut world, holder);
        assert_eq!(
            carried,
            vec![CarriedParcelEntry {
                entity: parcel,
                weight: 5.0
            }]
        );
        assert!(refresh_cargo_cache(&mut world, holder));
        assert_eq!(
            world.get::<CarriedBy>(parcel).map(|carried| carried.holder),
            Some(holder)
        );
        assert_eq!(
            *world
                .get::<ParcelState>(parcel)
                .expect("picked-up parcel should keep a ParcelState"),
            ParcelState::CarriedBy(holder)
        );
        assert_eq!(
            world
                .get::<Cargo>(holder)
                .expect("holder should keep a Cargo component")
                .current_weight,
            5.0
        );
    }

    #[test]
    fn refresh_cache_replaces_stale_weight_with_derived_load() {
        let mut world = World::new();
        let holder = spawn_holder(&mut world, 99.0, 40.0);
        let parcel = spawn_loose_parcel(&mut world, 5.0);
        world.entity_mut(parcel).insert(CarriedBy {
            holder,
            slot: CarrySlot::Back,
        });

        assert!(refresh_cargo_cache(&mut world, holder));
        assert_eq!(
            world
                .get::<Cargo>(holder)
                .expect("holder should keep a Cargo component")
                .current_weight,
            5.0
        );
    }
}
