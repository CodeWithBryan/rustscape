# Movement pulses and destinations

`crates/server/src/world/movement/` ports the original MovementPulse orchestration,
DestinationFlag geometry, Node center calculations used by that flow, and
ContentAPI.truncateLoc. The owning world still supplies entities, queue/pathfinder
services, script/content execution, packet delivery and scheduling. This module
does not yet enable walking in the running transport fixture.

The native pulse retains original constructor replacement, NPC finder overrides,
captured base checks, current destination, early region gating, weak-then-normal
script removal, facing, immediate interactions, near-path messages and minimap
clearing. It preserves combat/moving-target checks, predictive intersections,
forced running, path consumption, hint callbacks, repeated stop calls and caught
versus propagated failures. Base checks keep their original nodes when the live
destination changes.

Location handles retain identity. Pulse fields and route/deque handles remain
shared across reentrant callbacks, and no lock is held across those callbacks.
For example, clearing the target during a collision callback does not erase the
path and interaction state that the original publishes before a later null
failure. Truncation at exactly the limit still counts as truncated. Vector
transforms retain the original plane-zero and zero-length behavior.

Destination selection covers location, entity, following, combat, ground-item
and scenery flags. Door destinations remain an explicit service, ready to bind
to the separately ported door rules. Object rotation/size/walking flags, nearest
border order, Euclidean distance overflow, directional checks and late mutable
location reads follow the pinned source.

The native Context uses stable node identifiers. Its owner must retain each
identifier's original node while any pulse or captured check references it;
recycling an identifier into a different entity would violate the contract.
An entity's current movement also outlives its scheduling slot in the original:
stopping the pulse does not clear that field. The world must keep that stopped
movement available until the entity replaces or releases its current handle.
Native geometry loops are bounded at 65,536 iterations for malformed sizes or
nonterminating mutation. Ordinary cache geometry fits inside those bounds.

`oracle/movement/generate.py` compiles the complete unchanged MovementPulse,
DestinationFlag, Pulse, Point, Direction, Path and Vector classes. It also uses
verbatim truncateLoc, Node center and reach methods. Explicit adapters provide
mutable nodes/regions, behavior/combat/scripts, queue operations, returned paths,
option/use/override handlers, collision/teleport/door services, hints/packets and
normalized exception output. The queue/pathfinder adapters are deliberately
separate from the existing WalkingQueue/SmartPathfinder corpora.

All 626 scenarios compare state after each command: pulse fields, node state,
ordered callbacks, return values/errors, path/queue consumption, retained deque
views and Location aliases. Cases include constructor replacement, active/stopped
regions, immediate interactions, moving/combat targets, truncation boundaries,
rotation shifts, near outcomes, null results, reentrant replacements and failure
prefixes. Source and fixture hashes are checked in Rust tests and CI.

```powershell
cargo test -p rs09-server --test movement
cargo xtask movement-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

The [navigation composition](navigation.md) now joins MovementPulse to native
WalkingQueue, SmartPathfinder and Node destination dispatch in 133 additional
JVM scenarios. Scene/door collision feeds the same Navigator in a native
regression. The [player-walking assembly](player-walking.md) adds original walk
handling, scheduling and phase kernels with a separately labeled live probe.
Complete entity scripts, UpdateSequence, remaining finders, Path.walk and full
player/entity startup still remain. Complete content interactions and gameplay
capacity are not established by these corpora. Full
game/content, visual/audio, headless, persistence, parallel replay and performance
acceptance gates stay open.
