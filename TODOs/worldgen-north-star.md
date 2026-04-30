# Worldgen / Historygen Design

## Goals

- Generate a large persistent world that one character is unlikely to fully see.
- Make geography feel physically motivated at play scale: rivers, biomes, paths, crossings.
- Generate coarse world data once per world save, then realize chunks at higher fidelity as the player reaches them.
- Support multiple characters in the same world.
- Let civilisation/history influence set dressing, jobs, settlements, NPCs, and paths without requiring a full sociology simulator.

## Non-Goals For First Pass

- No live geopolitical simulation during play.
- No ship travel or ocean logistics.
- No religion/language systems yet.
- No detailed NPC historical simulation yet.
- No settlement event logs yet, beyond leaving room for them.
- No named river/road entities required.

## Core Model

### Coarse World Layer

A persistent low-resolution world grid larger than chunks. Each coarse cell may contain:

- elevation
- moisture / climate hints
- biome
- water / river presence
- civ influence summary
- path strength / quality

Chunks sample this layer when realized.

### Geology First Pass

Use authored/game-dev plausible generation rather than scientific simulation:

1. Generate continent/archipelago shape with one main continent.
2. Generate elevation.
3. Derive broad climate/biome bands.
4. Draw continuous multi-chunk rivers downhill or along plausible drainage paths.
5. Persist the coarse result.

Rivers and lakes should matter because they are continuous obstacles/features, not just random local tiles.

### Civilisation First Pass

Civilisations are persistent world-level records. Initially they can have:

- id
- name
- active years
- capital settlement
- settlements
- broad traits/color/flavor tags

Mechanically, civs mostly create/own settlements and influence path maintenance.

### Settlements

Settlements are persistent nodes placed mostly by civilisation expansion/contraction logic.

Good terrain features may bias placement later:

- river crossings
- fertile areas
- coastlines
- resource points
- route intersections

Settlements can eventually have event logs, former names, foundation year, civ ownership history, and notable figures.

Settlements come in different sizes. Its size is influenced by:
- Strength of civilisational influence. (If it's on the periphery it will be smaller than if it's close to the capital.)
- If it's isolated or near to other settlements.
- If it's near any useful points of interest (rivers, mines, farmland).

Smaller settlements are abandoned more quickly when civilisational influence fades.
Smaller settlements are converted more quickly when another civilisation encroaches.

### Influence

Influence can be modeled as a graph/field generated from settlements and paths.

For now:

- living settlements project influence nearby
- connected settlements reinforce paths between them
- abandoned or isolated areas lose maintenance over time

No need for deep causal politics yet.

### Paths

“Path” is the generic concept, not “road”.

Paths include:

- roads
- tracks
- bridges
- fords
- ruined/decayed routes

Paths are not entities. They are generated/derived map affordances with a quality value.

Path quality initially affects stamina cost. Later it can affect:

- navigation
- porter traffic
- job generation
- safety
- rendering
- settlement prosperity

Path decay model:

- paths degrade every N simulated years
- nearby active settlements/influence maintain them
- lost settlement networks leave behind degraded paths
- path-river intersections can become crude crossings for now

## Gameplay Feel

Most jobs are practical logistics: food, tools, parcels, medicine, materials.

History should usually be ambient rather than melodramatic. A player might notice:

- a maintained road giving way to an old broken route
- a renamed settlement
- different NPC/item flavor across civ regions
- abandoned paths near dead settlements
- long rivers shaping travel choices

## Inspection / Tuning

Headless ASCII inspection should expose:

- elevation
- biome
- rivers
- settlements
- civ influence
- paths/path quality

This is important because the world is too large to tune by walking around manually.

## Implementation Notes

### Current Map Shape

The current map code is already close to the desired streaming shape:

- Gameplay systems use global `TileCoord`s.
- `Map` translates global tiles into `ChunkCoord` plus `LocalTileCoord`.
- `Chunk` stores fixed `16x16` arrays of terrain, elevation, and water depth.
- Loaded chunks are saved as exact tile history.
- Missing chunks can already be generated from world seed plus chunk coordinate.

Keep that boundary. Normal terrain should remain chunk data, not ECS entities.
Realized chunks are authoritative: once a chunk has been generated, loaded,
modified, or saved, its tile data is the source of truth for that part of the
world.

### First Slice Target

Add a persistent coarse world layer and use it to realize chunks. For the first
implementation, one coarse world cell can match one playable chunk. That keeps
coordinate conversion simple:

```text
CoarseCoord(x, y) == ChunkCoord(x, y)
CoarseCell        == broad plan for one 16x16 playable chunk
Chunk             == realized 16x16 tile data
```

The world should be finite but very large. Do not target a world where a normal
character can reasonably see most of it. A useful first scale is thousands of
chunks in each dimension, not merely thousands of tiles total. Because the
coarse layer stores one compact record per chunk, this can still be manageable
if each coarse cell stays small.

First-pass coarse cells should be boring and practical:

- elevation band / broad height
- moisture or climate hint
- biome
- optional path quality placeholder

Skip rivers, settlements, civilisation records, and history simulation in the
first implementation slice. Leave room in the data model for them, but do not
block basic coarse-to-chunk realization on solving them.

### Runtime Pipeline

World creation:

```text
world seed
  -> generate persistent WorldPlan / coarse grid once
  -> save WorldPlan with the world
```

Chunk realization during play:

```text
player/camera approaches ChunkCoord
  -> if saved chunk exists, load saved chunk
  -> otherwise sample WorldPlan at/near ChunkCoord
  -> realize a new Chunk
  -> insert it into Map.loaded_chunks
```

Saving:

```text
WorldPlan is saved as persistent world metadata.
Loaded chunks are saved as authoritative realized history.
Unseen chunks are not saved individually; they can be realized later from the
same WorldPlan.
```

This keeps the save model clear: the coarse world is the promise, realized
chunks are the observed history.

### Suggested Runtime Types

The exact module split can change, but the concepts should stay separate:

```rust
pub struct WorldPlan {
    pub seed: u64,
    pub bounds: CoarseWorldBounds,
    pub cell_size_tiles: i32,
    pub cells: Vec<CoarseCell>,
}

pub struct CoarseCell {
    pub elevation: i16,
    pub moisture: u8,
    pub biome: Biome,
    pub path_quality: u8,
}
```

For the first slice, `cell_size_tiles` should equal `CHUNK_WIDTH` and
`CHUNK_HEIGHT`. If chunk width and height ever diverge, replace this with an
explicit cell width/height.

Chunk generation should move toward an interface shaped like:

```rust
pub fn realize_chunk(plan: &WorldPlan, coord: ChunkCoord) -> Chunk
```

The realization step can still use deterministic tile-level noise, but that
noise should elaborate the coarse cell rather than inventing unrelated local
geography. Neighboring coarse cells should be sampled when needed to avoid hard
seams at chunk boundaries.

### Persistence

Add world-plan data beside the existing saved chunks. Existing saved chunk logic
should remain conceptually the same:

- `SavedWorldData` owns world metadata, including seed, bounds, and the coarse
  plan.
- `SavedChunk` owns exact realized tile data.
- Loading a saved world restores the plan first, then restores realized chunks.
- Generating an unseen chunk uses the restored plan, not a fresh random plan.

This is also the point where `Map` may need to know less about raw generation.
Prefer keeping `Map` as storage/query/streaming glue and moving world creation
or chunk realization into a `worldgen` module once the generator grows.

### Inspection Before Complexity

Before adding rivers or settlements, add inspection tools for the coarse layer.
The game will be too large to tune by walking. Headless/debug views should be
able to show at least:

- coarse elevation
- biome
- moisture
- realized chunk presence vs unseen coarse cells
- path quality if included in the first slice

Rivers, settlements, civilisation influence, and named historical features can
then be layered onto a debuggable base instead of being mixed into the initial
terrain generator.
