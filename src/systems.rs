use bevy_ecs::prelude::*;

use crate::components::*;
use crate::map::Map;
use crate::movement::{
    resolve_movement, terrain_cooldown_cost, CargoLoad, MovementOutcome, MovementRequest,
    StaminaBudget,
};
use crate::resources::{
    Direction, GameScreen, MenuAction, MenuInputState, PauseMenuEntry, PauseMenuState,
    PlayerAction, PlayerIntent, SimulationClock, TurnState,
};

type AgentJobItem<'a> = (
    Entity,
    &'a mut Position,
    &'a mut Velocity,
    &'a mut Cargo,
    &'a mut AssignedJob,
    &'a mut StepCooldown,
);

const WAIT_STAMINA_RECOVERY: f32 = 3.0;

pub fn tick_clock(mut clock: ResMut<SimulationClock>) {
    clock.turn += 1;
}

pub fn tick_cooldowns(mut query: Query<&mut StepCooldown>) {
    for mut cooldown in &mut query {
        cooldown.frames = cooldown.frames.saturating_sub(1);
    }
}

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

pub fn player_movement(
    intent: Res<PlayerIntent>,
    map: Res<Map>,
    mut turn: ResMut<TurnState>,
    mut query: Query<(Entity, &mut Position, &mut Velocity, &mut Stamina, &Cargo), With<Player>>,
) {
    turn.consumed = false;

    let map = &*map;
    let Ok((entity, mut position, mut velocity, mut stamina, cargo)) = query.get_single_mut()
    else {
        return;
    };

    velocity.dx = 0;
    velocity.dy = 0;

    let Some(action) = intent.action else {
        return;
    };

    if action == PlayerAction::Wait {
        stamina.current = (stamina.current + WAIT_STAMINA_RECOVERY).min(stamina.max);
        turn.consumed = true;
        return;
    }

    let PlayerAction::Move(direction) = action else {
        return;
    };
    let mut request = MovementRequest::walking(*position, direction);
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
        turn.consumed = result.turn_cost > 0;
    }
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
    mut clock: ResMut<SimulationClock>,
    mut agents: Query<AgentJobItem, With<Agent>>,
    mut parcels: Query<(&Position, &CargoParcel, &mut ParcelState), Without<Agent>>,
) {
    let map = &*map;
    for (agent_entity, mut position, mut velocity, mut cargo, mut job, mut cooldown) in &mut agents
    {
        velocity.dx = 0;
        velocity.dy = 0;

        if cooldown.frames > 0 {
            continue;
        }

        let Some(parcel_entity) = job.parcel else {
            continue;
        };
        let Ok((parcel_position, parcel, mut parcel_state)) = parcels.get_mut(parcel_entity) else {
            job.phase = JobPhase::FindParcel;
            job.parcel = None;
            continue;
        };

        match job.phase {
            JobPhase::FindParcel | JobPhase::Done => {}
            JobPhase::GoToParcel => {
                if *parcel_state != ParcelState::AssignedTo(agent_entity) {
                    job.phase = JobPhase::FindParcel;
                    job.parcel = None;
                    continue;
                }

                if position.x == parcel_position.x && position.y == parcel_position.y {
                    *parcel_state = ParcelState::CarriedBy(agent_entity);
                    cargo.current_weight += parcel.weight;
                    job.phase = JobPhase::GoToDepot;
                    cooldown.frames = 10;
                    continue;
                }

                if let Some(moved) = greedy_step(
                    map,
                    agent_entity,
                    &mut position,
                    cargo.current_weight,
                    cargo.max_weight,
                    parcel_position.x,
                    parcel_position.y,
                ) {
                    velocity.dx = moved.actual_delta.0;
                    velocity.dy = moved.actual_delta.1;
                    cooldown.frames = moved.cooldown_cost;
                } else {
                    cooldown.frames = step_delay(map, position.x, position.y);
                }
            }
            JobPhase::GoToDepot => {
                if position.x == map.depot.0 && position.y == map.depot.1 {
                    *parcel_state = ParcelState::Delivered;
                    cargo.current_weight = (cargo.current_weight - parcel.weight).max(0.0);
                    clock.delivered_parcels += 1;
                    job.phase = JobPhase::Done;
                    job.parcel = None;
                    cooldown.frames = 18;
                    continue;
                }

                if let Some(moved) = greedy_step(
                    map,
                    agent_entity,
                    &mut position,
                    cargo.current_weight,
                    cargo.max_weight,
                    map.depot.0,
                    map.depot.1,
                ) {
                    velocity.dx = moved.actual_delta.0;
                    velocity.dy = moved.actual_delta.1;
                    cooldown.frames = moved.cooldown_cost;
                } else {
                    cooldown.frames = step_delay(map, position.x, position.y);
                }
            }
        }
    }
}

fn greedy_step(
    map: &Map,
    entity: Entity,
    position: &mut Position,
    current_weight: f32,
    max_weight: f32,
    target_x: i32,
    target_y: i32,
) -> Option<crate::movement::MovementResult> {
    let dx = (target_x - position.x).signum();
    let dy = (target_y - position.y).signum();
    let candidates = if (target_x - position.x).abs() >= (target_y - position.y).abs() {
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

fn step_delay(map: &Map, x: i32, y: i32) -> u32 {
    map.terrain_at(x, y).map_or(11, terrain_cooldown_cost)
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
            StepCooldown::default(),
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
        world.insert_resource(TurnState::default());
        spawn_test_player(world, start, stamina);

        let mut schedule = Schedule::default();
        schedule.add_systems(player_movement);
        schedule.run(world);
    }

    #[test]
    fn failed_player_movement_does_not_consume_turn() {
        let mut world = World::new();
        world.insert_resource(Map::generate());
        world.insert_resource(PlayerIntent {
            action: Some(PlayerAction::Move(Direction::West)),
        });
        world.insert_resource(TurnState { consumed: true });
        spawn_test_player(&mut world, Position { x: 0, y: 0 }, 35.0);

        let mut schedule = Schedule::default();
        schedule.add_systems(player_movement);
        schedule.run(&mut world);

        let turn = world.resource::<TurnState>();
        assert!(!turn.consumed);

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

        let turn = world.resource::<TurnState>();
        assert!(turn.consumed);

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

        let turn = world.resource::<TurnState>();
        assert!(turn.consumed);

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
        world.insert_resource(TurnState::default());
        spawn_test_player(&mut world, Position { x: 0, y: 0 }, 10.0);

        let mut schedule = Schedule::default();
        schedule.add_systems(player_movement);
        schedule.run(&mut world);

        let turn = world.resource::<TurnState>();
        assert!(turn.consumed);

        let mut player_query = world.query_filtered::<&Stamina, With<Player>>();
        let stamina = player_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert!(stamina.current > 10.0);
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
        schedule.add_systems((tick_cooldowns, assign_agent_jobs, agent_jobs));
        for _ in 0..12 {
            schedule.run(&mut world);
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
