# Global item handlers (`world/global_items`)

`crates/server/src/world/global_items/` ports the general-purpose item
handlers of `content/global/handlers/item` a playable world needs. This pass
covers the teleport items (the first of the three groups of the slice); the
everyday item options and the item-on-object basics are not ported yet (see
the end of this page).

Each Kotlin lambda and OptionHandler the original registers is a typed native
value (`registry::Interaction`, `registry::OptionPlugin`,
`registry::DialoguePlugin`); its body is a function over the `Host` trait,
where every call the original makes outside the ported bodies is a required
method with no default. Deferred bodies (queued scripts, the shield's pulse,
the explorer's ring dialogue action and item-select callback) are `Task`
values; the jewellery dialogue file and the teleport crystal's dialogue plugin
are `Dialogue` values holding their stage. The world stores them and runs them
with `registry::run_task`, `registry::run_dialogue` and
`registry::open_dialogue`.

## What is ported

| Original | Rust | Scope |
| --- | --- | --- |
| `content.data.EnchantedJewellery` | `jewellery::Jewellery`, `use_jewellery`, `attempt_teleport`, `teleport_script`, `id_map_keys`, `pluralise` | the nine constants (options, locations, ids, crumble), `idMap` and its key walk, `use`, `attemptTeleport` with its queued script (all three stages), `replaceJewellery`, the slayer progress dialogue, `getNext`/`getLocation`/`getJewelleryType`/`getItemIndex`/`isLastItemIndex`; `SlayerUtils.pluralise` |
| `EnchantedJewelleryListener` and `EnchantedJewelleryDialogueFile` | `jewellery::handle`, `jewellery::dialogue` | operate/rub (ring of life refusal, lost charge, the direct use without options), the two dialogue stages |
| `EnchantJewelleryTabListener` | `tablets::enchant_break`, `enchant_use`, `enchant_script` | the six tablets' break message, the 24 use-with pairs, the queued enchant and its dupe alert |
| `TeleTabsListener` | `tablets::TeleTab`, `tablets::teletab` | all twenty tablets with destinations and requirement checks (the Plague City attribute, the runecrafting altar checks) |
| `HouseTeleTabOptionListener` | `tablets::house_tablet` | the house check, lock, enter, teleport and removal |
| `TeleportCrystalPlugin` and `TeleportCrystalDialogue` | `items::crystal`, `crystal_dialogue`, `crystal_open`, `crystal_handle` | Mourning's End and the level-20 wilderness rule, the dialogue (open's argument casts, the Lletya teleport, the degrade to the next crystal) |
| `EctophialListener` | `items::ectophial_refill`, `ectophial_empty`, `ectophial_script` | the refill at the Ectofuntus, emptying (Ghosts Ahoy, tele-block), the three-stage script |
| `ExplorersRingPlugin` | `items::ring_replenish`, `ring_alchemy` (with its action and select), `ring_cabbage`, `ring_operate` | the daily run and alchemy charges in the `daily-explorer-ring` store, the cabbage-patch teleport |
| `FaladorShieldPlugin` | `items::falador_shield`, `falador_pulse` | prayer restore with its daily time, the operate emote pulse |
| ContentAPI `teleport`, `delayEntity`, `resetAnimator`, `animate` | `teleport`, `delay_entity`, `reset_animator`, `animate` in `mod.rs` | the bodies these handlers call (INSTANT writes the teleport location; any other type is `teleporter.send`) |
| the classes' registrations | `registry::define`, `register_scanned`, `Native` | each class's `defineListeners`/`newInstance(null)` in the original's order; `register_scanned` walks the start-up manifest (content interfaces, then `@Initializable` plugins) |

## Behaviour that is easy to get wrong

- Registration: the jewellery ids are `idMap.keys`, a JDK `HashMap` walk
  (table of 128 for the 50 ids, buckets by `id & 127`, insertion order inside
  a bucket), not declaration order. A Kotlin lambda that captures nothing
  (the tablets' `break`, the teleport and house tablets) is one object for
  every registration; the rest are new objects (the tablets' use-with lambda
  captures its enchanted id). FaladorShieldPlugin puts `option:operate` for
  the three shields, then `option:prayer-restore`.
- Enchanted jewellery: `operate` is the equipped path, `rub` the inventory
  one. The ring of life refuses before the table lookup; a last charge that
  does not crumble refuses with two messages; the digsite pendant and ring of
  life teleport without a dialogue. The zone rule is `zoneMonitor.teleport(1,
  next charge item)` (the wilderness level-30 jewellery rule lives in the
  zones); Tarn's Lair needs Haunted Mine. A choice past the last location only
  shows the slayer progress for a ring of slaying; button 0 (choice -1) passes
  the zone and Tarn's Lair checks and then fails at the location array.
- The teleport script is INSTANT (`teleport(player, location)` defaults to
  it), with the script delay at `ticks + 4`, `lock 4`, impacts disabled for 4,
  dueling/games graphics both before and after, the TeleportEvent with the
  original item, the Varrock diary task for the digsite pendant. The last ring
  of slaying, equipped, is removed and the gem added a stage later; when that
  removal fails the code falls through to the inventory slot replacement and
  then to the crumble removal (from the equipment), logging each failure.
  `replaceSlot(...) == item` is an identity comparison.
- The enchanting tablets read the animation duration twice (delayEntity and
  queueScript) and alert on any slot replacement that did not return the used
  item, with `Item.toString` of both (`null` for none).
- Teleport tablets: `inInventory` is asked twice; Ardougne's attribute is
  read with the "/save:" prefix and Kotlin's Boolean cast (another type is a
  cast failure); only Ardougne has its own refusal message.
- The house tablet answers false without a house, locks for 5 before the
  inventory check, and a null enter location fails Kotlin's parameter check.
- The teleport crystal's dialogue reads `args[1]` before switching on
  `args[0]`; button 1 teleports and degrades whatever `send` answers and does
  not end the dialogue; it constructs two NPCs on every handle.
- The explorer's ring compares charges with `>=` against the ring level, uses
  the default-locale lower case for the run key and the root lower case for
  the alchemy key, and writes `remaining - 1` with the count captured when the
  option was used. `JSONObject.getInt` converts Long, Double and Float with
  Kotlin's `toInt` and anything else to the default.
- The Falador shield: `(Item) node` lets null through and fails at `getId`;
  `switch (option)` fails on null; the restore time is cast to Long; the
  prayer effect runs before the graphics array read (an unknown shield fails
  there); the emote pulse fails at its graphics read with an unknown shield
  after locking and animating, every time it is pulsed.

## Required services and joins

The `Host` methods name the original call each answers. The world joins them
to: the native teleports (`teleport_send` is `teleport::Manager::send`,
`set_teleport_location` the walking queue's teleport location); the zones
(`zone_teleport` is `zones::Monitor::teleport` with the jewellery rule of
`wilderness_zone`, `is_tele_blocked` is `zones::is_tele_blocked`,
`wilderness_check_teleport` is `wilderness_zone::check_teleport`); the native
attributes store (GameAttributes' "/save:" rule); the script processor
(`queue_script`, `delay_script`, `stop_executing`, the delay field); action
locks; the Animator; the impact handler (`combat_pulse::impact`); the
InterfaceManager and dialogue interpreter; containers (`remove_item`,
`replace_slot`, `add_item`, `add_item_or_drop`, `in_inventory` and the
inventory calls); native consumables' `Effect::Prayer` for `prayer_effect`;
Settings' run energy; the ServerStore archives; audio packets; diaries.
Content stays a named service: the slayer manager and task name
(`slayer_*`), the house manager (`house_*`),
`MysteriousRuinListener.checkReq` (`altar_requirement`),
`ModernListeners.alchemize`, `hasRequirement`/`isQuestComplete` (the quest
requirement tables), `PlayerMonitor.log`. `home_location` is
`ServerConstants.HOME_LOCATION` as the RING_OF_LIFE constant read it.

## How the live world registers them

At start-up, after the InteractionListeners registry exists: build
`registry::Native::new(&interactions, &casing)` and call
`registry::register_scanned(&mut native)`. Then put each of
`native.item_handlers` (`(id, "option:...", handle)`) into the item
definition's handlers map (`definitions::option_handler`), and register each
of `native.dialogue_plugins` with the world's dialogue plugin registry
(`world/dialogue`), where opening that key calls `registry::open_dialogue`
and a button calls `registry::run_dialogue` on the stored `Dialogue`. The
item option packet path (InteractionListeners first, then the definition's
option handler) dispatches a registered `Interaction` through
`registry::run_interaction` and an `OptionPlugin` through
`registry::run_handler`. Each `Task` handed over is stored with its queue,
pulse, dialogue action or item-select callback (the GenericItemSelect
interface of `world/global_handlers` calls it) and run with
`registry::run_task`; `open_dialogue` hands the jewellery `Dialogue` to the
interpreter the same way. Nothing in `--world` installs these yet.

## Reference tests

```powershell
cargo test -p rs09-server --test global_items
cargo xtask global-items-oracle --check --java-home <jdk11>
```

`oracle/global-items` compiles the originals listed in `UNCHANGED` of
generate.py unchanged on JDK 11 with Kotlin 1.8.20 and the pinned ConstLib
1.4.28 (Items, Sounds, Scenery and Animations constants checked against their
names), with the unchanged OptionHandler, Plugin, Pulse, IntType,
core.api.Container, Script, DialogueConst, DialogueAction and TeleportMethod.
Original text inserted at generation time: `Item.toString`, the DialoguePlugin
constructor, `init` and `end`, `DialogueFile.end`, the TeleportEvent class,
`ServerStore.getInt`, the Skills ids, `SlayerUtils.pluralise`,
`ARDOUGNE_TELE_ATTRIBUTE` and the ContentAPI functions `teleport`,
`delayEntity`, `lock`, `unlock`, `visualize`, `animate`, `resetAnimator`,
`face` and `getAttribute`. Everything else is a recording adapter answering
from the case; the adapters' fields are named apart from the getters' Kotlin
properties. Times the original reads from `System.currentTimeMillis` (the
shield's restore time) are recorded relative to that clock in seconds.

`fixtures/global-items/oracle.jsonl`: 835 cases (494 designed, 341 with one
recorded call failing in turn), 3,135 operations, 13,355 recorded calls, in
16 groups (registration with the scan order; the jewellery by charge, by
location, equipped, the last ring of slaying with every fallback, crumbling,
refusals, the slayer progress and the dialogue file; the enchanting tablets;
the teleport tablets; the house tablet; the teleport crystal; the ectophial;
the explorer's ring; the Falador shield). No random generator is reached by
these sources, so the corpus has no draws. The test replays every
operation through the native bodies and registrations with a recording host
that follows the adapters and compares the result, the error, every recorded
call (registrations included, with the original class names and object
identities), the attributes, the store archives and the dialogue stages after
each operation. Every case matches; there are no pinned gaps. The same file
checks that `register_scanned` over the start-up manifest produces exactly the
oracle's scan-order registration, that `Native` installs the typed objects
into the native InteractionListeners registry (one object for the
non-capturing tablet lambda), and the `idMap` key walk.

Thirty-nine deliberate breaks of the port were tried (the pulse clear, the
lost-charge test, the HashMap table size, the last-location bound, the Tarn's
Lair button, the script delay, the stage delays, the dueling graphics, the
slayer ring's delayed gem, the varp shift, a plural, the lower case, the item
index, the next charge's amount, the inventory replacement's current item, the
dialogue stage step, the tablets' second duration read and identity test, the
teleport tablets' first inventory check, Ardougne's Boolean cast, the house
lock, the refusal messages, the crystal's degrade bound, NPC construction, the
ectophial's facing and item cast, the ring's stale count, charge comparison and
operate level, the pulse's reset tick, the shield's array bounds and item cast,
`getInt`'s Double conversion, INSTANT, delayEntity's order, the handler
registration order, the shared lambda and a lambda class name): 38 fail the
replay. The one that does not is the shield's `attrTime >
currentTimeMillis()` turned into `>=`: the oracle runs against the real clock,
so no case can hold a time equal to it; the port keeps `>`.

## Differences from the original

- The registry never dispatches a null node to an `on`/use-with lambda; the
  bodies take the node by reference (the option handlers still take an
  optional node and option, as the original can be handed null).
- `ServerConstants.HOME_LOCATION` is read when the ring of life's location is
  needed, not when the enum is initialised.
- `ContentAPI.openDialogue`, `queueScript`, `sendItemSelect` and the other
  ContentAPI functions besides the four ported ones are services at their
  ContentAPI level (their bodies belong to the dialogue interpreter, the
  script processor, the item-select interface and the containers).
- Errors carry a category, not a Java stack trace.

## Not ported or not joined

- The rest of the slice: the everyday item options (EmptyOptionListener,
  LightSourceListeners, the spade, caskets, bird nests, XP lamps, the starter
  pack, ItemQuestRequirementListener, MorphItemListener, RingUnchargePlugin,
  EnchantedGemListener, the newspaper, rune packs, herb boxes, cadava potion,
  blamish oil, toys and pet rock, the global books) and the item-on-object
  basics (water and sand sources, chest keys, the hat stand, sack on hay, the
  brawling gloves and Fist of Guthix gloves managers). PickupPlugin is
  already native (`world/pickup.rs`).
- The content services named above (slayer, construction, runecrafting,
  magic alchemy, quest requirement tables).
- Nothing installs the library in `rs09-server --world` yet (see above for
  how).
