# Player variables and varbit definitions

The native implementation is `server/definitions/varbit.rs`,
`server/player/variables.rs` and `server/player/owned/variables.rs`. It ports the
pinned server's VarbitDefinition, VarpManager and CustomVarbitParser algorithms,
plus the ContentAPI variable methods. It uses primitive value/save maps owned by
the canonical player and a shared definition registry owned by the world.
Runtime code does not invoke Java.

`fixtures/variables/oracle.jsonl` contains 474 scenarios produced by compiling the
unchanged original classes, selected unchanged ContentAPI and PacketDispatch
methods, and complete original Config/VarcUpdate/IoBuffer writers with Kotlin
1.8.20 and JDK11. The native differential replay compares every action result,
error category, completed state mutation, map/definition/reader alias, callback
prefix and packet byte. `provenance.json` pins those sources and compiler inputs.
Player session/artificial/counter services, zero ISAAC output and cache index22
are explicit recording adapters. Real public HashMap fields are used, without
invented getter callbacks. Loader files are isolated per load: retained original
FileReaders prevent deletion on Windows, so reusing one path would fail to test
an actually missing file.

Behavior preserved includes signed widths and wrapping shifts, inclusive masks,
cache miss defaults, malformed decode before publication, mapping replacement
with retained old definitions, and `getValue` looking up the current definition
again by ID. A varbit update mutates its underlying varp and sends Config; it
does not use the standalone Varbit packet. The optional boolean is a save flag.
Setting a varp still accesses the save map when that flag is false. A true save
flag is sticky until unsaveVarp removes it. State writes precede packet failures.
Varc IDs narrow to signed shorts; its counter increments before session writes.

Reinitialization retains the original map and integer HashMap iteration order,
including resizing, tree bins and table capacity retained after clear. Existing
value replacement is nonstructural. Structural mutations fail at the next
iterator advance; changing the last entry does not cause an extra trailing check.
Legacy VarpManager parsing writes completed rows in order without packets. Its
save method remains the original no-op, including the nonnull argument check.

The original custom-definition file is preserved verbatim with Git blob
`720b1676e05e753655d3afb0c00aba1e1fb117a0`, also recorded in provenance.
It supplies the bank insert/note definitions rather
than inferred cache metadata. The text loader publishes its reader after open
and preserves completed definition rows on later failures.

Limits remain explicit: native text parsing uses bounded UTF8 input, not full
FileReader cursor, default-charset or read-ahead semantics. Definition mapping
snapshots are sorted diagnostics, not proof of arbitrary exposed HashMap view
iteration. Native getMask rejects endBit INT_MAX and ranges exceeding one million
steps, for which the original can run forever or for impractical durations.
Canonical variable services target human-player frames; full original AIPlayer,
session/cipher/repository behavior, full profiles, startup and live banking still
require separate integration and evidence. No throughput or player-limit claim
follows from these tests.

Four canonical regressions cover shared interface-counter wrapping, real custom
bank definitions, partial mutations on null save maps/disconnected delivery,
foreign-world rejection before entry, retained managers/views after wire-index
reuse and final collection with detached primitive maps. Both controlled live
example targets exercise original chatbox opening through the actual canonical
varp store. Their runtime loads archive22 and both pinned custom definitions
before admission; UI metadata no longer substitutes an outgoing frame for a
player-state mutation. This is still a development admission path.

Reproduce with `cargo test -p rs09-server --test variables` and
`cargo xtask variables-oracle --check --java-home PATH_TO_JDK11`.
