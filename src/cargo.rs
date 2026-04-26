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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CargoTarget {
    Slot(CarrySlot),
    Container(Entity),
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CarriedBy {
    pub holder: Entity,
    pub slot: CarrySlot,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Container {
    pub volume_capacity: f32,
    pub weight_capacity: f32,
}

/// Relationship from an item to the container entity currently holding it.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
#[relationship(relationship_target = ContainerContents)]
pub struct ContainedIn {
    #[relationship]
    pub container: Entity,
}

/// Maintained by Bevy when `ContainedIn` is inserted or removed.
#[derive(Component, Clone, Debug)]
#[relationship_target(relationship = ContainedIn)]
pub struct ContainerContents(Vec<Entity>);

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
    pub location: CarriedItemLocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarriedItemLocation {
    Slot(CarrySlot),
    Container(Entity),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CargoError {
    MissingCargo,
    MissingItem,
    MissingContainer,
    NotLoose,
    NotCarriedByHolder,
    SlotOccupied,
    CapacityExceeded,
    ContainerCapacityExceeded,
}

/// Lists parcel cargo currently carried by a holder, independent of slot.
pub fn carried_parcels(world: &mut World, holder: Entity) -> Vec<CarriedParcelEntry> {
    let carried_containers = carried_containers(world, holder);
    let mut parcel_query = world.query::<(
        Entity,
        &CargoParcel,
        Option<&CarriedBy>,
        Option<&ContainedIn>,
    )>();
    let mut parcels = parcel_query
        .iter(world)
        .filter_map(|(entity, parcel, carried_by, contained_in)| {
            let carried_directly = carried_by.is_some_and(|carried_by| carried_by.holder == holder);
            let carried_in_container = contained_in
                .is_some_and(|contained_in| carried_containers.contains(&contained_in.container));
            if carried_directly || carried_in_container {
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
    let carried_containers = carried_containers(world, holder);
    let mut item_query = world.query::<(
        Entity,
        &CargoStats,
        Option<&CarriedBy>,
        Option<&ContainedIn>,
    )>();
    let mut items = item_query
        .iter(world)
        .filter_map(|(entity, stats, carried_by, contained_in)| {
            if let Some(carried_by) = carried_by.filter(|carried_by| carried_by.holder == holder) {
                return Some(CarriedItemEntry {
                    entity,
                    weight: stats.weight,
                    volume: stats.volume,
                    location: CarriedItemLocation::Slot(carried_by.slot),
                });
            }

            let contained_in = contained_in
                .filter(|contained_in| carried_containers.contains(&contained_in.container))?;
            Some(CarriedItemEntry {
                entity,
                weight: stats.weight,
                volume: stats.volume,
                location: CarriedItemLocation::Container(contained_in.container),
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

fn carried_containers(world: &mut World, holder: Entity) -> Vec<Entity> {
    let mut container_query = world.query_filtered::<(Entity, &CarriedBy), With<Container>>();
    container_query
        .iter(world)
        .filter_map(|(entity, carried_by)| (carried_by.holder == holder).then_some(entity))
        .collect()
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
    fn derived_load_includes_items_inside_carried_containers() {
        let mut world = World::new();
        let holder = spawn_holder(&mut world, 0.0, 40.0);
        let backpack = world
            .spawn((
                Item,
                CargoStats {
                    weight: 2.0,
                    volume: 3.0,
                },
                Container {
                    volume_capacity: 20.0,
                    weight_capacity: 30.0,
                },
                CarriedBy {
                    holder,
                    slot: CarrySlot::Back,
                },
            ))
            .id();
        world.spawn((
            Item,
            CargoStats {
                weight: 5.0,
                volume: 1.0,
            },
            ContainedIn {
                container: backpack,
            },
        ));

        assert_eq!(derived_load(&mut world, holder), 7.0);
        let contents = world
            .get::<ContainerContents>(backpack)
            .expect("relationship target should be maintained");
        assert_eq!(contents.len(), 1);
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
