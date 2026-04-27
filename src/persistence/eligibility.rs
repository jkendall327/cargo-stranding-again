use bevy_ecs::prelude::*;

use crate::{
    components::{Momentum, Player, Velocity},
    resources::GameScreen,
};

/// Whether the current world state is allowed to write a save.
///
/// Keeping this as simulation-domain logic gives menu and debug UI a single
/// place to ask, and avoids letting persistence rules leak into rendering code.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SaveEligibility {
    Eligible,
    MissingGameScreen,
    NotInGameplay { screen: GameScreen },
    NoPlayer,
    MultiplePlayers,
    MissingPlayerVelocity,
    MissingPlayerMomentum,
    PlayerMoving { dx: i32, dy: i32 },
    PlayerHasMomentum { momentum: Momentum },
}

impl SaveEligibility {
    pub const fn can_save(self) -> bool {
        matches!(self, Self::Eligible)
    }
}

/// Returns the current reason a player can or cannot save.
///
/// The first rule is intentionally conservative: saving is allowed only while
/// the player exists, gameplay simulation is active, and the player's transient
/// motion state is settled.
pub fn player_can_save(world: &mut World) -> SaveEligibility {
    let Some(screen) = world.get_resource::<GameScreen>().copied() else {
        return SaveEligibility::MissingGameScreen;
    };
    if !screen.allows_simulation() {
        return SaveEligibility::NotInGameplay { screen };
    }

    let mut query = world.query_filtered::<(Option<&Velocity>, Option<&Momentum>), With<Player>>();
    let mut players = query.iter(world);
    let Some((velocity, momentum)) = players.next() else {
        return SaveEligibility::NoPlayer;
    };
    if players.next().is_some() {
        return SaveEligibility::MultiplePlayers;
    }

    let Some(velocity) = velocity else {
        return SaveEligibility::MissingPlayerVelocity;
    };
    if velocity.dx != 0 || velocity.dy != 0 {
        return SaveEligibility::PlayerMoving {
            dx: velocity.dx,
            dy: velocity.dy,
        };
    }

    let Some(momentum) = momentum else {
        return SaveEligibility::MissingPlayerMomentum;
    };
    if *momentum != Momentum::default() {
        return SaveEligibility::PlayerHasMomentum {
            momentum: *momentum,
        };
    }

    SaveEligibility::Eligible
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        map::Map,
        resources::Direction,
        world_setup::{init_resources, init_world},
    };

    #[test]
    fn initial_world_can_save() {
        let mut world = World::new();
        init_world(&mut world);

        assert_eq!(player_can_save(&mut world), SaveEligibility::Eligible);
        assert!(player_can_save(&mut world).can_save());
    }

    #[test]
    fn missing_player_cannot_save() {
        let mut world = World::new();
        init_resources(&mut world, Map::generate());

        assert_eq!(player_can_save(&mut world), SaveEligibility::NoPlayer);
    }

    #[test]
    fn paused_game_cannot_save() {
        let mut world = World::new();
        init_world(&mut world);
        *world.resource_mut::<GameScreen>() = GameScreen::PauseMenu;

        assert_eq!(
            player_can_save(&mut world),
            SaveEligibility::NotInGameplay {
                screen: GameScreen::PauseMenu
            }
        );
    }

    #[test]
    fn player_velocity_blocks_saving() {
        let mut world = World::new();
        init_world(&mut world);
        let player = player_entity(&mut world);
        *world
            .get_mut::<Velocity>(player)
            .expect("test player should have velocity") = Velocity { dx: 1, dy: 0 };

        assert_eq!(
            player_can_save(&mut world),
            SaveEligibility::PlayerMoving { dx: 1, dy: 0 }
        );
    }

    #[test]
    fn player_momentum_blocks_saving() {
        let mut world = World::new();
        init_world(&mut world);
        let player = player_entity(&mut world);
        *world
            .get_mut::<Momentum>(player)
            .expect("test player should have momentum") = Momentum {
            direction: Some(Direction::East),
            amount: 1.0,
        };

        assert_eq!(
            player_can_save(&mut world),
            SaveEligibility::PlayerHasMomentum {
                momentum: Momentum {
                    direction: Some(Direction::East),
                    amount: 1.0
                }
            }
        );
    }

    fn player_entity(world: &mut World) -> Entity {
        let mut query = world.query_filtered::<Entity, With<Player>>();
        query
            .single(world)
            .expect("test world should have exactly one player")
    }
}
