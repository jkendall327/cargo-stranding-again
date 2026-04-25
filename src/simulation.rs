use bevy_ecs::prelude::*;

use crate::schedules;
use crate::systems::timeline;

pub struct SimulationRunner {
    player_action: Schedule,
    agents: Schedule,
}

impl SimulationRunner {
    pub fn new() -> Self {
        Self {
            player_action: schedules::player_action_phase_schedule(),
            agents: schedules::agent_phase_schedule(),
        }
    }

    pub fn run_player_intent(&mut self, world: &mut World) {
        if !timeline::has_player_intent(world) {
            return;
        }

        timeline::advance_to_player_ready(world, &mut self.agents);
        self.player_action.run(world);
        self.advance_after_player_action_if_spent(world);
    }

    pub fn advance_after_player_action_if_spent(&mut self, world: &mut World) {
        if timeline::player_spent_action_energy(world) {
            timeline::advance_after_player_action_spent(world, &mut self.agents);
        }
    }
}

impl Default for SimulationRunner {
    fn default() -> Self {
        Self::new()
    }
}
