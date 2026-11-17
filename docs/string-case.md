# JDK 11 ROOT string casing

`game_core::java_lower::lowercase_root` implements the ROOT String lowercasing
used by Kotlin `lowercase()` in the timer registry. It uses reviewed JDK 11 data
rather than Rust's current Unicode version. ASCII has a direct native path.
Non-ASCII mapping preserves expansions such as U+0130 to `i` plus a combining dot,
supplementary characters, and the JDK's contextual capital-sigma behavior.

JDK 11 sigma context uses its word BreakIterator, including random UTF-16 lookup
and cache behavior. A forward-only Unicode word split is insufficient: for
example, the original lowercases `𐐀𐐁Σ` to `𐐨𐐩σ`. The native port preserves this
behavior using the original forward/backward automata and UTF-16 cursor rules.
U+FFFF's CharacterIterator sentinel behavior is retained as well.

The checked-in data records all 1,114,112 code points' word categories and cased
flags, and every valid scalar's singleton ROOT lowercase result. It compresses
these into ranges, 1,304 changed mappings and state tables. Runtime code and data
are entirely Rust; no JDK, reflection, downloading or generation occurs at startup.
The adaptation retains the original OpenJDK notices and Classpath exception; see
THIRD_PARTY_NOTICES.md and third-party/openjdk11-case.json.

```powershell
cargo test -p xtask --test string_case
cargo xtask string-case-oracle --check --python <python> --java-home <jdk11>
python tools/port_jdk_lower.py --check
```

The oracle reproduces the full observed tables and 20,105 actual String/word
boundary scenarios. These include each changed singleton mapping, category-range
endpoints, contextual sigma, supplementary characters, controls and randomized
mixed strings. Fixtures pin original observations, source adapters and hashes.
Native tests compare both lowercase results and forward boundary UTF-16 offsets.

This helper supports valid Rust strings; unpaired UTF-16 cannot be represented.
Other locales and general BreakIterator APIs are outside scope. Existing
authentication/SQL lowercasing still requires a separate migration that models
the JVM's default locale; adding this ROOT helper does not establish that parity.
It does not change `equalsIgnoreCase`, which has its own UTF-16 unit contract and
existing tests. No casing performance claim follows from these comparisons.
