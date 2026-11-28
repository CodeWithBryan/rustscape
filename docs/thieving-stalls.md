# Market stalls (`world::thieving_stall`)

Scope: the pinned `content/global/skill/thieving/Stall.java`,
`StallThiefPulse.java` (with `SkillPulse`'s `start`, `pulse` and `stop` and
`Pulse.update`) and `ThievingOptionPlugin.java`.

| original | here |
|---|---|
| `Stall` (18 constants: full and empty ids, level, rewards, experience, delay, the message's item) | `STALLS` |
| `Stall.forObject`, `getEmpty` | `for_object`, `Stall::empty` |
| `ThievingOptionPlugin.handle` | `handle` |
| `StallThiefPulse` over `SkillPulse` | `StallThiefPulse` (`start`, `update`, `pulse`, `stop`, `check_requirements`, `reward`, `message`) |
| `StringUtils.isPlusN` | `is_plus_n` |

The bodies are functions over `thieving_stall::Host`; every call the
original makes outside these classes is a required method with no default.

Not ported: doors (`PickableDoorHandler`). Pickpocketing is
docs/thieving-pickpocket.md and chests are docs/thieving-chests.md.

## Evidence, and what it does not show

- `fixtures/thieving-stalls/oracle.jsonl` (1,842 cases, about 12,200
  operations) is recorded from the unmodified Java sources compiled with
  Kotlin 1.8.20/JDK 11 (`oracle/thieving-stalls/generate.py`, `cargo xtask
  thieving-stalls-oracle`), and `cargo test -p rs09-server --test
  thieving_stalls` replays every case over this module and compares each
  result and each recorded call in order. One operation compares every
  constant's fields; others ask `forObject` for every listed id and its
  neighbours and `getEmpty` of every constant for every listed id.
- What the corpus does not establish (see `scope` in
  `fixtures/thieving-stalls/provenance.json`): the player, levels, combat
  state, inventory, attributes, region, quest answer, animation, locks,
  audio, the saved silk stamp, the scenery (name, activity, `transform`),
  `SceneryBuilder.replace`, the world tick, the local NPCs with their
  combat pulses, item names, `stun`, `impact`, `sendDialogue` and the
  dispatched event are recording adapters answered from declared case
  state. `PulseManager.run` is an adapter that starts the pulse: what the
  manager clears is not covered, nor what `stun`'s `pulseManager.clear()`
  does to the running pulse. The driver calls the plugin's `handle` and the
  pulse's `update` and `stop` itself; no pulse runner or option dispatch is
  run. The generator is not in `oracle/ci.toml`: CI replays the committed
  fixture but does not regenerate it.
- `cargo test -p rs09-server --lib thieving_stall`: one table check.
- The live world's `gather` scenario (`stallman`) uses the Varrock tea
  stall's "Steal-from" option at Thieving 40: the attempt's message, a cup
  of tea with its message and 16 experience, a second click that finds no
  full stall on the tile, and a second cup once the twelve ticks have
  passed. No live check asserts a failed attempt, a guard's attack, the
  candles, a requirement refusal, the Rellekka refusal, the interaction
  lock or the silk stamp.

## Kept behaviour worth knowing

- `idMap` is filled with `putIfAbsent`: 6166 is listed by the crafting stall
  and the spice stall, and is the crafting stall's.
- The vegetable stall lists two full ids and one empty id. A theft from 4708
  plays the sound, then `getEmpty` throws inside the pulse: `Pulse.update`
  prints it and drops the pulse, so nothing is handed over.
- `hasInactiveNode` only asks the pulse's nodes while the `thieveDelay`
  attribute is later than the world tick. `start()` sets it to the current
  tick, so the check never runs unless something else moves the attribute.
- The requirements are checked at the start and again on every tick of the
  pulse. The first tick animates (832) and locks interactions for two ticks;
  the option itself locks them for six. The third tick decides.
- `success()` draws `RandomFunction.random(15)`; below 4 the candles smite
  (a 15 tick stun without a message, a hit of 1, "A higher power smites
  you") and any other stall calls the first NPC within eight tiles that is
  not attacking and has id 32 or 2236. With no such NPC the theft succeeds.
- A success replaces the scenery only while it is active, then draws the
  loot, adds it without reading the inventory's answer, and rewards the
  experience. The candles say nothing; the grapes message is unreachable (no
  constant rewards 1987).
- The article is `isPlusN` of the item's name: an "h" not followed by "e"
  takes "an". An empty name throws inside the pulse.
- The Rellekka checks run only in region 10553, and ask the quest before
  they look at the stall's ids.

## Live world

`world/live/items/stalls.rs` registers the plugin for the three option
names at boot when `ThievingOptionPlugin` is in the scan, and runs the
pulse as a STANDARD pulse of the player's pulse manager. `Scenery.transform`
and `SceneryBuilder.replace` are the cache scene's
(`world::scenery`), so the empty stall is seen by every viewer and restored
after the constant's delay.

No quest is ported: `isQuestComplete` answers false by name, so the Rellekka
fur and fish stalls always refuse there. The ResourceProducedEvent is a
named skip. `stun` is composed in the binding (docs/thieving-pickpocket.md);
its `pulseManager.clear()` reaches the running pulse in the original, and
the binding makes that pulse's animation reset itself before the stun. A
body that throws is logged and the pulse dropped, as `Pulse.update` does.
The binding has no JVM corpus.
