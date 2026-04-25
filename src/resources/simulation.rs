use bevy_ecs::prelude::*;

#[derive(Resource, Clone, Copy, Debug)]
pub struct SimulationClock {
    pub turn: u64,
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct DeliveryStats {
    pub delivered_parcels: u32,
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct EnergyTimeline {
    pub now: u64,
}

pub const CARGO_LOSS_RISK_THRESHOLD: u32 = 100;

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct CargoLossRisk {
    pub amount: u32,
}

impl CargoLossRisk {
    pub fn reset(&mut self) {
        self.amount = 0;
    }

    pub fn add(&mut self, amount: u32) {
        self.amount = self.amount.saturating_add(amount);
    }

    pub fn crosses_threshold(self) -> bool {
        self.amount >= CARGO_LOSS_RISK_THRESHOLD
    }
}
