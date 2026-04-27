Let's go sicko psycho mode and ideate on what a generalised AI system might look
  like for this game. I think the basic metaphor of goal-seeking behaviour works
  well and is general enough. I want to encompass things like other porters (as we
  have now), harmful/harmless wildlife, monsters, idle NPCs in towns and such. So
  to abandon goals, fail goals, interrupt their goals when something higher
  priority comes up. I don't think we need to go down the route of modelling
  desires and psychology; we can probably just say things like 'seeing a monster
  triggers the 'flee' goal which has a high priority'. So goals would be a priority
  queue of sorts for them. How much of what I've said so far can be expressed in
  the language of standard work in prior art? Like, I'm guessing there are clear
  models we can follow for at least some of this.

***

Yes, a lot of what you described maps very cleanly onto established game-AI patterns. The really good news: you’re not inventing a weird bespoke monster. You’re circling a known family of designs.

The closest prior-art names are:

**Goal-Oriented Action Planning / GOAP**
This is probably the main one. Agents have goals, actions have preconditions/effects/costs, and the AI picks a plan that transforms the current world state into the goal state. Classic examples: “I want to deliver parcel” means “find parcel -> move to parcel -> pick up -> move to depot -> deliver.” If a precondition breaks, the plan is invalidated and replanned.

This fits porters beautifully, especially once there are multiple job types.

**Utility AI**
This covers your “seeing a monster triggers flee with high priority” instinct. Instead of a strict scripted tree, each possible goal gets a score based on context:

```text
Deliver parcel: 45
Rest: 30
Flee monster: 900
Investigate sound: 20
Idle: 1
```

Then the agent selects the highest-scoring goal, with hysteresis/cooldowns so they don’t thrash every turn. This is very good for wildlife, monsters, town NPCs, and “soft” behavior.

**Behavior Trees**
These are commonly used for “how do I execute this goal once chosen?” They’re less about choosing grand intent and more about structured execution:

```text
DeliverParcel
  Sequence:
    HasAssignedParcel?
    MoveToParcel
    PickUpParcel
    MoveToDepot
    Deliver
```

Behavior trees are good, but for this game I’d avoid making them the whole AI architecture. They can become big blobs if every creature gets one giant tree. I’d rather use them, if at all, as goal executors.

**Hierarchical FSMs**
The current `AssignedJob { phase }` is basically a tiny finite state machine:

```text
FindParcel -> GoToParcel -> GoToDepot -> Done
```

That is not bad. It’s a perfectly respectable shape for simple jobs. The issue is that it hardcodes one job type. A generalized version could be “active goal has an execution state,” not “porter has parcel-specific phase.”

**HTN / Hierarchical Task Network Planning**
This is like GOAP’s more authored cousin. A high-level task decomposes into subtasks:

```text
DeliverCargo
  -> AcquireCargo
  -> TravelToDestination
  -> HandOffCargo
```

For your game, HTN might be more approachable than full GOAP because the world is simulation-heavy but still authored. You can define decompositions manually and keep behavior legible.

**Blackboard / Working Memory**
This maps to “agents having a notion of goals which they keep in their head.” More precisely, agents often have memory/perception state:

```rust
KnownThreats
KnownJobs
LastSeenPlayer
HomePosition
CurrentGoal
GoalQueue
```

The blackboard is shared-ish state that evaluators and behaviors read/write. In ECS terms, I’d probably avoid one huge “AI brain” component and instead use focused components/resources: `Perception`, `GoalStack`, `KnownThreats`, `AssignedTask`, etc.

The model I’d suggest for this game is a hybrid:

```text
Perception systems update what the actor knows
Goal evaluators propose/score goals
Arbitration picks/interrupts goals
Goal execution turns selected goals into actions
Shared action systems resolve movement, pickup, delivery, attack, flee, wait, etc.
```

So conceptually:

```text
World state
  -> perception
  -> candidate goals
  -> priority / utility arbitration
  -> active goal
  -> plan / task state
  -> concrete action request
  -> existing simulation systems
```

This preserves your roguelike philosophy: the AI does not teleport into special-case behavior. It emits the same kind of action requests the player does.

A nice ECS-ish version might look like:

```rust
struct AiActor;

struct GoalMemory {
    active: Option<GoalInstance>,
    candidates: Vec<GoalCandidate>,
}

enum GoalKind {
    DeliverParcel,
    FleeThreat,
    Rest,
    Wander,
    Hunt,
    Graze,
    Patrol,
    IdleAtTown,
}

struct GoalCandidate {
    kind: GoalKind,
    priority: i32,
    reason: GoalReason,
}

struct GoalInstance {
    kind: GoalKind,
    target: GoalTarget,
    state: GoalState,
    interruptible: bool,
}
```

Then systems stay tight:

```text
sense_threats
score_flee_goals
score_delivery_goals
score_rest_goals
choose_active_goal
execute_flee_goal
execute_delivery_goal
execute_idle_goal
```

The crucial distinction: **goal selection** and **goal execution** should be separate.

For example:

```text
Flee goal selection:
  “There is a hostile monster within 6 tiles, score 900.”

Flee goal execution:
  “Pick a tile farther from the monster, path there, spend movement actions.”
```

That keeps the system extensible. Wildlife, monsters, porters, and villagers can all use the same machinery but have different evaluators or different goal weights.

For abandonment/failure/interruption, standard terms would be:

**Plan invalidation**
The chosen plan no longer works. Parcel disappeared, route blocked, target died, actor lacks stamina.

**Goal preemption**
A higher-priority goal interrupts the current goal. Flee interrupts delivery.

**Goal failure**
The goal tried and concluded it cannot complete. Destination unreachable, target inaccessible, no valid flee tile.

**Replanning**
The active goal remains valid, but the current path/action sequence should be recomputed.

**Commitment / hysteresis**
The agent should not switch goals every tick just because scores wobble. You usually want rules like “new goal must beat current by 20%” or “don’t reconsider low-risk goals for 5 turns.”

The one place I’d be careful: a literal priority queue of goals is close, but maybe too rigid. A priority queue says “things already exist in the queue.” Utility AI says “every turn, context can make goals appear, disappear, or change score.” For this game, I’d think in terms of **candidate goals generated by evaluators**, then an active goal with some memory.

For your examples:

```text
Porter:
  delivery goal, rest goal, flee goal, maybe seek shelter

Harmless wildlife:
  graze, wander, flee, rest, stay near herd

Harmful wildlife:
  hunt, stalk, flee if injured, defend territory

Monster:
  pursue sensed actor, investigate noise, return to lair, idle

Town NPC:
  idle at workplace, wander home, converse, flee, perform job
```

All of those fit the same skeleton.

My strong recommendation: don’t jump straight to full GOAP. The current codebase would benefit from a smaller stepping stone:

1. Replace `AssignedJob` with a generic `ActiveGoal`.
2. Keep delivery as the first implemented goal.
3. Add a simple goal arbitration system.
4. Add one interrupting goal, probably `Rest` or `Flee`.
5. Only then decide whether execution wants GOAP, HTN, or plain per-goal state machines.

That gives you prior-art dignity without building a cathedral before there are enough weird NPCs to justify it. The architecture name I’d put on the roadmap is something like:

> Utility-selected goals with per-goal task execution, evolving toward HTN/GOAP if behavior complexity demands it.

That’s a very solid, serious-game-codebase direction.