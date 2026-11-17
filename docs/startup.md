# Native start-up (`GameWorld.prompt`)

`crates/server/src/startup/` holds the original server's start-up as a native
library: the loaders that had no port, one world-owned set of the original's
random generators, the class scan as a native registration manifest, and the
start-up sequence in the original order with every step either run natively or
run in part with what it leaves out named. It is a library and a test. Nothing
installs it: `main.rs`, the examples and the endpoint are unchanged, and the
`World` it produces is **not** a playable world.

## The oracles

### The original start-up, one statement at a time

`cargo xtask startup-loaders-oracle [--check]` (Python 3.10+, JDK 11, the
pinned cache) builds on `oracle/world-trace`: the same unchanged compiled
server (3,487 sources; the compiled classes are shared with
`runs/world-trace-oracle`), the same redirected clock and generator call
sites, the same world directory and the boot scenario's inputs (all three
`java.util.Random` sources seeded 0, thread-local seed 537462064,
`-XX:hashCode=2`). Instead of calling `GameWorld.prompt(false)`,
`oracle/startup-loaders/harness/.../StartupPhases.java` calls the statements
of `prompt` one at a time in their order, with `ConfigParser.prePlugin` and
`postPlugin` expanded loader by loader and the start-up listeners run one by
one. After every call it records the generator positions, the standard-error
line count and the original's own log lines. After the loaders this port
covers it dumps the tables they left behind, read only (no lookup that creates
a definition, region or data map). Then it writes the boot record exactly as
`WorldTrace` does (`BootRecord.java` is a statement-for-statement copy of the
part of `WorldTrace.boot` after its `prompt` call).

A run is accepted only if two fresh JVM runs agree byte for byte **and** its
boot record is byte-identical to the boot record of
`fixtures/world-trace/trace.jsonl` (same digest). It is: running the start-up
one statement at a time did not change it. The world-trace generator, harness
and fixture are unchanged.

After the boot record the harness runs XteaParser, ObjectConfigParser,
ObjectOverrideParser, ShopParser, MusicConfigLoader, RangedConfigLoader and
ClueRewardParser again over 47 authored inputs (`CONFIG_PATH` pointed at a case
directory; malformed rows, missing files, bad JSON, wrong types, short arrays,
duplicate keys, Kotlin/Java parsing edge cases) and records the exception
class that left the loader, the draws it made, its log lines and what it left
behind.

`fixtures/startup-loaders/`: `oracle.jsonl` (52 phases, 6 tables, 47 cases),
`cases.json` (the authored inputs), the nine pinned input files the portable
tests read (each checked against the world-trace provenance digest) and
`provenance.json`.

### Who draws what (`cargo xtask startup-draws-oracle`)

`oracle/startup-draws` builds on the oracle above. Its harness
(`StartupDraws.java`) runs the same statements, through `StartupPhases`' own
phase and table methods, with the three counting generators wrapped by a
`java.util.Random` that records, for every `next(bits)` call, the
`Random`/`Math`/`Collections` method that called it, the `RandomFunction`
method, the first non-library frame, the innermost `content.*` frame, the
first frame outside `core.api.utils`, the phase's own entry frame and what that
entry called. After every phase it reads, without creating anything: the NPC
slots the phase added to `Repository.npcs` (ranges), the NPCs whose class is
not the base `NPC` (class, count, ids), the shared draws by
`Repository.npcs.size()` at draw time, the region ids, NPC and scenery
definition ids each phase added, `GroundItemManager`'s list size,
`ChunkUpdateTracker`'s dirty set (the chunk bases each phase added), the
renderable NPCs, `RegionChunk.dirtyListener`'s class, `Auth`'s two fields,
`ServerStore`'s file map and counter, `GraveController`'s graves, and every
scenery definition's cached examine text (a digest over all definitions and
over the ones `Cache.init` created).

A run is accepted only if two fresh JVM runs agree byte for byte, its phase
and table lines are **identical, line for line, to those of
`fixtures/startup-loaders/oracle.jsonl`** (so recording changed no draw, log
line or table), its boot record is byte-identical to the world-trace fixture's
and every phase's attributed draws add up to that phase's counted draws. It
is. Thread-local draws cannot be intercepted (their seed per phase is in the
startup-loaders phases); draws inside a phase are aggregated, not ordered.
`fixtures/startup-draws/`: `draws.jsonl` (52 phases) and `provenance.json`
(source, harness, jar and fixture digests). The startup-loaders oracle and its
fixture are unchanged by this.

### What the original's start-up draws, by phase and by class

Shared `RandomFunction.RANDOM` / `Math.random` / `Collections.shuffle` draws
(`next(bits)` calls); TL = the thread-local seed moved.

| Phase | shared | math | shuffle | TL | who draws (fixtures/startup-draws) |
| --- | ---: | ---: | ---: | --- | --- |
| ClassScanner.loadPureInterfaces | 328 | 0 | 2,336 | yes | `WeightBasedTable` insertions in content static initialisers and constructors: ImplingLoot 187, TheDigSiteListeners 70, MazeInterface 9, RoguesCastleListeners 8, CasketListener 7; the holiday world events Giftmas 35 and EasterEvent 12; shuffle: Vinesweeper.populateSeeds 2,336 |
| ClassScanner.loadSideEffectfulPlugins | 50 | 14 | 6 | yes | AntiMacro registrations of 28 NPCs content plugins create (MimeActivity 4, GnomeStrongholdCourse 5, NoraTHaggNPC 1, SheepHerder 17, ShamusTreeListener 1), Forager 21, EnchantingZone 1; math: PeerTheSeerDialogue 14; shuffle: AlchemistZone 6 |
| DropTableParser | 8,412 | 0 | 0 | | DropTableParser |
| NPCSpawner | 13,846 | 0 | 0 | | one `AntiMacro.onRegister` draw (`RandomFunction.random(3000, 9001)`) per NPC: 13,709 spawned + 12 GodWarsMinionNPCs GodwarsBossNPC.init creates; MineSlaveNPC's constructor 90; PuroPuroPlugin's WheatSet 35 |
| ClueRewardParser | 358 | 0 | 0 | | ClueRewardParser |
| startup: RareDropTable | 27 | 0 | 0 | | RareDropTable |
| startup: PenguinHNSEvent | 10 | 0 | 0 | yes | AntiMacro of the 10 penguins it spawns |
| startup: SpotManager | 10 | 0 | 0 | yes | 5 spot TTLs, AntiMacro of the 5 spots |
| every other phase | 0 | 0 | 0 | | |
| **total** | **23,041** | **14** | **2,342** | | |

So the class scan's own draws are all content or the holiday world events;
the engine's only start-up draw outside the loaders is `AntiMacro`'s, made on
every NPC `Entity.init` registers the automatic timers on. NPCs created before
`loadTimers` (the 3 the content interfaces create) make none. The thread-local
seed moves once more when the boot record is written: that pumps the one block
the start-up queued on the dispatcher. `ServerConfigParser` constructs the
three seeded `java.util.Random`s.

## Loaders ported (`startup/loaders/`)

Each follows its Kotlin source statement by statement: JSON-simple grammar and
stringification (`java_json`), `Integer.parseInt`/`Double.parseDouble`,
Kotlin `split` (trailing empty parts kept), `trim`/`isNotBlank` with Kotlin's
whitespace, `removeSurrounding`, evaluation order inside a row, the exception
at the same point (`LoadError::java_class` names it), rows before a failure
kept. None of them catches anything, as in the original.

- `xtea` (`XteaParser`): `REGION_XTEA` and `getRegionXTEA` (four zeros by
  default). All key parts are parsed before the fourth is indexed; `+7` is a
  valid region; extra keys are ignored.
- `object_config` (`ObjectConfigParser`): `examine` handler and
  `animationId` on scenery definitions. `forId` stores a new definition for an
  id the cache lacks. A row's fields are visited in `JSONObject` (`HashMap`)
  order, so a bad `render_anim` fails before or after the row's `examine`;
  empty and `"null"` values are skipped; `65535` stays `65535`.
- `object_override` (`ObjectOverrideParser`): `addSceneries`/`removeSceneries`
  per region (`startup/regions.rs`). The stored rotation is the direction's
  **ordinal** (`n` is 1, `nw` 0), not its value; an id below 1 makes the type
  22; `Location` keeps 15-bit signed x/y and z modulo 4; duplicates by
  `Scenery.equals` are not added but are counted; an unknown mode is logged at
  ERR and counted; the Scenery constructor's definition lookup creates
  definitions.
- `shops` (`ShopParser`): the pinned body only counts array elements (the
  shop construction is commented out in the original). Shops are built by the
  `core.game.shops.Shops` start-up listener, now a separate library
  (`world::shops::controller::startup`, docs/shops.md) that this sequence
  does not run: it belongs to the refused start-up listener step.
- `music` (`MusicConfigLoader`): `DataMap` (cache index 17) decoding with
  `CP1252` types and the original string reader that widens bytes above 127 as
  signed (U+FF80..U+FFFF); songs from maps 1351/1345 in `HashMap<Integer,…>`
  order with `putIfAbsent`; region tracks; tile zones with the original
  exception-string assembly, one shared zone object per border box added to
  every region `getRegionIds` returns.
- `ranged` (`RangedConfigLoader`): ammunition (`put`, last wins) and range
  weapons (`putIfAbsent`, first wins); `itemId as String` (a JSON number is a
  `ClassCastException`); blank dark-bow graphic is none; `BoltEffect.forId`;
  attack speed from the item's `attack_speed` configuration, default 4;
  `toBoolean` is case-insensitive `true`. The two throwaway `NPC` objects the
  original constructs for each projectile are not modelled: the pinned phase
  draws nothing and nothing reads them.
- `clue_rewards` (`ClueRewardParser`): four `WeightBasedTable`s (the existing
  `world::drop_tables` type) filled with random insertions from the shared
  generator; a table replaces the old one only when complete.

Already native and joined by the sequence (not re-ported): item and scenery
cache definitions, lazy NPC definitions, `NPCConfigParser`,
`ItemConfigParser`, `InterfaceConfigParser`, `DropTableParser`,
`DoorConfigLoader`, `CustomVarbitParser`, `NPCSpawner`'s parser
(`world::npc_spawns`), NPC creation and initialisation (`world::npc_init`),
`GroundSpawnLoader` (`world::ground_spawn_loader`) with the ground-item
manager and `GroundSpawn` (`world::ground_manager`, `world::ground_spawn`),
`ChunkUpdateTracker` (`world::chunk_sync::DirtyTracker`), `Auth.configure`
(`auth::factory`), the repository (`world::repository`),
`setDefaultBehavior` (`world::npc_behaviour::aggression`), the timer manager
and registry (`timers`, `timers::registry`) and the `take` option handler
(`world::pickup`, through `world::ground_options`).

## Ported in this module besides the loaders

- `store` (`ServerStore.parse`, `GraveController.parse`): one JSON object per
  file of `STORE_PATH`, keyed by the name without its extension, and the
  counter. A missing directory is created (through the host's
  `WorldWrites`, a required input) and nothing is read; a file that is not a
  JSON object is logged at ERR and skipped; a file that cannot be opened stops
  the start-up (it is opened outside the original's `try`). Files are read in
  the order the operating system lists them, as `File.listFiles` does.
  `GraveController.parse` reads `getArchive("active-graves")`, which stores an
  empty object under that name; an archive with graves is refused (graves are
  content).
- `scan` (the class scan): `scan_order.txt` is the original's scan result and
  listener registration order on the world-trace build, section by section
  (`scan.contentInterfaces` 667, `scan.initializable` 1,501, `scan.timers` 35,
  and the nine `listeners.*` lists), checked against the boot record by the
  test. `scan::native` names the scanned classes with a native registration:
  `core.ServerStore` and `GraveController` (world persists), the timers below,
  `PickupPlugin` and the AbstractNPC classes of `npcs::STARTUP_NPC_CLASSES`
  (docs/npc-classes.md). Every other scanned class is reported by name, in the
  original's order, as "content not ported".
- `timers` (the engine timers the scan registers): native prototypes of
  `GrandExchangeTimer` (500, "GE periodic poll", soft, automatic), `AntiMacro`
  (0, "antimacro", automatic, persistent) and `HolidayRandoms` (0, "holiday",
  automatic, persistent) with what a registration on an NPC runs.
  `AntiMacro.onRegister` removes the timer from a non-player and, with
  `runInterval` 0, sets `runInterval = RandomFunction.random(3000, 9001)` (one
  `RANDOM.nextInt(6001)`) and `nextExecution = ticks + runInterval`;
  `HolidayRandoms.onRegister` removes it from a non-player. Their player
  branches, `run`, `save` and `parse` refuse. `SkillRestore` is not ported: its
  `onRegister` hooks two entity events and `Entity.hook` has no native port.
- `ground` (the `GroundSpawnLoader` binding) and `npcs` (the `NPCSpawner`
  binding): see their module comments for every service and what answers it.

## The native start-up (`startup/sequence.rs`)

`sequence::start(&Inputs, Policy)` runs `ORDER`, which is the oracle's phase
order (the start-up listeners are one step). `Inputs` carries the world
directory, the seeds, a definition bound, the host's `WorldWrites` and its
default-locale `toLowerCase` (no defaults). `Policy::StopAtRefusal` stops
before the first step that is not entirely native with
`StartupError::NotPorted`; `Policy::ContinuePastRefusals` runs the native part
of such a step, logs it (`partly ported: …`), records the named gaps in
`Outcome::Partial` and returns `complete = false`. A failing native step is
the original's uncaught exception: `StartupError::Failed`.

| # | Step | Native, or native in part |
| --- | --- | --- |
| 1 | ServerConfigParser.parse | native (`config::Configuration::load` of `worldprops/default.conf`) |
| 2 | prompt log line | native |
| 3 | Cache.init | native: cache store; item (19) and scenery (16) definitions; NPC archive (18) for lazy lookups |
| 4 | RegionChunk.dirtyListener | native: the world's `DirtyTracker` (`ChunkUpdateTracker`) replaces the initial no-op listener |
| 5 | Auth.configure | native: `auth::factory::Registry` from the configuration; its online-friend lookup reads the world repository's name map with the host's lower-casing |
| 6 | ConfigParser() | native (no state) |
| 7 | NPCConfigParser | native (`definitions::npc::config`) |
| 8 | ItemConfigParser | native (`definitions::item_config`) |
| 9 | ObjectConfigParser | native |
| 10 | XteaParser | native |
| 11 | ObjectOverrideParser | native |
| 12 | InterfaceConfigParser | native (`world::interfaces`) |
| 13 | ClassScanner.scanClasspath | native: the manifest is the scan result |
| 14 | ClassScanner.loadPureInterfaces | in part: ServerStore and GraveController registered as world persists; 665 other scanned classes named as content not ported |
| 15 | ClassScanner.loadTimers | in part: GrandExchangeTimer, AntiMacro, HolidayRandoms registered in the original's order (with its log lines); 31 other timers named |
| 16 | ServerStore.parse | native (`store`) |
| 17 | other worldPersists.parse (GraveController) | native for an empty `active-graves` archive (`store`) |
| 18 | ClassScanner.loadSideEffectfulPlugins | in part: PickupPlugin (the `take` handler) and the prototypes and ids of ShopNPC, CowNPC, DuckNPC, RatNPC, GuardNPC, MonkNPC, HighwayManNPC and IceGiantNPC; 1,492 other plugins named |
| 19 | ShopParser | native |
| 20 | DropTableParser | native (`world::drop_tables::parser`, generators from the world) |
| 21 | NPCSpawner | in part (`npcs`): NPCs of the registered AbstractNPC classes constructed as those classes (`npcs.subclasses` matches the original's), every other NPC a base NPC; zone updates, NPC interaction defaults, slayer tasks, magic autocast spells and the SkillRestore timer counted as not ported |
| 22 | DoorConfigLoader | native (`world::doors`) |
| 23 | GroundSpawnLoader | in part (`ground`): 717 spawns; ground items' option tables hold only the native `take` handler |
| 24 | MusicConfigLoader | native |
| 25 | RangedConfigLoader | native |
| 26 | CustomVarbitParser | native (`definitions::varbit`) |
| 27 | ClueRewardParser | native (generators from the world) |
| 28 | start-up listeners | in part: none is native; the 23 listeners are named |
| 29 | SceneryDefinition.getExamine loop | native: `examine` cached on every definition (`Definition.getExamine`, `StringUtils.isPlusN` over the host's lower-casing) |
| 30 | PRELOAD_MAP | native when off (pinned); refused when on |
| 31 | System.gc | native (nothing to do) |

The paths are resolved against `Inputs::world` (the original's working
directory); nothing reads the process working directory.

## Comparison with the boot record

`crates/server/tests/startup.rs`:

- Portable (fixtures only): the 47 authored loader cases; the pinned XTEA
  table, overrides, shop count, clue tables and drop-table draws at the
  original's generator positions; the fixtures' pins and boot identity; the
  generator report format; the draw attribution's pins and per-phase totals;
  the drawing classes of the class scan and the listeners (pinned per class,
  none of them natively registered); the scan manifest against the boot
  record's scan and listener lists.
- Whole start-up (needs `runs/startup-loaders-oracle/template`, extracted by
  the generator, or `RS09_STARTUP_WORLD`; every file is checked against the
  world-trace provenance first; without it the test prints `SKIPPED` and
  compares nothing): `sequence::start` over the pinned data, compared field by
  field. It asserts that the set of mismatching fields is exactly
  `KNOWN_GAPS`, so a new mismatch and a closed gap both fail. Its writes are
  recorded, not made.

Before this slice: 56 fields matched and 22 were pinned gaps (eight of the
56 were a refused step's zero draws against the original's zero). Now **79
fields match and 27 are pinned gaps**; more fields are compared: the draws and
log lines of every step that runs natively or in part, the NPC digests, and
the checks against the attribution.

Newly matching: `GroundItemManager.GROUND_ITEMS` (717, a closed gap); the log
lines of RegionChunk.dirtyListener, Auth.configure,
ClassScanner.scanClasspath, ServerStore.parse, worldPersists.parse,
NPCSpawner (`Spawned 13709 NPCs`), GroundSpawnLoader (`Initialized 717 ground
items.`) and SceneryDefinition.examine; and:

- `npcs.spawner`: the native spawner's 13,709 NPCs (id and position) are, in
  order, the NPCs of the original's spawner phase; every NPC of that phase
  left over is a GodWarsMinionNPC (content).
- `draws-explained:NPCSpawner`: one native draw per native NPC, one AntiMacro
  draw per NPC the original's phase added, and native draws + the 12 minions'
  + the content draws = the original's 13,846.
- `npcDefinitions.nativePlusContent`, `sceneryDefinitions.nativePlusContent`,
  `regions.nativePlusContent`, `regions.nativeSubsetOfOriginal`: the native
  key sets plus the keys the original's content phases created digest to the
  boot record's. The only NPC definition that content alone looks up is 3813;
  the scenery definitions are 42027, 42028 and 48248; three regions of the
  spawner phase (10541, 13367, 14382) hold no spawn location and are reached by
  something the native spawner does not run (content NPC classes' overrides,
  zone updates, NPC interaction defaults, slayer tasks, autocast spells; not
  identified individually).
- `dirtyChunks.GroundSpawnLoader`: the chunks the native ground spawns flagged
  through the installed listener are exactly the 492 the original's phase
  added to `ChunkUpdateTracker`.
- `auth` (DevelopmentAuthenticator over InMemoryStorageProvider),
  `serverStore` (0 files, `active-graves`), `sceneryExamines.cacheDefinitions`
  (all 42,006 cached examine texts), `log-native:ClassScanner.loadTimers` (the
  three native registrations are the original's lines for those classes, in
  order), `timers.automatic` (the original's automatic timers without
  SkillRestore), `listeners.persistWorld`.
- `serverStore.created`: the native step asked the host to create exactly
  `STORE_PATH`. The oracle does not record directories; this checks the
  original's code path (`dir.mkdirs()` when it is missing), read from source.

Known gaps (27), each with its cause in `KNOWN_GAPS`: the draws of
loadPureInterfaces, loadSideEffectfulPlugins, NPCSpawner and the listeners
(all attributed to content above), hence the four final generator positions
(native shared 22,479 of 23,041: the 562 content draws; math 0 of 14; shuffle
0 of 2,342; thread-local unmoved); the logs of loadPureInterfaces,
loadSideEffectfulPlugins, loadTimers and the listeners; `npcs.count` (13,709 of
13,767: 31 NPCs from the class scan, 12 GodWarsMinionNPCs, 15 from listeners)
and the two NPC digests (slots move); NPC definitions (8,218 of 8,219),
scenery definitions (42,006 of 42,009), component definitions (162 of 253),
the region cache (796 of 982; 796 of 951 at the music loader); content
interfaces (2 of 645), legacy plugins (1 of 1,796), timer types (3 of 34),
pulse classes; and the clue tables' stored order (built at native shared draw
22,121 instead of 22,636).

Not compared, no native counterpart: the other listener lists, the other
registries, renderable NPCs (0 in both), pulses, system state, threads, files,
clock reads and seeded-construction count.

## Deviations from the original

- Configuration and store files that are not UTF-8 are refused (the
  original's `FileReader` would substitute U+FFFD).
- A music zone covering more than 65,536 regions is refused (the original
  would try to create every region).
- A `DataMap` that reads past its end or names type byte 0 is refused (Java's
  `BufferUnderflowException`/`IllegalArgumentException` would leave `prompt`).
- `StartupError::Failed` carries the native error text, not a Java stack trace.
- The original's log lines are produced for the native steps; their clock
  prefix and the clock reads that make it are not modelled (nor
  `HolidayRandoms`' four `LocalDate.now()` reads per NPC).
- Region chunks are created one at a time when `RegionManager.getRegionChunk`
  asks for them; `world::region::Region` (which builds all 256 chunks of a
  region) is not used for the thousand regions the start-up touches. Joining
  `world::scene` later must adopt these chunk objects.
- NPC skills keep static and dynamic levels and lifepoints; the experience
  `setStaticLevel` computes is not kept. NPC properties keep the members
  `initConfig` and `configure` set.
- A region's `isActive()` answers false during start-up (no player's viewport
  can have activated one).
- The `ServerStore` file map keeps insertion order; its `HashMap` iteration
  order is not modelled (nothing in the start-up walks it).

## What login and the tick driver need from this

The world now holds what a login and a tick reach first: the repository with
the spawner's NPCs (canonical actors, chunk membership, timer managers with
their automatic timers, default aggression), 717 ground spawns on their chunks
with the dirty set the first tick resets, the store, the authenticator and its
provider, the timer registry, and the generators at a known position. Still
missing for them: the content the class scan registers (login listeners and
`LoginConfiguration` plugins, interface and interaction listeners, NPC classes,
behaviours, zones, the other 31 timers, `SkillRestore` with entity event
hooks), the start-up listeners (shops, the Grand Exchange, ZoneBuilder, drop
tables), the NPC option tables and slayer tasks, and the NPCs content creates.
Every random draw after start-up happens at a different generator position
than in the world-trace scenarios until the 562 content draws above exist:
the per-class table says exactly which.

## Reproducing

`cargo xtask bootstrap`, `cargo xtask cache-fetch`, then
`cargo xtask startup-loaders-oracle --check --java-home <JDK 11>` and
`cargo xtask startup-draws-oracle --check --java-home <JDK 11>` (and
`--python` where needed). The first compiles the original server once (about
three minutes, shared with `world-trace-oracle`); each boots it twice (about a
minute each). Then `cargo test -p rs09-server --test startup`.
