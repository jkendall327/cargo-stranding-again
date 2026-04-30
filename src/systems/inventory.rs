use bevy_ecs::prelude::*;

use crate::cargo::{CargoStats, CarriedBy, ContainedIn, Container};
use crate::components::{ActionEnergy, Player, Position};
use crate::resources::{EnergyTimeline, InventoryAction, InventoryIntent, InventoryMenuState};
use crate::systems::DropRequest;

type CarriedItemQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static CarriedBy>,
        Option<&'static ContainedIn>,
    ),
    With<CargoStats>,
>;

pub fn inventory_actions(
    timeline: Res<EnergyTimeline>,
    mut intent: ResMut<InventoryIntent>,
    mut inventory_menu: ResMut<InventoryMenuState>,
    player: Query<(Entity, &Position, &ActionEnergy), With<Player>>,
    carried_items: CarriedItemQuery,
    containers: Query<&CarriedBy, With<Container>>,
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
                &carried_items,
                &containers,
                &mut drop_requests,
            );
        }
    }
}

fn drop_selected_inventory_item(
    timeline: &EnergyTimeline,
    inventory_menu: &mut InventoryMenuState,
    player: (Entity, &Position, &ActionEnergy),
    carried_items: &CarriedItemQuery,
    containers: &Query<&CarriedBy, With<Container>>,
    drop_requests: &mut MessageWriter<DropRequest>,
) -> bool {
    let (player_entity, player_position, energy) = player;
    if !energy.is_ready(timeline.now) {
        return false;
    }

    let mut items = carried_items
        .iter()
        .filter_map(|(entity, carried_by, contained_in)| {
            item_carried_by_actor(carried_by, contained_in, containers, player_entity)
                .then_some(entity)
        })
        .collect::<Vec<_>>();
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

fn item_carried_by_actor(
    carried_by: Option<&CarriedBy>,
    contained_in: Option<&ContainedIn>,
    containers: &Query<&CarriedBy, With<Container>>,
    actor: Entity,
) -> bool {
    carried_by.is_some_and(|carried_by| carried_by.holder == actor)
        || contained_in.is_some_and(|contained_in| {
            containers
                .get(contained_in.container)
                .is_ok_and(|carried_by| carried_by.holder == actor)
        })
}
