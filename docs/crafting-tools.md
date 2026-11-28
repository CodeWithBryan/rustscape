# Crafting: gem cutting, limestone bricks and stringing (`world::crafting_tools`)

Scope: the pinned `content/global/skill/crafting/GemCutPlugin.java`,
`ChiselLimestonePlugin.java`, `AmuletStringingPlugin.java`,
`gem/GemCutPulse.java`, `gem/Gems.java`,
`limestone/ChiselLimestonePulse.java`,
`silver/SilverStringingListener.kt` and `silver/SilverProduct.kt`'s
`forProductID`, with `SkillPulse`'s `start`, `pulse` and `stop` and the
`JewelleryItem` rows AmuletStringingPlugin reads. Battlestaves, glass and
spiky vambraces are docs/crafting-wares.md. The rest of the crafting
package (jewellery and silver casting, spinning, leather, pottery,
weaving and the rest) is not ported.

| original | here |
|---|---|
| `Gems` (9 rows), `forId` | `GEMS`, `gem_for_id` |
| `JewelleryCrafting.JewelleryItem` (29 rows), `forProduct` | `JEWELLERY`, `jewellery_for_product` |
| `SilverProduct` (8 rows, product and strung id), `forProductID` | `SILVER`, `silver_strung` |
| the three plugins' `newInstance` (keys and allowed nodes) | `CHISEL`, `BALL_OF_WOOL`, `GEM_ALLOWED`, `LIMESTONE_ALLOWED`, `AMULET_ALLOWED` |
| `GemCutPlugin.handle` and its anonymous SkillDialogueHandler | `gem_handle`, `gem_get_all`, `gem_create` |
| `ChiselLimestonePlugin.handle` | `limestone_handle` |
| `AmuletStringingPlugin.handle` | `amulet_handle` |
| `GemCutPulse`, `ChiselLimestonePulse` | `GemPulse`, `LimestonePulse` |
| `SkillPulse.start`, `pulse`, `stop` for both | `Pulse` |
| `SilverStringingListener.defineListeners`, `stringSilverProduct` | `STRINGABLE`, `SILVER_LAMBDA`, `string_silver` |

The bodies are functions over `crafting_tools::Host`; every call the
original makes outside these classes is a required method with no default.

## Evidence, and what it does not show

- `fixtures/crafting-tools/oracle.jsonl` (804 cases, about 4,700
  operations) is recorded from the unmodified sources above with the
  unmodified `SkillPulse`, `Pulse`, `NodeUsageEvent`, `IntType`, `Plugin`
  and `Initializable`, compiled with Kotlin 1.8.20/JDK 11
  (`oracle/crafting-tools/generate.py`, `cargo xtask
  crafting-tools-oracle`), and `cargo test -p rs09-server --test
  crafting_tools` replays every case over this module and compares each
  result and each recorded call in order. One operation compares the
  tables row by row and another the `addHandler` calls and the `onUseWith`
  registration with its lambda class.
- What the corpus does not establish (see `scope` in
  `fixtures/crafting-tools/provenance.json`): the player, the inventory,
  the level, `removeItem`, `addItem`, messages, the animation, the sound,
  the experience and `PulseManager.run` are recording adapters answered
  from declared case state. `JewelleryCrafting` is an adapter shell with
  the pinned constants and the pinned `JewelleryItem` enum spliced in; its
  interface and `JewelleryPulse` are not covered. `SilverProduct` is
  compiled whole, but only `forProductID` and the strung ids are compared.
  `UseWithHandler` is an adapter that records `addHandler`: its `run`, its
  lookup by key and its pulses are not covered, and the driver calls a
  plugin's `handle` with an event it names. The driver calls
  `SkillPulse.pulse()` and `stop()` itself, not `Pulse.update()`: the
  corpus does not show the ticks a pulse runs on. The skill dialogue is
  an adapter that records `open()`. The generator is not in
  `oracle/ci.toml`: CI replays the committed fixture but does not
  regenerate it.
- `cargo test -p rs09-server --lib crafting_tools`: one hand-read check.

## Kept behaviour worth knowing

- GemCutPlugin takes the gem from whichever of the two items is not the
  chisel. With exactly one of the uncut gem held it cuts at once
  (`create(0, 1)`: the amount 0 still makes one cut, because the pulse
  counts down after its reward); otherwise it opens the one-option
  dialogue, also when none is held.
- The level is asked when the pulse starts and before every cut, not
  before the dialogue. Gems are never crushed here: opal, jade and red
  topaz always cut.
- A gem pulse cuts on every tick. A limestone pulse makes a brick on
  every second tick and goes on until no limestone is held; there is no
  dialogue and no amount.
- Neither pulse resets the animation when it stops.
- AmuletStringingPlugin looks the amulet up from the node the wool was
  used with, except that an onyx amulet as the used item is found either
  way. Any other amulet used on the wool finds no row and does nothing.
  The strung id is the unstrung id plus 19 (6581 for the onyx). It gives
  no experience, and neither does silver stringing.
- SilverStringingListener registers the ball of wool with the unstrung
  symbol 1714 and the unstrung emblem 1720 only.

## Live world

`world/live/items/crafting_tools.rs` installs AmuletStringingPlugin,
ChiselLimestonePlugin and GemCutPlugin in the world's
`UseWithHandler.HANDLERS` when the class scan names them, in the scan's
order (keyed by ITEM_TYPE under the ball of wool, the chisel and the
chisel), and SilverStringingListener's use-with registration when the scan
names it. The binding has no JVM corpus.

- The three handlers are reached through `UseWithHandler.run`'s
  item-on-item pulse (docs/herblore-tools.md), a tick after the click.
- The gem dialogue opens through `dialogue_dsl::skill` (ONE_OPTION).
- Both pulses run on the player's pulse manager with a delay of 1.
- `remove(Item...)` of an amulet and the wool is the canonical
  container's, both or neither.

The live world's `gather` scenario covers it with `gemman` at Lumbridge,
at Crafting 20: the chisel on one of two uncut sapphires opens the
dialogue and "All" cuts both (100 experience), the one uncut opal used on
the chisel is cut without a dialogue (10), an uncut diamond is refused
with its level message, the chisel on a limestone makes a brick (6), a
ball of wool on a gold amulet makes the strung amulet and another on an
unstrung symbol the strung symbol, both without experience. No live check
covers "Make X", an amulet's level refusal, the onyx amulet, an amulet
used on the wool, the unstrung emblem or a pulse another action
interrupts.
