use bevy_ecs::prelude::*;

use crate::components::Player;
use crate::energy::ActionEnergy;
use crate::map::Map;
use crate::resources::{EnergyTimeline, PlayerIntent, SimulationClock};

pub(crate) fn has_player_intent(world: &World) -> bool {
    world.resource::<PlayerIntent>().action.is_some()
}

pub(crate) fn advance_to_player_ready(world: &mut World, agent_schedule: &mut Schedule) {
    if let Some(player_ready_at) = player_ready_at(world) {
        let now = world.resource::<EnergyTimeline>().now;
        if player_ready_at > now {
            world.resource_mut::<EnergyTimeline>().now = player_ready_at;
            catch_up_agents(world, agent_schedule);
        }
    }
}

pub(crate) fn advance_after_player_action_spent(world: &mut World, agent_schedule: &mut Schedule) {
    world.resource_mut::<SimulationClock>().turn += 1;
    if let Some(player_ready_at) = player_ready_at(world) {
        world.resource_mut::<EnergyTimeline>().now = player_ready_at;
        if world.contains_resource::<Map>() {
            catch_up_agents(world, agent_schedule);
        }
    }
}

pub(crate) fn player_spent_action_energy(world: &mut World) -> bool {
    let now = world.resource::<EnergyTimeline>().now;
    let mut query = world.query_filtered::<&ActionEnergy, With<Player>>();
    query
        .iter(world)
        .next()
        .is_some_and(|energy| energy.last_cost > 0 && energy.ready_at > now)
}

pub(crate) fn player_ready_at(world: &mut World) -> Option<u64> {
    let mut query = world.query_filtered::<&ActionEnergy, With<Player>>();
    query.iter(world).next().map(|energy| energy.ready_at)
}

fn catch_up_agents(world: &mut World, agent_schedule: &mut Schedule) {
    agent_schedule.run(world);
}
