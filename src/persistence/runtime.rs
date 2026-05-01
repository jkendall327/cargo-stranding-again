use std::collections::HashMap;

use bevy_ecs::prelude::*;

use crate::cargo::{
    Cargo, CargoParcel, CargoStats, CarriedBy, CarrySlot, ContainedIn, Container, Item,
    ParcelDelivery,
};
use crate::components::{
    ActionEnergy, Actor, AssignedJob, AutonomousActor, JobPhase, Momentum, MovementState, Player,
    Porter, Position, Stamina, Velocity, WantsAction,
};
use crate::map::Map;
use crate::map::{Chunk, TileCoord};
use crate::movement::MovementMode;
use crate::resources::{DeliveryStats, EnergyTimeline, SimulationClock};

use super::{
    CharacterId, ItemDefinitionId, PersistentId, SavedActionEnergy, SavedActorState,
    SavedCargoItem, SavedCargoLocation, SavedCargoStats, SavedCarrySlot, SavedCharacterData,
    SavedContainerState, SavedEntity, SavedJobPhase, SavedMovementMode, SavedParcelState,
    SavedPlayer, SavedPorter, SavedWorldData, WorldId,
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

/// Error produced while translating autonomous actors into save data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorSaveError {
    MissingPersistentId { entity: Entity },
    JobParcelMissingPersistentId { entity: Entity },
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

/// Error produced while translating the player character into save data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterSaveError {
    NoPlayer,
    MissingPersistentId { entity: Entity },
    Cargo(CargoSaveError),
}

/// Error produced while rebuilding a saved player character.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterLoadError {
    Cargo(CargoLoadError),
}

/// Error produced while building a world-owned save payload from ECS state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldSaveError {
    MissingMap,
    Cargo(CargoSaveError),
    Actor(ActorSaveError),
}

/// Error produced while rebuilding world-owned runtime state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldLoadError {
    Chunk(super::SavedChunkError),
    Cargo(CargoLoadError),
}

/// Builds the character-owned save payload from the current player entity.
///
/// Character saves own the player's actor state and whatever cargo is currently
/// attached to the player, including items inside player-carried containers.
pub fn save_character_data(
    world: &mut World,
    character_id: CharacterId,
    world_id: WorldId,
) -> Result<SavedCharacterData, CharacterSaveError> {
    let (player_entity, actor, movement_mode) = saved_player_state(world)?;
    let carried_entities = save_cargo_owned_by_holder(world, player_entity)?;

    Ok(SavedCharacterData {
        character_id,
        world_id,
        player: SavedPlayer {
            actor,
            movement_mode,
        },
        carried_entities,
    })
}

/// Spawns a saved player character and its carried cargo into `world`.
///
/// The returned entity is the newly spawned player. Relationship-bearing cargo
/// is rebuilt in a second pass so save files never need runtime `Entity` IDs.
pub fn spawn_saved_character_data(
    world: &mut World,
    save: &SavedCharacterData,
) -> Result<Entity, CharacterLoadError> {
    let player_entity = spawn_saved_player(world, save.player.clone());
    let player_id = save.player.actor.id;
    let mut entity_by_id = HashMap::from([(player_id, player_entity)]);
    spawn_saved_cargo(world, &save.carried_entities, &mut entity_by_id)
        .map_err(CharacterLoadError::Cargo)?;
    Ok(player_entity)
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
    let bounds = map.bounds();
    let depot = map.depot_coord();
    let turn = world
        .get_resource::<SimulationClock>()
        .map_or(0, |clock| clock.turn);
    let timeline = world
        .get_resource::<EnergyTimeline>()
        .map_or(0, |timeline| timeline.now);
    let delivered_parcels = world
        .get_resource::<DeliveryStats>()
        .map_or(0, |stats| stats.delivered_parcels);
    let chunks = map.loaded_chunks().map(Into::into).collect::<Vec<_>>();
    let mut world_entities = save_porters(world)?
        .into_iter()
        .map(SavedEntity::Porter)
        .collect::<Vec<_>>();
    world_entities.extend(
        save_loose_cargo(world)?
            .into_iter()
            .map(SavedEntity::CargoItem),
    );

    Ok(SavedWorldData {
        world_id,
        seed,
        bounds: bounds.into(),
        depot_x: depot.x,
        depot_y: depot.y,
        turn,
        timeline,
        delivered_parcels,
        chunks,
        world_entities,
    })
}

/// Rebuilds world-owned runtime resources and entities from save data.
///
/// This first load slice restores the map and loose world cargo. NPC actor
/// state is still deferred, so saved cargo that references missing world actors
/// will fail loudly instead of silently dropping the relationship.
pub fn spawn_saved_world_data(
    world: &mut World,
    save: &SavedWorldData,
) -> Result<HashMap<PersistentId, Entity>, WorldLoadError> {
    spawn_saved_world_data_with_entities(world, save, &HashMap::new())
}

pub(crate) fn spawn_saved_world_data_with_entities(
    world: &mut World,
    save: &SavedWorldData,
    entity_by_id: &HashMap<PersistentId, Entity>,
) -> Result<HashMap<PersistentId, Entity>, WorldLoadError> {
    let chunks = save
        .chunks
        .iter()
        .map(Chunk::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(WorldLoadError::Chunk)?;
    let map = Map::from_loaded_chunks(
        save.seed,
        save.bounds.into(),
        TileCoord::new(save.depot_x, save.depot_y),
        chunks,
    );
    world.insert_resource(map);
    world.insert_resource(SimulationClock { turn: save.turn });
    world.insert_resource(EnergyTimeline { now: save.timeline });
    world.insert_resource(DeliveryStats {
        delivered_parcels: save.delivered_parcels,
    });

    let mut entity_by_id = entity_by_id.clone();
    for entity in &save.world_entities {
        if let SavedEntity::Porter(porter) = entity {
            let spawned = spawn_saved_porter(world, porter);
            entity_by_id.insert(porter.id, spawned);
        }
    }

    let cargo = save
        .world_entities
        .iter()
        .filter_map(|entity| match entity {
            SavedEntity::CargoItem(item) => Some(item.clone()),
            SavedEntity::Porter(_) => None,
        })
        .collect::<Vec<_>>();
    let spawned_cargo =
        spawn_saved_loose_cargo(world, &cargo, &entity_by_id).map_err(WorldLoadError::Cargo)?;
    entity_by_id.extend(spawned_cargo);
    restore_saved_porter_jobs(world, &save.world_entities, &entity_by_id)?;
    Ok(entity_by_id)
}

fn save_porters(world: &mut World) -> Result<Vec<SavedPorter>, WorldSaveError> {
    let mut query = world.query_filtered::<(
        Entity,
        Option<&PersistentId>,
        &Porter,
        &Position,
        &Cargo,
        &ActionEnergy,
        &AssignedJob,
    ), With<AutonomousActor>>();

    let mut porters = Vec::new();
    for (entity, persistent_id, porter, position, cargo, energy, job) in query.iter(world) {
        let id = persistent_id.copied().ok_or(WorldSaveError::Actor(
            ActorSaveError::MissingPersistentId { entity },
        ))?;
        let job_parcel = job
            .parcel()
            .map(|parcel| {
                world
                    .get::<PersistentId>(parcel)
                    .copied()
                    .ok_or(WorldSaveError::Actor(
                        ActorSaveError::JobParcelMissingPersistentId { entity: parcel },
                    ))
            })
            .transpose()?;
        porters.push(SavedPorter {
            id,
            porter_id: porter.id,
            x: position.x,
            y: position.y,
            cargo_max_weight: cargo.max_weight,
            action_energy: (*energy).into(),
            job_phase: job.phase().into(),
            job_parcel,
        });
    }

    porters.sort_by_key(|porter| porter.id.0);
    Ok(porters)
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
        Option<&ItemDefinitionId>,
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
        definition_id,
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
            definition_id: saved_cargo_definition_id(
                definition_id,
                parcel_marker.is_some(),
                container.is_some(),
            ),
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

fn save_cargo_owned_by_holder(
    world: &mut World,
    holder: Entity,
) -> Result<Vec<SavedCargoItem>, CharacterSaveError> {
    let holder_id = world
        .get::<PersistentId>(holder)
        .copied()
        .ok_or(CharacterSaveError::MissingPersistentId { entity: holder })?;
    let carried_containers = player_carried_container_ids(world, holder);
    let mut query = world.query_filtered::<(
        Entity,
        Option<&PersistentId>,
        &CargoStats,
        Option<&ItemDefinitionId>,
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
        stats,
        definition_id,
        parcel_marker,
        parcel_delivery,
        container,
        carried_by,
        contained_in,
    ) in query.iter(world)
    {
        let location =
            if let Some(carried_by) = carried_by.filter(|carried_by| carried_by.holder == holder) {
                SavedCargoLocation::CarriedBy {
                    holder: holder_id,
                    slot: carried_by.slot.into(),
                }
            } else if let Some(contained_in) = contained_in
                .filter(|contained_in| carried_containers.contains_key(&contained_in.container))
            {
                SavedCargoLocation::ContainedIn {
                    container: carried_containers[&contained_in.container],
                }
            } else {
                continue;
            };

        let id = persistent_id
            .copied()
            .ok_or(CharacterSaveError::MissingPersistentId { entity })?;
        cargo.push(SavedCargoItem {
            id,
            definition_id: saved_cargo_definition_id(
                definition_id,
                parcel_marker.is_some(),
                container.is_some(),
            ),
            stats: SavedCargoStats {
                weight: stats.weight,
                volume: stats.volume,
            },
            location,
            parcel: saved_parcel_state(world, parcel_delivery)
                .map_err(CharacterSaveError::Cargo)?,
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
            item.definition_id.clone(),
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

fn spawn_saved_cargo(
    world: &mut World,
    cargo: &[SavedCargoItem],
    entity_by_id: &mut HashMap<PersistentId, Entity>,
) -> Result<(), CargoLoadError> {
    for item in cargo {
        let parcel = runtime_parcel_state(item.id, item.parcel, entity_by_id)?;
        let mut entity = world.spawn((
            Item,
            item.id,
            item.definition_id.clone(),
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

        entity_by_id.insert(item.id, entity.id());
    }

    for item in cargo {
        let entity = entity_by_id[&item.id];
        match item.location {
            SavedCargoLocation::Loose { x, y } => {
                world.entity_mut(entity).insert(Position { x, y });
            }
            SavedCargoLocation::CarriedBy { holder, slot } => {
                let holder = entity_by_id.get(&holder).copied().ok_or(
                    CargoLoadError::ReservedByMissingEntity {
                        id: item.id,
                        holder,
                    },
                )?;
                world.entity_mut(entity).insert(CarriedBy {
                    holder,
                    slot: slot.into(),
                });
            }
            SavedCargoLocation::ContainedIn { container } => {
                let container_entity = entity_by_id.get(&container).copied().ok_or(
                    CargoLoadError::ReservedByMissingEntity {
                        id: item.id,
                        holder: container,
                    },
                )?;
                world.entity_mut(entity).insert(ContainedIn {
                    container: container_entity,
                });
            }
        }
    }

    Ok(())
}

fn saved_player_state(
    world: &mut World,
) -> Result<(Entity, SavedActorState, SavedMovementMode), CharacterSaveError> {
    let mut query = world.query_filtered::<(
        Entity,
        Option<&PersistentId>,
        &Position,
        &Cargo,
        &Stamina,
        &MovementState,
        &ActionEnergy,
    ), With<Player>>();
    let Some((entity, persistent_id, position, cargo, stamina, movement, energy)) =
        query.iter(world).next()
    else {
        return Err(CharacterSaveError::NoPlayer);
    };
    let id = persistent_id
        .copied()
        .ok_or(CharacterSaveError::MissingPersistentId { entity })?;
    Ok((
        entity,
        SavedActorState {
            id,
            x: position.x,
            y: position.y,
            cargo_max_weight: cargo.max_weight,
            stamina_current: stamina.current,
            stamina_max: stamina.max,
            action_energy: (*energy).into(),
        },
        movement.mode.into(),
    ))
}

fn spawn_saved_player(world: &mut World, saved: SavedPlayer) -> Entity {
    world
        .spawn((
            Actor,
            Player,
            saved.actor.id,
            Position {
                x: saved.actor.x,
                y: saved.actor.y,
            },
            Velocity::default(),
            Momentum::default(),
            Cargo {
                max_weight: saved.actor.cargo_max_weight,
            },
            Stamina {
                current: saved.actor.stamina_current,
                max: saved.actor.stamina_max,
            },
            MovementState {
                mode: saved.movement_mode.into(),
            },
            ActionEnergy::from(saved.actor.action_energy),
        ))
        .id()
}

fn spawn_saved_porter(world: &mut World, saved: &SavedPorter) -> Entity {
    world
        .spawn((
            Actor,
            AutonomousActor,
            WantsAction,
            Porter {
                id: saved.porter_id,
            },
            saved.id,
            Position {
                x: saved.x,
                y: saved.y,
            },
            Velocity::default(),
            Cargo {
                max_weight: saved.cargo_max_weight,
            },
            initial_saved_job(saved.job_phase),
            ActionEnergy::from(saved.action_energy),
        ))
        .id()
}

fn restore_saved_porter_jobs(
    world: &mut World,
    saved_entities: &[SavedEntity],
    entity_by_id: &HashMap<PersistentId, Entity>,
) -> Result<(), WorldLoadError> {
    for entity in saved_entities {
        let SavedEntity::Porter(porter) = entity else {
            continue;
        };
        let porter_entity = entity_by_id[&porter.id];
        let job_parcel = porter
            .job_parcel
            .map(|parcel| {
                entity_by_id.get(&parcel).copied().ok_or({
                    WorldLoadError::Cargo(CargoLoadError::ReservedByMissingEntity {
                        id: porter.id,
                        holder: parcel,
                    })
                })
            })
            .transpose()?;
        if let Some(mut job) = world.get_mut::<AssignedJob>(porter_entity) {
            *job = assigned_job_from_saved(porter.job_phase, job_parcel);
        }
    }
    Ok(())
}

fn initial_saved_job(phase: SavedJobPhase) -> AssignedJob {
    match phase {
        SavedJobPhase::FindParcel | SavedJobPhase::GoToParcel | SavedJobPhase::GoToDepot => {
            AssignedJob::FindParcel
        }
        SavedJobPhase::Done => AssignedJob::Done,
    }
}

fn assigned_job_from_saved(phase: SavedJobPhase, parcel: Option<Entity>) -> AssignedJob {
    match (phase, parcel) {
        (SavedJobPhase::FindParcel, _) => AssignedJob::FindParcel,
        (SavedJobPhase::GoToParcel, Some(parcel)) => AssignedJob::GoToParcel { parcel },
        (SavedJobPhase::GoToDepot, Some(parcel)) => AssignedJob::GoToDepot { parcel },
        (SavedJobPhase::Done, _) => AssignedJob::Done,
        (SavedJobPhase::GoToParcel | SavedJobPhase::GoToDepot, None) => AssignedJob::FindParcel,
    }
}

fn player_carried_container_ids(
    world: &mut World,
    holder: Entity,
) -> HashMap<Entity, PersistentId> {
    let mut query =
        world.query_filtered::<(Entity, Option<&PersistentId>, &CarriedBy), With<Container>>();
    query
        .iter(world)
        .filter_map(|(entity, persistent_id, carried_by)| {
            (carried_by.holder == holder).then_some((entity, persistent_id.copied()?))
        })
        .collect()
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

fn saved_cargo_definition_id(
    definition_id: Option<&ItemDefinitionId>,
    is_parcel: bool,
    is_container: bool,
) -> ItemDefinitionId {
    definition_id
        .cloned()
        .unwrap_or_else(|| cargo_definition_id(is_parcel, is_container))
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

impl From<ActionEnergy> for SavedActionEnergy {
    fn from(energy: ActionEnergy) -> Self {
        Self {
            ready_at: energy.ready_at,
            last_cost: energy.last_cost,
        }
    }
}

impl From<SavedActionEnergy> for ActionEnergy {
    fn from(energy: SavedActionEnergy) -> Self {
        Self {
            ready_at: energy.ready_at,
            last_cost: energy.last_cost,
        }
    }
}

impl From<MovementMode> for SavedMovementMode {
    fn from(mode: MovementMode) -> Self {
        match mode {
            MovementMode::Walking => Self::Walking,
            MovementMode::Sprinting => Self::Sprinting,
            MovementMode::Steady => Self::Steady,
        }
    }
}

impl From<SavedMovementMode> for MovementMode {
    fn from(mode: SavedMovementMode) -> Self {
        match mode {
            SavedMovementMode::Walking => Self::Walking,
            SavedMovementMode::Sprinting => Self::Sprinting,
            SavedMovementMode::Steady => Self::Steady,
        }
    }
}

impl From<JobPhase> for SavedJobPhase {
    fn from(phase: JobPhase) -> Self {
        match phase {
            JobPhase::FindParcel => Self::FindParcel,
            JobPhase::GoToParcel => Self::GoToParcel,
            JobPhase::GoToDepot => Self::GoToDepot,
            JobPhase::Done => Self::Done,
        }
    }
}

impl From<SavedJobPhase> for JobPhase {
    fn from(phase: SavedJobPhase) -> Self {
        match phase {
            SavedJobPhase::FindParcel => Self::FindParcel,
            SavedJobPhase::GoToParcel => Self::GoToParcel,
            SavedJobPhase::GoToDepot => Self::GoToDepot,
            SavedJobPhase::Done => Self::Done,
        }
    }
}

impl From<CargoSaveError> for WorldSaveError {
    fn from(error: CargoSaveError) -> Self {
        Self::Cargo(error)
    }
}

impl From<CargoSaveError> for CharacterSaveError {
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
        let saved_cargo = saved
            .world_entities
            .iter()
            .find_map(|entity| match entity {
                SavedEntity::CargoItem(cargo) => Some(cargo),
                SavedEntity::Porter(_) => None,
            })
            .expect("world save should include loose cargo");
        assert_eq!(saved_cargo.id, cargo_id);
        assert_eq!(
            saved_cargo.location,
            SavedCargoLocation::Loose { x: 8, y: 8 }
        );
        assert_eq!(saved_cargo.parcel, Some(SavedParcelState::Available));
    }

    #[test]
    fn character_save_restores_player_and_carried_cargo_relationships() {
        let mut world = World::new();
        let player_id = PersistentId(1);
        let player = world
            .spawn((
                Actor,
                Player,
                player_id,
                Position { x: 12, y: -4 },
                Velocity::default(),
                Cargo { max_weight: 55.0 },
                Stamina {
                    current: 18.0,
                    max: 30.0,
                },
                MovementState {
                    mode: MovementMode::Sprinting,
                },
                ActionEnergy {
                    ready_at: 90,
                    last_cost: 65,
                },
            ))
            .id();
        world.spawn((
            Item,
            PersistentId(2),
            CargoStats {
                weight: 4.0,
                volume: 1.0,
            },
            CarriedBy {
                holder: player,
                slot: CarrySlot::Chest,
            },
        ));
        let backpack = world
            .spawn((
                Item,
                PersistentId(3),
                CargoStats {
                    weight: 2.0,
                    volume: 3.0,
                },
                Container {
                    volume_capacity: 12.0,
                    weight_capacity: 25.0,
                },
                CarriedBy {
                    holder: player,
                    slot: CarrySlot::Back,
                },
            ))
            .id();
        world.spawn((
            Item,
            PersistentId(4),
            CargoStats {
                weight: 6.0,
                volume: 1.0,
            },
            CargoParcel,
            ParcelDelivery::ReservedBy(player),
            ContainedIn {
                container: backpack,
            },
        ));

        let saved = save_character_data(&mut world, CharacterId(5), WorldId(9))
            .expect("character should save");

        assert_eq!(saved.character_id, CharacterId(5));
        assert_eq!(saved.world_id, WorldId(9));
        assert_eq!(saved.player.actor.id, player_id);
        assert_eq!(saved.player.actor.x, 12);
        assert_eq!(saved.player.actor.y, -4);
        assert_eq!(saved.player.actor.cargo_max_weight, 55.0);
        assert_eq!(saved.player.actor.stamina_current, 18.0);
        assert_eq!(saved.player.actor.action_energy.ready_at, 90);
        assert_eq!(saved.player.movement_mode, SavedMovementMode::Sprinting);
        assert_eq!(saved.carried_entities.len(), 3);
        assert!(saved.carried_entities.iter().any(|item| {
            item.id == PersistentId(4)
                && item.location
                    == SavedCargoLocation::ContainedIn {
                        container: PersistentId(3),
                    }
                && item.parcel == Some(SavedParcelState::ReservedBy(player_id))
        }));

        let mut restored = World::new();
        let restored_player =
            spawn_saved_character_data(&mut restored, &saved).expect("saved character should load");
        assert_eq!(
            restored.get::<Position>(restored_player),
            Some(&Position { x: 12, y: -4 })
        );
        assert_eq!(
            restored
                .get::<Stamina>(restored_player)
                .map(|stamina| stamina.current),
            Some(18.0)
        );
        assert_eq!(
            restored
                .get::<Cargo>(restored_player)
                .map(|cargo| cargo.max_weight),
            Some(55.0)
        );
        assert_eq!(
            restored
                .get::<MovementState>(restored_player)
                .map(|movement| movement.mode),
            Some(MovementMode::Sprinting)
        );
        assert_eq!(
            restored
                .get::<ActionEnergy>(restored_player)
                .map(|energy| energy.ready_at),
            Some(90)
        );

        let restored_backpack = entity_with_id(&mut restored, PersistentId(3));
        let restored_parcel = entity_with_id(&mut restored, PersistentId(4));
        assert_eq!(
            restored
                .get::<CarriedBy>(restored_backpack)
                .map(|carried_by| carried_by.holder),
            Some(restored_player)
        );
        assert_eq!(
            restored
                .get::<ContainedIn>(restored_parcel)
                .map(|contained_in| contained_in.container),
            Some(restored_backpack)
        );
        assert_eq!(
            restored.get::<ParcelDelivery>(restored_parcel),
            Some(&ParcelDelivery::ReservedBy(restored_player))
        );
    }

    fn entity_with_id(world: &mut World, id: PersistentId) -> Entity {
        let mut query = world.query::<(Entity, &PersistentId)>();
        query
            .iter(world)
            .find_map(|(entity, persistent_id)| (*persistent_id == id).then_some(entity))
            .expect("entity with persistent ID should exist")
    }
}
