use bevy_ecs::prelude::*;

use crate::components::*;
use crate::energy::{ActionEnergy, ITEM_ACTION_ENERGY_COST, WAIT_ENERGY_COST};
use crate::map::Map;
use crate::momentum::{movement_effect, wait_momentum};
use crate::movement::{
    resolve_movement, CargoLoad, MovementOutcome, MovementRequest, StaminaBudget,
};
use crate::resources::{
    CargoLossRisk, EnergyTimeline, GameScreen, InventoryMenuState, PlayerAction, PlayerIntent,
};

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

const WAIT_STAMINA_RECOVERY: f32 = 3.0;

pub fn player_actions(
    intent: Res<PlayerIntent>,
    timeline: Res<EnergyTimeline>,
    map: Res<Map>,
    mut screen: ResMut<GameScreen>,
    mut inventory_menu: ResMut<InventoryMenuState>,
    mut cargo_loss_risk: ResMut<CargoLossRisk>,
    mut player_query: Query<PlayerActionItem, With<Player>>,
    mut parcels: Query<(Entity, &Position, &CargoParcel, &mut ParcelState), Without<Player>>,
) {
    let now = timeline.now;

    let map = &*map;
    let Ok((
        entity,
        mut position,
        mut velocity,
        mut stamina,
        mut cargo,
        mut movement_state,
        mut momentum,
        mut energy,
    )) = player_query.single_mut()
    else {
        return;
    };

    if !energy.is_ready(now) {
        return;
    }

    velocity.dx = 0;
    velocity.dy = 0;

    let Some(action) = intent.action else {
        return;
    };

    tracing::debug!(?action, now, "processing player action");

    if action == PlayerAction::OpenInventory {
        let carried_count = carried_parcel_count(entity, &parcels);
        inventory_menu.clamp_to_item_count(carried_count);
        *screen = GameScreen::InventoryMenu;
        tracing::debug!(carried_count, "opened inventory");
        return;
    }

    if action == PlayerAction::Wait {
        stamina.current = (stamina.current + WAIT_STAMINA_RECOVERY).min(stamina.max);
        *momentum = wait_momentum((*momentum).into()).into();
        energy.spend(now, WAIT_ENERGY_COST);
        tracing::debug!(
            ready_at = energy.ready_at,
            stamina = stamina.current,
            momentum = momentum.amount,
            "player waited"
        );
        return;
    }

    match action {
        PlayerAction::Move(direction) => {
            let mut request = MovementRequest::new(*position, direction, movement_state.mode);
            request.entity = Some(entity);
            request.stamina = Some(StaminaBudget {
                current: stamina.current,
                max: stamina.max,
            });
            request.cargo = CargoLoad {
                current_weight: cargo.current_weight,
                max_weight: cargo.max_weight,
            };

            let outcome = resolve_movement(map, request);
            let result = outcome.result();
            if matches!(outcome, MovementOutcome::Moved(_)) {
                let momentum_effect =
                    movement_effect((*momentum).into(), direction, movement_state.mode);
                let energy_cost =
                    apply_energy_multiplier(result.energy_cost, momentum_effect.energy_multiplier);
                let stamina_delta = result.stamina_delta + momentum_effect.stamina_delta;

                position.x = result.target.x;
                position.y = result.target.y;
                velocity.dx = result.actual_delta.0;
                velocity.dy = result.actual_delta.1;
                stamina.current = (stamina.current + stamina_delta).clamp(0.0, stamina.max);
                *momentum = momentum_effect.momentum.into();
                cargo_loss_risk.add(momentum_effect.cargo_loss_risk);
                energy.spend(now, energy_cost);
                tracing::debug!(
                    x = position.x,
                    y = position.y,
                    terrain = ?result.terrain,
                    energy_cost,
                    stamina = stamina.current,
                    momentum = momentum.amount,
                    cargo_loss_risk = momentum_effect.cargo_loss_risk,
                    "player moved"
                );
            } else {
                tracing::debug!(
                    outcome = ?outcome,
                    target_x = result.target.x,
                    target_y = result.target.y,
                    "player movement did not resolve"
                );
            }
        }
        PlayerAction::PickUp => {
            if pick_up_loose_parcel(entity, *position, &mut cargo, &mut parcels) {
                energy.spend(now, ITEM_ACTION_ENERGY_COST);
                tracing::info!(
                    x = position.x,
                    y = position.y,
                    cargo = cargo.current_weight,
                    "player picked up parcel"
                );
            } else {
                tracing::debug!(
                    x = position.x,
                    y = position.y,
                    "player pickup found no parcel"
                );
            }
        }
        PlayerAction::CycleMovementMode => {
            movement_state.cycle_mode();
            tracing::info!(
                mode = movement_state.mode.label(),
                "player movement mode changed"
            );
        }
        PlayerAction::OpenInventory => {}
        PlayerAction::Wait => {}
    }
}

pub fn reset_cargo_loss_risk(mut cargo_loss_risk: ResMut<CargoLossRisk>) {
    cargo_loss_risk.reset();
}

pub fn resolve_cargo_loss_risk(
    cargo_loss_risk: Res<CargoLossRisk>,
    mut player_query: Query<(Entity, &Position, &mut Cargo), With<Player>>,
    mut parcels: Query<(&mut Position, &mut ParcelState), (With<CargoParcel>, Without<Player>)>,
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

fn apply_energy_multiplier(base: u32, multiplier: f32) -> u32 {
    ((base as f32) * multiplier).round().max(1.0) as u32
}

fn carried_parcel_count(
    holder: Entity,
    parcels: &Query<(Entity, &Position, &CargoParcel, &mut ParcelState), Without<Player>>,
) -> usize {
    parcels
        .iter()
        .filter(|(_, _, _, state)| **state == ParcelState::CarriedBy(holder))
        .count()
}

fn pick_up_loose_parcel(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Terrain;
    use crate::resources::Direction;

    fn insert_player_action_resources(world: &mut World, action: PlayerAction) {
        world.insert_resource(PlayerIntent {
            action: Some(action),
        });
        world.insert_resource(EnergyTimeline::default());
        world.insert_resource(CargoLossRisk::default());
        world.insert_resource(GameScreen::Playing);
        world.insert_resource(InventoryMenuState::default());
    }

    fn spawn_test_parcel(world: &mut World, position: Position) {
        world.spawn((position, CargoParcel { weight: 5.0 }, ParcelState::Loose));
    }

    fn spawn_carried_test_parcel(world: &mut World, holder: Entity, position: Position) {
        world.spawn((
            position,
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
                Cargo {
                    current_weight: 0.0,
                    max_weight: 40.0,
                },
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
        for y in 0..map.height {
            for x in 0..(map.width - 1) {
                if map.terrain_at(x, y) == Some(terrain)
                    && map.terrain_at(x + 1, y) == Some(terrain)
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
        assert_eq!(map.terrain_at(start.x, start.y), Some(Terrain::Road));
        assert_eq!(map.terrain_at(target.x, target.y), Some(Terrain::Road));
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
        schedule.add_systems(player_actions);
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
        schedule.add_systems(player_actions);
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
        assert!(map.is_passable(start.x, start.y + 1));
        world.insert_resource(map);
        insert_player_action_resources(&mut world, PlayerAction::Move(Direction::South));
        let player = spawn_test_player(&mut world, start, 35.0);
        {
            let mut cargo = world
                .get_mut::<Cargo>(player)
                .expect("test player should have cargo");
            cargo.current_weight = 5.0;
        }
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
            )
                .chain(),
        );
        schedule.run(&mut world);

        let mut player_query = world.query_filtered::<(&Position, &Cargo), With<Player>>();
        let (player_position, cargo) = player_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert_eq!(*player_position, Position { x: 6, y: 7 });
        assert_eq!(cargo.current_weight, 0.0);
        let player_position = *player_position;

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
        let player = spawn_test_player(&mut world, Position { x: 2, y: 2 }, 35.0);
        {
            let mut cargo = world
                .get_mut::<Cargo>(player)
                .expect("test player should have cargo");
            cargo.current_weight = 5.0;
        }
        spawn_carried_test_parcel(&mut world, player, Position { x: 0, y: 0 });

        let mut schedule = Schedule::default();
        schedule.add_systems(resolve_cargo_loss_risk);
        schedule.run(&mut world);

        let cargo = world
            .get::<Cargo>(player)
            .expect("test player should have cargo");
        assert_eq!(cargo.current_weight, 0.0);

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
        schedule.add_systems(player_actions);
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
        schedule.add_systems(player_actions);
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
        schedule.add_systems(player_actions);
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
        schedule.add_systems(player_actions);
        schedule.run(&mut world);

        let mut energy_query = world.query_filtered::<&ActionEnergy, With<Player>>();
        let energy = energy_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert!(energy.ready_at > 0);

        let mut cargo_query = world.query_filtered::<&Cargo, With<Player>>();
        let cargo = cargo_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert_eq!(cargo.current_weight, 5.0);

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
        schedule.add_systems(player_actions);
        schedule.run(&mut world);

        let mut energy_query = world.query_filtered::<&ActionEnergy, With<Player>>();
        let energy = energy_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert_eq!(energy.ready_at, 0);
    }
}
