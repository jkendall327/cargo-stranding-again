use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt, fs,
    path::Path,
};

use bevy_ecs::prelude::*;
use serde::Deserialize;

use crate::cargo::{CargoStats, Item};
use crate::components::Position;
use crate::ids::ItemDefinitionId;
use crate::persistence::{PersistentId, PersistentIdAllocator};

/// Human-facing item label copied from a definition at spawn time.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct DisplayName(pub String);

/// Broad categories for generation, UI filtering, and AI interest.
///
/// If one of these starts carrying precise simulation rules, promote it to a
/// typed field or component spec instead.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Deserialize)]
pub enum ItemTag {
    Herb,
    Medicinal,
    TradeGood,
    Organic,
}

/// Physical material is typed data because environmental systems may need it.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum MaterialKind {
    PlantMatter,
    Cloth,
    Metal,
    Ceramic,
}

/// Coarse item shape. This lets cargo and rendering code ask a clearer question
/// than "which tags did this definition happen to include?"
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum ItemForm {
    Bundle,
    Package,
    LooseObject,
}

/// A gameplay component representing medicinal use, not merely a category tag.
#[derive(Component, Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct MedicinalProperties {
    pub potency: f32,
    pub uses: u32,
}

/// Spawn-time component specs decoded from item data.
///
/// This is the typed bridge from RON to ECS. Data can opt into known gameplay
/// components, but Rust still owns their behavior and invariants.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum ItemComponentSpec {
    CargoStats { weight: f32, volume: f32 },
    Material(MaterialKind),
    Form(ItemForm),
    Medicine { potency: f32, uses: u32 },
}

impl ItemComponentSpec {
    fn insert(&self, entity: &mut EntityWorldMut<'_>) {
        match *self {
            Self::CargoStats { weight, volume } => {
                entity.insert(CargoStats { weight, volume });
            }
            Self::Material(material) => {
                entity.insert(material);
            }
            Self::Form(form) => {
                entity.insert(form);
            }
            Self::Medicine { potency, uses } => {
                entity.insert(MedicinalProperties { potency, uses });
            }
        }
    }

    fn kind(&self) -> ItemComponentKind {
        match self {
            Self::CargoStats { .. } => ItemComponentKind::CargoStats,
            Self::Material(_) => ItemComponentKind::Material,
            Self::Form(_) => ItemComponentKind::Form,
            Self::Medicine { .. } => ItemComponentKind::Medicine,
        }
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum ItemComponentKind {
    CargoStats,
    Material,
    Form,
    Medicine,
}

/// Persistence policy for one spawned item instance.
///
/// Definitions describe what an item is; this describes whether this particular
/// instance is important enough to survive save/load.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ItemPersistence {
    #[default]
    None,
    Specific(PersistentId),
    Allocate,
}

/// Instance-specific data added around an item definition at spawn time.
///
/// Worldgen, save loading, and authored fixtures can share this without each
/// becoming their own partial item factory.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemSpawnContext {
    pub position: Option<Position>,
    pub persistence: ItemPersistence,
}

/// Author-authored blueprint for spawning one kind of item.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ItemDefinition {
    pub id: ItemDefinitionId,
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: HashSet<ItemTag>,
    #[serde(default)]
    pub components: Vec<ItemComponentSpec>,
}

/// Validated item definition registry.
#[derive(Clone, Debug, Default, Resource)]
pub struct ItemDefinitions {
    definitions: HashMap<ItemDefinitionId, ItemDefinition>,
}

impl ItemDefinitions {
    /// Builds a validated registry from decoded item definitions.
    pub fn new(definitions: Vec<ItemDefinition>) -> Result<Self, ItemDefinitionError> {
        let mut registry = HashMap::new();
        for definition in definitions {
            validate_definition(&definition)?;
            let id = definition.id.clone();
            if registry.insert(id.clone(), definition).is_some() {
                return Err(ItemDefinitionError::DuplicateId(id));
            }
        }
        Ok(Self {
            definitions: registry,
        })
    }

    pub fn get(&self, id: &ItemDefinitionId) -> Option<&ItemDefinition> {
        self.definitions.get(id)
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}

/// Loads every `.ron` item definition in a directory.
///
/// This is deliberately small and boring; the interesting boundary is still
/// validation plus typed component insertion.
pub fn load_item_definitions_from_dir(
    path: impl AsRef<Path>,
) -> Result<ItemDefinitions, ItemDefinitionError> {
    let mut definitions = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|extension| extension == "ron") {
            definitions.push(load_item_definition_from_file(&path)?);
        }
    }
    ItemDefinitions::new(definitions)
}

pub fn load_item_definition_from_file(
    path: impl AsRef<Path>,
) -> Result<ItemDefinition, ItemDefinitionError> {
    let text = fs::read_to_string(path)?;
    load_item_definition_from_str(&text)
}

pub fn load_item_definition_from_str(text: &str) -> Result<ItemDefinition, ItemDefinitionError> {
    let definition = ron::de::from_str(text)?;
    validate_definition(&definition)?;
    Ok(definition)
}

/// Startup-shaped API for when this registry is ready to wire into the game.
pub fn init_item_definitions(
    world: &mut World,
    path: impl AsRef<Path>,
) -> Result<(), ItemDefinitionError> {
    let definitions = load_item_definitions_from_dir(path)?;
    world.insert_resource(definitions);
    Ok(())
}

/// Spawns an ECS item from a validated definition.
///
/// Contextual spawn rules can wrap this later: call this for the baseline
/// blueprint, then insert cave dirt, wetness, ownership, delivery destination,
/// or other instance-specific components.
pub fn spawn_item(
    world: &mut World,
    definitions: &ItemDefinitions,
    id: &ItemDefinitionId,
    context: ItemSpawnContext,
) -> Result<Entity, ItemSpawnError> {
    let definition = definitions
        .get(id)
        .ok_or_else(|| ItemSpawnError::UnknownDefinition(id.clone()))?;

    let persistent_id = match context.persistence {
        ItemPersistence::None => None,
        ItemPersistence::Specific(id) => Some(id),
        ItemPersistence::Allocate => Some(world.resource_mut::<PersistentIdAllocator>().mint()),
    };

    let mut entity = world.spawn((
        Item,
        definition.id.clone(),
        DisplayName(definition.display_name.clone()),
    ));
    for component in &definition.components {
        component.insert(&mut entity);
    }
    if let Some(position) = context.position {
        entity.insert(position);
    }
    if let Some(id) = persistent_id {
        entity.insert(id);
    }
    Ok(entity.id())
}

fn validate_definition(definition: &ItemDefinition) -> Result<(), ItemDefinitionError> {
    if definition.id.as_str().trim().is_empty() {
        return Err(ItemDefinitionError::validation(
            None,
            "item definition has an empty id",
        ));
    }
    if definition.display_name.trim().is_empty() {
        return Err(ItemDefinitionError::validation(
            Some(definition.id.clone()),
            "empty display name",
        ));
    }

    let mut component_kinds = HashSet::new();
    for component in &definition.components {
        let kind = component.kind();
        if !component_kinds.insert(kind) {
            return Err(ItemDefinitionError::validation(
                Some(definition.id.clone()),
                format!("duplicate component spec {kind:?}"),
            ));
        }
        validate_component(definition, component)?;
    }

    Ok(())
}

fn validate_component(
    definition: &ItemDefinition,
    component: &ItemComponentSpec,
) -> Result<(), ItemDefinitionError> {
    match *component {
        ItemComponentSpec::CargoStats { weight, volume } => {
            if !is_sensible_nonnegative_number(weight) || !is_sensible_nonnegative_number(volume) {
                return Err(ItemDefinitionError::validation(
                    Some(definition.id.clone()),
                    "cargo stats must be finite and nonnegative",
                ));
            }
        }
        ItemComponentSpec::Medicine { potency, uses } => {
            if !is_sensible_nonnegative_number(potency) || potency > 1.0 || uses == 0 {
                return Err(ItemDefinitionError::validation(
                    Some(definition.id.clone()),
                    "medicinal properties require finite potency in 0.0..=1.0 and at least one use",
                ));
            }
        }
        ItemComponentSpec::Material(_) | ItemComponentSpec::Form(_) => {}
    }
    Ok(())
}

fn is_sensible_nonnegative_number(value: f32) -> bool {
    value.is_finite() && value >= 0.0
}

#[derive(Debug)]
pub enum ItemDefinitionError {
    Io(std::io::Error),
    Deserialize(ron::error::SpannedError),
    Validation {
        id: Option<ItemDefinitionId>,
        message: String,
    },
    DuplicateId(ItemDefinitionId),
}

impl ItemDefinitionError {
    fn validation(id: Option<ItemDefinitionId>, message: impl Into<String>) -> Self {
        Self::Validation {
            id,
            message: message.into(),
        }
    }
}

impl fmt::Display for ItemDefinitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to read item definitions: {error}"),
            Self::Deserialize(error) => {
                write!(formatter, "failed to parse item definition: {error}")
            }
            Self::Validation { id: None, message } => write!(formatter, "{message}"),
            Self::Validation {
                id: Some(id),
                message,
            } => write!(
                formatter,
                "item definition '{}' is invalid: {message}",
                id.as_str()
            ),
            Self::DuplicateId(id) => {
                write!(formatter, "duplicate item definition id '{}'", id.as_str())
            }
        }
    }
}

impl Error for ItemDefinitionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Deserialize(error) => Some(error),
            Self::Validation { .. } | Self::DuplicateId(_) => None,
        }
    }
}

impl From<std::io::Error> for ItemDefinitionError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ron::error::SpannedError> for ItemDefinitionError {
    fn from(error: ron::error::SpannedError) -> Self {
        Self::Deserialize(error)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemSpawnError {
    UnknownDefinition(ItemDefinitionId),
}

impl fmt::Display for ItemSpawnError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownDefinition(id) => {
                write!(formatter, "unknown item definition '{}'", id.as_str())
            }
        }
    }
}

impl Error for ItemSpawnError {}

#[cfg(test)]
mod tests {
    use super::*;

    const FEVERFEW: &str = include_str!("../../data/items/feverfew_package.ron");

    #[test]
    fn feverfew_definition_loads_and_validates() {
        let definition = load_item_definition_from_str(FEVERFEW).expect("definition should load");

        assert_eq!(definition.id.as_str(), "feverfew_package");
        assert_eq!(
            definition.display_name,
            "package of feverfew medicinal herbs"
        );
        assert!(definition.tags.contains(&ItemTag::Medicinal));
        assert!(definition.tags.contains(&ItemTag::Herb));
    }

    #[test]
    fn spawn_item_inserts_component_specs() {
        let definition = load_item_definition_from_str(FEVERFEW).expect("definition should load");
        let id = definition.id.clone();
        let definitions = ItemDefinitions::new(vec![definition]).expect("registry should validate");
        let mut world = World::new();

        let entity = spawn_item(&mut world, &definitions, &id, ItemSpawnContext::default())
            .expect("item should spawn");
        let entity_ref = world.entity(entity);

        assert!(entity_ref.contains::<Item>());
        assert_eq!(entity_ref.get::<ItemDefinitionId>(), Some(&id));
        assert_eq!(
            entity_ref.get::<DisplayName>(),
            Some(&DisplayName(
                "package of feverfew medicinal herbs".to_string()
            ))
        );
        assert_eq!(
            entity_ref.get::<CargoStats>(),
            Some(&CargoStats {
                weight: 0.2,
                volume: 0.3
            })
        );
        assert_eq!(
            entity_ref.get::<MaterialKind>(),
            Some(&MaterialKind::PlantMatter)
        );
        assert_eq!(entity_ref.get::<ItemForm>(), Some(&ItemForm::Package));
        assert_eq!(
            entity_ref.get::<MedicinalProperties>(),
            Some(&MedicinalProperties {
                potency: 0.35,
                uses: 3
            })
        );
    }

    #[test]
    fn spawn_item_can_insert_instance_context() {
        let definition = load_item_definition_from_str(FEVERFEW).expect("definition should load");
        let id = definition.id.clone();
        let definitions = ItemDefinitions::new(vec![definition]).expect("registry should validate");
        let mut world = World::new();

        let entity = spawn_item(
            &mut world,
            &definitions,
            &id,
            ItemSpawnContext {
                position: Some(Position { x: 2, y: 3 }),
                persistence: ItemPersistence::Specific(PersistentId(99)),
            },
        )
        .expect("item should spawn");
        let entity_ref = world.entity(entity);

        assert_eq!(entity_ref.get::<Position>(), Some(&Position { x: 2, y: 3 }));
        assert_eq!(entity_ref.get::<PersistentId>(), Some(&PersistentId(99)));
    }

    #[test]
    fn spawn_item_can_allocate_persistent_id() {
        let definition = load_item_definition_from_str(FEVERFEW).expect("definition should load");
        let id = definition.id.clone();
        let definitions = ItemDefinitions::new(vec![definition]).expect("registry should validate");
        let mut world = World::new();
        world.insert_resource(PersistentIdAllocator::new(42));

        let entity = spawn_item(
            &mut world,
            &definitions,
            &id,
            ItemSpawnContext {
                position: None,
                persistence: ItemPersistence::Allocate,
            },
        )
        .expect("item should spawn");

        assert_eq!(world.get::<PersistentId>(entity), Some(&PersistentId(42)));
    }

    #[test]
    fn registry_rejects_duplicate_ids() {
        let definition = load_item_definition_from_str(FEVERFEW).expect("definition should load");
        let duplicate = definition.clone();

        let error = ItemDefinitions::new(vec![definition, duplicate])
            .expect_err("duplicate IDs should be rejected");

        assert!(matches!(error, ItemDefinitionError::DuplicateId(_)));
    }

    #[test]
    fn validation_allows_non_cargo_items() {
        let text = r#"(
            id: "floating_idea",
            display_name: "floating idea",
            tags: [],
            components: [],
        )"#;

        let definition =
            load_item_definition_from_str(text).expect("CargoStats should be optional");

        assert_eq!(definition.id.as_str(), "floating_idea");
        assert!(definition.components.is_empty());
    }

    #[test]
    fn validation_rejects_non_sensible_numbers() {
        let definition = ItemDefinition {
            id: ItemDefinitionId("impossible_box".to_owned()),
            display_name: "impossible box".to_owned(),
            description: String::new(),
            tags: HashSet::new(),
            components: vec![ItemComponentSpec::CargoStats {
                weight: f32::INFINITY,
                volume: 1.0,
            }],
        };

        let error = ItemDefinitions::new(vec![definition])
            .expect_err("infinite cargo values should fail validation");

        assert!(matches!(error, ItemDefinitionError::Validation { .. }));
    }

    #[test]
    fn validation_rejects_nan_medicine_potency() {
        let definition = ItemDefinition {
            id: ItemDefinitionId("strange_tincture".to_owned()),
            display_name: "strange tincture".to_owned(),
            description: String::new(),
            tags: HashSet::new(),
            components: vec![ItemComponentSpec::Medicine {
                potency: f32::NAN,
                uses: 1,
            }],
        };

        let error = ItemDefinitions::new(vec![definition])
            .expect_err("NaN medicine potency should fail validation");

        assert!(matches!(error, ItemDefinitionError::Validation { .. }));
    }
}
