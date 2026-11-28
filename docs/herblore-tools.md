# Herblore: grinding, herb tar and barbarian mixes (`world::herblore_tools`)

Scope: the pinned `content/global/skill/herblore/GrindItemPlugin.kt`,
`GrindingItem.java`, `HerbTarPlugin.java`, `HerbTarPulse.java`,
`Tars.java`, `BarbarianMixListener.kt` and `BarbarianPotion.kt`, with
`SkillPulse`'s `start`, `pulse` and `stop`. Cleaning herbs and mixing
potions are docs/herblore.md.

| original | here |
|---|---|
| `GrindingItem` (17 rows), `forItem` | `GRINDING`, `grinding_for_item` |
| `Tars` (5 rows), `forItem` | `TARS`, `tar_for_item` |
| `BarbarianPotion` (24 rows), `forId` | `MIXES`, `mix_for_id` |
| `GrindItemPlugin.newInstance`, `HerbTarPlugin.newInstance` | `grind_handler_ids`, `tar_handler_ids` |
| `GrindItemPlugin.handle`, `HerbTarPlugin.handle` | `grind_handle`, `tar_handle` |
| the two anonymous SkillDialogueHandlers' `getAll` and `create` | `Dialogue`, `get_all`, `create` |
| GrindItemPlugin's anonymous `SkillPulse` (its `init`, `reward`) | `GrindPulse` |
| `HerbTarPulse` (`checkRequirements`, `reward`) | `TarPulse` |
| `SkillPulse.start`, `pulse`, `stop` for both | `Pulse` |
| `BarbarianMixListener.defineListeners`, `handle` | `mix_registrations`, `mix` |

The bodies are functions over `herblore_tools::Host`; every call the
original makes outside these classes is a required method with no default.

Not ported: `SwampToadPlugin` ("remove-legs"), the last class of
`content.global.skill.herblore`. It is an `ItemDefinition.setOptionHandler`
plugin, and inventory item option handlers are refused by name in the live
world.

## Evidence, and what it does not show

- `fixtures/herblore-tools/oracle.jsonl` (1,016 cases, about 4,500
  operations) is recorded from the unmodified sources above with the
  unmodified `Herbs`, `BarbarianTraining`, `SkillPulse`, `Pulse`,
  `NodeUsageEvent`, `ChildPositionContext`, `Plugin`, `Quests` and
  `IntType`, compiled with Kotlin 1.8.20/JDK 11
  (`oracle/herblore-tools/generate.py`, `cargo xtask
  herblore-tools-oracle`), and `cargo test -p rs09-server --test
  herblore_tools` replays every case over this module and compares each
  result and each recorded call in order. One operation compares the three
  tables row by row and another the 29 `onUseWith` registrations with
  their lambda classes and the 22 `addHandler` calls.
- What the corpus does not establish (see `scope` in
  `fixtures/herblore-tools/provenance.json`): the player, item names, the
  inventory, both levels, the quest stage, attributes, `removeItem`,
  `addItem`, messages, `sendDialogue`, the animations, the experience,
  `PacketRepository.send` and `PulseManager.run` are recording adapters
  answered from declared case state. `UseWithHandler` is an adapter that
  records `addHandler`: its `run`, its lookup by key and its pulses are not
  covered, and the driver calls a plugin's `handle` with an event it
  names. The driver calls `SkillPulse.pulse()` and `stop()` itself, not
  `Pulse.update()`: the corpus shows the delay a pulse leaves, not the
  ticks it runs on. The use-with lookup is a declared model and the skill
  dialogue an adapter that records `open()`. The generator is not in
  `oracle/ci.toml`: CI replays the committed fixture but does not
  regenerate it.
- `cargo test -p rs09-server --lib herblore_tools`: one hand-read check.

## Kept behaviour worth knowing

- Neither plugin checks anything before its dialogue. Grinding has no
  quest, level or pestle check at all once the handler is reached, and no
  experience.
- The grinding pulse's node is the used item of the event. Using the item
  on the pestle and mortar grinds the item. Using the pestle and mortar on
  the item makes the pestle and mortar the node: "All" is the number of
  pestles held, and the first reward removes a pestle and mortar and then
  fails with a NullPointerException (`GrindingItem.forItem(node)` is
  null), which the corpus records.
- Fishing bait is ground ten at a time (or all that is left), and the
  amount asked is cut to a tenth of the bait held, rounded up, when it
  is more than a tenth rounded down.
- The grinding pulse waits two ticks between rewards. The tar pulse's
  first run only sets its delay to four.
- Herb tar asks the Druidic Ritual (`isComplete`), the level, a pestle and
  mortar and 15 swamp tar on every run, each with its own message. A run
  without the ingredient ends the pulse silently.
- "All" for herb tar is the amount held of the used item, which is the
  swamp tar when the tar was the item used.
- A barbarian mix answers `false` (so the use falls through to
  `UseWithHandler.run`) when the training attribute is 0, when the potion
  is not held or when the egg is not held; in the last case the potion is
  put back. The level asked is the static one. The first attack mix at
  stage 1 sets the attribute to 2 and sends Otto's hint.

## Live world

`world/live/items/herblore_tools.rs` installs GrindItemPlugin and
HerbTarPlugin in the world's `UseWithHandler.HANDLERS` when the class scan
names them (keyed by ITEM_TYPE, allowed nodes 233 and 1939), and
BarbarianMixListener's 29 use-with registrations when the scan names it.
The binding has no JVM corpus.

- `UseWithHandler.run`'s item-on-item pulse (`UseWithHandler$2`) is now
  run (`items/use_with.rs`): a `Pulse(1)` as the player's STANDARD pulse
  whose body, a tick later, closes the interface and walks the handlers.
  These two plugins are the only handlers keyed by an item here.
- The dialogues open through `dialogue_dsl::skill` (ONE_OPTION) and each
  is followed by the `RepositionChild` of 309's child 2.
- Both pulses run on the player's pulse manager; the grinding pulse is
  made with its delay of 2 and the tar pulse's change to 4 is written back
  to its handle.
- The Druidic Ritual stage is the save's; nothing in this world advances
  it. The barbarian training attribute is the one Otto's dialogue sets
  (docs/live-world.md).

The live world's `gather` scenario covers it with two characters at
Lumbridge. `mixnovice`, without the training attribute, uses roe on an
attack potion (2) and is told nothing interesting happens. `grindman`,
at Herblore 19 with the quest saved and the attribute at 1, grinds two
unicorn horns with "All", grinds ten of 25 fishing bait with "Make 1",
makes two guam tars from 30 swamp tar with "All" (60 experience) and
makes an attack mix from the potion and the roe (8 experience, Otto's
hint). No live check covers the pestle and mortar used on an item, a tar
refusal (quest, level, pestle, tar), caviar, a mix's level refusal or a
pulse another action interrupts.
