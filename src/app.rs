use bevy_ecs::prelude::*;
use macroquad::prelude::*;

use crate::render;
use crate::resources::{GameScreen, PlayerIntent};
use crate::schedules;
use crate::world_setup::init_world;

pub struct Game {
    world: World,
    player_schedule: Schedule,
    menu_schedule: Schedule,
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

impl Game {
    pub fn new() -> Self {
        let mut world = World::new();
        init_world(&mut world);

        let player_schedule = schedules::player_intent_schedule();
        let menu_schedule = schedules::menu_schedule();

        tracing::debug!("created game schedules");

        Self {
            world,
            player_schedule,
            menu_schedule,
        }
    }

    pub async fn run(&mut self) {
        loop {
            self.run_frame();
            next_frame().await;
        }
    }

    fn run_frame(&mut self) {
        // Macroquad owns the outer async frame loop and immediate-mode input.
        // Each frame we copy only the compact input intent into an ECS resource.
        crate::input::copy_to_ecs(&mut self.world);
        self.menu_schedule.run(&mut self.world);

        // Bevy ECS owns simulation state. The energy timeline stays
        // input-paced for now: player input advances time, and NPCs catch up
        // to the player's next ready moment.
        if self.world.resource::<GameScreen>().allows_simulation()
            && self.world.resource::<PlayerIntent>().has_action()
        {
            self.player_schedule.run(&mut self.world);
        }

        // Rendering is deliberately a plain Macroquad function that manually
        // queries ECS state. This keeps drawing separate from deterministic sim.
        render::render(&mut self.world);
    }
}
