//! Versioned save-schema types.
//!
//! Persistence uses explicit data structs instead of serializing Bevy ECS
//! internals. Runtime `Entity` values should be translated to `PersistentId`
//! values before anything reaches this module.

mod cargo;
mod envelope;
mod ids;
mod map;
mod migration;
mod player;
mod runtime;
mod slot;
mod storage;
mod timeline;

pub use cargo::{
    SavedCargoItem, SavedCargoLocation, SavedCargoStats, SavedCarrySlot, SavedContainerState,
    SavedParcelState,
};
pub use envelope::{Save, SaveKind, SaveMetadata, SaveVersion, CURRENT_SAVE_VERSION};
pub use ids::{CharacterId, ItemDefinitionId, PersistentId, PersistentIdAllocator, WorldId};
pub use map::{
    SavedChunk, SavedChunkCoord, SavedChunkError, SavedMapBounds, SavedTerrain, SavedTile,
};
pub use migration::{migrate_save, SaveMigrationError};
pub use player::{SavedActorState, SavedCharacterData, SavedMovementMode, SavedPlayer};
pub use runtime::{
    save_character_data, save_loose_cargo, save_world_data, spawn_saved_character_data,
    spawn_saved_loose_cargo, spawn_saved_world_data, CargoLoadError, CargoSaveError,
    CharacterLoadError, CharacterSaveError, WorldLoadError, WorldSaveError,
};
pub use slot::{load_save_slot, save_slot, LoadedSaveSlot, SaveSlotError, SaveSlotIds};
pub use storage::{
    read_character_file, read_world_directory, write_character_file, write_world_directory,
    SaveDirectoryError,
};
pub use timeline::{SavedActionEnergy, SavedTimeline};
pub use world::{SavedEntity, SavedJobPhase, SavedPorter, SavedWorldData};

mod world {
    use serde::{Deserialize, Serialize};

    use super::{
        PersistentId, SavedActionEnergy, SavedCargoItem, SavedChunk, SavedMapBounds, WorldId,
    };

    /// Persistent data owned by a world rather than by a specific character.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct SavedWorldData {
        pub world_id: WorldId,
        pub seed: u64,
        pub bounds: SavedMapBounds,
        pub depot_x: i32,
        pub depot_y: i32,
        pub turn: u64,
        pub timeline: u64,
        pub delivered_parcels: u32,
        pub chunks: Vec<SavedChunk>,
        pub world_entities: Vec<SavedEntity>,
    }

    /// World-owned persistent entity payloads.
    ///
    /// Keeping this as an enum makes entity categories explicit as persistence
    /// grows beyond cargo, actors, and buildables.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    pub enum SavedEntity {
        CargoItem(SavedCargoItem),
        Porter(SavedPorter),
    }

    /// World-owned autonomous porter state.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct SavedPorter {
        pub id: PersistentId,
        pub porter_id: usize,
        pub x: i32,
        pub y: i32,
        pub cargo_max_weight: f32,
        pub action_energy: SavedActionEnergy,
        pub job_phase: SavedJobPhase,
        pub job_parcel: Option<PersistentId>,
    }

    /// Save-file representation of porter delivery phases.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum SavedJobPhase {
        FindParcel,
        GoToParcel,
        GoToDepot,
        Done,
    }
}
