# Native NPC migration

The Rust NPC work covers the original tick/wandering orchestration, DUMB
pathfinding, spawn-loader publication and NPC update encoding. It connects
those kernels to the existing native entity queues and pulse runner. Complete
definitions, NPC subclasses, behavior plugins, combat, region activation and
production startup remain required. Native server cache/configuration metadata
is now covered by [npc-definitions.md](npc-definitions.md); applying that metadata
through complete content services is still required. The base factory/init/
initConfig orchestration now has a native kernel and a controlled live binding.

## Original comparisons

| Behavior | Production Rust | Original JVM evidence |
| --- | --- | --- |
| Tick, respawn timing, automatic walking, return scheduling/arrival, dialogue and forced-talk scheduling | world/npc.rs | 357 scenarios, verbatim selected NPC.java bodies, packed Location/Vector and seeded RandomFunction methods |
| DUMB routing, sized traversal, singleton fields, reach order and clipping callbacks | world/pathfinding/dumb.rs | 638 scenarios against unchanged DumbPathfinder.java; authored traversal checked by tools/port_dumb.py |
| Spawn parser, creation/setters/attributes/init/log order and partial failures | world/npc_spawns.rs | 77 scenarios against unchanged NPCSpawner.kt, including all 3,225 rows / 13,709 pinned spawns |
| Local membership, motion bits, additions, masks and tolerance callbacks | world/npc_render.rs | 221 scenarios against unchanged NPCRenderer.kt and original packet writer |
| Joined SMART/DUMB dispatch, movement pulses and walking queues | world/navigation.rs | 167 original navigation scenarios, including 34 added DUMB movement/reach cases |
| Graphics-free packet fields, membership, types and queue heads | protocol/game/npcs.rs | 64 original RT4 streams / 1,536 frames; the visual reducer shares the numeric reader |
| Base factory, init/config application, children and callback failure order | world/npc_init.rs | 261 scenarios using verbatim NPC.java methods, explicit entity/property/skill/repository services |

NPC.tick tests use explicit Entity, behavior, combat, locks, regions, attributes,
dialogue, pulse and path services. They compare seeded random draws, callbacks,
state and failure prefixes; they do not instantiate the complete original NPC
class. NPCSpawner's factory, definition and init services are likewise explicit.
The full spawn file is portable pinned test/configuration data, not evidence
that all NPC subclasses or their content have been initialized in Rust.

`world/npc_walk.rs` connects the NPC kernel to the same canonical WalkingQueue,
navigation and world pulse runner as players. It uses DUMB for wandering and
SMART for the original return pulse. Movement consumes a private transient
path; mutable public Path/Deque aliases and iteration are a separate contract.
Owners supply region/content/lock/dialogue/RNG services and retain node leases.
Water pathfinding requires an explicit service and fails when it is unbound.

Five joined native regressions cover 180 seeded wandering ticks, a clipped wall,
NPC encoding followed by shared headless decoding, return-to-spawn and lock
release, return-attribute ownership, captured arrival identity, and the pinned
running-bit discrepancy. These compose differential-tested kernels; they are
not a complete-NPC JVM/world comparison. The existing movement_session endpoint
keeps its player-only mode; the separate npc_session mode adds NPC startup.

## Initialization and live binding

`npc_init` preserves factory prototype dispatch/null fallback, static skill order,
animation fallback and repeat reads, shared bonuses, forced talk/radius, partial
graphic/configuration failures, repository/region/configure/behavior order,
and children whose array references are captured across callbacks. Children
inherit the parent's late index/size and can have no location. The owning graph
retains children and externally exposed old arrays while collecting detached
cycles; native regression tests exercise those lifetime contracts.

`npc_session` uses original pinned definitions/configuration/spawn data and real
cache collision. It initializes all 52 spawns within 25 tiles of (3207,3216), on
all planes, and uses the native 600ms update sequence, wandering, movement queues
and NPC renderer alongside players. The geographic bound is explicit fixture
scope. It installs empty prototype/content/combat/timer/dialogue services,
partial skill/property storage and preactivated regions. Candidate enumeration
now uses the original ordered chunk query over native scene membership; viewer
startup, inactivity and general masks remain pending. See
[entity-regions.md](entity-regions.md). Forced-chat masks use explicit UTF-8; this
does not complete the original update-mask subsystem or charset behavior.

The playable configuration adds `--world-npcs` for all 13,709 original
placements, instantiated once per region as players approach, and
`--npc-combat` for basic unarmed attacks on ordinary NPCs. See
[Playable multiplayer alpha](playable-alpha.md) for the launcher and map
coverage. Water NPC navigation uses the original water collision supplier
through the same DUMB navigator as other wandering NPCs.

```powershell
cargo run --release -p rs09-server --example npc_session -- --keys runs/player-session/xteas.json
target/release/rs09-headless.exe --npcs --address 127.0.0.1:43602 --username observer --rounds 32
```

The example defaults to TCP43602/WebSocket43603 and development password testing.
Existing stationary, animation and player-only movement endpoints are separate.
Optional `--npc-trace NEW_FILE` records canonical queue positions for each
player's NPC update, permitting per-update comparisons with decoded headless
observations. It creates a new file and refuses to overwrite one; synchronous
trace I/O is a diagnostic fixture cost, never performance evidence.

## Shared observations

`rs09-headless --npcs` enables world observation and waits for both player and
NPC updates. It includes NPC wire slot, type, absolute tile and the most recent
numeric mask record. The visual client shares these reads, then applies its
actor, animation and cache metadata behavior. Headless observations need no
meshes, textures, graphics context or audio.
With `--npcs`, `npcTrajectory` records additions, changed type/tile/plane and
removals for at most 256 change-bearing updates, with an explicit omitted count.
The final NPC list remains complete for the observed local set; mask and actor
semantics remain subject to the limits below.

`lastMask` is the last received numeric record, not visible health/chat timers
or the full actor state. Fine-coordinate interpolation and animation remain
visual-client state. NPC planes come from the current scene; the reader's tile
is the absolute queue head. Old player-only fixtures work without `--npcs`.
Enabling it against a server that never sends opcode32 times out instead of
inventing NPC data.

Valid original streams are compared in full. Bounded malformed-packet rejection
can differ in partial mutation order: the shared reader groups a numeric record
before the visual reducer applies it.

## Pinned running-bit discrepancy

The original server's NPCRenderer kind2 branch writes two three-bit directions
without the crawl/run selector that the pinned RT4 NPC decoder reads first.
Both Rust implementations preserve their separately verified references. An
existing NPC encoded as east/east running is decoded as north then northwest
by this pinned pair. A regression records that result. Normal random wandering
walks and does not enter this branch. Keep this visible when implementing combat
and follow running; silently changing either side invalidates the original-byte
and actor-state comparisons.

## Reproduction and remaining work

```powershell
cargo test -p rs09-server --test npc_tick --test npc_spawns --test npc_render --test npc_walk --test dumb_path --test navigation --test player_walk
cargo test -p rs09-protocol --test npcs
cargo test -p rs09-client --test npc_updates
cargo xtask npc-tick-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
cargo xtask npc-spawns-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
cargo xtask npc-render-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
cargo xtask npc-init-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
cargo test -p rs09-server --example npc_session
cargo xtask dumb-path-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
python tools/port_dumb.py --check
python oracle/players/generate.py --check
```

NPC death up to the drop (`DeathTask`, kill credit, `finalizeDeath`,
`handleDrops`, the respawn tick this tick routine waits on) is a separate
library with its own corpus: see [npc-death.md](npc-death.md). It is not bound
to `npc_session` or the playable alpha.

Next: complete NPC/Entity constructors and remaining methods, original skill/
property/timer/content bindings, full repository/chunk lifecycle and region activity,
concrete masks/aggression/combat/behavior/content, inactivity/removal and full
world startup beyond this live fixture. Full profiles, content, persistence,
deterministic parallel simulation and measured performance/player capacity
remain part of the overall replacement objective.
