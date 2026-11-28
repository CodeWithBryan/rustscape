# Crafting: spinning and silver casting (`world::crafting_stations`)

Scope: the pinned `content/global/handlers/scenery/SpinSpinningWheelPlugin.java`,
`content/global/handlers/iface/SpinningInterface.java`,
`content/global/skill/crafting/spinning/SpinningItem.java`,
`spinning/SpinningPulse.java`, `silver/SilverCraftingListener.kt`,
`silver/SilverCraftingPulse.kt` and `silver/SilverProduct.kt`'s rows and
`forButtonID`, with `SkillPulse`'s `start`, `pulse` and `stop`. Silver
stringing, gem cutting and limestone are docs/crafting-tools.md;
battlestaves, glass and spiky vambraces are docs/crafting-wares.md. The
rest of the crafting package (jewellery casting, leather, pottery,
weaving, light sources, snelms, snakeskin) is not ported.

| original | here |
|---|---|
| `SpinningItem` (11 rows), `forId` | `SPINNING`, `spin_for_button` |
| `SilverProduct` (8 rows), `forButtonID` | `SILVER`, `silver_for_button` |
| `SpinSpinningWheelPlugin.newInstance`, `SpinningInterface.newInstance`, the listener's `defineListeners` and `defineInterfaceListeners` | `WHEELS`, `SPIN_INTERFACE`, `SILVER_FURNACES`, `SILVER_INTERFACE`, the class and lambda names |
| `SpinSpinningWheelPlugin.handle` | `wheel_handle` |
| `SpinningInterface.handle`, its amount handler | `spin_button`, `spin_input` |
| `SpinningPulse` over `SkillPulse` | `SpinPulse` (`start`, `pulse`, `stop`) |
| the listener's use-with lambda, `onOpen(438)`, `on(438)`, the amount handler, `make` | `silver_use`, `silver_open`, `silver_button`, `silver_input` |
| `SilverCraftingPulse` | `SilverPulse` |

The bodies are functions over `crafting_stations::Host`; every call the
original makes outside these classes is a required method with no default.

## Evidence, and what it does not show

- `fixtures/crafting-stations/oracle.jsonl` (973 cases, about 8,000
  operations) is recorded from the unmodified sources above with the
  unmodified `core.api.Container`, `InputType`, `Event`, `Pulse`,
  `SkillPulse`, `IntType`, `Plugin` and `Initializable` and with
  ContentAPI's `hasLevelDyn`, `rewardXP`, `submitIndividualPulse`,
  `itemDefinition`, `getAttribute`, `setAttribute` and the Boolean
  `sendInputDialogue` spliced verbatim, compiled with Kotlin 1.8.20/JDK 11
  (`oracle/crafting-stations/generate.py`, `cargo xtask
  crafting-stations-oracle`), and `cargo test -p rs09-server --test
  crafting_stations` replays every case over this module and compares
  each result and each recorded call in order. One operation compares the
  two tables row by row and another the registrations with their classes.
- What the corpus does not establish (see `scope` in
  `fixtures/crafting-stations/provenance.json`): the player, the
  inventory, the equipment, the level, messages, the animation, the sound,
  the experience, the diary, the location, the attribute store, the
  interface manager, the amount prompts, `Entity.dispatch` and
  `PulseManager.run` are recording adapters answered from declared case
  state. An item's name is a declared text of its id, not the cache's
  name. The driver calls the wheel's `handle`, the interface's `handle`,
  the lambdas, a prompt's handler (with an Integer or a String) and
  `pulse()` and `stop()` themselves, not `Pulse.update()`: the corpus does
  not show the ticks a pulse runs on, that a finished pulse is stopped, or
  how the manager replaces a running pulse. `Integer.parseInt` is covered
  for the ASCII texts of the corpus only. The generator is not in
  `oracle/ci.toml`: CI replays the committed fixture but does not
  regenerate it.
- `cargo test -p rs09-server --lib crafting_stations`: one hand-read check.

## Kept behaviour worth knowing

- Six `SpinningItem` rows share button 23 and `forId` answers the first:
  only the root 6051 can be spun into magic string. TREE_ROOTS (button 31)
  asks for 6043 and makes a crossbow string.
- The interface asks for one of the needed item with "You need *name* to
  make this." before anything else; an opcode that is none of the four
  runs the pulse with the amount -1, which spins once.
- A SpinningPulse closes the interface every time it checks its
  requirements, also when they fail, and names the level in lower case
  ("crafting level").
- An item is spun on every fourth run of the pulse (every second with the
  Seers' hard diary complete, the Seers' headband worn and the player in
  rendering distance of the Seers' wheel). The amount counts the spins
  tried, not made: a spin whose item cannot be removed still counts.
- The animation and the sound play on a run where the tick counter is a
  multiple of five, counted before that run's increment.
- Every spin in region 10806 counts toward the Seers' bowstring task,
  whatever is spun; the fifth finishes it. The counter is read twice on a
  spin that only counts.
- A silver bar on a furnace stores the furnace and opens 438; the button
  asks for the mould, then refuses the silvthrill rod and the silvthril
  chain ("You can't do that yet."), then asks for the level.
- A silver pulse first runs a tick after it is made and then every five.
  It animates and plays its sound before it removes the bar. When the
  product cannot be added the bar is already gone and the pulse ends.
- The furnace of a pulse is read when the pulse is made; without one it is
  `Scenery(-1, -1, 0)`.
- "Make X" of the wheel takes a number or parses a text; the furnace's
  parses the text of whatever it is given. A text that is not a number
  throws.

## Live world

`world/live/items/crafting_stations.rs` puts SpinSpinningWheelPlugin on
the ten wheels' "option:spin" (a `scenery::SceneryPlugin`, whose
`getDestination` is null) and SpinningInterface on 459 as its component
plugin when the class scan names them, and registers
SilverCraftingListener in the class scan's order (the use-with
registration in the native InteractionListeners, `onOpen(438)` and
`on(438)` in the InterfaceListeners; `onOpen(438)` runs inside the
interface manager's open, beside the bank's and the shops'). The binding
has no JVM corpus.

- Both pulses run on the player's pulse manager. A SpinningPulse's
  `start()` runs inside the manager's `run`; `SkillPulse.stop()` (the
  animation reset) runs when the pulse finishes and when the manager
  stops it. A silver pulse's change of its delay is written back to its
  handle.
- The wheel's Make-X is the input prompt with `InputType.NUMERIC`, the
  furnace's with `InputType.AMOUNT`; an answer in text is parsed as a
  decimal `i32` and refused as a NumberFormatException otherwise (Java's
  `parseInt` also accepts non-ASCII digits; that is not reproduced).
- The item names are the item definitions', the hat is the canonical
  equipment's slot 0, the distance and the region are the player's
  location's, the bowstring counter is the player's attribute
  (`/save:diary:seers:bowstrings-spun`) and the diary is the persisted
  one.
- The furnace attribute is kept in this world's state by player, not in
  the player's attribute store. Only the ResourceProducedEvent reads it,
  and that event is skipped by name: entity event hooks are not ported.

The live world's `gather` scenario covers it with two characters.
`spinman`, at Crafting 1 beside the Lumbridge Castle wheel, opens the
interface with "Spin", is refused yak hair ("You need hair to make
this.") and flax (its level message), opens it again and spins four balls
of wool with "Make All" (10 experience). `silverman`, at Crafting 20 at
the Al Kharid furnace, uses a silver bar on the furnace, is refused a
tiara without its mould and casts two unstrung symbols with "Make All"
(100) after the nine item models of 438 arrived. No live check covers
Make-X, the Seers' headband or the bowstring task, the other wheels and
furnaces, a full inventory or a pulse another action interrupts.
