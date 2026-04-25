use bevy_ecs::prelude::*;

use crate::components::Position;
use crate::map::{MapBounds, TileCoord};

pub const DEFAULT_CAMERA_TILE_SPAN: i32 = 31;
pub const INPUT_REPEAT_INITIAL_DELAY: f64 = 0.18;
pub const INPUT_REPEAT_INTERVAL: f64 = 0.08;

#[derive(Resource, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GameScreen {
    #[default]
    Playing,
    PauseMenu,
    InventoryMenu,
    OptionsMenu,
}

impl GameScreen {
    pub fn allows_simulation(self) -> bool {
        matches!(self, Self::Playing)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    West,
    East,
    North,
    South,
}

impl Direction {
    pub fn delta(self) -> (i32, i32) {
        match self {
            Self::West => (-1, 0),
            Self::East => (1, 0),
            Self::North => (0, -1),
            Self::South => (0, 1),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::West => "west",
            Self::East => "east",
            Self::North => "north",
            Self::South => "south",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayerAction {
    Move(Direction),
    OpenInventory,
    PickUp,
    CycleMovementMode,
    Wait,
}

impl PlayerAction {
    pub fn repeats_while_held(self) -> bool {
        matches!(self, Self::Move(_) | Self::Wait)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    MoveSelectionUp,
    MoveSelectionDown,
    Confirm,
    Cancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PauseMenuEntry {
    Resume,
    Options,
}

impl PauseMenuEntry {
    pub const ALL: [Self; 2] = [Self::Resume, Self::Options];

    pub fn label(self) -> &'static str {
        match self {
            Self::Resume => "Resume",
            Self::Options => "Options",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MenuSelection {
    selected_index: usize,
}

impl MenuSelection {
    pub fn selected_index(self) -> usize {
        self.selected_index
    }

    pub fn select_next(&mut self, len: usize) {
        if len > 0 {
            self.selected_index = (self.selected_index + 1) % len;
        }
    }

    pub fn select_previous(&mut self, len: usize) {
        if len > 0 {
            self.selected_index = (self.selected_index + len - 1) % len;
        }
    }

    pub fn clamp_to_len(&mut self, len: usize) {
        if len == 0 {
            self.selected_index = 0;
        } else if self.selected_index >= len {
            self.selected_index = len - 1;
        }
    }
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct PauseMenuState {
    selection: MenuSelection,
}

impl PauseMenuState {
    pub fn selected(self) -> PauseMenuEntry {
        PauseMenuEntry::ALL[self.selection.selected_index()]
    }

    pub fn select_next(&mut self) {
        self.selection.select_next(PauseMenuEntry::ALL.len());
    }

    pub fn select_previous(&mut self) {
        self.selection.select_previous(PauseMenuEntry::ALL.len());
    }
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct InventoryMenuState {
    selection: MenuSelection,
}

impl InventoryMenuState {
    pub fn selected_index(self) -> usize {
        self.selection.selected_index()
    }

    pub fn select_next(&mut self, item_count: usize) {
        self.selection.select_next(item_count);
    }

    pub fn select_previous(&mut self, item_count: usize) {
        self.selection.select_previous(item_count);
    }

    pub fn clamp_to_item_count(&mut self, item_count: usize) {
        self.selection.clamp_to_len(item_count);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryAction {
    DropSelected,
}

#[derive(Resource, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InventoryIntent {
    pub action: Option<InventoryAction>,
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct MenuInputState {
    pub action: Option<MenuAction>,
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct InputRepeat {
    held_action: Option<PlayerAction>,
    next_repeat_at: f64,
}

impl InputRepeat {
    pub fn reset(&mut self) {
        self.held_action = None;
    }

    pub fn action_for_frame(
        &mut self,
        held_action: Option<PlayerAction>,
        newly_pressed_action: Option<PlayerAction>,
        now: f64,
    ) -> Option<PlayerAction> {
        if let Some(action) = newly_pressed_action {
            self.held_action = Some(action);
            self.next_repeat_at = now + INPUT_REPEAT_INITIAL_DELAY;
            return Some(action);
        }

        let Some(action) = held_action else {
            self.held_action = None;
            return None;
        };

        if self.held_action != Some(action) {
            self.held_action = Some(action);
            self.next_repeat_at = now + INPUT_REPEAT_INITIAL_DELAY;
            return Some(action);
        }

        if !action.repeats_while_held() {
            return None;
        }

        if now >= self.next_repeat_at {
            self.next_repeat_at = now + INPUT_REPEAT_INTERVAL;
            return Some(action);
        }

        None
    }
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct PlayerIntent {
    pub action: Option<PlayerAction>,
}

impl PlayerIntent {
    pub fn has_action(self) -> bool {
        self.action.is_some()
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct SimulationClock {
    pub turn: u64,
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct DeliveryStats {
    pub delivered_parcels: u32,
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct EnergyTimeline {
    pub now: u64,
}

pub const CARGO_LOSS_RISK_THRESHOLD: u32 = 100;

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct CargoLossRisk {
    pub amount: u32,
}

impl CargoLossRisk {
    pub fn reset(&mut self) {
        self.amount = 0;
    }

    pub fn add(&mut self, amount: u32) {
        self.amount = self.amount.saturating_add(amount);
    }

    pub fn crosses_threshold(self) -> bool {
        self.amount >= CARGO_LOSS_RISK_THRESHOLD
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct Camera {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Camera {
    pub fn square(tile_span: i32) -> Self {
        let tile_span = tile_span.max(1);
        Self {
            x: 0,
            y: 0,
            width: tile_span,
            height: tile_span,
        }
    }

    /// Centers the camera on an ECS position while clamping to finite map bounds.
    pub fn center_on(&mut self, position: Position, bounds: MapBounds) {
        let coord = TileCoord::from(position);
        self.x = clamp_axis(
            coord.x - self.width / 2,
            self.width,
            bounds.min_x,
            bounds.max_x(),
        );
        self.y = clamp_axis(
            coord.y - self.height / 2,
            self.height,
            bounds.min_y,
            bounds.max_y(),
        );
    }

    /// Returns the camera's top-left tile as a world tile coordinate.
    pub fn origin_coord(self) -> TileCoord {
        TileCoord::new(self.x, self.y)
    }

    pub fn contains(self, position: Position) -> bool {
        position.x >= self.x
            && position.y >= self.y
            && position.x < self.x + self.width
            && position.y < self.y + self.height
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::square(DEFAULT_CAMERA_TILE_SPAN)
    }
}

fn clamp_axis(origin: i32, view_size: i32, min: i32, max: i32) -> i32 {
    let map_size = max - min;
    if view_size >= map_size {
        min
    } else {
        origin.clamp(min, max - view_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_centers_on_player_until_it_hits_map_edges() {
        let mut camera = Camera::square(31);

        camera.center_on(Position { x: 30, y: 20 }, MapBounds::new(0, 0, 60, 40));
        assert_eq!((camera.x, camera.y), (15, 5));

        camera.center_on(Position { x: 2, y: 2 }, MapBounds::new(0, 0, 60, 40));
        assert_eq!((camera.x, camera.y), (0, 0));

        camera.center_on(Position { x: 58, y: 38 }, MapBounds::new(0, 0, 60, 40));
        assert_eq!((camera.x, camera.y), (29, 9));

        camera.center_on(
            Position { x: -14, y: -14 },
            MapBounds::new(-16, -16, 76, 56),
        );
        assert_eq!((camera.x, camera.y), (-16, -16));
    }

    #[test]
    fn camera_contains_positions_inside_visible_tiles() {
        let camera = Camera {
            x: 10,
            y: 5,
            width: 31,
            height: 31,
        };

        assert!(camera.contains(Position { x: 10, y: 5 }));
        assert!(camera.contains(Position { x: 40, y: 35 }));
        assert!(!camera.contains(Position { x: 41, y: 35 }));
        assert!(!camera.contains(Position { x: 40, y: 36 }));
    }

    #[test]
    fn input_repeat_emits_on_press_then_after_delay() {
        let mut repeat = InputRepeat::default();

        assert_eq!(
            repeat.action_for_frame(
                Some(PlayerAction::Move(Direction::North)),
                Some(PlayerAction::Move(Direction::North)),
                0.0
            ),
            Some(PlayerAction::Move(Direction::North))
        );
        assert_eq!(
            repeat.action_for_frame(Some(PlayerAction::Move(Direction::North)), None, 0.1),
            None
        );
        assert_eq!(
            repeat.action_for_frame(
                Some(PlayerAction::Move(Direction::North)),
                None,
                INPUT_REPEAT_INITIAL_DELAY
            ),
            Some(PlayerAction::Move(Direction::North))
        );
        assert_eq!(
            repeat.action_for_frame(
                Some(PlayerAction::Move(Direction::North)),
                None,
                INPUT_REPEAT_INITIAL_DELAY + INPUT_REPEAT_INTERVAL
            ),
            Some(PlayerAction::Move(Direction::North))
        );
    }

    #[test]
    fn input_repeat_resets_when_key_is_released() {
        let mut repeat = InputRepeat::default();

        assert_eq!(
            repeat.action_for_frame(Some(PlayerAction::Wait), Some(PlayerAction::Wait), 0.0),
            Some(PlayerAction::Wait)
        );
        assert_eq!(repeat.action_for_frame(None, None, 0.05), None);
        assert_eq!(
            repeat.action_for_frame(Some(PlayerAction::Wait), Some(PlayerAction::Wait), 0.06),
            Some(PlayerAction::Wait)
        );
    }

    #[test]
    fn non_repeatable_actions_do_not_repeat_while_held() {
        let mut repeat = InputRepeat::default();

        assert_eq!(
            repeat.action_for_frame(
                Some(PlayerAction::CycleMovementMode),
                Some(PlayerAction::CycleMovementMode),
                0.0
            ),
            Some(PlayerAction::CycleMovementMode)
        );
        assert_eq!(
            repeat.action_for_frame(Some(PlayerAction::CycleMovementMode), None, 1.0),
            None
        );
    }
}
