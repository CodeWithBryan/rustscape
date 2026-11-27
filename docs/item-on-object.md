# Five small item-on-object listeners (`world::item_on_object`)

Scope: five listener classes of the pinned
`content/global/handlers/item/withobject` package. The package's other
classes are either in `world::global_items2` (docs/global-items2.md),
`world::smelting`, `world::smithing`, or not ported.

| original | here |
|---|---|
| `SapCollectListener` (`onUseWith(SCENERY, KNIFE_946, *TREES)` and its `Pulse(2)`; `on(BUCKET_OF_SAP_4687, ITEM, "empty")`) | `Class::SapCollect`; `Interaction::SapCollect`, `sap_collect`, `Pulse::Sap`; `Interaction::SapEmpty`, `sap_empty`; `SAP_TREES` |
| `PoisonFountainListener` (`onUseWith(SCENERY, POISONED_FISH_FOOD_274, FOUNTAIN_153)` and its `Pulse(1)`) | `Class::PoisonFountain`, `Interaction::PoisonFountain`, `poison_fountain`, `Pulse::Piranhas` |
| `HairdresserCheeseListener` (`onUseWith(SCENERY, CHEESE_1985, TREADMILL_11677)` and its `runTask(player, 3, 1)`) | `Class::HairdresserCheese`, `Interaction::HairdresserCheese`, `hairdresser_cheese`, `Pulse::Cheese` |
| `FishEasterEgg` (`onUseWith(SCENERY, fish, *TREE_IDs)`, `onUseWith(SCENERY, fish, *doors)`) | `Class::FishEasterEgg`, `Interaction::FishOnTree`, `Interaction::FishOnDoor`, `woodcutting_ids()` (the `full` ids of `gathering::nodes::WoodcuttingNode::values()`) |
| `GWDEntranceRopeListener` (`onUseWith(SCENERY, ROPE_954, HOLE_26340)`) | `Class::GwdEntranceRope`, `Interaction::GwdRope`, `gwd_rope` |

The bodies are functions over `item_on_object::Host`, whose every method is
a call the original makes outside the lambda (ContentAPI `inInventory`,
`sendMessage`, `removeItem`, `addItem`, `replaceSlot`, `animate`,
`getAttribute`, `setAttribute`, `lock`, `unlock`, `setVarbit`,
`submitIndividualPulse`/`runTask`; the diary manager's `finishTask`; an
item's slot). `define` registers a class in the native
`InteractionListeners` registry as the original's `defineListeners` does
(the validating vararg overload for the single used item, the overwriting
array overload for the two herrings, `on` for the "empty" option). `pulse`
is the `pulse()` of the three pulse classes.

## Evidence, and what it does not show

**There is no JVM differential corpus for this module.** It was ported by
reading the five Kotlin files and the ContentAPI functions they call:

- `cargo test -p rs09-server --test item_on_object`: a recording host
  written from the same sources checks each branch's calls in order, the
  pulses' answers run by run, the registry rows `define` writes, and (where
  the reference checkout is present) the sap tree ids against the source
  text. It shows the port agrees with one reading of the sources, not with
  the original classes running.
- `live_world`'s `usesap`, `usepoison` and `usecheese` (docs/live-world.md)
  run sap collecting with the "empty" option, one herring joke, the
  fountain and the treadmill through a socket.

Not shown: any of this against a JVM; the animations on a client; the
herring on a door; the rope on the God Wars hole (the listener only removes
the rope and sets varbit 3932; nothing here reads that varbit, and the hole
itself is not ported); what else reads `piranhas-killed` (Ernest the
Chicken's fountain search, not ported).

## Kept from the sources

- The knife on a tree submits its pulse whether or not a bucket is held.
  Without one the player is told "You need a bucket to do that." and the
  pulse then runs every two ticks, removing nothing, until something stops
  it (a walk, another pulse, a logout).
- A full inventory cannot happen in the sap pulse (the bucket's slot was
  just freed), but the original's order is kept: the animation plays before
  the add, and the message depends on the add.
- The "empty" option replaces the item in the bucket's slot and answers
  false for a negative slot without a message.
- The fountain reads `piranhas-killed` and writes `/save:piranhas-killed`
  (one attribute, saved). The player is locked for five ticks; the pulse's
  counter says nothing on its first run, the first line on its second and
  the second line with the unlock on its third.
- The cheese lambda reads `isTaskSuccessful` before the task has run, so it
  always answers false. `InteractionListeners.run` ignores a scenery
  lambda's answer, so nothing follows from it. When the cheese is gone by
  the time the task runs (three ticks later) the task returns early without
  unlocking; the five-tick lock ends by itself.
- FishEasterEgg registers the herrings on every `WoodcuttingNode` id,
  duplicates included (the array overload overwrites).
- The rope listener answers false when the rope cannot be removed.

## Live world

`rs09-server --world` registers the five classes at boot when the class scan
names them (`world/live/items/everyday.rs` `install_content_use`), runs the
use-with lambdas from `InteractionListeners.run`'s arrival and the "empty"
option from the item option path. `WithObject`
(`world/live/items/withobject.rs`) is the host: see docs/live-world.md for
the services bound.
