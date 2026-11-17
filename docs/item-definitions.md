# Native server item-cache definitions

`server::definitions::item` loads the original archive-19 item records and applies
the pinned server's note, lending and recolour transfers. It implements stored
definition fields, constructor defaults, raw decoding, registry lookup/loading,
ascending template resolution, equipment-ID assignment and the selected
stackability/action/special-bar/quest/archive/file helpers.

This is the definition data layer before SQL/content configuration. It does not
replace the full ItemDefinition class, equipment, shops, plugins, item
requirements/Entrana decisions, or a playable world. Native item/container state
libraries now have separate coverage in [inventory.md](inventory.md). The existing
saved-data ItemData representation remains a separate record adapter.

## Preserved behavior

- Unknown opcodes stop decoding and leave trailing bytes untouched. Opcode 10
  consumes no operands. All original signed/unsigned field widths and per-read
  positions are preserved, including four individual reads for the value field
  and three individual reads for a client-script key.
- Strings retain the Kotlin decoder's signed-byte-to-char conversion. They are
  not reinterpreted as UTF-8 or CP1252. Empty and repeated arrays, duplicate script
  keys, overwritten actions and null defaults remain distinct.
- Missing lookup returns a fresh, unregistered default. A missing cache file is
  inserted under its requested key with a default internal ID of zero. A failed
  load keeps earlier inserts and does not run the final template pass. Existing
  out-of-range registry entries remain present. The ID-14958 stackability override
  is tested separately because that ID is beyond the pinned archive's size.
- Templates run by numeric ID and in note/lend/recolour order, without recursive
  dependency sorting. They retain the original selective field transfers,
  including repeated assignments and omitted note offset/texture fields.
- Definition handles and mutable arrays/maps preserve aliases. Lending and
  recolouring clone the options array while sharing the other transferred arrays
  and script map. Per-field assignment order preserves self-reference behavior;
  copying an initial whole-record snapshot would change some of these cases.
- Equipment IDs use the original male-model test, ascending assignment and
  ID-2513 override from ID 3140. No gameplay or equipment-slot inference is added.
- Action comparisons use JDK 11's UTF-16 case behavior. The native case table is
  derived from public Character API observations over all 65,536 code units,
  with actual String.equalsIgnoreCase regression cases. Supplementary letters
  retain this JDK's separate-surrogate behavior. Runtime comparison uses Rust and
  checked-in data, independent of the host's current Unicode tables.

Shared handles use Rust Arc/RwLock. Template loading is ordered initialization,
not a parallel simulation algorithm; these checks make no parallel-world claim.
Content handlers now retain typed native scalar, array, requirements, animation
and audio values through the separate [configuration loader](item-configuration.md).
Arbitrary Java plugin objects and their reference identity remain outside this
data layer. Scalar getters/setters are native fields. No measured performance
claim is made for this loader.
Script/requirements map contents and aliases are covered. These helpers do not
observe map iteration. Configured requirement maps separately preserve JDK
integer-map iteration; actual gameplay requirement checks remain pending.

## Differential tests

The committed corpus contains **1,831 original JVM observations**. It includes
87 compact real-cache byte samples, every opcode, numeric boundaries, truncated
payloads, repeated commands, complete mutable state/identity observations, failed
loads, missing records, self-references and helper methods. Five native tests
also verify provenance, JDK casing and rejection before mutation when the archive
record count exceeds its caller-selected cap.

The oracle compiles complete, unchanged Definition.java and ByteBufferUtils.kt.
Selected ItemDefinition field declarations, constructor and methods are compiled
verbatim. The cache adapter supplies isolated file bytes/counts; its item-count
method is also copied verbatim. Numeric TRADEABLE configuration is an explicit
adapter. The harness records every source hash and never modifies the reference.

```powershell
cargo test -p rs09-server --test item_definitions
cargo xtask item-oracle --check --java-home PATH_TO_JDK_11
python tools/port_jdk_case.py --check
```

`tools/port_jdk_case.py` is an offline authoring tool for the reviewed lookup
data. It never runs in a native server or client. Ordinary native tests use the
committed fixtures and require no Java, Python, reference checkout or full cache.

## Complete pinned archive audit

```powershell
cargo xtask cache-fetch
cargo run --release -p xtask -- cache-audit --jvm
cargo run --release -p xtask -- item-audit --java-home PATH_TO_JDK_11
```

The item audit verifies asset hashes, opens the strict native archive loader and
compares all loaded field values and mutable identities with the original
parse/template methods. Its input bytes come from the native JS5 layer, whose
extraction has a separate whole-cache comparison against original RT4 classes.
Both the archive entry point and direct file-parser entry point are checked.

The recorded archive contains **14,659 definitions**: 14,654 present files,
five missing/default records and 583,421 input bytes. It resolves 3,329 note and
220 lending templates with zero observed field or alias differences. No recolour
templates occur in these assets; synthetic cases verify that transfer method.
Build/source/asset identities and observation hashes are retained in
`parity/item-definitions-audit.json`; detailed temporary observations stay under
`runs/item-oracle/`. This audit is a correctness check, not a benchmark.

To intentionally refresh compact input samples from the verified pinned assets:

```powershell
cargo xtask item-samples
cargo xtask item-oracle --java-home PATH_TO_JDK_11
```

Review sample and fixture/provenance changes together. `--check` never rewrites
fixtures. Full archive loading enforces a caller-selected record limit and rejects
empty or overflowing index capacity; these native errors are deliberate safety
differences outside the valid pinned cache. No client decompression fallback is
enabled in this server loader.

## Next integration work

Connect the native item configuration layer and port inventory/bank/equipment
subclasses and profile sections. Connect the tested Item/Container state rules to the player
lifecycle, actual content handlers and packet encoders. Complete profile loading
and world entry remain separate acceptance gates.
