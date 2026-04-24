use bevy_ecs::prelude::Entity;

use crate::components::Position;
use crate::map::{Map, Terrain};
use crate::resources::Direction;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MovementMode {
    Walking,
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
    pub stamina_delta: f32,
    pub turn_cost: u32,
    pub cooldown_cost: u32,
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
        Self {
            entity: None,
            origin,
            direction,
            mode: MovementMode::Walking,
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
    let terrain = map.terrain_at(target.x, target.y);
    let mut result = MovementResult {
        entity: request.entity,
        mode: request.mode,
        requested_delta,
        actual_delta: (0, 0),
        origin: request.origin,
        target,
        terrain,
        stamina_delta: 0.0,
        turn_cost: 0,
        cooldown_cost: 0,
    };

    let Some(terrain) = terrain else {
        return MovementOutcome::Blocked(result);
    };
    if !map.is_passable(target.x, target.y) {
        return MovementOutcome::Blocked(result);
    }

    result.stamina_delta = stamina_delta_for(terrain, request.cargo);
    if let Some(stamina) = request.stamina {
        let available_stamina = stamina.current.clamp(0.0, stamina.max);
        if result.stamina_delta.is_sign_negative() && available_stamina < result.stamina_delta.abs()
        {
            return MovementOutcome::InsufficientStamina(result);
        }
    }

    result.actual_delta = requested_delta;
    result.turn_cost = 1;
    result.cooldown_cost = terrain_cooldown_cost(terrain);
    MovementOutcome::Moved(result)
}

pub fn terrain_cooldown_cost(terrain: Terrain) -> u32 {
    (6.0 + terrain.movement_cost() * 5.0) as u32
}

fn stamina_delta_for(terrain: Terrain, cargo: CargoLoad) -> f32 {
    let terrain_delta = terrain.stamina_delta();
    if terrain_delta.is_sign_negative() {
        terrain_delta * cargo_load_factor(cargo)
    } else {
        terrain_delta
    }
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

    fn find_target_terrain(map: &Map, terrain: Terrain) -> (Position, Direction) {
        for y in 0..map.height {
            for x in 0..map.width {
                if map.terrain_at(x, y) != Some(terrain) {
                    continue;
                }

                for (direction, start) in [
                    (Direction::East, Position { x: x - 1, y }),
                    (Direction::West, Position { x: x + 1, y }),
                    (Direction::South, Position { x, y: y - 1 }),
                    (Direction::North, Position { x, y: y + 1 }),
                ] {
                    if map.in_bounds(start.x, start.y) {
                        return (start, direction);
                    }
                }
            }
        }
        panic!("test map should contain reachable {terrain:?}");
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
        let map = Map::generate();
        let (start, direction) = find_target_terrain(&map, Terrain::Water);

        let outcome = resolve_movement(&map, request_from(start, direction));

        let result = outcome.result();
        assert!(matches!(outcome, MovementOutcome::Blocked(_)));
        assert_eq!(result.terrain, Some(Terrain::Water));
        assert_eq!(result.actual_delta, (0, 0));
    }

    #[test]
    fn grass_is_stamina_neutral() {
        let map = Map::generate();
        let (start, direction) = find_target_terrain(&map, Terrain::Grass);

        let outcome = resolve_movement(&map, request_from(start, direction));

        let result = outcome.moved().expect("grass should be passable");
        assert_eq!(result.terrain, Some(Terrain::Grass));
        assert_eq!(result.stamina_delta, 0.0);
        assert_eq!(result.turn_cost, 1);
    }

    #[test]
    fn road_restores_stamina() {
        let map = Map::generate();
        let (start, direction) = find_target_terrain(&map, Terrain::Road);

        let outcome = resolve_movement(&map, request_from(start, direction));

        let result = outcome.moved().expect("road should be passable");
        assert_eq!(result.terrain, Some(Terrain::Road));
        assert!(result.stamina_delta > 0.0);
    }

    #[test]
    fn mud_and_rock_drain_stamina() {
        let map = Map::generate();

        for terrain in [Terrain::Mud, Terrain::Rock] {
            let (start, direction) = find_target_terrain(&map, terrain);
            let outcome = resolve_movement(&map, request_from(start, direction));
            let result = outcome.moved().expect("rough terrain should be passable");

            assert_eq!(result.terrain, Some(terrain));
            assert!(result.stamina_delta < 0.0);
        }
    }

    #[test]
    fn cargo_load_increases_negative_stamina_costs() {
        let map = Map::generate();
        let (start, direction) = find_target_terrain(&map, Terrain::Mud);
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
    fn insufficient_stamina_blocks_draining_terrain() {
        let map = Map::generate();
        let (start, direction) = find_target_terrain(&map, Terrain::Rock);

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
    }
}
