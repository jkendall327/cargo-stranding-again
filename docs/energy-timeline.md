# Energy Timeline

The simulation uses an energy timeline to decide when actors can act. It is
still player-paced from the outside, because the player provides input one
action at a time, but NPCs use the same readiness model while catching up
between player actions.

The useful mental model is:

```text
systems perform actions
the timeline decides when systems run
```

## Moving Parts

`EnergyTimeline` is the global simulation time. Its `now` value is not wall
clock time and it is not the rendered frame count. It is the current timestamp
inside the deterministic simulation.

`ActionEnergy` is an actor component. It stores:

```rust
ready_at: u64
last_cost: u32
```

An actor is ready when:

```rust
ready_at <= EnergyTimeline.now
```

When an actor successfully performs an action, it spends energy:

```rust
ready_at = now + cost
```

The action cost comes from the rule that resolved the action. Walking, sprinting,
steady movement, waiting, pickup, and inventory drop all spend action energy
when they succeed. Failed actions generally do not spend energy yet.

## Player Pacing

The outer game loop is still input-paced. A rendered frame copies Macroquad
input into ECS resources, menu systems run, and then player simulation only
advances when there is a `PlayerIntent`.

When the player tries to act, the simulation runner does this:

```text
if the player is not ready:
    advance simulation time through NPC-ready moments
    stop at the player's ready_at timestamp

run the player action once

if the player spent energy:
    increment the turn counter
    advance simulation time through NPC-ready moments
    stop at the player's new ready_at timestamp
```

This means the player remains the pacing boundary, but NPCs do not simply get
"one turn per player turn." They act according to their own `ActionEnergy`
timestamps during the time that elapses between player-ready moments.

## Agent Catch-Up

Agent catch-up is coordinated in `src/systems/timeline.rs`.

The timeline asks for the next actionable agent ready time up to the player's
next ready timestamp:

```text
next_ready_at = minimum actionable agent ready_at <= player_ready_at
```

Then it advances `EnergyTimeline.now` to that timestamp and runs the agent
schedule once. This repeats until no actionable agent is ready before the player
is ready.

At the end, `EnergyTimeline.now` is set to the player's `ready_at` timestamp.
That keeps the player action code simple: if input is being resolved, the player
should already be ready.

## Agent Actions

`src/systems/agents.rs` intentionally does not own the catch-up loop.

`agent_jobs` loops over agents and lets each ready agent perform at most one job
action at the current `EnergyTimeline.now`:

```text
for each agent:
    if not ready at now:
        skip

    if ready:
        perform one job action
        spend energy from now
```

One job action means one of these:

- pick up a parcel
- move one tile toward the assigned parcel
- move one tile toward the depot
- deliver a carried parcel
- spend a default action cost when blocked

This is important. Agents used to contain an internal catch-up loop with a
fixed upper bound. That made the agent system both "agent behavior" and
"scheduler." Now the timeline owns scheduling, and the agent system only answers
"what does a ready agent do at this timestamp?"

## Same-Timestamp Batching

The current scheduler uses a simple batch rule: when several agents are ready at
the same timestamp, the agent schedule can process all of them once in the same
schedule run.

For this project, that is the right amount of precision. It keeps the ECS system
shape simple and deterministic enough for the current porter behavior.

If exact per-entity tie-breaking becomes important later, the next step would be
to have the timeline choose one active actor at a time. That would add more
ceremony, so it is not worth doing until actor order has gameplay consequences.

## Actionable Agents

Not every ready agent should cause timeline work.

An idle porter may have an old `ready_at` timestamp, but if there are no loose
parcels and it has no active job, it has nothing useful to do. The timeline
therefore looks for actionable agents, not merely ready agents.

An agent is currently actionable if:

- there is a loose parcel it could be assigned, or
- it already has an active parcel job that is not `FindParcel` or `Done`

This prevents idle agents from creating empty catch-up loops.

## Energy Vs Turn Vs Momentum

These concepts are related but deliberately separate.

`EnergyTimeline.now` is simulation time.

`ActionEnergy.ready_at` answers when an actor can act again.

`SimulationClock.turn` is a player-facing turn count. It increments when the
player successfully spends action energy.

`Momentum` is body state and stability risk. It affects movement consequences,
turning penalties, and cargo-loss risk, but it is not the scheduler.

`MovementMode` is the player's chosen posture or effort, such as walking,
sprinting, or steady movement. It affects movement cost and stamina behavior,
but the resulting movement action still reports a normal action energy cost.

Keeping these apart avoids one system becoming responsible for every gameplay
idea that happens to change how movement feels.

## Code Map

The frame-level flow starts in `src/app.rs`, where input is copied into ECS and
the simulation runner is invoked only when the current screen allows simulation.

`src/simulation.rs` owns the high-level player action phase. It asks timeline
helpers to advance to the player-ready timestamp before and after player
actions.

`src/systems/timeline.rs` owns time advancement. It moves
`EnergyTimeline.now`, runs ready agents until the player is ready, and increments
the player-facing turn count after successful player actions.

`src/systems/player.rs` consumes `PlayerIntent`. Successful player actions spend
the player's `ActionEnergy`.

`src/systems/agents.rs` consumes ready agent jobs. It performs one action per
ready agent at the current timestamp.

`src/energy.rs` defines `ActionEnergy` and the baseline action costs.

`src/movement.rs` resolves single-tile movement and reports the energy cost of
successful movement.

## Design Rule

When adding a new actor or action, try to preserve this split:

```text
action system:
    decide what happens for one ready actor at the current timestamp
    spend that actor's ActionEnergy if the action succeeds

timeline system:
    decide which timestamps should be simulated before the player acts again
```

If an action system starts looping over time, repeatedly advancing the same
actor, or inventing its own maximum catch-up count, that logic probably belongs
in the timeline instead.
