# 2009scape in Rust

The goal is a complete Rust replacement of the revision 530 server, visual
client, and headless client, preserving the pinned game's observable behavior.
This workspace is under active migration. Controlled native visual and walking
sessions now run against the pinned cache; the complete production world and
full gameplay/content parity remain in progress.

The existing project in the parent folder is independent.

To play the native live world locally with the visual client in one command
(`pwsh tools/play.ps1`), see [docs/play-locally.md](docs/play-locally.md).

## Behavioral reference

[upstream.lock.json](upstream.lock.json) pins both repositories:

| Component | Commit |
| --- | --- |
| 2009scape server | 9921b98d49308edd0489dd50ee98011d53914a57 |
| RT4 client | 61b952d2b46132b9a5d696f4e0c4c0df75c7f6d2 |

The inventory includes 3,589 server and 453 client Java/Kotlin files, including
supporting modules and tests. This is a source inventory, not a completion
percentage. Partial function ports never count as complete file replacements.

## Implemented foundation

| Crate | Current scope |
| --- | --- |
| rs09-protocol | Byte/bit formats, ISAAC, Huffman, XTEA, CRCs, cache/lobby framing, signed RSA and revision-530 login packets |
| rs09-game-core | Java integer/RNG behavior, String HashMap iteration, server direction codes and clipping probes, RT4 collision flags and line checks |
| rs09-cache | Disk/resident sector reads, reference tables, XTEA/striped file extraction, bounded shared group cache |
| rs09-net | Shared native TCP/binary WebSocket transport, cache peer and login peer |
| rs09-server | Bounded transport, account/storage systems, native scene and player-walk assembly; complete production world/content pending |
| rs09-headless | Shared cache/login transport, live player-state observations and absolute walk/run requests; remaining gameplay actions pending |
| rs09-client | Visual client workbench, developed separately from the shared foundation |
| xtask | Pinned reference bootstrap, JVM oracle, source inventory, isolated benchmarks |

The visual client work is prioritizing Windows desktop, actual 3D assets,
animation, and audio, with a portable browser path. A rendering workbench is
not a completed game client; the client work documents its own current limits.

[Native player walking](docs/player-walking.md) describes the separate TCP43600 /
WebSocket43601 movement probe, Windows/browser validation and headless commands.
Its required content services are explicitly empty; complete original world and
player initialization remain separate. The latest gates and remaining work are
recorded in [parity/verification.md](parity/verification.md).

[Scenery interactions](docs/scenery-actions.md) covers the original object-click
dispatcher and generic door/furniture/trapdoor decisions. Their 1,229 JVM
scenarios preserve callback order and retained handles; live approach, arrival
and scene delivery are the next composition work.

[World update lifecycle](docs/world-update.md) covers the original packet buffers,
deferred entity membership and update phases now used by the movement probe.
Their JVM corpus includes 257 scenarios and joined tick replays; complete Player,
NPC/content, repository/world transitions and full packet dispatch remain open.

The shared foundation has **6,827 primitive, 129 cache, 1,311 network/lobby and
2,049 login, 368 account/password/factory/admission, 139 SQL/collection, 742 server-configuration and 285 player-metadata Java/Kotlin oracle cases**. These include
multiple ISAAC blocks, signed/overflow boundaries, mixed Huffman code lengths,
unaligned bit reads, RNG rejection sampling, multi-block compressed data,
wall rotations, clipping probe order, 5,000 collision line queries, sector
chains, sparse IDs/names, and client fallback behavior. A complete cache audit
also compared all 411,863 extracted files in 29 archives with the original RT4
classes; see [docs/cache.md](docs/cache.md).

The original RT4 Java cache queue has received verified bytes from the Rust
server over a real TCP connection. Native probes also exercise 500 concurrent
TCP and 500 WebSocket connections. These are cache connections, not active game
players. Run instructions and exact limits are in [docs/server.md](docs/server.md).
Native login codecs also interoperate with the original writer/parser over real
TCP in both directions. This proves packet-level behavior, not authentication or
world entry. See [docs/login.md](docs/login.md).
The native account library covers development/production rules, original record
and storage behavior, and BCrypt compatibility. It remains separate from the
unimplemented world-entry path; configured endpoints now run its account checks.
See [docs/accounts.md](docs/accounts.md).
Its SQL provider has been compared with the original provider against a real,
isolated database over plaintext and TLS; see [docs/sql-storage.md](docs/sql-storage.md).
The native configuration library preserves ordered defaults, paths, packed
locations and world settings; [docs/configuration.md](docs/configuration.md)
describes its coverage and the startup fields now consumed by `--config`.
Native player/communication metadata now runs after configured account checks;
[docs/player-metadata.md](docs/player-metadata.md) describes the original parse
and save behavior covered, and the remaining profile/world boundary.
Activity, quest and global character records and their JSON profile sections have
14,670 separate saved-data/numeric reference cases; see
[docs/saved-data.md](docs/saved-data.md) for coverage and pending integration.
The native attribute store and JSON/XML persistence paths add 503 original JVM
sequences; [docs/attributes.md](docs/attributes.md) records their scope and limits.
Skills/XP rules and profile sections have 1,565 reference cases and a measured
cached-lookup improvement; [docs/skills.md](docs/skills.md) gives exact coverage
and the benchmark's limited scope.
The server item-cache decoder and note/lend/recolour templates have 1,831
reference cases. All 14,659 definitions in the pinned item archive match the
original fields and mutable aliases before content configuration; see
[docs/item-definitions.md](docs/item-definitions.md). Native item/container state
and event behavior now have 389 original reference scenarios; see
[docs/inventory.md](docs/inventory.md). Live inventory, bank and equipment
integration remains pending.
The native item-configuration loader adds 1,087 reference cases and matches all
12,006 rows in the pinned configuration file, including typed properties and
constructor values; see [docs/item-configuration.md](docs/item-configuration.md).
Core character inventory/banks/equipment, tabs, location, varps and persistent
timers now have 302 integrated profile cases plus 242 timer scenarios. These
native libraries preserve callback and failure ordering; full profiles and
world entry remain pending. See [docs/core-profiles.md](docs/core-profiles.md).
Timer registration, factories, automatic installation and spawn/get-or-start
wrappers add 226 original scenarios; [docs/timer-registry.md](docs/timer-registry.md)
records the native content/world boundaries. Registry ID matching uses native
JDK 11 ROOT casing, validated with exhaustive code-point data and 20,105 string
scenarios; see [docs/string-case.md](docs/string-case.md).
Base pulses, entity pulse slots and the world task queue add 262 original
scenarios plus a parallel-producer/serial-replay check. These preserve action
timing and interruption behavior; actual content and the world tick driver
remain pending. See [docs/pulses.md](docs/pulses.md).
Entity interaction and queued-script processing now add 295 original scenarios,
covering movement phases, executor callbacks, priorities, timing, API helpers
and partial failures. Concrete content and world wiring remain pending; see
[docs/scripts.md](docs/scripts.md).

Shared revision-530 gameplay framing, 69 numeric action decoders and typed
scene/interface/inventory/script/variable/stat updates have a 4,313-case original JVM corpus.
Live world entry and authoritative action handling remain pending; see
[docs/game-packets.md](docs/game-packets.md).
Native door decisions, geometry, autowalk pulse bodies and all 514 original
configuration rows now have 463 JVM scenarios; see [docs/doors.md](docs/doors.md).
SceneryBuilder replacement/restoration, server ChunkFlags, chunk object storage
and landscape decoding/placement add 240, 1,244, 269 and 179 JVM scenarios.
The native scene joins these libraries with region loading, door rules, collision,
packets and timed restoration. Scenery constructors, Region.load ordering and
joined scene behavior have 176, 206 and 37 additional JVM scenarios; see
[docs/scene.md](docs/scene.md). Live world delivery remains pending.
The native scenery definition decoder/registry matches all 42,005 definitions
in the pinned server archive, with 1,940 portable JVM scenarios. Terrain parsing
and floor clipping add 343 scenarios. See
[docs/scenery-definitions.md](docs/scenery-definitions.md) for exact coverage;
these data libraries do not establish live world entry.

Native world-session transport now supports explicit ready-world admission and
incremental game frames. Player rendering/rebuild and appearance encoding have
207 and 127 original JVM scenarios. A controlled stationary-player endpoint has
been consumed by both visual targets and headless TCP/WebSocket observers; full
profile/startup, movement, NPC/content and authoritative actions remain pending.
See [docs/world-sessions.md](docs/world-sessions.md) for its exact scope and commands.

See [parity/README.md](parity/README.md) for exact coverage and the remaining
work. No full server capacity measurement exists yet.

## Build and check

Install Rust 1.86 or newer, Git, and a JDK providing java and javac on PATH.
The native BZip2 dependency also needs the platform's C build tools; on Windows
this is Visual Studio Build Tools with the C++ workload.

Committed fixtures allow ordinary regression tests without Java or reference
downloads:

    cargo test --workspace
    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings

To recreate expected outputs from the original, unmodified JVM classes:

    cargo xtask bootstrap
    cargo xtask oracle --check
    cargo xtask cache-oracle --check
    cargo xtask network-oracle --check

Login-method reproduction additionally uses Python 3.10+ and JDK 11 with the
original Kotlin 1.8.20 compiler:

    cargo xtask login-oracle --check --java-home PATH_TO_JDK_11
    cargo xtask login-interop --java-home PATH_TO_JDK_11
    cargo xtask auth-oracle --check --java-home PATH_TO_JDK_11
    cargo xtask auth-interop --java-home PATH_TO_JDK_11
    cargo xtask config-oracle --check --java-home PATH_TO_JDK_11
    cargo xtask player-oracle --check --java-home PATH_TO_JDK_11
    cargo xtask saved-data-oracle --check --java-home PATH_TO_JDK_11
    cargo xtask attributes-oracle --check --java-home PATH_TO_JDK_11
    cargo xtask skills-oracle --check --java-home PATH_TO_JDK_11
    cargo xtask item-oracle --check --java-home PATH_TO_JDK_11
    cargo xtask inventory-oracle --check --java-home PATH_TO_JDK_11
    cargo xtask item-config-oracle --check --java-home PATH_TO_JDK_11

On Windows, SQL reproduction also requires the OpenSSL command. The harness
downloads a pinned portable MariaDB archive and starts temporary loopback-only
processes; it does not install a service or use an existing database:

    cargo xtask sql-oracle --check --java-home PATH_TO_JDK_11

Bootstrap uses shallow, sparse, pinned checkouts and skips LFS asset downloads.
The oracle checks both Git revisions, refuses modified tracked reference
files, and verifies source hashes and expected output. Adapters are explicitly
isolated under oracle/adapters; they do not stand in for a complete Java game.

To intentionally regenerate fixtures after reviewing a reference/harness change:

    cargo xtask oracle
    cargo xtask cache-oracle
    cargo xtask network-oracle
    cargo xtask login-oracle --java-home PATH_TO_JDK_11
    cargo test --workspace

Review fixture and provenance changes together. Never regenerate expectations
just to make a failing native implementation pass.

    cargo xtask inventory
    cargo run --release -p xtask -- bench

To acquire all pinned cache objects and compare complete extraction with Java:

    cargo xtask cache-fetch
    cargo run --release -p xtask -- cache-audit --jvm
    cargo run --release -p xtask -- item-audit --java-home PATH_TO_JDK_11
    cargo run --release -p xtask -- item-config-audit --java-home PATH_TO_JDK_11

Acquisition uses Git metadata and HTTPS curl, with pinned LFS size/hash checks.
The runtime cache library needs neither Java nor Python.

Benchmarks compare warmed JVM and release Rust primitive workloads with matching
checksums. [benchmarks/README.md](benchmarks/README.md) explains the first baseline
and its limits.

## Migration approach

Port actual behaviors against the reference before optimizing their execution.
Keep gameplay, graphics, networking, persistence, and agent/model inference
separate. Both human and headless clients should use the same game-facing client
logic, with structured agent observations restricted to what that character
could know.

Parallel execution must preserve the reference's observable ordering and state
transitions. Serial/parallel replay comparisons are required before enabling
parallel game systems. Rust threads alone do not establish gameplay parity or
an unlimited player count.

The native walking queue now matches 284 original JVM scenarios, including run
energy, teleports, scene updates, zone vetoes and reentrant callbacks.
[docs/walking.md](docs/walking.md) records its scope and the pathfinding/world
integration still needed before enabling live movement.

The server's SmartPathfinder and reach rules now match 5,287 JVM cases, with
native scene collision and door-restoration route checks. See
[docs/pathfinding.md](docs/pathfinding.md) for remaining movement/world work.

Native NPC kernels now cover wandering/respawn/return scheduling, original spawn
loading and update encoding. DUMB routing is integrated alongside SMART; a joined
regression carries NPC movement through collision, queues, packet encoding and
the shared graphics-free reader. [docs/npcs.md](docs/npcs.md) records original
comparisons, the pinned run-bit discrepancy, and remaining live startup/content
work. These kernels do not establish a complete NPC/world replacement.

The base NPC factory/init/configuration orchestration adds 261 JVM scenarios.
`npc_session` now initializes 52 pinned Lumbridge-area spawns with real cache
collision and streams authoritative walking NPCs over TCP/WebSocket. The
headless client records bounded movement deltas and can be compared per update
against optional canonical queue traces. This controlled session still uses
empty combat/content/timer services and preactivated regions; complete world
startup and complete repository services remain required. Commands and
scope are in [docs/npcs.md](docs/npcs.md).

Native scene chunks now hold shared player/NPC memberships. The original
ordered local queries, movement callbacks and aggression tolerance timing match
227 JVM scenarios; both live render paths use those queries. Canonical leases
preserve duplicate/retained list entries and query-result lifetimes. See
[docs/entity-regions.md](docs/entity-regions.md) for scope and pending activity,
zone and dynamic-region services.

Native interface metadata and gameframe orchestration add 238 original JVM
scenarios and the pinned 162 configuration rows. Controlled movement/NPC
sessions can enable `--gameframe` for original root/chat/tab ordering, real
empty inventory, window-mode switching and keepalive. See
[docs/interfaces.md](docs/interfaces.md) for commands and explicit remaining
player, plugin, tutorial and full login-content work.

Player Settings operations now have 871 original JVM scenarios, preserving
partial loads, receiver capture, chat/special-attack quirks and weight rules.
The gameframe fixture constructs fresh native Skills and publishes all 24 skill
packets; headless observations expose the received boosted levels and XP.
[docs/settings.md](docs/settings.md) records the remaining Settings runtime,
profile and XP/content bindings.

The server NPC cache decoder/configuration parser also matches original fields
and aliases across all 8,591 lookup slots (8,590 files) and 8,033 configuration
rows. [docs/npc-definitions.md](docs/npc-definitions.md) records defaults, partial
loads, typed metadata and remaining class/startup work.

Movement-pulse orchestration and destination selection add 626 original JVM
scenarios. [docs/movement.md](docs/movement.md) records the native behavior and
the entity/queue/path/action services still needed for live movement.

The acceptance criteria and implementation sequence are recorded in
[docs/architecture.md](docs/architecture.md). Completion requires native game
and content coverage, integration tests, visual/audio comparisons, save/reload
compatibility, and measured world workloads.

## Source and assets

This derived source workspace uses AGPL-3.0-only; see [LICENSE](LICENSE) and
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Cache assets have separate
provenance and are not included in this source foundation. Reference checkouts,
downloaded assets, compiled output, and run logs are ignored by Git.
