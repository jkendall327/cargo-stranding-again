use bevy_ecs::prelude::*;

use crate::cargo::Cargo;
use crate::cargo::{derived_load, player_carried_parcels};
use crate::components::{
    AssignedJob, JobPhase, Momentum, MovementState, Player, Porter, Position, Stamina,
};
use crate::energy::ActionEnergy;
use crate::map::{Map, TileCoord};
use crate::movement::MovementMode;
use crate::resources::{
    Camera, DeliveryStats, Direction, EnergyTimeline, InventoryMenuState, SimulationClock,
};

pub struct PlayerHudSnapshot {
    pub turn: u64,
    pub timeline: u64,
    pub camera: Camera,
    pub position: Position,
    pub elevation: i16,
    pub water_depth: u8,
    pub stamina_current: f32,
    pub stamina_max: f32,
    pub movement_mode: MovementMode,
    pub momentum_amount: f32,
    pub momentum_direction: Option<Direction>,
    pub ready_label: String,
    pub cargo_current: f32,
    pub cargo_max: f32,
    pub delivered_parcels: u32,
}

impl PlayerHudSnapshot {
    pub fn from_world(world: &mut World, camera: Camera) -> Option<Self> {
        let clock = *world.resource::<SimulationClock>();
        let delivery_stats = *world.resource::<DeliveryStats>();
        let timeline = *world.resource::<EnergyTimeline>();

        let (
            player_entity,
            position,
            stamina_current,
            stamina_max,
            movement_mode,
            momentum_amount,
            momentum_direction,
            cargo_max,
            energy,
        ) = {
            let mut player_query = world.query_filtered::<(
                Entity,
                &Position,
                &Stamina,
                &Cargo,
                &MovementState,
                &Momentum,
                &ActionEnergy,
            ), With<Player>>();
            let (entity, position, stamina, cargo, movement_state, momentum, energy) =
                player_query.iter(world).next()?;
            (
                entity,
                *position,
                stamina.current,
                stamina.max,
                movement_state.mode,
                momentum.amount,
                momentum.direction,
                cargo.max_weight,
                *energy,
            )
        };
        let cargo_current = derived_load(world, player_entity);

        let (elevation, water_depth) = {
            let map = world.resource::<Map>();
            let player_coord = TileCoord::from(position);
            (
                map.elevation_at_coord(player_coord).unwrap_or_default(),
                map.water_depth_at_coord(player_coord).unwrap_or_default(),
            )
        };

        Some(Self {
            turn: clock.turn,
            timeline: timeline.now,
            camera,
            position,
            elevation,
            water_depth,
            stamina_current,
            stamina_max,
            movement_mode,
            momentum_amount,
            momentum_direction,
            ready_label: ready_label(energy, timeline.now),
            cargo_current,
            cargo_max,
            delivered_parcels: delivery_stats.delivered_parcels,
        })
    }
}

pub struct PorterDebugRow {
    pub id: usize,
    pub position: Position,
    pub phase_label: &'static str,
    pub load: f32,
    pub ready_label: String,
}

pub struct InventoryEntry {
    pub label: String,
    pub selected: bool,
}

impl InventoryEntry {
    pub fn all_from_world(world: &mut World) -> Vec<Self> {
        let selected_index = world.resource::<InventoryMenuState>().selected_index();
        player_carried_parcels(world)
            .iter()
            .enumerate()
            .map(|(index, entry)| Self {
                label: format!("Parcel {:.0} weight", entry.weight),
                selected: selected_index == index,
            })
            .collect()
    }
}

impl PorterDebugRow {
    pub fn all_from_world(world: &mut World) -> Vec<Self> {
        let timeline = world.resource::<EnergyTimeline>().now;
        let mut query = world.query::<(Entity, &Position, &Porter, &AssignedJob, &ActionEnergy)>();
        let snapshots = query
            .iter(world)
            .map(|(entity, position, porter, job, energy)| {
                (entity, *position, porter.id, job.phase, *energy)
            })
            .collect::<Vec<_>>();
        let mut rows = snapshots
            .into_iter()
            .map(|(entity, position, id, phase, energy)| Self {
                id,
                position,
                phase_label: phase_label(phase),
                load: derived_load(world, entity),
                ready_label: ready_label(energy, timeline),
            })
            .collect::<Vec<_>>();
        rows.sort_by_key(|row| row.id);
        rows
    }
}

fn phase_label(phase: JobPhase) -> &'static str {
    match phase {
        JobPhase::FindParcel => "finding",
        JobPhase::GoToParcel => "to parcel",
        JobPhase::GoToDepot => "to depot",
        JobPhase::Done => "done",
    }
}

fn ready_label(energy: ActionEnergy, now: u64) -> String {
    if energy.is_ready(now) {
        "ready".to_owned()
    } else {
        format!("ready in {}", energy.ready_at - now)
    }
}
