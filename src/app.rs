use std::path::PathBuf;

use bevy::prelude::*;
use bevy::window::WindowResolution;

use crate::persistence::{
    load_save_slot_into_world, player_can_save, save_slot, CharacterId, SaveEligibility,
    SaveSlotIds, WorldId,
};
use crate::render::{self, initial_window_height, initial_window_width};
use crate::resources::{
    GameScreen, PersistenceAction, PersistenceIntent, PersistenceStatus, PlayerIntent,
};
use crate::simulation::SimulationRunner;
use crate::world_setup::init_world;

const DEBUG_SAVE_IDS: SaveSlotIds = SaveSlotIds {
    world_id: WorldId(1),
    character_id: CharacterId(1),
};

/// Windowed-only schedule state for simulation phases that can loop within one frame.
///
/// The simulation entities and resources live directly in Bevy's app world.
/// This resource only keeps the reusable schedules that need to be driven from
/// an exclusive system.
#[derive(Resource)]
pub struct GameSchedules {
    simulation: SimulationRunner,
    menu_schedule: bevy_ecs::prelude::Schedule,
}

impl Default for GameSchedules {
    fn default() -> Self {
        Self::new()
    }
}

impl GameSchedules {
    pub fn new() -> Self {
        let menu_schedule = crate::schedules::menu_schedule();
        let simulation = SimulationRunner::new();

        tracing::debug!("created game schedules");

        Self {
            simulation,
            menu_schedule,
        }
    }
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(render::color_rgba(16, 18, 20, 255)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Cargo Stranding Again".to_owned(),
                resolution: WindowResolution::new(initial_window_width(), initial_window_height()),
                ..Default::default()
            }),
            ..Default::default()
        }));

    init_world(app.world_mut());

    app.insert_resource(GameSchedules::new())
        .init_resource::<render::MenuUiFingerprint>()
        .add_systems(
            Startup,
            (
                render::setup_camera,
                render::spawn_scene_view,
                render::spawn_hud,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                tick_game,
                render::sync_rendered_scene,
                render::update_hud,
                render::sync_menu_ui,
            )
                .chain(),
        )
        .run();
}

fn tick_game(world: &mut World) {
    let keyboard = world.resource::<ButtonInput<KeyCode>>().clone();
    let now_seconds = world.resource::<Time>().elapsed_secs_f64();

    // Bevy owns the outer frame loop and input devices. Each frame we copy
    // compact input intent into the same ECS world as the simulation.
    crate::input::copy_to_ecs(world, &keyboard, now_seconds);

    world.resource_scope(|world, mut schedules: Mut<GameSchedules>| {
        schedules.menu_schedule.run(world);
        handle_persistence_intent(world, &mut schedules);
        schedules
            .simulation
            .advance_after_player_action_if_spent(world);

        // The energy timeline stays input-paced for now: player input advances
        // time, and NPCs catch up to the player's next ready moment.
        if world.resource::<GameScreen>().allows_simulation()
            && world.resource::<PlayerIntent>().has_action()
        {
            schedules.simulation.run_player_intent(world);
        }
    });
}

fn debug_save_path() -> PathBuf {
    PathBuf::from("saves").join("debug-slot")
}

fn handle_persistence_intent(world: &mut World, schedules: &mut GameSchedules) {
    let action = {
        let mut intent = world.resource_mut::<PersistenceIntent>();
        intent.action.take()
    };

    match action {
        Some(PersistenceAction::SaveDebugSlot) => save_debug_slot(world),
        Some(PersistenceAction::LoadDebugSlot) => load_debug_slot(world, schedules),
        None => {}
    }
}

fn save_debug_slot(world: &mut World) {
    match player_can_save(world) {
        SaveEligibility::Eligible => {
            let path = debug_save_path();
            match save_slot(&path, world, DEBUG_SAVE_IDS) {
                Ok(()) => {
                    set_persistence_status(world, format!("Saved debug slot to {}", path.display()))
                }
                Err(error) => set_persistence_status(world, format!("Save failed: {error:?}")),
            }
        }
        reason => set_persistence_status(
            world,
            format!("Save blocked: {}", save_eligibility_label(reason)),
        ),
    }
}

fn load_debug_slot(world: &mut World, schedules: &mut GameSchedules) {
    let path = debug_save_path();
    match load_save_slot_into_world(&path, DEBUG_SAVE_IDS.character_id, world) {
        Ok(_) => {
            *world.resource_mut::<GameScreen>() = GameScreen::PauseMenu;
            schedules.simulation = SimulationRunner::new();
            schedules.menu_schedule = crate::schedules::menu_schedule();
            set_persistence_status(world, format!("Loaded debug slot from {}", path.display()));
        }
        Err(error) => set_persistence_status(world, format!("Load failed: {error:?}")),
    }
}

fn set_persistence_status(world: &mut World, message: String) {
    world.resource_mut::<PersistenceStatus>().message = Some(message);
}

fn save_eligibility_label(reason: SaveEligibility) -> String {
    match reason {
        SaveEligibility::Eligible => "eligible".to_owned(),
        SaveEligibility::MissingGameScreen => "missing game screen".to_owned(),
        SaveEligibility::NotInGameplay { screen } => format!("screen is {screen:?}"),
        SaveEligibility::NoPlayer => "no player exists".to_owned(),
        SaveEligibility::MultiplePlayers => "multiple players exist".to_owned(),
        SaveEligibility::MissingPlayerVelocity => "player has no velocity".to_owned(),
        SaveEligibility::MissingPlayerMomentum => "player has no momentum".to_owned(),
        SaveEligibility::PlayerMoving { dx, dy } => format!("player is moving ({dx}, {dy})"),
        SaveEligibility::PlayerHasMomentum { momentum } => {
            format!("player has momentum {momentum:?}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedules_rebuild_after_loading_debug_slot() {
        let mut world = World::new();
        init_world(&mut world);
        let mut schedules = GameSchedules::new();
        schedules.menu_schedule.run(&mut world);

        schedules.simulation = SimulationRunner::new();
        schedules.menu_schedule = crate::schedules::menu_schedule();

        *world.resource_mut::<GameScreen>() = GameScreen::PauseMenu;
        schedules.menu_schedule.run(&mut world);

        assert_eq!(*world.resource::<GameScreen>(), GameScreen::PauseMenu);
    }
}
