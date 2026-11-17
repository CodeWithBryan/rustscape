# NPC drop tables

`world/drop_tables` ports the pinned server's drop-table model, its
configuration loader, the shared tables a slot can hand off to, and the roll
with its original random-draw order. A roll ends with the list of items the
original would construct. Turning that list into drops (`drop`, `createDrop`,
loot share, the bone crusher, drop messages, the ground item) and the NPC
death that leads there are in [npc-death.md](npc-death.md); both are
libraries that no session installs. Java is a test oracle; nothing here runs
it.

| Original | Rust | Scope |
| --- | --- | --- |
| `WeightBasedTable.kt`, `WeightedItem.kt`, `NPCDropTable.kt` | `world/drop_tables.rs` | add, both roll overloads, `convertWeightedItems`, `create`, the ten `insert*` helpers, `excluding`, `canRoll`, `toString` |
| `NPCDropTables.java` | `world/drop_tables.rs` (`NpcDropTables`) | the table holder, `roll`, `getStabilizerRatio`, `modRate`, `DROP_RATES`, `MESSAGE_NPCS`. `drop`, `createDrop`, `getLooter`, `sendDropMessage` and `handleBoneCrusher` are in `world/drop_tables/drop.rs`, see [npc-death.md](npc-death.md) |
| `NPCDefinition.dropTables` | `definitions/npc` (`drop_tables`), `world/drop_tables/definitions.rs` | the field on native NPC definitions and the loader's assignment onto them |
| `DropFrequency.java` | `world/drop_tables.rs` | the five constants and `rate` |
| `DropTableParser.kt` | `world/drop_tables/parser.rs` | `load` over the file's text, and over a file read as UTF-8 |
| `RareDropTable.kt` and the six `*DropTable.java`/`CELEMinorTable.java` | `world/drop_tables/shared.rs` | `parse`, `startup`, `retrieve`, and `RandomFunction.rollWeightedChanceTable` |
| `RandomFunction.java` (`random(int)`, `random(int, int)`, `randomDouble(double)`, `nextBool`) | `world/drop_tables/random.rs` | plus the JDK 11 `ThreadLocalRandom` double and `Collections.shuffle` |

`ChanceItem.java` and `RandomFunction`'s chance-item methods compile in the
oracle because `RandomFunction` names them; they are not ported.

## Evidence

`cargo xtask drop-tables-oracle` compiles the classes above unchanged with
Kotlin 1.8.20 and runs them on JDK 11. The corpus has 510 cases with 8,386
actions, 4,695 of them rolls producing 17,897 items. Across it the original
advances `RandomFunction.RANDOM` 91,842 steps, the shuffle generator 33,354
steps and the thread-local generator 14,844 draws. It holds:

- the complete pinned `drop_tables.json` (444 rows, 2,152 NPC ids, 9,414
  entries) loaded under two seeds, with every table recorded as the original
  holds it: stored order, total weight bits, guaranteed, charm and tertiary
  entries, and which NPC ids share one table object;
- two rolls of every one of the 2,152 NPC ids (three times, then once) through
  `NPCDropTables.roll`, with the seven pinned shared tables loaded and seven
  kinds of receiver in rotation: 4,304 rolls;
- designed cases for the parser, table construction, every slot hand-off, the
  shared-table files and their start-up, and draws placed on, just below and
  just above every cumulative weight boundary. The rare drop table's guaranteed
  list, which its own loader never fills, is reached by calling the original
  `add` on the private table through reflection.

Each case seeds the three generators the original draws from, and each action
records how far each one advanced. The native replay must produce the same
value, error category and service calls for every action, must leave each
generator where the recorded counts put it, and must end each case with the
same next value from all three. The fixture also pins the eight data files by
hash and every compiled source file by hash.

`crates/server/tests/drop_tables.rs` adds tests the JVM corpus cannot give:
rolls driven by a scripted list of numbers (each request must match the next
listed draw in kind and bound), the two native bounds, and the configuration
file read from disk.

## The three generators

The original draws from three unrelated generators. Native code takes them as
one `Source`; `JavaSource` is the JDK 11 implementation of each.

| Generator | Used for |
| --- | --- |
| `RandomFunction.RANDOM`, a `java.util.Random` shared by the whole server | insertion position of each weighted entry, item amounts, hand-off counts of the herb and uncommon-seed slots, the clue choice, the herb-slot coin, the weighted chance tables' pick |
| `ThreadLocalRandom.current()` | the one double of each weighted selection, in the main tables and in the rare drop table |
| the `java.util.Random` private to `Collections.shuffle(List)`, shared by the whole JVM | the rare drop table's order for one roll, the weighted chance tables' in-place shuffle |

The first and third are shared with every other caller in the original
process, so reproducing a whole server run needs every one of those callers in
the same order. This slice reproduces the draws of the drop path given the
generators' positions when it starts.

The pinned JDK (Temurin 11.0.32.1) computes the rounding correction of
`ThreadLocalRandom.nextDouble(origin, bound)` and discards it:
`if (r >= bound) Math.nextAfter(bound, origin);`. The double can therefore
equal the bound, and native code does the same. The corpus has draws where
that happens. Another JDK build may differ here; the oracle is this one.

## Behaviour that matters for gameplay

- Amount ranges are half open. `RandomFunction.random(min, max)` is
  `min + nextInt(max - min)`: a 5 to 50 entry gives 5 to 49. Equal ends draw
  nothing. Reversed and overflowing ranges follow Java's wrapping arithmetic.
- A weighted entry is stored at a random position: it swaps with the entry
  there, which moves to the end. The first one draws nothing; guaranteed
  entries are kept apart and never draw. The total weight is summed in
  insertion order, which is not the stored order.
- One table roll selects, for each requested time, every guaranteed entry and
  then one weighted entry. The weighted entry is the first whose running
  subtraction reaches `<= 0`. All selection is drawn before any item is made.
  Subtracting in stored order can leave a positive remainder where the total
  said there was none; that roll then selects nothing (one corpus case).
- A table with weighted entries and a total that is zero, negative or NaN
  throws on every roll. An empty table draws nothing.
- Only the herb slot (14424) and the uncommon-seed slot (14422) use their
  amount range as a number of hand-offs. Every other slot hands off once and
  ignores its range.
- An NPC table rolls its charm table for all times, then its tertiary table,
  then its main table; each is selected and converted before the next starts.
- The herb slot flips its coin before looking at the receiver. A player who
  wins it is asked about the four gloves in the order irit, avantoe, kwuarm,
  cadantine; the first one worn gives that herb and uses a charge. Everyone
  else, and every loser, rolls the herb table.
- A clue slot with no clue registered for its level logs an error and gives
  nothing. The clue item has amount 1.
- The rare drop table keeps its "nothing" entries (item 0) in its total unless
  the receiver is a player for whom `shouldRemoveNothings` is true, but its
  walk always passes them by, so a draw beyond the other entries gives nothing.
  It draws the double before shuffling a copy of its entries. A table with one
  entry gives it without drawing. Only the first item of its result is used.
  It can hand off to any slot, itself included.
- The six weighted chance tables draw their pick, then shuffle themselves in
  place, and stay in that order for the next call. The walk compares
  `rand <= weight`, so the first entry after the shuffle wins one pick more
  than its weight.
- `DropTableParser.load` builds one table per row from `main`, `charm`,
  `tertiary` (used only if it is an array) and `default`, in that order, and
  gives that one table to every id in `ids`. A later row replaces an earlier
  one for the same id. `ids` is split on commas without trimming.
- A minimum above the maximum is the only failure the loader catches: the row
  is logged and dropped, and loading continues. The draws that row already made
  still shift every later table. Any other failure (a non-integer id or amount,
  a weight that `Double.parseDouble` refuses, a missing or mistyped list, a bad
  entry in `ids`) ends the whole load; rows before it keep their tables, and
  ids of the same row before the bad one keep theirs. The pinned file has no
  such row.
- A shared-table file adds every element written exactly as `item`, at any
  depth, in document order. An attribute that is missing or not an `int` stops
  the load there and keeps what was added. Loading again appends again.
  `startup` with a null path fails to parse and still logs success.
- `NPCDropTables.roll` passes the list to the NPC's behaviour callback and
  returns that same list. Without a looter it throws after the table has been
  rolled and its draws spent. `getStabilizerRatio` divides integers: 10 at
  combat level 0, -10 at -2, a division by zero at -1, 0 otherwise.

## Inputs for systems that are not ported

| Input | Stands for | Corpus values |
| --- | --- | --- |
| `Looter::None`, `Entity`, `Player` | the `receiver` entity | all three for every slot |
| `PlayerLooter::removes_nothings` | `core.api.shouldRemoveNothings` (ring of wealth) | true and false |
| `PlayerLooter::in_equipment` | `core.api.inEquipment` for the four gloves | none, each one, several |
| `PlayerLooter::update_glove_charges` | `FOGGlovesManager.updateCharges` | recorded call |
| `Host::clue_scrolls` | the clue plugins registered per level | absent, empty, one, several |
| `Host::log`, `Host::stack_trace` | `core.api.log`, a printed stack trace | recorded |
| `Definitions::assign`, `name` | `NPCDefinition.forId(id).dropTables` and `.name` | present, null drop tables, null definition, failing lookup, null name |
| `on_rolled` of `NpcDropTables::roll` | `npc.behavior.onDropTableRolled` | no change, remove, add, failure |
| `Inventory` | `player.inventory` in `canRoll` | all four answers |
| the `name` function of `describe` | `ItemDefinition.forId(id).name` | text, empty, null |

The oracle's adapters for these record the call and return the case's answer.
None of them is answered from native equipment or clue state. The 510 cases
use the corpus' own `Definitions`; `NativeDefinitions` gives the loader the
native NPC definitions instead, and `crates/server/tests/npc_death.rs` checks
that the complete pinned file loaded onto them holds, for all 2,152 ids, the
tables and the sharing this corpus recorded from the original.

## Differences from the original

- `RolledItem` is the pair of arguments of `Item(id, amount)`. The original
  `Item`, its definition lookup and its 16-bit id are not joined; the oracle's
  `Item` keeps its constructor arguments.
- Two native bounds replace unbounded original work: at most 65,536 weighted
  items per roll (`TooManyItems`), and at most 64 nested rare-table hand-offs
  (`RareDepth`) where the original overflows the JVM stack. The corpus has
  rare tables that hand off to themselves and end; it has none that cannot.
- `WeightedItem` is a value. The original shares one mutable object between a
  table and its `excluding` copy. A loaded table is an immutable
  `Arc<NpcDropTable>`: ids of one row share it, as in the original, but nothing
  mutates it in place.
- Shared-table files are parsed with `roxmltree` from UTF-8 text, not with the
  JDK's parser from a URI. The seven pinned files and 33 designed documents
  agree, including an internal DTD entity. Other encodings, external entities
  and URI resolution of the path are not claimed.
- The configuration file is read as UTF-8 (the pinned file is ASCII). The
  loader's `reader` and `parser` fields and the JVM's default charset are not
  modelled.
- The logged text for a refused row is the exception's first line; the
  original appends its stack trace.
- `ThreadLocalRandom` belongs to a thread in the original. Native code takes
  one explicitly.

## Reproduction

```powershell
cargo test -p rs09-server --test drop_tables
cargo xtask drop-tables-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

The whole pinned configuration is in the portable corpus, so there is no
separate whole-data audit command.

## Remaining work

The roll is joined to the native drop and death libraries
([npc-death.md](npc-death.md)), and those to the native ground manager, pulse
manager and NPC definitions, but to no session. Still required: binding the
death and drop services to a live world; the behaviour callbacks that edit a
rolled list; the clue registry, ring-of-wealth and glove answers from real
player state, with glove charges; the order in which the original starts its
loaders, which decides where the shared generator stands when each table is
built; and the other `WeightBasedTable` users in content (pickpocketing,
implings, caskets).
