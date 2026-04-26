use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ApplyDeferred;

use crate::systems;

pub(crate) fn player_action_phase_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            systems::reset_cargo_loss_risk,
            systems::open_inventory_from_player_intent,
            systems::emit_player_cycle_movement_request,
            systems::resolve_cycle_movement_requests,
            systems::maintain_cycle_movement_requests,
            systems::pick_up_player_parcel_from_intent,
            systems::emit_player_wait_request,
            systems::resolve_wait_requests,
            systems::maintain_wait_requests,
            systems::player_actions,
            systems::resolve_cargo_loss_risk,
            systems::resolve_cargo_requests,
            ApplyDeferred,
            systems::refresh_changed_cargo_caches,
            systems::handle_cargo_action_results,
            systems::maintain_cargo_messages,
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
            systems::resolve_cargo_requests,
            ApplyDeferred,
            systems::refresh_changed_cargo_caches,
            systems::handle_cargo_action_results,
            systems::maintain_cargo_messages,
        )
            .chain(),
    );
    schedule
}

pub fn menu_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            systems::menu_navigation,
            systems::inventory_actions,
            systems::resolve_cargo_requests,
            ApplyDeferred,
            systems::refresh_changed_cargo_caches,
            systems::handle_cargo_action_results,
            systems::maintain_cargo_messages,
        )
            .chain(),
    );
    schedule
}
