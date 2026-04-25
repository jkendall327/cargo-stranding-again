use bevy_ecs::prelude::*;

pub use crate::energy::ActionEnergy;
use crate::movement::MovementMode;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub x: i32,
    pub y: i32,
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
