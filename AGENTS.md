# AGENTS.md

This is a small Rust game using Macroquad for the window/input/render loop and
standalone Bevy ECS for deterministic simulation state.

Look at the TODO.md file. It contains a big task list for stuff we're going to be doing.
If I mentioned numbers like '#3' I'm referring to stuff here.

Look at glossary.md too. It's where I distinguish between things like 'cargo' and 'parcel'.

You can use your `mcp__rust_lsp_plugin__` tool for some AST-style codebase manipulation stuff, probably cheaper than burning tokens doing things the hard way. Don't feel pressured to use it, but be aware it exists.

## Verification

Verify your work with this command:

- `cargo run --bin xtask -- verify`

That command runs `cargo fmt --check`, `cargo clippy --all-targets`, and
`cargo test` the headless scenario commands explained later.

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
