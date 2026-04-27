//! Versioned save-schema types.
//!
//! Persistence uses explicit data structs instead of serializing Bevy ECS
//! internals. Runtime `Entity` values should be translated to `PersistentId`
//! values before anything reaches this module.

mod cargo;
mod envelope;
mod ids;
mod map;
mod player;
mod runtime;
mod timeline;

pub use cargo::{
    SavedCargoItem, SavedCargoLocation, SavedCargoStats, SavedCarrySlot, SavedContainerState,
    SavedParcelState,
};
pub use envelope::{Save, SaveKind, SaveMetadata, SaveVersion, CURRENT_SAVE_VERSION};
pub use ids::{CharacterId, ItemDefinitionId, PersistentId, WorldId};
pub use map::{
    SavedChunk, SavedChunkCoord, SavedChunkError, SavedMapBounds, SavedTerrain, SavedTile,
};
pub use player::{SavedActorState, SavedCharacterData, SavedMovementMode, SavedPlayer};
pub use runtime::{save_loose_cargo, spawn_saved_loose_cargo, CargoLoadError, CargoSaveError};
pub use timeline::{SavedActionEnergy, SavedTimeline};
pub use world::{SavedEntity, SavedWorldData};

mod world {
    use serde::{Deserialize, Serialize};

    use super::{SavedCargoItem, SavedChunk, WorldId};

    /// Persistent data owned by a world rather than by a specific character.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct SavedWorldData {
        pub world_id: WorldId,
        pub seed: u64,
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
    }
}
