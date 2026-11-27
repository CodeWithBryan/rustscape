# Cooking on ranges and fires (`world::cooking`)

Scope: the use-with path of the pinned `content/global/skill/cooking`:

| original | here |
|---|---|
| `CookingRewrite` (`onUseWith(IntType.SCENERY, RAW_FOODS, *COOKING_OBJs)`, `cook`) | `define`, `use_with`, `cook`, `raw_foods`, `COOKING_OBJECTS` |
| `StandardCookingPulse`, `PieCookingPulse`, `PizzaCookingPulse`, `IntentionalBurnPulse` | `CookingPulse` with `Kind` |
| `CookingDialogue` (the amount chatbox 307, the sinew options, Cook-X) | `Dialogue`, `AmountInput` |
| `CookableItems` (`forId`, `getBurnt`, `intentionalBurn`, `getIntentionalBurn`, `gauntletValues`, `lumbridgeRangeValues`) | `COOKABLES`, `for_id`, `burnt`, `intentional_burn`, `intentional_burn_product`, `GAUNTLET_VALUES`, `LUMBRIDGE_RANGE_VALUES` |

The bodies are functions over `cooking::Host`; every call the original
makes outside these classes is a required method with no default.
`RandomFunction.getSkillSuccessChance` is `gathering::random`'s (verified
by the gathering corpus).

## Evidence, and what it does not show

**There is no JVM differential corpus for this module.** It was ported by
reading the pinned sources.

- `cargo test -p rs09-server --test cooking`: a recording host written from
  the same sources checks the calls of each branch in order (one item, a
  burn, the dialogue and its buttons, Cook-X, the Lumbridge range, a level
  too low, gauntlets, the intentional burn, pies and pizzas away from a
  range, the range-only foods, seaweed, barley, a bowl of water, sinew,
  spits and cake tins, the tutorial branch). It shows agreement with one
  reading of the sources, not with the original classes running.
- The same test compares every `CookableItems` row, the gauntlet and
  Lumbridge tables and `COOKING_OBJs` with the text of the pinned
  `CookableItems.java` and `CookingRewrite.kt` when `reference/` is present
  (it is in CI). That checks transcription, not behaviour.
- `live_world`'s `usecook` (docs/live-world.md) cooks through a socket.

Not shown by any of this: the burn roll against the original's generator
state, the chatbox 307's packets against the original's bytes, and
experience totals against the original Skills (the live world's Skills is
the ported one).

## Kept from the sources

- `CookableItems.forId` reads a map filled with `putIfAbsent` by raw id:
  raw chicken is CHICKEN (never UGTHANKI), raw beef is BEEF (never SINEW),
  and raw id 0 is RAW_OOMLIE, so seaweed, barley and raw oomlie have no
  cooking row. Seaweed and barley are cooked through the intentional-burn
  map (keyed by the `cooked` column) with no level, roll or experience.
- A new `Pulse` is running, and `PulseManager.run` submits a pulse that is
  running after `start()`. A cooking pulse whose `start()` refuses (the
  Lumbridge range without Cook's Assistant, a level too low, a pie on a
  fire) is therefore still submitted, and its first run checks again: the
  refusal is sent twice, a tick apart.
- `checkRequirements` draws the burn roll (`randomDouble(100.0)`) every time
  it runs: in `start()` and in every `pulse()`, including the run that only
  sets the delay.
- The first run of `reward` only sets the delay (5 at an object whose name
  contains "range", else 4; `IntentionalBurnPulse` wants the name to equal
  "range"); the first item was cooked by `start()`.
- `IntentionalBurnPulse` declares its own `amount`, which its `reward`
  counts down and tests with `<= 1`, while the base `pulse()` still tests
  the base field that only `start()` decremented.
- Drying sinew runs a `StandardCookingPulse` with BEEF's row (level 1, 30
  experience, a burn roll, "You manage to cook some beef."):
  `SinewCookingPulse` exists in the pinned source but nothing constructs it.
  It is not ported.
- The fire animation (897) plays only at an object named exactly "fire";
  everything else gets the range animation (883).
- `getMessage` answers null for barley malt, a bowl of hot water and any
  food whose name starts with neither "Raw" nor "Uncooked";
  `PacketDispatch.sendMessage(null)` sends nothing.
- A broken spit is a `RandomFunction.random(15) == 5` draw before the food
  is removed; the cake tin comes back before the cake is removed.

## Live world

`rs09-server --world` registers CookingRewrite at boot
(`world/live/items/everyday.rs`, at its place in the class scan among the
other installed use-with classes) and binds `Host` in
`world/live/items/cooking.rs`. See docs/live-world.md for the answers by
name: no quest is complete (the Lumbridge castle range 114 always
refuses), the Hasty Cooking perk is off, `ResourceProducedEvent` is not
dispatched, and cooking before the tutorial is complete is refused.

## Not ported

The tutorial island branch of the listener (a named service, refused in
the live world); the rest of the package: CakeListener, ChocolateBarCutter,
CoconutListener, DoughMakingListener, NettleTeaListener,
SaucyKebabListener, SkeweredFoodListener, TeaInteraction,
WatermelonSliceListener, WineFermentListener, FireOptionPotteryPlugin,
MakeStewPlugin, `dairy/`, `fermenting/` and `recipe/`. Fires lit with a
tinderbox (firemaking) are not ported either, so in the live world the
only fires are the scene's own.
