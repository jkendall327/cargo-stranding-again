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
