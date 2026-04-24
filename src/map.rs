use bevy_ecs::prelude::*;
use macroquad::prelude::*;

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
            Terrain::Water => f32::INFINITY,
            Terrain::Road => 0.6,
            Terrain::Depot => 0.8,
        }
    }

    pub fn color(self) -> Color {
        match self {
            Terrain::Grass => Color::from_rgba(64, 128, 72, 255),
            Terrain::Mud => Color::from_rgba(104, 75, 48, 255),
            Terrain::Rock => Color::from_rgba(92, 96, 100, 255),
            Terrain::Water => Color::from_rgba(34, 92, 138, 255),
            Terrain::Road => Color::from_rgba(150, 126, 78, 255),
            Terrain::Depot => Color::from_rgba(214, 174, 68, 255),
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Terrain::Grass => ".",
            Terrain::Mud => "~",
            Terrain::Rock => "^",
            Terrain::Water => "w",
            Terrain::Road => "=",
            Terrain::Depot => "D",
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
    pub depot: (i32, i32),
}

impl Map {
    pub fn generate() -> Self {
        let width = MAP_WIDTH;
        let height = MAP_HEIGHT;
        let mut tiles = vec![Terrain::Grass; (width * height) as usize];
        let depot = (48, 30);

        let mut map = Self {
            width,
            height,
            tiles: std::mem::take(&mut tiles),
            depot,
        };

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
                    }
                }
            }
        }

        for x in 5..55 {
            map.set(x, 31, Terrain::Road);
        }
        for y in 8..32 {
            map.set(48, y, Terrain::Road);
        }
        for x in 8..22 {
            map.set(x, 12, Terrain::Road);
        }

        map.set(depot.0, depot.1, Terrain::Depot);
        map
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width && y < self.height
    }

    pub fn terrain_at(&self, x: i32, y: i32) -> Option<Terrain> {
        self.in_bounds(x, y)
            .then(|| self.tiles[(y * self.width + x) as usize])
    }

    pub fn is_passable(&self, x: i32, y: i32) -> bool {
        self.terrain_at(x, y).is_some_and(Terrain::passable)
    }

    fn set(&mut self, x: i32, y: i32, terrain: Terrain) {
        if self.in_bounds(x, y) {
            self.tiles[(y * self.width + x) as usize] = terrain;
        }
    }
}

fn deterministic_noise(x: i32, y: i32) -> i32 {
    let n = (x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663) ^ 0x5bd1_e995;
    (n % 100) as i32
}
