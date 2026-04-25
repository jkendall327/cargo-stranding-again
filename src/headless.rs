use bevy_ecs::prelude::*;
use serde::Deserialize;

use crate::app::init_world;
use crate::components::{
    Cargo, CargoParcel, MovementState, ParcelState, Player, Position, Stamina,
};
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

        tracing::debug!("created headless game");

        Self {
            world,
            player_schedule,
        }
    }

    pub fn step(&mut self, action: PlayerAction) {
        tracing::debug!(?action, "headless step");

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
    pub player_movement_mode: crate::movement::MovementMode,
    pub player_cargo: f32,
    pub loose_parcels: usize,
    pub assigned_parcels: usize,
    pub carried_parcels: usize,
}

impl HeadlessSnapshot {
    fn from_world(world: &mut World) -> Option<Self> {
        let clock = *world.resource::<SimulationClock>();
        let timeline = world.resource::<EnergyTimeline>().now;

        let (player_position, player_stamina, player_movement_mode, player_cargo) = {
            let mut player_query = world
                .query_filtered::<(&Position, &Stamina, &MovementState, &Cargo), With<Player>>();
            let (position, stamina, movement_state, cargo) = player_query.iter(world).next()?;
            (
                *position,
                stamina.current,
                movement_state.mode,
                cargo.current_weight,
            )
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
            player_movement_mode,
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
            "inventory" | "inv" => PlayerAction::OpenInventory,
            "pickup" | "pick-up" | "pick_up" => PlayerAction::PickUp,
            "mode" | "movement" | "cycle-mode" | "cycle_mode" | "sprint" | "toggle-sprint"
            | "toggle_sprint" => PlayerAction::CycleMovementMode,
            _ => return None,
        };
        Some(Self::Action(action))
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct HeadlessScenario {
    pub name: Option<String>,
    #[serde(default)]
    pub commands: Vec<ScenarioCommand>,
    #[serde(default)]
    pub expect: HeadlessExpect,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum ScenarioCommand {
    Token(String),
    Repeat {
        repeat: usize,
        command: Box<ScenarioCommand>,
    },
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct HeadlessExpect {
    pub turn: Option<u64>,
    #[serde(alias = "time")]
    pub timeline: Option<u64>,
    pub delivered_parcels: Option<u32>,
    #[serde(alias = "player")]
    pub player_position: Option<ExpectedPosition>,
    pub player_stamina: Option<f32>,
    pub player_movement_mode: Option<ExpectedMovementMode>,
    pub player_cargo: Option<f32>,
    pub loose_parcels: Option<usize>,
    pub assigned_parcels: Option<usize>,
    pub carried_parcels: Option<usize>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct ExpectedPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExpectedMovementMode {
    Walking,
    Sprinting,
    Steady,
}

pub struct HeadlessScenarioReport {
    pub name: String,
    pub final_snapshot: HeadlessSnapshot,
    pub failures: Vec<ExpectationFailure>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectationFailure {
    pub field: &'static str,
    pub expected: String,
    pub actual: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HeadlessScenarioError {
    UnknownCommand { command: String },
    NoPlayer,
}

impl HeadlessScenario {
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or("unnamed scenario")
    }
}

pub fn run_scenario(
    scenario: &HeadlessScenario,
) -> Result<HeadlessScenarioReport, HeadlessScenarioError> {
    let mut game = HeadlessGame::new();
    for command in scenario.commands.iter().flat_map(expand_command) {
        let Some(parsed) = HeadlessCommand::from_token(&command) else {
            return Err(HeadlessScenarioError::UnknownCommand { command });
        };

        match parsed {
            HeadlessCommand::Action(action) => game.step(action),
        }
    }

    let final_snapshot = game.snapshot().ok_or(HeadlessScenarioError::NoPlayer)?;
    let failures = scenario.expect.compare(final_snapshot);
    Ok(HeadlessScenarioReport {
        name: scenario.display_name().to_owned(),
        final_snapshot,
        failures,
    })
}

fn expand_command(command: &ScenarioCommand) -> Vec<String> {
    match command {
        ScenarioCommand::Token(token) => vec![token.clone()],
        ScenarioCommand::Repeat { repeat, command } => {
            let expanded = expand_command(command);
            expanded
                .iter()
                .cycle()
                .take(expanded.len() * *repeat)
                .cloned()
                .collect()
        }
    }
}

impl HeadlessExpect {
    pub fn compare(self, snapshot: HeadlessSnapshot) -> Vec<ExpectationFailure> {
        let mut failures = Vec::new();

        expect_eq(&mut failures, "turn", self.turn, snapshot.turn);
        expect_eq(&mut failures, "timeline", self.timeline, snapshot.timeline);
        expect_eq(
            &mut failures,
            "delivered_parcels",
            self.delivered_parcels,
            snapshot.delivered_parcels,
        );
        if let Some(expected) = self.player_position {
            let actual = snapshot.player_position;
            if actual.x != expected.x || actual.y != expected.y {
                failures.push(ExpectationFailure {
                    field: "player_position",
                    expected: format!("{},{}", expected.x, expected.y),
                    actual: format!("{},{}", actual.x, actual.y),
                });
            }
        }
        expect_f32(
            &mut failures,
            "player_stamina",
            self.player_stamina,
            snapshot.player_stamina,
        );
        if let Some(expected) = self.player_movement_mode {
            let actual = snapshot.player_movement_mode.label();
            if actual != expected.label() {
                failures.push(ExpectationFailure {
                    field: "player_movement_mode",
                    expected: expected.label().to_owned(),
                    actual: actual.to_owned(),
                });
            }
        }
        expect_f32(
            &mut failures,
            "player_cargo",
            self.player_cargo,
            snapshot.player_cargo,
        );
        expect_eq(
            &mut failures,
            "loose_parcels",
            self.loose_parcels,
            snapshot.loose_parcels,
        );
        expect_eq(
            &mut failures,
            "assigned_parcels",
            self.assigned_parcels,
            snapshot.assigned_parcels,
        );
        expect_eq(
            &mut failures,
            "carried_parcels",
            self.carried_parcels,
            snapshot.carried_parcels,
        );

        failures
    }
}

impl ExpectedMovementMode {
    fn label(self) -> &'static str {
        match self {
            Self::Walking => "walking",
            Self::Sprinting => "sprinting",
            Self::Steady => "steady",
        }
    }
}

fn expect_eq<T>(
    failures: &mut Vec<ExpectationFailure>,
    field: &'static str,
    expected: Option<T>,
    actual: T,
) where
    T: Eq + std::fmt::Display,
{
    if let Some(expected) = expected {
        if expected != actual {
            failures.push(ExpectationFailure {
                field,
                expected: expected.to_string(),
                actual: actual.to_string(),
            });
        }
    }
}

fn expect_f32(
    failures: &mut Vec<ExpectationFailure>,
    field: &'static str,
    expected: Option<f32>,
    actual: f32,
) {
    const EPSILON: f32 = 0.05;
    if let Some(expected) = expected {
        if (expected - actual).abs() > EPSILON {
            failures.push(ExpectationFailure {
                field,
                expected: format!("{expected:.1}"),
                actual: format!("{actual:.1}"),
            });
        }
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

    #[test]
    fn scenario_runs_commands_and_checks_expectations() {
        let scenario: HeadlessScenario = serde_json::from_str(
            r#"{
                "name": "walk east",
                "commands": ["east"],
                "expect": {
                    "turn": 1,
                    "timeline": 100,
                    "player_position": { "x": 7, "y": 6 },
                    "player_movement_mode": "walking"
                }
            }"#,
        )
        .expect("scenario should parse");

        let report = run_scenario(&scenario).expect("scenario should run");

        assert!(report.failures.is_empty());
    }

    #[test]
    fn scenario_supports_structured_repeat_commands() {
        let scenario: HeadlessScenario = serde_json::from_str(
            r#"{
                "commands": [
                    { "repeat": 2, "command": "east" }
                ],
                "expect": {
                    "player": { "x": 8, "y": 6 },
                    "turn": 2
                }
            }"#,
        )
        .expect("scenario should parse");

        let report = run_scenario(&scenario).expect("scenario should run");

        assert!(report.failures.is_empty());
    }

    #[test]
    fn scenario_reports_expectation_failures() {
        let scenario: HeadlessScenario = serde_json::from_str(
            r#"{
                "commands": ["east"],
                "expect": {
                    "turn": 99
                }
            }"#,
        )
        .expect("scenario should parse");

        let report = run_scenario(&scenario).expect("scenario should run");

        assert_eq!(
            report.failures,
            vec![ExpectationFailure {
                field: "turn",
                expected: "99".to_owned(),
                actual: "1".to_owned()
            }]
        );
    }
}
