use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::{Save, SaveKind, SavedChunk, SavedChunkCoord, SavedEntity, SavedWorldData, WorldId};

const WORLD_MANIFEST_FILE: &str = "world.ron";
const CHUNK_DIRECTORY: &str = "chunks";

/// Filesystem persistence errors.
#[derive(Debug)]
pub enum SaveDirectoryError {
    Io {
        path: PathBuf,
        message: String,
    },
    Serialize {
        path: PathBuf,
        message: String,
    },
    Deserialize {
        path: PathBuf,
        message: String,
    },
    UnexpectedKind {
        expected: SaveKind,
        actual: SaveKind,
    },
}

/// Lightweight world manifest kept in `world.ron`.
///
/// Full chunk history lives in one RON file per chunk so generated worlds can
/// grow incrementally without rewriting one increasingly huge save blob.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SavedWorldManifest {
    world_id: WorldId,
    seed: u64,
    chunks: Vec<SavedChunkCoord>,
    world_entities: Vec<SavedEntity>,
}

/// Writes a world save directory using RON files.
pub fn write_world_directory(
    path: impl AsRef<Path>,
    save: &Save<SavedWorldData>,
) -> Result<(), SaveDirectoryError> {
    ensure_kind(save.metadata.kind, SaveKind::World)?;

    let path = path.as_ref();
    let chunk_dir = path.join(CHUNK_DIRECTORY);
    create_dir_all(path)?;
    create_dir_all(&chunk_dir)?;

    let manifest = Save {
        metadata: save.metadata,
        payload: SavedWorldManifest {
            world_id: save.payload.world_id,
            seed: save.payload.seed,
            chunks: save
                .payload
                .chunks
                .iter()
                .map(|chunk| chunk.coord)
                .collect(),
            world_entities: save.payload.world_entities.clone(),
        },
    };
    write_ron(&path.join(WORLD_MANIFEST_FILE), &manifest)?;

    for chunk in &save.payload.chunks {
        let chunk_save = Save::new(SaveKind::Chunk, chunk.clone());
        write_ron(&chunk_path(&chunk_dir, chunk.coord), &chunk_save)?;
    }

    Ok(())
}

/// Reads a world save directory written by `write_world_directory`.
pub fn read_world_directory(
    path: impl AsRef<Path>,
) -> Result<Save<SavedWorldData>, SaveDirectoryError> {
    let path = path.as_ref();
    let manifest_path = path.join(WORLD_MANIFEST_FILE);
    let manifest: Save<SavedWorldManifest> = read_ron(&manifest_path)?;
    ensure_kind(manifest.metadata.kind, SaveKind::World)?;

    let chunk_dir = path.join(CHUNK_DIRECTORY);
    let mut chunks = Vec::new();
    for coord in &manifest.payload.chunks {
        let chunk_save: Save<SavedChunk> = read_ron(&chunk_path(&chunk_dir, *coord))?;
        ensure_kind(chunk_save.metadata.kind, SaveKind::Chunk)?;
        chunks.push(chunk_save.payload);
    }

    Ok(Save {
        metadata: manifest.metadata,
        payload: SavedWorldData {
            world_id: manifest.payload.world_id,
            seed: manifest.payload.seed,
            chunks,
            world_entities: manifest.payload.world_entities,
        },
    })
}

fn ensure_kind(actual: SaveKind, expected: SaveKind) -> Result<(), SaveDirectoryError> {
    if actual == expected {
        Ok(())
    } else {
        Err(SaveDirectoryError::UnexpectedKind { expected, actual })
    }
}

fn create_dir_all(path: &Path) -> Result<(), SaveDirectoryError> {
    fs::create_dir_all(path).map_err(|error| SaveDirectoryError::Io {
        path: path.to_owned(),
        message: error.to_string(),
    })
}

fn write_ron<T: Serialize>(path: &Path, value: &T) -> Result<(), SaveDirectoryError> {
    let text =
        ron::ser::to_string_pretty(value, ron::ser::PrettyConfig::default()).map_err(|error| {
            SaveDirectoryError::Serialize {
                path: path.to_owned(),
                message: error.to_string(),
            }
        })?;
    fs::write(path, text).map_err(|error| SaveDirectoryError::Io {
        path: path.to_owned(),
        message: error.to_string(),
    })
}

fn read_ron<T: DeserializeOwned>(path: &Path) -> Result<T, SaveDirectoryError> {
    let text = fs::read_to_string(path).map_err(|error| SaveDirectoryError::Io {
        path: path.to_owned(),
        message: error.to_string(),
    })?;
    ron::de::from_str(&text).map_err(|error| SaveDirectoryError::Deserialize {
        path: path.to_owned(),
        message: error.to_string(),
    })
}

fn chunk_path(chunk_dir: &Path, coord: SavedChunkCoord) -> PathBuf {
    chunk_dir.join(format!("{}_{}.ron", coord.x, coord.y))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::persistence::{
        ItemDefinitionId, PersistentId, SavedCargoItem, SavedCargoLocation, SavedCargoStats,
        SavedParcelState,
    };

    #[test]
    fn world_directory_round_trips_manifest_and_chunks() {
        let root = temp_save_dir("world_directory_round_trips_manifest_and_chunks");
        let chunk = SavedChunk::from(&crate::map::generate_chunk(
            123,
            crate::map::ChunkCoord::new(-1, 2),
        ));
        let save = Save::new(
            SaveKind::World,
            SavedWorldData {
                world_id: WorldId(5),
                seed: 123,
                chunks: vec![chunk],
                world_entities: vec![SavedEntity::CargoItem(SavedCargoItem {
                    id: PersistentId(9),
                    definition_id: ItemDefinitionId("parcel.generic".to_owned()),
                    stats: SavedCargoStats {
                        weight: 4.0,
                        volume: 1.0,
                    },
                    location: SavedCargoLocation::Loose { x: 3, y: 4 },
                    parcel: Some(SavedParcelState::Available),
                    container: None,
                })],
            },
        );

        write_world_directory(&root, &save).expect("world directory should write");
        let restored = read_world_directory(&root).expect("world directory should read");

        assert_eq!(restored, save);
        assert!(root.join(WORLD_MANIFEST_FILE).is_file());
        assert!(root.join(CHUNK_DIRECTORY).join("-1_2.ron").is_file());

        fs::remove_dir_all(root).expect("test save directory should clean up");
    }

    fn temp_save_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "cargo-stranding-again-{label}-{}-{unique}",
            std::process::id()
        ))
    }
}
