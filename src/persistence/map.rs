use serde::{Deserialize, Serialize};

use crate::map::{
    Chunk, ChunkCoord, ChunkTileCountError, MapBounds, Terrain, TileInfo, CHUNK_HEIGHT, CHUNK_WIDTH,
};

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

/// Error returned when saved chunk data cannot rebuild a runtime chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SavedChunkError {
    WrongTileCount { expected: usize, actual: usize },
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

impl From<ChunkCoord> for SavedChunkCoord {
    fn from(coord: ChunkCoord) -> Self {
        Self {
            x: coord.x,
            y: coord.y,
        }
    }
}

impl From<SavedChunkCoord> for ChunkCoord {
    fn from(coord: SavedChunkCoord) -> Self {
        Self::new(coord.x, coord.y)
    }
}

impl From<MapBounds> for SavedMapBounds {
    fn from(bounds: MapBounds) -> Self {
        Self {
            min_x: bounds.min_x,
            min_y: bounds.min_y,
            width: bounds.width,
            height: bounds.height,
        }
    }
}

impl From<SavedMapBounds> for MapBounds {
    fn from(bounds: SavedMapBounds) -> Self {
        Self::new(bounds.min_x, bounds.min_y, bounds.width, bounds.height)
    }
}

impl From<Terrain> for SavedTerrain {
    fn from(terrain: Terrain) -> Self {
        match terrain {
            Terrain::Grass => Self::Grass,
            Terrain::Mud => Self::Mud,
            Terrain::Rock => Self::Rock,
            Terrain::Water => Self::Water,
            Terrain::Road => Self::Road,
            Terrain::Depot => Self::Depot,
        }
    }
}

impl From<SavedTerrain> for Terrain {
    fn from(terrain: SavedTerrain) -> Self {
        match terrain {
            SavedTerrain::Grass => Self::Grass,
            SavedTerrain::Mud => Self::Mud,
            SavedTerrain::Rock => Self::Rock,
            SavedTerrain::Water => Self::Water,
            SavedTerrain::Road => Self::Road,
            SavedTerrain::Depot => Self::Depot,
        }
    }
}

impl From<TileInfo> for SavedTile {
    fn from(tile: TileInfo) -> Self {
        Self {
            terrain: tile.terrain.into(),
            elevation: tile.elevation,
            water_depth: tile.water_depth,
        }
    }
}

impl From<SavedTile> for TileInfo {
    fn from(tile: SavedTile) -> Self {
        Self {
            terrain: tile.terrain.into(),
            elevation: tile.elevation,
            water_depth: tile.water_depth,
        }
    }
}

impl From<&Chunk> for SavedChunk {
    fn from(chunk: &Chunk) -> Self {
        Self::new(
            chunk.coord().into(),
            chunk.tiles().map(|(_, tile)| tile.into()).collect(),
        )
    }
}

impl TryFrom<&SavedChunk> for Chunk {
    type Error = SavedChunkError;

    fn try_from(chunk: &SavedChunk) -> Result<Self, Self::Error> {
        Chunk::from_tile_infos(
            chunk.coord.into(),
            chunk.tiles.iter().copied().map(TileInfo::from),
        )
        .map_err(SavedChunkError::from)
    }
}

impl From<ChunkTileCountError> for SavedChunkError {
    fn from(error: ChunkTileCountError) -> Self {
        Self::WrongTileCount {
            expected: error.expected,
            actual: error.actual,
        }
    }
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

    #[test]
    fn terrain_round_trips_between_runtime_and_save_schema() {
        for terrain in [
            Terrain::Grass,
            Terrain::Mud,
            Terrain::Rock,
            Terrain::Water,
            Terrain::Road,
            Terrain::Depot,
        ] {
            let saved = SavedTerrain::from(terrain);

            assert_eq!(Terrain::from(saved), terrain);
        }
    }

    #[test]
    fn generated_chunk_round_trips_exact_tile_history() {
        let chunk = crate::map::generate_chunk(99, ChunkCoord::new(-2, 3));
        let saved = SavedChunk::from(&chunk);
        let restored = Chunk::try_from(&saved).expect("valid saved chunk should restore");

        assert_eq!(restored.coord(), chunk.coord());
        assert_eq!(
            restored.tiles().collect::<Vec<_>>(),
            chunk.tiles().collect::<Vec<_>>()
        );
        assert_eq!(SavedChunk::from(&restored), saved);
    }

    #[test]
    fn authored_chunk_data_round_trips_exact_tile_history() {
        let coord = SavedChunkCoord { x: 4, y: -1 };
        let mut tiles = vec![
            SavedTile {
                terrain: SavedTerrain::Grass,
                elevation: 0,
                water_depth: 0,
            };
            expected_tile_count()
        ];
        tiles[0] = SavedTile {
            terrain: SavedTerrain::Water,
            elevation: -2,
            water_depth: 3,
        };
        tiles[expected_tile_count() - 1] = SavedTile {
            terrain: SavedTerrain::Depot,
            elevation: 7,
            water_depth: 0,
        };
        let saved = SavedChunk::new(coord, tiles);

        let restored = Chunk::try_from(&saved).expect("valid saved chunk should restore");

        assert_eq!(SavedChunk::from(&restored), saved);
    }

    #[test]
    fn saved_chunk_restore_rejects_incomplete_tile_data() {
        let saved = SavedChunk {
            coord: SavedChunkCoord { x: 0, y: 0 },
            tiles: vec![SavedTile {
                terrain: SavedTerrain::Grass,
                elevation: 0,
                water_depth: 0,
            }],
        };

        assert_eq!(
            Chunk::try_from(&saved).expect_err("incomplete chunk should not restore"),
            SavedChunkError::WrongTileCount {
                expected: expected_tile_count(),
                actual: 1,
            }
        );
    }
}
