# Native server NPC definitions and configuration

`definitions/npc` ports the pinned server's cache decoding, lazy definition
lookup, child selection and option queries. Its configuration parser uses the
original JSON-simple DOM behavior and JSONObject field order. The runtime reads
cache bytes and UTF-8 configuration in Rust; Java is a test oracle only.

The portable corpus compares 1,749 scenarios against complete unchanged
NPCDefinition.java, Definition.java, NPCConfigParser.kt, ByteBufferUtils.kt,
Animation.java and Graphics.java. Cache, player/varbit, drop-table construction,
enum-value metadata and world services are explicit oracle bindings. All stored
scalar, string, array and handler fields are observed except the opaque
dropTables object. Outer array/map/definition identities are compared as well.

The full-data audit covers every lookup slot through the final archive18 file:
8,591 slots, 8,590 present files and the missing-file default at ID3813. It compares
original and native fields/aliases both before and after all 8,033 pinned
configuration rows. The public archive loader is independently compared with
lazy file-input lookups and the production configuration parser.

## Behavior that matters for gameplay

- Lookups preserve cached definition identity, including after the source cache
  changes. A missing file produces the original constructor defaults. Failed
  decoding leaves an uncached lookup; parsing an existing object retains its
  partial mutations. Array replacement and per-element publication match Java.
- Server NPC options retain strings such as Hidden. Animation fields use signed
  shorts, renderAnimationId initially equals zero, and opcode126 falls through
  to the opcode127 read. Unknown opcodes consume no extra data. Signed-byte
  strings use the original UTF-16 conversion. These contracts differ from the
  visual client's NPC definition decoder; they must not be silently unified.
- Child lookup keeps the original parent when the selected/default child is -1.
  A missing varbit service object fails rather than choosing a fabricated value.
  The callback can resolve a different child through the lazy definition registry.
- Configured name and examine values are handler entries; loading configuration
  does not overwrite the raw definition's name. Repeated rows overwrite only
  assigned keys, and failures retain earlier writes. Array entries are parsed
  without trimming. Empty and literal null text are skipped. Unknown populated
  keys log in original JSONObject iteration order and can fail the load.
- Animations have explicit native constructor state and identity. Combat styles
  and clue levels are native typed enum values in their original declaration
  order. These are metadata values; their combat or treasure behavior is separate.
- Combat animation selection preserves the index/key mapping. Combat graphic
  initialization reads IDs from its argument map and heights from the definition's
  handlers, replacing only present entries and preserving earlier writes on errors.

The generated field schema is authored from the pinned declarations/constructor
by tools/port_npc_schema.py and checked after rustfmt. Runtime decoding is ordinary
Rust code. Serialized field maps and tagged properties are observations; they
are not a new game save format or arbitrary Java-object serialization contract.

## Reproduction and remaining work

```powershell
cargo test -p rs09-server --test npc_definitions --test item_config
cargo xtask npc-definition-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
python tools/port_npc_schema.py --check
cargo run --release -p xtask -- npc-definition-audit --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

Malformed/excessive native input is bounded. File readers use explicit UTF-8;
arbitrary platform-default encodings and unpaired UTF-16 text remain outside the
native text contract. The native parser retains its current reader after a failed
load; its owned file resources are released with their Rust owner.

A definition now also holds its drop tables (`drop_tables`, the original
`dropTables`: a table of its own in a new definition, replaceable by null).
The field is not serialized, so the stored-field observations and the audit
are unchanged and still exclude it; what it holds is verified separately, see
[drop-tables.md](drop-tables.md) and [npc-death.md](npc-death.md).
`tools/port_npc_schema.py` authors the field; its changed source hash is the
one line that differs in `fixtures/npc-definitions/provenance.json`.

Remaining class/world work includes concrete default/registered
option handlers and invocation, examine/default-locale helpers, the remaining
varbit getter methods/cache implementation, complete NPC/Entity methods and
factory/init service bindings, behavior subclasses, combat and original live
repository/chunk/region startup. The base initialization kernel and controlled
NPC session are described in [npcs.md](npcs.md).
The metadata audit does not establish any of those behaviors or player capacity.
