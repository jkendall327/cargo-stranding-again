use bevy_ecs::prelude::*;

use crate::cargo::{Cargo, CargoParcel, ParcelState};
use crate::components::{Player, Position};
use crate::resources::CargoLossRisk;

type SpilledParcelItem<'a> = (&'a mut Position, &'a mut ParcelState);

pub fn reset_cargo_loss_risk(mut cargo_loss_risk: ResMut<CargoLossRisk>) {
    cargo_loss_risk.reset();
}

pub fn resolve_cargo_loss_risk(
    cargo_loss_risk: Res<CargoLossRisk>,
    mut player_query: Query<(Entity, &Position, &mut Cargo), With<Player>>,
    mut parcels: Query<SpilledParcelItem, (With<CargoParcel>, Without<Player>)>,
) {
    if !cargo_loss_risk.crosses_threshold() {
        return;
    }

    let Ok((player_entity, player_position, mut cargo)) = player_query.single_mut() else {
        return;
    };

    let mut dropped = 0;
    for (mut parcel_position, mut parcel_state) in &mut parcels {
        if *parcel_state != ParcelState::CarriedBy(player_entity) {
            continue;
        }

        *parcel_position = *player_position;
        *parcel_state = ParcelState::Loose;
        dropped += 1;
    }

    if dropped > 0 || cargo.current_weight > 0.0 {
        cargo.current_weight = 0.0;
        tracing::info!(
            dropped,
            risk = cargo_loss_risk.amount,
            x = player_position.x,
            y = player_position.y,
            "player cargo spilled from accumulated action risk"
        );
    }
}

pub(super) fn carried_parcel_count(
    holder: Entity,
    parcels: &Query<(Entity, &Position, &CargoParcel, &mut ParcelState), Without<Player>>,
) -> usize {
    parcels
        .iter()
        .filter(|(_, _, _, state)| **state == ParcelState::CarriedBy(holder))
        .count()
}

pub(super) fn pick_up_loose_parcel(
    holder: Entity,
    holder_position: Position,
    cargo: &mut Cargo,
    parcels: &mut Query<(Entity, &Position, &CargoParcel, &mut ParcelState), Without<Player>>,
) -> bool {
    for (_parcel_entity, parcel_position, parcel, mut parcel_state) in parcels.iter_mut() {
        if *parcel_state != ParcelState::Loose || *parcel_position != holder_position {
            continue;
        }

        if cargo.current_weight + parcel.weight > cargo.max_weight {
            return false;
        }

        *parcel_state = ParcelState::CarriedBy(holder);
        cargo.current_weight += parcel.weight;
        return true;
    }

    false
}
