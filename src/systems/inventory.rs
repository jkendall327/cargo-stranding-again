use bevy_ecs::prelude::*;

use crate::cargo::{CargoStats, CarriedItems, Container, ContainerContents};
use crate::components::{ActionEnergy, Player, Position};
use crate::resources::{EnergyTimeline, InventoryAction, InventoryIntent, InventoryMenuState};
use crate::systems::DropRequest;

pub fn inventory_actions(
    timeline: Res<EnergyTimeline>,
    mut intent: ResMut<InventoryIntent>,
    mut inventory_menu: ResMut<InventoryMenuState>,
    player: Query<(Entity, &Position, &ActionEnergy, Option<&CarriedItems>), With<Player>>,
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
            let Some(player) = player.iter().next() else {
                return;
            };
            drop_selected_inventory_item(
                &timeline,
                &mut inventory_menu,
                player,
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

    let mut items = carried_item_entities(carried_items, cargo_items, containers);
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

fn carried_item_entities(
    carried_items: Option<&CarriedItems>,
    cargo_items: &Query<(), With<CargoStats>>,
    containers: &Query<&ContainerContents, With<Container>>,
) -> Vec<Entity> {
    let direct_items = carried_items
        .into_iter()
        .flat_map(|items| items.iter())
        .collect::<Vec<_>>();

    let mut items = direct_items
        .iter()
        .copied()
        .filter(|entity| cargo_items.contains(*entity))
        .collect::<Vec<_>>();
    for container in direct_items {
        let Ok(contained_items) = containers.get(container) else {
            continue;
        };
        items.extend(
            contained_items
                .iter()
                .filter(|entity| cargo_items.contains(*entity)),
        );
    }
    items
}
