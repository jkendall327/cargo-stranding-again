use std::path::PathBuf;

use bevy_ecs::prelude::*;
use macroquad::prelude::*;

use crate::persistence::{
    load_save_slot, player_can_save, save_slot, CharacterId, SaveEligibility, SaveSlotIds, WorldId,
};
use crate::render;
use crate::resources::{
    GameScreen, PersistenceAction, PersistenceIntent, PersistenceStatus, PlayerIntent,
};
use crate::schedules;
use crate::simulation::SimulationRunner;
use crate::world_setup::init_world;

const DEBUG_SAVE_IDS: SaveSlotIds = SaveSlotIds {
    world_id: WorldId(1),
    character_id: CharacterId(1),
};

pub struct Game {
    world: World,
    simulation: SimulationRunner,
    menu_schedule: Schedule,
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

impl Game {
    pub fn new() -> Self {
        let mut world = World::new();
        init_world(&mut world);

        let menu_schedule = schedules::menu_schedule();
        let simulation = SimulationRunner::new();

        tracing::debug!("created game schedules");

        Self {
            world,
            simulation,
            menu_schedule,
        }
    }

    pub async fn run(&mut self) {
        loop {
            self.run_frame();
            next_frame().await;
        }
    }

    fn run_frame(&mut self) {
        // Macroquad owns the outer async frame loop and immediate-mode input.
        // Each frame we copy only the compact input intent into an ECS resource.
        crate::input::copy_to_ecs(&mut self.world);
        self.menu_schedule.run(&mut self.world);
        self.handle_persistence_intent();
        self.simulation
            .advance_after_player_action_if_spent(&mut self.world);

        // Bevy ECS owns simulation state. The energy timeline stays
        // input-paced for now: player input advances time, and NPCs catch up
        // to the player's next ready moment.
        if self.world.resource::<GameScreen>().allows_simulation()
            && self.world.resource::<PlayerIntent>().has_action()
        {
            self.simulation.run_player_intent(&mut self.world);
        }

        // Rendering is deliberately a plain Macroquad function that manually
        // queries ECS state. This keeps drawing separate from deterministic sim.
        render::render(&mut self.world);
    }

    fn handle_persistence_intent(&mut self) {
        let action = {
            let mut intent = self.world.resource_mut::<PersistenceIntent>();
            intent.action.take()
        };

        match action {
            Some(PersistenceAction::SaveDebugSlot) => self.save_debug_slot(),
            Some(PersistenceAction::LoadDebugSlot) => self.load_debug_slot(),
            None => {}
        }
    }

    fn save_debug_slot(&mut self) {
        match player_can_save(&mut self.world) {
            SaveEligibility::Eligible => {
                let path = debug_save_path();
                match save_slot(&path, &mut self.world, DEBUG_SAVE_IDS) {
                    Ok(()) => set_persistence_status(
                        &mut self.world,
                        format!("Saved debug slot to {}", path.display()),
                    ),
                    Err(error) => {
                        set_persistence_status(&mut self.world, format!("Save failed: {error:?}"))
                    }
                }
            }
            reason => set_persistence_status(
                &mut self.world,
                format!("Save blocked: {}", save_eligibility_label(reason)),
            ),
        }
    }

    fn load_debug_slot(&mut self) {
        let path = debug_save_path();
        match load_save_slot(&path, DEBUG_SAVE_IDS.character_id) {
            Ok(loaded) => {
                self.install_loaded_world(loaded.world);
                *self.world.resource_mut::<GameScreen>() = GameScreen::PauseMenu;
                set_persistence_status(
                    &mut self.world,
                    format!("Loaded debug slot from {}", path.display()),
                );
            }
            Err(error) => {
                set_persistence_status(&mut self.world, format!("Load failed: {error:?}"))
            }
        }
    }

    /// Installs a fresh ECS world and rebuilds every schedule that may have
    /// cached system state for the previous Bevy `WorldId`.
    fn install_loaded_world(&mut self, world: World) {
        self.world = world;
        self.simulation = SimulationRunner::new();
        self.menu_schedule = schedules::menu_schedule();
    }
}

fn debug_save_path() -> PathBuf {
    PathBuf::from("saves").join("debug-slot")
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
    fn loaded_world_rebuilds_world_bound_menu_schedule() {
        let mut game = Game::new();
        game.menu_schedule.run(&mut game.world);

        let mut loaded_world = World::new();
        init_world(&mut loaded_world);
        game.install_loaded_world(loaded_world);

        *game.world.resource_mut::<GameScreen>() = GameScreen::PauseMenu;
        game.menu_schedule.run(&mut game.world);

        assert_eq!(*game.world.resource::<GameScreen>(), GameScreen::PauseMenu);
    }
}
