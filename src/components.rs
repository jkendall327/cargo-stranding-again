use bevy_ecs::prelude::*;

pub use crate::energy::ActionEnergy;
use crate::map::TileCoord;
use crate::momentum::MomentumState;
use crate::movement::MovementMode;
use crate::resources::Direction;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl From<TileCoord> for Position {
    /// Converts a world tile coordinate into an ECS position component.
    fn from(coord: TileCoord) -> Self {
        Self {
            x: coord.x,
            y: coord.y,
        }
    }
}

impl From<Position> for TileCoord {
    /// Converts an ECS position component into a world tile coordinate.
    fn from(position: Position) -> Self {
        Self::new(position.x, position.y)
    }
}

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Velocity {
    pub dx: i32,
    pub dy: i32,
}

#[derive(Component, Debug)]
pub struct Player;

#[derive(Component, Debug)]
pub struct Agent {
    pub id: usize,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Cargo {
    pub current_weight: f32,
    pub max_weight: f32,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Stamina {
    pub current: f32,
    pub max: f32,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovementState {
    pub mode: MovementMode,
}

impl Default for MovementState {
    fn default() -> Self {
        Self {
            mode: MovementMode::Walking,
        }
    }
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Momentum {
    pub direction: Option<Direction>,
    pub amount: f32,
}

impl From<Momentum> for MomentumState {
    fn from(momentum: Momentum) -> Self {
        Self {
            direction: momentum.direction,
            amount: momentum.amount,
        }
    }
}

impl From<MomentumState> for Momentum {
    fn from(momentum: MomentumState) -> Self {
        Self {
            direction: momentum.direction,
            amount: momentum.amount,
        }
    }
}

impl MovementState {
    pub fn cycle_mode(&mut self) {
        self.mode = match self.mode {
            MovementMode::Walking => MovementMode::Sprinting,
            MovementMode::Sprinting => MovementMode::Steady,
            MovementMode::Steady => MovementMode::Walking,
        };
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct CargoParcel {
    pub weight: f32,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParcelState {
    Loose,
    AssignedTo(Entity),
    CarriedBy(Entity),
    Delivered,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct AssignedJob {
    pub phase: JobPhase,
    pub parcel: Option<Entity>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobPhase {
    FindParcel,
    GoToParcel,
    GoToDepot,
    Done,
}
