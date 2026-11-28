# Gathering skills: tables, woodcutting, mining and fishing

`world::gathering` ports, as a library, the pinned `SkillingTool`,
`WoodcuttingNode`, `MiningNode` and `SkillingResource` tables, the
`RandomFunction` methods they use, `WoodcuttingListener`,
`WoodcuttingSkillPulse`, `MiningListener`, `MiningSkillPulse`,
`ProspectListener`, and for fishing the `Fish`, `FishingOption` and
`FishingSpot` tables, `FishingListener` and the legacy `FishingPulse` with the
`SkillPulse` members it inherits (`fishing.rs`). `binding.rs` joins the
library to the native skills and item containers (see "Binding"). Of all
this woodcutting runs in the live world: `WoodcuttingSkillPulse`, started by
an axe used on a tree, and `WoodcuttingListener`, whose `defineInteraction`
metadata is in the native listener registry and whose handler runs as an
interaction script of the player's ScriptProcessor
(`world/live/items/woodcutting.rs`, a `service::Context` written without a
corpus of its own, and `world/live/scripts.rs`: docs/live-world.md). The
mining and fishing listeners are not registered there, and the controlled
examples do not use
the library: the bronze-axe stand-in under
`crates/server/examples/common/skilling.rs` is unchanged and is still not a
port. **Not ported:** barbarian fishing (`fishing/barbfishing/`),
`PickaxeRepairPlugin`, fishing-spot movement (`FishingNPC`, `FishSpots`) and
the option handlers that start `FishingPulse`.

## Shape

Every call the original bodies make into a system outside this slice is one
`service::Service` request, issued in the original order with the original
arguments, through `service::Context::call`. Scenery replacement, animation,
audio, messages and dialogue, the experience hand-off, containers, equipment,
item names, diaries, familiars, perks, timers, farming patches, bird nests,
firemaking data, projectiles, the script queue/clock helpers and the pulse
manager are all such requests. The library performs none of them and supplies
no defaults; a failing service stops the body with its completed prefix in
place. Deferred bodies (the one-chop branch reward, the farming-patch
reactivation, the prospect message) are typed `service::Task` values handed to
`Context::defer`. Service names exist for recording and comparison only.

The server's generators are services too, and each method draws from the same
one as the original: `RandomFunction.RANDOM` (`rng.shared`), `Math.random()`
(`rng.math`), `ThreadLocalRandom.current()` (`rng.threadLocal`, behind
`RandomFunction.randomDouble` and Kotlin's `Array.random()`), and the private
generator of `Collections.shuffle` (`rng.shuffle`). `random::ThreadLocalRandom`
is the JDK 11 algorithm for owners and tests. World inputs read from static
configuration are explicit: `Settings::max_players` and
`mining::State::gem_rock_gems`.

`tables.rs` holds the enum constructor arguments in declaration order and is
written by `tools/port_gathering.py` from the pinned sources; the constructor
switches are ported by hand in `nodes.rs`.

## Preserved behaviour

- `SkillingTool.checkTool` gates on the static level, then the weapon slot,
  then the inventory. The inferno adze replaces the best hatchet or pickaxe
  only when it passes that check and the *current* Firemaking level is at
  least 92. `getHarpoon` walks the butterfly-net constants against Fishing.
  `forId(13661)` is `INFERNO_ADZE`, the first of the two constants.
- Respawn duration divides by `MAX_PLAYERS / players` as doubles: an empty
  repository yields the minimum, and `0 / 0` yields zero.
- The woodcutting success roll is one thread-local draw compared with
  `getSkillSuccessChance`; the black axe halves both tier modifiers. In the
  listener Kotlin checks a null resource or tool where they are passed, before
  anything is read; the Java pulse fails only at `tool.ordinal()`, after the
  level reads and the draw.
- The pinned JDK 11 `ThreadLocalRandom.nextDouble(origin, bound)` computes
  `Math.nextAfter(bound, origin)` on rounding and discards it, so the bound can
  be returned. This is kept.
- The listener and the pulse differ and each keeps its own source: the sound
  index is drawn after (listener) or before (pulse) the local players are read;
  the adze burns on `roll(4)` versus `random(100) < 25`, with different
  experience and projectile plane; bark pays 82.5 or nothing versus 275.2 or
  the base amount; a full inventory is a message versus a dialogue; identifiers
  1, 2, 3, 4 and 6 always fall versus 1, 2, 3 and 6; only the listener reads
  `instachop`, queues the patch reactivation and starts the stump timer.
  Bark with amount zero is still added to the inventory with amount zero.
- Dramen and swaying trees reward once through a queued STRONG script and end.
- `MiningListener.checkReward` always succeeds for essence; the pulse has no
  such shortcut and re-reads the pickaxe between its two `Math.random()` draws.
  Varrock armour passes below 4, 3 and 2 in the listener and at or below 10 in
  the pulse. The Family Crest area exists only in the listener; the perfect
  gold location replacement only in the pulse.
- Gem rocks roll `MiningNode.gemRockGems`, a static list that every roll
  shuffles in place, with the `rand <= weight` comparison.
- Fishing. `FishingSpot.getOptionByName` lowers the used option with ROOT
  rules and takes the first matching option of the spot; a node that is not an
  NPC, an unknown spot and an unknown option each end with `clearScripts`.
  `rollFish` draws `randomize(100)` for the big net first (0, 50 and 90 are the
  oyster, casket and seaweed), reads the dynamic level once, adds the familiar
  boost with wrapping, skips fish above the unboosted level, asks for the
  stripy feather once per lure fish, and draws one shared `nextDouble` per
  remaining fish, compared with `<`. `hasBait` stops asking once a bait is
  held; `removeBait` tries the last bait first. The missing-tool text tests
  the tool name for "net" in any case at any UTF-16 offset and asks for the
  name a second time otherwise; the missing-bait text asks for the bait name
  twice and for the name of item 313.
- The fishing listener and the pulse differ and each keeps its own source: the
  listener checks space with `hasSpaceFor` and counts with
  `incrementAttribute` (twice on a doubled catch), rolls `roll(20)` for
  GREAT_AIM after the catch and the big fish with `roll(5000)` (10 in
  development mode, read on every catch), pays through `rewardXP`, and has
  barehand fishing (the unlock attribute, both level pairs, per-fish
  requirements, Strength experience, the Fremennik task for tuna). The pulse
  writes the tool check to `player.debug`, checks space on the container,
  rolls `random(100) <= 5` before the catch is added, reads and writes the
  counter attribute, pays with the three-argument `addExperience`, decreases
  skill tasks, and compares the spot `Location` by identity. Its first
  `reward` only changes the delay from 1 to 5.
- `FishingPulse.animate` hands a `Pulse(1)` that never finishes to the world
  pulser on every barehanded call; the body acts once, when its counter is
  five, and `RandomFunction.random(1)` there draws `nextInt(1)`, which is never
  one, so levels 70/50 and above always take the swordfish branch.
- `forId` keeps the first declared row for a repeated id in the two node enums
  and the last in `SkillingResource`; the four-argument `WoodcuttingNode`
  constructor leaves the reward amount at zero unless its switch sets it.
  The pinned rows have no repeated node or resource id and every farming row
  sets its amount, so these two rules are ported from the source but have no
  corpus evidence.

## Verification

`cargo xtask gathering-oracle` compiles the unmodified sources above, plus
`ChanceItem`, `WeightedChanceItem`, `DropFrequency`, `Pulse`, `Clocks`,
`IntType`, `StatAttributeKeys`, `EquipmentSlot`, the `Container` enum and
`Globals.kt`, with Kotlin 1.8.20 on JDK 11 against the pinned ConstLib 1.4.28
artifact (`oracle/bank/constants.lock.json`). Adapters live under
`oracle/gathering/adapters`. The fishing sources (`Fish.kt`, `FishingOption.kt`,
`FishingSpot.kt`, `FishingListener.kt`, `FishingPulse.kt`, `SkillPulse.java`,
`SkillTasks.java`, `Difficulty.java`) are compiled unmodified as well.
`fixtures/gathering/oracle.jsonl` holds **10,504 cases** and the source
hashes. The first 9,281 lines are the woodcutting/mining corpus as first
recorded, byte for byte (SHA-256 `45f37cb6…184570ff`, asserted by the test);
fishing is appended after them:

| Cases | Content |
| --- | --- |
| 88 / 391 / 481 / 36 | every `WoodcuttingNode`, `MiningNode`, `SkillingResource` and `SkillingTool` row, read from the real enum objects |
| 28 | `forId`/`isEmpty` over 1,649 keys (every id, every empty id, their neighbours, signed extremes) |
| 992 | respawn duration for each distinct range across population and cap boundaries |
| 315 | 17,878 `RandomFunction` calls: bounds, ranges, rolls, the success chance for levels -1 to 101 and extremes, thread-local doubles at unit boundaries, gem-table rolls |
| 2,952 | tool selection: every tool below, at and above its level, wielded, carried, both or in the wrong slot |
| 838 / 408 | woodcutting listener / pulse scenarios |
| 1,480 / 870 / 401 | mining listener / pulse / prospect scenarios |
| 31 / 17 / 11 + 1 | every `Fish`, `FishingOption` and `FishingSpot` row and the counts with `getAllIds`, read from the real enum objects |
| 5 | fishing lookups: `forId` over 205 keys, `forItem`, `forName`, `getSuccessChance` for every fish at eleven levels, `getOptionByName` for every spot and 19 spellings |
| 734 / 424 | fishing listener / pulse scenarios |

The scenarios are 19,809 operations with 339,468 recorded service calls, of
which fishing is 6,899 operations and 56,498 calls. The fishing scenarios
cover every spot id through each option it offers, every option catching
nothing and each of its fish by designed draws, the option and fish level
gates, requirement order and texts (tool and bait names in several cases and
scripts), the used-option spellings, non-NPC and unknown nodes, distance and
plane, the big fish with and without development mode, GREAT_AIM, the gloves,
barehand fishing around every threshold, foraging familiars, long runs until
the inventory fills or the bait runs out, the pulse members called directly
(null option, `checkFish` around each level pair, the submitted catch pulse)
and 163 injected service failures. The woodcutting and mining scenarios
cover every node row, requirement order and messages, level gates, success
draws exactly on the chance and one unit below it, designed draws for felling,
nests, bark, the adze, Varrock armour, the star bonus, the bonus gem and
jewellery, sandstone and granite sizes, essence, the clay bracelet, long runs
until the node is spent or the inventory fills, and 299 injected service
failures at successive positions of full passes.

## Binding

`binding::Host` is a `service::Context` over a player's native
`player::skills::Skills`, the inventory and equipment
`inventory::container::Container`s and the item definition registry. It
answers these requests from that state, with the pinned ContentAPI bodies
where the original goes through one, and hands every other request to the
owner's `required` context unchanged:

| Bound to | Requests |
| --- | --- |
| native skills | `skills.getLevel`, `skills.getStaticLevel`, `skills.addExperience2/3`, `api.getDynLevel`, `api.hasLevelDyn`, `api.rewardXP` |
| native inventory container | `inventory.freeSlots`, `inventory.contains`, `inventory.add`, `inventory.hasSpaceFor`, `api.freeSlots`, `api.inInventory`, `api.hasSpaceFor`, `api.addItemOrDrop`, `api.addItem` and `api.removeItem` for `INVENTORY` |
| native equipment slots (a base container of 14) | `equipment.get`, `equipment.getNew`, `api.getItemFromEquipment`, `api.addItem` and `api.removeItem` for `EQUIPMENT`, and the skills' own `containsItem` checks |
| native item definitions | `api.getItemName`, `itemDefinition.name`, `item.name` |

The callbacks the native skills and containers make themselves stay required
services of the owner: `player::skills::Context` (clock reads, packets,
level-up, XP plugins and events, appearance, combat level, news, XP log,
assist, brawling gloves, perk, tutorial flag) and
`inventory::container::Context` (listener `update`/`refresh`). A refused
`addItemOrDrop` asks the new required service `groundItemManager.create`.

**Still required services, not bound:** the script queue and clocks
(`finishedMoving`, `clockReady`, `delayClock`, `delayScript`, `keepRunning`,
`restartScript`, `clearScripts`, `queueScript`), the pulse manager and world
pulser, listener registration, attributes, messages and dialogue, animation,
audio, scenery, NPC and familiar state, perks, diaries, timers, farming,
generators, `api.inEquipment` (the original reads the item configuration's
equip slot), bank and beast-of-burden containers. The native `pulses`,
`scripts`, `interaction_listeners`, `action_locks` and `equipment`
(`EquipmentContainer` with its listener) modules are not joined: doing that
against the original needs `ScriptProcessor`, `PulseManager`, the listener
registry and `EquipmentContainer` in the same oracle, which this slice does
not have.

`fixtures/gathering/binding.jsonl` holds **362 cases** (8,794 operations,
30,285 recorded calls, 39 injected failures; 111 / 12 woodcutting listener /
pulse, 74 / 24 mining, 116 / 25 fishing). `oracle/gathering/binding.py`
compiles the same unmodified gathering sources against the complete unchanged
`Skills`, `Container`, `ContainerEvent`, `ContainerListener`, `ContainerType`,
`SortType`, `Item`, `Definition` and `BrawlingGloves`, verbatim selected
`ItemDefinition` and `Node` members, and the pinned texts of `inInventory`,
`hasLevelDyn`, `hasSpaceFor`, `freeSlots`, `addItem`, `addItemOrDrop`,
`removeItem`, `rewardXP`, `getDynLevel`, `getItemFromEquipment` and
`getItemName`. The native replay builds the same player from the case, runs
the ported bodies through `Host`, and compares after every operation all 24
dynamic and static levels and experience bits, lifepoints, prayer points,
gained experience and total level, every inventory and equipment slot (id,
amount, charge, in slot order), and the recorded calls: messages, the skills'
packets with the level and experience at send time, 86 level-ups, XP events
and plugins, the 200m news and the XP log, 644 listener updates with their
changed slots, and three ground-item hand-offs.

Limits of the binding evidence:

- Equipment is the base original `Container(14)`, not `EquipmentContainer`;
  the containers carry one recording `ContainerListener` each, not
  `InventoryListener`/`EquipmentListener`.
- No assist session and no brawling gloves exist in the corpus; the tutorial
  cap, level-ups across several levels, boosted levels and 200m are covered.
- `api.removeItem` carries only the item id, so the host removes one unit with
  the default charge. This equals the original for the single unstackable
  items the gathering sources remove; a stacked or charged equipment item
  would differ.
- `equipment.get`, `skills.addExperience2` (the same host arm as
  `api.rewardXP`) and `api.addItem` are bound but were not requested by any
  replayed body; the corpus is no evidence for them.
- No failure is injected on the two clock reads, which the native skills
  cannot fail.
- The script driver is still the modelled one, so "per tick" means per
  modelled tick.

`crates/server/tests/gathering.rs` replays both corpora. Recorded service
results are a tape: each native request must match the next recorded name and
arguments and receives the recorded result. Generator draws are not taken from
the tape; `JavaRandom` and `ThreadLocalRandom` run natively from the case seeds
and every draw is compared. Thread-local draws cannot be intercepted in the
JVM, so their position is checked through the seed recorded on each call.

```powershell
cargo test -p rs09-server --test gathering
cargo xtask gathering-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
python tools/port_gathering.py --check
```

## What this does not establish

- In the first corpus service results come from a simple declared world in
  the adapters (a slot model for the inventory, flat level arrays, a clock
  array). They are inputs. Only the binding corpus compares the original
  `Container` and `Skills.addExperience`; neither compares `SceneryBuilder`,
  `Animator`, dialogue, audio or event code, and no native binding to
  `scripts`, `pulses` or `interaction_listeners` exists yet.
- Fishing: the spot NPC, familiars, `Pathfinder`, `GameWorld`, skill tasks,
  the diary task and the gloves are recording adapters. `player.direction` is
  a declared step vector, not the `Direction` enum. `withinMaxnormDistance`
  and `transform(Direction)` are the pinned expressions compiled into an
  adapter `Location`. The `<` of the catch roll is not exercised at equality.
  `GameAttributes` strips "/save:" on reads; the adapters now do the same.
- Item names are declared inputs, not cache definitions. The default locale is
  pinned to en-US, where default-locale lowering equals ROOT lowering; other
  locales are the owner's `string.lowercaseDefault` service.
- The per-tick driver is modelled on `ScriptProcessor.processInteractScript`;
  it is not the original processor. Base `Pulse.update` is not driven: the
  pulse cases call `start`, `pulse` and `stop` directly.
- Generator replacement and seed access in the oracle use `Unsafe` on JDK
  internals. The mapping of Kotlin's default generator to the thread-local one
  holds for the pinned Kotlin 1.8.20 standard library on JDK 11.
- The gem-name capitalisation is verified for ASCII names only.
- The fixtures are 19.8 MB and 3.0 MB; most of the first is recorded tool
  lookups.

Remaining work: bind the script, clock, pulse, listener-registry, attribute,
scenery and `EquipmentContainer` services to their native modules and verify
that composition against the original processors; `api.inEquipment` over the
item configuration; barbarian fishing; pickaxe repair; fishing-spot movement
and the pulse's option handlers; `FruitTreeChopper` and other consumers of
`SkillingResource`; random events, diaries and familiars behind the services;
a live session. Nothing here marks a source file as replaced.
