use bevy_ecs::prelude::*;

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct InputState {
    pub move_x: i32,
    pub move_y: i32,
    pub wait: bool,
}

impl InputState {
    pub fn has_action(self) -> bool {
        self.wait || self.move_x != 0 || self.move_y != 0
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct SimulationClock {
    pub turn: u64,
    pub delivered_parcels: u32,
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct TurnState {
    pub consumed: bool,
}
