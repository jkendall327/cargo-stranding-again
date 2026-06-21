use bevy_ecs::prelude::*;

use crate::components::{Player, Position};
use crate::map::{Map, TileCoord};
use crate::schedules::{ensure_simulation_schedules, PlayerActionPhase};
use crate::systems::timeline;

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct SimulationRunner {}

impl SimulationRunner {
    pub fn new() -> Self {
        Self {}
    }

    pub fn run_player_intent(&mut self, world: &mut World) {
        ensure_simulation_schedules(world);
        if !timeline::has_player_intent(world) {
            return;
        }

        timeline::advance_to_player_ready(world);
        stream_chunks_around_player(world);
        world.run_schedule(PlayerActionPhase);
        stream_chunks_around_player(world);
        self.advance_after_player_action_if_spent(world);
    }

    pub fn advance_after_player_action_if_spent(&mut self, world: &mut World) {
        ensure_simulation_schedules(world);
        if timeline::player_spent_action_energy(world) {
            timeline::advance_after_player_action_spent(world);
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
