use std::{collections::HashMap, path::Path};

use bevy_ecs::prelude::*;

use super::{
    read_character_file, read_world_directory, save_character_data, save_world_data,
    spawn_saved_character_data, write_character_file, write_world_directory, CharacterId,
    CharacterLoadError, CharacterSaveError, Save, SaveDirectoryError, SaveKind, WorldId,
    WorldLoadError, WorldSaveError,
};
use crate::{map::Map, world_setup};

/// Stable IDs that choose the world and character roots for one save slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaveSlotIds {
    pub world_id: WorldId,
    pub character_id: CharacterId,
}

/// Fully loaded save slot ready for simulation.
pub struct LoadedSaveSlot {
    pub ids: SaveSlotIds,
    pub world: World,
}

/// Error produced by the high-level save-slot workflow.
#[derive(Debug)]
pub enum SaveSlotError {
    WorldSave(WorldSaveError),
    CharacterSave(CharacterSaveError),
    WorldLoad(WorldLoadError),
    CharacterLoad(CharacterLoadError),
    Directory(SaveDirectoryError),
    CharacterWorldMismatch {
        world_id: WorldId,
        character_world_id: WorldId,
    },
}

/// Writes the current playable state to a single save-slot directory.
///
/// The directory contains the world manifest/chunks plus one character file,
/// matching the long-term world/character split without exposing callers to
/// the individual persistence roots.
pub fn save_slot(
    path: impl AsRef<Path>,
    world: &mut World,
    ids: SaveSlotIds,
) -> Result<(), SaveSlotError> {
    let path = path.as_ref();
    let world_save = Save::new(
        SaveKind::World,
        save_world_data(world, ids.world_id).map_err(SaveSlotError::WorldSave)?,
    );
    write_world_directory(path, &world_save).map_err(SaveSlotError::Directory)?;

    let character_save = Save::new(
        SaveKind::Character,
        save_character_data(world, ids.character_id, ids.world_id)
            .map_err(SaveSlotError::CharacterSave)?,
    );
    write_character_file(path, &character_save).map_err(SaveSlotError::Directory)
}

/// Loads a save slot into a fresh ECS world.
pub fn load_save_slot(
    path: impl AsRef<Path>,
    character_id: CharacterId,
) -> Result<LoadedSaveSlot, SaveSlotError> {
    let path = path.as_ref();
    let world_save = read_world_directory(path).map_err(SaveSlotError::Directory)?;
    let character_save =
        read_character_file(path, character_id).map_err(SaveSlotError::Directory)?;

    if character_save.payload.world_id != world_save.payload.world_id {
        return Err(SaveSlotError::CharacterWorldMismatch {
            world_id: world_save.payload.world_id,
            character_world_id: character_save.payload.world_id,
        });
    }

    let mut world = World::new();
    world_setup::init_resources(&mut world, Map::generate());
    let player = spawn_saved_character_data(&mut world, &character_save.payload)
        .map_err(SaveSlotError::CharacterLoad)?;
    let entity_by_id = HashMap::from([(character_save.payload.player.actor.id, player)]);
    super::runtime::spawn_saved_world_data_with_entities(
        &mut world,
        &world_save.payload,
        &entity_by_id,
    )
    .map_err(SaveSlotError::WorldLoad)?;
    world_setup::reserve_existing_persistent_ids(&mut world);

    Ok(LoadedSaveSlot {
        ids: SaveSlotIds {
            world_id: world_save.payload.world_id,
            character_id,
        },
        world,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        headless::{HeadlessGame, HeadlessSnapshot},
        resources::PlayerAction,
    };

    use super::*;

    #[test]
    fn save_slot_round_trips_played_state_through_disk() {
        let root = temp_save_dir("save_slot_round_trips_played_state_through_disk");
        let mut game = HeadlessGame::new();
        game.step(PlayerAction::Move(crate::resources::Direction::East));
        game.step(PlayerAction::PickUp);
        let before = game.snapshot().expect("played game should have a player");
        let ids = SaveSlotIds {
            world_id: WorldId(55),
            character_id: CharacterId(77),
        };

        save_slot(&root, game.world_mut(), ids).expect("save slot should write");
        let mut loaded = load_save_slot(&root, ids.character_id).expect("save slot should load");
        let after = HeadlessSnapshot::from_world(&mut loaded.world)
            .expect("loaded game should have a player");

        assert_eq!(loaded.ids, ids);
        assert_eq!(after.player_position, before.player_position);
        assert_eq!(after.player_stamina, before.player_stamina);
        assert_eq!(after.player_movement_mode, before.player_movement_mode);
        assert_eq!(after.timeline, before.timeline);
        assert_eq!(after.player_cargo, before.player_cargo);
        assert_eq!(after.carried_parcels, before.carried_parcels);
        assert_eq!(after.loose_parcels, before.loose_parcels);

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
