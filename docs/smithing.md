# Smithing at anvils (`world::smithing`)

Scope: the anvil half of the pinned smithing content:

| original | here |
|---|---|
| `SmithingPlugin` (the `UseWithHandler` of the nine anvils, `handle`) and its anonymous `OptionHandler` on 42027's "smith" | `new_instance` over `Registrar`, `handle`, `option_handle`, `ALLOWED`, `ANVILS` |
| `SmithingBuilder` (`build`: the interface 300) | `build` |
| `SmithingInterface` | `interface_button`, `AmountInput` |
| `SmithingPulse` over `SkillPulse` | `SmithingPulse` (`start`, `stop`, `pulse`, `update`, `check_requirements`, `animate`, `reward`) |
| `Bars` (`forId`, `getBars`, `getItemId`), `BarType`, `SmithingType` (`forButton`) | `BARS`, `bars_for_id`, `bars_of`, `item_id`, `BAR_TYPES`, `bar_type_for_id`, `TYPES`, `for_button` |

The bodies are functions over `smithing::Host`; every call the original
makes outside these classes is a required method with no default.
`Bar.forId` is `world::smelting`'s.

Not ported: the two dialogue plugins the plugin opens (82127843 for the
dragon square shield and the dragonfire shield, 62362 for the godsword
blade), `Bars.getIndex` and `Bars.constructInterface` (no caller in these
classes), and the bots that smith.

## Evidence, and what it does not show

**There is no JVM differential corpus for this module.** It was ported by
reading the pinned sources.

- `cargo test -p rs09-server --test smithing`: a recording host written
  from the same sources checks the calls of each branch in order (Doric's
  anvil, the hammer, the shield and godsword refusals and dialogue keys, the
  interface for bronze and for blurite, the level refusal, the option
  handler's hammer branch with and without a bar, a silver bar's null type,
  the buttons, Make-X, four ticks a product after the first run, the bars
  and amounts of a platebody and of nails, each requirement refusal, the
  bars running out, the barefisted animation, the three diary tasks, the
  registrations). It shows agreement with one reading of the sources, not
  with the original classes running.
- The same test compares every `Bars`, `SmithingType` and `BarType` row,
  the allowed nodes and the anvil ids with the text of the pinned sources
  when `reference/` is present (it is in CI). That checks transcription,
  not behaviour.
- `live_world`'s `usesmith` (docs/live-world.md) smiths three bronze
  daggers through a socket.

Not shown by any of this: the interface 300's packets against the
original's bytes (the client script 150 arguments, the access mask, the
item container keys), and experience totals against the original Skills.

## Kept from the sources

- `checkRequirements` replaces the amount with the inventory's count of the
  bar whenever `required * amount` bars are not held: the count of bars,
  not of products they make (two platebodies asked with seven bars make the
  amount 7; the pulse then ends on the missing bars with the "You don't
  have enough" dialogue).
- `checkRequirements` closes the interface every time it runs.
- The first run of `reward` only sets the delay to 4; the first product
  comes four ticks after it.
- `reward` does not look at what `inventory.remove` or `inventory.add`
  answered.
- The message is "You hammer the bronze and make a bronze dagger." (the
  bar name lower-cased with "smithing" removed, its space left in place),
  with "an" by `StringUtils.isPlusN`.
- `SmithingType.forButton` answers 1, 5, -1 (X) and the inventory's count
  of the bar by the button's place; TYPE_STUDS has a fifth button, which
  answers its place, 4.
- TYPE_BULLSEYE and TYPE_OIL_LANTERN share their buttons and children;
  `Bars.getItemId` answers the first row of the bar type in declaration
  order.
- The hammer branch (the "smith" option of 42027) takes the first
  inventory item `Bar.forId` knows, which may be a silver or gold bar: the
  "smith-type" attribute is then set to null and `type.name()` throws.
- The shield checks use `&&`: one half (or one of visage and shield) is
  enough to reach the dialogue.
- Make-X casts the prompt's value with `(int)`: a String is a
  ClassCastException.
- The diary tasks: blurite crossbow limbs within 10 tiles of 3000,3145
  (Falador 1,9), a steel longsword within 15 of 3112,9688 (Lumbridge 2,0),
  an adamant medium helm within 15 of 3247,3404 (Varrock 2,3).

## In the live world

`world/live/items/smithing.rs`: `Smith` is the `Host` (the canonical
inventory, the native Skills, the interpreter's plain dialogue, the
interface manager, the persisted diaries, the animator, the numeric input
prompt). `InterfaceContainer.generateItems` is sent as the client script
150, the access mask and the item container, with the class's static key
counter kept in `SmithingWorld`; the grand exchange's port has its own
copy of that counter and is not in the live world. The three game
attributes are kept per player beside the attribute map (they hold enum
constants) and dropped at logout. Quests and quest requirements answer
false by name, so Doric's anvil 2782 always refuses and dart tips and claws
are never smithed; the skillcape perk answers false by name;
`ResourceProducedEvent` is a named skip; the two dialogue plugins are
refused by name. The plugin and the interface are registered at boot when
the class scan names them.
