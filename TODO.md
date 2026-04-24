# Cargo Stranding Again TODO

This is the working roadmap distilled from `SERIOUS_GAME.md`.

The current codebase is a small but healthy Bevy ECS + Macroquad prototype. Keep the split:

- Macroquad owns the outer frame loop, input polling, windowing, and drawing.
- Bevy ECS owns deterministic-ish game state and simulation systems.
- Rendering manually queries ECS for now.
- Normal terrain should stay in map/chunk arrays, not become ECS entities.
- ECS entities are for things that behave: player, NPCs, cargo, containers, ropes, vehicles, fires, doors, buildables, etc.

Already done:

- [x] Raw key presses are mapped through `src/input.rs` `KeyBindings`.
- [x] Gameplay and menu input have separate abstract actions.
- [x] The frame loop writes compact `PlayerIntent` / `MenuInputState` resources.
- [x] Turns only advance when a player action actually consumes a turn.
- [x] The player has basic stamina and cargo-weight movement effects.
- [x] NPC porters can reserve, pick up, and deliver simple cargo parcels.

## North Star Architecture

Evolve the prototype around this pipeline:

```text
Raw input -> abstract intent/action -> movement/job/simulation consequences -> render
```

Avoid growing one large "movement plus terrain plus stamina plus cargo plus weather plus NPC" system. Prefer small domain concepts and explicit phases.

Potential future phases:

```text
Input Collection
Menu / UI Handling
Intent Resolution
Movement Resolution
Cargo / Balance Effects
NPC Planning
NPC Acting
Environment / Weather
Clock / Turn Advancement
Cleanup
```

## 1. Movement Resolver

Goal: move player and NPC movement through shared movement-resolution code instead of player-only and agent-only movement logic.

- [ ] Add a movement domain module, likely `src/movement.rs`.
- [ ] Introduce shared movement request/result types.
  - [ ] `MovementRequest { entity, direction, mode }` or a non-entity helper shape usable in tests.
  - [ ] `MovementOutcome::Moved`, `Blocked`, `InsufficientStamina`, etc.
  - [ ] Include the actual `dx/dy`, target position, terrain, stamina delta, and turn/cooldown cost in the result.
- [ ] Introduce `MovementMode`.
  - [ ] Start with `Walking`.
  - [ ] Leave room for `Sprinting`, `Crawling`, `Swimming`, `Climbing`, `Rappelling`, and `Falling`.
  - [ ] Do not build all modes until a feature uses them.
- [ ] Extract terrain/stamina/cargo movement calculation out of `systems::player_movement`.
- [ ] Make `systems::player_movement` consume `PlayerIntent` and call the shared resolver.
- [ ] Make `systems::agent_jobs` call the shared resolver or a shared passability/cost helper.
- [ ] Keep failed movement from consuming a turn.
- [ ] Add tests for the resolver itself.
  - [ ] Bounds blocked.
  - [ ] Water blocked.
  - [ ] Grass neutral.
  - [ ] Road restores stamina.
  - [ ] Mud/rock drain stamina.
  - [ ] Cargo load increases negative stamina costs.

Notes for future Codex runs:

- Current movement code lives in `src/systems.rs::player_movement`.
- Agent stepping lives in `greedy_step` and `step_delay` in `src/systems.rs`.
- Terrain stats currently live as methods on `src/map.rs::Terrain`.
- Be careful with Rust borrow rules if the resolver needs map data plus mutable components; a pure helper returning a decision is probably easiest first.

## 2. Expanded Player Actions

Goal: grow `PlayerAction` deliberately without turning it into input-shaped movement again.

- [ ] Add action variants only as systems exist to consume them.
- [ ] Candidate variants:
  - [ ] `Sprint(Direction)`
  - [ ] `Crawl(Direction)`
  - [ ] `PickUp`
  - [ ] `Drop`
  - [ ] `Interact`
  - [ ] `OpenInventory`
  - [ ] `AdjustBalance(BalanceShift)`
- [ ] Decide whether movement modes are selected by action (`Sprint(North)`) or by a persistent component (`MovementMode::Sprinting` plus `Move(North)`).
- [ ] Add a player movement-state component when the first non-walking mode needs persistence.
- [ ] Keep menu actions separate from gameplay actions.
- [ ] Keep contextual actions resolved after input, not inside keybinding lookup.

## 3. Serious Cargo Model

Goal: replace `Cargo { current_weight, max_weight }` as the core cargo model with entity relationships, carry slots, and derived load totals.

- [ ] Keep the current `Cargo` component temporarily as a compatibility/cache layer.
- [ ] Define cargo/item components.
  - [ ] `Item`
  - [ ] `CargoStats { weight, volume }`
  - [ ] Optional later: dimensions, fragility, stackability, rigidity, wetness, value.
- [ ] Define carrying/container relationship data.
  - [ ] `CarrySlot` enum: `HandLeft`, `HandRight`, `Back`, `Hip`, `Chest`, `Container(Entity)` or similar.
  - [ ] `CarriedBy { holder: Entity, slot: CarrySlot }`
  - [ ] `Container { volume_capacity, weight_capacity }`
  - [ ] `ContainedIn { container: Entity }`
- [ ] Rework `CargoParcel` / `ParcelState` to use the new item relationship model.
- [ ] Add pickup/drop systems.
  - [ ] Pick up a loose item at the actor position.
  - [ ] Fail clearly if the slot is occupied.
  - [ ] Fail clearly if weight/volume capacity is exceeded.
  - [ ] Drop carried item at the actor position.
- [ ] Add derived load calculation.
  - [ ] Compute total carried weight per actor.
  - [ ] Cache it into the existing `Cargo` component or replace `Cargo` with `Load`.
  - [ ] Ensure movement uses derived load, not hand-edited totals.
- [ ] Update NPC porter jobs to use pickup/drop relationship transitions instead of mutating `cargo.current_weight`.
- [ ] Render loose, assigned, and carried items clearly.
- [ ] Add tests.
  - [ ] Cannot pick up oversized cargo.
  - [ ] Cannot put cargo into a full container.
  - [ ] Dropping cargo makes it loose at the actor position.
  - [ ] NPC delivery still increments delivered parcel count.

Notes:

- Existing parcel state is in `src/components.rs::ParcelState`.
- Existing porter job code directly adds/subtracts parcel weight from `Cargo`.
- This is probably the largest near-term domain change.

## 4. Data-Driven Terrain And Items

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

## 5. NPC Goals And Jobs

Goal: move from hardcoded porter delivery behavior toward simple goal-driven agents.

- [ ] Split job assignment from job execution more clearly.
- [ ] Replace `AssignedJob { phase, parcel }` with a more general job/task representation when the second job type appears.
- [ ] Add explicit job targets.
  - [ ] target entity
  - [ ] target tile
  - [ ] delivery depot / destination
- [ ] Replace greedy movement with pathfinding.
  - [ ] Start with BFS or A* on the current fixed map.
  - [ ] Account for passability first.
  - [ ] Later account for terrain movement cost, stamina, load, and danger.
- [ ] Allow agents to fail or abandon jobs.
  - [ ] Parcel no longer exists.
  - [ ] Parcel already carried by someone else.
  - [ ] Destination unreachable.
  - [ ] Agent lacks capacity.
- [ ] Add at least one non-delivery goal later.
  - [ ] Rest/recover stamina.
  - [ ] Seek shelter.
  - [ ] Avoid weather.
  - [ ] Fetch container/tool.

Current code pointers:

- `assign_agent_jobs` reserves loose parcels.
- `agent_jobs` moves through `FindParcel`, `GoToParcel`, `GoToDepot`, `Done`.
- `greedy_step` is deliberately simple and should be replaced before worldgen gets serious.

## 6. Simulationist Body, Balance, And Weather

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

## 7. Chunked Map And Procgen

Goal: move from one fixed rectangle to deterministic, streamable world data.

- [ ] Introduce coordinate types.
  - [ ] `TileCoord`
  - [ ] `ChunkCoord`
  - [ ] Local tile coordinate inside chunk
- [ ] Introduce `Chunk`.
  - [ ] fixed width/height
  - [ ] terrain tile array
  - [ ] optional elevation/depth arrays later
- [ ] Introduce `WorldMap` or evolve `Map`.
  - [ ] active chunk storage
  - [ ] seed
  - [ ] chunk lookup by world tile coordinate
- [ ] Keep initial implementation compatible with the current generated map.
  - [ ] One chunk or a small fixed set of chunks is fine at first.
- [ ] Make procedural generation deterministic by seed and chunk coordinate.
- [ ] Add chunk load/unload boundaries around the camera or player.
- [ ] Add persistence for visited/modified chunks.
  - [ ] Start with a simple save directory or single save file.
  - [ ] Persist only changed chunks if generation is deterministic.
- [ ] Update rendering to draw visible tiles across chunk boundaries.
- [ ] Update pathfinding/movement to query world coordinates, not fixed `0..width` / `0..height` assumptions.
- [ ] Add tests.
  - [ ] Same seed + chunk coordinate produces same terrain.
  - [ ] World coordinate lookup crosses chunk boundaries correctly.
  - [ ] Modified chunk state round-trips through save/load.

Current code pointers:

- `src/map.rs::Map` assumes one fixed `width`, `height`, and `Vec<Terrain>`.
- `src/resources.rs::Camera` clamps to map width/height.
- Rendering loops over `camera.x..camera.x + camera.width`, using `map.terrain_at`.

## 8. Verticality

Goal: support cliffs, deep water, falling, climbing, ropes, rappelling, and slopes without pretending the current `Position { x, y }` is enough forever.

- [ ] Decide the verticality model before changing `Position`.
  - [ ] Option A: discrete z-levels (`Position { x, y, z }`).
  - [ ] Option B: 2D grid plus elevation/depth fields.
  - [ ] Likely target: both. Tile columns have elevation/depth; entities can have vertical state.
- [ ] Add tile elevation/depth first.
  - [ ] elevation
  - [ ] water depth
  - [ ] slope / grade derived from neighboring elevation
- [ ] Make movement outcomes account for slope.
  - [ ] Uphill costs more stamina.
  - [ ] Downhill can increase momentum or fall risk.
  - [ ] Steep cliffs block normal walking.
- [ ] Add vertical movement states later.
  - [ ] `Climbing`
  - [ ] `Rappelling`
  - [ ] `Falling`
  - [ ] `Swimming`
- [ ] Add rope/climbing entities only when there is an interaction loop for them.
- [ ] Update rendering with enough visual information to debug elevation.
- [ ] Add tests for cliff blocking, slope costs, and water depth behavior.

Note:

- Do not immediately rewrite the whole game as 3D. Add elevation/depth to terrain/chunks first, then entity z/vertical state when gameplay demands it.

## 9. UI, Menus, And Rebinding

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

## 10. Save/Load

Goal: add persistence before worldgen and cargo relationships become too large to reason about casually.

- [ ] Decide save scope.
  - [ ] Full world snapshot for early development.
  - [ ] Later: deterministic world seed plus changed chunks plus ECS entity state.
- [ ] Add serializable save structs instead of serializing ECS internals directly.
- [ ] Save core resources.
  - [ ] simulation clock
  - [ ] map/chunk seed and modified tiles
  - [ ] player position/stamina/load
  - [ ] loose/carried/delivered cargo state
  - [ ] NPC positions/jobs
- [ ] Add load path in startup/debug menu.
- [ ] Add round-trip tests.

## 11. Code Organization

Goal: keep the project pleasant as it stops being tiny.

- [ ] Split large domains into modules only when the code has real weight.
  - [ ] `movement.rs`
  - [ ] `cargo.rs`
  - [ ] `jobs.rs`
  - [ ] `terrain.rs` or `data.rs`
  - [ ] `worldgen.rs`
  - [ ] `save.rs`
- [ ] Keep `components.rs` from becoming an unstructured dumping ground.
  - [ ] Either group related components into modules or re-export from `components/mod.rs`.
- [ ] Add schedule construction helpers once schedules become more complex.
- [ ] Keep tests near the domain they verify.
- [ ] Prefer pure helper functions for calculations that need lots of tests.

## Suggested Implementation Order

1. Extract shared movement resolution.
2. Add `MovementMode::Walking` and prepare for sprint/crawl/swim.
3. Replace direct cargo-weight mutation with item/carry relationships.
4. Update NPC porter jobs to use the cargo model.
5. Add simple pathfinding for NPCs.
6. Move terrain stats into data definitions.
7. Add real options/keybinding UI and keybinding persistence.
8. Add save/load for the current fixed world.
9. Introduce chunk coordinate types and chunk-backed map storage.
10. Add deterministic chunk generation.
11. Add elevation/depth fields.
12. Add weather/balance systems once movement/cargo/world data have stable boundaries.

## Useful Guardrails

- Keep the game playable after each task.
- Keep `cargo check` and tests green after each slice.
- Prefer adding one new concept at a time over broad rewrites.
- If a task requires both UI and simulation, land the simulation first with tests, then expose it in rendering/UI.
- When in doubt, preserve the current Macroquad/Bevy split.
