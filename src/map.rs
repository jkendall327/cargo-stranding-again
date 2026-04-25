use std::collections::HashMap;

use bevy_ecs::prelude::*;

pub const MAP_WIDTH: i32 = 60;
pub const MAP_HEIGHT: i32 = 40;
pub const CHUNK_WIDTH: i32 = 16;
pub const CHUNK_HEIGHT: i32 = 16;
pub const DEFAULT_MAP_SEED: u64 = 0xCA6E_057A;

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

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct TileCoord {
    pub x: i32,
    pub y: i32,
}

impl TileCoord {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct ChunkCoord {
    pub x: i32,
    pub y: i32,
}

impl ChunkCoord {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct LocalTileCoord {
    pub x: i32,
    pub y: i32,
}

impl LocalTileCoord {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Debug)]
pub struct Chunk {
    coord: ChunkCoord,
    tiles: Vec<Terrain>,
    elevations: Vec<i16>,
    water_depths: Vec<u8>,
}

#[derive(Resource, Clone, Debug)]
pub struct Map {
    pub width: i32,
    pub height: i32,
    pub seed: u64,
    chunks: HashMap<ChunkCoord, Chunk>,
    pub depot: TileCoord,
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

impl Chunk {
    pub fn new(coord: ChunkCoord) -> Self {
        let len = (CHUNK_WIDTH * CHUNK_HEIGHT) as usize;
        Self {
            coord,
            tiles: vec![Terrain::Grass; len],
            elevations: vec![0; len],
            water_depths: vec![0; len],
        }
    }

    pub fn coord(&self) -> ChunkCoord {
        self.coord
    }

    fn tile_at(&self, local: LocalTileCoord) -> Option<TileInfo> {
        self.index(local).map(|index| TileInfo {
            terrain: self.tiles[index],
            elevation: self.elevations[index],
            water_depth: self.water_depths[index],
        })
    }

    fn set_terrain(&mut self, local: LocalTileCoord, terrain: Terrain) {
        if let Some(index) = self.index(local) {
            self.tiles[index] = terrain;
            if terrain != Terrain::Water {
                self.water_depths[index] = 0;
            }
        }
    }

    fn set_elevation(&mut self, local: LocalTileCoord, elevation: i16) {
        if let Some(index) = self.index(local) {
            self.elevations[index] = elevation;
        }
    }

    fn set_water_depth(&mut self, local: LocalTileCoord, depth: u8) {
        if let Some(index) = self.index(local) {
            self.water_depths[index] = depth;
        }
    }

    fn index(&self, local: LocalTileCoord) -> Option<usize> {
        (local.x >= 0 && local.y >= 0 && local.x < CHUNK_WIDTH && local.y < CHUNK_HEIGHT)
            .then_some((local.y * CHUNK_WIDTH + local.x) as usize)
    }
}

impl Map {
    pub fn generate() -> Self {
        let width = MAP_WIDTH;
        let height = MAP_HEIGHT;
        let depot = TileCoord::new(48, 30);

        let mut map = Self::blank(width, height, DEFAULT_MAP_SEED, depot);

        map.generate_elevation();

        for y in 0..height {
            for x in 0..width {
                let noise = deterministic_noise(x, y);
                if noise < 7 {
                    map.set(TileCoord::new(x, y), Terrain::Mud);
                } else if noise > 92 {
                    map.set(TileCoord::new(x, y), Terrain::Rock);
                }
            }
        }

        for &(cx, cy, radius) in &[(13, 10, 5), (24, 25, 4), (43, 13, 6)] {
            for y in (cy - radius)..=(cy + radius) {
                for x in (cx - radius)..=(cx + radius) {
                    let dx = x - cx;
                    let dy = y - cy;
                    if dx * dx + dy * dy <= radius * radius {
                        let coord = TileCoord::new(x, y);
                        map.set(coord, Terrain::Water);
                        let distance_squared = dx * dx + dy * dy;
                        let inner_radius = (radius - 2).max(1);
                        let depth = if distance_squared <= inner_radius * inner_radius {
                            3
                        } else {
                            1
                        };
                        map.set_water_depth(coord, depth);
                    }
                }
            }
        }

        for x in 5..55 {
            let coord = TileCoord::new(x, 31);
            map.set(coord, Terrain::Road);
            map.set_water_depth(coord, 0);
        }
        for y in 8..32 {
            let coord = TileCoord::new(48, y);
            map.set(coord, Terrain::Road);
            map.set_water_depth(coord, 0);
        }
        for x in 8..22 {
            let coord = TileCoord::new(x, 12);
            map.set(coord, Terrain::Road);
            map.set_water_depth(coord, 0);
        }
        map.flatten_roads();

        map.set(depot, Terrain::Depot);
        map.set_water_depth(depot, 0);
        map
    }

    pub fn in_bounds_coord(&self, coord: TileCoord) -> bool {
        coord.x >= 0 && coord.y >= 0 && coord.x < self.width && coord.y < self.height
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        self.in_bounds_coord(TileCoord::new(x, y))
    }

    pub fn terrain_at_coord(&self, coord: TileCoord) -> Option<Terrain> {
        self.tile_at_coord(coord).map(|tile| tile.terrain)
    }

    pub fn terrain_at(&self, x: i32, y: i32) -> Option<Terrain> {
        self.terrain_at_coord(TileCoord::new(x, y))
    }

    pub fn elevation_at_coord(&self, coord: TileCoord) -> Option<i16> {
        self.tile_at_coord(coord).map(|tile| tile.elevation)
    }

    pub fn elevation_at(&self, x: i32, y: i32) -> Option<i16> {
        self.elevation_at_coord(TileCoord::new(x, y))
    }

    pub fn water_depth_at_coord(&self, coord: TileCoord) -> Option<u8> {
        self.tile_at_coord(coord).map(|tile| tile.water_depth)
    }

    pub fn water_depth_at(&self, x: i32, y: i32) -> Option<u8> {
        self.water_depth_at_coord(TileCoord::new(x, y))
    }

    pub fn tile_at_coord(&self, coord: TileCoord) -> Option<TileInfo> {
        if !self.in_bounds_coord(coord) {
            return None;
        }
        let (chunk_coord, local_coord) = Self::split_tile_coord(coord);
        self.chunks
            .get(&chunk_coord)
            .and_then(|chunk| chunk.tile_at(local_coord))
    }

    pub fn tile_at(&self, x: i32, y: i32) -> Option<TileInfo> {
        self.tile_at_coord(TileCoord::new(x, y))
    }

    pub fn movement_edge(&self, origin: TileCoord, target: TileCoord) -> Option<MovementEdge> {
        let origin = self.tile_at_coord(origin)?;
        let target = self.tile_at_coord(target)?;
        Some(MovementEdge {
            origin,
            target,
            elevation_delta: target.elevation - origin.elevation,
        })
    }

    pub fn is_passable(&self, x: i32, y: i32) -> bool {
        self.terrain_at(x, y).is_some_and(Terrain::passable)
    }

    pub fn split_tile_coord(coord: TileCoord) -> (ChunkCoord, LocalTileCoord) {
        (
            ChunkCoord::new(
                coord.x.div_euclid(CHUNK_WIDTH),
                coord.y.div_euclid(CHUNK_HEIGHT),
            ),
            LocalTileCoord::new(
                coord.x.rem_euclid(CHUNK_WIDTH),
                coord.y.rem_euclid(CHUNK_HEIGHT),
            ),
        )
    }

    fn blank(width: i32, height: i32, seed: u64, depot: TileCoord) -> Self {
        let mut chunks = HashMap::new();
        for chunk_y in 0..chunk_span(height, CHUNK_HEIGHT) {
            for chunk_x in 0..chunk_span(width, CHUNK_WIDTH) {
                let coord = ChunkCoord::new(chunk_x, chunk_y);
                chunks.insert(coord, Chunk::new(coord));
            }
        }
        Self {
            width,
            height,
            seed,
            chunks,
            depot,
        }
    }

    fn set(&mut self, coord: TileCoord, terrain: Terrain) {
        if let Some(chunk) = self.chunk_mut(coord) {
            let (_, local) = Self::split_tile_coord(coord);
            chunk.set_terrain(local, terrain);
        }
    }

    fn set_elevation(&mut self, coord: TileCoord, elevation: i16) {
        if let Some(chunk) = self.chunk_mut(coord) {
            let (_, local) = Self::split_tile_coord(coord);
            chunk.set_elevation(local, elevation);
        }
    }

    fn set_water_depth(&mut self, coord: TileCoord, depth: u8) {
        if let Some(chunk) = self.chunk_mut(coord) {
            let (_, local) = Self::split_tile_coord(coord);
            chunk.set_water_depth(local, depth);
        }
    }

    fn chunk_mut(&mut self, coord: TileCoord) -> Option<&mut Chunk> {
        if !self.in_bounds_coord(coord) {
            return None;
        }
        let (chunk_coord, _) = Self::split_tile_coord(coord);
        self.chunks.get_mut(&chunk_coord)
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
                self.set_elevation(TileCoord::new(x, y), (averaged / 11).clamp(0, 9) as i16);
            }
        }
    }

    fn flatten_roads(&mut self) {
        for y in 0..self.height {
            for x in 0..self.width {
                let coord = TileCoord::new(x, y);
                let Some(terrain) = self.terrain_at_coord(coord) else {
                    continue;
                };
                if !matches!(terrain, Terrain::Road | Terrain::Depot) {
                    continue;
                }

                let mut total = 0;
                let mut samples = 0;
                for neighbor in [
                    TileCoord::new(x - 1, y),
                    TileCoord::new(x + 1, y),
                    TileCoord::new(x, y - 1),
                    TileCoord::new(x, y + 1),
                ] {
                    if matches!(
                        self.terrain_at_coord(neighbor),
                        Some(Terrain::Road | Terrain::Depot)
                    ) {
                        if let Some(elevation) = self.elevation_at_coord(neighbor) {
                            total += elevation;
                            samples += 1;
                        }
                    }
                }
                if samples > 0 {
                    self.set_elevation(coord, total / samples);
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
        let mut map = Self::blank(width, height, DEFAULT_MAP_SEED, TileCoord::new(0, 0));
        for y in 0..height {
            for x in 0..width {
                let coord = TileCoord::new(x, y);
                map.set(coord, terrain);
                map.set_elevation(coord, elevation);
            }
        }
        map
    }

    pub(crate) fn set_for_tests(&mut self, x: i32, y: i32, terrain: Terrain) {
        self.set(TileCoord::new(x, y), terrain);
    }

    pub(crate) fn set_elevation_for_tests(&mut self, x: i32, y: i32, elevation: i16) {
        self.set_elevation(TileCoord::new(x, y), elevation);
    }

    pub(crate) fn set_water_depth_for_tests(&mut self, x: i32, y: i32, depth: u8) {
        self.set_water_depth(TileCoord::new(x, y), depth);
    }

    fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
}

fn chunk_span(size: i32, chunk_size: i32) -> i32 {
    size.div_euclid(chunk_size) + i32::from(size.rem_euclid(chunk_size) != 0)
}

fn deterministic_noise(x: i32, y: i32) -> i32 {
    let n = (x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663) ^ 0x5bd1_e995;
    (n % 100) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_map_allocates_chunks_for_finite_bounds() {
        let map = Map::generate();

        assert_eq!(map.width, MAP_WIDTH);
        assert_eq!(map.height, MAP_HEIGHT);
        assert_eq!(map.chunk_count(), 12);
    }

    #[test]
    fn tile_coordinates_split_across_chunk_boundaries() {
        assert_eq!(
            Map::split_tile_coord(TileCoord::new(15, 15)),
            (ChunkCoord::new(0, 0), LocalTileCoord::new(15, 15))
        );
        assert_eq!(
            Map::split_tile_coord(TileCoord::new(16, 16)),
            (ChunkCoord::new(1, 1), LocalTileCoord::new(0, 0))
        );
        assert_eq!(
            Map::split_tile_coord(TileCoord::new(-1, -1)),
            (ChunkCoord::new(-1, -1), LocalTileCoord::new(15, 15))
        );
    }

    #[test]
    fn generated_heightfields_are_stable() {
        let first = Map::generate();
        let second = Map::generate();

        for coord in [
            TileCoord::new(0, 0),
            TileCoord::new(6, 6),
            TileCoord::new(13, 10),
            TileCoord::new(48, 30),
            TileCoord::new(59, 39),
        ] {
            assert_eq!(first.tile_at_coord(coord), second.tile_at_coord(coord));
        }
    }

    #[test]
    fn lookups_cross_chunk_boundaries() {
        let mut map = Map::flat_for_tests(32, 32, Terrain::Grass, 4);
        map.set_for_tests(15, 16, Terrain::Road);
        map.set_elevation_for_tests(15, 16, 7);
        map.set_water_depth_for_tests(16, 15, 1);

        assert_eq!(
            map.terrain_at_coord(TileCoord::new(15, 16)),
            Some(Terrain::Road)
        );
        assert_eq!(map.elevation_at_coord(TileCoord::new(15, 16)), Some(7));
        assert_eq!(map.water_depth_at_coord(TileCoord::new(16, 15)), Some(1));
        assert!(map
            .movement_edge(TileCoord::new(15, 15), TileCoord::new(16, 15))
            .is_some());
    }

    #[test]
    fn finite_world_lookups_outside_bounds_are_absent() {
        let map = Map::generate();

        assert_eq!(map.tile_at_coord(TileCoord::new(-1, 0)), None);
        assert_eq!(map.tile_at_coord(TileCoord::new(0, -1)), None);
        assert_eq!(map.tile_at_coord(TileCoord::new(MAP_WIDTH, 0)), None);
        assert_eq!(map.tile_at_coord(TileCoord::new(0, MAP_HEIGHT)), None);
    }

    #[test]
    fn water_tiles_have_depth_and_dry_tiles_do_not() {
        let map = Map::generate();

        for y in 0..map.height {
            for x in 0..map.width {
                let tile = map
                    .tile_at_coord(TileCoord::new(x, y))
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
