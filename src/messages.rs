use bevy_ecs::prelude::*;

use crate::systems::{
    CargoActionResult, CycleMovementRequest, DeliverRequest, DropRequest, PickUpRequest,
    WaitRequest,
};

/// Registers every buffered message type used by simulation schedules.
///
/// Standalone `bevy_ecs` does not have a Bevy `App` to install message
/// resources for us, so this keeps the game's message registry in one place.
pub fn init_simulation_messages(world: &mut World) {
    world.init_resource::<Messages<WaitRequest>>();
    world.init_resource::<Messages<CycleMovementRequest>>();
    world.init_resource::<Messages<PickUpRequest>>();
    world.init_resource::<Messages<DropRequest>>();
    world.init_resource::<Messages<DeliverRequest>>();
    world.init_resource::<Messages<CargoActionResult>>();
}

/// Advances the short-lived action request message buffers.
pub fn maintain_action_request_messages(
    mut wait_requests: ResMut<Messages<WaitRequest>>,
    mut cycle_movement_requests: ResMut<Messages<CycleMovementRequest>>,
) {
    wait_requests.update();
    cycle_movement_requests.update();
}

/// Advances cargo request and result message buffers after cargo resolution.
pub fn maintain_cargo_messages(
    mut pickup_requests: ResMut<Messages<PickUpRequest>>,
    mut drop_requests: ResMut<Messages<DropRequest>>,
    mut deliver_requests: ResMut<Messages<DeliverRequest>>,
    mut results: ResMut<Messages<CargoActionResult>>,
) {
    pickup_requests.update();
    drop_requests.update();
    deliver_requests.update();
    results.update();
}
