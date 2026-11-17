# World-trace oracle

Every other oracle in this repository runs a few original classes in
isolation. This one boots the whole original server, headless, and records what
it does tick by tick, so that later work can compare a native world with it.

**No native world replays these traces yet.** The fixture and its Rust reader
establish what the original did in four scripted runs. They say nothing about
the Rust server, and acceptance gates 2 and 3 in `parity/README.md` stay open.
The login ticks are compared with the native login library and every later
tick with the native tick driver, with stand-ins for what is not ported; see
[world-login.md](world-login.md) and [tick-driver.md](tick-driver.md).

## What runs

`cargo xtask world-trace-oracle [--check]` (Python 3.10+, JDK 11):

1. checks both reference pins with read-only git;
2. compiles all 3,487 files under `reference/server/Server/src/main` (1,863
   Kotlin, 1,624 Java) unchanged, with Kotlin 1.8.20 and javac, against the
   sixteen jars the original's POM needs (`oracle/world-trace/artifacts.lock.json`);
3. copies the class files and redirects 230 call sites that read the host clock
   or construct an unseeded generator, and flags one more (see "Pinned inputs");
4. builds a world directory from the pinned commit: `Server/data` straight from
   git objects, the cache from `assets/cache` after checking every file against
   its pinned LFS pointer, and `worldprops/default.conf` with one key changed;
5. runs each scenario in its own JVM and its own copy of that directory, twice,
   and refuses to continue unless the two passes agree byte for byte;
6. writes, or with `--check` compares, `fixtures/world-trace/trace.jsonl` and
   `provenance.json`. `--check` never rewrites them.

In the JVM, `oracle.worldtrace.WorldTrace` does what `Server.main` does up to
networking: `ServerConfigParser.parse("worldprops/default.conf")`, then
`GameWorld.prompt(false)`. That is the original start-up in its own order:
cache, authenticator, the pre-plugin configuration parsers, the class scan,
content interfaces, timers, world stores, legacy plugins, the post-plugin
parsers, start-up listeners. On this machine a boot takes about a minute.

After that the script drives the world:

| Step | What the harness calls |
| --- | --- |
| `connect` | constructs a session |
| `recv` | `session.getProducer().produceReader(session, bytes).run()`, as `IoEventHandler.read` does |
| `tick` | advances the clock 600 ms, then `MajorUpdateWorker.handleTickActions(false)` |
| `close` | `session.disconnect()`, as `IoEventHandler.read` does at end of stream |

Handshake, login decoding, RSA, the development authenticator, in-memory
account storage, `Login.proceedWith`, `LoginParser`, `Player.init`,
`LoginConfiguration`, the packet write queue, the update sequence, the
disconnection queue, `Player.clear`/`finishClear` and `PlayerSaver` are the
original classes, unchanged.

## What is not the original

- **The socket.** `TraceSession` extends `IoSession` and replaces the three
  members that touch the channel: `queue(ByteBuffer)` records the bytes,
  `write()` does nothing, `getRemoteAddress()` answers `127.0.0.1` as the
  original's own test session does. The reactor's thread pool is an executor
  that runs each event on the calling thread.
- **The real-time loop.** The worker thread in `MajorUpdateWorker` is never
  started. Its 600 ms sleep, `Server.heartbeat`, the 20-second ping timeout,
  the inactive-player failsafe and the midnight store reset and daily restart
  are therefore not exercised. `SystemManager.flag` would start that thread,
  so the harness sets the state it would have set (`PRIVATE`) directly.
- **`Server.main` after `prompt`:** the NIO reactor, the WebSocket server, the
  console reader, the watchdog and the shutdown hook.
- **Shutdown.** The JVM is halted after the last record.

## Pinned inputs

Each of these would otherwise differ from run to run. Every one is either
replaced by a recorded value or made to happen at a recorded point.

| Input | How it is pinned |
| --- | --- |
| Wall clock | After compilation `oracle/world-trace/tool/Rewrite.java` redirects every `System.currentTimeMillis` (158 sites), `new Date()` (7), `Calendar.getInstance` (6) and `LocalDate`/`LocalDateTime`/`LocalTime.now` (7) in the original's class files to `Pin`. The clock starts at 2023-07-12T12:00:00Z, stands still during a step and moves 600 ms per tick. The number of reads is recorded. |
| `new Random()` | 52 sites get the seed `n` for the n-th such construction; the count is recorded. |
| `RandomFunction.RANDOM`, `Math.random`, `Collections.shuffle` | each replaced, as in the other oracles, by a `java.util.Random` seeded 0 whose draws are counted |
| `ThreadLocalRandom`, `kotlin.random.Random` | the world thread's seed is set; its value is recorded |
| `GlobalScope.launch` (`TaskExecutor`, the management-event loop) | `Dispatchers.Default` is replaced by a queue. Blocks run on the world thread, first in first out, when the harness pumps after each step. The count is recorded. |
| Management events | `ManagementEvents.eventQueue` is replaced by a queue whose `take()` returns only when the harness grants it, one event per grant. Events are handled on the world thread after the step that published them, in publication order; their types are recorded. In the original they are handled concurrently, whenever the worker gets to them. |
| Reactor pool | runs on the calling thread |
| Identity hash codes | `-XX:+UnlockExperimentalVMOptions -XX:hashCode=2`: every object's identity hash is the same constant. See below. |
| JVM | `-XX:+UseSerialGC -Xshare:off`, UTF-8, `en-US`, UTC, in `provenance.json` |
| File system | a fresh directory per scenario and pass under `runs/world-trace-oracle`; a security manager refuses any write or delete outside it, any socket and any process, and a refusal fails the run |
| Database | none. `default.conf` has `use_auth = false` and `persist_accounts = false`, so the original uses `DevelopmentAuthenticator` and `InMemoryStorageProvider`, as its tests do. The MySQL driver is on the class path and never connects. The original creates two SQLite files inside the scenario directory (`data/eco/grandexchange.db`, `data/playerstats/player_stats.db`); their digests are in the boot record. |
| Configuration | `worldprops/default.conf` from the pinned commit with `enable_bots = true` changed to `false`. With bots on, the original spawns them from a separate thread and from wall-clock timers. Both digests are in `provenance.json`. |

**The original's world depends on identity hash codes.** Somewhere on the tick
path it iterates a hash map or set whose keys do not define `hashCode`, so the
order, and with it which NPC consumes which random draw, follows the JVM's
identity hashes. This was measured, not inferred: with the JVM's default
generator, a wrapper that handed out one extra identity hash on the world
thread before start-up, and changed nothing else, produced different NPC state
from tick 3 on; and an edit to the harness's own recording code changed the
trace from tick 5. Each variant was reproducible, none was the only valid
execution. With `-XX:hashCode=2` the same wrapper handing out 0 or 5 hashes
gives byte-identical traces, so the fixture uses it. The consequence is exact:
a map or set keyed by identity-hashed objects iterates in an order fixed by its
insertion history alone (one bucket, then HashMap's own list and tree-bin
order). A real server uses the default generator and would iterate
differently; this fixture is one valid execution, chosen because it can be
described. The collection or collections involved have not been identified.

**Not pinned individually, only observed to repeat:** the order in which
ClassGraph returns classes (hence registration order, which is recorded),
directory listing order, and anything else that depends on the JDK build or
the operating system. The evidence is empirical: the generator's two fresh JVM
passes agree, and a separate `--check` (two more fresh passes) reproduced the
fixture, all on one machine on one day. It was produced on Temurin 11.0.32.1
on Windows 11; a different JDK build or operating system may legitimately give
a different trace, and no proof rules out a rare difference here.

**Flagged, not pinned:** one `new SecureRandom()` site. A run that reaches it is
rejected; none of the four scenarios does.

The rewritten class files differ from the compiled ones only at the sites
counted in `provenance.json` (`redirectedCallSites`); a class with no such site
is copied byte for byte. A time or identity API the tool does not know stops the
build.

## Corpus

| Scenario | Script | Records | Ticks | Packets sent | Bytes sent |
| --- | --- | --- | --- | --- | --- |
| `boot` | start-up only | 2 | 0 | 0 | 0 |
| `new-character` | a new character logs in, 20 ticks without client input, the connection closes, 6 more ticks | 32 | 26 | 476 | 4,224 |
| `two-characters` | a new character, 5 ticks, a second new character, 15 ticks, first closes, 5 ticks, second closes, 6 ticks | 41 | 31 | 953 | 8,316 |
| `returning-character` | as `new-character`, starting from the save `new-character` wrote | 32 | 26 | 452 | 3,907 |

Bytes sent exclude what was queued after a connection closed (5, 12 and 5
entries). The boot record holds 13,767 NPCs, 14,659 item, 8,219 NPC and 42,009 scenery
definitions, 645 content interfaces and 1,796 legacy plugins.

The login block is built in `oracle/world-trace/scenarios.py` in the field
order `Login.decodeFromBuffer` reads, with the header values of the packet in
the original's `LoginTests`, and encrypted with the pinned client's public
exponent. Every byte a session receives is in the fixture.

## Trace schema, version 1

`fixtures/world-trace/trace.jsonl`: one JSON object per line, keys sorted.
Scenarios follow one another; each starts with a `scenario` record.

`scenario`: `name`, `description`, `clockStart` (epoch ms), `threadLocalSeed`,
`tickMillis`, `steps` (number of scripted steps), `filesFrom` (the scenario
whose saves seed this world, or null).

Every other record has:

| Key | Meaning |
| --- | --- |
| `t` | `boot`, `io`, `tick` or `end` |
| `i` | position within the scenario, from 0 |
| `tick` | `GameWorld.ticks` after the step |
| `op`, `session`, `hex` | the scripted step: `connect`, `recv` (with the bytes received), `close`, or `tick` |
| `out` | per session, in order, what was handed to its socket during the step |
| `sessions` | per session: `active`, and `producer` (the event producer class now reading it) |
| `players` | `Repository.players` in slot order: `slot`, `index` (what the player reports), `name`, `location`, `playing`, `active`, `artificial`, `renderable`, `queuedForDisconnect` |
| `lobby` | names in `Repository.lobbyPlayers` |
| `npcs` | `count` (occupied slots), `renderable`, `indexSha256` over `slot:index:id;` and `stateSha256` over `slot:index:id:x,y,z;` in slot order |
| `pulses` | length of the world pulse queue |
| `generators` | `clock`, `clockReads`, `seeded`, `shared`/`math`/`shuffle` as `[draws, 48-bit state]`, `threadLocal` (seed) |
| `handoffs` | `blocks` run from the dispatcher queue, `events` (management event types handled), `foreign` (must be empty) |
| `files` | files created, changed or removed in the world directory during the step: `path`, `size`, `sha256`; a character save also has `hex` |
| `stderrLines`, `refused`, `unpinned` | lines the original printed to standard error; guard refusals; unpinned sources reached. All zero or empty in this corpus. |

An entry of `out` has `k`: `handshake` (the 9-byte reply to opcode 14),
`login` (with `response`, the `AuthResponse` name), `packet` (with `op`, and
`size`: `NORMAL`, `BYTE` or `SHORT`) or `raw`; and `hex`, the bytes exactly as
the original queued them for the socket, header included. `dropped: true` marks
bytes queued after the connection closed: the original still queues them and
then sends nothing.

The `boot` record of the first scenario adds `listeners` (class names in
registration order for login, logout, tick, start-up, shutdown, world and
player persistence, login plugins and auto-start timers), `scan` (the class
names the scan returned, in its order, for content interfaces, `Initializable`
plugins and timers: the order the original instantiated them in), `tables` (counts,
the pulse queue's classes, timer type names), `registries` (size, and where the
keys allow a digest of the sorted keys, of every static map and collection of
21 registry classes), `npcs.list` (`[slot, index, id, x, y, z]` for every
occupied slot), `systemState`, `threads` (live non-JVM threads) and `sha256`.
Later scenarios boot the same world and carry `sameAsFirstScenario` and that
digest instead.

A slot and the index its occupant reports are separate fields because they
can differ: after start-up the pinned world holds one NPC (id 654) in slots 3
and 31, and it reports 31 from both. The trace records that as it is.

`provenance.json` holds the upstream pins, the source-tree manifest digest and
git tree, per-file hashes of the harness, tool, generator and the original
classes the harness drives, every jar's digest, the compiler and JVM flags, the
JDK version string, the call-site census, the configuration and data digests,
per-scenario counts and the fixture digest.

`crates/server/tests/world_trace.rs` parses the fixture, checks those pins and
asserts what a replayer will rely on: record and tick numbering, the clock,
monotonic generator positions, every sent byte attributed to a connected
session, frame headers consistent with their lengths, the login reply carrying
the listed player index, one save per character, and the counts above.

## Not covered

- Any client input after login. A real client sends display, map-loaded and
  keep-alive packets; here it sends nothing, so walking, the logout button,
  interfaces and every action packet are absent.
- A clean logout. The connection simply closes; the original queues the
  player, removes it at the next disconnection-queue tick and saves.
- Reconnect login (opcode 18), wrong passwords, banned or duplicate logins, the
  account limit, the lobby screen (a character with `tutorial:complete`), world
  list and JS5 requests.
- The 50-tick autosave: no scenario reaches tick 50.
- Bots, the daily restart, the real-time loop's checks, shutdown, MySQL,
  production authentication, the management server.
- NPC and player state beyond index, id and position; timers, attributes,
  varps and containers are visible only through packets and the save.
- Other JDKs and operating systems.
- Per-draw records: generator positions are per step, so a replayer that
  diverges learns the step, not the draw.

## What a native replayer needs next

A replayer for `boot` has to load the same tables and reach the same NPC list
and generator positions; it will need the start-up order of the loaders and
their draws, which this trace gives only in total. A replayer for the login
scenarios has to produce the same bytes per session per tick. Expect to extend
the schema (per-phase generator positions, per-NPC state) when the first native
mismatch needs localising; bump `schema` and regenerate, never edit.

## Reproducing from a fresh checkout

`cargo xtask bootstrap` and `cargo xtask cache-fetch` (the cache is needed),
then the command above with `--java-home` and, where `python` is not on the
path, `--python`. Missing jars are copied from other `runs/*` directories when
their digests match and otherwise downloaded from the pinned URLs (Maven
Central, and the project's package registry for ConstLib). The reference
checkout is a partial clone, and the generator reads `Server/data` from its
object store without fetching. Where those objects are absent it stops; setting
`RS09_WORLD_TRACE_FETCH_DATA=1` lets git fetch exactly those pinned objects.
That path has not been exercised: on the machine the fixture was made on, the
objects were already present. Everything built or extracted goes to
`runs/world-trace-oracle`, which is ignored by git. A first run takes about ten
minutes; with the compiled classes cached, about seven.
