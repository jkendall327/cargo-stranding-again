use serde::{Deserialize, Serialize};

use super::{CharacterId, PersistentId, SavedActionEnergy, SavedCargoItem, WorldId};

/// Character save root tied to exactly one world.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedCharacterData {
    pub character_id: CharacterId,
    pub world_id: WorldId,
    pub player: SavedPlayer,
    pub carried_entities: Vec<SavedCargoItem>,
}

/// Persisted player-character state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedPlayer {
    pub actor: SavedActorState,
    pub movement_mode: SavedMovementMode,
}

/// Shared persisted state for player and future NPC actors.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedActorState {
    pub id: PersistentId,
    pub x: i32,
    pub y: i32,
    pub stamina_current: f32,
    pub stamina_max: f32,
    pub action_energy: SavedActionEnergy,
}

/// Movement modes as represented in save files.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SavedMovementMode {
    Walking,
    Sprinting,
    Steady,
}
