use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

/// Stable authoring ID for an item blueprint.
///
/// Save files and spawn tables should refer to this instead of display names,
/// because display names are presentation and are likely to change.
#[derive(Component, Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemDefinitionId(pub String);

impl ItemDefinitionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
