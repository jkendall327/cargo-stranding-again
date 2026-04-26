# AGENTS.md

This is a small Rust game using Macroquad for the window/input/render loop and
standalone Bevy ECS for deterministic simulation state.

Look at the TODO.md file. It contains a big task list for stuff we're going to be doing.
If I mentioned numbers like '#3' I'm referring to stuff here.

You can use your `mcp__rust_lsp_plugin__` tool for some AST-style codebase manipulation stuff, probably cheaper than burning tokens doing things the hard way. Don't feel pressured to use it, but be aware it exists.

## Philosophy

I want this to be a 'serious' codebase with legs, so I want to do things the 'right way', even if it means a bit more upfront complexity.

I'm not very familiar with ECS patterns, so warn me if we're falling into antipatterns there.

I want to follow the old rougelike philosophy of 'the player follows the same rules as everyone else', as much as possible. So systems should, if appropriate, not be scoped just to the player, but theoretically be available to all entities.

This entails that systems should be scoped tightly in a nice ECS way, rather than big 'do everything' blobs.

I want to stay somewhat familiar with the code. To that end, leave *some* doc comments on important structs and functions.
You don't need to go crazy with it; I don't care about having line-by-line logic explained.
I care more about the 'why' of it all.

## Codebase Tour

Start with `src/app.rs` when you want the frame-level story. `Game::run_frame`
copies Macroquad input into ECS resources, runs menu handling, advances the
energy timeline, optionally resolves the player's intent, and finally calls the
plain Macroquad renderer. `src/main.rs` is only the window entry point.

World initialization lives in `src/world_setup.rs`. It inserts long-lived
resources, spawns the player, NPC porters, starter parcels, and the generated
map. Shared ECS data is mostly split between `src/components.rs` for entity
components and `src/resources.rs` for singleton state such as the current
screen, input intents, menu selections, camera, clock, and energy timeline.

The simulation orchestration is in `src/simulation.rs` and `src/schedules.rs`.
Those files define the small Bevy schedules for player actions, agent actions,
and menus. Timeline-specific rules live in `src/systems/timeline.rs`: this is
where player readiness, turn advancement, and NPC catch-up are coordinated.

Gameplay rules that should stay reusable or easy to test live outside systems.
`src/map.rs` owns terrain, passability, movement cost, stamina effects, and the
current deterministic map generation. `src/movement.rs` is the shared movement
resolver used by both the player and agents; it answers "can this actor enter
that tile, and what did it cost?" `src/energy.rs` defines action-energy costs
and `ActionEnergy`. `src/momentum.rs` handles momentum state, straight-line
discounts, turning penalties, and cargo-loss risk.

ECS mutation happens under `src/systems/`. `src/systems/player.rs` consumes
`PlayerIntent` and delegates details to `src/systems/player/movement.rs` and
`src/systems/player/cargo.rs`. `src/systems/agents.rs` assigns porter jobs,
reserves parcels, moves agents greedily, and delivers cargo to the depot.
`src/systems/menu.rs` translates menu input into screen/selection changes, and
`src/systems/inventory.rs` performs inventory actions such as dropping the
selected carried parcel.

Input is intentionally thin. `src/input.rs` maps raw Macroquad keys through
`KeyBindings` into compact `PlayerIntent` and `MenuInputState` resources,
including held-action repeat behavior. Add new controls there only after adding
the abstract action/resource shape that will consume them.

Rendering is deliberately not a Bevy schedule. `src/render.rs` manually queries
the ECS world and draws the map, entities, UI, and overlays through Macroquad.
If a visual looks wrong but the sim snapshot is right, look here first.

The fast test harness is in `src/headless.rs`, with the CLI wrapper at
`src/bin/headless.rs`. It uses the same `init_world` and `SimulationRunner` as
the real game, then exposes snapshots and ASCII views for smoke scenarios under
`scenarios/headless/`.

## Useful Commands

Run the normal verification suite with one command:

- `cargo run --bin xtask -- verify`

That command runs `cargo fmt --check`, `cargo clippy --all-targets`, and
`cargo test`.

Headless scenario smoke tests are separate from normal verification:

- `cargo run --bin headless -- <commands>`
- `cargo run --bin headless -- all`

## Headless Harness

Use the headless harness when you need fast automated feedback on gameplay or
simulation changes without opening a Macroquad window.

Example:

```sh
cargo run --bin headless -- mode move east wait pickup
```

Run all JSON smoke scenarios:

```sh
cargo run --bin headless -- all
```

Headless logs are quiet by default. Set `RUST_LOG` when you need tracing output.

Run one scenario file:

```sh
cargo run --bin headless -- --scenario scenarios/headless/walk-east.json
```

Scenario files live in `scenarios/headless`. They contain a list of commands and
an `expect` object checked against the final snapshot. Failed scenarios print a
final ASCII camera view automatically. Add `"view": true` to print that view on
success too. Commands can be plain strings or structured repeats:

```json
{
  "name": "repeated wait",
  "view": true,
  "commands": [
    {
      "repeat": 3,
      "command": "wait"
    }
  ],
  "expect": {
    "turn": 3,
    "player_position": {
      "x": 6,
      "y": 6
    }
  }
}
```

Supported commands include:

- `north`, `south`, `east`, `west`
- `n`, `s`, `e`, `w`
- `up`, `down`, `left`, `right`
- `move <direction>`
- `wait`
- `pickup`
- `mode`

The harness prints one compact snapshot after each command, including turn,
timeline, player position, stamina, cargo, parcel counts, and delivered parcels.
It reuses the same `init_world` setup and simulation timeline system as the real
game, so it is the preferred smoke test after gameplay logic changes.
