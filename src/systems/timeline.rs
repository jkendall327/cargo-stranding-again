use bevy_ecs::prelude::*;

use crate::components::{Agent, AssignedJob, CargoParcel, JobPhase, ParcelState, Player};
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
            if world.contains_resource::<Map>() {
                catch_up_agents_until(world, agent_schedule, player_ready_at);
            } else {
                world.resource_mut::<EnergyTimeline>().now = player_ready_at;
            }
        }
    }
}

pub(crate) fn advance_after_player_action_spent(world: &mut World, agent_schedule: &mut Schedule) {
    world.resource_mut::<SimulationClock>().turn += 1;
    if let Some(player_ready_at) = player_ready_at(world) {
        if world.contains_resource::<Map>() {
            catch_up_agents_until(world, agent_schedule, player_ready_at);
        } else {
            world.resource_mut::<EnergyTimeline>().now = player_ready_at;
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

fn catch_up_agents_until(world: &mut World, agent_schedule: &mut Schedule, until_time: u64) {
    while let Some(next_ready_at) = next_ready_agent_time(world, until_time) {
        world.resource_mut::<EnergyTimeline>().now = next_ready_at;
        let before = ready_agent_progress_marker(world);
        agent_schedule.run(world);
        let after = ready_agent_progress_marker(world);

        if after == before {
            break;
        }
    }

    world.resource_mut::<EnergyTimeline>().now = until_time;
}

fn next_ready_agent_time(world: &mut World, until_time: u64) -> Option<u64> {
    let now = world.resource::<EnergyTimeline>().now;
    let has_loose_parcel = {
        let mut parcels = world.query_filtered::<&ParcelState, With<CargoParcel>>();
        parcels
            .iter(world)
            .any(|state| matches!(state, ParcelState::Loose))
    };

    let mut agents = world.query_filtered::<(&ActionEnergy, &AssignedJob), With<Agent>>();
    agents
        .iter(world)
        .filter(|(energy, job)| {
            energy.ready_at <= until_time
                && (has_loose_parcel
                    || (job.parcel.is_some()
                        && !matches!(job.phase, JobPhase::FindParcel | JobPhase::Done)))
        })
        .map(|(energy, _)| energy.ready_at.max(now))
        .min()
}

fn ready_agent_progress_marker(world: &mut World) -> Vec<(Entity, u64)> {
    let now = world.resource::<EnergyTimeline>().now;
    let mut agents = world.query_filtered::<(Entity, &ActionEnergy), With<Agent>>();
    agents
        .iter(world)
        .filter(|(_, energy)| energy.ready_at <= now)
        .map(|(entity, energy)| (entity, energy.ready_at))
        .collect()
}
