use bevy_ecs::prelude::*;

use crate::components::Player;
use crate::energy::ActionEnergy;
use crate::map::Map;
use crate::resources::{EnergyTimeline, PlayerIntent, SimulationClock};
use crate::systems::agents::{agent_jobs, assign_agent_jobs};
use crate::systems::player::{player_actions, reset_cargo_loss_risk, resolve_cargo_loss_risk};

pub fn advance_timeline_for_player_intent(world: &mut World) {
    if world.resource::<PlayerIntent>().action.is_none() {
        return;
    }

    if let Some(player_ready_at) = player_ready_at(world) {
        let now = world.resource::<EnergyTimeline>().now;
        if player_ready_at > now {
            world.resource_mut::<EnergyTimeline>().now = player_ready_at;
            catch_up_agents(world);
        }
    }

    if process_player_action(world) {
        advance_after_player_action_spent(world);
    }
}

pub(crate) fn advance_after_player_action_spent(world: &mut World) {
    world.resource_mut::<SimulationClock>().turn += 1;
    if let Some(player_ready_at) = player_ready_at(world) {
        world.resource_mut::<EnergyTimeline>().now = player_ready_at;
        if world.contains_resource::<Map>() {
            catch_up_agents(world);
        }
    }
}

fn player_ready_at(world: &mut World) -> Option<u64> {
    let mut query = world.query_filtered::<&ActionEnergy, With<Player>>();
    query.iter(world).next().map(|energy| energy.ready_at)
}

fn process_player_action(world: &mut World) -> bool {
    if world.resource::<PlayerIntent>().action.is_none() {
        return false;
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            reset_cargo_loss_risk,
            player_actions,
            resolve_cargo_loss_risk,
        )
            .chain(),
    );
    schedule.run(world);

    let mut query = world.query_filtered::<&ActionEnergy, With<Player>>();
    query.iter(world).next().is_some_and(|energy| {
        energy.last_cost > 0 && energy.ready_at > world.resource::<EnergyTimeline>().now
    })
}

fn catch_up_agents(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems((assign_agent_jobs, agent_jobs));
    schedule.run(world);
}
