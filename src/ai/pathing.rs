use bevy_ecs::prelude::Entity;
use pathfinding::prelude::astar;

use crate::components::Position;
use crate::map::{Map, TileCoord};
use crate::movement::{resolve_movement, CargoLoad, MovementRequest};
use crate::resources::Direction;

const CARDINAL_DIRECTIONS: [Direction; 4] = [
    Direction::West,
    Direction::East,
    Direction::North,
    Direction::South,
];

/// Returns the first movement direction on a shortest currently-walkable path.
///
/// Pathfinding asks the shared movement resolver for valid edges instead of
/// reading only terrain passability. That keeps agents planning with the same
/// slope, water, and cargo rules that are used when movement is applied.
pub fn first_step_toward(
    map: &Map,
    entity: Entity,
    origin: Position,
    target: Position,
    cargo: CargoLoad,
) -> Option<Direction> {
    let start = TileCoord::from(origin);
    let goal = TileCoord::from(target);
    if start == goal {
        return None;
    }

    let (path, _) = astar(
        &start,
        |coord| movement_successors(map, entity, *coord, cargo),
        |coord| manhattan_distance(*coord, goal),
        |coord| *coord == goal,
    )?;
    path.get(1).and_then(|next| direction_between(start, *next))
}

fn movement_successors(
    map: &Map,
    entity: Entity,
    coord: TileCoord,
    cargo: CargoLoad,
) -> Vec<(TileCoord, u32)> {
    CARDINAL_DIRECTIONS
        .into_iter()
        .filter_map(|direction| {
            let mut request = MovementRequest::walking(Position::from(coord), direction);
            request.entity = Some(entity);
            request.cargo = cargo;

            let result = resolve_movement(map, request).moved()?;
            let target = TileCoord::from(result.target);
            Some((target, result.energy_cost.max(1)))
        })
        .collect()
}

fn manhattan_distance(a: TileCoord, b: TileCoord) -> u32 {
    a.x.abs_diff(b.x) + a.y.abs_diff(b.y)
}

fn direction_between(origin: TileCoord, target: TileCoord) -> Option<Direction> {
    match (target.x - origin.x, target.y - origin.y) {
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
    use bevy_ecs::world::World;

    use crate::map::Terrain;

    fn test_entity() -> Entity {
        World::new().spawn_empty().id()
    }

    #[test]
    fn returns_none_when_origin_is_target() {
        let map = Map::flat_for_tests(3, 3, Terrain::Grass, 0);
        let origin = Position { x: 1, y: 1 };

        assert_eq!(
            first_step_toward(&map, test_entity(), origin, origin, CargoLoad::default()),
            None
        );
    }

    #[test]
    fn routes_around_blocked_deep_water() {
        let mut map = Map::flat_for_tests(4, 3, Terrain::Grass, 0);
        map.set_for_tests(1, 1, Terrain::Water);
        map.set_water_depth_for_tests(1, 1, 3);

        let step = first_step_toward(
            &map,
            test_entity(),
            Position { x: 0, y: 1 },
            Position { x: 3, y: 1 },
            CargoLoad::default(),
        );

        assert!(matches!(step, Some(Direction::North | Direction::South)));
    }

    #[test]
    fn respects_movement_slope_limits() {
        let mut map = Map::flat_for_tests(3, 3, Terrain::Grass, 0);
        map.set_elevation_for_tests(1, 1, 5);

        let step = first_step_toward(
            &map,
            test_entity(),
            Position { x: 0, y: 1 },
            Position { x: 2, y: 1 },
            CargoLoad::default(),
        );

        assert!(matches!(step, Some(Direction::North | Direction::South)));
    }
}
