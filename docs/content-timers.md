# Engine timers

`timers::content` ports the timers the engine itself defines in
`core/game/system/timer/impl`: `SkillRestore`, `Poison`, `PoisonImmunity`,
`Disease`, `Frozen`, `FrozenImmunity`, `Teleblock`, `Skulled`,
`HealOverTime`, `Miasmic`, `MiasmicImmunity`, `DragonFireImmunity`,
`SpellbookSwap` and the timer part of `AntiMacro`, together with the
`ContentAPI` glue they and their callers use. It runs on the generic timer
manager and registry (`timers`, `timers::registry`, docs/timer-registry.md):
`Natives` is their `Context` and `Factories`. It is a library. Nothing in a
session, the tick driver or the start-up installs it yet.

## What is ported

| Original | Native |
| --- | --- |
| Each class's constructor (interval, identifier, `isSoft`, `isAuto`, `ClearOnDeath`), `retrieveInstance` and `getTimer(vararg args)` | `Kind::instance`, `Factories::instantiate` |
| Each class's `beforeRegister`, `onRegister`, `run`, `onRemoval`, `save`, `parse` | `Natives`' `Context` members, per class in `combat.rs`, `restore.rs`, `swap.rs`, `antimacro.rs` |
| The classes' own fields (severity and source, hits left, ticks remaining, immunity flag, heal amounts, restore counters, pause and forced event) | `State`, kept on the timer (`Timer::content`), created with the class's initialisers on first use |
| `SkillRestore.PrayerActivatedHook`/`PrayerDeactivatedHook`, SpellbookSwap's three hooks and its logout listener | `process_hook`, `logout` |
| `ClassScanner.loadTimers` for these classes (scan order) | `register_prototypes` |
| `ContentAPI`: `spawnTimer<T>`, `getOrStartTimer<T>`, `hasTimerActive<T>`, `getTimer<T>`, `removeTimer<T>`, `hasTimerActive(entity, id)`, `applyPoison`, `isPoisoned`, `curePoison` | `spawn`, `get_or_start`, `has`, `get`, the manager's `remove_where`, `has_identifier`, `apply_poison`, `is_poisoned`, `cure_poison` |
| `AntiMacro.pause`, `unpause`, `forceEvent`, `getEventNpc`, `terminateEventNpc`, `rollEventLoot`, `parseCommandArgs` | `antimacro::*` |
| `core.tools.colorize`, `RandomFunction.random(a, b)`/`random(max)` | `colorize`, `random_between`, `random` |

Skill levels, lifepoints and healing are the native `player::skills::Skills`
(`heal`, `updateLevel`, `getLevel` with its assist read, `getMaximumLifepoints`
with its damage-sponge read); special energy is the native
`player::settings::Settings` (`setSpecialEnergy` writes varp 300).

## Behaviour that is easy to get wrong

- `Miasmic.run` spawns its immunity with `spawnTimer<MiasmicImmunity>(entity,
  7)`: the first argument is the entity, so `getTimer` falls back to 100
  ticks, not 7. `Frozen.run` spawns `FrozenImmunity(7)` only when
  `applyImmunity` is set, otherwise it debugs "Can't apply immunity".
- `Poison` deals `(severity + 4) / 5` with the severity read before its
  decrement, reads `damageSource` before that (an unset source throws
  Kotlin's `UninitializedPropertyAccessException`, also from `onRegister`
  after "You have been poisoned." and from `save`), and only players get the
  "worn off" message. `getTimer` with one entity argument fails on
  `args[1]`; a first argument that is not an entity leaves the source unset.
  `onRegister` reads the NPC's slayer task twice and compares its level with
  the source player's (or the familiar owner's) Slayer level, removing the
  timer and debugging `"[Poison] -> Removed poison from <name> [x, y, z] due to
  Slayer reqs!"` to that player. `save` writes `source-uid` as a JSON number
  (the player's uid, -1 for any other source); `parse` falls back to the
  entity itself when no player has the uid.
- `applyPoison` checks the immunity timer, then `isPoisonImmune`, then keeps a
  stronger existing poison; a weaker one gets the new severity and source
  without a message or a new registration.
- `Disease` draws `random(1, 5)` (1 to 4) damage, hits with the entity as
  source, then drains a random skill (`random(24)`, hitpoints moved to
  strength) to a floor of 0. A second disease removes itself (the old one
  keeps running) without the "You have become diseased." message; the end
  message is "The disease has wore off.".
- `Frozen.beforeRegister` removes the new timer when the entity is immune or
  already frozen (it still lands in the pending list and leaves on the next
  process); `FrozenImmunity.onRegister` removes any `Frozen`.
  `Frozen.save` writes `applyImmunity` as a JSON boolean and `parse` accepts
  only a JSON boolean.
- `HealOverTime.onRegister` removes itself when `Miasmic` or
  `MiasmicImmunity` is active (twice when both are), and heals
  `min(healRemaining, healPerTick)` per interval after lowering the
  remainder. `MiasmicImmunity.onRegister` removes `Miasmic`;
  `Miasmic.beforeRegister` removes the new timer under either.
- `DragonFireImmunity` warns at exactly 50 ticks left (sound 3120, three
  loops) and expires at 0 (sound 2607); `PoisonImmunity` removes `Poison` on
  registration and expires at 0. Both are soft and keep counting while a
  modal or a script delay stalls the others.
- `Skulled.run` resets the skull and its removal resets it again.
- `SkillRestore` skips prayer and summoning, increments each counter before
  comparing it (even when the restore then fails; counters wrap), heals
  hitpoints by 1 (2 with the regeneration bracelet 11133 in the hands slot)
  while below the maximum, and otherwise moves a level one step towards its
  static level. Special energy rises by 10 (capped at 100) every 51 ticks
  whatever `restoreTicks[24]` holds, and the varp is written even at 100.
  Rapid Heal sets only the hitpoints interval to 50, Rapid Restore every other
  restored skill; deactivating sets 100; both restart the counters. The hooks
  `getOrStartTimer<SkillRestore>`, starting one when none runs.
- `SpellbookSwap` registers three hooks per instance and a logout listener.
  Casting Lunar spell 12 is ignored; any other cast reverts and removes the
  timer; a book change from anything but the swap's own sources removes it; a
  closed dialogue whose ids contain 3264731 removes it (and `spell:runes`)
  while the book is Lunar. Reverting does nothing on the Lunar book; otherwise
  it forgets the autocast spell (`selectAutoSpell(-1, true, true)`, only with
  a weapon interface and an autocast spell), closes the autocast selection,
  dispatches `SpellbookChangeEvent(old, LUNAR, SPELLBOOK_SWAP_RESTORE)`, sets
  the book field and runs `SpellBookManager.update` (tab, varbit 357). A
  book field naming no book fails as a null before the dispatch. It has no
  `ClearOnDeath` flag.
- `AntiMacro.onRegister` removes the timer from an NPC or an artificial
  player, pauses it for an administrator and draws `random(3000, 9001)` while
  the interval is 0. Every run draws a new interval first; it then checks the
  zone restriction, an existing `re-npc` attribute (a value of another class
  fails as a cast) and the pause, and picks a forced event or (skill draw,
  non-skill draw, `random(100)`) the skill event when the roll is at least 65
  and experience was gained less than 250 ticks ago. `save`/`parse` use
  `ticksRemaining` relative to the tick; `parse` takes 0 for anything
  `toIntOrNull` refuses.

## Required services

Everything outside these classes is a method of `content::Host`, called in
the original order, with no default: the tick, modal and script-delay reads,
weak-script removal, names and locations, logging and printed stack traces,
messages, debug messages, audio, `RANDOM.nextInt`, `manualHit`, equipment,
artificial flag and rights, uids and the uid lookup, slayer tasks, familiar
owners, poison immunity, the skull manager, `Entity.hook`/`unhook`/`dispatch`,
logout listener registration, the weapon interface extension and its
`selectAutoSpell`, the autocast spell, tabs, varbits, attributes, the zone
restriction, the random event pool, event NPC creation, spawn location, init,
terminate and loot, and dialogue ids. Plain accessors (not calls): entity
kind, the native skills and settings, the spell book field and the random
event names.

The host owns each entity's hook map. Its `dispatch` runs every hook of the
event's class from a copy of the list (as `Entity.dispatch` does) and calls
`process_hook` for these timers' hooks with the entity's timer manager.

## Joins

- **Save file.** `Natives` implements `timers::Context<S>` for any state, so
  it is the `T: timers::Context<CoreData>` the save path takes
  (`CoreData::save`/`parse`, `player::save`, `owned::profile`). The test
  writes the `core_data.timers` section of a player running all fourteen
  timers through `CoreData::save` and gets the original's `saveTimers` text,
  and loads the original's section through `CoreData::parse`.
- **Spells, teleports, walking, zones.** Their `hasTimerActive(entity,
  "teleblock")`, `"frozen"` (and any other identifier) services are
  `has_identifier(manager, id)`; `combat_pulse::Host::miasmic_active` is
  `has(manager, Kind::Miasmic)`. A spell's `fireEffect` starts these timers
  with `spawn`/`get_or_start` and the manager's `register_optional`.
- **Combat.** `combat_pulse::Host::apply_poison(victim, source, severity)`
  is `apply_poison` on a `Natives` for the victim. `Host::manual_hit` is
  `combat_pulse::impact::manual_hit` in a world that has both.
- **Prayer.** `prayer::Host::dispatch(player, PrayerEvent)` becomes the
  host's dispatch of `Event::PrayerActivated`/`PrayerDeactivated`, which runs
  SkillRestore's hooks through `process_hook`.
- **Start-up.** `startup::timers` still registers its own three prototypes
  (and only AntiMacro's NPC branch); `register_prototypes` registers these
  fourteen in the class-scan order.

## Reference tests

```powershell
cargo test -p rs09-server --test content_timers
cargo xtask content-timers-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK_11
```

`fixtures/content-timers/` holds **2,311 original JVM cases (1,163 designed
and 1,148 single injected failures) with 15,042 operations and 227,497
recorded calls**, and the hashes of every source. The oracle compiles
`RSTimer`, `PersistTimer`, `TimerFlag`, `TimerManager`, `TimerRegistry`, the
fourteen classes, `SpellBookManager`, `Event`, `EventHook`, `EquipmentSlot`,
`Privilege`, `Globals` and `TickUtils` unchanged with Kotlin 1.8.20 on JDK 11,
kotlin-reflect (`createInstance`), JSON-simple 1.1.1 and the published
ConstLib 1.4.28 (Sounds and Items values checked against their name
suffixes); the fourteen classes register once each through their own
constructors in scan order. Original text inserted at generation time: the
ContentAPI timer, poison, level, equipment and attribute functions; Entity's
hook map with its `dispatch`, `unhook` and `hook` bodies; Skills' constants,
fields and the members above; Settings' special energy members; Player's
logout listener map; `RandomFunction.random`; the Log, Rights, PrayerType,
ZoneRestriction and HitsplatType constants; `Location.toString`; the five
hooked event declarations and their `core.api.Event` constants; the
`RandomEvents` names with their loot and type arguments. Every other class is
a recording stand-in in `oracle/content-timers/adapters.py`; stand-ins for
paths the corpus does not drive throw. `RandomFunction.RANDOM` is a seeded
recording `java.util.Random` and printed stack traces are recorded by
exception class.

The corpus covers every class's arguments (18 shapes) and identifiers,
lifecycles on players, NPCs and familiars until expiry, soft and normal
timers under modals and script delays, poison severities to the integer
limits, every slayer branch, `applyPoison`/`curePoison` over immunities and
existing poisons, unset sources, disease over five seeds, skill restoration
over levels, bracelets, the damage sponge, special energy and every prayer
event, automatic installation, wrapping counters, freezing and immunities,
miasmic and healing interplay, teleblock and skull, spellbook swap over four
books, weapon/autocast/selection states, eleven events, logout and expiry,
foreign hooks before and after its own, AntiMacro registration, runs (the skill-event roll at 64, 65 and 66, experience gained 249 and 250 ticks earlier),
companions, persistence and command parsing, every class's persistence
(valid, missing, malformed and out-of-range values) through `saveTimers`,
`parseTimers` and the members, death, and 160 random sequences. Expected rows
hold the result, the error class, the ordered calls and draws, each entity's
state (skills, special energy, book, timer lists, attributes, hooks, logout
listeners) and every labelled timer's fields whenever they changed. The
native test replays every row and compares it whole; `GAPS` is empty.

## Differences from the original

- The timers' fields live in `Timer::content` and are created with the
  class's initialisers on first use; a timer of another class has none.
- Kotlin's `UninitializedPropertyAccessException` is
  `Error::Binding(UNINITIALIZED)`; `ClassCastException` is `Error::Type`.
- `getTimer` arguments are `Arg`: an `Int`, a `Boolean`, an entity or
  anything else (which no `as?` cast in these classes accepts).
- `parseCommandArgs` uppercases with Rust's Unicode mapping; the corpus
  covers ASCII and Latin-1 letters, not every character where Java's ROOT
  uppercase differs.

## Not ported

`AntiMacro.defineCommands` (the `::revent` command), the random event content
(`RandomEvents` pools, `RandomEventNPC`), the content timers of other
packages (farming, cannons, ...), `Entity.hook`/`dispatch` themselves (host
services), `Entity.init`'s `addAutoTimers` call site, `finalizeDeath`'s
`onEntityDeath` call site and the tick loop. Installing the registry and
these bindings in a world, and answering the spells, teleport, walking,
combat and prayer services from them in a session, are the next steps; all
source inventory rows remain `not_replaced`.
