use bevy_ecs::prelude::*;

use crate::components::{Momentum, MovementState, Position, Stamina, Velocity};
use crate::energy::ActionEnergy;
use crate::map::Map;
use crate::momentum::movement_effect;
use crate::movement::{
    resolve_movement, CargoLoad, MovementOutcome, MovementRequest, StaminaBudget,
};
use crate::resources::{CargoLossRisk, Direction, EnergyTimeline};

pub(super) struct PlayerMovement<'a> {
    pub entity: Entity,
    pub position: &'a mut Position,
    pub velocity: &'a mut Velocity,
    pub stamina: &'a mut Stamina,
    pub current_load: f32,
    pub max_load: f32,
    pub movement_state: &'a MovementState,
    pub momentum: &'a mut Momentum,
    pub energy: &'a mut ActionEnergy,
}

pub(super) fn try_move_player(
    actor: PlayerMovement<'_>,
    direction: Direction,
    map: &Map,
    timeline: &EnergyTimeline,
    cargo_loss_risk: &mut CargoLossRisk,
) {
    let mut request = MovementRequest::new(*actor.position, direction, actor.movement_state.mode);
    request.entity = Some(actor.entity);
    request.stamina = Some(StaminaBudget {
        current: actor.stamina.current,
        max: actor.stamina.max,
    });
    request.cargo = CargoLoad {
        current_weight: actor.current_load,
        max_weight: actor.max_load,
    };

    let outcome = resolve_movement(map, request);
    let result = outcome.result();
    if matches!(outcome, MovementOutcome::Moved(_)) {
        let momentum_effect = movement_effect(
            (*actor.momentum).into(),
            direction,
            actor.movement_state.mode,
        );
        let energy_cost =
            apply_energy_multiplier(result.energy_cost, momentum_effect.energy_multiplier);
        let stamina_delta = result.stamina_delta + momentum_effect.stamina_delta;

        actor.position.x = result.target.x;
        actor.position.y = result.target.y;
        actor.velocity.dx = result.actual_delta.0;
        actor.velocity.dy = result.actual_delta.1;
        actor.stamina.current =
            (actor.stamina.current + stamina_delta).clamp(0.0, actor.stamina.max);
        *actor.momentum = momentum_effect.momentum.into();
        cargo_loss_risk.add(momentum_effect.cargo_loss_risk);
        actor.energy.spend(timeline.now, energy_cost);
        tracing::debug!(
            x = actor.position.x,
            y = actor.position.y,
            terrain = ?result.terrain,
            energy_cost,
            stamina = actor.stamina.current,
            momentum = actor.momentum.amount,
            cargo_loss_risk = momentum_effect.cargo_loss_risk,
            "player moved"
        );
    } else {
        tracing::debug!(
            outcome = ?outcome,
            target_x = result.target.x,
            target_y = result.target.y,
            "player movement did not resolve"
        );
    }
}

fn apply_energy_multiplier(base: u32, multiplier: f32) -> u32 {
    ((base as f32) * multiplier).round().max(1.0) as u32
}
