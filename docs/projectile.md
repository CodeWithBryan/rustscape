# Projectile pathfinding and script line of sight

The controlled Rust server runs the original projectile traversal when an
interaction script reaches its target range. It reads both collision layers
from the owning scene and uses the actor's current location and footprint.
The destination is the script's selected `Location`, a size-one target in
the original game.

Each diagonal checks the horizontal edge, vertical edge and destination in
order. The original stops at the first blocked diagonal even though its
direction helper lists cardinal alternatives. It reads projectile flags first, then the `0x20000`
walking flag when the projectile flags did not already block the step.
It uses the source plane, even when the target has a different plane.
Combat's footprint helper tries tile pairs in the original x-then-y loop
order. Its reach check counts steps without allocating a list of points.
The path-returning variant retains point order and failed-path progress.

`cargo xtask projectile-oracle --check` uses JDK 11 to reproduce 403 cases
against the unchanged `ProjectilePathfinder` and verbatim combat and content
API entry points. It compares results, ordered flag queries, failure prefixes,
footprint traversal, and mutable packed locations. The Rust example test
drives an actual interaction callback through the pre-movement script phase
and shows that each collision layer can independently keep it from running.

The host selects flags from the existing Rust region and scene code. Sparse
flags in the JVM corpus are explicit oracle inputs; no combat, spell damage,
projectile rendering, full-content wire flow, or performance benchmark is
claimed here. Rust traversal state is per call rather than shared through a
singleton. A destination continually mutated by a flag provider is bounded
at 65536 iterations; the bound cannot truncate a fixed packed-coordinate path.
