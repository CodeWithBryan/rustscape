# Bone burying (`world::bone_burying`)

Scope: the pinned `content/global/skill/prayer/BoneBuryListener.kt` and the
`Bones` enum it reads.

| original | here |
|---|---|
| `Bones` (the 31 constants: item id and experience), `forId`, `array` | `BONES`, `experience`, `ids` |
| the `on(Bones.array, IntType.ITEM, "bury")` lambda | `bury` |
| its `queueScript(player, 1, QueueStrength.STRONG)` lambda and `removeBones` | `buried` |

The bodies are functions over `bone_burying::Host`; every call the original
makes outside these classes is a required method with no default.

Not ported: `Bones.forBoneMeal` and the bonemeal ids (the Ectofuntus), and
`PrayerAltarListener` (bones used on an altar).

## Evidence, and what it does not show

**There is no JVM differential corpus for this module.** It was ported by
reading the pinned sources.

- `cargo test -p rs09-server --lib bone_burying`: a recording host written
  from the same sources checks the table's size and a few rows, the order
  of the lambda's calls, its three silent returns (an id outside the table,
  the skilling clock, the inventory), the queued script's removal, message,
  experience and event, the pyre bonus, and an item that left its slot. It
  shows agreement with one reading of the sources, not with the original
  classes running.
- The live world's `gather` scenario (`burybones`) buries two bones and big
  bones through the client's first item option and reads the messages, the
  inventory and the prayer experience (4, 9 and 24 as the client shows it),
  and sends two clicks at once to see the skilling clock drop the second.

## Kept quirks

- `removeBones` empties the slot the item's own `slot` field names before
  it compares what was there with the clicked item. If the item left that
  slot in the tick between the click and the script, whatever lies there
  now is removed and nothing is rewarded.
- `Item` does not override `equals`, so the comparison is by identity.
- The pyre ship bonus (`/save:barbtraining:pyre-prayer-bonus`) multiplies
  the experience by four and uses one charge per burial. Nothing ported
  here grants charges.

## Live world

`world/live/items/bones.rs` registers the listener at boot when
`BoneBuryListener` is in the scan and runs the queued script on the
player's ScriptProcessor. `BoneBuryEvent` is skipped by name (entity event
hooks are not ported), so `JobManager`'s bone burying jobs see nothing.
