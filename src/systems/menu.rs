use bevy_ecs::prelude::*;

use crate::resources::{GameScreen, MenuAction, MenuInputState, PauseMenuEntry, PauseMenuState};

pub fn menu_navigation(
    input: Res<MenuInputState>,
    mut screen: ResMut<GameScreen>,
    mut pause_menu: ResMut<PauseMenuState>,
) {
    let Some(action) = input.action else {
        return;
    };

    match (*screen, action) {
        (GameScreen::Playing, MenuAction::Cancel) => {
            *screen = GameScreen::PauseMenu;
        }
        (GameScreen::PauseMenu, MenuAction::Cancel) => {
            *screen = GameScreen::Playing;
        }
        (GameScreen::OptionsMenu, MenuAction::Cancel) => {
            *screen = GameScreen::PauseMenu;
        }
        (GameScreen::PauseMenu, MenuAction::MoveSelectionUp) => {
            pause_menu.select_previous();
        }
        (GameScreen::PauseMenu, MenuAction::MoveSelectionDown) => {
            pause_menu.select_next();
        }
        (GameScreen::PauseMenu, MenuAction::Confirm) => match pause_menu.selected() {
            PauseMenuEntry::Resume => *screen = GameScreen::Playing,
            PauseMenuEntry::Options => *screen = GameScreen::OptionsMenu,
        },
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_menu_action(world: &mut World, screen: GameScreen, action: MenuAction) {
        world.insert_resource(screen);
        world.insert_resource(PauseMenuState::default());
        world.insert_resource(MenuInputState {
            action: Some(action),
        });

        let mut schedule = Schedule::default();
        schedule.add_systems(menu_navigation);
        schedule.run(world);
    }

    #[test]
    fn escape_opens_and_closes_pause_menu() {
        let mut world = World::new();

        run_menu_action(&mut world, GameScreen::Playing, MenuAction::Cancel);
        assert_eq!(*world.resource::<GameScreen>(), GameScreen::PauseMenu);

        world.insert_resource(MenuInputState {
            action: Some(MenuAction::Cancel),
        });
        let mut schedule = Schedule::default();
        schedule.add_systems(menu_navigation);
        schedule.run(&mut world);

        assert_eq!(*world.resource::<GameScreen>(), GameScreen::Playing);
    }

    #[test]
    fn pause_menu_confirm_can_open_options() {
        let mut world = World::new();

        run_menu_action(
            &mut world,
            GameScreen::PauseMenu,
            MenuAction::MoveSelectionDown,
        );
        assert_eq!(
            world.resource::<PauseMenuState>().selected(),
            PauseMenuEntry::Options
        );

        world.insert_resource(MenuInputState {
            action: Some(MenuAction::Confirm),
        });
        let mut schedule = Schedule::default();
        schedule.add_systems(menu_navigation);
        schedule.run(&mut world);

        assert_eq!(*world.resource::<GameScreen>(), GameScreen::OptionsMenu);
    }
}
