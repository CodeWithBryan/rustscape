# Native item configuration

`server::definitions::item_config` implements the pinned ItemConfigParser's
loading and conversion rules. `definitions::property` stores the resulting
runtime value types. The native server needs no JVM to apply configuration data.

The complete original loader compiles unchanged in the oracle. Its 43 public
string constants are also authored into native code and checked against JVM
observations. The loader is still separate from server world startup; actual
equipment requirements, plugins, item interactions and profile/world wiring
remain pending. No complete-game or performance claim is made here.

## Conversion and mutation rules

Fields retain their original scalar types: integer, boolean, string and double.
Weight preserves f64 bits, including negative zero and nonfinite input. Numeric
conversions use the pinned JDK-compatible routines rather than locale rules.

Bonuses/absorb use shared integer arrays. Requirements use a shared integer map
with original insertion/resize/tree-bin iteration. The map is published before
its entries are parsed, so errors preserve partially populated requirements.
Other arrays are assigned only after their complete conversion succeeds.

Defence/attack animation values retain ID, zero delay and HIGH priority.
Audio-array values retain ID, zero delay, one loop and radius eight. These are
constructor-state ports. The oracle compiles complete Animation.java and Audio.kt,
but native animation definition lookup, scenery targets, duration/execution and
audio scheduling remain separate systems. Throwing adapters prevent those paths
from being silently emulated in the constructor tests.

The loader visits fields in Java String HashMap order, including collision and
resize behavior. Duplicate IDs/reloads, skipped empty/`null` text, ignored keys,
failed conversions and logging retain their original effects. Missing IDs receive
a fresh unregistered definition; configuration is not inserted into the registry
on their behalf. A `name` field changes its configuration property and does not
rename the cache definition or invalidate cached examine text.

The native JSON-simple DOM parser preserves observed leniency, including omitted
or repeated separators, duplicate keys, leading-zero integers, overflowing double
values, literal unknown escapes and unescaped controls. Stringification uses JDK
map order, JSON-simple escaping and legacy JDK double text. Tests exercise both
configuration loads and direct original JSONParser observations.

Native properties retain object/array/map identity across captures and reloads.
Their serde representation supports definition observations and scalar authoring;
it is not a general Java object model or a world-checkpoint format.

## Tests and full-data audit

The committed corpus contains **1,087 original JVM cases**. Four native tests
cover typed values, HashMap order, partial updates, retained references, reloads,
parser grammar/stringification, constants, file-reader state, UTF-8 and bounded
rejection of malformed Unicode/excessive nesting.

```powershell
cargo test -p rs09-server --test item_config
cargo xtask item-config-oracle --check --java-home PATH_TO_JDK_11
python tools/port_item_config.py --check
```

The harness compiles complete unchanged ItemConfigParser.kt, Audio.kt,
Animation.java and Definition.java, plus the original priority enum and selected
item constructor/lookup code. It supplies controlled logging, a metadata registry
and explicit external world/definition adapters. Actual isolated FileReader input
uses UTF-8; the reference checkout is never modified. Ordinary Rust tests require
neither Java nor Python.

The complete data audit loads the native archive-19 definitions, applies the
pinned item_configs.json through the actual native file loader, and compares
all resulting property types, values, references, requirement iteration and logs
against the original loader. It also checks the text-loading entry point.

```powershell
cargo run --release -p xtask -- item-audit --java-home PATH_TO_JDK_11
cargo run --release -p xtask -- item-config-audit --java-home PATH_TO_JDK_11
```

The pinned file is **3,155,252 bytes and 12,006 rows**. Across 14,659 native
definitions, 12,001 registered definitions receive configuration. The recorded
comparison completes with zero differences. Evidence, source/build/asset hashes
and property counts are in `parity/item-config-audit.json`. Raw definition
decoding and templates have their own full-archive audit.

During audit development, a Windows-default Python decoder corrupted literal
UTF-8 names in the test input. The harness now selects UTF-8 explicitly; ordinary
fixture outputs were verified unchanged during that repair. The source's signed
cache-string decoding remains preserved.

## Bounds and remaining integration

Native input is UTF-8, limited to 64 MiB and nesting depth 256. Unpaired UTF-16
escapes return an error. These are explicit safety differences outside the valid
pinned data. Other platform-default encodings and arbitrary Java string/object
interoperability have not been established. Files/readers are owned by Rust and
released deterministically rather than relying on JVM garbage collection.

The next integration work connects these properties to full item-definition
gameplay helpers, inventory/bank/equipment subclasses, persistence and player
startup. First world entry, content execution and complete client/server/headless
gameplay remain open acceptance gates.
