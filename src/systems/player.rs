use bevy_ecs::prelude::*;

mod cargo;
mod movement;

use crate::cargo::{
    derived_load, Cargo, CargoParcel, CargoTarget, CarriedBy, ContainedIn, Container, Item,
    ParcelState,
};
use crate::components::*;
use crate::energy::ActionEnergy;
use crate::map::Map;
use crate::resources::{
    CargoLossRisk, EnergyTimeline, GameScreen, InventoryMenuState, PlayerAction, PlayerIntent,
};
use crate::systems::PickUpRequest;

pub use cargo::{reset_cargo_loss_risk, resolve_cargo_loss_risk};

use self::movement::{try_move_player, PlayerMovement};

type PlayerActionItem<'a> = (
    Entity,
    &'a mut Position,
    &'a mut Velocity,
    &'a mut Stamina,
    &'a mut Cargo,
    &'a mut MovementState,
    &'a mut Momentum,
    &'a mut ActionEnergy,
);

type PlayerPickupItem<'a> = (Entity, &'a Position, &'a ParcelState);
type PlayerPickupFilter = (With<Item>, With<CargoParcel>, Without<CarriedBy>);

pub fn open_inventory_from_player_intent(
    intent: Res<PlayerIntent>,
    timeline: Res<EnergyTimeline>,
    mut screen: ResMut<GameScreen>,
    mut inventory_menu: ResMut<InventoryMenuState>,
    player: Single<(Entity, &ActionEnergy), With<Player>>,
    carried_parcels: Query<(Option<&CarriedBy>, Option<&ContainedIn>), With<CargoParcel>>,
    containers: Query<&CarriedBy, With<Container>>,
) {
    let Some(PlayerAction::OpenInventory) = intent.action else {
        return;
    };

    let (player_entity, energy) = player.into_inner();
    if !energy.is_ready(timeline.now) {
        return;
    }

    let carried_count = carried_parcels
        .iter()
        .filter(|(carried_by, contained_in)| {
            parcel_carried_by_actor(*carried_by, *contained_in, &containers, player_entity)
        })
        .count();
    inventory_menu.clamp_to_item_count(carried_count);
    *screen = GameScreen::InventoryMenu;
    tracing::debug!(carried_count, "opened inventory");
}

fn parcel_carried_by_actor(
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

pub fn pick_up_player_parcel_from_intent(
    intent: Res<PlayerIntent>,
    timeline: Res<EnergyTimeline>,
    player: Single<(Entity, &Position, &mut Velocity, &ActionEnergy), With<Player>>,
    parcels: Query<PlayerPickupItem, PlayerPickupFilter>,
    containers: Query<(Entity, &CarriedBy), With<Container>>,
    mut pickup_requests: MessageWriter<PickUpRequest>,
) {
    let Some(PlayerAction::PickUp) = intent.action else {
        return;
    };

    let (player_entity, player_position, mut velocity, energy) = player.into_inner();
    if !energy.is_ready(timeline.now) {
        return;
    }

    velocity.dx = 0;
    velocity.dy = 0;

    let Some((parcel_entity, _, _)) = parcels.iter().find(|(_, position, state)| {
        **position == *player_position && matches!(**state, ParcelState::Loose)
    }) else {
        tracing::debug!(
            x = player_position.x,
            y = player_position.y,
            "player pickup found no parcel"
        );
        return;
    };

    pickup_requests.write(PickUpRequest {
        actor: player_entity,
        item: parcel_entity,
        target: carried_container_target(player_entity, &containers),
    });
    tracing::debug!(
        x = player_position.x,
        y = player_position.y,
        item = ?parcel_entity,
        "player pickup requested"
    );
}

fn carried_container_target(
    actor: Entity,
    containers: &Query<(Entity, &CarriedBy), With<Container>>,
) -> CargoTarget {
    containers
        .iter()
        .filter_map(|(entity, carried_by)| (carried_by.holder == actor).then_some(entity))
        .min_by_key(|entity| entity.to_bits())
        .map_or(
            CargoTarget::Slot(crate::cargo::CarrySlot::Back),
            CargoTarget::Container,
        )
}

pub fn player_actions(world: &mut World) {
    let now = world.resource::<EnergyTimeline>().now;
    let Some(action) = world.resource::<PlayerIntent>().action else {
        return;
    };

    if matches!(
        action,
        PlayerAction::OpenInventory
            | PlayerAction::PickUp
            | PlayerAction::CycleMovementMode
            | PlayerAction::Wait
    ) {
        return;
    }

    let map = world.resource::<Map>().clone();
    let timeline = *world.resource::<EnergyTimeline>();
    let mut cargo_loss_risk = *world.resource::<CargoLossRisk>();
    let Some(player_entity) = ({
        let mut query = world.query_filtered::<Entity, With<Player>>();
        query.iter(world).next()
    }) else {
        return;
    };
    let current_load = derived_load(world, player_entity);

    let mut player_query = world.query_filtered::<PlayerActionItem, With<Player>>();
    let Ok((
        entity,
        mut position,
        mut velocity,
        mut stamina,
        cargo,
        movement_state,
        mut momentum,
        mut energy,
    )) = player_query.single_mut(world)
    else {
        return;
    };

    if !energy.is_ready(now) {
        return;
    }

    velocity.dx = 0;
    velocity.dy = 0;

    tracing::debug!(?action, now, "processing player action");

    match action {
        PlayerAction::Move(direction) => {
            try_move_player(
                PlayerMovement {
                    entity,
                    position: &mut position,
                    velocity: &mut velocity,
                    stamina: &mut stamina,
                    current_load,
                    max_load: cargo.max_weight,
                    movement_state: &movement_state,
                    momentum: &mut momentum,
                    energy: &mut energy,
                },
                direction,
                &map,
                &timeline,
                &mut cargo_loss_risk,
            );
            *world.resource_mut::<CargoLossRisk>() = cargo_loss_risk;
        }
        PlayerAction::PickUp => {}
        PlayerAction::CycleMovementMode => {}
        PlayerAction::OpenInventory => {}
        PlayerAction::Wait => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::{derived_load, CargoParcel, CargoStats, CarriedBy, CarrySlot, Item};
    use crate::map::{Terrain, TileCoord};
    use crate::resources::Direction;
    use crate::systems::{
        clamp_inventory_after_cargo_drop, clear_failed_porter_cargo_jobs,
        emit_player_cycle_movement_request, emit_player_wait_request, log_failed_cargo_actions,
        maintain_cargo_messages, maintain_cycle_movement_requests, maintain_wait_requests,
        resolve_cycle_movement_requests, resolve_delivery_requests, resolve_drop_requests,
        resolve_pickup_requests, resolve_wait_requests, spend_energy_for_successful_cargo_actions,
        update_porter_jobs_from_cargo_results, CargoActionResult, CycleMovementRequest,
        DeliverRequest, DropRequest, PickUpRequest, WaitRequest,
    };
    use bevy_ecs::schedule::ApplyDeferred;

    fn insert_player_action_resources(world: &mut World, action: PlayerAction) {
        world.insert_resource(PlayerIntent {
            action: Some(action),
        });
        world.insert_resource(EnergyTimeline::default());
        world.insert_resource(CargoLossRisk::default());
        world.insert_resource(GameScreen::Playing);
        world.insert_resource(InventoryMenuState::default());
        world.insert_resource(crate::resources::DeliveryStats::default());
        world.init_resource::<Messages<WaitRequest>>();
        world.init_resource::<Messages<CycleMovementRequest>>();
        world.init_resource::<Messages<PickUpRequest>>();
        world.init_resource::<Messages<DropRequest>>();
        world.init_resource::<Messages<DeliverRequest>>();
        world.init_resource::<Messages<CargoActionResult>>();
    }

    fn spawn_test_parcel(world: &mut World, position: Position) {
        spawn_test_parcel_with_weight(world, position, 5.0);
    }

    fn spawn_test_parcel_with_weight(world: &mut World, position: Position, weight: f32) {
        world.spawn((
            position,
            Item,
            CargoStats {
                weight,
                volume: 1.0,
            },
            CargoParcel { weight },
            ParcelState::Loose,
        ));
    }

    fn spawn_carried_test_parcel(world: &mut World, holder: Entity, position: Position) {
        world.spawn((
            position,
            Item,
            CargoStats {
                weight: 5.0,
                volume: 1.0,
            },
            CarriedBy {
                holder,
                slot: CarrySlot::Back,
            },
            CargoParcel { weight: 5.0 },
            ParcelState::CarriedBy(holder),
        ));
    }

    fn spawn_test_player(world: &mut World, position: Position, stamina: f32) -> Entity {
        world
            .spawn((
                Player,
                position,
                Velocity::default(),
                Cargo { max_weight: 40.0 },
                Stamina {
                    current: stamina,
                    max: 35.0,
                },
                MovementState::default(),
                Momentum::default(),
                ActionEnergy::default(),
            ))
            .id()
    }

    fn find_adjacent_terrain_pair(map: &Map, terrain: Terrain) -> (Position, Position) {
        let bounds = map.bounds();
        for y in 0..bounds.height {
            for x in 0..(bounds.width - 1) {
                let coord = TileCoord::new(x, y);
                let east = TileCoord::new(x + 1, y);
                if map.terrain_at_coord(coord) == Some(terrain)
                    && map.terrain_at_coord(east) == Some(terrain)
                {
                    return (Position { x, y }, Position { x: x + 1, y });
                }
            }
        }
        panic!("test map should contain adjacent {terrain:?} tiles");
    }

    fn run_player_move(world: &mut World, start: Position, target: Position, stamina: f32) {
        let dx = target.x - start.x;
        let dy = target.y - start.y;
        assert!(dx.abs() + dy.abs() == 1);

        let direction = match (dx, dy) {
            (-1, 0) => Direction::West,
            (1, 0) => Direction::East,
            (0, -1) => Direction::North,
            (0, 1) => Direction::South,
            _ => unreachable!("test movement should be cardinal"),
        };
        insert_player_action_resources(world, PlayerAction::Move(direction));
        spawn_test_player(world, start, stamina);

        let mut schedule = Schedule::default();
        schedule.add_systems(player_actions);
        schedule.run(world);
    }

    #[test]
    fn failed_player_movement_does_not_consume_turn() {
        let mut world = World::new();
        world.insert_resource(Map::generate());
        insert_player_action_resources(&mut world, PlayerAction::Move(Direction::West));
        spawn_test_player(&mut world, Position { x: 0, y: 0 }, 35.0);

        let mut schedule = Schedule::default();
        schedule.add_systems(player_actions);
        schedule.run(&mut world);

        let mut energy_query = world.query_filtered::<&ActionEnergy, With<Player>>();
        let energy = energy_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert_eq!(energy.ready_at, 0);

        let mut player_query = world.query_filtered::<(&Position, &Momentum), With<Player>>();
        let (position, momentum) = player_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert_eq!(*position, Position { x: 0, y: 0 });
        assert_eq!(*momentum, Momentum::default());
        assert_eq!(world.resource::<CargoLossRisk>().amount, 0);
    }

    #[test]
    fn grass_movement_consumes_turn_without_draining_stamina() {
        let mut world = World::new();
        let map = Map::generate();
        let (start, target) = find_adjacent_terrain_pair(&map, Terrain::Grass);
        world.insert_resource(map);

        run_player_move(&mut world, start, target, 10.0);

        let mut energy_query = world.query_filtered::<&ActionEnergy, With<Player>>();
        let energy = energy_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert!(energy.ready_at > 0);

        let mut player_query = world.query_filtered::<&Stamina, With<Player>>();
        let stamina = player_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert_eq!(stamina.current, 10.0);
    }

    #[test]
    fn road_movement_consumes_turn_and_recovers_stamina() {
        let mut world = World::new();
        let map = Map::generate();
        let target = Position { x: 6, y: 31 };
        let start = Position { x: 5, y: 31 };
        assert_eq!(map.terrain_at_coord(start.into()), Some(Terrain::Road));
        assert_eq!(map.terrain_at_coord(target.into()), Some(Terrain::Road));
        world.insert_resource(map);

        run_player_move(&mut world, start, target, 10.0);

        let mut energy_query = world.query_filtered::<&ActionEnergy, With<Player>>();
        let energy = energy_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert!(energy.ready_at > 0);

        let mut player_query = world.query_filtered::<&Stamina, With<Player>>();
        let stamina = player_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert!(stamina.current > 10.0);
    }

    #[test]
    fn successful_movement_updates_momentum() {
        let mut world = World::new();
        let map = Map::generate();
        let (start, target) = find_adjacent_terrain_pair(&map, Terrain::Grass);
        world.insert_resource(map);

        run_player_move(&mut world, start, target, 35.0);

        let mut player_query = world.query_filtered::<&Momentum, With<Player>>();
        let momentum = player_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert_eq!(momentum.amount, 1.0);
        assert!(momentum.direction.is_some());
    }

    #[test]
    fn wait_consumes_turn_and_recovers_stamina() {
        let mut world = World::new();
        world.insert_resource(Map::generate());
        insert_player_action_resources(&mut world, PlayerAction::Wait);
        spawn_test_player(&mut world, Position { x: 0, y: 0 }, 10.0);

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                emit_player_wait_request,
                resolve_wait_requests,
                maintain_wait_requests,
            )
                .chain(),
        );
        schedule.run(&mut world);

        let mut energy_query = world.query_filtered::<&ActionEnergy, With<Player>>();
        let energy = energy_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert!(energy.ready_at > 0);

        let mut player_query = world.query_filtered::<&Stamina, With<Player>>();
        let stamina = player_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert!(stamina.current > 10.0);

        let mut momentum_query = world.query_filtered::<&Momentum, With<Player>>();
        let momentum = momentum_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert_eq!(*momentum, Momentum::default());
    }

    #[test]
    fn wait_decays_existing_momentum() {
        let mut world = World::new();
        world.insert_resource(Map::generate());
        insert_player_action_resources(&mut world, PlayerAction::Wait);
        let player = spawn_test_player(&mut world, Position { x: 0, y: 0 }, 10.0);
        *world
            .get_mut::<Momentum>(player)
            .expect("test player should have momentum") = Momentum {
            direction: Some(Direction::East),
            amount: 5.0,
        };

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                emit_player_wait_request,
                resolve_wait_requests,
                maintain_wait_requests,
            )
                .chain(),
        );
        schedule.run(&mut world);

        let momentum = world
            .get::<Momentum>(player)
            .expect("test player should have momentum");
        assert_eq!(momentum.direction, Some(Direction::East));
        assert_eq!(momentum.amount, 3.0);
    }

    #[test]
    fn high_risk_sharp_turn_drops_carried_player_parcels() {
        let mut world = World::new();
        let map = Map::generate();
        let start = Position { x: 6, y: 6 };
        assert!(map.is_passable_coord(TileCoord::new(start.x, start.y + 1)));
        world.insert_resource(map);
        insert_player_action_resources(&mut world, PlayerAction::Move(Direction::South));
        let player = spawn_test_player(&mut world, start, 35.0);
        *world
            .get_mut::<Momentum>(player)
            .expect("test player should have momentum") = Momentum {
            direction: Some(Direction::East),
            amount: 5.0,
        };
        spawn_carried_test_parcel(&mut world, player, Position { x: 0, y: 0 });

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                reset_cargo_loss_risk,
                player_actions,
                resolve_cargo_loss_risk,
                resolve_pickup_requests,
                resolve_drop_requests,
                resolve_delivery_requests,
                ApplyDeferred,
                spend_energy_for_successful_cargo_actions,
                update_porter_jobs_from_cargo_results,
                clear_failed_porter_cargo_jobs,
                clamp_inventory_after_cargo_drop,
                log_failed_cargo_actions,
                maintain_cargo_messages,
            )
                .chain(),
        );
        schedule.run(&mut world);

        let (player_entity, player_position) = {
            let mut player_query = world.query_filtered::<(Entity, &Position), With<Player>>();
            let (player_entity, player_position) = player_query
                .iter(&world)
                .next()
                .expect("test player should exist");
            (player_entity, *player_position)
        };
        assert_eq!(player_position, Position { x: 6, y: 7 });
        assert_eq!(derived_load(&mut world, player_entity), 0.0);

        let mut parcel_query = world.query::<(&Position, &ParcelState)>();
        let (parcel_position, parcel_state) = parcel_query
            .iter(&world)
            .find(|(_, state)| **state == ParcelState::Loose)
            .expect("carried parcel should be dropped");
        assert_eq!(*parcel_position, player_position);
        assert_eq!(*parcel_state, ParcelState::Loose);
    }

    #[test]
    fn cargo_loss_resolver_accepts_non_momentum_risk_source() {
        let mut world = World::new();
        world.insert_resource(CargoLossRisk { amount: 100 });
        world.insert_resource(EnergyTimeline::default());
        world.insert_resource(InventoryMenuState::default());
        world.insert_resource(crate::resources::DeliveryStats::default());
        world.init_resource::<Messages<PickUpRequest>>();
        world.init_resource::<Messages<DropRequest>>();
        world.init_resource::<Messages<DeliverRequest>>();
        world.init_resource::<Messages<CargoActionResult>>();
        let player = spawn_test_player(&mut world, Position { x: 2, y: 2 }, 35.0);
        spawn_carried_test_parcel(&mut world, player, Position { x: 0, y: 0 });

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                resolve_cargo_loss_risk,
                resolve_pickup_requests,
                resolve_drop_requests,
                resolve_delivery_requests,
                ApplyDeferred,
                spend_energy_for_successful_cargo_actions,
                update_porter_jobs_from_cargo_results,
                clear_failed_porter_cargo_jobs,
                clamp_inventory_after_cargo_drop,
                log_failed_cargo_actions,
                maintain_cargo_messages,
            )
                .chain(),
        );
        schedule.run(&mut world);

        assert_eq!(derived_load(&mut world, player), 0.0);

        let mut parcel_query = world.query::<&ParcelState>();
        assert_eq!(
            parcel_query
                .iter(&world)
                .filter(|state| matches!(state, ParcelState::Loose))
                .count(),
            1
        );
    }

    #[test]
    fn cycling_movement_mode_changes_mode_without_consuming_turn() {
        let mut world = World::new();
        world.insert_resource(Map::generate());
        insert_player_action_resources(&mut world, PlayerAction::CycleMovementMode);
        spawn_test_player(&mut world, Position { x: 0, y: 0 }, 35.0);

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                emit_player_cycle_movement_request,
                resolve_cycle_movement_requests,
                maintain_cycle_movement_requests,
            )
                .chain(),
        );
        schedule.run(&mut world);

        let mut energy_query = world.query_filtered::<&ActionEnergy, With<Player>>();
        let energy = energy_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert_eq!(energy.ready_at, 0);

        let mut player_query = world.query_filtered::<&MovementState, With<Player>>();
        let movement_state = player_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert_eq!(
            movement_state.mode,
            crate::movement::MovementMode::Sprinting
        );
    }

    #[test]
    fn cycling_movement_mode_reaches_steady_on_second_tap() {
        let mut world = World::new();
        world.insert_resource(Map::generate());
        insert_player_action_resources(&mut world, PlayerAction::CycleMovementMode);
        spawn_test_player(&mut world, Position { x: 0, y: 0 }, 35.0);

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                emit_player_cycle_movement_request,
                resolve_cycle_movement_requests,
                maintain_cycle_movement_requests,
            )
                .chain(),
        );
        schedule.run(&mut world);
        schedule.run(&mut world);

        let mut player_query = world.query_filtered::<&MovementState, With<Player>>();
        let movement_state = player_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert_eq!(movement_state.mode, crate::movement::MovementMode::Steady);
    }

    #[test]
    fn opening_inventory_changes_screen_without_consuming_turn() {
        let mut world = World::new();
        world.insert_resource(Map::generate());
        insert_player_action_resources(&mut world, PlayerAction::OpenInventory);
        spawn_test_player(&mut world, Position { x: 0, y: 0 }, 35.0);

        let mut schedule = Schedule::default();
        schedule.add_systems(open_inventory_from_player_intent);
        schedule.run(&mut world);

        assert_eq!(*world.resource::<GameScreen>(), GameScreen::InventoryMenu);

        let mut energy_query = world.query_filtered::<&ActionEnergy, With<Player>>();
        let energy = energy_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert_eq!(energy.ready_at, 0);
    }

    #[test]
    fn player_can_pick_up_loose_parcel_on_same_tile() {
        let mut world = World::new();
        world.insert_resource(Map::generate());
        insert_player_action_resources(&mut world, PlayerAction::PickUp);
        spawn_test_player(&mut world, Position { x: 2, y: 2 }, 35.0);
        spawn_test_parcel(&mut world, Position { x: 2, y: 2 });

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                pick_up_player_parcel_from_intent,
                resolve_pickup_requests,
                resolve_drop_requests,
                resolve_delivery_requests,
                ApplyDeferred,
                spend_energy_for_successful_cargo_actions,
                update_porter_jobs_from_cargo_results,
                clear_failed_porter_cargo_jobs,
                clamp_inventory_after_cargo_drop,
                log_failed_cargo_actions,
                maintain_cargo_messages,
            )
                .chain(),
        );
        schedule.run(&mut world);

        let mut energy_query = world.query_filtered::<&ActionEnergy, With<Player>>();
        let energy = energy_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert!(energy.ready_at > 0);

        let mut cargo_query = world.query_filtered::<Entity, With<Player>>();
        let player = cargo_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert_eq!(derived_load(&mut world, player), 5.0);

        let mut parcel_query = world.query::<&ParcelState>();
        let carried_parcels = parcel_query
            .iter(&world)
            .filter(|state| matches!(state, ParcelState::CarriedBy(_)))
            .count();
        assert_eq!(carried_parcels, 1);
    }

    #[test]
    fn failed_pickup_does_not_consume_turn() {
        let mut world = World::new();
        world.insert_resource(Map::generate());
        insert_player_action_resources(&mut world, PlayerAction::PickUp);
        spawn_test_player(&mut world, Position { x: 2, y: 2 }, 35.0);

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                pick_up_player_parcel_from_intent,
                resolve_pickup_requests,
                resolve_drop_requests,
                resolve_delivery_requests,
                ApplyDeferred,
                spend_energy_for_successful_cargo_actions,
                update_porter_jobs_from_cargo_results,
                clear_failed_porter_cargo_jobs,
                clamp_inventory_after_cargo_drop,
                log_failed_cargo_actions,
                maintain_cargo_messages,
            )
                .chain(),
        );
        schedule.run(&mut world);

        let mut energy_query = world.query_filtered::<&ActionEnergy, With<Player>>();
        let energy = energy_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert_eq!(energy.ready_at, 0);
    }

    #[test]
    fn failed_oversized_pickup_does_not_consume_turn() {
        let mut world = World::new();
        world.insert_resource(Map::generate());
        insert_player_action_resources(&mut world, PlayerAction::PickUp);
        let player = spawn_test_player(&mut world, Position { x: 2, y: 2 }, 35.0);
        spawn_test_parcel_with_weight(&mut world, Position { x: 2, y: 2 }, 45.0);

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                pick_up_player_parcel_from_intent,
                resolve_pickup_requests,
                resolve_drop_requests,
                resolve_delivery_requests,
                ApplyDeferred,
                spend_energy_for_successful_cargo_actions,
                update_porter_jobs_from_cargo_results,
                clear_failed_porter_cargo_jobs,
                clamp_inventory_after_cargo_drop,
                log_failed_cargo_actions,
                maintain_cargo_messages,
            )
                .chain(),
        );
        schedule.run(&mut world);

        let energy = world
            .get::<ActionEnergy>(player)
            .expect("test player should have energy");
        assert_eq!(energy.ready_at, 0);
        assert_eq!(derived_load(&mut world, player), 0.0);

        let mut parcel_query = world.query::<&ParcelState>();
        assert!(parcel_query
            .iter(&world)
            .all(|state| matches!(state, ParcelState::Loose)));
    }
}
