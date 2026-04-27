# Cargo Stranding Again TODO

This is the working roadmap distilled from `SERIOUS_GAME.md`.

The current codebase is a small but healthy Bevy ECS + Macroquad prototype. Keep the split:

- Macroquad owns the outer frame loop, input polling, windowing, and drawing.
- Bevy ECS owns deterministic-ish game state and simulation systems.
- Rendering manually queries ECS for now.
- Normal terrain should stay in map/chunk arrays, not become ECS entities.
- ECS entities are for things that behave: player, NPCs, cargo, containers, ropes, vehicles, fires, doors, buildables, etc.

Dependency notes:

- Keep dependencies small and purpose-driven.
- [x] Add `tracing` / `tracing-subscriber` for basic structured logging.
- [ ] Consider `pathfinding` when replacing NPC greedy stepping.
  - Useful first targets: BFS for passability-only paths, then A* or Dijkstra once terrain cost matters.
  - Keep it wrapped behind a small local pathing API so the rest of the sim does not care which algorithm crate is underneath.
- [ ] Consider `rand` with a portable seeded RNG, or `rand_chacha`, once map/content generation needs seeds.
  - Use deterministic seeds for generated maps, parcel placement, weather rolls, and test fixtures.
  - Avoid thread-local randomness in simulation systems so headless runs stay reproducible.

## North Star Architecture

Evolve the prototype around this pipeline:

```text
Raw input -> abstract intent/action -> movement/job/simulation consequences -> render
```

Avoid growing one large "movement plus terrain plus stamina plus cargo plus weather plus NPC" system. Prefer small domain concepts and explicit phases.

## 1. Movement Resolver

Goal: move player and NPC movement through shared movement-resolution code instead of player-only and agent-only movement logic.

Status: done.

## 2. Expanded Player Actions

Goal: grow `PlayerAction` deliberately without turning it into input-shaped movement again.

Status: foundations done. Can add more actions like Interact, AdjustBalance etc.

## 3. Action Energy Timeline

Goal: replace the current player-action heartbeat with a shared energy timeline so movement speed, terrain, stamina, cargo, combat, pickup/drop, and future debuffs all speak the same scheduling language.

Design decisions:

- One player input attempts one atomic action.
  - `Move(North)` moves at most one tile.
  - Sprinting should be visible through lower action delay / NPC timing, not by skipping multiple tiles in one input.
- Use an energy timeline rather than "player action, then every NPC acts once."
  - Actors act when they have enough energy for their next action.
  - A priority queue or equivalent ready-time scheduler is the likely fit.
- Sprinting lowers movement energy cost and increases stamina/stability cost.
- Steady walking raises movement energy cost and reduces rough-terrain stamina cost.
- Stamina can start as a hard gate for draining actions.
  - Later, low stamina can become a risk/performance modifier without changing the scheduler architecture.
- Non-movement actions should also cost energy.
  - Simple defaults for now: movement, wait/rest, pickup/drop, interact, and future attacks all spend action energy.
  - Exhaustion, bleeding, heavy load, weather, or injuries can become generalized energy/recovery modifiers.
- Changing movement posture remains free for now.
  - Bracing can later be a separate action that costs energy because it has immediate defensive value.
- Keep physical momentum separate from scheduling energy.
  - `Energy`: when/how often an actor can act.
  - `Momentum`: body state and stability risk.
  - `MovementState`: chosen posture/effort.
- Food for thought later: if autonomous wildlife/NPC simulation becomes a larger
  feature, consider whether the energy timeline should become more ECS-native
  with first-class ready actor/timeline event phases. For now, keep player input
  as the pacing boundary and use an explicit simulation runner for the custom
  timeline orchestration.

Status: done.

## 4. Serious Cargo Model

Goal: replace `Cargo { current_weight, max_weight }` as the core cargo model with entity relationships, carry slots, and derived load totals.

Status: done.

## 5. Data-Driven Terrain And Items

Goal: stop hardcoding every terrain/item stat in Rust once the domain model settles enough.

- [ ] Add `serde` and a data format.
  - [ ] Prefer RON or TOML for hand-authored game data.
  - [ ] JSON is fine if tool simplicity matters more.
- [ ] Introduce stable terrain IDs in the map.
  - [ ] Keep the existing `Terrain` enum until there is a clear benefit to replacing it.
  - [ ] Consider `TerrainId` plus `TerrainDefinition`.
- [ ] Move terrain definitions out of `Terrain` methods.
  - [ ] movement cost
  - [ ] stamina delta
  - [ ] passability
  - [ ] color/glyph, unless rendering gets its own definition layer
  - [ ] later: elevation behavior, wetness, exposure, wind shelter, traction
- [ ] Add a `TerrainDefinitions` resource.
- [ ] Load default terrain definitions at startup.
- [ ] Decide fallback behavior if data files are missing or invalid.
  - [ ] For development, panic with useful errors is acceptable.
  - [ ] For release, fall back to embedded defaults or show an error screen.
- [ ] Add item/cargo definitions after the cargo model exists.
  - [ ] item ID
  - [ ] display name
  - [ ] weight
  - [ ] volume
  - [ ] cargo tags/properties
- [ ] Add tests for loading and validating definitions.

## 6. NPC Goals And Jobs

Goal: move from hardcoded porter delivery behavior toward simple goal-driven agents.

- [ ] Split job assignment from job execution more clearly.
- [ ] Replace `AssignedJob { phase, parcel }` with a more general job/task representation when the second job type appears.
- [ ] Add explicit job targets.
  - [ ] target entity
  - [ ] target tile
  - [ ] delivery depot / destination
- [ ] Replace greedy movement with pathfinding.
  - [ ] Consider adding the `pathfinding` crate for BFS/A*/Dijkstra rather than hand-rolling graph search.
  - [ ] Add a small `pathing` module that converts `Map` passability/costs into pathfinding successors.
  - [ ] Start with BFS or A* on the current fixed map.
  - [ ] Account for passability first.
  - [ ] Later account for terrain movement cost, stamina, load, and danger.
- [ ] Allow agents to fail or abandon jobs.
  - [ ] Parcel no longer exists.
  - [ ] Parcel already carried by someone else.
  - [ ] Destination unreachable.
  - [ ] Porter lacks capacity.
- [ ] Add at least one non-delivery goal later.
  - [ ] Rest/recover stamina.
  - [ ] Seek shelter.
  - [ ] Avoid weather.
  - [ ] Fetch container/tool.

Current code pointers:

- `assign_porter_jobs` reserves loose parcels.
- `porter_jobs` moves through `FindParcel`, `GoToParcel`, `GoToDepot`, `Done`.
- `greedy_step` is deliberately simple and should be replaced before worldgen gets serious.

## 7. Simulationist Body, Balance, And Weather

Goal: add deeper simulation features as layered ECS systems, not as extra branches in movement.

- [ ] Add a body/load state model once cargo relationships exist.
  - [ ] `BodyBalance`
  - [ ] `CenterOfMass`
  - [ ] `LoadDistribution`
  - [ ] `BalanceShift`
- [ ] Make cargo placement affect balance.
  - [ ] Back-heavy load changes stumble/fall risk.
  - [ ] Hand-carried load affects sprinting/climbing.
  - [ ] Uneven left/right load affects movement cost or drift.
- [ ] Add weather resources.
  - [ ] `WeatherState`
  - [ ] `WindField` or per-region wind.
  - [ ] precipitation / wetness later.
- [ ] Add environmental effects as separate systems.
  - [ ] Wind pushes exposed actors or modifies movement outcome.
  - [ ] Rain changes terrain traction/cost.
  - [ ] Cold/wetness affects stamina recovery.
- [ ] Make the UI expose enough debug state to understand the simulation.
- [ ] Add deterministic tests for balance/weather calculations where practical.

Implementation note:

- Build this only after movement and cargo have clean enough boundaries. Weather that directly edits player movement will make later refactors painful.

## 8. Chunked Map And Procgen

Goal: move from one fixed rectangle to deterministic, streamable world data.

- [x] Introduce coordinate types.
  - [x] `TileCoord`
  - [x] `ChunkCoord`
  - [x] Local tile coordinate inside chunk
- [x] Introduce `Chunk`.
  - [x] fixed width/height
  - [x] terrain tile array
  - [x] optional elevation/depth arrays later
- [x] Introduce `WorldMap` or evolve `Map`.
  - [x] active chunk storage
  - [x] seed
  - [x] chunk lookup by world tile coordinate
- [x] Keep initial implementation compatible with the current generated map.
  - [x] One chunk or a small fixed set of chunks is fine at first.
- [x] Make procedural generation deterministic by seed and chunk coordinate.
- [x] Add chunk load boundaries around the camera or player.
  - Unloading is deliberately deferred while generated chunks stay in memory.
- [ ] Add persistence for visited/modified chunks.
  - [x] Add explicit saved chunk/tile schema types.
  - [x] Add runtime chunk <-> saved chunk round-trip tests.
  - [ ] Start with a simple save directory or single save file.
  - [ ] Persist only changed chunks if generation is deterministic.
- [x] Update rendering to draw visible tiles across chunk boundaries.
- [x] Update pathfinding/movement to query world coordinates, not fixed `0..width` / `0..height` assumptions.
- [ ] Add tests.
  - [x] Same seed + chunk coordinate produces same terrain.
  - [x] World coordinate lookup crosses chunk boundaries correctly.
  - [x] Modified chunk state round-trips through save/load model.
  - [ ] Modified chunk state round-trips through filesystem save/load.

Current code pointers:

- `src/map.rs::Map` still exposes finite `width`/`height` bounds, but terrain storage is chunk-backed.
- `src/resources.rs::Camera` clamps to map width/height.
- Rendering iterates `Map::visible_tiles` and looks up chunk-backed tile data.

## 9. Verticality

Goal: support cliffs, deep water, falling, climbing, ropes, rappelling, and slopes without pretending the current `Position { x, y }` is enough forever.

- [ ] Decide the verticality model before changing `Position`.
  - [ ] Option A: discrete z-levels (`Position { x, y, z }`).
  - [X] Option B: 2D grid plus elevation/depth fields.
  - [ ] Likely target: both. Tile columns have elevation/depth; entities can have vertical state.
- [x] Add tile elevation/depth first.
  - [x] elevation
  - [x] water depth
  - [x] slope / grade derived from neighboring elevation
- [ ] Make movement outcomes account for slope.
  - [x] Uphill costs more stamina.
  - [ ] Downhill can increase momentum or fall risk.
  - [x] Steep cliffs block normal walking.
- [ ] Add vertical movement states later.
  - [ ] `Climbing`
  - [ ] `Rappelling`
  - [ ] `Falling`
  - [ ] `Swimming`
- [ ] Add rope/climbing entities only when there is an interaction loop for them.
- [x] Update rendering with enough visual information to debug elevation.
- [x] Add tests for cliff blocking, slope costs, and water depth behavior.

Note:

- Do not immediately rewrite the whole game as 3D. Add elevation/depth to terrain/chunks first, then entity z/vertical state when gameplay demands it.

## 10. UI, Menus, And Rebinding

Goal: make the existing keybinding resource visible and editable through the game.

- [ ] Expand the options menu beyond a placeholder.
- [ ] Add a keybinding view.
  - [ ] Show gameplay bindings.
  - [ ] Show menu bindings.
  - [ ] Show conflicts.
- [ ] Add rebinding flow.
  - [ ] Select action.
  - [ ] Capture next key.
  - [ ] Reject or warn on conflicts.
  - [ ] Allow reset to defaults.
- [ ] Serialize keybindings to user config.
- [ ] Load keybindings at startup, with defaults as fallback.
- [ ] Tests for binding lookup and conflict detection.

Current code pointers:

- `src/input.rs` has the default `KeyBindings`.
- `src/main.rs::copy_input_to_ecs` reads those bindings.
- `src/systems.rs::menu_navigation` already opens `OptionsMenu`, but there is not yet real options UI.

## 11. Save/Load

Goal: add persistence before worldgen and cargo relationships become too large to reason about casually.

- [ ] Decide save scope.
  - [x] Split world and character save roots.
  - [ ] Full world snapshot for early development.
  - [ ] Later: deterministic world seed plus changed chunks plus ECS entity state.
- [x] Add serializable save structs instead of serializing ECS internals directly.
  - [x] Add versioned save envelope metadata.
  - [x] Add stable persistent ID types/components.
  - [x] Add map/chunk schema and conversions.
  - [x] Add player/character schema types.
  - [x] Add cargo/container/parcel schema types using persistent IDs.
- [ ] Save core resources.
  - [ ] simulation clock
  - [x] map/chunk seed and tile schema
  - [x] in-memory world save payload for loaded chunks and loose cargo
  - [x] map/chunk seed and loaded chunk tiles on disk
  - [ ] player position/stamina/load
  - [ ] loose/carried/delivered cargo state
  - [ ] NPC positions/jobs
- [ ] Add load path in startup/debug menu.
- [ ] Add round-trip tests.
  - [x] generated/authored chunk state round-trips through save model
  - [x] loose cargo keeps identity, position, stats, and parcel state
  - [x] filesystem save/load round-trip

## Useful Guardrails

- Keep the game playable after each task.
- Keep `cargo check` and tests green after each slice.
- Prefer adding one new concept at a time over broad rewrites.
- If a task requires both UI and simulation, land the simulation first with tests, then expose it in rendering/UI.
- When in doubt, preserve the current Macroquad/Bevy split.
