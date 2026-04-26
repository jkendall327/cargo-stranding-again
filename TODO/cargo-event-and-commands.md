# Cargo Events And Commands Migration

## Why Do This

The game wants the player and autonomous actors to follow the same cargo and
item rules. A player picking up a parcel, a porter picking up a parcel, and a
future vehicle/container interaction should all flow through the same rule
surface instead of each caller mutating cargo state in its own way.

The current code is still healthy, but the cargo path is beginning to lean on
exclusive `World` access:

- Player action code calls cargo helpers directly with `&mut World`.
- Porter job code calls the same helpers directly with `&mut World`.
- Cargo helpers inspect and mutate several entities internally.
- `Cargo.current_weight`, `CarriedBy`, `CargoStats`, `CargoParcel`, and
  `ParcelState` currently overlap as sources of cargo truth.

That works at prototype scale, but it makes the system harder to reason about
as more actors and item types arrive. It also encourages broad "service
locator" access to the ECS world rather than small systems with explicit data
dependencies.

Moving cargo actions to ECS events plus `Commands` gives us a cleaner boundary:

```text
actor systems decide what they want to try
cargo systems decide whether cargo rules allow it
commands apply the component changes
derived-cache systems refresh load totals
```

This preserves the important design principle: the player is just another actor
whose intent comes from human input.

## Events vs Commands

Events and `Commands` are complementary.

Events express game language:

```text
PickUpRequest
DropRequest
DeliverRequest
CargoChanged
PickUpSucceeded
PickUpFailed
```

They are useful when one system wants to announce a domain request or outcome
without knowing how another system implements it.

`Commands` express structural ECS mutation:

```text
insert CarriedBy
remove CarriedBy
insert/update ParcelState
spawn/despawn entities
```

They are deferred mutations applied by Bevy after the system/schedule reaches a
sync point. The cargo resolver can read events, validate rules, and then use
`Commands` to make the actual component changes.

In other words:

```text
Event = "an actor is trying to pick this up"
Command = "insert this component on that entity"
```

Actor systems should mostly know about events. Cargo resolver systems can know
about both.

## Preferred Shape

Use specific request events where the caller has already chosen the target:

```rust
#[derive(Event, Clone, Copy, Debug)]
pub struct PickUpRequest {
    pub actor: Entity,
    pub item: Entity,
    pub slot: CarrySlot,
}

#[derive(Event, Clone, Copy, Debug)]
pub struct DropRequest {
    pub actor: Entity,
    pub item: Entity,
    pub at: Position,
}

#[derive(Event, Clone, Copy, Debug)]
pub struct DeliverRequest {
    pub actor: Entity,
    pub item: Entity,
}
```

This keeps responsibility clear:

- Player/NPC intent systems choose what they are trying to interact with.
- Cargo systems decide if the action is legal.
- Cargo systems apply state changes and emit results.

Avoid making the cargo system responsible for interpreting player input such as
"pick up whatever is under me" unless there is a strong reason. Selection and
rules are separate concerns.

## Cargo Rule Resolver

A pickup resolver should validate at least:

- Actor has cargo capacity data.
- Item has `Item` and `CargoStats`.
- Item is not already carried.
- Parcel state allows this actor to pick it up.
- Target slot is legal and available once slots matter.
- Total derived load will not exceed capacity.

After validation, use `Commands` to apply structural changes:

```rust
commands.entity(request.item).insert(CarriedBy {
    holder: request.actor,
    slot: request.slot,
});
commands
    .entity(request.item)
    .insert(ParcelState::CarriedBy(request.actor));
```

The exact code will need to match the Bevy ECS version in the project. The
important part is that cargo mutation happens in one cargo-owned system, not in
player and NPC code separately.

## Derived Cargo Load

Long-term, `CarriedBy` plus item stats should be the source of truth.
`Cargo.current_weight` can remain temporarily, but treat it as a cache.

Recommended direction:

1. Emit `CargoChanged { actor }` after pickup/drop/delivery succeeds.
2. Run a cargo-cache refresh system after cargo command application.
3. Recompute `Cargo.current_weight` from carried items.

Eventually, if the cache becomes unnecessary or too easy to desync, remove
`current_weight` and query/derive load where needed. For now, keeping the cache
is reasonable because movement uses cargo load every action.

## Schedule Shape

A practical phase layout could be:

```text
Player Intent
  - player movement intent
  - player pickup/drop intent
  - player wait/mode/menu intent

Agent Intent
  - porter job planning
  - porter pickup/drop/deliver requests
  - porter movement requests

Action Resolution
  - resolve movement
  - resolve pickup requests
  - resolve drop requests
  - resolve delivery requests

Post Action Maintenance
  - apply deferred commands / schedule sync point
  - refresh cargo caches
  - spend energy for successful actions
  - update timeline / catch-up
```

This does not all need to happen in one refactor. The useful direction is to
move from "one controller switches over an enum and mutates everything" toward
"small systems react to facts and emit requests."

## Suggested Migration Plan

1. Add cargo request/result event types.

   Start with `PickUpRequest` because both player and porters already need it.
   Add drop/deliver after the pickup path feels right.

2. Add cargo resolver systems.

   Implement `resolve_pickup_requests` using explicit `Query` parameters and
   `Commands`. Keep existing cargo helper tests if useful, but start moving the
   actual mutation path into systems.

3. Convert player pickup to emit a request.

   The player system should find the intended item at the player's position and
   emit `PickUpRequest`. It should not directly mutate `CarriedBy`,
   `ParcelState`, or cargo weight.

4. Convert porter pickup/delivery to emit requests.

   Porter job logic can still choose targets and phases, but cargo relationship
   mutation should go through the same resolver as the player.

5. Refresh cargo caches as a dedicated post-action system.

   This reduces the number of places that can forget to update
   `Cargo.current_weight`.

6. Split `player_actions` into smaller real ECS systems.

   Move/wait/mode changes are good first candidates because they are mostly
   actor-local. Pickup/drop should follow once cargo requests exist.

7. Remove broad cargo helpers that take `&mut World` where possible.

   Keep pure helpers for validation/calculation. Prefer systems for ECS
   mutation.

## Notes And Tradeoffs

- `Commands` are deferred, so do not expect a component inserted via
  `commands.entity(...).insert(...)` to be visible later in the same system.
  Design the schedule so follow-up work happens in a later phase.

- Events are also phase-sensitive. They should be consumed in a clearly ordered
  resolver phase so action timing remains deterministic.

- A failed pickup should probably emit a result event eventually, even if the UI
  ignores it at first. This will help debugging and future feedback text.

- Energy spending needs a clear policy. Prefer spending energy from the resolver
  result, not from the request emitter, so failed requests can remain free if
  that is the intended rule.

- It is okay to keep `SimulationRunner` as an explicit orchestrator. The goal is
  not to remove all custom scheduling. The goal is to keep game rules inside
  small ECS systems with explicit data access.

## Desired End State

The player and NPCs submit the same kind of cargo requests, and one cargo rule
surface resolves them. Actor-specific code decides intent; shared cargo systems
decide legality and mutate relationships. This should make future item slots,
containers, vehicles, fragile cargo, theft, delivery, and NPC interaction feel
like additions to one model rather than special cases scattered across player
and agent code.
