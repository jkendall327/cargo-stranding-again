use bevy_ecs::prelude::*;

use crate::components::{Player, Position};
use crate::map::{Map, TileCoord};
use crate::schedules;
use crate::systems::timeline;

pub struct SimulationRunner {
    player_action: Schedule,
    autonomous_actors: Schedule,
}

impl SimulationRunner {
    pub fn new() -> Self {
        Self {
            player_action: schedules::player_action_phase_schedule(),
            autonomous_actors: schedules::autonomous_actor_phase_schedule(),
        }
    }

    pub fn run_player_intent(&mut self, world: &mut World) {
        if !timeline::has_player_intent(world) {
            return;
        }

        timeline::advance_to_player_ready(world, &mut self.autonomous_actors);
        stream_chunks_around_player(world);
        self.player_action.run(world);
        stream_chunks_around_player(world);
        self.advance_after_player_action_if_spent(world);
    }

    pub fn advance_after_player_action_if_spent(&mut self, world: &mut World) {
        if timeline::player_spent_action_energy(world) {
            timeline::advance_after_player_action_spent(world, &mut self.autonomous_actors);
        }
    }
}

fn stream_chunks_around_player(world: &mut World) {
    let player_position = {
        let mut query = world.query_filtered::<&Position, With<Player>>();
        query.iter(world).next().copied()
    };
    let Some(player_position) = player_position else {
        return;
    };

    world
        .resource_mut::<Map>()
        .stream_chunks_near(TileCoord::from(player_position));
}

impl Default for SimulationRunner {
    fn default() -> Self {
        Self::new()
    }
}
