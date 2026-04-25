use bevy_ecs::prelude::*;

use crate::systems;

pub fn player_intent_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems(systems::advance_timeline_for_player_intent);
    schedule
}

pub fn menu_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems((systems::menu_navigation, systems::inventory_actions).chain());
    schedule
}
