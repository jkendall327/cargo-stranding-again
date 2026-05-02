# AGENTS.md

This is a Rust game using Macroquad for the window/input/render loop and standalone Bevy ECS for simulation state.

Use your github MCP tools to access tickets; there is no github CLI installed locally.

## Verification

Verify your work with `cargo run --bin xtask -- verify`.

That command runs `cargo fmt`, `cargo clippy --all-targets`, and `cargo test`. It also includes the headless scenario commands explained later.

You shouldn't need to run `cargo fmt` yourself manually because of this; just let the xtask handle it unless something crops up.

## Philosophy

I want this to be a 'serious' codebase with legs, so I want to do things the 'right way', even if it means a bit more upfront complexity.

I'm not very familiar with ECS patterns, so warn me if we're falling into antipatterns there.

I want to follow the old rougelike philosophy of 'the player follows the same rules as everyone else', as much as possible. So systems should, if appropriate, not be scoped just to the player, but theoretically be available to all entities.

This entails that systems should be scoped tightly in a nice ECS way, rather than big 'do everything' blobs.

I want to stay somewhat familiar with the code. To that end, leave *some* doc comments on important structs and functions.
You don't need to go crazy with it; I don't care about having line-by-line logic explained.
I care more about the 'why' of it all.

Tests are great, obviously, but I'm also OK with some debug_asserts to catch any sneaky invariant failures. Throw in a few if you think they are high ROI.

## Headless Harness

The headless harness provides automated feedback on gameplay or simulation changes without opening a Macroquad window.

Prefer `cargo run --bin xtask -- verify` over running it directly. But you can if you want to.

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

## Glossary

These are *domain* terms, not terms as they are used in the code.

Entity: something that persists over time in the game-world.

Agent: an entity able to act autonomously, move, interact with the environment. A rock isn't an agent. People, animals and the wind are agents.

NPC: an agent who isn't the player.

Porter: an NPC who delivers parcels.

Item: an object that an agent can pick and use.

Container: an item which holds other items within it.

Cargo: an object that an agent can pick up, use and carry on their person.

Parcel: cargo that an agent delivers to a destination as part of a job. Cargo can cease to be a parcel if a job is cancelled.

Carry slot: a place on an agent's body where they can carry cargo. E.g. back, hands, head.

Depot: a place where parcels can be delivered.

Goal: a state of affairs that an agent strives to make real in the game-world.

Destination: a place an agent tries to reach as part of a goal.

Job: a goal for an agent to deliver parcels to a depot.

Energy: representation of how much work an agent is able to perform in an arbitrary unit of time in the game-world.

Stamina: how much physical action an agent can undertake before exertion.

Movement mode: an agent's style of movement. E.g. crawling, walking, sprinting.

Chunk: a small portion of the world that is stored, saved and loaded independently.
