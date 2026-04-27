use serde::{Deserialize, Serialize};

use super::{ItemDefinitionId, PersistentId};

/// Persistent state for an exact cargo item instance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedCargoItem {
    pub id: PersistentId,
    pub definition_id: ItemDefinitionId,
    pub stats: SavedCargoStats,
    pub location: SavedCargoLocation,
    pub parcel: Option<SavedParcelState>,
    pub container: Option<SavedContainerState>,
}

/// Physical cargo properties that belong to this concrete item instance.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedCargoStats {
    pub weight: f32,
    pub volume: f32,
}

/// Persisted physical location for cargo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SavedCargoLocation {
    Loose {
        x: i32,
        y: i32,
    },
    CarriedBy {
        holder: PersistentId,
        slot: SavedCarrySlot,
    },
    ContainedIn {
        container: PersistentId,
    },
}

/// Save-file carry slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SavedCarrySlot {
    Back,
    Chest,
}

/// Persisted parcel delivery state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SavedParcelState {
    Available,
    ReservedBy(PersistentId),
    Delivered,
}

/// Capacity state for a cargo item that is also a container.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedContainerState {
    pub volume_capacity: f32,
    pub weight_capacity: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_relationships_use_persistent_ids() {
        let actor = PersistentId(10);
        let cargo = SavedCargoItem {
            id: PersistentId(11),
            definition_id: ItemDefinitionId("parcel.small".to_owned()),
            stats: SavedCargoStats {
                weight: 2.0,
                volume: 1.0,
            },
            location: SavedCargoLocation::CarriedBy {
                holder: actor,
                slot: SavedCarrySlot::Back,
            },
            parcel: Some(SavedParcelState::ReservedBy(actor)),
            container: None,
        };

        assert_eq!(
            cargo.location,
            SavedCargoLocation::CarriedBy {
                holder: actor,
                slot: SavedCarrySlot::Back
            }
        );
        assert_eq!(cargo.parcel, Some(SavedParcelState::ReservedBy(actor)));
    }
}
