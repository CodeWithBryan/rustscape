# Server animation behavior and prepared actors

`crates/server/src/definitions/animation.rs` ports the complete pinned server
`AnimationDefinition`. `world/animation.rs` ports `Animation`, `Graphics` and
`Animator`; `world/prepared_animation.rs` supplies the actual prepared player's
ten clocks, retained masks, owning-world definition registry and reset factory.
The stationary and movement/NPC example actors now own this controller. The
stationary animation/spot previews call it with the actual index20 cache source.

The JVM oracle compiles the complete unchanged classes, `Clocks`, `EntityFlag`
and `ByteBufferUtils`. Its 626 deterministic cases compare every stored
definition field, partial decoder positions/mutations, lazy cache identity,
priority, deadlines, clocks, retained records, registration and graphics-send
receiver ordering. Recording cache/tick/entity-mask/chunk services inject
reentry and failures. These adapters are identified in fixture provenance.
The full-cache audit compares all 11,155 present records (1,802,997 raw bytes)
through the strict native Archive source and original server definition methods.
It includes all stored fields, durations, repeated lookup identity and callbacks.

Preserved behavior includes:

- The server's Java frame high-word expressions mask a sign-extended short with
  `0xFFFF << 16`. They do not decode like the RT4 client. Both implementations
  retain their own source behavior.
- Durations above 100 are skipped when summing milliseconds; all durations enter
  the cycle sum. Arithmetic wraps as Java ints. Tick duration truncates
  milliseconds/600 and clamps to one; animation delay is not part of that value.
- Setting an Animation ID leaves its cached definition intact. Registered masks
  retain the actual Animation and read its current ID during encoding.
- An unexpired higher priority rejects both the replacement animation and its
  accompanying graphic. Equal priority can replace it.
- A false mask registration result does not reject the Animator's own state
  updates. This matters after masks have entered their render phase.
- Reset always clears the animation clock after its animation attempt returns,
  including when priority rejected the reset. Force stores its temporary
  deadline before invoking normal animation behavior.
- Each owning world has the three distinct original reset objects. Multiple
  actors in that world share those references, including mutable Animation IDs
  and cached definitions. Independent worlds have independent factories.

Native composition regressions join the actual Animator, prepared mask lifecycle,
wire encoders and example Actor/host. They cover rejection, delayed animation
encoding, retained contexts, clock publication, world separation, wire-index reuse
and eventual owner release. The JVM adapter registration events and the native
composition checks have separate scopes; they are not an original full-Entity
world replay.

Headless world observations also expose `playerAnimations`: up to 256 updates
containing the shared protocol decoder's player slot, animation ID and delay,
plus the exact PLAYER_INFO payload bytes. Clear IDs use the decoder's existing
`-1` representation; no game tick duration or rendered pose is inferred. The
history reports omitted entries after reaching its bound. Existing spot
observations stay separate.

Run `cargo test -p rs09-server --test animator --test prepared_masks` and all three
example targets, then `cargo xtask animator-oracle --check`. With pinned assets,
run `cargo run --release -p xtask -- animator-audit --output DIR`. Python-based
commands accept `--python PATH --java-home PATH`; use the pinned compiler's JDK11.
Windows local Cargo checks require the four debug/build environment settings in
AGENTS.md. Fixture generation changes authored expectations; `--check` only
reproduces them.

Full Entity/profile/body preparation, combat/content producers, original mutable
flag registry startup and remaining concrete providers are still required.
Graphics.send's control flow is ported, but its scene/chunk flag delivery remains
an explicit service. The movement host now keeps the same animator as its Actor;
original packet-to-approach-to-Pickup/inventory/content binding remains unfinished.
No accepted Take endpoint, full game replacement, speedup, parallel simulation or
player-capacity claim follows from these checks. All source inventory entries
remain `not_replaced` until complete runtime replacements are proven.
