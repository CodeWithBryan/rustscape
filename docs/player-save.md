# The original character save file

`server::player::save` ports the two pinned files that write and read a whole
character: `PlayerSaver.kt` and `PlayerSaveParser.kt`. It produces the JSON text
the original writes and loads what the original writes, using the native
profile libraries that already exist for the individual sections.
`server::player::persisted` holds the persisted state of most systems the two
files reach through `save::Host` (quests, music, emotes, diaries, autocast,
grave type, pouches, POH storage, and the saving side of the house and
familiars); see [The persisted state behind `Host`](#the-persisted-state-behind-host).

This is a library. Nothing calls it at login or logout yet, and the playable
alpha still uses its own snapshot format (see [playable-alpha.md](playable-alpha.md)).
It is not a complete profile, a playable login or a replacement for either
source file's place in the running server; both source inventory entries stay
`not_replaced`.

## What it does

`save::populate` builds the root object in the original section order and
`save::save_text` returns `populate().toJSONString()`, which is what the saver
writes on Java 11 and later. `save::save_file` adds the file step: nothing
happens until `saveParsed` is set, the directory is created when the file is
missing, and the file is replaced in place. A failed write is reported as
`SaveOutcome::Failed`; the original prints the exception and returns.

`save::Parser` has the two fields of a `PlayerSaveParser`, `read` and
`save_file`. `Parser::parse` reads `<save path><name>.json`, logs the original
warning and clears `read` when the file is missing, parses the text and runs
`parseData`. `Parser::run_content_hooks` runs the content hooks and then sets
`saveParsed`. Each `parse*` and `save*` section is also a public function,
because the original methods are public and are called on their own.

A profile is passed as `save::Profile`, a set of borrows of the existing typed
state: `CoreData` with its timer manager, `GameAttributes`, `Skills`,
`Settings`, `SavedData`, `Appearance`, the bank PIN manager, the ironman
manager, the save version, the `saveParsed` flag and the spellbook interface
id. The module does not hold a second model of any of them.

## Sections

Handled by native libraries, joined here:

| Save keys | Native state |
| --- | --- |
| `version` | `Profile::version` |
| `core_data` (containers, bank tabs, bank selector, location, varps, timers) | `core_data`, `timers` |
| `attributes`, and the legacy `<name>.xml` file when the key is absent | `attributes` |
| `skills`, `totalEXP`, `exp_multiplier`, `milestone` | `skills` |
| `settings` | `settings`; `retaliation` is a host value |
| `activityData`, `questData`, `globalData` | `saved_data` |
| `appearance` | `appearance` |
| `bankPinManager` | `bank_pin` |
| `ironManMode` | `ironman` |
| `spellbook` | `Profile::spell_book` and the three pinned interface ids |
| `varps`, `configs` (legacy, read only) | the same varp maps as `core_data` |
| `statistics` | written empty; rows are converted and discarded when read, as in the original |

Reached only through `save::Host`. The last column says where
`persisted::host::Native` (next section) gets the answer:

| Save keys | Saving | Parsing | `Native` answers from |
| --- | --- | --- | --- |
| `quests` | points and stages as plain values | the object is handed over | `persisted::quests`, both ways |
| `autocastSpell` | book ordinal and spell id | book index is checked, the spell lookup is the host's | `persisted::spells`, both ways |
| `unlockedMusic` | ids in the host's order | each id is handed over for lookup and unlock | `persisted::music`, both ways |
| `familiarManager` | pet details, current pet or familiar, burden container | the object is handed over | `persisted::familiars` when saving; parsing stays a required service |
| `houseData` | ordinals, servant, rooms and hotspots | the object is handed over | `persisted::house` when saving; parsing stays a required service |
| `achievementDiaries` | per-diary flags | the array is handed over, or rewards are reset when the key is absent | `persisted::diaries`, both ways, including resetRewards |
| `emoteData` | ordinals when the host says a save is required | index is checked, contains and add are the host's | `persisted::emotes`, both ways |
| `grave_type` | not written by the saver | ordinal is checked and handed over with the attributes | `persisted::graves`; the AttributeSetEvent stays a required service |
| `pouches` | the host writes its own keys into the root | the array is handed over | `persisted::pouches`, both ways |
| `pohstorage` | the host supplies the object | the object is handed over | `persisted::poh_storage`, both ways |
| content hooks | each hook writes into the root | each hook receives the root | required services |

For these sections this module ports what the two files themselves do: key
names and put order, which values are stringified, the casts and index checks,
and the order of calls. The systems' own state lives in `player::persisted`,
not here. Every `Host` method is a required service and must not report
success for work it did not do. Iteration orders that belong to those systems'
own maps are supplied by the host, not guessed here.

## The persisted state behind `Host`

`player::persisted` holds, for each of these systems, what a character file
holds, a new character's values, and the original save and parse methods:

- `quests`: QuestRepository's HashMap from quest index to stage, the point
  total and the world's registry (a TreeMap in `Quests` order). A new
  character gets stage 0 for every quest registered at that moment. `parse`
  stores the points, reads the stages, then lets syncPoints recompute the
  points; a registered quest whose index the map lacks fails there. Also the
  stage and point mutators and the read methods around them; log lines name
  a quest by `Quests.toString()`, its display name.
- `music`: MusicPlayer's HashMap from list index to song, against the world's
  MusicEntry table.
- `emotes`: EmoteManager's list, the first 22 constants for a new character.
- `diaries`: the six diaries with three started and rewarded flags and three
  25-task bitsets each, the DiaryType rewards table, `parse` (the first key in
  HashMap order names the diary, read with `_` as `' ` and compared ignoring
  case), and resetRewards over the inventory, the active bank and the
  equipment. AchievementDiary's static `completedLevels` list, which every
  player's parse appends to, is the world's `completed_levels`.
- `spells`: the autocast spell as its own book and spell id, against the
  world's spell books.
- `graves`: the type is the saved attribute `/save:gravetype`; reading it
  unboxes any integral value as Kotlin's `Number.intValue()` does.
- `pouches`: the four pouches in declaration order with charges, current
  capacity and their essence containers; `parse` ends quietly at an unknown
  pouch id and keeps what it stored before a failure.
- `poh_storage`: one insertion-ordered id set and page per StorableFamily.
- `house` and `familiars`: the values the saver reads, with a new
  character's defaults (NOWHERE, BASIC_WOOD, crest NULL, no rooms, no servant;
  no pets or familiar).

`IntHashMap` keeps a Java `HashMap<Integer, V>`'s iteration for its insertion
order and table size, which `clear` keeps. `host::Persisted::new` builds a new
character's state; `host::World` holds the server-wide inputs (registered
quests, songs, spells, `completed_levels`), which are world start-up content
and the caller's to provide. `host::Native` implements `save::Host` from them
and forwards everything else to a `host::Remaining`: logging, retaliation, the
AttributeSetEvent dispatch, the container listeners resetRewards notifies, the
skill and settings services, FamiliarManager.parse, HouseManager.parse and the
content hooks. Those stay required services with no defaults. With them, a
brand-new character's whole file is produced from native state alone.

No gameplay of these systems is ported: quest content, music playback, emote
playing, diary tasks and rewards, spells, graves, pouch filling and decay,
storage interfaces, construction and summoning.

### Content hooks

The pinned server registers seven `PersistPlayer` classes (ClassScanner adds
every one it finds to both hook lists). None is ported; all stay behind
`Remaining`:

| Class | Root key it writes |
| --- | --- |
| TreasureTrailManager | `treasureTrails` (current trail, completed clues) |
| BrawlingGlovesManager | `brawlingGloves` (glove id and charges), only when it holds any |
| SlayerManager | `slayer` (removed tasks, streak, totals, flag words) |
| BlastFurnace | `bf-state`, only for a player with a furnace state |
| BarcrawlManager | `barCrawl` (started, bars visited) |
| GrandExchangeRecords | `ge-history` |
| JobManager | no key: it writes four `/save:jobs:*` attributes, which reach the file only on the next save because the hooks run after `saveAttributes` |

`parseFarming` and `parseMonitor` have empty bodies in the pinned source, and
`savePlayerMonitor` writes nothing.

## Behaviour kept from the original

- A `JSONObject` is a `HashMap`. Key order in the written text follows hash
  buckets, and keys sharing a bucket keep put order. The record libraries
  produce sorted maps, so `save/order.rs` restores each section's put order
  before the shared JSON-simple writer lays the object out.
- Numbers are written as strings, except `version` and a familiar's
  `lifepoints`, which the saver puts as integers.
- `acceptAid` is written from `Settings.isAcceptAid`, which is false for every
  ironman, including a null mode. A stored `true` is therefore lost on the next
  save of an ironman.
- The diary key is the `DiaryType` constant name, not its display name.
- `saveBankPinData` reads the clock once per comparison, and not at all for a
  pending delay of -1. A null status or ironman mode fails when its ordinal is
  read. `saveAppearance` fails on a null gender or body part.
- `populate` builds a new object, so a failure publishes nothing. The section
  methods write into the caller's root: `saveSavedData` leaves the earlier
  sections in place when a later one fails.
- `parseData` runs its sections in the original order and stops at the first
  failure, leaving every earlier change in place. Within a section the casts,
  conversions and writes keep their source order.
- `as JSONObject`, `as JSONArray` and `as String` fail with a null error for a
  missing or null value and a type error otherwise; `toString()` on a missing
  value reads as the text `null`, so a null `version` is a number error raised
  after the version was already reset to 0.
- `grave_type` is ignored unless it is a string. `configs` writes varps without
  setting save flags and skips index 1048 before converting its value.
  `varps` uses `VarpManager.parse`'s wrapping shift.
- An unknown `spellbook` id fails as a null error, because `forInterface`
  returns null and `setSpellBook` dereferences it.
- `runContentHooks` evaluates `saveFile!!` per hook: a parser that never
  loaded a file fails only when hooks exist, and `saveParsed` is then not set.
  With no file and no hooks it still sets `saveParsed`, which is how a new
  character becomes saveable.
- Files are read the way the JDK reads UTF-8 with replacement. This matches
  `String::from_utf8_lossy` except for encoded surrogates, which the JDK
  replaces once per three-byte sequence. A leading byte-order mark is not
  skipped and makes the document invalid, as it does for JSON-simple.

## Differences from the original

- **Java below 11.** The saver pretty-prints through a script engine when
  `ServerConstants.JAVA_VERSION` is below 11. Only the Java 11+ branch is
  ported; the oracle fixes the constant at 11.
- **Charset.** `FileReader` and `FileWriter` use the JVM default charset. The
  oracle runs with `file.encoding=UTF-8` and the native code always uses UTF-8.
  A reference server running with another default charset writes different
  bytes for non-ASCII text.
- **Unpaired UTF-16 escapes.** JSON-simple accepts `"\ud800"` and the saver
  would later write it as `?`. The shared native JSON reader rejects it, so
  such a file does not load. The same reader bounds text to 64 MiB and nesting
  to 256. `Error::category` reports these as `NativeLimit`.
- **Save path.** `PLAYER_SAVE_PATH` is nullable in the original and would be
  used as the text `null`. The native functions take a string.
- **I/O failures.** A missing file, a created directory, a replaced file, a
  save path that is a directory and a save directory that cannot be created
  are compared with the original, which swallows the write failure in both of
  the last two. Permission errors, full disks and partially written files are
  not compared.
- **Concurrency.** The saver wraps its work in `runBlocking` and
  `Dispatchers.IO`. The native functions are synchronous, and the oracle uses a
  same-thread stand-in for those three coroutine symbols. Two saves of one
  character at once are not examined.
- **Legacy attribute file.** Its failures are returned in `Loaded` instead of
  being printed. The limits of that loader are in [attributes.md](attributes.md).
- **Nested values read as text.** The record libraries receive `serde_json`
  values. An object or array placed where a scalar is expected is stringified
  with sorted keys rather than JSON-simple's hash order. No save written by the
  original contains one.
- **Pouch capacity.** A pouch container larger than 1,000,000 slots is refused
  by the native container factory (`Index`); the original would try to
  allocate it. A negative capacity fails as `NegativeArraySize` in both.
- **Grave type held as an object.** An object-valued `/save:gravetype`
  attribute is refused natively; the original would read a `Number` object
  through `intValue()`. Saved attributes cannot hold one.
- **World inputs.** Registered quests, songs and spells, and the static
  `completedLevels` list, are explicit world state passed in, not process
  globals. Which quests, songs and spells the pinned content registers is not
  part of these files and not checked here.

## Verification

```powershell
cargo test -p rs09-server --test player_save --test player_state
cargo xtask player-save-oracle --check --python <python> --java-home <jdk11>
```

`fixtures/player-save/` holds **2,637 original JVM cases with 5,291 steps**,
with source hashes. The oracle compiles `PlayerSaver.kt` and
`PlayerSaveParser.kt` unchanged with Kotlin 1.8.20 and runs them on JDK 11,
including `populate`, `save`, `parse`, `parseData` and `runContentHooks` and
real file I/O in temporary directories it owns. It first reproduces the timer
and inventory corpora, whose complete original classes it reuses.

Also unchanged and complete: `VarpManager`, `JSONUtils`, `PersistPlayer`,
`GraveType`, `GameAttributes`, `SavedData`, `ActivityData`, `QuestData`,
`GlobalData`, `IronmanManager`, `IronmanMode`, `SpellBookManager`, `BodyPart`,
`Gender`, `MusicEntry` and `PetDetails`.

Not complete: `Skills`, `Settings`, `Appearance` and `BankPinManager`
contribute their fields, `parse` methods and the getters the saver reads,
extracted verbatim; `BankPinManager`'s constructor and status enum bodies are
adapters. Their complete classes are compared in their own corpora. `Player`,
`Entity`, `Properties`, `EmoteManager`, the bank and equipment containers,
`Location` and `ServerConstants` are adapters built around verbatim members.
Quests, house, familiars, music player, diaries, pouches, POH storage, graves,
autocast spells and content hooks are recording adapters that hold getter
values and record what the parser hands them. `Emotes` and `DiaryType` are
generated with the original constant names; the generator fails if their
count changes.

Each step records the action's result or failure category, the written text or
file, every call into an unported system with the JSON it received, clock
reads, log lines, timer events, and each profile section that changed, read
through the original accessors. The first observation of a case is relative to
the state after setup unless the case asks for all of it.

The corpus covers new and default profiles; every saver and parser section on
its own; a populated profile; each top-level key absent, null or of each wrong
type; nested keys absent or wrong; numeric and index boundaries; strings with
quotes, control characters, U+2028, non-BMP text and JSON-simple's lenient
escapes; attribute types and expiry at the clock boundary; legacy skill rows,
the experience squish and its `permadeath` exemption; PIN delays against the
clock; a populated save truncated at 78 offsets; byte-order marks, invalid UTF-8
and surrounding text; failures raised by each unported system and by timer
callbacks; a save path that is a directory or cannot be created; random
profiles; and 40 texts written by the original loaded back into empty profiles
and written again. 39 of those come back byte for byte. The other differs in
one value: a persistent timer loaded with `ticksLeft` 9 is written back as 0,
because the save happens before the timer manager has processed a tick. The
original does this too; the timer rules are in [core-profiles.md](core-profiles.md).

### The persisted state cases

**267 more original cases with 2,233 steps** follow those 2,637 in the same
fixture, each marked `"state": true`. They are appended: the first 2,637 lines
are byte for byte the earlier fixture, and `tests/player_state.rs` pins their
SHA-256. `oracle/player-save/state.py` compiles `PlayerSaver.kt` and
`PlayerSaveParser.kt` unchanged a second time, now over the complete unchanged
`QuestRepository`, `Quest`, `Quests`, `MusicPlayer`, `MusicEntry`,
`EmoteManager`, `AchievementDiaryManager`, `AchievementDiary`, `DiaryType`,
`DiaryLevel`, `Bar`, `PouchManager`, `StorageState`, `StorageContainer`,
`StorableFamily`, `SpellBookManager`, `Plugin`, `PluginManifest`, `PluginType`
and `CloseEvent`. Verbatim selected members: GraveController's attribute key
and its two accessors, ContentAPI's `setAttribute`, `getAttribute` and
`setVarp`, Entity's attribute methods, and HouseManager's persisted fields and
their accessors. `Emotes`, `HouseLocation`, `HousingStyle` and `CrestType`
are generated with the original constant names and order only, and
`org.rs09.consts.Items` constants take the number their name ends in. Familiar,
pet and house loading and the content hooks stay recording adapters; packets,
interfaces and the remaining ContentAPI functions are stand-ins outside the
driven paths, and container listeners record. `oracle/player-save/State.kt`
reads every observed value back through the original accessors, in the
original collections' order.

The cases cover, per system, a new character; values at their boundaries;
each HashMap's iteration under several insertion orders, shared buckets,
resizes and a clear that keeps the table; missing, null, extra and
wrong-typed keys, with Java's and Kotlin's different `toString` of null; the
static `completedLevels` list across two players; resetRewards with either
bank active and a failing listener; and populated characters written whole,
loaded into a new character and written again. Most important: **a brand-new
character's whole file**, for four registries (none, a large quest/song/spell
registry, two quests, and content hooks), written by the original and written
natively from native state alone, before and after loading it back. All 16 of
those texts and files are byte for byte identical
(`brand_new_character_file_matches_byte_for_byte`).

## Remaining work

- Call the module from the real login and logout paths, and replace or
  migrate the playable alpha's snapshot. Neither is started.
- The rest of the persisted state behind `Host`: HouseManager.parse (rooms
  from RoomProperties, configured for the style, hotspots by index, Servant
  parsing) and FamiliarManager.parse (the FAMILIARS registry, Pets, the v0/v1
  migrations), and the seven content hooks. Until each exists its section is
  carried, not implemented. The gameplay of every system behind `Host`,
  ported or not, is separate work.
- Bind `persisted::Persisted` and its `World` to the canonical player and to
  world start-up (the real quest, song and spell registration). Nothing does
  yet; the stand-ins in the corpus are oracle inputs.
- The varp maps here are `core_data`'s. The canonical player's save and load
  ([canonical-player.md](canonical-player.md)) project its variable store into
  them and write the result back; this module itself is unchanged.
- The Java-below-11 output, default-charset files and the I/O failure paths.
- A decision on write safety. The original replaces the file in place; a
  crash mid-write leaves a truncated save, which the parser then rejects.
- Complete-class oracles for `Skills`, `Settings`, `Appearance` and
  `BankPinManager` inside this composite, instead of selected members.
- No throughput or capacity measurement has been made.
