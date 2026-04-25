use bevy_ecs::prelude::*;

use crate::components::{ActionEnergy, Cargo, CargoParcel, ParcelState, Player, Position};
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

    let parcels = player_carried_parcels_for(world, player_entity);
    world
        .resource_mut::<InventoryMenuState>()
        .clamp_to_item_count(parcels.len());

    let selected_index = world.resource::<InventoryMenuState>().selected_index();
    let Some(parcel_entity) = parcels.get(selected_index).copied() else {
        return false;
    };

    let Some(parcel_weight) = world
        .get::<CargoParcel>(parcel_entity)
        .map(|parcel| parcel.weight)
    else {
        return false;
    };

    if let Some(mut parcel_position) = world.get_mut::<Position>(parcel_entity) {
        *parcel_position = player_position;
    }
    if let Some(mut parcel_state) = world.get_mut::<ParcelState>(parcel_entity) {
        *parcel_state = ParcelState::Loose;
    }

    let now = world.resource::<EnergyTimeline>().now;
    let cargo_weight = {
        let mut player_query =
            world.query_filtered::<(&mut Cargo, &mut ActionEnergy), With<Player>>();
        let Some((mut cargo, mut energy)) = player_query.iter_mut(world).next() else {
            return false;
        };

        cargo.current_weight = (cargo.current_weight - parcel_weight).max(0.0);
        energy.spend(now, ITEM_ACTION_ENERGY_COST);
        cargo.current_weight
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

fn player_carried_parcels_for(world: &mut World, player_entity: Entity) -> Vec<Entity> {
    let mut parcel_query = world.query_filtered::<(Entity, &ParcelState), With<CargoParcel>>();
    let mut parcels = parcel_query
        .iter(world)
        .filter_map(|(entity, state)| {
            if *state == ParcelState::CarriedBy(player_entity) {
                Some(entity)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    parcels.sort_by_key(|entity| entity.to_bits());
    parcels
}
