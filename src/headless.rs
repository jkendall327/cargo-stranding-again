use bevy_ecs::prelude::*;

use crate::app::init_world;
use crate::components::{Cargo, CargoParcel, ParcelState, Player, Position, Stamina};
use crate::resources::{
    Direction, EnergyTimeline, GameScreen, PlayerAction, PlayerIntent, SimulationClock,
};
use crate::systems;

pub struct HeadlessGame {
    world: World,
    player_schedule: Schedule,
}

impl HeadlessGame {
    pub fn new() -> Self {
        let mut world = World::new();
        init_world(&mut world);

        let mut player_schedule = Schedule::default();
        player_schedule.add_systems(systems::advance_timeline_for_player_intent);

        Self {
            world,
            player_schedule,
        }
    }

    pub fn step(&mut self, action: PlayerAction) {
        *self.world.resource_mut::<PlayerIntent>() = PlayerIntent {
            action: Some(action),
        };

        if self.world.resource::<GameScreen>().allows_simulation() {
            self.player_schedule.run(&mut self.world);
        }

        *self.world.resource_mut::<PlayerIntent>() = PlayerIntent::default();
    }

    pub fn snapshot(&mut self) -> Option<HeadlessSnapshot> {
        HeadlessSnapshot::from_world(&mut self.world)
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }
}

impl Default for HeadlessGame {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadlessSnapshot {
    pub turn: u64,
    pub timeline: u64,
    pub delivered_parcels: u32,
    pub player_position: Position,
    pub player_stamina: f32,
    pub player_cargo: f32,
    pub loose_parcels: usize,
    pub assigned_parcels: usize,
    pub carried_parcels: usize,
}

impl HeadlessSnapshot {
    fn from_world(world: &mut World) -> Option<Self> {
        let clock = *world.resource::<SimulationClock>();
        let timeline = world.resource::<EnergyTimeline>().now;

        let (player_position, player_stamina, player_cargo) = {
            let mut player_query =
                world.query_filtered::<(&Position, &Stamina, &Cargo), With<Player>>();
            let (position, stamina, cargo) = player_query.iter(world).next()?;
            (*position, stamina.current, cargo.current_weight)
        };

        let mut loose_parcels = 0;
        let mut assigned_parcels = 0;
        let mut carried_parcels = 0;
        let mut parcel_query = world.query_filtered::<&ParcelState, With<CargoParcel>>();
        for state in parcel_query.iter(world) {
            match state {
                ParcelState::Loose => loose_parcels += 1,
                ParcelState::AssignedTo(_) => assigned_parcels += 1,
                ParcelState::CarriedBy(_) => carried_parcels += 1,
                ParcelState::Delivered => {}
            }
        }

        Some(Self {
            turn: clock.turn,
            timeline,
            delivered_parcels: clock.delivered_parcels,
            player_position,
            player_stamina,
            player_cargo,
            loose_parcels,
            assigned_parcels,
            carried_parcels,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeadlessCommand {
    Action(PlayerAction),
}

impl HeadlessCommand {
    pub fn from_token(token: &str) -> Option<Self> {
        let action = match token {
            "north" | "n" | "up" => PlayerAction::Move(Direction::North),
            "south" | "s" | "down" => PlayerAction::Move(Direction::South),
            "west" | "w" | "left" => PlayerAction::Move(Direction::West),
            "east" | "e" | "right" => PlayerAction::Move(Direction::East),
            "wait" | "." => PlayerAction::Wait,
            "pickup" | "pick-up" | "pick_up" => PlayerAction::PickUp,
            "sprint" | "toggle-sprint" | "toggle_sprint" => PlayerAction::ToggleSprint,
            _ => return None,
        };
        Some(Self::Action(action))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stepping_wait_advances_the_headless_simulation() {
        let mut game = HeadlessGame::new();

        game.step(PlayerAction::Wait);

        let snapshot = game.snapshot().expect("headless game should have a player");
        assert_eq!(snapshot.turn, 1);
        assert!(snapshot.timeline > 0);
    }

    #[test]
    fn parser_accepts_direction_shortcuts() {
        assert_eq!(
            HeadlessCommand::from_token("e"),
            Some(HeadlessCommand::Action(PlayerAction::Move(Direction::East)))
        );
        assert_eq!(
            HeadlessCommand::from_token("pickup"),
            Some(HeadlessCommand::Action(PlayerAction::PickUp))
        );
        assert_eq!(HeadlessCommand::from_token("???"), None);
    }
}
