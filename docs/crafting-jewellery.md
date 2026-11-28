# Crafting: jewellery casting (`world::crafting_jewellery`)

Scope: the pinned `content/global/skill/crafting/JewelleryCraftPlugin.java`,
`jewellery/JewelleryCrafting.java` (`JewelleryItem`'s rows, `open`, `make`,
`mouldFor`), `jewellery/JewelleryPulse.java` and
`content/global/handlers/iface/JewelleryInterface.java`, with `SkillPulse`'s
`start`, `pulse` and `stop`. Amulet and silver stringing, gem cutting and
limestone are docs/crafting-tools.md; battlestaves, glass and spiky
vambraces are docs/crafting-wares.md; spinning and silver casting are
docs/crafting-stations.md. The rest of the crafting package (leather,
pottery, weaving, light sources, snelms, snakeskin) is not ported.

| original | here |
|---|---|
| `JewelleryCrafting.JewelleryItem` (29 rows) | `JEWELLERY` |
| `JewelleryCraftPlugin.newInstance`, `JewelleryInterface.newInstance` | `FURNACES`, `GOLD_BAR`, `INTERFACE`, the class names |
| `JewelleryCraftPlugin.handle`, `JewelleryCrafting.open` | `handle`, `open` |
| `JewelleryInterface.handle` (its two `switch`es), its amount handler | `button` (`for_button`), `input` |
| `JewelleryCrafting.make`, `mouldFor` | `make`, `mould_for` |
| `JewelleryPulse` over `SkillPulse` | `JewelleryPulse` (`start`, `pulse`, `stop`) |
| `StringUtils.isPlusN` | `is_plus_n` |

The bodies are functions over `crafting_jewellery::Host`; every call the
original makes outside these classes is a required method with no default.

## Evidence, and what it does not show

- `fixtures/crafting-jewellery/oracle.jsonl` (1,654 cases, about 13,500
  operations) is recorded from the unmodified sources above with the
  unmodified `Pulse`, `SkillPulse`, `InputType`, `Plugin` and
  `Initializable`, with ContentAPI's Boolean `sendInputDialogue` and
  `StringUtils.isPlusN` spliced verbatim, compiled with Kotlin 1.8.20/JDK
  11 (`oracle/crafting-jewellery/generate.py`, `cargo xtask
  crafting-jewellery-oracle`), and `cargo test -p rs09-server --test
  crafting_jewellery` replays every case over this module and compares
  each result and each recorded call in order. One operation compares the
  table row by row, one the buttons 0 to 127, one the registrations with
  their classes.
- What the corpus does not establish (see `scope` in
  `fixtures/crafting-jewellery/provenance.json`): the player, the
  inventory, the level, messages, the interface configs and item models,
  the animation, the sound, the experience, the interface manager, the
  amount prompt, the Slayer flags and `PulseManager.run` are recording
  adapters answered from declared case state. An item's name is a declared
  table of the 29 products' and four blanks' names (with per-case
  overrides), not the cache's. The driver calls `handle`, the interface's
  `handle`, the prompt's handler (with an Integer or a String) and
  `pulse()` and `stop()` themselves, not `Pulse.update()`: the corpus does
  not show the ticks a pulse runs on, that a finished pulse is stopped, or
  how the manager replaces a running pulse. `Inventory.remove(Item...)` is
  an adapter that answers whether every item was held and removes the ones
  that were. The generator is not in `oracle/ci.toml`: CI replays the
  committed fixture but does not regenerate it.
- `cargo test -p rs09-server --lib crafting_jewellery`: one hand-read
  check.

## Kept behaviour worth knowing

- `open` hides a held mould's placeholder (children 14, 36, 55, 74) and
  then walks the 29 rows: a row whose items and mould are held at its
  level shows its product at zoom 170; otherwise a row whose mould is held
  shows the blank of its kind (1647, 1666, 1685, 11067) at zoom 1, and a
  row whose mould is missing shows nothing.
- Which kind a row is comes from its product's item name in lower case,
  not from the constant: the Slayer ring is a "ring" by its item name. An
  amulet row with its mould and a missing item is sent the blank amulet
  twice (zoom 220, then zoom 1).
- The interface's buttons are the row's component plus one; the necklaces,
  amulets and bracelets are matched in a second `switch (button - 3)`. A
  button of no row answers true and does nothing.
- The button checks the level first ("You need a crafting level of *n* to
  craft a/an *name*."), then the mould, then the opcode. "Make All" is the
  gold bars for a GOLD row and otherwise the smaller amount of the row's
  first two items; an opcode that is none of the four is the amount 0.
  "Make X" opens the prompt before the Slayer ring is checked.
- The Slayer ring is refused with two messages unless
  `SlayerFlags.isRingUnlocked()`; the flags are read for every row.
- `make` clamps the amount to "Make All", then asks for that many of each
  item ("You don't have the required items to make this item."), then the
  level again, closes the interface and runs the pulse. An amount of 0 or
  less passes the item check and the pulse still casts once.
- A prompt answered with a text throws a ClassCastException.
- The pulse animates (3243) and plays its sound (2725) on a run where the
  tick counter is a multiple of five, counted before that run's increment,
  and casts on every fifth run. The amount counts the casts tried: a cast
  whose items cannot all be removed still counts and gives nothing.
- The product's `add` answer is not read: with a full inventory the items
  are gone and the experience is still given.

## Live world

`world/live/items/crafting_jewellery.rs` adds JewelleryCraftPlugin to
`UseWithHandler.HANDLERS` (the tenth handler there) under the thirteen
furnaces for the gold bar, and puts JewelleryInterface on 446 as its
component plugin, when the class scan names them. The binding has no JVM
corpus.

- The pulse runs on the player's pulse manager. `start()` runs inside the
  manager's `run`; `SkillPulse.stop()` (the animation reset) runs when the
  pulse finishes and when the manager stops it.
- "Make X" is the input prompt with `InputType.NUMERIC`; an answer in text
  is the ClassCastException, reported as the handler's failure.
- The item names are the item definitions' of the loaded cache, the
  inventory is the canonical one and the level is the dynamic level.
- `SlayerFlags.isRingUnlocked()` answers false by name: slayer is not
  ported, so the Slayer ring is always refused here.
- `Inventory.remove(Item...)` is one removal per item in the row's order
  with every item tried; the container is refreshed after each here and
  once after all in the original.

The live world's `gather` scenario covers it with one character.
`ringman`, at Crafting 10 at the Al Kharid furnace with two gold bars, a
ring mould and a sapphire, uses a bar on the furnace, receives the eight
ring models of 446 and none for the kinds whose mould is missing, is
refused a sapphire ring (its level message) and a gold necklace (its
mould message) and casts two gold rings with "Make All" (30 experience).
No live check covers Make-X, the other kinds and furnaces, a gem row, a
full inventory or a pulse another action interrupts.
