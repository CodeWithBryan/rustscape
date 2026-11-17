# Native server scenery definitions

`server::definitions::scenery` implements the pinned server's scenery cache
decoder, ordered registry loading/configuration, memoized lookup, child selection
and action/options queries. It stores all original instance fields, including
currently unused fields, and retains shared arrays, handler maps and definitions.
Runtime parsing uses Rust; schema authoring and JVM comparison are offline tools.

The decoder preserves signed-byte strings, signed colours/offsets, model-list
precedence, discarded opcodes, original stop/log behavior, hidden-option handling,
the opcode-81 arithmetic quirk and the original 31017/29292 postprocessing. Empty
data logs and configures the record; a truncated field throws before publication.
Repeated model lists skip their payload without overwriting the first list.

Registry loads publish each completed record immediately. Missing files publish
a fresh constructor-default record with ID zero. A missing `for_id` instead
inserts a persistent default whose ID is the requested key. Configuration writes
its varbit configuration ID into every child, including the -1 fallback. This
can create forward placeholders or mutate earlier records. Later file loads can
replace those records while retained references keep their original objects.
Self-child configuration and reconfiguration preserve these identity effects.

A scenery factory must retain the exact definition returned during construction,
use the requested scenery ID even when a missing cache record's own ID is zero,
and capture the node name while reading later actions from the retained definition.
Looking up the registry again after a reload changes the original lifetime
behavior. Full scenery construction, including morph child/wrapper ownership,
remains part of world integration rather than this definition library.

Child selection preserves negative/out-of-range indexes, -1 children, the final
fallback and parent fallback. Null-player selection uses index zero. Player
varbit lookup takes precedence over varp lookup; these values remain explicit
native player bindings. Selected children receive the parent's current varbit
configuration ID. Action matching uses the existing JDK-compatible UTF-16 case
comparison; options exclude exact `null` and optionally exact `Examine` values.

## Verification

```powershell
cargo test -p rs09-server --test scenery_definitions
cargo xtask scenery-definition-oracle --check --python <python> --java-home <jdk11>
python tools/port_scenery_schema.py --check
cargo run --release -p xtask -- scenery-definition-audit --python <python> --java-home <jdk11>
```

The 1,940 differential scenarios include 181 pinned raw cache samples. They
compare all instance fields, read positions, logs, partial loads/errors,
nullable options, shared arrays/maps, retained definitions and child queries.
The oracle compiles complete unchanged SceneryDefinition, Definition and
ByteBufferUtils with explicit cache/count, logging and player-value adapters.
Fixtures pin original and harness hashes. The full archive audit checks the
public strict native archive-16 loader against the original bulk parser and
retains its exact input, observations and source/build fingerprints under
`runs/scenery-definition-oracle`. Its report is evidence only after success.

The recorded whole-data audit passed for all 42,005 definitions, 42,004 present
files and 42,006 registry entries, with zero field or alias differences and no
decoder logs. One missing file retains the original default-ID behavior; the
extra -1 registry entry comes from child configuration. It compared 1,190,913
decoded input bytes. The report and 80 MB original observation retain their own
source/build fingerprints; later unrelated world/client work does not become
part of that historical build automatically.

The harness streams JSON output with JSON-simple's `writeJSONString`. Its
`toJSONString` implementation copies the complete prefix after every map entry,
which caused the first full audit to hit the harness time limit. Switching only
harness output preserved all 1,940 portable observations byte-for-byte. The
full diagnostic run allowed an 8 GiB JVM heap; this is an offline oracle setting,
not a native runtime requirement or a game performance measurement.

`scenery-definition-samples` authors a fresh cache-sample input file after
verifying the pinned asset manifest. Regenerate and review the fixtures after
using that command. The schema checker validates authored Rust fields/defaults;
it does not replace the independent JVM comparisons.

Base examine-text caching, concrete option-handler execution/default registration,
the varbit cache parser and arbitrary Java collection/reflection APIs remain
separate ports. Native opaque Object fields accept observation data, not arbitrary
JVM objects. Child/player/logging bindings do not establish content configuration,
complete Scenery/Node behavior, region loading, routing or playable world entry.
The world landscape layer now accepts these same shared native definition
handles. Native scenery constructors and the owning scene now bind each scenery
to its constructor-retained definition; see [scene.md](scene.md). Player/content
services and playable world entry remain pending.
