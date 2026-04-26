use bevy_ecs::prelude::*;

use crate::cargo::{CargoParcel, CarriedBy};
use crate::components::{ActionEnergy, Player, Position};
use crate::resources::{EnergyTimeline, InventoryAction, InventoryIntent, InventoryMenuState};
use crate::systems::DropRequest;

pub fn inventory_actions(
    timeline: Res<EnergyTimeline>,
    mut intent: ResMut<InventoryIntent>,
    mut inventory_menu: ResMut<InventoryMenuState>,
    player: Query<(Entity, &Position, &ActionEnergy), With<Player>>,
    carried_parcels: Query<(Entity, &CarriedBy), With<CargoParcel>>,
    mut drop_requests: MessageWriter<DropRequest>,
) {
    let action = intent.action.take();
    let Some(action) = action else {
        return;
    };

    match action {
        InventoryAction::DropSelected => {
            let Some(player) = player.iter().next() else {
                return;
            };
            drop_selected_inventory_parcel(
                &timeline,
                &mut inventory_menu,
                player,
                &carried_parcels,
                &mut drop_requests,
            );
        }
    }
}

fn drop_selected_inventory_parcel(
    timeline: &EnergyTimeline,
    inventory_menu: &mut InventoryMenuState,
    player: (Entity, &Position, &ActionEnergy),
    carried_parcels: &Query<(Entity, &CarriedBy), With<CargoParcel>>,
    drop_requests: &mut MessageWriter<DropRequest>,
) -> bool {
    let (player_entity, player_position, energy) = player;
    if !energy.is_ready(timeline.now) {
        return false;
    }

    let mut parcels = carried_parcels
        .iter()
        .filter_map(|(entity, carried_by)| (carried_by.holder == player_entity).then_some(entity))
        .collect::<Vec<_>>();
    parcels.sort_by_key(|entity| entity.to_bits());
    inventory_menu.clamp_to_item_count(parcels.len());

    let selected_index = inventory_menu.selected_index();
    let Some(parcel) = parcels.get(selected_index).copied() else {
        return false;
    };

    drop_requests.write(DropRequest {
        actor: player_entity,
        item: parcel,
        at: *player_position,
    });

    tracing::info!(
        x = player_position.x,
        y = player_position.y,
        item = ?parcel,
        "player drop requested"
    );

    true
}
