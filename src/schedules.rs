use bevy_ecs::prelude::*;

use crate::systems;

pub(crate) fn player_action_phase_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            systems::reset_cargo_loss_risk,
            systems::player_actions,
            systems::resolve_cargo_loss_risk,
        )
            .chain(),
    );
    schedule
}

pub(crate) fn agent_phase_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems((systems::assign_agent_jobs, systems::agent_jobs));
    schedule
}

pub fn menu_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems((systems::menu_navigation, systems::inventory_actions).chain());
    schedule
}
