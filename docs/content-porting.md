# Porting content

How to port one piece of original content (an NPC's dialogue, a listener, an option handler) into the native server
and the live world, checked against the original. The kit makes the common case, a dialogue, a short job:

```
cargo xtask content-scaffold dialogue content/region/misthalin/lumbridge/dialogue/DonieDialogue.kt
```

lays the port out and generates its empty corpus; the porter writes the cases and the native bodies. The first port
made with it is Donie (docs/donie-dialogue.md).

AGENTS.md is binding. This page collects what it asks of every content port; the subsystem bullets there still
apply to the libraries a port runs on.

## What a port is made of

| Piece | Where | Made by |
| --- | --- | --- |
| The oracle: the original source compiled **unchanged** over recording stand-ins, run on JDK 11 | `oracle/<topic>/generate.py` | scaffold |
| The cases: worlds and actions to run | `oracle/<topic>/cases.py` | porter |
| The corpus and its provenance (source hashes, counts, scope) | `fixtures/<topic>/` | the generator |
| The native module: the classes and a `Host` trait of the calls they make outside their library | `crates/server/src/world/content/<area>/<module>.rs` | scaffold, then porter |
| The replay test: the corpus against the native module over a recording host | `crates/server/tests/content_<module>.rs` | scaffold |
| Registration: the module, its Host bound, its class name in the plugin table | `world/content/mod.rs` | scaffold |
| The live world's answers to the port's Host | `crates/server/src/world/live/ported/<module>.rs` | scaffold (refusals), then porter |
| CI: `cargo xtask <topic>-oracle --check` | `oracle/ci.toml` | scaffold |
| The port's document and an AGENTS.md bullet | `docs/<topic>.md`, `AGENTS.md` | scaffold (doc stub), porter |
| A live_world scenario | `crates/server/tests/live_world.rs` | porter |

`cargo xtask <topic>-oracle` needs no xtask edit: any `oracle/<topic>/generate.py` is reachable as `<topic>-oracle`,
and `tools/run_oracles.py` finds its runs/ folder from the generator's `RUN` line.

## The recipe

1. **Pick the class.** Check that no open PR or branch ports it (`gh pr list --repo CodeWithBryan/rustscape`,
   `git grep -i <name> $(git branch -r)`). Read the source and the classes it calls; know which library it runs on
   (the dialogue DSLs, InteractionListeners, an option handler) and that the library is ported.
2. **Scaffold.** `cargo xtask content-scaffold dialogue <source>` (path relative to
   `reference/server/Server/src/main`; `--java-home` for the JDK). It prints the files it wrote, the outside calls it
   found and the calls that are already shared services, and runs the generator once: the source must compile
   unchanged over the kit's stand-ins. If it does not, the source names something the stand-ins lack: add it to the
   kit (below), never to the source.
3. **Write the cases** (`oracle/<topic>/cases.py`). Each case is a world and a list of actions; mark one action per
   case as the `sweep` and the generator adds a case for every recorded call in it failing once in turn. Cover each
   branch, each option, the ends (closing mid-way, continuing after the end), a missing NPC, an unregistered key,
   and every world value the content reads (`serverName`, `settings: None`, `male: False`, attributes). List the
   recorded call names the corpus must reach in `NEEDED_EVENTS`. Then `cargo xtask <topic>-oracle` writes the
   corpus. Read some rows: the original often does something other than what the source suggests at a glance
   (Donie: with no settings the interpreter fails before Donie's own `?: "2009Scape"` is reached).
4. **Port the bodies** in the native module, in the original's order: every read, every call, every evaluation the
   original repeats (a DialogueLabeller re-runs `addConversation` on every pass, so arguments such as `player!!` or a
   new file are evaluated on every pass). Give each outside call its real signature on the port's `Host` trait,
   or on `content::Services` when other content will need it too (then add its pattern to the scaffold's
   `SHARED_SERVICES`).
5. **Replay.** `cargo test -p rs09-server --test content_<module>`. The recording host answers the port's services
   in the test file exactly as the oracle's stand-ins do (same recorded name, same values from the case). The first
   difference is printed as a JSON path into the snapshot (`/1/chat: native=1, original=0`).
6. **Bind it live.** Fill `live/ported/<module>.rs` with the live world's answers. A service that needs a system
   without a port refuses with a named error and is listed in docs/live-world.md; never answer with a fabricated
   success. Registration is already done: `Live::install_ported` walks `scan.initializable` in the original's order
   and initialises every class `content::dialogue_plugin` knows.
7. **Prove it live.** Add a scenario to `crates/server/tests/live_world.rs` that does what a player does (log in
   next to the NPC, Talk-to, Continue, choose) and asserts what the client receives. Run it with the cache and the
   template (`RS09_LIVE_SCENARIOS=<name>`); a SKIPPED run establishes nothing.
8. **Document.** Fill `docs/<topic>.md` (what is ported, the cases and counts, what each service became, what is not
   covered) and add an AGENTS.md bullet naming the suites and the oracle to run and the quirks to keep.
9. **Check.** `cargo fmt --check`, `cargo +1.97.0 clippy --workspace --all-targets -- -D warnings`,
   `cargo xtask <topic>-oracle --check`, the port's test, the suites of the libraries it runs on (for a dialogue:
   `dialogue_dsl`, `dialogue`, `plain_dialogue`, `live_world`) and `cargo xtask dialogue-dsl-oracle --check` if the
   kit or the dialogue-dsl host changed.

## The dialogue kit

`oracle/content-kit/dialogue.py` builds a dialogue port's oracle on top of the dialogue-dsl oracle instead of
repeating it: the dialogue core and DSLs compiled unchanged, the original text the dialogue-dsl generator inserts, and
its recording stand-ins (Player, NPC, PacketDispatch, InterfaceManager, ...). It adds the topic's sources, its own
stand-ins (`oracle/content-kit/stand-ins/`, which replace a dialogue-dsl stand-in of the same path) and two harness
actions (`DialogueOps.java`):

- `register` (`class`): ClassScanner's DIALOGUE branch, the no-argument constructor then `init()`;
- `openId` (`key`, `npc`): `DialogueInterpreter.open(id, npc)`, what NPCTalkListener does on Talk-to.

Every dialogue-dsl action stays available (`handle` is the Continue packet: `c` the interface, `b` the button;
options are buttons 2 and up; `close`, `input`, `closeEvent`, ...). The world is dialogue-dsl's (`npcs`, `attrs`,
`items`, `male`, `noLocation`, `bank`, ...) plus the kit's `settings` (null: no settings) and `serverName`.

The native side mirrors it: `oracle/content-kit/dialogue.rs` replays a corpus over the dialogue-dsl recording host
(`oracle/dialogue-dsl/native.rs`, `evaluate_with`) and handles the kit's actions. The dialogue-dsl fixtures are not
touched: the kit builds in `runs/<topic>-oracle`.

Adding to the kit: a stand-in a content source needs goes in `oracle/content-kit/stand-ins/<path>` (a Kotlin object
for a Kotlin `object` such as GameWorld, so `GameWorld.settings` compiles as against the original); a recorded call
it makes must be answered identically by the native recording host (a shared service: `oracle/content-kit/dialogue.rs`;
a port's own: its test file). A changed stand-in changes every dialogue port's provenance: regenerate each and check
their corpora are byte-identical, or explain why not.

## Other content

The scaffold's `generic` kind writes the native module (with the outside calls as a Host trait), the area
registration and the doc stub only. There is no shared oracle layer for listeners or option handlers yet: write
`oracle/<topic>/generate.py` from the closest existing corpus, using the shared compiler
(`oracle/login/generate.py`: `run`, `read`, `method`, `artifacts`, `SOURCES`) as every generator does. The best
models: `oracle/global-items2` (InteractionListener and option handler content with HashMap-order and generator
draws), `oracle/global-handlers` (registrations and option handlers), and the use-with listeners of #16. A shared
listener layer, built like the dialogue kit on one of those, is the next kit step.

## Rules every port keeps (from AGENTS.md)

- **Unchanged sources.** The reference is never edited; adapters and stand-ins live under `oracle/`. Original text an
  adapter needs is inserted from the pinned source at generation time (the dialogue-dsl `markers()`), not retyped.
- **No defaults for services.** Every call outside the port's library is a required Host method. A host that cannot
  answer refuses; it never fabricates success. Labels and recorded names never select behaviour.
- **Getters, not fields.** An oracle adapter must not expose a public field named like a Kotlin getter property
  (`username` once bypassed `getUsername`): Kotlin would read the field and the corpus would miss the call.
- **Generators stay separate.** RandomFunction.RANDOM, Math.random, ThreadLocalRandom (also Kotlin's `random()`)
  and Collections.shuffle's generator are distinct services with their own draw counts.
- **Identity hashes are inputs.** HashMap/HashSet order that depends on identity hashes is an explicit host input;
  never infer it from a sorted view.
- **Order and quirks are behaviour.** Keep repeated reads, evaluation order, integer wrapping, failure prefixes (what
  a failed call leaves behind) and the original's catch categories.
- **UTF-8 and LF.** Write sources and fixtures with explicit UTF-8 (`encoding='utf-8'`, `newline='\n'`); the
  fixtures are hashed byte for byte and Windows defaults differ.
- **JDK 11 for Kotlin oracles** (`RS09_ORACLE_JAVA_HOME` or `--java-home`); CI checks with JDK 11. Never regenerate a
  committed fixture to work around a toolchain difference.
- **Scope claims.** A port's document says what its corpus establishes and what it does not; a live scenario proves
  the binding, not the whole content.
- **Windows builds:** `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0`.
