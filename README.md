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
- Water and map bounds block movement.
- Mud, rock, cargo weight, and roads affect stamina cost.
- Turns advance only when movement succeeds or you wait. Failed movement does not consume a turn.
- Waiting recovers stamina slowly.

The prototype spawns loose cargo parcels, NPC porters, one depot, and simple delivery jobs. Porters greedily walk to parcels, pick them up, walk to the depot, and drop them off.
