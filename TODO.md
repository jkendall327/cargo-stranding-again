# Cargo Stranding Again TODO

This is the working roadmap distilled from `SERIOUS_GAME.md`.

The current codebase is a small but healthy Bevy ECS + Macroquad prototype. Keep the split:

- Macroquad owns the outer frame loop, input polling, windowing, and drawing.
- Bevy ECS owns deterministic-ish game state and simulation systems.
- Rendering manually queries ECS for now.
- Normal terrain should stay in map/chunk arrays, not become ECS entities.
- ECS entities are for things that behave: player, NPCs, cargo, containers, ropes, vehicles, fires, doors, buildables, etc.

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

Status: done.

## 4. Serious Cargo Model

Goal: replace `Cargo { current_weight, max_weight }` as the core cargo model with entity relationships, carry slots, and derived load totals.

Status: done.

## 5. Data-Driven Terrain And Items

Goal: stop hardcoding every terrain/item stat in Rust once the domain model settles enough.

- [x] Add `serde` and a data format.
  - [x] Prefer RON or TOML for hand-authored game data.
  - [ ] JSON is fine if tool simplicity matters more.
- [ ] Introduce stable terrain IDs in the map.
  - [ ] Keep the existing `Terrain` enum until there is a clear benefit to replacing it.
  - [ ] Consider `TerrainId` plus `TerrainDefinition`.
- [x] Move terrain definitions out of `Terrain` methods.
  - [x] movement cost
  - [x] stamina delta
  - [x] passability
  - [x] color/glyph, unless rendering gets its own definition layer
  - [ ] later: elevation behavior, wetness, exposure, wind shelter, traction
- [ ] Add a `TerrainDefinitions` resource.
- [x] Load default terrain definitions at startup.
- [ ] Decide fallback behavior if data files are missing or invalid.
  - [ ] For development, panic with useful errors is acceptable.
  - [ ] For release, fall back to embedded defaults or show an error screen.
- [x] Add item/cargo definitions after the cargo model exists.
  - [x] item ID
  - [x] display name
  - [x] weight
  - [x] volume
  - [x] cargo tags/properties
- [x] Add tests for loading and validating definitions.

## 6. NPC Goals And Jobs

Goal: move from hardcoded porter delivery behavior toward simple goal-driven agents.

- [ ] Split job assignment from job execution more clearly.
- [ ] Replace `AssignedJob { phase, parcel }` with a more general job/task representation when the second job type appears.
- [ ] Add explicit job targets.
  - [ ] target entity
  - [ ] target tile
  - [ ] delivery depot / destination
- [x] Replace greedy movement with pathfinding.
  - [x] Consider adding the `pathfinding` crate for BFS/A*/Dijkstra rather than hand-rolling graph search.
  - [x] Add a small `pathing` module that converts `Map` passability/costs into pathfinding successors.
  - [x] Start with BFS or A* on the current fixed map.
  - [x] Account for passability first.
  - [ ] Later account for stamina budgets, actor-specific load constraints, and danger.
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
- `ai::pathing` plans porter movement with A* using the shared movement resolver.

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

Status: basically done. Only remaining task is 'persist only changed chunks if/when loaded chunk volume needs it'.

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

Status: persistence has a versioned world/character split, stable persistent IDs,
chunk files, cargo/player/NPC save data, migration scaffolding, round-trip tests,
and a pause-menu debug save/load flow.

Minor/future work:

- [ ] Replace the single debug slot with a real world/character picker.
- [ ] Add save metadata for display names, timestamps, and build/debug labels.
- [ ] Later: persist only changed chunks if loaded chunk volume needs it.
- [ ] Later: support long-term world history with multiple characters per world.

## 12. True Worldgen / Historygen

Dwarf Fortress style simulation of a planet's history over N years.
From geological change--the rise of mountains and the running of rivers--to the social.
Kingdoms fall and counties merge. People murder and fall in love.

Status: ideation.

## Useful Guardrails

- Keep the game playable after each task.
- Keep `cargo check` and tests green after each slice.
- Prefer adding one new concept at a time over broad rewrites.
- If a task requires both UI and simulation, land the simulation first with tests, then expose it in rendering/UI.
- When in doubt, preserve the current Macroquad/Bevy split.
