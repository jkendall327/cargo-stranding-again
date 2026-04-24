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
- Water and map bounds block movement.
- Mud, rock, cargo weight, and roads affect stamina cost.
- Waiting recovers stamina slowly.

The prototype spawns loose cargo parcels, NPC porters, one depot, and simple delivery jobs. Porters greedily walk to parcels, pick them up, walk to the depot, and drop them off.
