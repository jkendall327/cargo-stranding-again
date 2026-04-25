use crate::movement::MovementMode;
use crate::resources::Direction;

pub const MOMENTUM_MAX: f32 = 10.0;
const WALK_MOMENTUM_GAIN: f32 = 1.0;
const SPRINT_MOMENTUM_GAIN: f32 = 3.0;
const STEADY_MOMENTUM_DECAY: f32 = 4.0;
const WAIT_MOMENTUM_DECAY: f32 = 2.0;
const MAX_STRAIGHT_ENERGY_DISCOUNT: f32 = 0.35;
const TURN_ENERGY_PENALTY_PER_MOMENTUM: f32 = 0.05;
const TURN_STAMINA_PENALTY_PER_MOMENTUM: f32 = 0.35;
const HIGH_MOMENTUM_TURN_RISK_START: f32 = 5.0;
const TURN_RISK_PER_MOMENTUM: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MomentumEffect {
    pub momentum: MomentumState,
    pub energy_multiplier: f32,
    pub stamina_delta: f32,
    pub cargo_loss_risk: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MomentumState {
    pub direction: Option<Direction>,
    pub amount: f32,
}

pub fn movement_effect(
    previous: MomentumState,
    direction: Direction,
    mode: MovementMode,
) -> MomentumEffect {
    let was_moving = previous.amount > 0.0;
    let continuing_straight = was_moving && previous.direction == Some(direction);
    let changing_direction = was_moving && previous.direction != Some(direction);

    let energy_multiplier = if continuing_straight {
        let discount =
            (previous.amount / MOMENTUM_MAX).clamp(0.0, 1.0) * MAX_STRAIGHT_ENERGY_DISCOUNT;
        1.0 - discount
    } else if changing_direction {
        1.0 + previous.amount * TURN_ENERGY_PENALTY_PER_MOMENTUM
    } else {
        1.0
    };

    let stamina_delta = if changing_direction {
        -(previous.amount * TURN_STAMINA_PENALTY_PER_MOMENTUM)
    } else {
        0.0
    };

    let cargo_loss_risk = if changing_direction && previous.amount >= HIGH_MOMENTUM_TURN_RISK_START
    {
        (previous.amount * TURN_RISK_PER_MOMENTUM).round() as u32
    } else {
        0
    };

    MomentumEffect {
        momentum: moved_momentum(previous, direction, mode),
        energy_multiplier,
        stamina_delta,
        cargo_loss_risk,
    }
}

pub fn wait_momentum(previous: MomentumState) -> MomentumState {
    decay(previous, WAIT_MOMENTUM_DECAY)
}

fn moved_momentum(
    previous: MomentumState,
    requested_direction: Direction,
    mode: MovementMode,
) -> MomentumState {
    match mode {
        MovementMode::Walking => build(previous, requested_direction, WALK_MOMENTUM_GAIN),
        MovementMode::Sprinting => build(previous, requested_direction, SPRINT_MOMENTUM_GAIN),
        MovementMode::Steady => decay(previous, STEADY_MOMENTUM_DECAY),
    }
}

fn build(previous: MomentumState, direction: Direction, amount: f32) -> MomentumState {
    MomentumState {
        direction: Some(direction),
        amount: (previous.amount + amount).min(MOMENTUM_MAX),
    }
}

fn decay(previous: MomentumState, amount: f32) -> MomentumState {
    let decayed = (previous.amount - amount).max(0.0);
    MomentumState {
        direction: (decayed > 0.0).then_some(previous.direction).flatten(),
        amount: decayed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walking_builds_small_momentum() {
        let effect = movement_effect(
            MomentumState::default(),
            Direction::East,
            MovementMode::Walking,
        );

        assert_eq!(effect.momentum.direction, Some(Direction::East));
        assert_eq!(effect.momentum.amount, 1.0);
    }

    #[test]
    fn sprinting_builds_more_momentum() {
        let walking = movement_effect(
            MomentumState::default(),
            Direction::East,
            MovementMode::Walking,
        );
        let sprinting = movement_effect(
            MomentumState::default(),
            Direction::East,
            MovementMode::Sprinting,
        );

        assert!(sprinting.momentum.amount > walking.momentum.amount);
    }

    #[test]
    fn steady_decays_momentum() {
        let effect = movement_effect(
            MomentumState {
                direction: Some(Direction::East),
                amount: 6.0,
            },
            Direction::East,
            MovementMode::Steady,
        );

        assert_eq!(effect.momentum.amount, 2.0);
        assert_eq!(effect.momentum.direction, Some(Direction::East));
    }

    #[test]
    fn continuing_straight_lowers_energy_cost() {
        let effect = movement_effect(
            MomentumState {
                direction: Some(Direction::East),
                amount: 5.0,
            },
            Direction::East,
            MovementMode::Walking,
        );

        assert!(effect.energy_multiplier < 1.0);
        assert_eq!(effect.cargo_loss_risk, 0);
    }

    #[test]
    fn turning_with_momentum_adds_penalty_and_risk() {
        let effect = movement_effect(
            MomentumState {
                direction: Some(Direction::East),
                amount: 5.0,
            },
            Direction::South,
            MovementMode::Walking,
        );

        assert!(effect.energy_multiplier > 1.0);
        assert!(effect.stamina_delta < 0.0);
        assert_eq!(effect.cargo_loss_risk, 100);
    }

    #[test]
    fn wait_decays_momentum() {
        let momentum = wait_momentum(MomentumState {
            direction: Some(Direction::East),
            amount: 3.0,
        });

        assert_eq!(momentum.direction, Some(Direction::East));
        assert_eq!(momentum.amount, 1.0);
    }
}
