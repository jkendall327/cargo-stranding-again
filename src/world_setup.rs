use bevy_ecs::prelude::*;

use crate::cargo::{
    refresh_cargo_cache, Cargo, CargoParcel, CargoStats, CarriedBy, CarrySlot, Container, Item,
    ParcelState,
};
use crate::components::*;
use crate::energy::ActionEnergy;
use crate::input::KeyBindings;
use crate::map::Map;
use crate::resources::{
    Camera, CargoLossRisk, DeliveryStats, EnergyTimeline, GameScreen, InputRepeat, InventoryIntent,
    InventoryMenuState, MenuInputState, PauseMenuState, PlayerIntent, SimulationClock,
};
use crate::systems::{
    CargoActionResult, CargoChanged, CycleMovementRequest, DeliverRequest, DropRequest,
    PickUpRequest, WaitRequest,
};

pub fn init_world(world: &mut World) {
    tracing::info!("initializing world");

    world.insert_resource(Map::generate());
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
    world.init_resource::<Messages<WaitRequest>>();
    world.init_resource::<Messages<CycleMovementRequest>>();
    world.init_resource::<Messages<PickUpRequest>>();
    world.init_resource::<Messages<DropRequest>>();
    world.init_resource::<Messages<DeliverRequest>>();
    world.init_resource::<Messages<CargoChanged>>();
    world.init_resource::<Messages<CargoActionResult>>();

    let player_entity = world
        .spawn((
            Actor,
            Player,
            Position { x: 6, y: 6 },
            Velocity::default(),
            Cargo {
                current_weight: 0.0,
                max_weight: 40.0,
            },
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
    refresh_cargo_cache(world, player_entity);

    for (id, (x, y)) in [(0, (41, 30)), (1, (52, 26))] {
        let porter_entity = world
            .spawn((
                Actor,
                AutonomousActor,
                WantsAction,
                Porter { id },
                Position { x, y },
                Velocity::default(),
                Cargo {
                    current_weight: 0.0,
                    max_weight: 35.0,
                },
                AssignedJob {
                    phase: JobPhase::FindParcel,
                    parcel: None,
                },
                ActionEnergy::default(),
            ))
            .id();
        world.spawn((
            Item,
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
        refresh_cargo_cache(world, porter_entity);
    }

    for (x, y, weight) in [
        (8, 8, 6.0),
        (18, 15, 9.0),
        (26, 33, 5.0),
        (36, 9, 8.0),
        (55, 19, 7.0),
    ] {
        world.spawn((
            Position { x, y },
            Item,
            CargoStats {
                weight,
                volume: 1.0,
            },
            CargoParcel { weight },
            ParcelState::Loose,
        ));
    }

    tracing::info!(
        porters = 2,
        parcels = 5,
        player_x = 6,
        player_y = 6,
        "world initialized"
    );
}
