use bevy_ecs::prelude::*;

use crate::cargo::{cargo_load, carried_parcels, drop_carried_parcel};
use crate::components::{ActionEnergy, Player, Position};
use crate::energy::ITEM_ACTION_ENERGY_COST;
use crate::resources::{EnergyTimeline, InventoryAction, InventoryIntent, InventoryMenuState};

pub fn inventory_actions(world: &mut World) {
    let action = world.resource_mut::<InventoryIntent>().action.take();
    let Some(action) = action else {
        return;
    };

    match action {
        InventoryAction::DropSelected => {
            drop_selected_inventory_parcel(world);
        }
    }
}

fn drop_selected_inventory_parcel(world: &mut World) -> bool {
    let Some((player_entity, player_position)) = ready_player(world) else {
        return false;
    };

    let parcels = carried_parcels(world, player_entity);
    world
        .resource_mut::<InventoryMenuState>()
        .clamp_to_item_count(parcels.len());

    let selected_index = world.resource::<InventoryMenuState>().selected_index();
    let Some(parcel) = parcels.get(selected_index).copied() else {
        return false;
    };

    if !drop_carried_parcel(world, player_entity, parcel.entity, player_position) {
        return false;
    }

    let now = world.resource::<EnergyTimeline>().now;
    let cargo_weight = {
        let Some(mut energy) = world.get_mut::<ActionEnergy>(player_entity) else {
            return false;
        };

        energy.spend(now, ITEM_ACTION_ENERGY_COST);
        cargo_load(world, player_entity).unwrap_or(0.0)
    };

    world
        .resource_mut::<InventoryMenuState>()
        .clamp_to_item_count(parcels.len().saturating_sub(1));

    tracing::info!(
        x = player_position.x,
        y = player_position.y,
        cargo = cargo_weight,
        "player dropped parcel"
    );

    true
}

fn ready_player(world: &mut World) -> Option<(Entity, Position)> {
    let now = world.resource::<EnergyTimeline>().now;
    let mut player_query =
        world.query_filtered::<(Entity, &Position, &ActionEnergy), With<Player>>();
    let (entity, position, energy) = player_query.iter(world).next()?;
    energy.is_ready(now).then_some((entity, *position))
}
