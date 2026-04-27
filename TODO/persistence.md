# Persistence North Star

Persistence should become a deliberate game-facing model, not a dump of Bevy ECS
internals. Saves are part of the player's relationship with the world, so the
format should be version-aware from day one and should preserve player-visible
history even while the codebase keeps evolving.

## Core Principles

- Save files are versioned from the beginning.
- Save data uses explicit save structs, not direct Bevy ECS serialization.
- Runtime `Entity` values never appear in save data.
- Player-visible history is authoritative once it exists.
- Generated chunks are saved as full chunk data after generation.
- The world and the player character are related but separate persistence roots.
- Cargo is history-bearing and should be saved in high detail.
- Failed save/load should fail loudly during development.
- Migrations should be explicit when save formats change.

## Save Envelope

All save files should use a small metadata envelope around a typed payload:

```rust
struct Save<T> {
    metadata: SaveMetadata,
    payload: T,
}

struct SaveMetadata {
    version: SaveVersion,
    kind: SaveKind,
}

enum SaveKind {
    World,
    Character,
}
```

The exact fields can grow, but `version` and `kind` should be present from the
first implementation. Useful later metadata may include created/updated
timestamps, display names, code build info, or debug labels.

## World And Character Split

A world and a character are orthogonal persistence objects.

- A world can exist without a currently active character.
- A character is always tied to exactly one world.
- A character cannot be moved into another world.
- The UI should eventually be a two-step flow:
  - choose world
  - choose character within that world

This supports long-term Dwarf Fortress-style world history simulation while
still allowing multiple player characters to inhabit the same world over time.

Approximate payload shapes:

```rust
struct WorldSaveData {
    world_id: WorldId,
    seed: u64,
    chunks: Vec<SavedChunk>,
    world_entities: Vec<SavedEntity>,
}

struct CharacterSaveData {
    character_id: CharacterId,
    world_id: WorldId,
    player: SavedPlayer,
    carried_entities: Vec<SavedEntity>,
}
```

The concrete storage layout can differ from these structs. For example, chunks
may live in separate files under a world directory while the world metadata file
contains only chunk indexes and global state.

## World Storage Layout

Prefer a save directory over one monolithic file.

One plausible layout:

```text
saves/
  worlds/
    <world-id>/
      world.ron
      chunks/
        <chunk-x>_<chunk-y>.ron
      characters/
        <character-id>.ron
```

Separate chunk files fit the expected growth model: worlds get larger on disk as
the player explores. They also make corruption less catastrophic; one damaged
chunk does not necessarily destroy an entire world save. In development, a
corrupt chunk can simply be treated as a loud load error. Later, we may allow
debug recovery by regenerating that chunk from the seed.

## Chunk And Procgen Policy

Save the world seed. Once a chunk has been generated, also save the full chunk
data.

The seed is for unexplored space. Saved chunk data is for history.

This avoids requiring old procedural generation code to remain perfectly alive
forever. It also protects existing saves from changes to terrain algorithms,
biome definitions, item tables, balance data, or world-history inputs.

At minimum, a saved chunk should include:

- chunk coordinate
- terrain tiles
- elevation values
- water/depth values
- later: tile modifications, constructions, damage, discovered state, local
  environmental state

## Persistent Identity

Persistent game objects need stable IDs independent of Bevy `Entity`.

```rust
struct PersistentId(u128);
```

Likely persistent objects:

- player character
- NPCs
- cargo items
- containers
- parcels
- future ropes, vehicles, buildables, fires, doors, corpses, and other
  world-history-bearing entities

Save/load should rebuild a temporary mapping:

```text
PersistentId -> newly spawned Bevy Entity
```

Relationships are then reconnected in a second pass. This is necessary for
relationships such as carried-by, contained-in, reserved-by, assigned jobs, and
future references between persistent entities.

## Cargo Ownership

Cargo should be saved wherever it physically belongs at save time.

- Loose cargo in the world is saved with the world/chunk.
- Cargo carried by the player is saved with the character.
- Cargo inside a player-carried container is saved with the character.
- Cargo carried by an NPC is saved with the world.
- Cargo inside an NPC/world container is saved with the world.

Shared-world consequence:

- If character A drops cargo in the world, character B can later find it.
- If character A quits while carrying cargo, that cargo is unavailable to
  character B.
- In the future, a retired or dead character may become an NPC or world object,
  but that is deferred.

This may become complex, so implementation should start with clear helper
functions for deciding which persistence root owns an entity.

## Cargo Detail

Cargo should be persisted aggressively. Future systems are likely to care about
details we cannot fully predict yet.

Likely saved cargo fields:

- persistent ID
- item definition ID
- cargo stats such as weight and volume
- location
- parcel/delivery state
- container capacity, if it is a container
- contained item relationships
- carry slot, if directly carried
- condition/damage/wetness later
- destination, owner, provenance, generated quirks, or other parcel metadata
  later

## Data-Driven Definitions

Save data should store stable definition IDs and instance state.

Definitions describe the default object. Saves describe this exact object.

For example, a saved cargo item should reference an item definition and then
store any instance values that differ from, or are not represented by, that
definition.

```rust
struct SavedCargoItem {
    id: PersistentId,
    definition_id: ItemDefinitionId,
    stats: SavedCargoStats,
    location: SavedCargoLocation,
    parcel: Option<SavedParcelState>,
    container: Option<SavedContainerState>,
}
```

This lets default data evolve while preserving concrete player-facing facts
where needed. A future migration can decide whether a save should accept new
definition defaults or preserve old instance values.

## Player And Actor State

The player save should include individual character state, not just world
position.

Likely saved player fields:

- persistent ID
- position
- stamina
- future HP/body state
- movement mode
- action energy
- carried cargo and containers
- current character-specific stats/progression

Momentum can be ignored initially unless it becomes important enough that
loading without it feels wrong. The player should only be allowed to save while
standing still, so transient body state can be kept minimal at first.

NPCs on the generated/current world should persist. Off-map autonomous NPC
simulation is deferred.

Likely saved NPC fields:

- persistent ID
- position
- stamina/HP/body state as those systems appear
- action energy
- cargo/container relationships
- current job or intent

Saved jobs must reference persistent IDs rather than Bevy entities.

## Timeline

Saving timeline state is cheap enough to include.

Save:

- global energy timeline time
- per-actor `ActionEnergy`

This does not require promising perfect combat-save fidelity yet. It simply
avoids unnecessary discontinuities after load. NPC intent/job state should also
be saved when present.

## Save Eligibility

Save eligibility should be a domain helper rather than UI-only logic.

```rust
fn player_can_save(world: &World) -> SaveEligibility
```

Initial behavior can be permissive for testing. The intended default rule is
that the player can save when standing still. Future conditions may include:

- player exists
- simulation is awaiting player input
- player velocity is zero
- player is not falling
- player is not in combat
- player is not inside another unresolved action state

A future campfire/save object may make these constraints feel natural in play,
but the save system should not depend on that yet.

## Format

RON is a good early primary format because debuggability matters while the save
model is still moving.

JSON is already available and can be useful for tests or tooling. Binary or
compressed saves can wait until there is a concrete need.

## Error Handling

During development, failed save/load should explode loudly with useful errors.

Do not silently recover, partially load, or regenerate missing data unless a
specific debug recovery path has been requested. Graceful production behavior can
come later.

## Migrations

Save migrations should be explicit.

Examples:

- version 1 has stamina; version 2 adds HP and gives old actors default HP
- version 2 stores loose cargo globally; version 3 moves it into chunk files
- version 3 stores cargo stats directly; version 4 stores definition IDs plus
  instance overrides

The goal is to make format changes intentional instead of forcing the codebase
to remain shaped like old save files forever.

## First Implementation Milestones

1. [x] Add save schema modules with versioned envelope types.
2. [x] Add persistent ID types/components for persistent entities.
3. [x] Add map/chunk save structs and chunk round-trip tests.
4. [x] Add loose cargo save/load round-trip tests.
5. [x] Add player/character save structs.
5a. [x] Add in-memory world save payload assembly for loaded chunks and loose cargo.
6. [ ] Add world directory layout and single-world/single-character save commands.
   - [x] Add RON world manifest plus per-chunk filesystem round-trip.
   - [ ] Add single-world/single-character save commands.
7. [ ] Add save eligibility helper.
8. [ ] Add migration scaffolding before the second save version exists.

The first useful test should prove that modified/generated chunk state
round-trips exactly. The second should prove that loose cargo keeps identity,
position, stats, and parcel state across save/load.

Current status:

- Save schema modules live under `src/persistence/`.
- `Save<T>`, `SaveMetadata`, `SaveVersion`, and `SaveKind` exist.
- `PersistentId`, `WorldId`, `CharacterId`, and `ItemDefinitionId` exist, with
  `PersistentId` available as an ECS component.
- Map/chunk persistence currently round-trips through the in-memory save model,
  not through filesystem storage yet.
- Runtime `Chunk` exposes row-major tile snapshots and can be rebuilt from
  complete tile snapshots with an explicit tile-count error.
- Generated and authored chunk data have tests proving exact schema round-trip,
  including terrain, elevation, and water depth.

## Deferred

- Off-map NPC history simulation.
- Retired/dead player characters becoming NPCs.
- Graceful recovery from corrupt saves.
- Binary/compressed saves.
- Full production UX for save slots.
- Perfect save support for combat, falling, or other unresolved action states.
