# Native navigation composition

`world/navigation.rs` binds the ported MovementPulse to real native destination
dispatch, SmartPathfinder, DumbPathfinder and WalkingQueue operations. It reuses the smart search
workspace; it does not return an authored route or expand a path in a test-only
queue. The same queue handle supplies movement's location, direction, points,
running, moving and has-path fields. Callbacks can observe and change canonical
queue state without a queue lock being held across the call.

`pathfinding/dispatch.rs` ports every original Node/entity/location `find`
overload. It preserves rectangular scenery sizes and flag rotation, wall and
decoration types, entity sizes, ground-item teleport permission, NPC behavior
suppliers, world fallback, water selection and explicit nullable suppliers.
Ground-item callbacks run before the final destination-location read. Signed
rotations retain Java shift masking and wrapping. The dispatch corpus compiles
the original overload bodies verbatim and compares 1,954 cases, including callback
mutations/failures and nulls.

The joined navigation oracle compiles unchanged MovementPulse, DestinationFlag,
Pulse, SmartPathfinder, DumbPathfinder, WalkingQueue, Path, Point, Direction and Vector, plus the
original dispatch/reach/truncation methods. Its 167 scenarios replay individual
movement-pulse updates and walking updates in an explicit serial command order.
They compare state after each command, including callback order, queue points,
directions, energy bits, Location aliases, pulse fields, arrival, near outcomes,
moving targets, combat restrictions, truncation, frozen/hamstrung timers, zone
vetoes and reentrant node/pulse replacement. The native side uses the production
Navigator and Queue bindings. It does not use the isolated movement oracle's
returned-path or fake-queue adapters.

Node/entity metadata, script/content callbacks, region activity, zones, timers,
settings/packet services and exception observations remain explicit. The command
order is a replay input, not proof of original complete world scheduling. The
oracle does not call Path.walk; MovementPulse consumes Path points itself in the
original. A scene regression supplies real native collision through Navigator
and verifies the closed-door detour, direct route after opening, and exact route
after 500-tick restoration. It also checks private Location-node dispatch with a
mover whose ID is usize::MAX.

```powershell
cargo test -p rs09-server --test navigation --test path_dispatch --test walking --test movement --test pathfinding --test scene
cargo xtask path-dispatch-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
cargo xtask navigation-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

The Navigator executes SMART and DUMB routes with their distinct original
algorithms. Other finder IDs return an explicit pending-port error. The NPC
wandering/return assembly and 638-case isolated DUMB comparison are described
in [npcs.md](npcs.md).
The host owns stable node IDs and movement handles, actual collision suppliers,
and one exclusively borrowed search workspace per executing navigation lane.
Arbitrary recursive entry into the same search workspace is not established by
this corpus. The [player-walking assembly](player-walking.md) now adds original
walk requests, manager/runner and phase kernels, plus a separate live movement
probe and headless actions. Complete Entity/script/zone lifecycles, UpdateSequence,
production profile/world entry, Path.walk, remaining finders and content are still
required. The ordinary and animation/audio fixtures remain stationary. No
performance or gameplay-capacity improvement is claimed from these correctness
tests.
