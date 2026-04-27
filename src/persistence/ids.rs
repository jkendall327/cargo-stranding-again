use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

/// Stable identity for an object that can outlive one ECS world instance.
///
/// Save/load code should build a temporary `PersistentId -> Entity` map after
/// spawning entities, then reconnect relationships in a second pass.
#[derive(Component, Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistentId(pub u128);

/// Stable identity for a persisted world.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldId(pub u128);

/// Stable identity for a player character inside one world.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterId(pub u128);

/// Stable definition key for data-driven item defaults.
#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemDefinitionId(pub String);
