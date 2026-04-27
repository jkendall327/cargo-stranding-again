use serde::{Deserialize, Serialize};

/// Current save schema version written by new saves.
pub const CURRENT_SAVE_VERSION: SaveVersion = SaveVersion::new(1);

/// Small metadata wrapper around a typed save payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Save<T> {
    pub metadata: SaveMetadata,
    pub payload: T,
}

impl<T> Save<T> {
    /// Wraps `payload` in the current save envelope for its root kind.
    pub const fn new(kind: SaveKind, payload: T) -> Self {
        Self {
            metadata: SaveMetadata {
                version: CURRENT_SAVE_VERSION,
                kind,
            },
            payload,
        }
    }
}

/// Metadata shared by all save roots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveMetadata {
    pub version: SaveVersion,
    pub kind: SaveKind,
}

/// Monotonic save schema version.
///
/// This is intentionally not tied to the crate version: changing gameplay code
/// is different from changing the persisted data contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SaveVersion(pub u32);

impl SaveVersion {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
}

/// Top-level persistence root carried by a save envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveKind {
    World,
    Chunk,
    Character,
}
