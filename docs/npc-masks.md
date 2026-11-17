# NPC update-mask encoders (`NPCFlags530`)

`crates/server/src/world/npc_masks.rs` ports the revision 530 NPC update-mask
providers of the pinned `core/game/world/update/flag/NPCFlags530.kt`: the
nine providers' constructor rows (flag, presence flag, ordinal) and their
`writeTo` bodies, with `EFlagProvider.logInvalidType`. The live world's NPC
masks (`world/live/combat/masks.rs`) encode through it; the library holds no
world state.

| Original | Native | Scope |
| --- | --- | --- |
| `NPCFlags530` (`PrimaryHit`, `SecondaryHit`, `Animate`, `FaceEntity`, `SpotAnim`, `TypeSwap`, `ForceChat`, `AnimationSequence`, `FaceLocation`) | `PROVIDERS`, `class_name`, `write_to` | every provider and body |
| `EFlagProvider.logInvalidType` | `write_to` (the message) and `Host::log_invalid` | the message text with the context's simple name and `typeOf<T>()` |
| `IoBuffer.p1`/`p1neg`/`p1sub`/`p2`/`p2add`/`ip2`/`ip4`/`putString` | private writers of `npc_masks` | the Java byte arithmetic |

## Behaviour that matters

- `PrimaryHit` writes the damage (`p1`) and the type (`p1neg`), then reads
  the hit mark's entity: `skills.maximumLifepoints` once, `skills.lifepoints`
  once for the comparison and once more when it is below the maximum;
  ratio `lifepoints * 255 / max` in Java int arithmetic (wrapping product,
  truncating division, `ArithmeticException` for a zero maximum after both
  bytes are written; a null entity throws after both bytes too).
- `SecondaryHit` is `p1neg(damage)`, `p1sub(type)`; `Animate` `p2(id)`,
  `p1(delay)`; `FaceEntity` `p2add(-1)` for null, else `p2add(getClientIndex())`;
  `SpotAnim` `p2add(id)`, `ip4((height << 16) + delay)`; `TypeSwap`
  `ip2(id)`; `ForceChat` `putString` (`String.getBytes()` and 0);
  `AnimationSequence` writes nothing (the base class's empty `writeTo`);
  `FaceLocation` `p2add((x << 1) + 1)`, `p2((y << 1) + 1)`.
- A context of another class (null included, except for `FaceEntity`) is
  logged — `Invalid context of type <simple name or null> passed to <class>
  flag which expects <type>.` — and nothing is written. `TypeSwap` names
  `core.game.world.update.flag.context.Graphics` as the expected type, as the
  original does.

## Required services

`npc_masks::Host`: the hit mark entity's maximum lifepoints and lifepoints,
a face target's client index, and the log. None has a default. The live
world answers the reads from the NPC rows and the player profiles
(`Live::health_of`, `Live::client_index`) and logs the line.

## Evidence

`cargo xtask npc-masks-oracle [--check]` (Python and JDK 11) compiles the
unchanged `NPCFlags530.kt`, `EFlagProvider.kt`, `EntityFlag.kt`,
`HitMark.java`, `IoBuffer.java`, `PacketHeader.java`, `ISAACCipher.kt` and
`ByteBufferUtils.kt` with Kotlin 1.8.20 and runs each provider's `writeTo`
on JDK 11 (`-Dfile.encoding=UTF-8`). `Entity`/`Player`/`NPC`, `Skills`,
`Animation`, `Graphics`, `Location` and `ForceMovement` are getter-only
adapters; the entity reads and `logWithStack` are recorded in order. The
corpus (`fixtures/npc-masks`) has 1,545 cases: every provider with ten
context kinds (null, a hit mark, an animation, graphics, a player, an NPC,
an Integer, a String, a Location, a Long), 225 damage/type pairs for both
hit marks, 126 lifepoint/maximum combinations (zero, negative and
`Integer.MAX_VALUE` maxima, overflowing products) and a null entity, 225
animations, 30 face targets, 375 graphics, 15 type swaps, 8 strings
(ASCII, colour tags, non-ASCII) and 225 face locations. Each case records
the provider's presence flag, ordinal, flag, the written bytes, the
recorded calls and the exception. `crates/server/tests/npc_masks.rs`
replays every case through `npc_masks::write_to` and must match; a
deliberately broken `p1sub` fails the replay.

## Differences and not covered

- `putString` uses the JVM's default charset; the oracle pins UTF-8 and the
  native writer writes UTF-8. A JVM with another default charset writes
  other bytes for non-ASCII text.
- The original's 2048-byte `IoBuffer` overflows on very long strings; the
  native writer grows (no case writes more than 2048 bytes).
- Not covered: the provider registry (`EntityFlags`), `UpdateMasks` itself
  (update-masks oracle), the NPC renderer and `AttackListener` (still ported
  without a corpus of its own in world/live/items.rs).
