use bevy_ecs::prelude::*;

use crate::components::*;
use crate::energy::{
    ActionEnergy, DEFAULT_ACTION_ENERGY_COST, PICKUP_ENERGY_COST, WAIT_ENERGY_COST,
};
use crate::map::Map;
use crate::movement::{
    resolve_movement, CargoLoad, MovementOutcome, MovementRequest, StaminaBudget,
};
use crate::resources::{
    Direction, EnergyTimeline, GameScreen, MenuAction, MenuInputState, PauseMenuEntry,
    PauseMenuState, PlayerAction, PlayerIntent, SimulationClock,
};

type AgentJobItem<'a> = (
    Entity,
    &'a mut Position,
    &'a mut Velocity,
    &'a mut Cargo,
    &'a mut AssignedJob,
    &'a mut ActionEnergy,
);

const WAIT_STAMINA_RECOVERY: f32 = 3.0;

pub fn menu_navigation(
    input: Res<MenuInputState>,
    mut screen: ResMut<GameScreen>,
    mut pause_menu: ResMut<PauseMenuState>,
) {
    let Some(action) = input.action else {
        return;
    };

    match (*screen, action) {
        (GameScreen::Playing, MenuAction::Cancel) => {
            *screen = GameScreen::PauseMenu;
        }
        (GameScreen::PauseMenu, MenuAction::Cancel) => {
            *screen = GameScreen::Playing;
        }
        (GameScreen::OptionsMenu, MenuAction::Cancel) => {
            *screen = GameScreen::PauseMenu;
        }
        (GameScreen::PauseMenu, MenuAction::MoveSelectionUp) => {
            pause_menu.select_previous();
        }
        (GameScreen::PauseMenu, MenuAction::MoveSelectionDown) => {
            pause_menu.select_next();
        }
        (GameScreen::PauseMenu, MenuAction::Confirm) => match pause_menu.selected() {
            PauseMenuEntry::Resume => *screen = GameScreen::Playing,
            PauseMenuEntry::Options => *screen = GameScreen::OptionsMenu,
        },
        _ => {}
    }
}

pub fn advance_timeline_for_player_intent(world: &mut World) {
    if world.resource::<PlayerIntent>().action.is_none() {
        return;
    }

    if let Some(player_ready_at) = player_ready_at(world) {
        let now = world.resource::<EnergyTimeline>().now;
        if player_ready_at > now {
            world.resource_mut::<EnergyTimeline>().now = player_ready_at;
            catch_up_agents(world);
        }
    }

    if process_player_action(world) {
        world.resource_mut::<SimulationClock>().turn += 1;
        if let Some(player_ready_at) = player_ready_at(world) {
            world.resource_mut::<EnergyTimeline>().now = player_ready_at;
            catch_up_agents(world);
        }
    }
}

fn player_ready_at(world: &mut World) -> Option<u64> {
    let mut query = world.query_filtered::<&ActionEnergy, With<Player>>();
    query.iter(world).next().map(|energy| energy.ready_at)
}

pub fn player_actions(
    intent: Res<PlayerIntent>,
    timeline: Res<EnergyTimeline>,
    map: Res<Map>,
    mut player_query: Query<
        (
            Entity,
            &mut Position,
            &mut Velocity,
            &mut Stamina,
            &mut Cargo,
            &mut MovementState,
            &mut ActionEnergy,
        ),
        With<Player>,
    >,
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
        mut energy,
    )) = player_query.get_single_mut()
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

    if action == PlayerAction::Wait {
        stamina.current = (stamina.current + WAIT_STAMINA_RECOVERY).min(stamina.max);
        energy.spend(now, WAIT_ENERGY_COST);
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
                position.x = result.target.x;
                position.y = result.target.y;
                velocity.dx = result.actual_delta.0;
                velocity.dy = result.actual_delta.1;
                stamina.current = (stamina.current + result.stamina_delta).clamp(0.0, stamina.max);
                energy.spend(now, result.energy_cost);
            }
        }
        PlayerAction::PickUp => {
            if pick_up_loose_parcel(entity, *position, &mut cargo, &mut parcels) {
                energy.spend(now, PICKUP_ENERGY_COST);
            }
        }
        PlayerAction::ToggleSprint => {
            movement_state.toggle_sprint();
        }
        PlayerAction::Wait => {}
    }
}

fn process_player_action(world: &mut World) -> bool {
    if world.resource::<PlayerIntent>().action.is_none() {
        return false;
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(player_actions);
    schedule.run(world);

    let mut query = world.query_filtered::<&ActionEnergy, With<Player>>();
    query.iter(world).next().is_some_and(|energy| {
        energy.last_cost > 0 && energy.ready_at > world.resource::<EnergyTimeline>().now
    })
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

pub fn assign_agent_jobs(
    mut parcels: Query<(Entity, &mut ParcelState), With<CargoParcel>>,
    mut agents: Query<(Entity, &mut AssignedJob), With<Agent>>,
) {
    for (agent_entity, mut job) in &mut agents {
        if job.parcel.is_some() && job.phase != JobPhase::Done {
            continue;
        }

        if let Some((parcel_entity, mut state)) = parcels
            .iter_mut()
            .find(|(_, state)| matches!(**state, ParcelState::Loose))
        {
            *state = ParcelState::AssignedTo(agent_entity);
            job.parcel = Some(parcel_entity);
            job.phase = JobPhase::GoToParcel;
        } else {
            job.parcel = None;
            job.phase = JobPhase::FindParcel;
        }
    }
}

pub fn agent_jobs(
    map: Res<Map>,
    timeline: Res<EnergyTimeline>,
    mut clock: ResMut<SimulationClock>,
    mut agents: Query<AgentJobItem, With<Agent>>,
    mut parcels: Query<(&Position, &CargoParcel, &mut ParcelState), Without<Agent>>,
) {
    let map = &*map;
    let now = timeline.now;
    for (agent_entity, mut position, mut velocity, mut cargo, mut job, mut energy) in &mut agents {
        velocity.dx = 0;
        velocity.dy = 0;

        for _ in 0..128 {
            if !energy.is_ready(now) {
                break;
            }

            let Some(parcel_entity) = job.parcel else {
                break;
            };
            let Ok((parcel_position, parcel, mut parcel_state)) = parcels.get_mut(parcel_entity)
            else {
                job.phase = JobPhase::FindParcel;
                job.parcel = None;
                break;
            };

            match job.phase {
                JobPhase::FindParcel | JobPhase::Done => break,
                JobPhase::GoToParcel => {
                    if *parcel_state != ParcelState::AssignedTo(agent_entity) {
                        job.phase = JobPhase::FindParcel;
                        job.parcel = None;
                        break;
                    }

                    if position.x == parcel_position.x && position.y == parcel_position.y {
                        *parcel_state = ParcelState::CarriedBy(agent_entity);
                        cargo.current_weight += parcel.weight;
                        job.phase = JobPhase::GoToDepot;
                        energy.spend(now, PICKUP_ENERGY_COST);
                        continue;
                    }

                    if let Some(moved) = greedy_step(
                        map,
                        agent_entity,
                        &mut position,
                        cargo.current_weight,
                        cargo.max_weight,
                        *parcel_position,
                    ) {
                        velocity.dx = moved.actual_delta.0;
                        velocity.dy = moved.actual_delta.1;
                        energy.spend(now, moved.energy_cost);
                    } else {
                        energy.spend(now, DEFAULT_ACTION_ENERGY_COST);
                    }
                }
                JobPhase::GoToDepot => {
                    if position.x == map.depot.0 && position.y == map.depot.1 {
                        *parcel_state = ParcelState::Delivered;
                        cargo.current_weight = (cargo.current_weight - parcel.weight).max(0.0);
                        clock.delivered_parcels += 1;
                        job.phase = JobPhase::Done;
                        job.parcel = None;
                        energy.spend(now, PICKUP_ENERGY_COST);
                        continue;
                    }

                    if let Some(moved) = greedy_step(
                        map,
                        agent_entity,
                        &mut position,
                        cargo.current_weight,
                        cargo.max_weight,
                        Position {
                            x: map.depot.0,
                            y: map.depot.1,
                        },
                    ) {
                        velocity.dx = moved.actual_delta.0;
                        velocity.dy = moved.actual_delta.1;
                        energy.spend(now, moved.energy_cost);
                    } else {
                        energy.spend(now, DEFAULT_ACTION_ENERGY_COST);
                    }
                }
            }
        }
    }
}

fn catch_up_agents(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems((assign_agent_jobs, agent_jobs));
    schedule.run(world);
}

fn greedy_step(
    map: &Map,
    entity: Entity,
    position: &mut Position,
    current_weight: f32,
    max_weight: f32,
    target: Position,
) -> Option<crate::movement::MovementResult> {
    let dx = (target.x - position.x).signum();
    let dy = (target.y - position.y).signum();
    let candidates = if (target.x - position.x).abs() >= (target.y - position.y).abs() {
        [(dx, 0), (0, dy), (0, -dy), (-dx, 0)]
    } else {
        [(0, dy), (dx, 0), (-dx, 0), (0, -dy)]
    };

    for (step_x, step_y) in candidates {
        if step_x == 0 && step_y == 0 {
            continue;
        }
        let Some(direction) = direction_from_delta(step_x, step_y) else {
            continue;
        };
        let mut request = MovementRequest::walking(*position, direction);
        request.entity = Some(entity);
        request.cargo = CargoLoad {
            current_weight,
            max_weight,
        };

        let outcome = resolve_movement(map, request);
        if let Some(result) = outcome.moved() {
            position.x = result.target.x;
            position.y = result.target.y;
            return Some(result);
        }
    }

    None
}

fn direction_from_delta(dx: i32, dy: i32) -> Option<Direction> {
    match (dx, dy) {
        (-1, 0) => Some(Direction::West),
        (1, 0) => Some(Direction::East),
        (0, -1) => Some(Direction::North),
        (0, 1) => Some(Direction::South),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Terrain;
    use crate::resources::Direction;

    fn spawn_test_agent(world: &mut World, id: usize, position: Position) {
        world.spawn((
            Agent { id },
            position,
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
        ));
    }

    fn spawn_test_parcel(world: &mut World, position: Position) {
        world.spawn((position, CargoParcel { weight: 5.0 }, ParcelState::Loose));
    }

    fn spawn_test_player(world: &mut World, position: Position, stamina: f32) {
        world.spawn((
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
            ActionEnergy::default(),
        ));
    }

    fn run_menu_action(world: &mut World, screen: GameScreen, action: MenuAction) {
        world.insert_resource(screen);
        world.insert_resource(PauseMenuState::default());
        world.insert_resource(MenuInputState {
            action: Some(action),
        });

        let mut schedule = Schedule::default();
        schedule.add_systems(menu_navigation);
        schedule.run(world);
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
        world.insert_resource(PlayerIntent {
            action: Some(PlayerAction::Move(direction)),
        });
        world.insert_resource(EnergyTimeline::default());
        spawn_test_player(world, start, stamina);

        let mut schedule = Schedule::default();
        schedule.add_systems(player_actions);
        schedule.run(world);
    }

    #[test]
    fn failed_player_movement_does_not_consume_turn() {
        let mut world = World::new();
        world.insert_resource(Map::generate());
        world.insert_resource(PlayerIntent {
            action: Some(PlayerAction::Move(Direction::West)),
        });
        world.insert_resource(EnergyTimeline::default());
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

        let mut player_query = world.query_filtered::<&Position, With<Player>>();
        let position = player_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert_eq!(*position, Position { x: 0, y: 0 });
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
    fn wait_consumes_turn_and_recovers_stamina() {
        let mut world = World::new();
        world.insert_resource(Map::generate());
        world.insert_resource(PlayerIntent {
            action: Some(PlayerAction::Wait),
        });
        world.insert_resource(EnergyTimeline::default());
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
    }

    #[test]
    fn toggling_sprint_changes_movement_mode_without_consuming_turn() {
        let mut world = World::new();
        world.insert_resource(Map::generate());
        world.insert_resource(PlayerIntent {
            action: Some(PlayerAction::ToggleSprint),
        });
        world.insert_resource(EnergyTimeline::default());
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
    fn player_can_pick_up_loose_parcel_on_same_tile() {
        let mut world = World::new();
        world.insert_resource(Map::generate());
        world.insert_resource(PlayerIntent {
            action: Some(PlayerAction::PickUp),
        });
        world.insert_resource(EnergyTimeline::default());
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
        world.insert_resource(PlayerIntent {
            action: Some(PlayerAction::PickUp),
        });
        world.insert_resource(EnergyTimeline::default());
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

    #[test]
    fn escape_opens_and_closes_pause_menu() {
        let mut world = World::new();

        run_menu_action(&mut world, GameScreen::Playing, MenuAction::Cancel);
        assert_eq!(*world.resource::<GameScreen>(), GameScreen::PauseMenu);

        world.insert_resource(MenuInputState {
            action: Some(MenuAction::Cancel),
        });
        let mut schedule = Schedule::default();
        schedule.add_systems(menu_navigation);
        schedule.run(&mut world);

        assert_eq!(*world.resource::<GameScreen>(), GameScreen::Playing);
    }

    #[test]
    fn pause_menu_confirm_can_open_options() {
        let mut world = World::new();

        run_menu_action(
            &mut world,
            GameScreen::PauseMenu,
            MenuAction::MoveSelectionDown,
        );
        assert_eq!(
            world.resource::<PauseMenuState>().selected(),
            PauseMenuEntry::Options
        );

        world.insert_resource(MenuInputState {
            action: Some(MenuAction::Confirm),
        });
        let mut schedule = Schedule::default();
        schedule.add_systems(menu_navigation);
        schedule.run(&mut world);

        assert_eq!(*world.resource::<GameScreen>(), GameScreen::OptionsMenu);
    }

    #[test]
    fn agents_reserve_distinct_loose_parcels() {
        let mut world = World::new();
        spawn_test_agent(&mut world, 0, Position { x: 0, y: 0 });
        spawn_test_agent(&mut world, 1, Position { x: 1, y: 0 });
        spawn_test_parcel(&mut world, Position { x: 2, y: 0 });
        spawn_test_parcel(&mut world, Position { x: 3, y: 0 });

        let mut schedule = Schedule::default();
        schedule.add_systems(assign_agent_jobs);
        schedule.run(&mut world);

        let mut job_query = world.query::<&AssignedJob>();
        let assigned_jobs = job_query
            .iter(&world)
            .filter(|job| matches!(job.phase, JobPhase::GoToParcel) && job.parcel.is_some())
            .count();
        assert_eq!(assigned_jobs, 2);

        let mut parcel_query = world.query::<&ParcelState>();
        let reserved_parcels = parcel_query
            .iter(&world)
            .filter(|state| matches!(state, ParcelState::AssignedTo(_)))
            .count();
        assert_eq!(reserved_parcels, 2);
    }

    #[test]
    fn agent_picks_up_and_delivers_parcel_to_depot() {
        let mut world = World::new();
        let map = Map::generate();
        let depot = map.depot;
        world.insert_resource(map);
        world.insert_resource(SimulationClock {
            turn: 0,
            delivered_parcels: 0,
        });
        world.insert_resource(EnergyTimeline::default());
        spawn_test_agent(
            &mut world,
            0,
            Position {
                x: depot.0,
                y: depot.1,
            },
        );
        spawn_test_parcel(
            &mut world,
            Position {
                x: depot.0,
                y: depot.1,
            },
        );

        let mut schedule = Schedule::default();
        schedule.add_systems((assign_agent_jobs, agent_jobs));
        for _ in 0..12 {
            schedule.run(&mut world);
            world.resource_mut::<EnergyTimeline>().now += 100;
        }

        let clock = world.resource::<SimulationClock>();
        assert_eq!(clock.delivered_parcels, 1);

        let mut parcel_query = world.query::<&ParcelState>();
        let delivered_parcels = parcel_query
            .iter(&world)
            .filter(|state| matches!(state, ParcelState::Delivered))
            .count();
        assert_eq!(delivered_parcels, 1);

        let mut cargo_query = world.query_filtered::<&Cargo, With<Agent>>();
        let empty_agents = cargo_query
            .iter(&world)
            .filter(|cargo| cargo.current_weight == 0.0)
            .count();
        assert_eq!(empty_agents, 1);
    }
}
