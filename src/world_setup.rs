use bevy_ecs::prelude::*;

use crate::cargo::{
    Cargo, CargoParcel, CargoStats, CarriedBy, CarrySlot, Container, Item, ParcelDelivery,
};
use crate::components::*;
use crate::energy::ActionEnergy;
use crate::input::KeyBindings;
use crate::map::Map;
use crate::persistence::{PersistentId, PersistentIdAllocator};
use crate::resources::{
    Camera, CargoLossRisk, DeliveryStats, EnergyTimeline, GameScreen, InputRepeat, InventoryIntent,
    InventoryMenuState, MenuInputState, PauseMenuState, PlayerIntent, SimulationClock,
};

const PLAYER_ID: PersistentId = PersistentId(1);
const PLAYER_CHEST_LOAD_ID: PersistentId = PersistentId(100);
const PLAYER_BACKPACK_ID: PersistentId = PersistentId(101);
const PORTER_ID_BASE: u128 = 1_000;
const PORTER_PACK_ID_BASE: u128 = 1_100;
const LOOSE_PARCEL_ID_BASE: u128 = 2_000;

pub fn init_world(world: &mut World) {
    tracing::info!("initializing world");

    init_resources(world, Map::generate());
    spawn_authored_entities(world);
    reserve_existing_persistent_ids(world);
    debug_assert_unique_persistent_ids(world);

    tracing::info!(
        porters = 2,
        parcels = 5,
        player_x = 6,
        player_y = 6,
        "world initialized"
    );
}

/// Inserts the non-entity resources needed by the windowed and headless game.
///
/// Save loading uses this with a restored map before spawning saved entities,
/// so the loaded world has the same input, UI, timeline, and message resources
/// as a freshly authored game.
pub fn init_resources(world: &mut World, map: Map) {
    world.insert_resource(PersistentIdAllocator::default());
    world.insert_resource(map);
    world.insert_resource(GameScreen::default());
    world.insert_resource(PlayerIntent::default());
    world.insert_resource(MenuInputState::default());
    world.insert_resource(InputRepeat::default());
    world.insert_resource(KeyBindings::default());
    world.insert_resource(PauseMenuState::default());
    world.insert_resource(InventoryMenuState::default());
    world.insert_resource(InventoryIntent::default());
    world.insert_resource(EnergyTimeline::default());
    world.insert_resource(CargoLossRisk::default());
    world.insert_resource(Camera::default());
    world.insert_resource(SimulationClock { turn: 0 });
    world.insert_resource(DeliveryStats::default());
    crate::messages::init_simulation_messages(world);
}

fn spawn_authored_entities(world: &mut World) {
    let player_entity = world
        .spawn((
            Actor,
            Player,
            PLAYER_ID,
            Position { x: 6, y: 6 },
            Velocity::default(),
            Cargo { max_weight: 40.0 },
            Stamina {
                current: 35.0,
                max: 35.0,
            },
            MovementState::default(),
            Momentum::default(),
            ActionEnergy::default(),
        ))
        .id();

    world.spawn((
        Item,
        PLAYER_CHEST_LOAD_ID,
        CargoStats {
            weight: 12.0,
            volume: 0.0,
        },
        CarriedBy {
            holder: player_entity,
            slot: CarrySlot::Chest,
        },
    ));
    world.spawn((
        Item,
        PLAYER_BACKPACK_ID,
        CargoStats {
            weight: 2.0,
            volume: 3.0,
        },
        Container {
            volume_capacity: 12.0,
            weight_capacity: 25.0,
        },
        CarriedBy {
            holder: player_entity,
            slot: CarrySlot::Back,
        },
    ));
    for (id, (x, y)) in [(0, (41, 30)), (1, (52, 26))] {
        let porter_entity = world
            .spawn((
                Actor,
                AutonomousActor,
                WantsAction,
                Porter { id },
                PersistentId(PORTER_ID_BASE + id as u128),
                Position { x, y },
                Velocity::default(),
                Cargo { max_weight: 35.0 },
                AssignedJob {
                    phase: JobPhase::FindParcel,
                    parcel: None,
                },
                ActionEnergy::default(),
            ))
            .id();
        world.spawn((
            Item,
            PersistentId(PORTER_PACK_ID_BASE + id as u128),
            CargoStats {
                weight: 2.0,
                volume: 3.0,
            },
            Container {
                volume_capacity: 10.0,
                weight_capacity: 20.0,
            },
            CarriedBy {
                holder: porter_entity,
                slot: CarrySlot::Back,
            },
        ));
    }

    for (index, (x, y, weight)) in [
        (8, 8, 6.0),
        (18, 15, 9.0),
        (26, 33, 5.0),
        (36, 9, 8.0),
        (55, 19, 7.0),
    ]
    .into_iter()
    .enumerate()
    {
        world.spawn((
            PersistentId(LOOSE_PARCEL_ID_BASE + index as u128),
            Position { x, y },
            Item,
            CargoStats {
                weight,
                volume: 1.0,
            },
            CargoParcel,
            ParcelDelivery::Available,
        ));
    }
}

pub fn reserve_existing_persistent_ids(world: &mut World) {
    let mut query = world.query::<&PersistentId>();
    let ids = query.iter(world).copied().collect::<Vec<_>>();
    let mut allocator = world.resource_mut::<PersistentIdAllocator>();
    for id in ids {
        allocator.reserve_existing(id);
    }
}

fn debug_assert_unique_persistent_ids(world: &mut World) {
    #[cfg(debug_assertions)]
    {
        use std::collections::HashMap;

        let mut query = world.query::<(Entity, &PersistentId)>();
        let mut seen = HashMap::new();
        for (entity, id) in query.iter(world) {
            let previous = seen.insert(*id, entity);
            debug_assert!(
                previous.is_none(),
                "duplicate persistent ID {id:?} on entities {previous:?} and {entity:?}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::{save_world_data, WorldId};
    use std::collections::HashSet;

    #[test]
    fn authored_persistent_entities_start_with_unique_ids() {
        let mut world = World::new();
        init_world(&mut world);

        let mut query =
            world.query::<(Entity, Option<&Actor>, Option<&Item>, Option<&PersistentId>)>();
        let persistent_entities = query
            .iter(&world)
            .filter(|(_, actor, item, _)| actor.is_some() || item.is_some())
            .collect::<Vec<_>>();

        assert_eq!(persistent_entities.len(), 12);
        assert!(
            persistent_entities.iter().all(|(_, _, _, id)| id.is_some()),
            "all authored actors and cargo items should have persistent IDs"
        );

        let unique_ids = persistent_entities
            .iter()
            .filter_map(|(_, _, _, id)| id.copied())
            .collect::<HashSet<_>>();
        assert_eq!(unique_ids.len(), persistent_entities.len());
    }

    #[test]
    fn initial_world_can_build_world_save_payload() {
        let mut world = World::new();
        init_world(&mut world);

        let saved = save_world_data(&mut world, WorldId(1)).expect("initial world should save");

        assert_eq!(saved.world_entities.len(), 7);
    }
}
