use bevy_ecs::prelude::*;

use crate::cargo::{
    carried_cargo_entities, CargoStats, CarriedItems, Container, ContainerContents,
};
use crate::components::{ActionEnergy, Player, Position};
use crate::resources::{EnergyTimeline, InventoryAction, InventoryIntent, InventoryMenuState};
use crate::systems::DropRequest;

pub fn inventory_actions(
    timeline: Res<EnergyTimeline>,
    mut intent: ResMut<InventoryIntent>,
    mut inventory_menu: ResMut<InventoryMenuState>,
    player: Single<(Entity, &Position, &ActionEnergy, Option<&CarriedItems>), With<Player>>,
    cargo_items: Query<(), With<CargoStats>>,
    containers: Query<&ContainerContents, With<Container>>,
    mut drop_requests: MessageWriter<DropRequest>,
) {
    let action = intent.action.take();
    let Some(action) = action else {
        return;
    };

    match action {
        InventoryAction::DropSelected => {
            drop_selected_inventory_item(
                &timeline,
                &mut inventory_menu,
                player.into_inner(),
                &cargo_items,
                &containers,
                &mut drop_requests,
            );
        }
    }
}

fn drop_selected_inventory_item(
    timeline: &EnergyTimeline,
    inventory_menu: &mut InventoryMenuState,
    player: (Entity, &Position, &ActionEnergy, Option<&CarriedItems>),
    cargo_items: &Query<(), With<CargoStats>>,
    containers: &Query<&ContainerContents, With<Container>>,
    drop_requests: &mut MessageWriter<DropRequest>,
) -> bool {
    let (player_entity, player_position, energy, carried_items) = player;
    if !energy.is_ready(timeline.now) {
        return false;
    }

    let mut items = carried_cargo_entities(carried_items, cargo_items, containers);
    items.sort_by_key(|entity| entity.to_bits());
    inventory_menu.clamp_to_item_count(items.len());

    let selected_index = inventory_menu.selected_index();
    let Some(item) = items.get(selected_index).copied() else {
        return false;
    };

    drop_requests.write(DropRequest {
        actor: player_entity,
        item,
        at: *player_position,
    });

    tracing::info!(
        x = player_position.x,
        y = player_position.y,
        item = ?item,
        "player drop requested"
    );

    true
}
