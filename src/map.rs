use std::collections::{hash_map::Entry, HashMap};

use bevy_ecs::prelude::*;

pub const MAP_WIDTH: i32 = 60;
pub const MAP_HEIGHT: i32 = 40;
/// Width in world tiles for one map chunk.
pub const CHUNK_WIDTH: i32 = 16;
/// Height in world tiles for one map chunk.
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

/// A tile coordinate in global world space.
///
/// Gameplay systems should use this coordinate space when asking map questions:
/// terrain at a position, movement between two tiles, parcel placement, and
/// world landmarks.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct TileCoord {
    pub x: i32,
    pub y: i32,
}

impl TileCoord {
    /// Builds a world tile coordinate.
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Identifies one fixed-size chunk in the world.
///
/// Chunks are the storage and future streaming unit. The map translates from a
/// `TileCoord` to a `ChunkCoord` plus `LocalTileCoord` before reading tile data.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct ChunkCoord {
    pub x: i32,
    pub y: i32,
}

impl ChunkCoord {
    /// Builds a chunk coordinate.
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// A tile coordinate local to a single chunk.
///
/// This is not a world position. Its valid range is `0..CHUNK_WIDTH` and
/// `0..CHUNK_HEIGHT`, and it is only meaningful together with a `ChunkCoord`.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct LocalTileCoord {
    pub x: i32,
    pub y: i32,
}

impl LocalTileCoord {
    /// Builds a chunk-local tile coordinate.
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Fixed-size terrain storage for one chunk.
///
/// Chunks currently store complete terrain/elevation/water arrays in memory.
/// Later, this is the natural unit to load, unload, generate, and persist.
#[derive(Clone, Debug)]
pub struct Chunk {
    coord: ChunkCoord,
    tiles: Vec<Terrain>,
    elevations: Vec<i16>,
    water_depths: Vec<u8>,
}

/// Chunk-backed map resource.
///
/// Callers address this as one continuous world using `TileCoord`. Internally,
/// the map resolves each world tile to a chunk and local tile coordinate.
#[derive(Resource, Clone, Debug)]
pub struct Map {
    width: i32,
    height: i32,
    pub seed: u64,
    loaded_chunks: HashMap<ChunkCoord, Chunk>,
    depot: TileCoord,
}

/// Finite world bounds in world tiles.
///
/// The current game still uses a finite prebaked world, so these bounds drive
/// camera clamping and out-of-bounds tile lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapBounds {
    pub width: i32,
    pub height: i32,
}

/// Complete terrain data for one world tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileInfo {
    pub terrain: Terrain,
    pub elevation: i16,
    pub water_depth: u8,
}

/// Terrain data for movement from one world tile to an adjacent world tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovementEdge {
    pub origin: TileInfo,
    pub target: TileInfo,
    pub elevation_delta: i16,
}

impl Chunk {
    /// Creates an empty grass-filled chunk at `coord`.
    pub fn new(coord: ChunkCoord) -> Self {
        let len = (CHUNK_WIDTH * CHUNK_HEIGHT) as usize;
        Self {
            coord,
            tiles: vec![Terrain::Grass; len],
            elevations: vec![0; len],
            water_depths: vec![0; len],
        }
    }

    /// Returns this chunk's world chunk coordinate.
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

/// Generates one deterministic terrain chunk from a world seed and chunk coordinate.
///
/// This is the procedural base hook for future streamed chunks. The current
/// startup map still uses its finite prebaked region, so roads, depot placement,
/// and authored lakes remain separate from this generic chunk generator.
pub fn generate_chunk(seed: u64, coord: ChunkCoord) -> Chunk {
    let mut chunk = Chunk::new(coord);

    for local_y in 0..CHUNK_HEIGHT {
        for local_x in 0..CHUNK_WIDTH {
            let local = LocalTileCoord::new(local_x, local_y);
            let world_x = coord.x * CHUNK_WIDTH + local_x;
            let world_y = coord.y * CHUNK_HEIGHT + local_y;

            let terrain_noise = deterministic_seeded_noise(seed, world_x, world_y, 0);
            let elevation = generated_elevation(seed, world_x, world_y);
            chunk.set_elevation(local, elevation);

            if terrain_noise < 5 {
                chunk.set_terrain(local, Terrain::Water);
                let depth_noise = deterministic_seeded_noise(seed, world_x, world_y, 1);
                chunk.set_water_depth(local, if depth_noise < 35 { 3 } else { 1 });
            } else if terrain_noise < 16 {
                chunk.set_terrain(local, Terrain::Mud);
            } else if terrain_noise > 91 {
                chunk.set_terrain(local, Terrain::Rock);
            }
        }
    }

    chunk
}

impl Map {
    /// Generates the current finite world into chunk-backed storage.
    ///
    /// This preserves the old `60x40` generated map behavior while giving the
    /// storage layer the same shape that future streamed/procedural worlds need.
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

    /// Returns the finite map dimensions in world tiles.
    pub fn bounds(&self) -> MapBounds {
        MapBounds {
            width: self.width,
            height: self.height,
        }
    }

    /// Returns the depot's world tile coordinate.
    pub fn depot_coord(&self) -> TileCoord {
        self.depot
    }

    /// Returns true when `coord` is inside the finite prebaked world.
    pub fn in_bounds_coord(&self, coord: TileCoord) -> bool {
        coord.x >= 0 && coord.y >= 0 && coord.x < self.width && coord.y < self.height
    }

    /// Returns terrain at a world tile, or `None` outside the finite world.
    pub fn terrain_at_coord(&self, coord: TileCoord) -> Option<Terrain> {
        self.tile_at_coord(coord).map(|tile| tile.terrain)
    }

    /// Returns elevation at a world tile, or `None` outside the finite world.
    pub fn elevation_at_coord(&self, coord: TileCoord) -> Option<i16> {
        self.tile_at_coord(coord).map(|tile| tile.elevation)
    }

    /// Returns water depth at a world tile, or `None` outside the finite world.
    pub fn water_depth_at_coord(&self, coord: TileCoord) -> Option<u8> {
        self.tile_at_coord(coord).map(|tile| tile.water_depth)
    }

    /// Returns all tile data at a world tile.
    ///
    /// This is the primary typed lookup. It splits the world coordinate into a
    /// chunk coordinate and chunk-local coordinate before reading storage.
    pub fn tile_at_coord(&self, coord: TileCoord) -> Option<TileInfo> {
        if !self.in_bounds_coord(coord) {
            return None;
        }
        let (chunk_coord, local_coord) = Self::split_tile_coord(coord);
        self.loaded_chunk(chunk_coord)
            .and_then(|chunk| chunk.tile_at(local_coord))
    }

    /// Returns terrain information needed to resolve movement between two tiles.
    pub fn movement_edge(&self, origin: TileCoord, target: TileCoord) -> Option<MovementEdge> {
        let origin = self.tile_at_coord(origin)?;
        let target = self.tile_at_coord(target)?;
        Some(MovementEdge {
            origin,
            target,
            elevation_delta: target.elevation - origin.elevation,
        })
    }

    /// Returns true when a world tile contains passable terrain.
    pub fn is_passable_coord(&self, coord: TileCoord) -> bool {
        self.terrain_at_coord(coord).is_some_and(Terrain::passable)
    }

    /// Iterates finite in-bounds world tiles visible from a rectangular viewport.
    ///
    /// The iterator clamps to the current finite map bounds. It still yields
    /// world tile coordinates, so rendering and headless views can cross chunk
    /// boundaries without knowing where those boundaries are.
    pub fn visible_tiles(&self, origin: TileCoord, width: i32, height: i32) -> VisibleTiles {
        VisibleTiles::new(origin, width, height, self.bounds())
    }

    /// Returns a loaded chunk, or `None` when that chunk is absent.
    pub fn loaded_chunk(&self, coord: ChunkCoord) -> Option<&Chunk> {
        self.loaded_chunks.get(&coord)
    }

    /// Returns a mutable loaded chunk, or `None` when that chunk is absent.
    pub fn loaded_chunk_mut(&mut self, coord: ChunkCoord) -> Option<&mut Chunk> {
        self.loaded_chunks.get_mut(&coord)
    }

    /// Ensures an in-bounds finite chunk exists in loaded storage.
    ///
    /// This is deliberately finite-world scoped for now: chunks outside the
    /// current prebaked map remain absent instead of being generated on demand.
    pub fn ensure_chunk_loaded(&mut self, coord: ChunkCoord) -> Option<&mut Chunk> {
        if !self.chunk_coord_in_bounds(coord) {
            return None;
        }

        match self.loaded_chunks.entry(coord) {
            Entry::Occupied(entry) => Some(entry.into_mut()),
            Entry::Vacant(entry) => Some(entry.insert(Chunk::new(coord))),
        }
    }

    /// Splits a world tile coordinate into chunk and chunk-local coordinates.
    ///
    /// Uses Euclidean division/remainder so negative world coordinates map to
    /// stable chunks correctly when infinite or streamed worlds arrive.
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
        let mut loaded_chunks = HashMap::new();
        for chunk_y in 0..chunk_span(height, CHUNK_HEIGHT) {
            for chunk_x in 0..chunk_span(width, CHUNK_WIDTH) {
                let coord = ChunkCoord::new(chunk_x, chunk_y);
                loaded_chunks.insert(coord, Chunk::new(coord));
            }
        }
        Self {
            width,
            height,
            seed,
            loaded_chunks,
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
        self.loaded_chunk_mut(chunk_coord)
    }

    fn chunk_coord_in_bounds(&self, coord: ChunkCoord) -> bool {
        coord.x >= 0
            && coord.y >= 0
            && coord.x < chunk_span(self.width, CHUNK_WIDTH)
            && coord.y < chunk_span(self.height, CHUNK_HEIGHT)
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

/// Iterator over visible finite world tiles.
///
/// It yields `TileCoord`s in row-major order, clamped to the current finite map
/// bounds. Callers then ask `Map` for terrain data at each yielded coordinate.
pub struct VisibleTiles {
    start_x: i32,
    end_x: i32,
    end_y: i32,
    next: TileCoord,
}

impl VisibleTiles {
    fn new(origin: TileCoord, width: i32, height: i32, bounds: MapBounds) -> Self {
        let start_x = origin.x.clamp(0, bounds.width);
        let start_y = origin.y.clamp(0, bounds.height);
        let end_x = (origin.x + width).clamp(0, bounds.width);
        let end_y = (origin.y + height).clamp(0, bounds.height);
        Self {
            start_x,
            end_x,
            end_y,
            next: TileCoord::new(start_x, start_y),
        }
    }
}

impl Iterator for VisibleTiles {
    type Item = TileCoord;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next.y >= self.end_y || self.next.x >= self.end_x {
            return None;
        }

        let coord = self.next;
        self.next.x += 1;
        if self.next.x >= self.end_x {
            self.next.x = self.start_x;
            self.next.y += 1;
        }
        Some(coord)
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
        self.loaded_chunks.len()
    }
}

fn chunk_span(size: i32, chunk_size: i32) -> i32 {
    size.div_euclid(chunk_size) + i32::from(size.rem_euclid(chunk_size) != 0)
}

fn deterministic_noise(x: i32, y: i32) -> i32 {
    let n = (x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663) ^ 0x5bd1_e995;
    (n % 100) as i32
}

fn generated_elevation(seed: u64, x: i32, y: i32) -> i16 {
    let mut total = 0;
    let mut samples = 0;
    for sample_y in (y - 2)..=(y + 2) {
        for sample_x in (x - 2)..=(x + 2) {
            total += deterministic_seeded_noise(seed, sample_x / 3, sample_y / 3, 2);
            samples += 1;
        }
    }
    ((total / samples) / 11).clamp(0, 9) as i16
}

fn deterministic_seeded_noise(seed: u64, x: i32, y: i32, salt: u64) -> i32 {
    let mut n = seed
        ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (x as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ (y as i64 as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
    n ^= n >> 30;
    n = n.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    n ^= n >> 27;
    n = n.wrapping_mul(0x94D0_49BB_1331_11EB);
    n ^= n >> 31;
    (n % 100) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_map_allocates_chunks_for_finite_bounds() {
        let map = Map::generate();
        let bounds = map.bounds();

        assert_eq!(bounds.width, MAP_WIDTH);
        assert_eq!(bounds.height, MAP_HEIGHT);
        assert_eq!(map.chunk_count(), 12);
        assert!(map.loaded_chunk(ChunkCoord::new(0, 0)).is_some());
        assert!(map.loaded_chunk(ChunkCoord::new(3, 2)).is_some());
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
    fn generated_chunks_are_stable_for_same_seed_and_coord() {
        let first = generate_chunk(12_345, ChunkCoord::new(2, -1));
        let second = generate_chunk(12_345, ChunkCoord::new(2, -1));

        assert_eq!(first.coord(), second.coord());
        for y in 0..CHUNK_HEIGHT {
            for x in 0..CHUNK_WIDTH {
                let local = LocalTileCoord::new(x, y);
                assert_eq!(first.tile_at(local), second.tile_at(local));
            }
        }
    }

    #[test]
    fn generated_chunks_include_seed_and_chunk_coord_in_terrain() {
        let base = generate_chunk(12_345, ChunkCoord::new(2, -1));
        let different_seed = generate_chunk(54_321, ChunkCoord::new(2, -1));
        let different_coord = generate_chunk(12_345, ChunkCoord::new(3, -1));

        assert!(chunks_differ(&base, &different_seed));
        assert!(chunks_differ(&base, &different_coord));
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
    fn visible_tile_iteration_clamps_to_bounds() {
        let map = Map::flat_for_tests(32, 32, Terrain::Grass, 4);
        let tiles = map
            .visible_tiles(TileCoord::new(30, 30), 5, 5)
            .collect::<Vec<_>>();

        assert_eq!(
            tiles,
            vec![
                TileCoord::new(30, 30),
                TileCoord::new(31, 30),
                TileCoord::new(30, 31),
                TileCoord::new(31, 31),
            ]
        );
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
    fn loaded_chunks_are_distinct_from_absent_chunks() {
        let map = Map::generate();

        assert!(map.loaded_chunk(ChunkCoord::new(0, 0)).is_some());
        assert!(map.loaded_chunk(ChunkCoord::new(3, 2)).is_some());
        assert!(map.loaded_chunk(ChunkCoord::new(4, 0)).is_none());
        assert!(map.loaded_chunk(ChunkCoord::new(-1, 0)).is_none());
    }

    #[test]
    fn ensuring_chunk_loaded_is_finite_world_scoped() {
        let mut map = Map::generate();
        let loaded_before = map.chunk_count();

        let chunk = map
            .ensure_chunk_loaded(ChunkCoord::new(3, 2))
            .expect("finite chunk should be loadable");
        assert_eq!(chunk.coord(), ChunkCoord::new(3, 2));
        assert_eq!(map.chunk_count(), loaded_before);

        assert!(map.ensure_chunk_loaded(ChunkCoord::new(4, 0)).is_none());
        assert!(map.ensure_chunk_loaded(ChunkCoord::new(-1, 0)).is_none());
        assert_eq!(map.chunk_count(), loaded_before);
    }

    #[test]
    fn water_tiles_have_depth_and_dry_tiles_do_not() {
        let map = Map::generate();
        let bounds = map.bounds();

        for y in 0..bounds.height {
            for x in 0..bounds.width {
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

    fn chunks_differ(a: &Chunk, b: &Chunk) -> bool {
        for y in 0..CHUNK_HEIGHT {
            for x in 0..CHUNK_WIDTH {
                let local = LocalTileCoord::new(x, y);
                if a.tile_at(local) != b.tile_at(local) {
                    return true;
                }
            }
        }
        false
    }
}
