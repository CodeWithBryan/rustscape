# Native skills and experience rules

`server::player::skills` implements the pinned Skills state and rules for all 24
skills. It covers XP awards, level lookup, static/dynamic levels, healing/hits,
prayer, level drains/boosts, combat-level calculation, refresh/restore, selective
copying and the profile skill parse/save sections. Scalar getter/setter pairs
become native fields; methods with behavior retain their original state changes.

Entity and world operations use the explicit `Context` interface. Calls to
equipment, assists, perks, plugins, level-up handling, packets/events, news and
XP logging occur in the original order. Effect handlers receive mutable skills
so synchronous callbacks can affect later operations. Binding errors stop the
method while preserving its preceding mutations. Actual implementations of
those external systems still need to be connected; no success defaults hide
missing world bindings.

Compatibility includes:

- XP lookup retains fractional boundaries at `threshold - 1`, NaN behavior and
  the level-99 cap. XP-by-level retains Java's saturating floating-point-to-int
  conversion. Once the integer sum saturates, every later output is identical;
  the native table stops there instead of iterating through enormous levels.
- XP awards retain the original multiplier, assist translation boundary,
  equipment bonus/charge ordering, 200m announcements, tutorial limit, plugin
  calls, level-up actions, events and 200-tick logging. Reported awarded XP and
  accumulated XP retain their original behavior when a stored value is capped.
- Lifepoints, prayer and dynamic levels remain distinct. Methods that clamp,
  wrap, emit updates or leave the lifepoints-update flag unchanged preserve those
  differences. Maximum lifepoints rechecks the perk and overwrites its extra
  amount each time, including repeated checks during healing.
- Combat-level arithmetic retains signed intermediate overflow and truncation.
  NPCs use the bound definition's combat level. Summoning's separate combat
  contribution follows the original update rule.
- Profile loading uses row position, rather than skill ID, for hitpoints/prayer
  migration. Old versions, reordered/duplicate rows, failed conversions and
  legacy XP squishing keep their original mutation order. Presence of the
  `permadeath` attribute suppresses squishing, even with a null/false value.
- Copying transfers only the fields copied by the original method. Multiplier,
  milestones, XP-log snapshots, timestamps and update flags remain on the target.

## Reference tests

`fixtures/skills/` contains **1,565 original JVM cases** and source hashes.
The harness compiles Skills and the BrawlingGloves enum unchanged, plus the
verbatim profile parse/save methods and assist skill-index mapping. Controlled
entity, clock, equipment, glove-manager, assist, perk and content/I/O adapters
record effects and allow synchronous mutations and failure injection.

Tests compare complete skill state, floating-point bits, packet/event request
order and observed values, callbacks, failures, totals and saved JSON. They
cover every XP level boundary, extended saturated XP-by-level results, signed
extremes, player/NPC/other entity branches and partial profile updates.

```powershell
cargo test -p rs09-server --test skills
cargo xtask skills-oracle --check --java-home PATH_TO_JDK_11
```

## XP lookup measurement

The Rust implementation builds an immutable curve once and uses binary search.
The original method recomputes the curve with Math.pow on every call. A release
benchmark compares both with identical inputs and matching rolling checksums:

```powershell
cargo run --release -p xtask -- skills-bench --java-home PATH_TO_JDK_11
```

The recorded run on this Windows machine measured medians of **207.40 ms JVM**
and **2.12 ms Rust** for 262,144 lookups, about **98.05×** faster for this lookup.
It uses 1,024 deterministic queries across broad XP ranges and level boundaries,
128 warmup passes, 256 passes per measured sample and seven samples. Inputs,
raw timings, checksums, toolchain/CPU details and source hashes are retained in
`parity/skills-benchmark.json`.

The measurement includes the checksum loop and excludes compilation, process
startup, input construction, entity setup and curve/JIT warmup. It measures a
single-threaded lookup, not full XP-award handling, world tick speed or player
capacity.

## Remaining integration

The context boundary does not replace assist sessions, glove charge management,
equipment/item definitions, skillcape perks, XP plugins, level-up UI, actual
packet encoding or event dispatch. Complete profile/file loading and the live
player/world lifecycle remain pending. Configured login still stops at the
existing metadata boundary. No whole-source-file or playable-login replacement
is claimed by these component tests.
