use serde::{Deserialize, Serialize};

use crate::map::{CHUNK_HEIGHT, CHUNK_WIDTH};

/// Persisted chunk coordinate in world chunk space.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedChunkCoord {
    pub x: i32,
    pub y: i32,
}

/// Persisted finite map bounds in world tile space.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedMapBounds {
    pub min_x: i32,
    pub min_y: i32,
    pub width: i32,
    pub height: i32,
}

/// Full saved data for a generated or modified chunk.
///
/// Once a chunk has existed in play, this data is authoritative history. The
/// world seed is only for chunks that have not been generated yet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedChunk {
    pub coord: SavedChunkCoord,
    pub tiles: Vec<SavedTile>,
}

impl SavedChunk {
    /// Builds a saved chunk and checks the fixed-size tile invariant in debug.
    pub fn new(coord: SavedChunkCoord, tiles: Vec<SavedTile>) -> Self {
        debug_assert_eq!(
            tiles.len(),
            expected_tile_count(),
            "saved chunks should contain one tile per local chunk coordinate"
        );
        Self { coord, tiles }
    }

    pub fn has_expected_tile_count(&self) -> bool {
        self.tiles.len() == expected_tile_count()
    }
}

/// Complete persisted tile state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedTile {
    pub terrain: SavedTerrain,
    pub elevation: i16,
    pub water_depth: u8,
}

/// Terrain variants as represented in save files.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SavedTerrain {
    Grass,
    Mud,
    Rock,
    Water,
    Road,
    Depot,
}

const fn expected_tile_count() -> usize {
    (CHUNK_WIDTH * CHUNK_HEIGHT) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_tile_count_matches_runtime_chunk_shape() {
        let chunk = SavedChunk::new(
            SavedChunkCoord { x: -1, y: 2 },
            vec![
                SavedTile {
                    terrain: SavedTerrain::Grass,
                    elevation: 0,
                    water_depth: 0,
                };
                (CHUNK_WIDTH * CHUNK_HEIGHT) as usize
            ],
        );

        assert!(chunk.has_expected_tile_count());
    }
}
