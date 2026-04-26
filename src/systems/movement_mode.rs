use bevy_ecs::prelude::*;

use crate::components::{MovementState, Player};
use crate::energy::ActionEnergy;
use crate::resources::{EnergyTimeline, PlayerAction, PlayerIntent};

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CycleMovementRequest {
    pub actor: Entity,
}

pub fn emit_player_cycle_movement_request(
    intent: Res<PlayerIntent>,
    player: Single<Entity, With<Player>>,
    mut requests: MessageWriter<CycleMovementRequest>,
) {
    let Some(PlayerAction::CycleMovementMode) = intent.action else {
        return;
    };

    requests.write(CycleMovementRequest { actor: *player });
}

pub fn resolve_cycle_movement_requests(
    timeline: Res<EnergyTimeline>,
    mut requests: MessageReader<CycleMovementRequest>,
    mut actors: Query<(&mut MovementState, &ActionEnergy)>,
) {
    for request in requests.read() {
        let Ok((mut movement_state, energy)) = actors.get_mut(request.actor) else {
            continue;
        };
        if !energy.is_ready(timeline.now) {
            continue;
        }

        movement_state.cycle_mode();
        tracing::info!(
            actor = ?request.actor,
            mode = movement_state.mode.label(),
            "movement mode changed"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_movement_request_resolves_for_non_player_actor() {
        let mut world = World::new();
        world.insert_resource(EnergyTimeline::default());
        crate::messages::init_simulation_messages(&mut world);
        let actor = world
            .spawn((MovementState::default(), ActionEnergy::default()))
            .id();
        world
            .resource_mut::<Messages<CycleMovementRequest>>()
            .write(CycleMovementRequest { actor });

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                resolve_cycle_movement_requests,
                crate::messages::maintain_action_request_messages,
            )
                .chain(),
        );
        schedule.run(&mut world);

        let movement_state = world
            .get::<MovementState>(actor)
            .expect("actor has movement state");
        assert_eq!(
            movement_state.mode,
            crate::movement::MovementMode::Sprinting
        );
        let energy = world
            .get::<ActionEnergy>(actor)
            .expect("actor has action energy");
        assert_eq!(energy.ready_at, 0);
    }

    #[test]
    fn cycle_movement_request_skips_actor_that_is_not_ready() {
        let mut world = World::new();
        world.insert_resource(EnergyTimeline::default());
        crate::messages::init_simulation_messages(&mut world);
        let energy = ActionEnergy {
            ready_at: 1,
            ..Default::default()
        };
        let actor = world.spawn((MovementState::default(), energy)).id();
        world
            .resource_mut::<Messages<CycleMovementRequest>>()
            .write(CycleMovementRequest { actor });

        let mut schedule = Schedule::default();
        schedule.add_systems(resolve_cycle_movement_requests);
        schedule.run(&mut world);

        let movement_state = world
            .get::<MovementState>(actor)
            .expect("actor has movement state");
        assert_eq!(movement_state.mode, crate::movement::MovementMode::Walking);
    }
}
