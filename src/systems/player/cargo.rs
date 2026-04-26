use bevy_ecs::prelude::*;

use crate::cargo as cargo_model;
use crate::components::{Player, Position};
use crate::resources::CargoLossRisk;
use crate::systems::DropRequest;

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

    let parcels = cargo_model::carried_parcels(world, player_entity);
    for parcel in &parcels {
        world
            .resource_mut::<Messages<DropRequest>>()
            .write(DropRequest {
                actor: player_entity,
                item: parcel.entity,
                at: player_position,
            });
    }

    if !parcels.is_empty() {
        tracing::info!(
            dropped = parcels.len(),
            risk = cargo_loss_risk.amount,
            x = player_position.x,
            y = player_position.y,
            "player cargo spilled from accumulated action risk"
        );
    }
}

fn player_entity_and_position(world: &mut World) -> Option<(Entity, Position)> {
    let mut player_query = world.query_filtered::<(Entity, &Position), With<Player>>();
    let (entity, position) = player_query.iter(world).next()?;
    Some((entity, *position))
}
