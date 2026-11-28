# Nine small item-on-object listeners (`world::item_on_object`)

Scope: nine listener classes of the pinned
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
| `AxeOnTree` (`onUseWith(SCENERY, axes, *trees)`) | `Class::AxeOnTree`, `Interaction::AxeOnTree`, `AXES`, `woodcutting_ids()`; the lambda only hands `WoodcuttingSkillPulse(player, scenery)` to the host (`run_woodcutting_pulse`). The pulse itself is `gathering::woodcutting::Pulse`, verified by the gathering corpus (docs/gathering.md) |
| `CoalTruckListener` (`on(COAL_TRUCK_2114, SCENERY, "remove-coal")`, `on(COAL_TRUCK_2114, SCENERY, "investigate")`, `onUseWith(SCENERY, COAL_453, COAL_TRUCK_2114)`) | `Class::CoalTruck`; `Interaction::CoalRemove`, `coal_remove`; `Interaction::CoalInvestigate`, `coal_investigate`; `Interaction::CoalDeposit`, `coal_deposit` |
| `SeersVillageAchievementDiary.onCoalRemovedFromSeersTruck` and the coal truck branch of its `onAttributeSet` (medium task `TRANSPORT_COAL`) | `coal_removed_from_seers_truck`, `seers_attribute_set`; the diary's other tasks are not part of this module |
| `KaramjaBananaCrateListener` (`onUseWith(SCENERY, KARAMJAN_RUM_431, CRATE_2072)`, `onUseWith(SCENERY, BANANA_1963, CRATE_2072)`, `on(CRATE_2072, SCENERY, "search")`, `on(CRATE_2072, SCENERY, "fill")`) | `Class::KaramjaBananaCrate`; `Interaction::CrateRum`, `crate_rum`; `Interaction::CrateBanana`, `crate_banana`; `Interaction::CrateSearch`, `crate_search`; `Interaction::CrateFill`, `crate_fill` |
| `SwampHoleRopeListener` (`onUseWith(SCENERY, ROPE_954, DARK_HOLE_5947)`) | `Class::SwampHoleRope`, `Interaction::SwampRope`, `swamp_rope` |

The bodies are functions over `item_on_object::Host`, whose every method is
a call the original makes outside the lambda (ContentAPI `inInventory`,
`sendMessage`, `removeItem`, `addItem`, `replaceSlot`, `animate`,
`getAttribute`, `setAttribute`, `lock`, `unlock`, `setVarbit`,
`submitIndividualPulse`/`runTask`; the diary manager's `finishTask`; an
item's slot). `define` registers a class in the native
`InteractionListeners` registry as the original's `defineListeners` does
(the validating vararg overload for the single used item, the overwriting
array overload for the two herrings, `on` for the "empty" option and the
truck's two scenery options). `pulse`
is the `pulse()` of the three pulse classes.

## Evidence, and what it does not show

**There is no JVM differential corpus for this module.** It was ported by
reading the nine Kotlin files, the two diary functions and the ContentAPI
functions they call:

- `cargo test -p rs09-server --test item_on_object`: a recording host
  written from the same sources checks each branch's calls in order, the
  pulses' answers run by run, the registry rows `define` writes, and (where
  the reference checkout is present) the sap tree ids against the source
  text. It shows the port agrees with one reading of the sources, not with
  the original classes running.
- `live_world`'s `usesap`, `usepoison`, `usecheese`, `usecoal`,
  `usecrate`, `userope` and `usechop` (docs/live-world.md) run sap collecting with the "empty"
  option, one herring joke, the fountain, the treadmill, the Seers'
  Village coal truck (three coal in and out, both options), Luthas'
  banana crate (search, one banana, fill, the rum), the swamp hole's rope
  and a bronze
  axe on a tree through a socket.

Not shown: any of this against a JVM; the animations on a client; the
herring on a door; the rope on the God Wars hole (the listener only removes
the rope and sets varbit 3932; nothing here reads that varbit, and the hole
itself is not ported); what else reads `piranhas-killed` (Ernest the
Chicken's fountain search, not ported); the truck's fill message at 120
coal and the Seers' Village diary task through a socket (the recording
host covers both); the mine cart end of the coal trucks (nothing else
here reads `coal-truck-inventory`); who sets and pays out Luthas' task
(his dialogue is not ported, so only a save that already has
`luthasTask` reaches the crate's bodies, and nothing here reads the
bananas or `stashed-rum` afterwards: Pirate's Treasure and the ship to
Port Sarim are not ported); the dark hole itself (its "Climb-down" and
the Lumbridge swamp caves are not ported, so the tied rope changes
nothing a player can reach yet).

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
- The coal truck reads `coal-truck-inventory` and writes
  `/save:coal-truck-inventory` (one attribute, saved). Coal used on the
  truck deposits every coal held, not the one used, up to 120 in the truck;
  the fill message comes before the removal, and a refused removal leaves
  the attribute alone.
- "Remove-coal" takes as many coal as there are free slots. With no free
  slot `addItem` of zero coal still succeeds and the attribute is written
  unchanged, but the diary is not asked.
- The diary task needs both halves: the attribute reaching 120 sets
  `/save:diary:seers:coal-truck-full` (once, and only while the task is
  open), and coal taken from a truck that was full, at one of the two
  Seers' Village trucks, finishes the task and removes that attribute.
- Every crate lambda says "I don't know what goes in there." and answers
  true without Luthas' task. The rum reads `stashed-rum` and writes
  `/save:stashed-rum`. A banana or the rum locks the player for two ticks,
  animates 832 and opens a plain dialogue; "fill" sends a game message
  instead.
- "Fill" computes the amount before its checks and says nothing at all
  when no banana is held, even for a full crate. "Search" says "1 banana"
  and "N bananas".
- The swamp rope is removed before its item dialogue (the dialogue shows
  the removed `Item`); a second rope gets a plain dialogue and stays.
- AxeOnTree starts the legacy `WoodcuttingSkillPulse`, not
  WoodcuttingListener's script: the two differ (docs/gathering.md), and an
  axe used on a tree gets the pulse's rules (a dialogue for a full
  inventory, no instachop, no stump timer). The trees' own options run
  the listener's script (docs/live-world.md).

## Live world

`rs09-server --world` registers the nine classes at boot when the class
scan names them (`world/live/items/everyday.rs` `install_content_use`),
runs the use-with lambdas from `InteractionListeners.run`'s arrival, the
"empty" option from the item option path and the truck's and the crate's
options from the scenery option path (`world/live/scenery.rs` defers the listener run
to `Live::scenery_listener_run`, which walks to the scenery and runs the
lambda on arrival). `WithObject`
(`world/live/items/withobject.rs`) is the host: see docs/live-world.md for
the services bound. The woodcutting pulse's services over the live world
are `world/live/items/woodcutting.rs`, written without a corpus of its own;
its header and docs/live-world.md list what is answered natively and what
is skipped or defaulted by name (familiars, skillcape perks, bird nests,
the adze's projectile, farming patches).
