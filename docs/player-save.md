# The original character save file

`server::player::save` ports the two pinned files that write and read a whole
character: `PlayerSaver.kt` and `PlayerSaveParser.kt`. It produces the JSON text
the original writes and loads what the original writes, using the native
profile libraries that already exist for the individual sections.

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

Reached only through `save::Host`, because no native port of the owning system
exists:

| Save keys | Saving | Parsing |
| --- | --- | --- |
| `quests` | points and stages as plain values | the object is handed over |
| `autocastSpell` | book ordinal and spell id | book index is checked, the spell lookup is the host's |
| `unlockedMusic` | ids in the host's order | each id is handed over for lookup and unlock |
| `familiarManager` | pet details, current pet or familiar, burden container | the object is handed over |
| `houseData` | ordinals, servant, rooms and hotspots | the object is handed over |
| `achievementDiaries` | per-diary flags | the array is handed over, or rewards are reset when the key is absent |
| `emoteData` | ordinals when the host says a save is required | index is checked, contains and add are the host's |
| `grave_type` | not written by the saver | ordinal is checked and handed over |
| `pouches` | the host writes its own keys into the root | the array is handed over |
| `pohstorage` | the host supplies the object | the object is handed over |
| content hooks | each hook writes into the root | each hook receives the root |

For these sections the module ports what the two files themselves do: key
names and put order, which values are stringified, the casts and index checks,
and the order of calls. It does not port quest, construction, summoning,
music, diary, emote, spell, grave, runecrafting-pouch or storage behaviour.
Every `Host` method is a required service and must not report success for work
it did not do. Iteration orders that belong to those systems' own maps are
supplied by the host, not guessed here.

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

## Verification

```powershell
cargo test -p rs09-server --test player_save
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

## Remaining work

- Call the module from the real login and logout paths, and replace or
  migrate the playable alpha's snapshot. Neither is started.
- Native ports of the systems behind `Host`. Until each exists its section is
  carried, not implemented.
- The varp maps here are `core_data`'s. The canonical player's variable store
  in `player/owned/variables.rs` is a separate object and is not yet joined.
- The Java-below-11 output, default-charset files and the I/O failure paths.
- A decision on write safety. The original replaces the file in place; a
  crash mid-write leaves a truncated save, which the parser then rejects.
- Complete-class oracles for `Skills`, `Settings`, `Appearance` and
  `BankPinManager` inside this composite, instead of selected members.
- No throughput or capacity measurement has been made.
