use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::{
    CharacterId, Save, SaveKind, SavedCharacterData, SavedChunk, SavedChunkCoord, SavedEntity,
    SavedWorldData, WorldId,
};

const WORLD_MANIFEST_FILE: &str = "world.ron";
const CHUNK_DIRECTORY: &str = "chunks";
const CHARACTER_DIRECTORY: &str = "characters";

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

/// Writes one character save under a world save directory.
pub fn write_character_file(
    world_path: impl AsRef<Path>,
    save: &Save<SavedCharacterData>,
) -> Result<(), SaveDirectoryError> {
    ensure_kind(save.metadata.kind, SaveKind::Character)?;

    let character_dir = world_path.as_ref().join(CHARACTER_DIRECTORY);
    create_dir_all(&character_dir)?;
    write_ron(
        &character_path(&character_dir, save.payload.character_id),
        save,
    )
}

/// Reads one character save from a world save directory.
pub fn read_character_file(
    world_path: impl AsRef<Path>,
    character_id: CharacterId,
) -> Result<Save<SavedCharacterData>, SaveDirectoryError> {
    let character_dir = world_path.as_ref().join(CHARACTER_DIRECTORY);
    let save: Save<SavedCharacterData> = read_ron(&character_path(&character_dir, character_id))?;
    ensure_kind(save.metadata.kind, SaveKind::Character)?;
    Ok(save)
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

fn character_path(character_dir: &Path, id: CharacterId) -> PathBuf {
    character_dir.join(format!("{}.ron", id.0))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::persistence::{
        ItemDefinitionId, PersistentId, SavedActionEnergy, SavedActorState, SavedCargoItem,
        SavedCargoLocation, SavedCargoStats, SavedMovementMode, SavedParcelState, SavedPlayer,
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

    #[test]
    fn character_file_round_trips_under_world_directory() {
        let root = temp_save_dir("character_file_round_trips_under_world_directory");
        let save = Save::new(
            SaveKind::Character,
            SavedCharacterData {
                character_id: CharacterId(7),
                world_id: WorldId(5),
                player: SavedPlayer {
                    actor: SavedActorState {
                        id: PersistentId(1),
                        x: 9,
                        y: -2,
                        cargo_max_weight: 40.0,
                        stamina_current: 21.0,
                        stamina_max: 35.0,
                        action_energy: SavedActionEnergy {
                            ready_at: 30,
                            last_cost: 100,
                        },
                    },
                    movement_mode: SavedMovementMode::Steady,
                },
                carried_entities: vec![],
            },
        );

        write_character_file(&root, &save).expect("character file should write");
        let restored =
            read_character_file(&root, CharacterId(7)).expect("character file should read");

        assert_eq!(restored, save);
        assert!(root.join(CHARACTER_DIRECTORY).join("7.ron").is_file());

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
