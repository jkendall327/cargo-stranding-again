use bevy_ecs::prelude::*;
use macroquad::prelude::*;

use crate::components::*;
use crate::energy::ActionEnergy;
use crate::input::KeyBindings;
use crate::map::Map;
use crate::render;
use crate::resources::{
    Camera, EnergyTimeline, GameScreen, InputRepeat, MenuInputState, PauseMenuState, PlayerIntent,
    SimulationClock,
};
use crate::systems;

pub struct Game {
    world: World,
    player_schedule: Schedule,
    menu_schedule: Schedule,
}

impl Game {
    pub fn new() -> Self {
        let mut world = World::new();
        init_world(&mut world);

        let mut player_schedule = Schedule::default();
        player_schedule.add_systems(systems::advance_timeline_for_player_intent);

        let mut menu_schedule = Schedule::default();
        menu_schedule.add_systems(systems::menu_navigation);

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

fn init_world(world: &mut World) {
    world.insert_resource(Map::generate());
    world.insert_resource(GameScreen::default());
    world.insert_resource(PlayerIntent::default());
    world.insert_resource(MenuInputState::default());
    world.insert_resource(InputRepeat::default());
    world.insert_resource(KeyBindings::default());
    world.insert_resource(PauseMenuState::default());
    world.insert_resource(EnergyTimeline::default());
    world.insert_resource(Camera::default());
    world.insert_resource(SimulationClock {
        turn: 0,
        delivered_parcels: 0,
    });

    world.spawn((
        Player,
        Position { x: 6, y: 6 },
        Velocity::default(),
        Cargo {
            current_weight: 12.0,
            max_weight: 40.0,
        },
        Stamina {
            current: 35.0,
            max: 35.0,
        },
        MovementState::default(),
        ActionEnergy::default(),
    ));

    for (id, (x, y)) in [(0, (41, 30)), (1, (52, 26))] {
        world.spawn((
            Agent { id },
            Position { x, y },
            Velocity::default(),
            Cargo {
                current_weight: 0.0,
                max_weight: 35.0,
            },
            AssignedJob {
                phase: JobPhase::FindParcel,
                parcel: None,
            },
            ActionEnergy::default(),
        ));
    }

    for (x, y, weight) in [
        (8, 8, 6.0),
        (18, 15, 9.0),
        (26, 33, 5.0),
        (36, 9, 8.0),
        (55, 19, 7.0),
    ] {
        world.spawn((
            Position { x, y },
            CargoParcel { weight },
            ParcelState::Loose,
        ));
    }
}
