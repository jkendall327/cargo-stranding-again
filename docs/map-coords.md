# Map Coordinates And Chunks

The map uses a few coordinate types so game logic can talk about world tiles
without caring how those tiles are stored.

## Coordinate Types

`TileCoord` is a world tile coordinate. This is the coordinate space gameplay
uses. If the player is at `(48, 30)`, that is a world tile coordinate. Movement,
terrain lookup, pathfinding, parcel placement, and landmarks should all ask map
questions in this space.

`ChunkCoord` identifies a chunk in the world. Chunks are fixed-size blocks of
terrain data. With the current `16x16` chunks, world tiles `(0..15, 0..15)` live
in chunk `(0, 0)`, tiles `(16..31, 0..15)` live in chunk `(1, 0)`, and world
tile `(48, 30)` lives in chunk `(3, 1)`.

`LocalTileCoord` is a tile coordinate inside one chunk. It is always relative to
that chunk, not the whole world. World tile `(48, 30)` becomes:

```text
TileCoord      { x: 48, y: 30 }
ChunkCoord     { x: 3,  y: 1  }
LocalTileCoord { x: 0,  y: 14 }
```

That comes from Euclidean division and remainder:

```text
48 / 16 = 3, 48 % 16 = 0
30 / 16 = 1, 30 % 16 = 14
```

The code uses Euclidean division/remainder so negative world coordinates will
also split correctly when the game later supports generated or streamed chunks
outside the current finite map.

## Tile Lookup

Callers should query the map with world tile coordinates:

```rust
map.tile_at_coord(TileCoord::new(48, 30))
```

Internally, `Map` converts that world tile into chunk-local storage:

```text
TileCoord(48, 30)
  -> ChunkCoord(3, 1)
  -> LocalTileCoord(0, 14)
  -> look up chunk (3, 1)
  -> look up tile (0, 14) inside that chunk
```

This keeps gameplay code independent from chunk boundaries. Movement can ask
"can I move from this world tile to that world tile?" and rendering can ask for
visible world tiles. Neither one needs to know which chunk each tile lives in.

## Current World Behavior

Right now the map is still finite and compatible with the original hardcoded
`60x40` generated world. `Map::generate()` builds that world into chunk-backed
storage up front.

Coordinates outside the current finite bounds return `None`, which means they
block movement and do not render as terrain. Camera centering still clamps to
the finite map bounds.

## Why This Exists

Chunk-backed storage prepares the game for a larger world without making every
system understand streaming details.

Future systems can use `ChunkCoord` as the unit for:

- loading nearby chunks from disk
- unloading chunks far from the player or camera
- generating missing chunks from `(seed, ChunkCoord)`
- saving only visited or modified chunks
- pathfinding and rendering across chunk boundaries

The intended boundary is:

- gameplay uses `TileCoord`
- map storage uses `ChunkCoord` plus `LocalTileCoord`
- `Map` translates between them

That lets the rest of the game keep treating the world as one continuous tile
space while the storage layer becomes streamable.
