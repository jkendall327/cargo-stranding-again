use bevy_ecs::prelude::*;

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct InputState {
    pub move_x: i32,
    pub move_y: i32,
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct SimulationClock {
    pub turn: u64,
    pub delivered_parcels: u32,
}
