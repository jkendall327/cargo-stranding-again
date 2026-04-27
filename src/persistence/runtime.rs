use std::collections::HashMap;

use bevy_ecs::prelude::*;

use crate::cargo::{
    CargoParcel, CargoStats, CarriedBy, CarrySlot, ContainedIn, Container, Item, ParcelDelivery,
};
use crate::components::Position;
use crate::map::Map;

use super::{
    ItemDefinitionId, PersistentId, SavedCargoItem, SavedCargoLocation, SavedCargoStats,
    SavedCarrySlot, SavedContainerState, SavedEntity, SavedParcelState, SavedWorldData, WorldId,
};

const GENERIC_ITEM_DEFINITION_ID: &str = "item.generic";
const GENERIC_CONTAINER_DEFINITION_ID: &str = "container.generic";
const GENERIC_PARCEL_DEFINITION_ID: &str = "parcel.generic";

/// Error produced while translating runtime cargo into explicit save data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CargoSaveError {
    MissingPersistentId { entity: Entity },
    ReservedByMissingPersistentId { entity: Entity },
}

/// Error produced while rebuilding runtime cargo from explicit save data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CargoLoadError {
    UnsupportedLocation {
        id: PersistentId,
    },
    ReservedByMissingEntity {
        id: PersistentId,
        holder: PersistentId,
    },
}

/// Error produced while building a world-owned save payload from ECS state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldSaveError {
    MissingMap,
    Cargo(CargoSaveError),
}

/// Builds the world-owned save payload from runtime resources and entities.
///
/// This intentionally saves every loaded chunk as authoritative history. The
/// seed remains useful for chunks that have not entered play yet, while loaded
/// chunks preserve the exact world the player has seen.
pub fn save_world_data(
    world: &mut World,
    world_id: WorldId,
) -> Result<SavedWorldData, WorldSaveError> {
    let map = world
        .get_resource::<Map>()
        .ok_or(WorldSaveError::MissingMap)?;
    let seed = map.seed;
    let chunks = map.loaded_chunks().map(Into::into).collect::<Vec<_>>();
    let world_entities = save_loose_cargo(world)?
        .into_iter()
        .map(SavedEntity::CargoItem)
        .collect();

    Ok(SavedWorldData {
        world_id,
        seed,
        chunks,
        world_entities,
    })
}

/// Converts currently loose cargo entities into world-owned save payloads.
///
/// This first runtime conversion slice intentionally handles only cargo with a
/// physical tile position and no carry/container relationship. Relationship
/// rebuilds need a broader two-pass loader and should stay out of this helper.
pub fn save_loose_cargo(world: &mut World) -> Result<Vec<SavedCargoItem>, CargoSaveError> {
    let mut query = world.query_filtered::<(
        Entity,
        Option<&PersistentId>,
        &Position,
        &CargoStats,
        Option<&CargoParcel>,
        Option<&ParcelDelivery>,
        Option<&Container>,
        Option<&CarriedBy>,
        Option<&ContainedIn>,
    ), With<Item>>();

    let mut cargo = Vec::new();
    for (
        entity,
        persistent_id,
        position,
        stats,
        parcel_marker,
        parcel_delivery,
        container,
        carried_by,
        contained_in,
    ) in query.iter(world)
    {
        if carried_by.is_some() || contained_in.is_some() {
            continue;
        }

        let id = persistent_id
            .copied()
            .ok_or(CargoSaveError::MissingPersistentId { entity })?;
        cargo.push(SavedCargoItem {
            id,
            definition_id: cargo_definition_id(parcel_marker.is_some(), container.is_some()),
            stats: SavedCargoStats {
                weight: stats.weight,
                volume: stats.volume,
            },
            location: SavedCargoLocation::Loose {
                x: position.x,
                y: position.y,
            },
            parcel: saved_parcel_state(world, parcel_delivery)?,
            container: container.map(|container| SavedContainerState {
                volume_capacity: container.volume_capacity,
                weight_capacity: container.weight_capacity,
            }),
        });
    }

    cargo.sort_by_key(|item| item.id.0);
    Ok(cargo)
}

/// Spawns saved loose cargo into `world` and returns the IDs of new entities.
///
/// `entity_by_id` is used only for parcel reservation targets in this first
/// slice. Later relationship loading can pass the same map through a complete
/// second pass for carried and contained cargo.
pub fn spawn_saved_loose_cargo(
    world: &mut World,
    cargo: &[SavedCargoItem],
    entity_by_id: &HashMap<PersistentId, Entity>,
) -> Result<HashMap<PersistentId, Entity>, CargoLoadError> {
    let mut spawned = HashMap::new();

    for item in cargo {
        let SavedCargoLocation::Loose { x, y } = item.location else {
            return Err(CargoLoadError::UnsupportedLocation { id: item.id });
        };

        let parcel = runtime_parcel_state(item.id, item.parcel, entity_by_id)?;
        let mut entity = world.spawn((
            Item,
            item.id,
            Position { x, y },
            CargoStats {
                weight: item.stats.weight,
                volume: item.stats.volume,
            },
        ));

        if item.parcel.is_some() {
            entity.insert(CargoParcel);
        }
        if let Some(parcel) = parcel {
            entity.insert(parcel);
        }
        if let Some(container) = item.container {
            entity.insert(Container {
                volume_capacity: container.volume_capacity,
                weight_capacity: container.weight_capacity,
            });
        }

        spawned.insert(item.id, entity.id());
    }

    Ok(spawned)
}

fn cargo_definition_id(is_parcel: bool, is_container: bool) -> ItemDefinitionId {
    let id = if is_parcel {
        GENERIC_PARCEL_DEFINITION_ID
    } else if is_container {
        GENERIC_CONTAINER_DEFINITION_ID
    } else {
        GENERIC_ITEM_DEFINITION_ID
    };
    ItemDefinitionId(id.to_owned())
}

fn saved_parcel_state(
    world: &World,
    parcel_delivery: Option<&ParcelDelivery>,
) -> Result<Option<SavedParcelState>, CargoSaveError> {
    match parcel_delivery.copied() {
        None => Ok(None),
        Some(ParcelDelivery::Available) => Ok(Some(SavedParcelState::Available)),
        Some(ParcelDelivery::Delivered) => Ok(Some(SavedParcelState::Delivered)),
        Some(ParcelDelivery::ReservedBy(entity)) => {
            let holder = world
                .get::<PersistentId>(entity)
                .copied()
                .ok_or(CargoSaveError::ReservedByMissingPersistentId { entity })?;
            Ok(Some(SavedParcelState::ReservedBy(holder)))
        }
    }
}

fn runtime_parcel_state(
    id: PersistentId,
    parcel: Option<SavedParcelState>,
    entity_by_id: &HashMap<PersistentId, Entity>,
) -> Result<Option<ParcelDelivery>, CargoLoadError> {
    match parcel {
        None => Ok(None),
        Some(SavedParcelState::Available) => Ok(Some(ParcelDelivery::Available)),
        Some(SavedParcelState::Delivered) => Ok(Some(ParcelDelivery::Delivered)),
        Some(SavedParcelState::ReservedBy(holder)) => {
            let holder_entity = entity_by_id
                .get(&holder)
                .copied()
                .ok_or(CargoLoadError::ReservedByMissingEntity { id, holder })?;
            Ok(Some(ParcelDelivery::ReservedBy(holder_entity)))
        }
    }
}

impl From<CarrySlot> for SavedCarrySlot {
    fn from(slot: CarrySlot) -> Self {
        match slot {
            CarrySlot::Back => Self::Back,
            CarrySlot::Chest => Self::Chest,
        }
    }
}

impl From<SavedCarrySlot> for CarrySlot {
    fn from(slot: SavedCarrySlot) -> Self {
        match slot {
            SavedCarrySlot::Back => Self::Back,
            SavedCarrySlot::Chest => Self::Chest,
        }
    }
}

impl From<CargoSaveError> for WorldSaveError {
    fn from(error: CargoSaveError) -> Self {
        Self::Cargo(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn_reserved_actor(world: &mut World, id: PersistentId) -> Entity {
        world.spawn(id).id()
    }

    #[test]
    fn loose_cargo_keeps_identity_position_stats_and_parcel_state() {
        let mut world = World::new();
        let porter_id = PersistentId(7);
        let porter = spawn_reserved_actor(&mut world, porter_id);
        let parcel_id = PersistentId(42);
        world.spawn((
            Item,
            parcel_id,
            Position { x: -3, y: 9 },
            CargoStats {
                weight: 6.5,
                volume: 2.0,
            },
            CargoParcel,
            ParcelDelivery::ReservedBy(porter),
        ));

        let saved = save_loose_cargo(&mut world).expect("loose cargo should save");

        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].id, parcel_id);
        assert_eq!(
            saved[0].definition_id,
            ItemDefinitionId(GENERIC_PARCEL_DEFINITION_ID.to_owned())
        );
        assert_eq!(saved[0].location, SavedCargoLocation::Loose { x: -3, y: 9 });
        assert_eq!(
            saved[0].stats,
            SavedCargoStats {
                weight: 6.5,
                volume: 2.0
            }
        );
        assert_eq!(
            saved[0].parcel,
            Some(SavedParcelState::ReservedBy(porter_id))
        );

        let mut restored = World::new();
        let restored_porter = spawn_reserved_actor(&mut restored, porter_id);
        let entity_by_id = HashMap::from([(porter_id, restored_porter)]);
        let spawned = spawn_saved_loose_cargo(&mut restored, &saved, &entity_by_id)
            .expect("saved loose cargo should restore");
        let restored_parcel = spawned[&parcel_id];

        assert_eq!(
            restored.get::<PersistentId>(restored_parcel),
            Some(&parcel_id)
        );
        assert_eq!(
            restored.get::<Position>(restored_parcel),
            Some(&Position { x: -3, y: 9 })
        );
        assert_eq!(
            restored.get::<CargoStats>(restored_parcel),
            Some(&CargoStats {
                weight: 6.5,
                volume: 2.0
            })
        );
        assert!(restored.get::<CargoParcel>(restored_parcel).is_some());
        assert_eq!(
            restored.get::<ParcelDelivery>(restored_parcel),
            Some(&ParcelDelivery::ReservedBy(restored_porter))
        );
    }

    #[test]
    fn carried_cargo_is_not_saved_by_loose_cargo_slice() {
        let mut world = World::new();
        let holder = spawn_reserved_actor(&mut world, PersistentId(1));
        world.spawn((
            Item,
            PersistentId(2),
            Position { x: 1, y: 1 },
            CargoStats {
                weight: 1.0,
                volume: 1.0,
            },
            CarriedBy {
                holder,
                slot: CarrySlot::Back,
            },
        ));

        let saved = save_loose_cargo(&mut world).expect("carried cargo should be ignored");

        assert!(saved.is_empty());
    }

    #[test]
    fn loose_cargo_without_persistent_id_fails_loudly() {
        let mut world = World::new();
        let entity = world
            .spawn((
                Item,
                Position { x: 1, y: 1 },
                CargoStats {
                    weight: 1.0,
                    volume: 1.0,
                },
            ))
            .id();

        assert_eq!(
            save_loose_cargo(&mut world),
            Err(CargoSaveError::MissingPersistentId { entity })
        );
    }

    #[test]
    fn world_save_data_includes_loaded_chunks_and_loose_cargo() {
        let mut world = World::new();
        world.insert_resource(Map::generate());
        let world_id = WorldId(99);
        let cargo_id = PersistentId(12);
        world.spawn((
            Item,
            cargo_id,
            Position { x: 8, y: 8 },
            CargoStats {
                weight: 3.0,
                volume: 1.0,
            },
            CargoParcel,
            ParcelDelivery::Available,
        ));

        let saved = save_world_data(&mut world, world_id).expect("world data should save");

        assert_eq!(saved.world_id, world_id);
        assert_eq!(saved.seed, Map::generate().seed);
        assert!(!saved.chunks.is_empty());
        assert!(saved.chunks.windows(2).all(|pair| {
            let a = pair[0].coord;
            let b = pair[1].coord;
            (a.x, a.y) <= (b.x, b.y)
        }));
        assert_eq!(saved.world_entities.len(), 1);
        let SavedEntity::CargoItem(saved_cargo) = &saved.world_entities[0];
        assert_eq!(saved_cargo.id, cargo_id);
        assert_eq!(
            saved_cargo.location,
            SavedCargoLocation::Loose { x: 8, y: 8 }
        );
        assert_eq!(saved_cargo.parcel, Some(SavedParcelState::Available));
    }
}
