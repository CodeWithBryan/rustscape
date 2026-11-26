# Late player appearance field writer

`player/appearance_writer.rs` ports the original PlayerFlags530.Appearance flow
over the real native Appearance class. It prepares body data through the supplied
player, captures that Appearance once and reads the original live receivers while
writing. It does not construct a prepared appearance DTO before encoding.

The original Kotlin method rejects a null output buffer before inspecting its
context. Invalid contexts log through the required provider service. For a valid
player, preserve both size calls, separate skull lookups for wilderness/disabled,
the captured body array, and all five color references before dereferencing any
color. The actual Appearance render override method performs its original
attribute reads. PvP branches keep separate world settings, properties, familiar,
skills and skull receivers and preserve short circuits and integer wrapping.

The shared output subset retains its actual cursor and backing data across
callbacks. Nested writers append to that same object. Rewinds can make the final
length negative; the original patch location and transformed byte are preserved.
Completed bytes and body/prepared-state mutations survive errors. Primitive short
and long writes acquire the output guard once; no guard spans a service callback.
This growing output implements only the operations this flag uses. It does not
replace full IoBuffer capacity/limits, framing, encryption, reads or bit modes.
Its explicit cursor setter is bounded to allocated bytes.

604 JVM scenarios compile the verbatim original Appearance flag and invalid-type
logger with complete original Appearance/BodyPart/Gender/Container/Item/Animation
and IoBuffer. Player properties, skull/familiar/skills and world settings are
recording service receivers. Tests compare callbacks, state, shared aliases,
nullable receivers, invalid contexts, nested output, rewinds and failed prefixes.
The first 468 verified observations have a separate immutable prefix pin; all 127
appearance cases from the older original field corpus also match the new writer.
Mixed-player writes and intended Unicode names extend those observations.

The harness verifies the reference's private field layout and requires actual
getter events. Public observer fields initially caused Kotlin to select raw field
access. Those rejected authoring observations/logs are retained under
`runs/appearance-writer-investigation`; the corrected observer uses distinct raw
field names. An editing helper also changed Unicode source literals through the
Windows default encoding. Exact prior inputs were restored using ASCII escapes,
the verified prefix stayed unchanged, and intended names are additional cases.

Run `cargo test -p rs09-server --test appearance_writer --test player_appearance`
and `cargo xtask appearance-writer-oracle --check` with Python and JDK 11, plus the
affected appearance/mask/render/Animator and example gates in AGENTS.md. Source
and observation pins are checked independently. The native recorder shares the
existing Appearance setup without changing its 448 original observations.

The live world (`world/live/appearance.rs`) runs this writer at the mask write
over its receivers: the canonical Appearance, equipment, render info and
attributes, the profile's Properties and Skills, the SkullManager, the node's
size and the configuration's `pvp`/`wild_pvp_enabled`. Its familiar receiver is
the pouch flag and the profile's summoning combat level (familiars are not
installed), and `transformNPC`'s services are refused. That join is covered by
the live-world test's appearance scenario (cache-backed, not run in CI) and the
login-tick comparison, not by a JVM fixture of its own. The examples' transport
publisher remains the explicitly prepared-data development path
(`PreparedMasks::write`); `write_late` takes the flag's encoder.
This kernel is not full Player initialization, accepted gameplay, full IoBuffer,
parallel simulation or a performance/capacity result. All 4,042 source-inventory
rows remain not_replaced; the complete Rust replacement objective stays active.
