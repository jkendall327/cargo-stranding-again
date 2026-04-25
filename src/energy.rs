use bevy_ecs::prelude::*;

use crate::map::Terrain;
use crate::movement::MovementMode;

pub const WALK_ENERGY_COST: u32 = 100;
pub const WAIT_ENERGY_COST: u32 = 100;
pub const ITEM_ACTION_ENERGY_COST: u32 = 100;
pub const DEFAULT_ACTION_ENERGY_COST: u32 = 100;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActionEnergy {
    pub ready_at: u64,
    pub last_cost: u32,
}

impl ActionEnergy {
    pub fn is_ready(self, now: u64) -> bool {
        self.ready_at <= now
    }

    pub fn spend(&mut self, now: u64, cost: u32) {
        self.last_cost = cost;
        self.ready_at = now + u64::from(cost);
    }
}

pub fn movement_energy_cost(terrain: Terrain, mode: MovementMode) -> u32 {
    let terrain_cost = (WALK_ENERGY_COST as f32 * terrain.movement_cost()).round() as u32;
    match mode {
        MovementMode::Walking => terrain_cost.max(1),
        MovementMode::Sprinting => ((terrain_cost as f32) * 0.65).round().max(1.0) as u32,
        MovementMode::Steady => ((terrain_cost as f32) * 1.35).round().max(1.0) as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faster_actor_can_act_more_often_over_the_same_timeline() {
        let mut fast = ActionEnergy::default();
        let mut slow = ActionEnergy::default();
        let mut fast_actions = 0;
        let mut slow_actions = 0;

        for now in 0..=300 {
            if fast.is_ready(now) {
                fast_actions += 1;
                fast.spend(now, 60);
            }
            if slow.is_ready(now) {
                slow_actions += 1;
                slow.spend(now, 100);
            }
        }

        assert!(fast_actions > slow_actions);
    }
}
