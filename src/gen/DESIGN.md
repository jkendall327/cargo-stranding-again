# Worldgen Influence Design

This folder is for pure Rust world/history generation experiments. The current
slice models civilisational influence as a coarse grid plus a settlement graph.
It deliberately does not use ECS: history generation should produce persistent
world facts that gameplay can later consume, not become part of the frame-to-
frame actor simulation.

## Moving Parts

- `InfluenceWorld` is the whole sketchable state for this model: current year,
  bounds, civilisations, settlements, and cell claims.
- `Civilisation` is a durable world-level record. For now it only has an id,
  name, and `vigor`, which acts as a broad expansion multiplier.
- `Settlement` is the graph node. It belongs to a civilisation, has a
  `strength`, may link to other settlements, and has a `footprint` that can
  cover more than one coordinate.
- `InfluenceClaim` is the current owner/strength of one coarse cell.
- `InfluenceRules` contains the tuning knobs for a history tick.
- `InfluenceReport` summarizes what changed during one tick, mostly so tests
  and future inspection tools can see expansion, contraction, and conversion.

## Pressure

Pressure is temporary per-year influence projected from active settlements. It
is not stored directly in the world. A settlement projects pressure into nearby
cells using:

```text
settlement strength * civilisation vigor - distance decay
```

Cells are claimed only if the winning pressure is strong enough and beats the
runner-up by enough margin. This keeps empty areas from being filled by tiny
edge noise and makes border fights less jittery.

Existing claims also get `held_tile_inertia`, a small home-field advantage. The
intent is that borders should bob and grind instead of flipping every time two
neighbours are nearly tied.

Unsupported claims decay each year. This gives contraction a simple shape:
settlements that die, convert, or become isolated stop maintaining their old
hinterlands, and those claims fade unless some active settlement supports them.

## Why Manhattan Distance

The first implementation uses Manhattan distance and cardinal neighbours
because the eventual game map is tile based and early path/movement logic is
also cardinal. That makes the prototype easy to reason about:

- influence radius forms a diamond on the coarse grid,
- route reinforcement follows simple orthogonal paths,
- tests can assert exact cells without needing floating point geometry,
- later terrain/path costs can replace distance without changing the public
  shape of the model.

This is not a claim that civilisation spreads in tidy diamonds. It is a cheap,
deterministic stand-in until geography matters more. A later pass could swap
the distance function for Dijkstra over coarse terrain, river crossings, passes,
or path quality.

## Settlement Graph

Settlements are the durable nodes of civilisation. Their links are not roads as
map tiles, but they are enough to say "these places reinforce each other." A
linked pair currently adds extra pressure along a simple Manhattan route between
their centers.

That route bonus is intentionally stored as influence rather than as generated
terrain. Later, the same graph can feed path quality, ruins, depots, traffic,
job generation, and road realization when chunks are built.

## Expansion, Fighting, Contraction

One history tick currently runs in three phases:

1. Active settlements project pressure.
2. Candidate cells resolve ownership.
3. Settlements react to their local control.

Expansion happens when a civilisation wins an empty cell above the growth
threshold. Fighting happens when another civilisation beats the current owner
by the takeover margin. Contraction happens when no active pressure supports an
old claim and it decays away.

Settlements look at local control around their footprint after claims resolve.
If their own civilisation has lost enough surrounding influence and another
civilisation is locally dominant, the settlement converts. If no one is locally
dominant and its own support is too low, it is abandoned instead.

This gives a first-pass version of:

- strong centers pushing into empty space,
- borders changing hands under sustained pressure,
- isolated claims fading,
- smaller or weaker settlements being absorbed or lost sooner.

## Coordinates And Scale

`GenCoord` is intentionally separate from gameplay `TileCoord` and `ChunkCoord`.
Right now it means "one coarse influence cell." Later it can map to a chunk,
several chunks, or a true world-plan cell.

Keeping it separate makes this model easier to tune before committing to the
exact scale of historygen versus chunk realization.

## Future Directions

Useful next steps:

- Replace Manhattan distance with terrain-aware coarse path cost.
- Add geography weights: rivers, fertile cells, coastlines, crossings, passes.
- Split settlement `strength` into population, defensibility, prosperity, or
  administrative reach if gameplay starts needing those distinctions.
- Persist ownership histories and foundation/conversion/abandonment years.
- Add inspection output for ASCII maps of claims, pressure, settlements, and
  contested borders.
- Feed stable claims and settlement links into path quality generation.

