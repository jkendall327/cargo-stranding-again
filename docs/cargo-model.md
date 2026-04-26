# Cargo Model

The cargo model treats cargo as normal ECS entities, not as numbers stored on an
actor. An actor does not "have 12 weight" because a field says so. It has load
because item entities are related to that actor through carry and container
relationships.

The useful mental model is:

```text
items are things
relationships say where those things are
derived queries answer how heavy an actor or container is
```

This keeps cargo physical. A parcel can be loose on the ground, reserved for a
porter job, carried by an actor, placed inside a carried container, dropped back
onto a tile, or delivered. Those are different pieces of state, not one giant
"cargo status" enum.

## Core Concepts

`Item` marks an entity as something that can participate in the cargo model. It
is intentionally generic. Parcels, backpacks, containers, tools, weapons, and
future supplies can all be items.

`CargoStats` describes the physical burden of an item:

```rust
weight: f32
volume: f32
```

Weight affects actor load. Volume matters when placing items inside containers.
Future properties like fragility, wetness, value, dimensions, or stackability
should extend item data without changing the basic relationship shape.

`Cargo` lives on actors. Despite the name, it is not the actor's current cargo
contents. It is the actor's carrying capability, currently just:

```rust
max_weight: f32
```

The current load is derived from item relationships. This is important because
there is one source of truth: the item entities themselves.

## Where An Item Is

The model currently uses three location shapes.

`Position` means the item is loose in the world at a tile. A loose parcel with a
position can be picked up by an actor standing on that same tile.

`CarriedBy` means the item is directly carried by an actor:

```rust
holder: Entity
slot: CarrySlot
```

The holder is the actor entity. The slot describes the body/carry point, such as
`Back` or `Chest`. Slots are deliberately small right now; hands, hips, rigs,
and more detailed attachment points can be added once gameplay needs them.

`ContainedIn` means the item is inside a container item:

```rust
container: Entity
```

The container itself is also an item. A container can be loose, carried directly
by an actor, or potentially placed somewhere else later. An item inside a carried
container contributes to the load of the actor carrying that container.

The intended invariant is that an item should have one physical location at a
time: loose with `Position`, directly carried with `CarriedBy`, or inside a
container with `ContainedIn`.

## Containers

`Container` marks an item as able to hold other items:

```rust
volume_capacity: f32
weight_capacity: f32
```

Container capacity is checked when placing an item inside it. The container's
own `CargoStats` still matter: a backpack can weigh something even when empty,
and the actor carrying it bears both the backpack's own weight and the weight of
its contents.

Bevy relationship metadata maintains `ContainerContents` from `ContainedIn`.
Most domain code should not need to hand-edit that target component. The source
relationship, `ContainedIn`, is the meaningful part of the model.

## Parcels And Delivery State

`CargoParcel` marks an item as a delivery parcel. It does not contain weight or
physical location. Those still come from `CargoStats` and the location
relationships.

`ParcelDelivery` describes delivery-job state:

```rust
Available
ReservedBy(Entity)
Delivered
```

This is not physical state. A parcel can be `ReservedBy(porter)` while still
being loose on the ground. Once a porter picks it up, the parcel's physical
state changes to `CarriedBy` or `ContainedIn`, while the delivery state still
explains why that porter is carrying it.

That separation is the key idea:

```text
Position / CarriedBy / ContainedIn = where the parcel physically is
ParcelDelivery = what delivery workflow thinks about it
```

Delivered parcels are removed from carry/container relationships and marked
`Delivered`. They currently do not render in the world.

## Load Derivation

Actor load is calculated from relationships.

Directly carried items count toward their holder:

```text
actor -> CarriedBy item -> CargoStats.weight
```

Contained items count toward the actor carrying their container:

```text
actor -> CarriedBy container -> ContainedIn item -> CargoStats.weight
```

The important consequence is that movement, UI, rendering, inventory, and tests
should ask "what load is derived from the relationships?" rather than updating a
cached `current_weight` field.

This avoids stale state. If an item is dropped, picked up, delivered, or moved
into a container, the relationship change is enough for the load calculation to
change too.

## Cargo Actions

Cargo mutations go through request/result systems.

`PickUpRequest` asks to move a loose item into either a carry slot or a
container. The resolver checks:

- the actor has cargo capacity
- the item exists and has cargo stats
- the item is available at the actor's position
- the target slot or container has room

`DropRequest` asks to place a carried or contained item at a world position.
Successful drops remove carry/container relationships and insert `Position`.

`DeliverRequest` asks to mark a carried or contained parcel as delivered.
Successful delivery removes physical carry/container relationships and marks the
parcel `Delivered`.

Each resolver emits `CargoActionResult`. Other systems react to those results to
spend action energy, update porter jobs, clamp inventory selection, update
delivery stats, or log failures. This keeps the cargo mutation rules separate
from the cross-cutting consequences.

## Player And NPC Symmetry

The cargo systems are actor-oriented, not player-only. A pickup request names an
actor entity. A drop request names an actor entity. A delivery request names an
actor entity.

That means the player and porters use the same relationship transitions and the
same capacity/load rules. This supports the roguelike-ish design goal that the
player follows the same rules as everyone else.

Porter jobs choose parcels, reserve them, walk toward them, request pickup, walk
to the depot, and request delivery. The cargo resolver owns whether those
requests are legal. The porter system owns porter intent, not special cargo
physics.

## Rendering And Inventory

Rendering reads the cargo model instead of inventing a separate display state.

Loose cargo renders from positioned item entities. Reserved loose parcels are
still positioned items, but their `ParcelDelivery::ReservedBy` state changes how
they are drawn. Carried cargo renders as a badge on the actor holding it,
including parcels inside carried containers.

Inventory follows the same rule. The player's inventory is built by finding
parcel entities carried directly by the player or contained inside containers
the player carries.

The UI may simplify how this is shown, but it should not become a second cargo
model.

## Design Rule

When adding a new cargo feature, first decide which part of the model it belongs
to:

- physical item data belongs on the item
- physical location belongs in `Position`, `CarriedBy`, or `ContainedIn`
- actor carrying limits belong on the actor's `Cargo`
- delivery workflow belongs in `ParcelDelivery` or future job components
- derived facts like current load should be calculated from relationships

If a new field starts duplicating one of those facts, pause and ask whether it
is cache, workflow state, or accidental second source of truth.
