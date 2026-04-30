use bevy_ecs::prelude::Entity;

use crate::components::Position;
use crate::energy::movement_energy_cost;
use crate::map::{Map, Terrain, TileCoord};
use crate::resources::Direction;

pub const MAX_WALKABLE_UP_STEP: i16 = 2;
pub const MAX_WALKABLE_DOWN_STEP: i16 = 3;
pub const SHALLOW_WATER_MAX_DEPTH: u8 = 1;
const UPHILL_STAMINA_COST_PER_LEVEL: f32 = 1.25;
const DOWNHILL_RISK_STAMINA_COST_PER_LEVEL: f32 = 0.5;
const SLOPE_ENERGY_COST_PER_LEVEL: f32 = 0.15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MovementMode {
    Walking,
    Sprinting,
    Steady,
}

impl MovementMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Walking => "walking",
            Self::Sprinting => "sprinting",
            Self::Steady => "steady",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MovementRequest {
    pub entity: Option<Entity>,
    pub origin: Position,
    pub direction: Direction,
    pub mode: MovementMode,
    pub stamina: Option<StaminaBudget>,
    pub cargo: CargoLoad,
}

#[derive(Clone, Copy, Debug)]
pub struct StaminaBudget {
    pub current: f32,
    pub max: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CargoLoad {
    pub current_weight: f32,
    pub max_weight: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MovementOutcome {
    Moved(MovementResult),
    Blocked(MovementResult),
    InsufficientStamina(MovementResult),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovementResult {
    pub entity: Option<Entity>,
    pub mode: MovementMode,
    pub requested_delta: (i32, i32),
    pub actual_delta: (i32, i32),
    pub origin: Position,
    pub target: Position,
    pub terrain: Option<Terrain>,
    pub origin_elevation: Option<i16>,
    pub target_elevation: Option<i16>,
    pub elevation_delta: i16,
    pub water_depth: Option<u8>,
    pub stamina_delta: f32,
    pub turn_cost: u32,
    pub cooldown_cost: u32,
    pub energy_cost: u32,
}

impl MovementOutcome {
    pub fn result(self) -> MovementResult {
        match self {
            Self::Moved(result) | Self::Blocked(result) | Self::InsufficientStamina(result) => {
                result
            }
        }
    }

    pub fn moved(self) -> Option<MovementResult> {
        match self {
            Self::Moved(result) => Some(result),
            Self::Blocked(_) | Self::InsufficientStamina(_) => None,
        }
    }
}

impl MovementRequest {
    pub fn walking(origin: Position, direction: Direction) -> Self {
        Self::new(origin, direction, MovementMode::Walking)
    }

    pub fn new(origin: Position, direction: Direction, mode: MovementMode) -> Self {
        Self {
            entity: None,
            origin,
            direction,
            mode,
            stamina: None,
            cargo: CargoLoad::default(),
        }
    }
}

pub fn resolve_movement(map: &Map, request: MovementRequest) -> MovementOutcome {
    let requested_delta = request.direction.delta();
    let target = Position {
        x: request.origin.x + requested_delta.0,
        y: request.origin.y + requested_delta.1,
    };
    let origin_coord = TileCoord::new(request.origin.x, request.origin.y);
    let target_coord = TileCoord::new(target.x, target.y);
    let terrain = map.terrain_at_coord(target_coord);
    let mut result = MovementResult {
        entity: request.entity,
        mode: request.mode,
        requested_delta,
        actual_delta: (0, 0),
        origin: request.origin,
        target,
        terrain,
        origin_elevation: map.elevation_at_coord(origin_coord),
        target_elevation: map.elevation_at_coord(target_coord),
        elevation_delta: 0,
        water_depth: map.water_depth_at_coord(target_coord),
        stamina_delta: 0.0,
        turn_cost: 0,
        cooldown_cost: 0,
        energy_cost: 0,
    };

    let Some(edge) = map.movement_edge(origin_coord, target_coord) else {
        return MovementOutcome::Blocked(result);
    };
    let terrain = edge.target.terrain;
    result.terrain = Some(terrain);
    result.origin_elevation = Some(edge.origin.elevation);
    result.target_elevation = Some(edge.target.elevation);
    result.elevation_delta = edge.elevation_delta;
    result.water_depth = Some(edge.target.water_depth);

    if !edge_is_walkable(edge) {
        return MovementOutcome::Blocked(result);
    }

    result.stamina_delta =
        stamina_delta_for(terrain, request.mode, request.cargo) + slope_stamina_delta(edge);
    if let Some(stamina) = request.stamina {
        let available_stamina = stamina.current.clamp(0.0, stamina.max);
        if result.stamina_delta.is_sign_negative() && available_stamina < result.stamina_delta.abs()
        {
            return MovementOutcome::InsufficientStamina(result);
        }
    }

    result.actual_delta = requested_delta;
    result.turn_cost = 1;
    result.cooldown_cost = movement_cooldown_cost(terrain, request.mode);
    result.energy_cost =
        slope_adjusted_energy_cost(movement_energy_cost(terrain, request.mode), edge);
    MovementOutcome::Moved(result)
}

fn edge_is_walkable(edge: crate::map::MovementEdge) -> bool {
    if edge.target.terrain == Terrain::Water {
        edge.target.water_depth <= SHALLOW_WATER_MAX_DEPTH
    } else if !edge.target.terrain.definition().passable {
        false
    } else {
        edge.elevation_delta <= MAX_WALKABLE_UP_STEP
            && edge.elevation_delta >= -MAX_WALKABLE_DOWN_STEP
    }
}

pub fn terrain_cooldown_cost(terrain: Terrain) -> u32 {
    (6.0 + terrain.definition().movement_cost * 5.0) as u32
}

fn movement_cooldown_cost(terrain: Terrain, mode: MovementMode) -> u32 {
    let terrain_cost = terrain_cooldown_cost(terrain);
    match mode {
        MovementMode::Walking => terrain_cost,
        MovementMode::Sprinting => ((terrain_cost as f32) * 0.65).max(1.0) as u32,
        MovementMode::Steady => ((terrain_cost as f32) * 1.35).max(1.0) as u32,
    }
}

fn stamina_delta_for(terrain: Terrain, mode: MovementMode, cargo: CargoLoad) -> f32 {
    let terrain_delta = terrain.definition().stamina_delta;
    match mode {
        MovementMode::Walking => {
            if terrain_delta.is_sign_negative() {
                terrain_delta * cargo_load_factor(cargo)
            } else {
                terrain_delta
            }
        }
        MovementMode::Sprinting => {
            let movement_cost = if terrain_delta.is_sign_negative() {
                terrain_delta
            } else {
                -1.0
            };
            movement_cost * 2.0 * cargo_load_factor(cargo)
        }
        MovementMode::Steady => {
            if terrain_delta.is_sign_negative() {
                terrain_delta * 0.5 * cargo_load_factor(cargo)
            } else {
                terrain_delta
            }
        }
    }
}

fn slope_stamina_delta(edge: crate::map::MovementEdge) -> f32 {
    if edge.elevation_delta > 0 {
        -(f32::from(edge.elevation_delta) * UPHILL_STAMINA_COST_PER_LEVEL)
    } else if edge.elevation_delta < -1 {
        -((f32::from(edge.elevation_delta.abs()) - 1.0) * DOWNHILL_RISK_STAMINA_COST_PER_LEVEL)
    } else {
        0.0
    }
}

fn slope_adjusted_energy_cost(base: u32, edge: crate::map::MovementEdge) -> u32 {
    let multiplier = 1.0 + f32::from(edge.elevation_delta.abs()) * SLOPE_ENERGY_COST_PER_LEVEL;
    ((base as f32) * multiplier).round().max(1.0) as u32
}

fn cargo_load_factor(cargo: CargoLoad) -> f32 {
    1.0 + cargo.current_weight / cargo.max_weight.max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Terrain;

    fn request_from(start: Position, direction: Direction) -> MovementRequest {
        MovementRequest {
            stamina: Some(StaminaBudget {
                current: 35.0,
                max: 35.0,
            }),
            ..MovementRequest::walking(start, direction)
        }
    }

    fn map_with_target_terrain(terrain: Terrain) -> Map {
        let mut map = Map::flat_for_tests(4, 4, Terrain::Grass, 4);
        map.set_for_tests(1, 0, terrain);
        map
    }

    #[test]
    fn bounds_block_movement() {
        let map = Map::generate();

        let outcome =
            resolve_movement(&map, request_from(Position { x: 0, y: 0 }, Direction::West));

        let result = outcome.result();
        assert!(matches!(outcome, MovementOutcome::Blocked(_)));
        assert_eq!(result.actual_delta, (0, 0));
        assert_eq!(result.terrain, None);
        assert_eq!(result.turn_cost, 0);
    }

    #[test]
    fn water_blocks_movement() {
        let mut map = map_with_target_terrain(Terrain::Water);
        map.set_water_depth_for_tests(1, 0, 3);

        let outcome =
            resolve_movement(&map, request_from(Position { x: 0, y: 0 }, Direction::East));

        let result = outcome.result();
        assert!(matches!(outcome, MovementOutcome::Blocked(_)));
        assert_eq!(result.terrain, Some(Terrain::Water));
        assert_eq!(result.actual_delta, (0, 0));
    }

    #[test]
    fn grass_is_stamina_neutral() {
        let map = map_with_target_terrain(Terrain::Grass);

        let outcome =
            resolve_movement(&map, request_from(Position { x: 0, y: 0 }, Direction::East));

        let result = outcome.moved().expect("grass should be passable");
        assert_eq!(result.terrain, Some(Terrain::Grass));
        assert_eq!(result.stamina_delta, 0.0);
        assert_eq!(result.turn_cost, 1);
    }

    #[test]
    fn road_restores_stamina() {
        let map = map_with_target_terrain(Terrain::Road);

        let outcome =
            resolve_movement(&map, request_from(Position { x: 0, y: 0 }, Direction::East));

        let result = outcome.moved().expect("road should be passable");
        assert_eq!(result.terrain, Some(Terrain::Road));
        assert!(result.stamina_delta > 0.0);
    }

    #[test]
    fn mud_and_rock_drain_stamina() {
        for terrain in [Terrain::Mud, Terrain::Rock] {
            let map = map_with_target_terrain(terrain);
            let outcome =
                resolve_movement(&map, request_from(Position { x: 0, y: 0 }, Direction::East));
            let result = outcome.moved().expect("rough terrain should be passable");

            assert_eq!(result.terrain, Some(terrain));
            assert!(result.stamina_delta < 0.0);
        }
    }

    #[test]
    fn cargo_load_increases_negative_stamina_costs() {
        let map = map_with_target_terrain(Terrain::Mud);
        let start = Position { x: 0, y: 0 };
        let direction = Direction::East;
        let unloaded = resolve_movement(&map, request_from(start, direction))
            .moved()
            .expect("mud should be passable");

        let loaded = resolve_movement(
            &map,
            MovementRequest {
                cargo: CargoLoad {
                    current_weight: 30.0,
                    max_weight: 30.0,
                },
                ..request_from(start, direction)
            },
        )
        .moved()
        .expect("mud should be passable");

        assert!(loaded.stamina_delta < unloaded.stamina_delta);
    }

    #[test]
    fn sprinting_costs_stamina_and_reduces_cooldown() {
        let map = map_with_target_terrain(Terrain::Grass);
        let start = Position { x: 0, y: 0 };
        let direction = Direction::East;

        let walking = resolve_movement(&map, request_from(start, direction))
            .moved()
            .expect("grass should be passable");

        let sprinting = resolve_movement(
            &map,
            MovementRequest {
                mode: MovementMode::Sprinting,
                ..request_from(start, direction)
            },
        )
        .moved()
        .expect("grass should be passable");

        assert_eq!(walking.stamina_delta, 0.0);
        assert!(sprinting.stamina_delta < walking.stamina_delta);
        assert!(sprinting.cooldown_cost < walking.cooldown_cost);
        assert!(sprinting.energy_cost < walking.energy_cost);
    }

    #[test]
    fn steady_movement_costs_more_energy_and_less_stamina_on_rough_ground() {
        let map = map_with_target_terrain(Terrain::Mud);
        let start = Position { x: 0, y: 0 };
        let direction = Direction::East;

        let walking = resolve_movement(&map, request_from(start, direction))
            .moved()
            .expect("mud should be passable");

        let steady = resolve_movement(
            &map,
            MovementRequest {
                mode: MovementMode::Steady,
                ..request_from(start, direction)
            },
        )
        .moved()
        .expect("mud should be passable");

        assert!(steady.stamina_delta > walking.stamina_delta);
        assert!(steady.cooldown_cost > walking.cooldown_cost);
        assert!(steady.energy_cost > walking.energy_cost);
    }

    #[test]
    fn insufficient_stamina_blocks_draining_terrain() {
        let map = map_with_target_terrain(Terrain::Rock);
        let start = Position { x: 0, y: 0 };
        let direction = Direction::East;

        let outcome = resolve_movement(
            &map,
            MovementRequest {
                stamina: Some(StaminaBudget {
                    current: 1.0,
                    max: 35.0,
                }),
                ..MovementRequest::walking(start, direction)
            },
        );

        assert!(matches!(outcome, MovementOutcome::InsufficientStamina(_)));
        assert_eq!(outcome.result().turn_cost, 0);
        assert_eq!(outcome.result().energy_cost, 0);
    }

    #[test]
    fn walkable_uphill_succeeds_and_costs_more_than_flat_ground() {
        let flat = map_with_target_terrain(Terrain::Grass);
        let mut uphill = map_with_target_terrain(Terrain::Grass);
        uphill.set_elevation_for_tests(1, 0, 6);
        let start = Position { x: 0, y: 0 };

        let flat_result = resolve_movement(&flat, request_from(start, Direction::East))
            .moved()
            .expect("flat grass should be passable");
        let uphill_result = resolve_movement(&uphill, request_from(start, Direction::East))
            .moved()
            .expect("small uphill step should be passable");

        assert_eq!(uphill_result.elevation_delta, 2);
        assert!(uphill_result.stamina_delta < flat_result.stamina_delta);
        assert!(uphill_result.energy_cost > flat_result.energy_cost);
    }

    #[test]
    fn steep_uphill_blocks_without_spending_energy() {
        let mut map = map_with_target_terrain(Terrain::Grass);
        map.set_elevation_for_tests(1, 0, 7);

        let outcome =
            resolve_movement(&map, request_from(Position { x: 0, y: 0 }, Direction::East));

        assert!(matches!(outcome, MovementOutcome::Blocked(_)));
        assert_eq!(outcome.result().elevation_delta, 3);
        assert_eq!(outcome.result().energy_cost, 0);
    }

    #[test]
    fn walkable_downhill_succeeds() {
        let mut map = map_with_target_terrain(Terrain::Grass);
        map.set_elevation_for_tests(0, 0, 7);
        map.set_elevation_for_tests(1, 0, 4);

        let outcome =
            resolve_movement(&map, request_from(Position { x: 0, y: 0 }, Direction::East));

        let result = outcome
            .moved()
            .expect("moderate downhill should be passable");
        assert_eq!(result.elevation_delta, -3);
    }

    #[test]
    fn steep_downhill_blocks_for_now() {
        let mut map = map_with_target_terrain(Terrain::Grass);
        map.set_elevation_for_tests(0, 0, 8);
        map.set_elevation_for_tests(1, 0, 4);

        let outcome =
            resolve_movement(&map, request_from(Position { x: 0, y: 0 }, Direction::East));

        assert!(matches!(outcome, MovementOutcome::Blocked(_)));
        assert_eq!(outcome.result().elevation_delta, -4);
        assert_eq!(outcome.result().energy_cost, 0);
    }

    #[test]
    fn shallow_water_is_walkable_but_deep_water_blocks() {
        let mut shallow = map_with_target_terrain(Terrain::Water);
        shallow.set_water_depth_for_tests(1, 0, 1);
        let mut deep = map_with_target_terrain(Terrain::Water);
        deep.set_water_depth_for_tests(1, 0, 2);
        let start = Position { x: 0, y: 0 };

        assert!(
            resolve_movement(&shallow, request_from(start, Direction::East))
                .moved()
                .is_some()
        );
        let outcome = resolve_movement(&deep, request_from(start, Direction::East));
        assert!(matches!(outcome, MovementOutcome::Blocked(_)));
        assert_eq!(outcome.result().energy_cost, 0);
    }
}
