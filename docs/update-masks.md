# Native update-mask lifecycle

`world::update_masks` ports the complete `UpdateMasks` registration, prepare,
write, writeSynced, unregisterSynced and reset control flow. The owning entity
and payload are retained native handles selected by the host. Registry, logger,
clock, impact queue and encoder services are required trait methods. They may
reenter mask operations without a held field guard.

There are eleven current slots and eleven persistent synced slots. Registration
ORs presence bits, even when a new element replaces a previous slot. Appearance
samples the clock and always registers persistently. A clock callback which
sets updating does not cause a second updating check. Provider lookup and the
missing-provider log occur before the updating check.

Reset clears current slots, current presence and updating only. Persistent slots,
their presence bits and the appearance timestamp remain. Unregistering a synced
slot removes its retained encoder's presence bits but leaves the current slot
untouched. Consequently clearing facing during one cycle can still emit that
cycle's facing payload. Treating facing as a transient flag would lose behavior
when another viewer starts observing the player.

Write captures the header presence and reads each slot when it reaches that
ordinal. An encoder retains its selected provider/context even if a callback
replaces or removes the slot. WriteSynced captures both presence fields before
the appearance-presence lookup, prefers current slots, and suppresses Appearance
only in the persistent fallback branch. The header uses a signed >=256 check;
negative masks emit a single low byte. Its extension bit depends on the actual
entity passed to write, rather than the cached owner type. A null viewer fails
at the first encoder call, after the header has already been written.

Prepare captures the entity's impact handler, then performs up to two separate
peek/poll pairs. The HitMark captures the passed entity, amount and type. Failed
preparation keeps completed queue removals and registrations and does not set
updating at the end. Reset retains persistent masks even after preparation.

The differential oracle compiles the complete pinned UpdateMasks class with its
sole System.currentTimeMillis expression redirected to a controlled clock.
EntityFlag, EFlagProvider and HitMark compile unchanged. Original player/NPC
provider constructor arguments supply the default table. Registry, recording
encoders, entity/impact queues and logger are explicit services. The 396 cases
compare every slot, context value, presence field, timestamp, queue, byte prefix
and service event after every action, including encoder reentry into the same
buffer, invalid ordinals, wrong encoder flags, null viewers, and partial failures.

Run `cargo test -p rs09-server --test update_masks` and
`cargo xtask update-masks-oracle --check --java-home runs/jdk11/jdk-11.0.32.1+1`,
alongside the affected checks in AGENTS.md. The prepared-player world now uses
this lifecycle in its actual Actor registration, host tick preparation, renderer
and post-render reset. See prepared-masks.md for that binding's precise scope.
All remaining concrete flags, registry startup, combat/impact production, full
profile/body preparation and authoritative pickup remain required integration.
The core's reentry/locking properties do not establish parallel simulation or
player capacity.
