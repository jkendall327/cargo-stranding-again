# Cargo Stranding Again

A tiny Rust MVP for a 2D simulationist logistics/survival roguelike.

It uses:

- `macroquad` for the async frame loop, windowing, input, and drawing.
- `bevy_ecs` standalone for game state and simulation systems.

## Run

```sh
cargo run
```

## Controls

- Move with `WASD` or arrow keys.
- Wait with `Space` or `.`.
- Toggle walking/sprinting with `Shift`.
- Pick up loose cargo on your tile with `E`.
- Pause and resume with `Esc`.
- In the pause menu, use `WASD`/arrow keys to choose `Resume` or `Options`, then `Enter`/`Space` to confirm.
- Water and map bounds block movement.
- Grass is stamina-neutral.
- Mud and rock drain stamina, with cargo making that drain worse.
- Roads and the depot restore stamina when traversed.
- Sprinting spends extra stamina and reduces movement energy cost.
- Timeline energy advances only when movement succeeds, pickup succeeds, or you wait. Failed movement and failed pickup do not spend energy.
- Waiting recovers stamina directly.

The prototype spawns loose cargo parcels, NPC porters, one depot, and simple delivery jobs. Porters greedily walk to parcels, pick them up, walk to the depot, and drop them off.
