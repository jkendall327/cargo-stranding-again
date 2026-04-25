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

pub(crate) fn autonomous_actor_phase_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            systems::update_porter_action_interest,
            systems::assign_porter_jobs,
            systems::porter_jobs,
        )
            .chain(),
    );
    schedule
}

pub fn menu_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems((systems::menu_navigation, systems::inventory_actions).chain());
    schedule
}
