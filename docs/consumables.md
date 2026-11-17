# Eating and drinking

`world::consumables` ports core/game/consumable (`Consumable`, `Food`,
`Drink`, `Potion`, `Cake`, `HalfableFood`, `BarbarianMix`,
`FakeConsumable`, `ConsumableEffect`), the `Consumables` table of
content/data/consumables, every class of content/data/consumables/effects
and `ConsumableListener`, the handler the "eat" and "drink" item options
run. It is a library: nothing in a session, the tick driver or the live
world installs it yet (see Joins).

## What is ported

| Original | Native |
| --- | --- |
| `Consumables`: 335 constants in declaration order (class, ids, animation, messages, effect object, `isIgnoreMainClock`), the `consumables` map filled with `putIfAbsent` and the `potions` list | `table::rows`, `Table::new`, `Table::lookup`, `Table::potions` |
| `Consumable.consume` of each class, `getNextItemId`, the default/custom/healing messages, `getFormattedName` (three variants), `getHealthEffectValue`, `Potion.getDose` | `Consumable::consume`, `next_item_id`, `formatted_name`, `health_value`, `dose` |
| The 34 effect classes: `activate` and `getHealthEffectValue` | `effects::Effect::activate`, `health_value` |
| `ConsumableListener.defineListeners`/`handleConsumable` with the clocks | `define_listeners`, `handle` |
| ContentAPI `replaceSlot`, `removeItem` (its use here), `getUsedOption`, `delayAttack`, `queueScript`, `stopExecuting`, `getStatLevel`, `inInventory`, `modPrayerPoints`; `RandomFunction.random(a, b)`, `random(max)`, `nextInt` | `replace_slot`, `used_option`, `delay_attack`, `queue_script`, `stop_executing` and the effects' helpers |
| Potion's queued dose message and TroubleBrewingRumEffect's two pulses | `DoseScript::run`, `RumPulse::pulse` |

Skills are the native `player::skills::Skills` (`heal`, `healNoRestrictions`,
`updateLevel`, `getLevel` with its assist read, `getMaximumLifepoints` with
its damage-sponge read, the prayer increments); run energy is the native
`player::settings::Settings::update_run_energy`.

## Behaviour that is easy to get wrong

- The listener looks the node's id up, reads the option, and then gates on
  the clocks before checking that the node is an `Item`: a non-item node with
  a consumable's id reads the ticks and returns true without consuming.
  Food needs `NEXT_CONSUME` and `NEXT_EAT` strictly below the tick and sets
  `NEXT_EAT` to tick + 2; the cooked karambwan (`isIgnoreMainClock`) needs
  only `NEXT_CONSUME` and sets both; drinks use `NEXT_DRINK` and + 1. Every
  read and write of the tick is its own `GameWorld.ticks` call. Eating calls
  `delayAttack(player, 3)`, which delays the combat pulse by 3 and sets
  `NEXT_ATTACK`. The option is compared case-sensitively with "eat"; any
  other value (also "INVALID" when unset) takes the drink branch; an
  attribute of another class fails as a cast.
- `getConsumableById` returns the first constant declaring an id:
  `CHOCOLATE_BOMB` (2195) is `VEG_BALL`, `SLAYERS_RESPITE_M_KEG` is
  `SLAYERS_RESPITE_KEG`, empty containers (1919, 1923, 2026, ...) are the
  first drink or dish declaring them. `consume` looks the effect up again by
  the item's id after the slot is replaced (a foreign id fails as a null).
- `Item.getId()` is `(id << 16 | 1000) >> 16 & 0xFFFF`: the "next id" -1 of
  the last element becomes 65535, which `replaceSlot` treats as "empty the
  slot". Single-id food replaces the slot with amount - 1 (0 empties it);
  multi-id food goes through `remove(current, slot, true)` and, when that
  fails, logs a DUPE_ALERT with `Item.toString` and removes the item from
  the equipment instead. Potions and cakes call `Container.replace`/`remove`
  directly (a potion's last dose becomes a vial, 229).
- Messages: food "You eat the X.", halves "You eat half the X." / "You eat
  the remaining X.", drinks "You drink the X.", potions "You drink some of
  your X." plus a SOFT script two ticks later ("You have N doses of potion
  left.", "1 dose", "finished") whose `stopExecuting` reads the active script
  twice and clears the queue when it persists; barbarian mixes "You drink the
  lumpy potion."; custom messages replace the defaults and (except for
  potions and cakes) the healing message. Cakes pick the message by the id's
  index. Names drop "(1)".."(4)" (drinks also "(m1)".."(m4)", "(m)";
  halves "1/2", "Half an", "Half a"), Java-trim and lowercase.
- Effects keep Java's arithmetic: `(int)` truncation, `Math.floor`/`ceil`,
  double accumulation order, int wrapping. `RandomHealthEffect` turns a
  non-positive roll into `DamageEffect(roll)`, which hits for the roll;
  `ZamorakMixEffect` heals 6 before hitting for `(int)(lp * 0.10 + 2)`;
  `RestoreEffect` restores only levels below the static level but adds the
  boost to prayer points in every case; `PrayerEffect` adds 0.02 to the bonus
  with a holy wrench (6714) in the inventory; `SetAttributeEffect` adds the
  tick to an `Integer` value (a `Boolean`/`String` with `isTicks` fails as a
  cast); `KebabEffect` draws `nextInt(100)` and, in the 87..95 band, a second
  `nextInt(100)` and a skill `nextInt(23)`; `SuperKebabEffect`'s skill draw
  is `nextInt(24)`.
- `getHealthEffectValue` draws again for the random effects (`RandomHealth`,
  `SmellingUgthankiKebab`, `SuperKebab`); `FakeConsumable` has no effect and
  fails as a null.

## Required services

Everything outside these classes is a method of `consumables::Host` with no
default, called in the original order: the tick, attributes, animations,
audio, packet-dispatch and ContentAPI messages, `manualHit`, chat,
teleports, the pulse manager, diary tasks, the familiar and its special
points, inventory/equipment `replace`, `remove`, slot `remove` and
`contains`, the DUPE_ALERT log, timer spawn/register/remove/get, the script
queue (active script, `addToQueue`, `reset`), `log`, `delayNextAttack` and
`RandomFunction.RANDOM.nextInt`. Plain accessors (not calls): the native
skills and settings, the clocks array, an item id's definition name and the
disease timer's `hitsLeft` field.

## Joins

- **Skills, run energy.** Native (`Host::skills`, `Host::settings`); the
  world binds them to the player's `skills::Context`/`settings::Context`.
- **Timers.** `consumables::join` answers the timer services from the native
  engine timers (`timers::content`, docs/content-timers.md):
  `spawnTimer(identifier, args)` is the registry's
  `instance(identifier, args)`, `removeTimer(identifier)` is
  `Manager::remove_identifier`, `getTimer<Disease>`/`removeTimer<Disease>`
  are `content::get`/`remove_where` of `Kind::Disease`, and `hitsLeft` is the
  timer's `State::Disease` field. `tests/consumables_timers.rs` drinks an
  antipoison and relicym against a real `timers::Manager` with the registry
  of `content::register_prototypes`: a PoisonImmunity of 150 ticks is
  registered and the active Poison queued for removal, the Disease's hits go
  from 12 to 3 and then the disease is cured and removed.
- **Inventory.** `replace`, `remove`, `remove_slot`, `contains` are the
  native `inventory::container` members of the player's inventory and
  equipment (`replace(item, slot, true)`, `remove(item, None, true)`,
  `remove(item, Some(slot), fire)`, `contains(id, amount)`).
- **Item options.** A world registers [`handle`] for `IntType.ITEM` options
  "eat" and "drink" (`define_listeners` names them) in its InteractionListeners
  registry; `getUsedOption` is the "interact:option" attribute the dispatch
  sets.
- **Scripts and pulses.** `add_to_queue` receives the potion's `DoseScript`
  with strength SOFT and the next execution; the world's script processor
  runs `DoseScript::run` at that tick. `run_pulse` receives a `RumPulse`
  (delay 4, then 6) the world schedules on `server::pulses` and fires with
  `RumPulse::pulse`.
- **Combat.** `delay_next_attack` is the player's combat pulse
  (`world/combat_pulse`); `manual_hit` is `combat_pulse::impact::manual_hit`.

## Reference tests

```powershell
cargo test -p rs09-server --test consumables --test consumables_timers
cargo xtask consumables-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK_11
```

`fixtures/consumables/` holds **3,867 original JVM cases (2,735 designed and
1,132 single injected failures) with 6,426 operations and 27,282 recorded
calls**, and the hashes of every source. The oracle compiles the nine
consumable classes, `Consumables.java`, all 34 effect classes,
`ConsumableListener.kt`, `Script.kt`, `Clocks.kt`, `IntType.kt`,
`core/api/Container.kt`, `Pulse.java` and `TickUtils.kt` unchanged with
Kotlin 1.8.20 on JDK 11, JSON-simple 1.1.1 and the published ConstLib
1.4.28 (Sounds and Items values checked against their name suffixes).
Original text inserted at generation time: the ContentAPI functions listed
above, Skills' constants, prayer field and members, Settings' run energy
field and `updateRunEnergy`, `RandomFunction`'s three members, the Log,
HitsplatType, DiaryType and LogType constants and `Item.toString`. Every
other class is a recording stand-in in `oracle/consumables/adapters.py`;
`RandomFunction.RANDOM` is a seeded recording `java.util.Random`.

The corpus covers the table (every row's reflected fields, every id's
lookup, the potions list), every row's every distinct id eaten or drunk
through the listener (with the potion script and the rum pulses), the
clock gates at, below and above the tick and at the integer limits,
consecutive food/karambwan/potion/drink sequences, option values and node
kinds, the consume paths of every class (stale and foreign slots, stacks,
the 65535 id, last ids, empty containers, the DUPE_ALERT fallback, slot
bounds), dose scripts with persistent scripts, every effect class with edge
arguments over 22 player states, health values of every row, 119 name
formats, `getDose` over ids and names, unknown timer identifiers, the random
effects over many seeds and at each outcome boundary (seeds chosen for the first draw), the holy wrench with every prayer row at four prayer levels, Saradomin brew overheals with and without the damage sponge, and 60 random sequences. The native test replays
every row and compares each operation's result, error, recorded calls and
draws and the player state whole; `GAPS` is empty. 20 of 20 deliberate breaks of the port (clock offsets, the 65535 id, the id mask, dose messages, the second active-script read, the restore and prayer bonuses, kebab and super-kebab draws, the cake message index, the halves' second id, `putIfAbsent`, a table value, Java trim, removal of a cake) fail the replay; the holy-wrench, kebab-boundary and overheal cases were added after the first round caught them in only one to three cases.

## Differences from the original

- The consumable objects are values; the table is built once per world
  (`Table::new`) instead of a JVM-wide enum.
- Item names are the definitions' names read through `Host::item_name`
  (the original reads them when an `Item` is constructed).

## Not ported

`ConsumableOptionPlugin` (not `@Initializable`; its body is commented out),
the decanting/herblore/bot/familiar/lunar callers of the table, and every
binding to a live session (the joins above).
