# Crafting: battlestaves, glass and spiky vambraces (`world::crafting_wares`)

Scope: the pinned `content/global/skill/crafting/BattlestaffListener.kt`
(with its `BattlestaffProduct`), `glass/GlassCraftingListener.kt`,
`glass/GlassCraftingPulse.kt`, `glass/GlassMakeListener.kt`,
`glass/GlassMakePulse.kt`, `glass/GlassProduct.kt` and
`leather/SpikyVambraces.kt`. Gem cutting, limestone and stringing are
docs/crafting-tools.md; spinning and silver casting are
docs/crafting-stations.md; jewellery casting is
docs/crafting-jewellery.md. The rest of the crafting package (leather,
pottery, weaving, light sources, snelms, snakeskin) is not ported.

| original | here |
|---|---|
| `BattlestaffProduct` (4 rows), `productMap` | `BATTLESTAVES`, `battlestaff_for_orb` |
| `GlassProduct` (8 rows), `forButtonID` | `GLASS`, `glass_for_button` |
| the three `defineListeners`, `defineInterfaceListeners`, `SpikyVambraces.newInstance` | `ORBS`, `GLASS_MAKE_INPUTS`, `FURNACES`, `GLASS_INTERFACE`, `VAMBRACES`, the lambda class names |
| BattlestaffListener's lambda, `getMaxAmount`, the dialogue's `create`, the task | `battlestaff_use`, `battlestaff_max`, `battlestaff_create`, `battlestaff_task` |
| GlassCraftingListener's use-with lambda, `on(542)`, the amount handler, `make` | `glass_use`, `glass_button`, `glass_input` |
| `GlassCraftingPulse` | `GlassPulse` |
| GlassMakeListener's lambda and the dialogue's callbacks | `glass_make_use`, `glass_make_max`, `glass_make_create` |
| `GlassMakePulse` | `GlassMakePulse` |
| `SpikyVambraces.handle`, `craftVamb` | `vambraces_handle` |

The bodies are functions over `crafting_wares::Host`; every call the
original makes outside these classes is a required method with no default.

## Evidence, and what it does not show

- `fixtures/crafting-wares/oracle.jsonl` (749 cases, about 3,900
  operations) is recorded from the unmodified sources above with the
  unmodified `core.api.Container`, `InputType`, `Event`, `Pulse`,
  `NodeUsageEvent`, `IntType`, `Plugin` and `Initializable` and with
  ContentAPI's `hasLevelDyn`, `rewardXP`, `submitIndividualPulse`,
  `runTask`, `sendSkillDialogue` and `SkillDialogueBuilder` spliced
  verbatim, compiled with Kotlin 1.8.20/JDK 11
  (`oracle/crafting-wares/generate.py`, `cargo xtask
  crafting-wares-oracle`), and `cargo test -p rs09-server --test
  crafting_wares` replays every case over this module and compares each
  result and each recorded call in order. One operation compares the two
  tables row by row and another the registrations with their lambda
  classes.
- What the corpus does not establish (see `scope` in
  `fixtures/crafting-wares/provenance.json`): the player, the inventory,
  the level, `removeItem`, `addItem`, messages, the animation, the sound,
  the experience, the diary, the interface manager, the amount prompt,
  `Entity.dispatch` and `PulseManager.run` are recording adapters answered
  from declared case state. The driver invokes a use-with lambda through
  a declared lookup with two Items as its nodes (also for the scenery
  registration), the interface lambda with a component, opcode and button
  it names, the prompt's handler with an Integer, and `pulse()` itself,
  not `Pulse.update()`: the corpus does not show the ticks a pulse runs
  on, and a prompt answered with text is not covered. `UseWithHandler` is
  an adapter that records `addHandler`. The skill dialogue is an adapter
  that records `open()`; its callbacks are called with item index 0 only.
  The generator is not in `oracle/ci.toml`: CI replays the committed
  fixture but does not regenerate it.
- `cargo test -p rs09-server --lib crafting_wares`: one hand-read check.

## Kept behaviour worth knowing

- A battlestaff is made at once when exactly one orb or exactly one
  battlestaff is held; otherwise the dialogue opens. The level is asked
  before either, and not again by the task.
- The dialogue's task runs every two ticks, `min(amount, max)` times, where
  the maximum is read when the amount is chosen. A run that cannot remove
  an orb does nothing and the task still runs out its count.
- The air battlestaff finishes the Varrock diary task also when nothing
  was made (the check is outside the removal).
- `on(542)` asks for the pipe, the molten glass and the level in that
  order, each with its own message; the pulse asks for the pipe and the
  glass again every run, without a message, and never for the level.
- A glassblowing pulse first runs a tick after it is made and then every
  three ticks; a molten glass pulse every two. Each sends its message and
  animates before it removes anything.
- The molten glass pulse removes the soda ash before the sand: a pulse
  that finds the ash but no sand cannot happen after its own check, so
  nothing is lost.
- "All" at the furnace is the smaller of the ash and the sand held. An
  amount below 1 ends either pulse on its first run.
- SpikyVambraces reads the node the item was used with: the claws used on
  the vambraces make the product, and a handler reached with the claws as
  that node does nothing. The level asked is 32 for all five.

## Live world

`world/live/items/crafting_wares.rs` registers BattlestaffListener,
GlassCraftingListener and GlassMakeListener in the class scan's order when
the scan names them (the use-with registrations in the native
InteractionListeners, `on(542)` in the InterfaceListeners), and
SpikyVambraces in the world's `UseWithHandler.HANDLERS` under the five
vambraces. The binding has no JVM corpus.

- Both dialogues open through `dialogue_dsl::skill` (ONE_OPTION).
- `runTask`'s pulse and the two glass pulses run on the player's pulse
  manager; a glass pulse's change of its delay is written back to its
  handle.
- Make-X is the input prompt with `InputType.AMOUNT`; an answer in text
  is parsed as a decimal `i32` and refused as a NumberFormatException
  otherwise (Java's `parseInt` also accepts non-ASCII digits; that is not
  reproduced).
- The ResourceProducedEvent is skipped by name: entity event hooks are not
  ported.
- The Varrock diary task is the persisted diary's.

The live world's `gather` scenario covers it with two characters.
`glassman`, at Crafting 10 at the Al Kharid furnace, uses soda ash on the
furnace, makes two molten glass with "All" (40 experience), opens the
glassblowing interface with the pipe, is refused a vial with its level
message and blows two beer glasses with "Make All" (35). `staffman`, at
Crafting 54 at Lumbridge, attaches kebbit claws to leather vambraces (6)
and makes two water battlestaves with "All" from the dialogue. No live
check covers Make-X, a battlestaff made without the dialogue, the air
battlestaff's diary task, the other furnaces, a missing pipe or a pulse
another action interrupts.
