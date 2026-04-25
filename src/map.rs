use bevy_ecs::prelude::*;

pub const MAP_WIDTH: i32 = 60;
pub const MAP_HEIGHT: i32 = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Terrain {
    Grass,
    Mud,
    Rock,
    Water,
    Road,
    Depot,
}

impl Terrain {
    pub fn movement_cost(self) -> f32 {
        match self {
            Terrain::Grass => 1.0,
            Terrain::Mud => 2.2,
            Terrain::Rock => 3.0,
            Terrain::Water => 4.0,
            Terrain::Road => 0.6,
            Terrain::Depot => 0.8,
        }
    }

    pub fn stamina_delta(self) -> f32 {
        match self {
            Terrain::Grass => 0.0,
            Terrain::Mud => -2.0,
            Terrain::Rock => -3.5,
            Terrain::Water => 0.0,
            Terrain::Road => 0.75,
            Terrain::Depot => 1.5,
        }
    }

    pub fn passable(self) -> bool {
        self != Terrain::Water
    }
}

#[derive(Resource, Clone, Debug)]
pub struct Map {
    pub width: i32,
    pub height: i32,
    tiles: Vec<Terrain>,
    elevations: Vec<i16>,
    water_depths: Vec<u8>,
    pub depot: (i32, i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileInfo {
    pub terrain: Terrain,
    pub elevation: i16,
    pub water_depth: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovementEdge {
    pub origin: TileInfo,
    pub target: TileInfo,
    pub elevation_delta: i16,
}

impl Map {
    pub fn generate() -> Self {
        let width = MAP_WIDTH;
        let height = MAP_HEIGHT;
        let depot = (48, 30);

        let mut map = Self {
            width,
            height,
            tiles: vec![Terrain::Grass; (width * height) as usize],
            elevations: vec![0; (width * height) as usize],
            water_depths: vec![0; (width * height) as usize],
            depot,
        };

        map.generate_elevation();

        for y in 0..height {
            for x in 0..width {
                let noise = deterministic_noise(x, y);
                if noise < 7 {
                    map.set(x, y, Terrain::Mud);
                } else if noise > 92 {
                    map.set(x, y, Terrain::Rock);
                }
            }
        }

        for &(cx, cy, radius) in &[(13, 10, 5), (24, 25, 4), (43, 13, 6)] {
            for y in (cy - radius)..=(cy + radius) {
                for x in (cx - radius)..=(cx + radius) {
                    let dx = x - cx;
                    let dy = y - cy;
                    if dx * dx + dy * dy <= radius * radius {
                        map.set(x, y, Terrain::Water);
                        let distance_squared = dx * dx + dy * dy;
                        let inner_radius = (radius - 2).max(1);
                        let depth = if distance_squared <= inner_radius * inner_radius {
                            3
                        } else {
                            1
                        };
                        map.set_water_depth(x, y, depth);
                    }
                }
            }
        }

        for x in 5..55 {
            map.set(x, 31, Terrain::Road);
            map.set_water_depth(x, 31, 0);
        }
        for y in 8..32 {
            map.set(48, y, Terrain::Road);
            map.set_water_depth(48, y, 0);
        }
        for x in 8..22 {
            map.set(x, 12, Terrain::Road);
            map.set_water_depth(x, 12, 0);
        }
        map.flatten_roads();

        map.set(depot.0, depot.1, Terrain::Depot);
        map.set_water_depth(depot.0, depot.1, 0);
        map
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width && y < self.height
    }

    pub fn terrain_at(&self, x: i32, y: i32) -> Option<Terrain> {
        self.in_bounds(x, y)
            .then(|| self.tiles[(y * self.width + x) as usize])
    }

    pub fn elevation_at(&self, x: i32, y: i32) -> Option<i16> {
        self.index(x, y).map(|index| self.elevations[index])
    }

    pub fn water_depth_at(&self, x: i32, y: i32) -> Option<u8> {
        self.index(x, y).map(|index| self.water_depths[index])
    }

    pub fn tile_at(&self, x: i32, y: i32) -> Option<TileInfo> {
        self.index(x, y).map(|index| TileInfo {
            terrain: self.tiles[index],
            elevation: self.elevations[index],
            water_depth: self.water_depths[index],
        })
    }

    pub fn movement_edge(&self, origin: (i32, i32), target: (i32, i32)) -> Option<MovementEdge> {
        let origin = self.tile_at(origin.0, origin.1)?;
        let target = self.tile_at(target.0, target.1)?;
        Some(MovementEdge {
            origin,
            target,
            elevation_delta: target.elevation - origin.elevation,
        })
    }

    pub fn is_passable(&self, x: i32, y: i32) -> bool {
        self.terrain_at(x, y).is_some_and(Terrain::passable)
    }

    fn set(&mut self, x: i32, y: i32, terrain: Terrain) {
        if let Some(index) = self.index(x, y) {
            self.tiles[index] = terrain;
            if terrain != Terrain::Water {
                self.water_depths[index] = 0;
            }
        }
    }

    fn set_elevation(&mut self, x: i32, y: i32, elevation: i16) {
        if let Some(index) = self.index(x, y) {
            self.elevations[index] = elevation;
        }
    }

    fn set_water_depth(&mut self, x: i32, y: i32, depth: u8) {
        if let Some(index) = self.index(x, y) {
            self.water_depths[index] = depth;
        }
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        self.in_bounds(x, y)
            .then_some((y * self.width + x) as usize)
    }

    fn generate_elevation(&mut self) {
        for y in 0..self.height {
            for x in 0..self.width {
                let mut total = 0;
                let mut samples = 0;
                for sample_y in (y - 2)..=(y + 2) {
                    for sample_x in (x - 2)..=(x + 2) {
                        total += deterministic_noise(sample_x / 3, sample_y / 3);
                        samples += 1;
                    }
                }
                let averaged = total / samples;
                self.set_elevation(x, y, (averaged / 11).clamp(0, 9) as i16);
            }
        }
    }

    fn flatten_roads(&mut self) {
        for y in 0..self.height {
            for x in 0..self.width {
                let Some(terrain) = self.terrain_at(x, y) else {
                    continue;
                };
                if !matches!(terrain, Terrain::Road | Terrain::Depot) {
                    continue;
                }

                let mut total = 0;
                let mut samples = 0;
                for (neighbor_x, neighbor_y) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
                    if matches!(
                        self.terrain_at(neighbor_x, neighbor_y),
                        Some(Terrain::Road | Terrain::Depot)
                    ) {
                        if let Some(elevation) = self.elevation_at(neighbor_x, neighbor_y) {
                            total += elevation;
                            samples += 1;
                        }
                    }
                }
                if samples > 0 {
                    self.set_elevation(x, y, total / samples);
                }
            }
        }
    }
}

#[cfg(test)]
impl Map {
    pub(crate) fn flat_for_tests(
        width: i32,
        height: i32,
        terrain: Terrain,
        elevation: i16,
    ) -> Self {
        Self {
            width,
            height,
            tiles: vec![terrain; (width * height) as usize],
            elevations: vec![elevation; (width * height) as usize],
            water_depths: vec![0; (width * height) as usize],
            depot: (0, 0),
        }
    }

    pub(crate) fn set_for_tests(&mut self, x: i32, y: i32, terrain: Terrain) {
        self.set(x, y, terrain);
    }

    pub(crate) fn set_elevation_for_tests(&mut self, x: i32, y: i32, elevation: i16) {
        self.set_elevation(x, y, elevation);
    }

    pub(crate) fn set_water_depth_for_tests(&mut self, x: i32, y: i32, depth: u8) {
        self.set_water_depth(x, y, depth);
    }
}

fn deterministic_noise(x: i32, y: i32) -> i32 {
    let n = (x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663) ^ 0x5bd1_e995;
    (n % 100) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_heightfields_match_map_dimensions() {
        let map = Map::generate();
        let expected_len = (map.width * map.height) as usize;

        assert_eq!(map.tiles.len(), expected_len);
        assert_eq!(map.elevations.len(), expected_len);
        assert_eq!(map.water_depths.len(), expected_len);
    }

    #[test]
    fn generated_heightfields_are_stable() {
        let first = Map::generate();
        let second = Map::generate();

        for (x, y) in [(0, 0), (6, 6), (13, 10), (48, 30), (59, 39)] {
            assert_eq!(first.tile_at(x, y), second.tile_at(x, y));
        }
    }

    #[test]
    fn water_tiles_have_depth_and_dry_tiles_do_not() {
        let map = Map::generate();

        for y in 0..map.height {
            for x in 0..map.width {
                let tile = map
                    .tile_at(x, y)
                    .expect("generated coordinate is in bounds");
                if tile.terrain == Terrain::Water {
                    assert!(tile.water_depth > 0);
                } else {
                    assert_eq!(tile.water_depth, 0);
                }
            }
        }
    }
}
