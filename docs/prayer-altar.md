# Praying at altars (`world::prayer_altar`)

Scope: the pinned `content/global/skill/prayer/PrayerAltarListener.kt`.

| original | here |
|---|---|
| the companion's `altars` (87 scenery ids) and `boostedAltars` | `ALTARS`, `BOOSTED_ALTARS` |
| the `on(altars, IntType.SCENERY, "pray", "pray-at")` lambda and the private `pray` | `pray_at` |
| the chaos altar 412's `queueScript(player, 4, QueueStrength.STRONG)` lambda | `trap` |

The bodies are functions over `prayer_altar::Host`; every call the original
makes outside the class is a required method with no default.

Not ported: the second registration, `on(Scenery.CHAOS_ALTAR_61,
IntType.SCENERY, "check")`, which reads and sets Merlin's Crystal's stage.

## Evidence, and what it does not show

**There is no JVM differential corpus for this module.** It was ported by
reading the pinned sources.

- `cargo test -p rs09-server --lib prayer_altar`: a recording host written
  from the same sources checks the recharge's calls in order, the full
  points message, the boosted altars, the tribal statue with and without
  its quest, Elidinis' statuette at full points and the chaos altar's trap.
  It shows agreement with one reading of the sources, not with the original
  class running.
- The live world's `gather` scenario (`prayaltar`) recharges a level 43
  player from 3 points at Lumbridge church's altar (36972 at 3243,3206) and
  then reads the full points message. No live check reaches a boosted
  altar, the statuette, the tribal statue or the trap.

## Kept behaviour worth knowing

- The recharge is `Skills.setLevel(PRAYER, level)`: it sets the prayer
  points and the dynamic prayer level, two above the static level at the
  altars 2640 and 3521.
- Elidinis' statuette (10439) sets hitpoints to the static level plus 7 and
  sends its message even when the prayer points were full.
- The trapped chaos altar (412) only springs when the prayer was
  recharged: four ticks locked, then "It's a trap!" and an instant
  teleport to 2583,9576.

## Live world

`world/live/items/altar.rs` registers the listener at boot when
`PrayerAltarListener` is in the scan. `hasRequirement` answers false by name
(no quest is registered in this world), so the tribal statue (3863) returns
without a message, as the original does for a player without Tai Bwo Wannai
Trio. `PrayerPointsRechargeEvent` is skipped by name (entity event hooks are
not ported).
