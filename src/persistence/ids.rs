use bevy_ecs::prelude::*;
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::fmt;

const FIRST_RUNTIME_PERSISTENT_ID: u128 = 1_000_000;

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

/// Monotonic source for new persistent entity IDs in a live world.
///
/// Authored starter content can use fixed IDs for deterministic saves. Dynamic
/// simulation content should mint IDs here so save/load relationships never
/// depend on Bevy's transient `Entity` values.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PersistentIdAllocator {
    next: u128,
}

impl Default for PersistentIdAllocator {
    fn default() -> Self {
        Self {
            next: FIRST_RUNTIME_PERSISTENT_ID,
        }
    }
}

impl PersistentIdAllocator {
    pub fn new(next: u128) -> Self {
        Self { next }
    }

    pub fn mint(&mut self) -> PersistentId {
        let id = PersistentId(self.next);
        self.next = self
            .next
            .checked_add(1)
            .expect("persistent ID allocator exhausted u128 range");
        id
    }

    /// Advances the allocator past an ID restored from disk or assigned by
    /// authored content, preventing future dynamic IDs from colliding with it.
    pub fn reserve_existing(&mut self, id: PersistentId) {
        self.next = self.next.max(
            id.0.checked_add(1)
                .expect("cannot reserve maximum u128 persistent ID"),
        );
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocator_mints_monotonic_ids_after_reserved_values() {
        let mut allocator = PersistentIdAllocator::new(10);

        assert_eq!(allocator.mint(), PersistentId(10));
        allocator.reserve_existing(PersistentId(50));

        assert_eq!(allocator.mint(), PersistentId(51));
        assert_eq!(allocator.mint(), PersistentId(52));
    }

    #[test]
    fn default_allocator_stays_clear_of_authored_ids() {
        let mut allocator = PersistentIdAllocator::default();

        assert_eq!(allocator.mint(), PersistentId(FIRST_RUNTIME_PERSISTENT_ID));
    }
}
