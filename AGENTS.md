# AGENTS.md

This is a small Rust game using Macroquad for the window/input/render loop and
standalone Bevy ECS for deterministic simulation state.

## Useful Commands

- `cargo fmt --check`
- `cargo check`
- `cargo test`
- `cargo run`
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
