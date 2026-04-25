use bevy_ecs::prelude::*;

use crate::components::{Cargo, CargoParcel, ParcelState, Player, Position};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarriedParcelEntry {
    pub entity: Entity,
    pub weight: f32,
}

pub fn carried_parcels(world: &mut World, holder: Entity) -> Vec<CarriedParcelEntry> {
    let mut parcel_query = world.query::<(Entity, &CargoParcel, &ParcelState)>();
    let mut parcels = parcel_query
        .iter(world)
        .filter_map(|(entity, parcel, state)| {
            if *state == ParcelState::CarriedBy(holder) {
                Some(CarriedParcelEntry {
                    entity,
                    weight: parcel.weight,
                })
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    parcels.sort_by_key(|entry| entry.entity.to_bits());
    parcels
}

pub fn carried_parcel_count(world: &mut World, holder: Entity) -> usize {
    carried_parcels(world, holder).len()
}

pub fn player_carried_parcels(world: &mut World) -> Vec<CarriedParcelEntry> {
    let Some(player_entity) = player_entity(world) else {
        return Vec::new();
    };
    carried_parcels(world, player_entity)
}

pub fn player_carried_parcel_count(world: &mut World) -> usize {
    let Some(player_entity) = player_entity(world) else {
        return 0;
    };
    carried_parcel_count(world, player_entity)
}

pub fn cargo_load(world: &World, holder: Entity) -> Option<f32> {
    world.get::<Cargo>(holder).map(|cargo| cargo.current_weight)
}

pub fn drop_carried_parcel(
    world: &mut World,
    holder: Entity,
    parcel: Entity,
    at: Position,
) -> bool {
    let Some(parcel_weight) = carried_parcel_weight(world, holder, parcel) else {
        return false;
    };

    let Some(mut cargo) = world.get_mut::<Cargo>(holder) else {
        return false;
    };
    cargo.current_weight = (cargo.current_weight - parcel_weight).max(0.0);

    if let Some(mut parcel_position) = world.get_mut::<Position>(parcel) {
        *parcel_position = at;
    }
    if let Some(mut parcel_state) = world.get_mut::<ParcelState>(parcel) {
        *parcel_state = ParcelState::Loose;
    }

    true
}

fn carried_parcel_weight(world: &World, holder: Entity, parcel: Entity) -> Option<f32> {
    let cargo_parcel = world.get::<CargoParcel>(parcel)?;
    let state = world.get::<ParcelState>(parcel)?;
    (*state == ParcelState::CarriedBy(holder)).then_some(cargo_parcel.weight)
}

fn player_entity(world: &mut World) -> Option<Entity> {
    let mut player_query = world.query_filtered::<Entity, With<Player>>();
    player_query.iter(world).next()
}
