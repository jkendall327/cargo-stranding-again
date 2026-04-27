use bevy_ecs::prelude::*;
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::fmt;

/// Stable identity for an object that can outlive one ECS world instance.
///
/// Save/load code should build a temporary `PersistentId -> Entity` map after
/// spawning entities, then reconnect relationships in a second pass.
#[derive(Component, Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct PersistentId(pub u128);

/// Stable identity for a persisted world.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct WorldId(pub u128);

/// Stable identity for a player character inside one world.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct CharacterId(pub u128);

/// Stable definition key for data-driven item defaults.
#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemDefinitionId(pub String);

impl Serialize for PersistentId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_u128_string(self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for PersistentId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_u128_string(deserializer).map(Self)
    }
}

impl Serialize for WorldId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_u128_string(self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for WorldId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_u128_string(deserializer).map(Self)
    }
}

impl Serialize for CharacterId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_u128_string(self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for CharacterId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_u128_string(deserializer).map(Self)
    }
}

fn serialize_u128_string<S>(value: u128, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_string())
}

fn deserialize_u128_string<'de, D>(deserializer: D) -> Result<u128, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_any(U128StringVisitor)
}

struct U128StringVisitor;

impl Visitor<'_> for U128StringVisitor {
    type Value = u128;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a u128 encoded as a decimal string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(u128::from(value))
    }

    fn visit_u128<E>(self, value: u128) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(value)
    }
}
