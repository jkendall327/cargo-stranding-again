use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ApplyDeferred;

use crate::systems;

pub(crate) fn player_action_phase_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            (
                systems::reset_cargo_loss_risk,
                systems::open_inventory_from_player_intent,
                systems::emit_player_cycle_movement_request,
                systems::resolve_cycle_movement_requests,
                systems::pick_up_player_item_from_intent,
                systems::emit_player_wait_request,
                systems::resolve_wait_requests,
                systems::player_actions,
                systems::resolve_cargo_loss_risk,
                crate::messages::maintain_action_request_messages,
            )
                .chain(),
            (
                systems::resolve_pickup_requests,
                systems::resolve_drop_requests,
                systems::resolve_delivery_requests,
                ApplyDeferred,
                systems::spend_energy_for_successful_cargo_actions,
                systems::update_porter_jobs_from_cargo_results,
                systems::clear_failed_porter_cargo_jobs,
                systems::clamp_inventory_after_cargo_drop,
                systems::log_failed_cargo_actions,
                crate::messages::maintain_cargo_messages,
            )
                .chain(),
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
            (
                systems::resolve_pickup_requests,
                systems::resolve_drop_requests,
                systems::resolve_delivery_requests,
                ApplyDeferred,
                systems::spend_energy_for_successful_cargo_actions,
                systems::update_porter_jobs_from_cargo_results,
                systems::clear_failed_porter_cargo_jobs,
                systems::clamp_inventory_after_cargo_drop,
                systems::log_failed_cargo_actions,
                crate::messages::maintain_cargo_messages,
            )
                .chain(),
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
            (
                systems::resolve_pickup_requests,
                systems::resolve_drop_requests,
                systems::resolve_delivery_requests,
                ApplyDeferred,
                systems::spend_energy_for_successful_cargo_actions,
                systems::update_porter_jobs_from_cargo_results,
                systems::clear_failed_porter_cargo_jobs,
                systems::clamp_inventory_after_cargo_drop,
                systems::log_failed_cargo_actions,
                crate::messages::maintain_cargo_messages,
            )
                .chain(),
        )
            .chain(),
    );
    schedule
}
