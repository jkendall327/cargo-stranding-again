use bevy_ecs::prelude::*;

use crate::cargo::Cargo;
use std::collections::HashMap;

use crate::cargo::{
    derived_load, player_carried_parcels, CargoParcel, CargoStats, CarriedBy, ContainedIn,
    Container, Item, ParcelDelivery,
};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LooseCargoState {
    Available,
    Reserved { porter_id: Option<usize> },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LooseCargoRender {
    pub entity: Entity,
    pub position: Position,
    pub weight: f32,
    pub state: LooseCargoState,
}

impl LooseCargoRender {
    pub fn all_from_world(world: &mut World) -> Vec<Self> {
        let porter_ids = porter_ids_by_entity(world);
        let mut query = world.query::<(
            Entity,
            &Position,
            &Item,
            &CargoStats,
            Option<&ParcelDelivery>,
            Option<&CarriedBy>,
            Option<&ContainedIn>,
        )>();
        let mut cargo = query
            .iter(world)
            .filter_map(
                |(entity, position, _, stats, delivery, carried_by, contained_in)| {
                    if carried_by.is_some() || contained_in.is_some() {
                        return None;
                    }
                    match delivery {
                        Some(ParcelDelivery::Delivered) => None,
                        Some(ParcelDelivery::ReservedBy(porter)) => Some(Self {
                            entity,
                            position: *position,
                            weight: stats.weight,
                            state: LooseCargoState::Reserved {
                                porter_id: porter_ids.get(porter).copied(),
                            },
                        }),
                        Some(ParcelDelivery::Available) | None => Some(Self {
                            entity,
                            position: *position,
                            weight: stats.weight,
                            state: LooseCargoState::Available,
                        }),
                    }
                },
            )
            .collect::<Vec<_>>();
        cargo.sort_by_key(|entry| entry.entity.to_bits());
        cargo
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CargoHolderKind {
    Player,
    Porter(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActorCargoRender {
    pub holder: Entity,
    pub position: Position,
    pub holder_kind: CargoHolderKind,
    pub item_count: usize,
    pub parcel_count: usize,
    pub total_weight: f32,
    pub has_contained_items: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct ActorCargoAccumulator {
    item_count: usize,
    parcel_count: usize,
    total_weight: f32,
    has_contained_items: bool,
}

impl ActorCargoRender {
    pub fn all_from_world(world: &mut World) -> Vec<Self> {
        let holders = cargo_holders_by_entity(world);
        let carried_containers = carried_containers_by_entity(world);
        let mut accumulators = HashMap::<Entity, ActorCargoAccumulator>::new();

        {
            let mut direct_query =
                world.query::<(Entity, &Item, &CargoStats, &CarriedBy, Option<&CargoParcel>)>();
            for (_, _, stats, carried_by, parcel) in direct_query.iter(world) {
                if !holders.contains_key(&carried_by.holder) {
                    continue;
                }
                let entry = accumulators.entry(carried_by.holder).or_default();
                entry.item_count += 1;
                entry.total_weight += stats.weight;
                if parcel.is_some() {
                    entry.parcel_count += 1;
                }
            }
        }

        {
            let mut contained_query =
                world.query::<(&Item, &CargoStats, &ContainedIn, Option<&CargoParcel>)>();
            for (_, stats, contained_in, parcel) in contained_query.iter(world) {
                let Some(holder) = carried_containers.get(&contained_in.container).copied() else {
                    continue;
                };
                if !holders.contains_key(&holder) {
                    continue;
                }
                let entry = accumulators.entry(holder).or_default();
                entry.item_count += 1;
                entry.total_weight += stats.weight;
                entry.has_contained_items = true;
                if parcel.is_some() {
                    entry.parcel_count += 1;
                }
            }
        }

        let mut cargo = accumulators
            .into_iter()
            .filter_map(|(holder, cargo)| {
                let holder_snapshot = holders.get(&holder)?;
                Some(Self {
                    holder,
                    position: holder_snapshot.position,
                    holder_kind: holder_snapshot.kind,
                    item_count: cargo.item_count,
                    parcel_count: cargo.parcel_count,
                    total_weight: cargo.total_weight,
                    has_contained_items: cargo.has_contained_items,
                })
            })
            .collect::<Vec<_>>();
        cargo.sort_by_key(|entry| entry.holder.to_bits());
        cargo
    }
}

#[derive(Clone, Copy, Debug)]
struct CargoHolderSnapshot {
    position: Position,
    kind: CargoHolderKind,
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

fn porter_ids_by_entity(world: &mut World) -> HashMap<Entity, usize> {
    let mut query = world.query::<(Entity, &Porter)>();
    query
        .iter(world)
        .map(|(entity, porter)| (entity, porter.id))
        .collect()
}

fn cargo_holders_by_entity(world: &mut World) -> HashMap<Entity, CargoHolderSnapshot> {
    let mut query = world.query::<(Entity, &Position, Option<&Player>, Option<&Porter>)>();
    query
        .iter(world)
        .filter_map(|(entity, position, player, porter)| {
            let kind = if player.is_some() {
                CargoHolderKind::Player
            } else {
                CargoHolderKind::Porter(porter?.id)
            };
            Some((
                entity,
                CargoHolderSnapshot {
                    position: *position,
                    kind,
                },
            ))
        })
        .collect()
}

fn carried_containers_by_entity(world: &mut World) -> HashMap<Entity, Entity> {
    let mut query = world.query_filtered::<(Entity, &CarriedBy), With<Container>>();
    query
        .iter(world)
        .map(|(container, carried_by)| (container, carried_by.holder))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::{CarrySlot, Container};
    use crate::components::{Player, Porter};

    fn spawn_player(world: &mut World) -> Entity {
        world.spawn((Player, Position { x: 1, y: 2 })).id()
    }

    fn spawn_porter(world: &mut World, id: usize) -> Entity {
        world.spawn((Porter { id }, Position { x: 3, y: 4 })).id()
    }

    fn spawn_loose_parcel(world: &mut World, delivery: ParcelDelivery) -> Entity {
        world
            .spawn((
                Item,
                CargoStats {
                    weight: 6.0,
                    volume: 1.0,
                },
                CargoParcel,
                delivery,
                Position { x: 8, y: 9 },
            ))
            .id()
    }

    #[test]
    fn available_positioned_parcel_renders_as_loose_cargo() {
        let mut world = World::new();
        let parcel = spawn_loose_parcel(&mut world, ParcelDelivery::Available);

        assert_eq!(
            LooseCargoRender::all_from_world(&mut world),
            vec![LooseCargoRender {
                entity: parcel,
                position: Position { x: 8, y: 9 },
                weight: 6.0,
                state: LooseCargoState::Available,
            }]
        );
    }

    #[test]
    fn reserved_positioned_parcel_renders_with_porter_id() {
        let mut world = World::new();
        let porter = spawn_porter(&mut world, 7);
        let parcel = spawn_loose_parcel(&mut world, ParcelDelivery::ReservedBy(porter));

        assert_eq!(
            LooseCargoRender::all_from_world(&mut world),
            vec![LooseCargoRender {
                entity: parcel,
                position: Position { x: 8, y: 9 },
                weight: 6.0,
                state: LooseCargoState::Reserved { porter_id: Some(7) },
            }]
        );
    }

    #[test]
    fn delivered_parcel_is_not_rendered_as_loose_cargo() {
        let mut world = World::new();
        spawn_loose_parcel(&mut world, ParcelDelivery::Delivered);

        assert!(LooseCargoRender::all_from_world(&mut world).is_empty());
    }

    #[test]
    fn directly_carried_parcel_renders_on_holder() {
        let mut world = World::new();
        let player = spawn_player(&mut world);
        world.spawn((
            Item,
            CargoStats {
                weight: 5.0,
                volume: 1.0,
            },
            CargoParcel,
            CarriedBy {
                holder: player,
                slot: CarrySlot::Chest,
            },
        ));

        assert_eq!(
            ActorCargoRender::all_from_world(&mut world),
            vec![ActorCargoRender {
                holder: player,
                position: Position { x: 1, y: 2 },
                holder_kind: CargoHolderKind::Player,
                item_count: 1,
                parcel_count: 1,
                total_weight: 5.0,
                has_contained_items: false,
            }]
        );
    }

    #[test]
    fn contained_parcel_renders_on_container_holder() {
        let mut world = World::new();
        let porter = spawn_porter(&mut world, 2);
        let container = world
            .spawn((
                Item,
                CargoStats {
                    weight: 2.0,
                    volume: 3.0,
                },
                Container {
                    volume_capacity: 10.0,
                    weight_capacity: 20.0,
                },
                CarriedBy {
                    holder: porter,
                    slot: CarrySlot::Back,
                },
            ))
            .id();
        world.spawn((
            Item,
            CargoStats {
                weight: 4.0,
                volume: 1.0,
            },
            CargoParcel,
            ContainedIn { container },
        ));

        assert_eq!(
            ActorCargoRender::all_from_world(&mut world),
            vec![ActorCargoRender {
                holder: porter,
                position: Position { x: 3, y: 4 },
                holder_kind: CargoHolderKind::Porter(2),
                item_count: 2,
                parcel_count: 1,
                total_weight: 6.0,
                has_contained_items: true,
            }]
        );
    }
}
