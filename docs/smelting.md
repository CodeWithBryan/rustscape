# Smelting at furnaces (`world::smelting`)

Scope: the furnace half of the pinned `content/global/skill/smithing` and
its chatbox in `content/global/handlers/iface`:

| original | here |
|---|---|
| `FurnaceOptionPlugin` (`newInstance`, `handle`, `show`, `getDestination`) and its `SmeltUseWithHandler` | `new_instance` over `Registrar`, `handle`, `show`, `destination`, `FURNACES`, `USE_FURNACES`, `OPTIONS`, `ore_ids` |
| `SmeltingInterface` and its `BarButton` | `interface_button`, `bar_button`, `AmountInput` |
| `SmeltingPulse` over `SkillPulse` | `SmeltingPulse` (`start`, `stop`, `pulse`, `update`, `check_requirements`, `animate`, `reward`, `success`) |
| `Bar` (`forId`, `forOre`) | `BARS`, `for_id`, `for_ore` |

The bodies are functions over `smelting::Host`; every call the original
makes outside these classes is a required method with no default.

Not ported: anvils (`SmithingPlugin`, `SmithingBuilder`, `SmithingType`,
`Bars`, `BarType`, `SmithingPulse`, `SmithingInterface`),
`AmmoMouldOnFurnace`, the jewellery and silver crafting furnaces, the
superheat spell that constructs a `SmeltingPulse` with `heat` (the pulse's
superheat branches are ported and tested, nothing calls them), and the
bots that smelt.

## Evidence, and what it does not show

**There is no JVM differential corpus for this module.** It was ported by
reading the pinned sources.

- `cargo test -p rs09-server --test smelting`: a recording host written
  from the same sources checks the calls of each branch in order (the
  chatbox, the Edgeville furnace's diary refusal, the destination, the bar
  buttons and Smelt-X, five ticks a bar, the ores running out, the level,
  ore and quest refusals, iron's coin toss, the ring of forging's charges
  and its last one, the goldsmith gauntlets, the two Lumbridge diary
  tasks, the Varrock armour's second bar, superheat, the registrations).
  It shows agreement with one reading of the sources, not with the
  original classes running.
- The same test compares every `Bar` row, the `BarButton` ids and
  `furnaceIDS` with the text of the pinned sources when `reference/` is
  present (it is in CI). That checks transcription, not behaviour.
- `live_world`'s `usesmelt` (docs/live-world.md) smelts two bronze bars
  through a socket.

Not shown by any of this: iron's roll and the Varrock armour's roll against
the original's generator state, the chatbox 311's packets against the
original's bytes, and experience totals against the original Skills.

## Kept from the sources

- `checkRequirements` closes the chatbox first, every time it runs: in
  `start()` and in every `pulse()`.
- The level refusal reads "in order to smelt bronze ." (the item's name
  lower-cased with "bar" removed, its space left behind), and closes the
  chatbox a second time.
- The attempt message names the enum constant through
  `StringUtils.formatDisplayName` ("a bar of Bronze."), the retrieve
  message the item's lower-cased name ("a bar of bronze.").
- `SkillPulse`'s constructor stops the pulse; `start()` sets it running
  only when the requirements hold, so a refused pulse is never submitted
  (unlike a cooking pulse, docs/cooking.md).
- A furnace bar takes five runs of the pulse; the animation and the
  furnace sound play on the first run and every fifth after.
- `reward` returns true (the pulse ends) when an ore cannot be removed,
  after any earlier ore of the bar was already removed.
- The ring of forging's last charge removes the ring, resets the charges to
  140 and calls `stop()` from inside `success`, and the bar of that run is
  still made; a ring that cannot be removed logs an error and fails the
  bar.
- The Varrock armour's second bar is rolled only when the inventory has a
  free slot and a second set of ores, inside 15 tiles of 3107,3500, with
  `random(100) <= 10`; the ResourceProducedEvent still names one bar.
- The goldsmith gauntlets replace the experience with 56.2 a bar for gold.
- `SmeltingPulse(player, node, bar, amount, heat)` turns `resetAnimation`
  off whatever `heat` is.
- `FurnaceOptionPlugin.getDestination` answers a tile only when the player
  is in a zone named "Donator Zone". No class of the pinned source
  registers a zone of that name, so it is always null there; the live
  world answers null without asking.

## In the live world

`world/live/items/smelting.rs`: `Smelt` is the `Host` (the canonical
inventory and equipment, the native Skills, the persisted diaries,
`RandomFunction.RANDOM`, the animator, the synth sound, the attributes,
the numeric input prompt). `QuestRepository.isComplete` answers false by
name (no quest is ported), so blurite is never offered or smelted;
`ResourceProducedEvent` is a named skip. The plugin and the interface are
registered at boot when the class scan names them. The Edgeville furnace's
refusal and the two diary rewards read the ported diaries, but nothing in
the live world completes a Varrock diary level, so furnace 26814 always
refuses there.
