# Native server pathfinding

`crates/server/src/world/pathfinding/` ports the pinned SmartPathfinder search
and five Pathfinder reach predicates: door, decoration, rectangle, sized
rectangle, and standing/overlap. The native scene context supplies real loaded
server collision flags. This is separate from RT4 client collision and routing.

The port preserves original A*/BFS selection, direction expansion order, diagonal
costs, first-visit rules, 4,096-entry ring buffers and Java PriorityQueue heap
ties. The A* heuristic uses the destination's own scene coordinates exactly as
the original does. Supplier exceptions abort A* and sized search, while single
BFS catches them per iteration. Near fallback, distance truncation, integer
wrapping, mutable Location reads, backtracking failures and duplicate endpoint
points retain original behavior.

The reach methods preserve ordered clipping reads and short circuits. Sized
rectangle reach uses RegionManager directly, even when its caller has another
ClipMaskSupplier. Decoration parameter names and SmartPathfinder's argument order
also differ in the original; the port retains that ordering. The offline authoring
tool `tools/port_reach.py` translates only these five selected methods to Rust
using wrapping i32 arithmetic. It is never invoked by the game runtime.

`oracle/pathfinding/generate.py` compiles unchanged SmartPathfinder, Direction,
Point and Vector source, plus verbatim original reach predicates. Explicit
adapters supply packed mutable locations, sparse supplier/global clipping,
settings and logging. The Path.walk entity adapter is outside this corpus's
scope. All 5,287 cases compare return values, point lists, errors, ordered
collision queries and complete search-array/cursor state. SHA-256 fingerprints
cover every query and every via/cost/ring-array element; prefixes and suffixes
remain readable in the fixtures. Cases include both search settings, movers of
different sizes, obstacles, unreachable targets, truncation, mutated locations,
supplier failures, overflow boundaries and seeded layouts.

```powershell
cargo test -p rs09-server --test pathfinding --test scene
cargo xtask pathfinding-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
python tools/port_reach.py --check
```

Scene integration adds a regression that routes around a closed native door,
finds the direct route after opening, then recovers the exact original route
after the 500-tick restore. It composes the separately checked scene and search
kernels; it is not an end-to-end JVM world or live movement comparison.

Search buffers are reused across calls and sized-edge probes use stack buffers.
Performance and gameplay capacity have not yet been measured for this workload.
Malformed sized-reach loops and enormous mover-edge loops are bounded to 65,536
iterations. SmartPathfinder's original outer exception recovery still applies.

MovementPulse and destination selection have a separate 626-case JVM port;
see [movement.md](movement.md). Node dispatch now has a 1,954-case original JVM
corpus, and 133 joined scenarios bind real native movement pulses, smart searches
and walking queues; see [navigation.md](navigation.md). Remaining movement work
includes the Dumb/Projectile finders, Path.walk, complete owning-world services,
entity/zone/content bindings and UpdateSequence. Original walk handling and a
separate real-cache live movement probe are described in
[player-walking.md](player-walking.md). The ordinary transport endpoint remains
stationary. Full game, client,
headless, persistence, parallel replay and performance acceptance gates remain
open.
