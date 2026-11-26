# Force movement

`world::force_movement` ports core/game/node/entity/impl/ForceMovement: the
pulse that moves an entity from a start tile to a destination with the
ForceMove update mask, used by agility obstacles, doors and shortcuts. It is
a library: nothing in a session or the live world installs it yet (the
live world's eighth install round stopped before it: nothing installed
there calls a force movement, and the ForceMove mask's encoder is not
ported).

## What is ported

| Original | Native |
| --- | --- |
| The five constructors (`super(1, e)`) | `ForceMovement::new`, `with_speed`, `with_speeds`, `with_animation`, `detached` |
| The ten static `run` forms | `run_static` with `RunForm`, `run` (the full form) |
| The instance `run(e, speed)`, `run(e)`, `run()` | `run_with`, `run_entity`, `run_self` |
| `start`, `pulse`, `stop`, `direction(s, d)` | `start`, `pulse`, `stop`, `direction` |
| `Pulse.update` with these overrides | `update` (`pulses::update_base` with a driver that dispatches `pulse`/`start`/`stop`) |
| The getters and setters, `WALKING_SPEED`, `RUNNING_SPEED`, `WALK_ANIMATION` | public fields, constants, `Statics::walk_animation` |
| `Direction.getDirection(dx, dy)`, `Animator.Priority`, `ForceMoveCtx` | `Direction::get`, `Priority`, `ForceMoveCtx` |

The pulse base (running, delay, ticks passed, the `[e]` node checks) is the
native `pulses::Pulse`. Locations are the packed `Location` values
(`prayer::location`: fifteen-bit signed x/y, z modulo 4) and distances
`Location.getDistance` (`prayer::distance`).

## Behaviour that is easy to get wrong

- `run` raises both animations' priority to `VERY_HIGH` on the objects
  themselves: `WALK_ANIMATION` is one static object, so after the first run
  every later movement (constructed or run) uses the walk animation at
  `VERY_HIGH`. The world owns that object (`Statics`).
- `start` turns the speeds into ticks: `ceil(distance(start, entity) /
  (commenceSpeed * 0.1))` and `ceil(distance(start, dest) / (pathSpeed *
  0.1))` (a zero speed gives `Integer.MAX_VALUE` or, for zero distance, 0);
  it plays the start animation and waits the commence ticks, or plays the
  path animation and waits the path ticks; disables the impact handler for
  `1 + commence + path` ticks (int wrapping); registers the mask with both
  tick counts times 30; and for a player updates the region. The location is
  read before the start is checked (the argument of `start.getDistance` is
  evaluated first); a missing direction fails in `ForceMoveCtx`'s null check
  after the impact handler call.
- The instance `run(e, speed)` first computes its own commence ticks
  (`1 + distance / (commenceSpeed * 0.1)` truncated) and replaces the path
  speed with `speed` unless it was 0; `start` then divides again by those
  values.
- `pulse()` after a commencing walk plays the path animation, sets the delay
  to the path ticks, clears the commence ticks, teleports to the start and
  continues; the next run finishes. `stop()` stops the pulse, teleports to
  the destination, plays the end animation and unlocks only with
  `unlockAfter`.
- `run(e, start, destination)` takes the direction from the entity's
  location, not from `start`; `run(e, destination)` and
  `run(e, destination, commence, path)` read the entity's location twice.
- `direction` takes the larger absolute axis of the packed delta (x only when
  strictly larger), so deltas beyond ±16,383 wrap.
- `Pulse.update` checks the nodes (an inactive entity stops the movement),
  then the running flag, then counts ticks against the delay; a failing body
  is printed and stops the pulse, and a failing `stop` propagates.

## Required services

Every outside call is a method of `force_movement::Host` with no default:
the entity's location, `animate`, `setDisabledTicks`, the ForceMove mask
registration, `WalkingQueue.updateRegion`, `setTeleportLocation`, `lock`,
`unlock`, `GameWorld.Pulser.submit`, the pulse node check and the printed
stack trace. `is_player` is a plain type test.

## Joins

- **Update masks.** `register_force_move` is `Masks::register(ctx,
  Flag::ForceMove, Some(context), false)` on the entity's native
  `world::update_masks` (presence 0x400, ordinal 5 for players). The test
  `start_registers_the_native_force_move_mask_and_the_pulse_lands_on_the_destination`
  runs a movement against real masks and the native pulse base.
- **Pulses.** A world's `pulses::Context` dispatches a ForceMovement pulse
  (class name `core.game.node.entity.impl.ForceMovement`) to `start`,
  `pulse` and `stop` with the movement owning that pulse handle; `submit` is
  the world's `pulses::Runner::submit`. `update` here drives one movement
  directly with `pulses::update_base`.
- **Locks, teleports, regions.** `lock`/`unlock` are the actor's action
  locks (world/action_locks), `set_teleport_location` the actor's properties,
  `update_region` the walking queue's region update.

## Reference tests

```powershell
cargo test -p rs09-server --test force_movement
cargo xtask force-movement-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK_11
```

`fixtures/force-movement/` holds **846 original JVM cases (548 designed and
298 single injected failures) with 2,893 operations and 11,730 recorded
calls**, and the hashes of every source. The oracle compiles
`ForceMovement.java`, `Pulse.java`, `Direction.java`, `Point.java`,
`ClipMaskSupplier.java`, `Animation.java`, `ForceMoveCtx.kt` and
`EntityFlag.kt` unchanged with Kotlin 1.8.20 on JDK 11 and JSON-simple
1.1.1; Location's `LocationHash` packing, constructors, properties, `create`,
`getDistance` and `getDelta` and `Animator.Priority` are original text
inserted into the stand-ins of `oracle/force-movement/adapters.py`. Each case
starts with `WALK_ANIMATION` at its initialiser's priority.

The corpus covers every static run form and constructor over players and
NPCs, starts on and off the entity's tile, packed coordinates beyond the
fifteen-bit range and planes outside 0..3, the four instance runs, speeds of
zero, negative, large and `Integer.MAX_VALUE`, full update cycles until the
pulse stops, inactive nodes, end animations, `unlockAfter`, direct pulse and
stop calls, every setter and getter, shared and null animations, null
entities, locations and directions, 656 direction pairs (256 of them in-range endpoints whose difference wraps) and 40 random
sequences. The native test replays every row and compares each operation's
result, error, recorded calls and printed stack traces, the animations'
priorities and every movement's fields whole; `GAPS` is empty. 12 of 12 deliberate breaks of the port (the direction axis test and packing, the arrive multiplier, the instance path-speed swap, unlockAfter, the location-before-start order, run(e, start, dest)'s direction source, pulse's continue, the VERY_HIGH priority, ceil, the player test) fail the replay; the delta-packing break survived the first corpus and the wrapping direction pairs were added for it.

## Not ported

ContentAPI `forceMove` (the script-based replacement the class's
deprecation points to), `Direction.canMove`/`canMoveFrom`, the ForceMove
mask's encoder, and every binding to a live session.
