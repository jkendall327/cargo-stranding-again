# AGENTS.md

This is a small Rust game using Macroquad for the window/input/render loop and
standalone Bevy ECS for deterministic simulation state.

## Useful Commands

- `cargo fmt --check`
- `cargo check`
- `cargo test`
- `cargo run`
- `cargo run --bin headless -- <commands>`

## Headless Harness

Use the headless harness when you need fast automated feedback on gameplay or
simulation changes without opening a Macroquad window.

Example:

```sh
cargo run --bin headless -- move east wait pickup
```

Supported commands include:

- `north`, `south`, `east`, `west`
- `n`, `s`, `e`, `w`
- `up`, `down`, `left`, `right`
- `move <direction>`
- `wait`
- `pickup`
- `sprint`

The harness prints one compact snapshot after each command, including turn,
timeline, player position, stamina, cargo, parcel counts, and delivered parcels.
It reuses the same `init_world` setup and simulation timeline system as the real
game, so it is the preferred smoke test after gameplay logic changes.

