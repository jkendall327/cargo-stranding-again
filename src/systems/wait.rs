use bevy_ecs::prelude::*;

use crate::components::{Momentum, Player, Stamina, Velocity};
use crate::energy::{ActionEnergy, WAIT_ENERGY_COST};
use crate::momentum::wait_momentum;
use crate::resources::{EnergyTimeline, PlayerAction, PlayerIntent};

const WAIT_STAMINA_RECOVERY: f32 = 3.0;

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaitRequest {
    pub actor: Entity,
}

type WaitActorItem<'a> = (
    &'a mut Velocity,
    &'a mut Stamina,
    &'a mut Momentum,
    &'a mut ActionEnergy,
);

pub fn emit_player_wait_request(
    intent: Res<PlayerIntent>,
    player: Single<Entity, With<Player>>,
    mut wait_requests: MessageWriter<WaitRequest>,
) {
    let Some(PlayerAction::Wait) = intent.action else {
        return;
    };

    wait_requests.write(WaitRequest { actor: *player });
}

pub fn resolve_wait_requests(
    timeline: Res<EnergyTimeline>,
    mut wait_requests: MessageReader<WaitRequest>,
    mut actors: Query<WaitActorItem>,
) {
    for request in wait_requests.read() {
        let Ok((mut velocity, mut stamina, mut momentum, mut energy)) =
            actors.get_mut(request.actor)
        else {
            continue;
        };
        if !energy.is_ready(timeline.now) {
            continue;
        }

        velocity.dx = 0;
        velocity.dy = 0;
        stamina.current = (stamina.current + WAIT_STAMINA_RECOVERY).min(stamina.max);
        *momentum = wait_momentum((*momentum).into()).into();
        energy.spend(timeline.now, WAIT_ENERGY_COST);
        tracing::debug!(
            actor = ?request.actor,
            ready_at = energy.ready_at,
            stamina = stamina.current,
            momentum = momentum.amount,
            "actor waited"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_request_resolves_for_non_player_actor() {
        let mut world = World::new();
        world.insert_resource(EnergyTimeline::default());
        crate::messages::init_simulation_messages(&mut world);
        let actor = world
            .spawn((
                Velocity { dx: 1, dy: 0 },
                Stamina {
                    current: 4.0,
                    max: 10.0,
                },
                Momentum {
                    direction: Some(crate::resources::Direction::East),
                    amount: 3.0,
                },
                ActionEnergy::default(),
            ))
            .id();
        world
            .resource_mut::<Messages<WaitRequest>>()
            .write(WaitRequest { actor });

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                resolve_wait_requests,
                crate::messages::maintain_action_request_messages,
            )
                .chain(),
        );
        schedule.run(&mut world);

        let velocity = world.get::<Velocity>(actor).expect("actor has velocity");
        assert_eq!(velocity.dx, 0);
        assert_eq!(velocity.dy, 0);
        let stamina = world.get::<Stamina>(actor).expect("actor has stamina");
        assert!(stamina.current > 4.0);
        let momentum = world.get::<Momentum>(actor).expect("actor has momentum");
        assert_eq!(momentum.amount, 1.0);
        let energy = world
            .get::<ActionEnergy>(actor)
            .expect("actor has action energy");
        assert!(energy.ready_at > 0);
    }
}
