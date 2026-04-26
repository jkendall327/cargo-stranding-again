use bevy_ecs::prelude::*;

use crate::cargo as cargo_model;
use crate::components::{Player, Position};
use crate::resources::CargoLossRisk;

pub fn reset_cargo_loss_risk(mut cargo_loss_risk: ResMut<CargoLossRisk>) {
    cargo_loss_risk.reset();
}

pub fn resolve_cargo_loss_risk(world: &mut World) {
    let cargo_loss_risk = *world.resource::<CargoLossRisk>();
    if !cargo_loss_risk.crosses_threshold() {
        return;
    }

    let Some((player_entity, player_position)) = player_entity_and_position(world) else {
        return;
    };

    let dropped = cargo_model::drop_carried_parcels(world, player_entity, player_position);
    let cargo_weight = cargo_model::cargo_load(world, player_entity).unwrap_or(0.0);

    if dropped > 0 {
        tracing::info!(
            dropped,
            risk = cargo_loss_risk.amount,
            x = player_position.x,
            y = player_position.y,
            cargo = cargo_weight,
            "player cargo spilled from accumulated action risk"
        );
    }
}

fn player_entity_and_position(world: &mut World) -> Option<(Entity, Position)> {
    let mut player_query = world.query_filtered::<(Entity, &Position), With<Player>>();
    let (entity, position) = player_query.iter(world).next()?;
    Some((entity, *position))
}
