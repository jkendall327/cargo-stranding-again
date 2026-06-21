use std::collections::HashMap;

use bevy_ecs::prelude::*;

use crate::components::Player;

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Item;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct CargoStats {
    pub weight: f32,
    pub volume: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CarrySlot {
    /// Primary back carry point for loose parcels and future backpacks.
    #[default]
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
#[relationship(relationship_target = CarriedItems)]
pub struct CarriedBy {
    #[relationship]
    pub holder: Entity,
    pub slot: CarrySlot,
}

/// Maintained by Bevy for every actor with items carrying `CarriedBy`.
///
/// Inventory-style reads should prefer this target over scanning all cargo
/// entities and filtering by holder by hand.
#[derive(Component, Clone, Debug)]
#[relationship_target(relationship = CarriedBy)]
pub struct CarriedItems(Vec<Entity>);

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
    /// Maximum derived carried weight this actor can support.
    pub max_weight: f32,
}

#[derive(Component, Clone, Copy, Debug)]
/// Marks an item as parcel-shaped delivery cargo.
///
/// Physical cargo properties live in `CargoStats`; this marker keeps delivery
/// gameplay distinct from generic carry mechanics without duplicating weight.
pub struct CargoParcel;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParcelDelivery {
    /// Parcel is not currently reserved for a delivery job.
    Available,
    /// Parcel is reserved by an actor, but its physical location is still
    /// described only by `Position`, `CarriedBy`, or `ContainedIn`.
    ReservedBy(Entity),
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
    pub is_parcel: bool,
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
    NotAvailable,
    NotAtActorPosition,
    NotCarriedByHolder,
    SlotOccupied,
    CapacityExceeded,
    ContainerCapacityExceeded,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ContainerLoad {
    pub weight: f32,
    pub volume: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirectCarryLoad {
    pub item: Entity,
    pub stats: CargoStats,
    pub carried_by: CarriedBy,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContainedLoad {
    pub stats: CargoStats,
    pub contained_in: ContainedIn,
}

/// Lists parcel cargo currently carried by a holder, independent of slot.
pub fn carried_parcels(world: &mut World, holder: Entity) -> Vec<CarriedParcelEntry> {
    carried_items(world, holder)
        .into_iter()
        .filter_map(|entry| {
            entry.is_parcel.then_some(CarriedParcelEntry {
                entity: entry.entity,
                weight: entry.weight,
            })
        })
        .collect::<Vec<_>>()
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

pub fn carried_item_count(world: &mut World, holder: Entity) -> usize {
    carried_items(world, holder).len()
}

pub fn player_carried_items(world: &mut World) -> Vec<CarriedItemEntry> {
    let Some(player_entity) = player_entity(world) else {
        return Vec::new();
    };
    carried_items(world, player_entity)
}

pub fn player_carried_item_count(world: &mut World) -> usize {
    let Some(player_entity) = player_entity(world) else {
        return 0;
    };
    carried_item_count(world, player_entity)
}

/// Lists all cargo currently held by an actor, including generic items.
///
/// Inventory and dropping should use this rather than parcel-only queries so
/// delivery state does not accidentally define what the player can hold.
pub fn carried_items(world: &mut World, holder: Entity) -> Vec<CarriedItemEntry> {
    let direct_items = carried_item_entities(world, holder);
    let carried_containers = direct_items
        .iter()
        .copied()
        .filter(|entity| world.get::<Container>(*entity).is_some())
        .collect::<Vec<_>>();

    let mut items = Vec::new();
    for entity in direct_items {
        let Some((stats, carried_by)) = world
            .get::<CargoStats>(entity)
            .zip(world.get::<CarriedBy>(entity))
        else {
            continue;
        };
        debug_assert_eq!(carried_by.holder, holder);
        items.push(CarriedItemEntry {
            entity,
            weight: stats.weight,
            volume: stats.volume,
            location: CarriedItemLocation::Slot(carried_by.slot),
            is_parcel: world.get::<CargoParcel>(entity).is_some(),
        });
    }

    for container in carried_containers {
        let contained_items = contained_item_entities(world, container);
        for entity in contained_items {
            let Some(stats) = world.get::<CargoStats>(entity) else {
                continue;
            };
            debug_assert_eq!(
                world
                    .get::<ContainedIn>(entity)
                    .map(|contained_in| contained_in.container),
                Some(container)
            );
            items.push(CarriedItemEntry {
                entity,
                weight: stats.weight,
                volume: stats.volume,
                location: CarriedItemLocation::Container(container),
                is_parcel: world.get::<CargoParcel>(entity).is_some(),
            });
        }
    }

    items.sort_by_key(|entry| entry.entity.to_bits());
    items
}

pub fn derived_load(world: &mut World, holder: Entity) -> f32 {
    let mut direct_query = world.query::<(Entity, &CargoStats, &CarriedBy)>();
    let direct_carries = direct_query
        .iter(world)
        .map(|(item, stats, carried_by)| DirectCarryLoad {
            item,
            stats: *stats,
            carried_by: *carried_by,
        })
        .collect::<Vec<_>>();
    let mut contained_query = world.query::<(&CargoStats, &ContainedIn)>();
    let contained_items = contained_query
        .iter(world)
        .map(|(stats, contained_in)| ContainedLoad {
            stats: *stats,
            contained_in: *contained_in,
        })
        .collect::<Vec<_>>();
    derived_load_from_relationships(direct_carries, contained_items, holder)
}

/// Derives an actor's current load from carry/container relationships.
///
/// This is the canonical load calculation: direct carried items count their own
/// weight plus any items contained inside them, while loose contained items only
/// affect a holder when their container is carried by that holder.
pub fn derived_load_from_relationships<D, C>(
    direct_carries: D,
    contained_items: C,
    holder: Entity,
) -> f32
where
    D: IntoIterator<Item = DirectCarryLoad>,
    C: IntoIterator<Item = ContainedLoad>,
{
    let direct_carries = direct_carries.into_iter().collect::<Vec<_>>();
    let container_loads = container_loads_from_relationships(contained_items);
    direct_carries
        .iter()
        .filter_map(|entry| {
            (entry.carried_by.holder == holder).then_some(
                entry.stats.weight
                    + container_loads
                        .get(&entry.item)
                        .map(|load| load.weight)
                        .unwrap_or_default(),
            )
        })
        .sum()
}

pub fn actor_loads_from_relationships<D, C>(
    direct_carries: D,
    contained_items: C,
) -> HashMap<Entity, f32>
where
    D: IntoIterator<Item = DirectCarryLoad>,
    C: IntoIterator<Item = ContainedLoad>,
{
    let container_loads = container_loads_from_relationships(contained_items);
    let mut actor_loads = HashMap::<Entity, f32>::new();
    for entry in direct_carries {
        let load = entry.stats.weight
            + container_loads
                .get(&entry.item)
                .map(|load| load.weight)
                .unwrap_or_default();
        *actor_loads.entry(entry.carried_by.holder).or_default() += load;
    }
    actor_loads
}

pub fn container_loads_from_relationships<C>(contained_items: C) -> HashMap<Entity, ContainerLoad>
where
    C: IntoIterator<Item = ContainedLoad>,
{
    let mut loads = HashMap::<Entity, ContainerLoad>::new();
    for entry in contained_items {
        let load = loads.entry(entry.contained_in.container).or_default();
        load.weight += entry.stats.weight;
        load.volume += entry.stats.volume;
    }
    loads
}

fn player_entity(world: &mut World) -> Option<Entity> {
    let mut player_query = world.query_filtered::<Entity, With<Player>>();
    player_query.iter(world).next()
}

fn carried_item_entities(world: &World, holder: Entity) -> Vec<Entity> {
    world
        .get::<CarriedItems>(holder)
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}

fn contained_item_entities(world: &World, container: Entity) -> Vec<Entity> {
    world
        .get::<ContainerContents>(container)
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::Position;

    fn spawn_holder(world: &mut World, max_weight: f32) -> Entity {
        world.spawn((Cargo { max_weight },)).id()
    }

    fn spawn_loose_parcel(world: &mut World, weight: f32) -> Entity {
        world
            .spawn((
                Item,
                CargoStats {
                    weight,
                    volume: 1.0,
                },
                CargoParcel,
                ParcelDelivery::Available,
            ))
            .id()
    }

    #[test]
    fn derived_load_sums_carried_item_weights() {
        let mut world = World::new();
        let holder = spawn_holder(&mut world, 40.0);
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
        let holder = spawn_holder(&mut world, 40.0);
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
    fn carried_parcels_lists_relationships() {
        let mut world = World::new();
        let holder = spawn_holder(&mut world, 40.0);
        let parcel = spawn_loose_parcel(&mut world, 5.0);
        world
            .entity_mut(parcel)
            .insert(CarriedBy {
                holder,
                slot: CarrySlot::Back,
            })
            .remove::<Position>();

        let carried = carried_parcels(&mut world, holder);
        assert_eq!(
            carried,
            vec![CarriedParcelEntry {
                entity: parcel,
                weight: 5.0
            }]
        );
        assert_eq!(
            world.get::<CarriedBy>(parcel).map(|carried| carried.holder),
            Some(holder)
        );
        assert_eq!(
            world
                .get::<CarriedItems>(holder)
                .map(|items| items.iter().collect::<Vec<_>>()),
            Some(vec![parcel])
        );
        assert_eq!(
            *world
                .get::<ParcelDelivery>(parcel)
                .expect("picked-up parcel should keep a ParcelDelivery"),
            ParcelDelivery::Available
        );
        assert_eq!(derived_load(&mut world, holder), 5.0);
    }

    #[test]
    fn carried_items_lists_generic_and_parcel_cargo() {
        let mut world = World::new();
        let holder = spawn_holder(&mut world, 40.0);
        let generic_item = world
            .spawn((
                Item,
                CargoStats {
                    weight: 3.0,
                    volume: 2.0,
                },
                CarriedBy {
                    holder,
                    slot: CarrySlot::Chest,
                },
            ))
            .id();
        let parcel = spawn_loose_parcel(&mut world, 5.0);
        world
            .entity_mut(parcel)
            .insert(CarriedBy {
                holder,
                slot: CarrySlot::Back,
            })
            .remove::<Position>();

        let mut expected = vec![
            CarriedItemEntry {
                entity: generic_item,
                weight: 3.0,
                volume: 2.0,
                location: CarriedItemLocation::Slot(CarrySlot::Chest),
                is_parcel: false,
            },
            CarriedItemEntry {
                entity: parcel,
                weight: 5.0,
                volume: 1.0,
                location: CarriedItemLocation::Slot(CarrySlot::Back),
                is_parcel: true,
            },
        ];
        expected.sort_by_key(|entry| entry.entity.to_bits());

        assert_eq!(carried_items(&mut world, holder), expected);
    }

    #[test]
    fn derived_load_from_relationships_is_pure_and_container_aware() {
        let mut world = World::new();
        let holder = spawn_holder(&mut world, 40.0);
        let backpack = Entity::from_raw_u32(100).expect("test entity id should be valid");
        let parcel = Entity::from_raw_u32(101).expect("test entity id should be valid");

        let direct_carries = [
            DirectCarryLoad {
                item: backpack,
                stats: CargoStats {
                    weight: 2.0,
                    volume: 3.0,
                },
                carried_by: CarriedBy {
                    holder,
                    slot: CarrySlot::Back,
                },
            },
            DirectCarryLoad {
                item: parcel,
                stats: CargoStats {
                    weight: 4.0,
                    volume: 1.0,
                },
                carried_by: CarriedBy {
                    holder,
                    slot: CarrySlot::Chest,
                },
            },
        ];
        let contained_items = [ContainedLoad {
            stats: CargoStats {
                weight: 5.0,
                volume: 1.0,
            },
            contained_in: ContainedIn {
                container: backpack,
            },
        }];

        assert_eq!(
            derived_load_from_relationships(direct_carries, contained_items, holder),
            11.0
        );
    }

    #[test]
    fn derived_load_ignores_stale_or_missing_capacity() {
        let mut world = World::new();
        let holder = spawn_holder(&mut world, 40.0);
        let parcel = spawn_loose_parcel(&mut world, 5.0);
        world
            .entity_mut(parcel)
            .insert(CarriedBy {
                holder,
                slot: CarrySlot::Back,
            })
            .remove::<Position>();

        assert_eq!(derived_load(&mut world, holder), 5.0);
    }
}
