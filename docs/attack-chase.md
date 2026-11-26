# The combat chase

`crates/server/src/world/combat_chase.rs` is `CombatPulse`'s private
`MovementPulse`: the pulse an entity's combat uses to get in range. It is the
native `MovementPulse` (`world::movement`, docs/movement.md) in the
configuration `CombatPulse` gives it, and its functions are `CombatPulse`'s
statements:

| `CombatPulse` | `combat_chase` |
| --- | --- |
| `movement = object : MovementPulse(entity, null) { override fun pulse() = false }` | `new(ctx, id, entity)`, `pulse()` |
| `movement.setLast(null); movement.setDestination(victim)` (`setVictim`) | `set_victim(&movement, victim)` (`set_last`, `set_destination`) |
| `movement.updatePath()` (the interaction check) | `update_path(&mut movement, ctx)` |

What follows from that configuration, and what the corpus pins:

- The destination is null when the pulse is constructed, so the constructor
  sets no destination flag. The chase never asks `DestinationFlag.ENTITY` for
  a tile beside the victim: the path is found to the victim node itself
  (`Pathfinder.find(mover, victim, true, pathfinder)`), with the player's
  smart pathfinder, an NPC behaviour's override, or the dumb one.
- The constructor takes over `mover.currentMovement`: a movement the entity
  already has is stopped and its walking queue reset. In the original this
  happens when the entity's combat pulse is made (with the entity), before
  any other movement exists.
- A mover standing inside the victim walks to a border tile; a destination
  beyond the pathfinding distance is truncated; a never-walking NPC and a
  victim without a location do nothing.
- While the victim walks, fights and has the mover as its victim, the mover
  waits, unless the victim's queue is the single tile it stands on.
- A path is not found again while the mover still has one and the destination
  location equals the previous one. For the chase that location is null (no
  flag), so every call finds a path, except after the border step of a mover
  inside its victim or the interrupt recorded one.
- When the destination is an NPC that is not the mover's combat victim, the
  path leads the NPC along its walking queue (`checkForEntityPathInterrupt`).
- Each step added faces the victim; the last step becomes the interaction
  location; a player with `draw-intersect` gets the hint icon there.
- `CombatPulse` never submits the movement pulse, so its `update()` and the
  subclass's `pulse()` (false) do not run in the original.

## Corpus

`cargo xtask attack-chase-oracle --check` (Python, JDK 11) runs the movement
oracle's harness (`oracle/movement/generate.py` and `Support.java`: the
unchanged `MovementPulse`, `DestinationFlag`, `Pulse`, `Point`, `Direction`,
`Path` and `Vector` over its recording world) with this oracle's cases and
fixture folder; `fixtures/movement` is not touched. Actions that stand for a
`CombatPulse` statement carry a `chase` key. 821 cases / 3,704 actions / 7,391
recorded calls (869 constructions, 1,002 `setVictim`s, 1,623 `updatePath`s):
every mover and victim kind and size at fourteen offsets, other planes, the
mover's victim being the destination or not with walking victims, the
wait rule, every pathfinder answer, repeats, moved and replaced victims, a
combat pulse made over an existing movement, NPC movers' pathfinders, the
hint icon, border tiles under every clipping answer, the pulse updated
anyway, every service failing or changing the world once, and a random walk.

`cargo test -p rs09-server --test attack_chase` replays them over the movement
oracle's native world (`oracle/movement/native.rs`, included), running each
`chase` action through `world::combat_chase`: events, nodes, pulse fields,
paths, aliases, results and errors after every action. The same cases are
also replayed as plain movement actions.

6 of 7 deliberate breaks of `combat_chase` fail the replay; the seventh
(returning early without a victim) is the same behaviour, because
`updatePath` itself returns for a null destination.
`CombatPulse.attack`, `setVictim` and the interaction check that call these
statements are combat-pulse-oracle; the pathfinders and the walking queue are
their own corpora.

## Replacing the live world's chase

`world/live/combat/host.rs` answers `combat_pulse::Host`'s three movement
services with a simplified path (`Live::chase`). To use the library:

- Keep a `movement::Movement` per combat entry (in place of
  `entry.chase: Option<Location>`), made by
  `self.with_walk(|world| combat_chase::new(world, id, Some(node)))` when the
  entity's combat pulse is made. If the live world keeps making combat entries
  lazily, the constructor's takeover of `currentMovement` happens later than
  in the original (at the first attack instead of at construction) and must
  be recorded as a deviation.
- `movement_set_last(entity)` is `combat_chase::set_last(&movement)`;
  `movement_set_destination(entity, victim)` is
  `combat_chase::set_destination(&movement, victim.map(P::node))`.
- `movement_update_path(entity)` is
  `self.with_walk(|world| combat_chase::update_path(&mut movement, world))`
  (`player_walk::World` is the `movement::Context`), replacing `Live::chase`.

Where `Live::chase` differs from the original today: it asks
`Flag::Entity.destination` for a tile beside the victim (the original's flag
is null and the path goes to the victim node); it does not run the
inside-the-victim border step, the truncation, the wait rule, the path
interrupt, the face per step or the hint icon; it skips a path when the last
one went to the same tile and steps remain (the original finds one every
time).
