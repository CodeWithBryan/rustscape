# Native walking queue

`crates/server/src/world/walking.rs` ports the pinned server's complete
`WalkingQueue` method bodies and `Point` data behavior. It uses the existing
server Direction port and shared packed Location handles. This is the movement
queue kernel; collision-aware path selection, `MovementPulse`, packet action
dispatch and the original world phases remain to be integrated.

The queue preserves its initially empty constructor, explicit reset origin,
origin consumption, interpolated and directly supplied point deltas, and mutable
run-disabled flags on retained points. Direction ordinals used in player updates
differ from Direction's public integer values. Zero-delta `addPoint` still creates
a north-facing point, as in the original.

Updates retain original ordering for teleport resets, scene boundary checks,
frozen timers, run depletion/toggle publication, one/two-step consumption, energy
updates and zone vetoes. A rejected second step still drains running energy and
can retain the first step. Footprints and scene locations retain shared identity;
callbacks may mutate the queue or locations and fail after partial changes.
The route-debug ground-item list follows the original captured ArrayList iterator
even when a callback replaces the public list.

Energy uses the original weight, agility, hamstrung and marathon formulas,
including wrapping agility subtraction and the perk's fourth-drain skip.
Floating-point fixtures include depletion thresholds, saturation, infinity and
NaN. Queue code publishes energy/toggle state before invoking packet callbacks.
The `Entity` record supplies the fields this class reads; it does not replace the
full original Entity, Player or Settings classes.

`oracle/walking/generate.py` compiles unchanged pinned `WalkingQueue.java`,
`Point.java`, `Direction.java` and `ClipMaskSupplier.java`, with original packed
LocationHash/transform source. Explicit adapters supply timers/perks, zones,
region moves, pulse clearing, ground-item construction and packets. Settings
energy/toggle mutations in those adapters follow the original methods.
The 284 JVM scenarios compare each command's complete queue/state, energy bits,
retained point/location aliases, callbacks, return values and failure prefixes.
Generated fixture and source hashes are checked in Rust tests and CI.

```powershell
cargo test -p rs09-server --test walking
cargo xtask walking-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

The native port rejects an `addPath` expansion larger than 65,536 points before
entering its interpolation loop. This bounds malformed caller inputs; original
pathfinder-produced paths fit within a 104-square build area. Debug-item creation
still precedes this guard, following the original prefix. A separate regression
checks that rejecting a huge expansion preserves the existing queue.

The live `player_session` endpoint remains a stationary transport fixture.
These comparisons do not establish live movement, wall/door collision, production
world startup, parallel simulation, or player capacity. The next integration is
the original SmartPathfinder and interaction reach rules, MovementPulse and the
authoritative movement phase, followed by live scene/door updates.
