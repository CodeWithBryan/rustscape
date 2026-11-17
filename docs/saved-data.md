# Character activity, quest and global records

`server::player::saved_data` provides native records for the pinned ActivityData,
QuestData and GlobalData classes. The port includes their defaults, ordered JSON
parsers, scalar/array fields and nontrivial counter/quest/query behavior. Rust
field access replaces Java's simple getter/setter pairs. `SavedData` implements
the corresponding PlayerSaveParser.parseSavedData and PlayerSaver.saveSavedData
sections, retaining activity → quest → global ordering.

This covers activity points, barrows/arena/Bork state, quest flags and Desert
Treasure item quantities, global cooldowns, assist experience, travel logs, god
books/pages, charges, task counters and the other fields in those records.
Fields absent from the original save methods remain absent: this includes the
saved-location coordinates and some perk switches. The Rust port preserves this
behavior instead of silently changing the save format.

Parsing mutates the existing records in source order. Short arrays keep their
unwritten tails; oversized arrays fail after their earlier elements were
assigned. Invalid later fields retain earlier scalar/array changes. The outer
parser obtains all three section objects before mutating the first record.
Saving publishes each complete section in turn and retains earlier sections if
a later one fails, such as a null quest item.

Counters retain signed Java widths and overflow. Pest points and barrow kills
retain their original method-specific caps; loading bypasses those setter caps.
Quest name helpers retain the source's string-identity comparisons via the
explicit `QuestName::Literal`/`Text` distinction. Cooldown queries use an explicit
clock value. Indexed methods return a native error where Java throws.

The stored item representation covers ID packing and amount construction used
by these sections; item definitions, plugins and interactions remain separate
pending systems. Arrays use owned Rust storage and borrowing; external Java
object/array aliasing is not established by these fixtures.

## Numeric save fidelity

The JDK 11 Double formatter differs from Rust's default representation on
notation thresholds and some rounding boundaries. Native `game_core::java_double`
preserves the legacy digit and rounding rules, negative zero, nonfinite values,
and decimal/exponent output. Parsing also handles Java's suffixes, ASCII trimming
and hexadecimal literals with ties-to-even rounding. Assist experience therefore
retains its original saved strings and floating-point bits. Integer JSON parsing
also preserves Java signed widths and BMP decimal digits.

The formatter's separate OpenJDK-derived license and Classpath exception are
retained in THIRD_PARTY_NOTICES.md and third-party/. Runtime execution uses Rust;
the JDK only generates differential test expectations.

## Verification and remaining integration

`fixtures/saved-data/` records **14,670 JVM cases**, with source hashes. The
oracle compiles all four original record classes unchanged and extracts the
relevant save/parser methods verbatim. Player association, clocks, god-book
ordinals and item fields are explicit adapters. Cases cover defaults, malformed
and partial loads, array lengths, counter overflow, quest identity, item/null
failures, save round trips, floating-point exponent/power-of-ten boundaries,
random bit patterns and hexadecimal parsing. Comparisons cover record state,
return/error outcomes and JSON sections, rather than whole-file JSON key order.

```powershell
cargo test -p rs09-server --test saved_data
cargo xtask saved-data-oracle --check --java-home PATH_TO_JDK_11
```

`tools/port_saved_data.py` is an offline authoring aid for the explicit Rust
fields and sequential parsers; it is not used by runtime loading. Its output is
checked against independently executed JVM code.

The full profile parser, inventory/bank/equipment, attribute/skill integration, settings,
quest repository, appearance, managers/content hooks, file saving and world
initialization still need their remaining ports. These records are a supporting
library for that integration; configured login still stops at the existing
metadata boundary. No complete profile, playable login or whole-source-file
replacement is claimed by this milestone.

The supporting attribute store and its profile JSON/XML paths are described in
[attributes.md](attributes.md).
Skill state/rules and profile sections are covered in [skills.md](skills.md).
