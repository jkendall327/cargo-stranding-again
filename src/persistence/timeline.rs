use serde::{Deserialize, Serialize};

/// Persisted global timeline plus per-actor action energy snapshots.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedTimeline {
    pub now: u64,
    pub actors: Vec<SavedActionEnergy>,
}

/// Actor scheduling state in energy timeline space.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedActionEnergy {
    pub ready_at: u64,
    pub last_cost: u32,
}
