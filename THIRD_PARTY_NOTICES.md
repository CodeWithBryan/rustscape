# Third-party source

This workspace derives behavior and implementation details from:

- 2009scape server, https://gitlab.com/2009scape/2009scape
  commit 9921b98d49308edd0489dd50ee98011d53914a57, AGPL-3.0-only.
- 2009scape RT4 client, https://gitlab.com/2009scape/rt4-client
  commit 61b952d2b46132b9a5d696f4e0c4c0df75c7f6d2, AGPL-3.0-only.

See the root LICENSE and the pinned repositories for original attribution.
Oracle builds compile selected original classes without modifying them; adapter
classes are maintained separately in oracle/adapters. Rust ports retain upstream
semantics and use the same source license.

Rust package dependencies and their declared licenses are recorded in Cargo.lock
and their package manifests. No cache/game asset distribution license is implied
by the source license. Downloaded cache assets are outside this source foundation
and need their own provenance manifest.

`crates/server/src/config/toml_compat.rs` adapts the string-decoding replacement
order from toml4j 0.7.2's StringValueReaderWriter and MultilineStringValueReader,
Copyright (c) 2013-2015 Moandji Ezana, MIT. The complete notice is retained in
`third-party/toml4j-LICENSE.txt`. The test-only original JAR/source artifacts are
pinned in `oracle/config/artifacts.lock.json`; upstream source is
https://github.com/mwanji/toml4j.

`crates/game-core/src/java_double.rs` adapts OpenJDK 11 FloatingDecimal's legacy
decimal digit generation and stopping/rounding rules, Copyright (c) 1996, 2016,
Oracle and/or its affiliates. This independent numeric module is licensed under
GPL-2.0-only WITH Classpath-exception-2.0, retaining the Classpath exception.
Its original notice is in the source; the complete license and exception are in
`third-party/openjdk11-numeric-LICENSE.txt`. Source identity is recorded in
`third-party/openjdk11-numeric.json`. Other native game modules remain under
the workspace's AGPL-3.0-only license. The JDK is used only for test oracles;
native runtime formatting and parsing run entirely in Rust.

`crates/game-core/src/java_lower.rs` adapts OpenJDK 11 ConditionalSpecialCasing
and RuleBasedBreakIterator's ROOT contextual lowercase and word-iterator behavior.
Both original copyright/license headers are retained in that module. It is
licensed GPL-2.0-only WITH Classpath-exception-2.0, with the complete license and
exception in `third-party/openjdk11-numeric-LICENSE.txt` and source identities in
`third-party/openjdk11-case.json`. Native mapping/category/automaton data are
authored from exhaustive JDK observations under `fixtures/string-case`; test-only
reflection and the JDK never enter native runtime packaging. Other locales and
general BreakIterator APIs are outside this module's scope.

`vendor/macroquad` retains Macroquad 0.4.16, Copyright (c) 2019-2021 Fedor Logachev,
MIT OR Apache-2.0. Both complete licenses are in that directory. A local atlas
texture retirement fix preserves queued glyph draws until end-of-frame submission;
source provenance and the exact two-file patch are recorded alongside the source.
The version and selected features are unchanged, and Miniquad remains the original
registry dependency. See `vendor/macroquad/UPSTREAM_PATCH.md`.
