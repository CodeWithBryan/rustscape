# Native start-up (`GameWorld.prompt`)

`crates/server/src/startup/` holds the original server's start-up as a native
library: the loaders that had no port, one world-owned set of the original's
random generators, and the start-up sequence in the original order with every
step either run natively or refused. It is a library and a test. Nothing
installs it: `main.rs`, the examples and the endpoint are unchanged, and the
`World` it produces is **not** a playable world.

## The oracle: the original start-up, one statement at a time

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

### What the original's start-up draws, by phase

Shared `RandomFunction.RANDOM` / `Math.random` / `Collections.shuffle` draws
(`next(bits)` calls); TL = the thread-local seed moved.

| Phase | shared | math | shuffle | TL |
| --- | ---: | ---: | ---: | --- |
| ClassScanner.loadPureInterfaces | 328 | 0 | 2,336 | yes |
| ClassScanner.loadSideEffectfulPlugins | 50 | 14 | 6 | yes |
| DropTableParser | 8,412 | 0 | 0 | |
| NPCSpawner | 13,846 | 0 | 0 | |
| ClueRewardParser | 358 | 0 | 0 | |
| startup: RareDropTable | 27 | 0 | 0 | |
| startup: PenguinHNSEvent | 10 | 0 | 0 | yes |
| startup: SpotManager | 10 | 0 | 0 | yes |
| every other phase | 0 | 0 | 0 | |
| **total** | **23,041** | **14** | **2,342** | |

The thread-local seed moves once more when the boot record is written: that
pumps the one block the start-up queued on the dispatcher. `ServerConfigParser`
constructs the three seeded `java.util.Random`s.

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
  `core.game.shops.Shops` start-up listener, which is content and not ported.
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
`DoorConfigLoader`, `CustomVarbitParser`.

## The generator set (`startup/generators.rs`)

`WorldGenerators` owns one world's `shared` (`RandomFunction.RANDOM`), `math`
(`Math.random`), `shuffle` (`Collections.shuffle`) generators, each a
`CountingRandom` (JDK 11 `java.util.Random` counting `next(bits)` calls in the
oracle's unit), and the world thread's `ThreadLocalRandom`. Seeds are an
explicit `Seeds` input with no default; `construct_unseeded` counts the
original's `new Random()` constructions with a host-supplied seed. It
implements `drop_tables::random::Source`, so `DropTableParser`, the shared and
NPC drop-table rolls and `ClueRewardParser` take it directly
(`&mut WorldGenerators`). It also has the member calls the other libraries'
host services ask for (`math_random` and `shared_next_int` for
`combat_swing`'s two random members, `shared_next_int`/`shared_next_double`/
`shuffle_next_int`/`thread_local_next_double` for gathering's four
`Rng*` services): those libraries still take the generators through their
host traits, and no host in this repository answers them from a
`WorldGenerators` yet.

## The native start-up (`startup/sequence.rs`)

`sequence::start(&Inputs, Policy)` runs `ORDER`, which is the oracle's phase
order (the start-up listeners are one step). `Policy::StopAtRefusal` stops at
the first refused step with `StartupError::NotPorted`;
`Policy::ContinuePastRefusals` logs each refusal (`not ported: …`), records it
and returns `complete = false`. A failing native step is the original's
uncaught exception: `StartupError::Failed`.

| # | Step | Native or refused |
| --- | --- | --- |
| 1 | ServerConfigParser.parse | native (`config::Configuration::load` of `worldprops/default.conf`) |
| 2 | prompt log line | native |
| 3 | Cache.init | native: cache store; item (19) and scenery (16) definitions; NPC archive (18) for lazy lookups |
| 4 | RegionChunk.dirtyListener | refused: chunk_sync not joined |
| 5 | Auth.configure | refused: auth factory exists, its world-owner lookup is not joined here |
| 6 | ConfigParser() | native (no state) |
| 7 | NPCConfigParser | native (`definitions::npc::config`) |
| 8 | ItemConfigParser | native (`definitions::item_config`) |
| 9 | ObjectConfigParser | native (new) |
| 10 | XteaParser | native (new) |
| 11 | ObjectOverrideParser | native (new) |
| 12 | InterfaceConfigParser | native (`world::interfaces`) |
| 13-15 | ClassScanner.scanClasspath / loadPureInterfaces / loadTimers | refused: content |
| 16 | ServerStore.parse | refused: not ported |
| 17 | other worldPersists.parse (GraveController) | refused: content |
| 18 | ClassScanner.loadSideEffectfulPlugins | refused: content |
| 19 | ShopParser | native (new) |
| 20 | DropTableParser | native (`world::drop_tables::parser`, generators from the world) |
| 21 | NPCSpawner | refused: row parsing exists (`world::npc_spawns`), NPC.create/init/Repository not joined |
| 22 | DoorConfigLoader | native (`world::doors`) |
| 23 | GroundSpawnLoader | refused: needs a ground-item manager/world context |
| 24 | MusicConfigLoader | native (new) |
| 25 | RangedConfigLoader | native (new) |
| 26 | CustomVarbitParser | native (`definitions::varbit`) |
| 27 | ClueRewardParser | native (new, generators from the world) |
| 28 | start-up listeners | refused: the list comes from the class scan |
| 29 | SceneryDefinition.getExamine loop | refused: not ported |
| 30 | PRELOAD_MAP | native when off (pinned); refused when on |
| 31 | System.gc | native (nothing to do) |

The paths are resolved against `Inputs::world` (the original's working
directory); nothing reads the process working directory.

## Comparison with the boot record

`crates/server/tests/startup.rs`:

- Portable (fixtures only): the 47 authored loader cases (exception class,
  draws, log lines, resulting tables); the pinned XTEA table (1,122 regions),
  the pinned overrides (26 rows, 11 regions, including the region-cache size
  and key digest), the shop count (245), the pinned clue tables and the drop
  tables' draws **at the original's generator positions** (22,636 and 378
  draws, then 358 and 8,412 draws ending on the original's state), the
  fixture's pins and boot identity, and the generator report format.
- Whole start-up (needs `runs/startup-loaders-oracle/template`, extracted by
  the generator, or `RS09_STARTUP_WORLD`; every file is checked against the
  world-trace provenance first; without it the test prints `SKIPPED` and
  compares nothing): `sequence::start` over the pinned data, compared field by
  field. It asserts that the set of mismatching fields is exactly
  `KNOWN_GAPS`, so a new mismatch and a closed gap both fail.

Matching exactly (56 fields): the step order; per native step its own draws
and the original's log lines (NPC/item/object/interface configs, XTEAs,
overrides, shops, drop tables, doors, music, ranged, varbits, clue rewards,
cache, prompt line); the ObjectConfigParser rows (13,939 definitions with
handler keys and animation ids), XTEA keys, music region rows, zones and
songs (612), ranged ammunition (221) and weapons (176); and in the boot
record `tables.itemDefinitions` and the size and key digest of
`ItemDefinition.DEFINITIONS`, `VarbitDefinition.MAPPING`,
`RangeWeapon.RANGE_WEAPONS` and `MusicEntry.SONGS`.

Known gaps (22), each caused by a refused step: the draws of the refused
class scan, plugins, NPCSpawner and start-up listeners, hence all four final
generator positions (native shared 8,770 of 23,041; math 0 of 14; shuffle 0
of 2,342; thread-local unmoved); the clue tables' stored order in the native
start-up (built at native draw 8,412 instead of 22,636; the drop tables'
order differs for the same reason although their draw count matches by
chance); NPC definitions (8,036 of 8,219) and scenery definitions (42,006 of
42,009) looked up by refused steps; component definitions (162 of 253);
ground items (0 of 717); the region cache (651 of 982, 951 at the music
loader); NPCs (0 of 13,767); content interfaces, legacy plugins, pulse classes
and timer types.

Not compared, no native counterpart: listeners, scan lists, the other
registries, the NPC list and digests, pulses, system state, threads, files,
clock reads and seeded-construction count.

## Deviations from the original

- Configuration files that are not UTF-8 are refused (the original's
  `FileReader` would substitute U+FFFD).
- A music zone covering more than 65,536 regions is refused (the original
  would try to create every region).
- A `DataMap` that reads past its end or names type byte 0 is refused (Java's
  `BufferUnderflowException`/`IllegalArgumentException` would leave `prompt`).
- `StartupError::Failed` carries the native error text, not a Java stack trace.
- The original's log lines are produced for the native steps; their clock
  prefix and the clock reads that make it are not modelled.

## What the next spine steps need from this

A complete `Player` and login need the refused steps that feed them: the class
scan's content (login listeners, `LoginConfiguration` plugins, interface
listeners, timers), `NPCSpawner` with NPC.create/init on canonical actors and
the repository, ground spawns on the ground world, the shops listener, the
chunk dirty listener and `Auth.configure` joined to the world. Until the
refused steps draw what the original draws, every later random draw of a
native world is at a different generator position than the world-trace
scenarios; per-step draw counts above say exactly how far.

## Reproducing

`cargo xtask bootstrap`, `cargo xtask cache-fetch`, then
`cargo xtask startup-loaders-oracle --check --java-home <JDK 11>` (and
`--python` where needed). It compiles the original server once (about three
minutes, shared with `world-trace-oracle`) and boots it twice (about a minute
each). Then `cargo test -p rs09-server --test startup`.
