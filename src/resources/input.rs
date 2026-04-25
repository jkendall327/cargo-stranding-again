use bevy_ecs::prelude::*;

pub const INPUT_REPEAT_INITIAL_DELAY: f64 = 0.18;
pub const INPUT_REPEAT_INTERVAL: f64 = 0.08;

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

#[cfg(test)]
mod tests {
    use super::*;

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
