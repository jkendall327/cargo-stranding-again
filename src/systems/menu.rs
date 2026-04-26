use bevy_ecs::prelude::*;

use crate::cargo::player_carried_parcel_count;
use crate::resources::{
    GameScreen, InventoryAction, InventoryIntent, InventoryMenuState, MenuAction, MenuInputState,
    PauseMenuEntry, PauseMenuState,
};

pub fn menu_navigation(world: &mut World) {
    let Some(action) = world.resource::<MenuInputState>().action else {
        return;
    };

    match (*world.resource::<GameScreen>(), action) {
        (GameScreen::Playing, MenuAction::Cancel) => {
            *world.resource_mut::<GameScreen>() = GameScreen::PauseMenu;
        }
        (GameScreen::PauseMenu, MenuAction::Cancel) => {
            *world.resource_mut::<GameScreen>() = GameScreen::Playing;
        }
        (GameScreen::InventoryMenu, MenuAction::Cancel) => {
            *world.resource_mut::<GameScreen>() = GameScreen::Playing;
        }
        (GameScreen::OptionsMenu, MenuAction::Cancel) => {
            *world.resource_mut::<GameScreen>() = GameScreen::PauseMenu;
        }
        (GameScreen::PauseMenu, MenuAction::MoveSelectionUp) => {
            world.resource_mut::<PauseMenuState>().select_previous();
        }
        (GameScreen::PauseMenu, MenuAction::MoveSelectionDown) => {
            world.resource_mut::<PauseMenuState>().select_next();
        }
        (GameScreen::PauseMenu, MenuAction::Confirm) => {
            match world.resource::<PauseMenuState>().selected() {
                PauseMenuEntry::Resume => *world.resource_mut::<GameScreen>() = GameScreen::Playing,
                PauseMenuEntry::Options => {
                    *world.resource_mut::<GameScreen>() = GameScreen::OptionsMenu
                }
            }
        }
        (GameScreen::InventoryMenu, MenuAction::MoveSelectionUp) => {
            let item_count = player_carried_parcel_count(world);
            world
                .resource_mut::<InventoryMenuState>()
                .select_previous(item_count);
        }
        (GameScreen::InventoryMenu, MenuAction::MoveSelectionDown) => {
            let item_count = player_carried_parcel_count(world);
            world
                .resource_mut::<InventoryMenuState>()
                .select_next(item_count);
        }
        (GameScreen::InventoryMenu, MenuAction::Confirm) => {
            world.resource_mut::<InventoryIntent>().action = Some(InventoryAction::DropSelected);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::{
        refresh_cargo_cache, Cargo, CargoParcel, CargoStats, CarriedBy, CarrySlot, Item,
        ParcelState,
    };
    use crate::components::{ActionEnergy, Player, Position};
    use crate::resources::{EnergyTimeline, InventoryIntent, PlayerIntent, SimulationClock};
    use crate::simulation::SimulationRunner;
    use crate::systems::inventory::inventory_actions;
    use crate::systems::{
        handle_cargo_action_results, maintain_cargo_messages, refresh_changed_cargo_caches,
        resolve_cargo_requests, CargoActionResult, CargoChanged, DeliverRequest, DropRequest,
        PickUpRequest,
    };
    use bevy_ecs::schedule::ApplyDeferred;

    fn spawn_test_player(world: &mut World, position: Position) -> Entity {
        world
            .spawn((
                Player,
                position,
                Cargo {
                    current_weight: 0.0,
                    max_weight: 40.0,
                },
                ActionEnergy::default(),
            ))
            .id()
    }

    fn spawn_carried_parcel(world: &mut World, holder: Entity, position: Position) -> Entity {
        let parcel = world
            .spawn((
                position,
                Item,
                CargoStats {
                    weight: 5.0,
                    volume: 1.0,
                },
                CarriedBy {
                    holder,
                    slot: CarrySlot::Back,
                },
                CargoParcel { weight: 5.0 },
                ParcelState::CarriedBy(holder),
            ))
            .id();
        refresh_cargo_cache(world, holder);
        parcel
    }

    fn setup_menu_world(world: &mut World, screen: GameScreen, action: MenuAction) {
        world.insert_resource(screen);
        world.insert_resource(PauseMenuState::default());
        world.insert_resource(InventoryMenuState::default());
        world.insert_resource(InventoryIntent::default());
        world.insert_resource(EnergyTimeline::default());
        world.insert_resource(crate::resources::DeliveryStats::default());
        world.init_resource::<Messages<PickUpRequest>>();
        world.init_resource::<Messages<DropRequest>>();
        world.init_resource::<Messages<DeliverRequest>>();
        world.init_resource::<Messages<CargoChanged>>();
        world.init_resource::<Messages<CargoActionResult>>();
        world.insert_resource(MenuInputState {
            action: Some(action),
        });
    }

    fn menu_with_inventory_resolution_schedule() -> Schedule {
        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                menu_navigation,
                inventory_actions,
                resolve_cargo_requests,
                ApplyDeferred,
                refresh_changed_cargo_caches,
                handle_cargo_action_results,
                maintain_cargo_messages,
            )
                .chain(),
        );
        schedule
    }

    fn run_menu_with_simulation(
        schedule: &mut Schedule,
        simulation: &mut SimulationRunner,
        world: &mut World,
    ) {
        schedule.run(world);
        simulation.advance_after_player_action_if_spent(world);
    }

    #[test]
    fn escape_opens_and_closes_pause_menu() {
        let mut world = World::new();

        setup_menu_world(&mut world, GameScreen::Playing, MenuAction::Cancel);
        let mut schedule = menu_with_inventory_resolution_schedule();
        schedule.run(&mut world);
        assert_eq!(*world.resource::<GameScreen>(), GameScreen::PauseMenu);

        world.insert_resource(MenuInputState {
            action: Some(MenuAction::Cancel),
        });
        schedule.run(&mut world);

        assert_eq!(*world.resource::<GameScreen>(), GameScreen::Playing);
    }

    #[test]
    fn pause_menu_confirm_can_open_options() {
        let mut world = World::new();

        setup_menu_world(
            &mut world,
            GameScreen::PauseMenu,
            MenuAction::MoveSelectionDown,
        );
        let mut schedule = menu_with_inventory_resolution_schedule();
        schedule.run(&mut world);
        assert_eq!(
            world.resource::<PauseMenuState>().selected(),
            PauseMenuEntry::Options
        );

        world.insert_resource(MenuInputState {
            action: Some(MenuAction::Confirm),
        });
        schedule.run(&mut world);

        assert_eq!(*world.resource::<GameScreen>(), GameScreen::OptionsMenu);
    }

    #[test]
    fn escape_closes_inventory_menu() {
        let mut world = World::new();
        setup_menu_world(&mut world, GameScreen::InventoryMenu, MenuAction::Cancel);

        let mut schedule = menu_with_inventory_resolution_schedule();
        schedule.run(&mut world);

        assert_eq!(*world.resource::<GameScreen>(), GameScreen::Playing);
    }

    #[test]
    fn confirming_empty_inventory_spends_no_energy() {
        let mut world = World::new();
        setup_menu_world(&mut world, GameScreen::InventoryMenu, MenuAction::Confirm);
        world.insert_resource(EnergyTimeline::default());
        world.insert_resource(SimulationClock { turn: 0 });
        world.insert_resource(PlayerIntent::default());
        spawn_test_player(&mut world, Position { x: 2, y: 2 });

        let mut schedule = menu_with_inventory_resolution_schedule();
        schedule.run(&mut world);

        let mut energy_query = world.query_filtered::<&ActionEnergy, With<Player>>();
        let energy = energy_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert_eq!(energy.ready_at, 0);
        assert_eq!(world.resource::<SimulationClock>().turn, 0);
    }

    #[test]
    fn inventory_selection_wraps_through_carried_parcels() {
        let mut world = World::new();
        setup_menu_world(
            &mut world,
            GameScreen::InventoryMenu,
            MenuAction::MoveSelectionDown,
        );
        let player = spawn_test_player(&mut world, Position { x: 2, y: 2 });
        spawn_carried_parcel(&mut world, player, Position { x: 0, y: 0 });
        spawn_carried_parcel(&mut world, player, Position { x: 0, y: 0 });

        let mut schedule = menu_with_inventory_resolution_schedule();
        schedule.run(&mut world);
        assert_eq!(world.resource::<InventoryMenuState>().selected_index(), 1);

        schedule.run(&mut world);
        assert_eq!(world.resource::<InventoryMenuState>().selected_index(), 0);
    }

    #[test]
    fn confirming_inventory_drops_selected_parcel() {
        let mut world = World::new();
        setup_menu_world(&mut world, GameScreen::InventoryMenu, MenuAction::Confirm);
        world.insert_resource(EnergyTimeline::default());
        world.insert_resource(SimulationClock { turn: 0 });
        world.insert_resource(PlayerIntent::default());
        let player = spawn_test_player(&mut world, Position { x: 2, y: 2 });
        let parcel = spawn_carried_parcel(&mut world, player, Position { x: 0, y: 0 });

        let mut schedule = menu_with_inventory_resolution_schedule();
        let mut simulation = SimulationRunner::new();
        run_menu_with_simulation(&mut schedule, &mut simulation, &mut world);

        assert_eq!(
            *world
                .get::<ParcelState>(parcel)
                .expect("test parcel should still exist"),
            ParcelState::Loose
        );
        assert_eq!(
            *world
                .get::<Position>(parcel)
                .expect("test parcel should still have a position"),
            Position { x: 2, y: 2 }
        );

        let mut player_query = world.query_filtered::<(&Cargo, &ActionEnergy), With<Player>>();
        let (cargo, energy) = player_query
            .iter(&world)
            .next()
            .expect("test player should exist");
        assert_eq!(cargo.current_weight, 0.0);
        assert!(energy.ready_at > 0);
        assert_eq!(world.resource::<SimulationClock>().turn, 1);
        assert_eq!(world.resource::<EnergyTimeline>().now, energy.ready_at);
    }
}
