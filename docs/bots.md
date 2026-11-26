# The artificial player engine

`world/bots` ports `core/game/bots`: the player without a client that the
original's content fills its world with, the helpers its scripts call and the
builders that dress it. It is a library. Nothing installs it in
`rs09-server --world`; the world-login and world-trace tests still put
stand-ins into the repository slots the original's bots took. Java and Kotlin
are test oracles; nothing here runs them.

| Original | Rust | Scope |
| --- | --- | --- |
| `AIPlayer.java` | `world/bots/ai_player.rs` (`AiPlayer`, the free functions) | both constructors and the private one, `updateRandomValues`, `init`, `tick`, `clear`, `reset`, `finalizeDeath`, `update`, `follow`, the four walks, `checkVictimIsPlayer`, `getItemById`, the three `getClosestNode...` and three `getClosesCreature` forms, `loadNames`, `getRandomName`, `updateRandomOSRScopyLine`, `retrieveRandomName`, `get`, `deregister`, the accessors, and the private `setLevels`, `giveArmor`, `parseOSRS`, `equipIfExists` |
| `ArtificialSession.java` | `ai_player.rs` (`ArtificialSession`) | the singleton, its five empty overrides and the `object` field |
| `java.util.Scanner` (JDK 11) | `world/bots/scanner.rs` | `hasNextLine`, `nextLine`, `hasNext` with the default delimiter |
| `AIRepository.kt` | `world/bots/repository.rs` | `addItem`, `getItems`, `addOffer`, `getOffer`, `clearAllBots`, `sendBotInfo`, the three maps |
| `Script.java` | `world/bots/script.rs` (`Script`) | the fields, `init`, `toString`, `setLevel` |
| `ScriptAPI.kt` | `world/bots/script_api.rs` (`ScriptApi`, `BottingOverlay`) | every member (see below) |
| `GeneralBotCreator.kt` | `world/bots/creator.rs` | the three constructors, `BotScriptPulse` (`pulse`, `stop`), `TransitionPulse`, `GBCTick`, `botPulsesTriggeredThisTick` |
| `AIPBuilder.java` | `world/bots/builder.rs` | `create`, both `copy`, `RandomAfkPlayer`, `immersiveSpawns` |
| `CombatBot.kt`, `PvMBots.java` | `ai_player.rs` (`BotClass`), `builder.rs` | the overridden `updateRandomValues` and `tick`, `CheckPrayer`, `eat`, `FindTargets`, `checkValidTargets`, both `AttackNpcsInRadius` |
| `PvMBotsBuilder.kt` | `builder.rs` | `create`, both pest control creators, `botsSpawned` |
| `CombatBotAssembler.kt` | `world/bots/assemblers.rs`, `tables.rs` | `produce`, the six assemblers, the six gear functions, `generateStats`, `equipHighest`, `lvlAvg`, the item tables |
| `SkillingBotAssembler.kt` | `assemblers.rs`, `tables.rs` | `produce`, `assembleBot`, `equipSet`, the three set tables |
| `PlayerScripts.kt`, `ScriptIdentifier`, `ScriptDescription`, `ScriptName`, `PlayerCompatible` | `world/bots/mod.rs` (`PlayerScript`, `StaticsState::identifier_map`) | the table and its entry; the annotations are the entry's fields |

## Model

An artificial player is a `Player`. Its `Player` object is a host node
(`Host::Node`): the canonical player once a world binds it. What `AIPlayer`
and its two engine subclasses add is `AiPlayer`, a shared handle with the uid,
the start location, the user name, the priority flag, the controller and the
subclasses' tick counter. `BotClass` says whether the object is an
`AIPlayer`, a `CombatBot` or a `PvMBots`; the two virtual members
(`updateRandomValues`, `tick`) dispatch on it.

The statics of the engine's classes are one `Statics` value per world:
`currentUID` (starting at 1), `botNames`, `botMapping`, `OSRScopyLine`, the
session singleton, `AIRepository`'s three maps,
`botPulsesTriggeredThisTick`, `botsSpawned` and `PlayerScripts.identifierMap`.
`AIPlayer`'s static initialiser (`loadNames("botnames.txt")`) runs at the
first use of the class, as class initialisation does.

`Script`, `BotScriptPulse` and the `buyFromGE` task are shared handles too,
because the original shares the objects. No lock is held across a service.

The pulses the engine creates are the variants of `bots::Pulse`. The host
keeps a pulse it is handed and runs `Pulse::pulse`; `delay`, `checks`,
`movement` and `class_name` say what the original's constructor was given and
which class the object is (pulse telemetry classifies by the class name).

## The three joins

- **Repository.** `new AIPlayer` calls `Repository.getPlayers().add(this)`:
  the native `NodeList::add`, first free slot from 1, and no entry in the uid
  or name map. A full list adds nothing and the constructor goes on.
  `deregister` removes the node with `NodeList::remove`.
- **Clear.** `AIPlayer.clear()` takes the bot out of `botMapping` and is then
  `Player.clear()`: `world::disconnection::player_clear`, which for an
  artificial player is `finishClear` at once, without the queue or a save.
  The engine's `clear`, `deregister`, `CombatBot` tick and `clearAllBots`
  therefore need the host to be a `repository::Host` as well.
  `ai_player::virtual_clear` is the virtual `player.clear()`; a binding's
  `repository::Host::player_clear` is that function.
- **Canonical player.** `bots::canonical` gives the user name the
  constructor builds (`/aip{currentUID + 1}:{name}`) and reads and writes the
  three plain fields the canonical profile holds: `Node.name`,
  `Player.artificial`, `Player.playing`. The corpus replay creates a canonical
  player (`player::owned::Player` with a fresh `Profile`) for every bot and
  answers those fields from it. The actor's location and slot index, and
  every manager of the player, are not bound here.

## What the original does that this keeps

- The details name is built from `currentUID + 1`, the display name from the
  name and `currentUID + 1` (`StringUtils.formatDisplayName`), and the uid is
  `currentUID++`: the first bot is `/aip2:name`, shows as `Name2` and has
  uid 1.
- A name is taken out of `botNames` when it is drawn. With the list empty the
  draw of 0 fails (`IndexOutOfBoundsException`). A missing name file is
  printed and leaves the list empty.
- The constructor's order: player, location, `artificial`, session, the list,
  the names, the uid, `updateRandomValues()`, `init()`. A failure leaves what
  was done: the player in its slot, the counter where it was, and no entry in
  `botMapping` unless `init` reached it.
- `updateRandomValues` draws `random(3)` for the gender (female at 1),
  `random(0, 10)` for nothing, calls `CharacterDesign.randomize(this, true)`
  and draws the direction from a new `java.util.Random` (an index outside
  the eight directions fails). `CombatBot` draws `random(5)` and skips the
  two middle steps.
- `updateRandomOSRScopyLine` keeps the n-th line when a draw of n is 0,
  unless it is shorter than three characters or starts with `#`; lines are
  counted while the scanner still has a *token*, so trailing blank lines are
  not read, a line of three spaces is kept, and a no-break space is a token.
  `OSRScopyLine` is static: a later call with a missing file keeps the line
  of an earlier one. `retrieveRandomName` repeats until the line has no `_`
  and no space, and fails on a line of colons only (`split` gives nothing).
- `tick()`: `Player.tick`, the wilderness level while the skull manager says
  wilderness, and a lifepoints read with no effect. `CombatBot` then
  deregisters itself at exactly 0 lifepoints; `PvMBots` sets 20 at 5 or
  fewer and wraps its counter at 100.
- `clearAllBots` stops each pulse, clears its bot and then deregisters it by
  uid. The clear has already taken the bot out of `botMapping`, so the
  deregistration finds nothing and the bot keeps its slot in the player list.
  A script whose bot is not an `AIPlayer` fails the cast after its clear
  (which queued the player); a script without a bot fails inside the stop.
- `AIRepository.addItem` fails for an item without a dropper (the map's key
  type is not nullable).
- `getClosestNode...` asks `RegionManager.getObject` four times per tile of a
  triangle around the bot on plane 0, so a tile on an axis is asked, and its
  object listed, more than once; the first listed node wins ties.
  `getClosesCreature` keeps its best distance as a truncated int, so after a
  diagonal neighbour (1.41, kept as 1) an adjacent NPC is not closer.
- `randomWalk` and `randomWalkAroundPoint` draw both offsets before a missing
  location fails. `walkPos` finds a path and drops it.
- `BotScriptPulse.pulse()` in order: the random delay; a movement pulse whose
  bot has not moved for more than five ticks is stopped (the tick difference
  wraps); a paused interaction with a modal open is closed unless the script
  keeps dialogues; then, with no pulse and no interaction, a stopped script
  ends the pulse, the 76th bot of a tick waits, one roll in ten starts a
  pause of `random(20, 50)` ticks (the roll is made before the `Idler` test;
  a high-priority bot makes no roll), and the script ticks. `stop()` runs
  `pulse()` once more, then `Pulse.stop`, and removes the repository entry
  only while the server runs.
- Only the constructor taking an `AIPlayer` (and `TransitionPulse`) enters
  the pulse in `PulseRepository`; the location and player constructors do
  not.
- `Script.init` sets levels in the order its `HashMap<Integer, Integer>`
  iterates, then quests to stage 100, equipment (`add(item, true, false)`),
  and the inventory after a clear.
- `ScriptAPI`: `useWith` casts its node to `Scenery`, so an NPC or item fails
  after the inventory lookup; `evaluateViability` refuses a node farther than
  the best so far *or* than `MAX_PATHFIND_DISTANCE`, so an equally near later
  node replaces an earlier one; an id of -1 accepts every id;
  `getNearestNode(name, isObject)` searches the node list whatever the flag;
  `getNearestGameObject` answers the last listed object with the id;
  `findTargets` looks at the first five local NPCs only and compares names
  exactly; the ground item search prefers the last matching item of the
  repository list and drops one the manager no longer holds;
  `walkToIterator` aims at most `MAX_PATHFIND_DISTANCE - 1` tiles along the
  line and always on plane 0; `teleportToGE` runs its pulse on the bot's
  manager and `teleport` on the world pulser; the alchemy value times the
  amount wraps as an int; `sellAllOnGe` keeps lobster, swordfish and shark;
  `sellAllOnGeAdv` leaves a big sale of one of six log ids in the bank;
  `eat` eats at `staticLevel * random(0.5, 0.75) >= lifepoints` and feeds an
  unknown consumable as `Food([found id], HealingEffect(1))`, `forceEat` as
  `Food([given id], ...)`; `withdraw` takes what the bank has when it has
  less than asked and what the inventory can add (never below 0);
  `equipAndSetStats` reads `equipment_slot` once and `requirements` twice.
- `Item`: `getId()` is the low 16 bits of the id it was made from; its
  definition, name and note id are those of the id as written.
- `CombatBot.eat` feeds an unknown consumable as a `Food` whose id array is
  that many zeros (`IntArray(food.id)`); `PvMBots.eat` fails on it.
- `CombatBotAssembler`: the mage type assembles a melee bot;
  `assembleRangedBot` compares `crossbow ?: (Random().nextInt() % 2)` with 0,
  so a given `crossbow` never chooses the crossbow; `equipHighest` reads the
  level of every requirement of every item, compares `(1 + levels) /
  (1 + count)` as ints, picks at random among the best and takes the slot of
  the first of them; a level cap refuses requirements above it.
  `generateStats` moves each skill by `(random(-0.5, 0.5) * 33).toInt()`
  around the tier's draw and gives hitpoints a fifth of the mean experience.
- `SkillingBotAssembler.equipSet` tests the *inventory* slot of the item's
  equipment slot number before it equips.
- Kotlin parameters of type `Location` are not nullable: `CombatBot`, the
  assemblers and `SkillingBotAssembler.produce` take a location, the Java
  constructors and `GeneralBotCreator` an optional one.

## Evidence

`cargo xtask bots-oracle` compiles the seventeen files of `core/game/bots`
unchanged with Kotlin 1.8.20 on JDK 11, together with the original
`NodeList`, `NodeListIterator`, `Location`, `Vector`, `Direction`, `Item`,
`IntType`, `Quests` and `Globals`, and runs every case in a class loader of
its own. `Player.clear`, `finishClear`, `sendLogoutEvents`, `setPlaying`,
`isPlaying`, `isArtificial`, `getName`, `getUsername`,
`StringUtils.formatDisplayName`, the `Skills` ids, the equipment slots, the
`Log` constants and `ItemConfigParser.GE_PRICE` are original text inserted
into stand-ins at generation time. Everything else is a recording stand-in
answering from the case. The name files are real files read by the original
`Scanner`.

Three kinds of draws are made by generators the original constructs itself.
`new Random()` and the standard library's `.random()` are resolved, in the
unchanged sources, to stand-ins declared in the engine's own package (a class
`Random` and two `random()` extensions), so they are recorded calls like
`RandomFunction`'s. `kotlin.random.Random.nextInt(from, until)` in
`randomizeLocationInRanges` cannot be replaced or seeded; its two draws are read
off the location the original returned and the replay answers with them, and
the corpus only gives it ranges that hold one value (so that the fixture
reproduces). How it draws from a wider range is not compared.

The corpus has 1,343 cases: 253 designed and 1,090 in which one recorded call
of a marked operation fails (every call of a short operation, every n-th of a
long one), with 3,331 operations and 117,795 recorded calls.
Each operation records its calls in order, its result or failure class and
the state afterwards: the repository's player list slot by slot, the statics
(class initialised, uid counter, names left, mapping, kept line, session
object), `AIRepository`'s maps (the pulse repository in `HashMap` order with
each pulse's fields), both counters, every bot (class, uid, user name, start
location, priority, controller, `artificial`, index, location, `playing`,
tick counter), every script, the disconnection queue and
`PlayerScripts.identifierMap`. `crates/server/tests/bots.rs` replays every
case on the native library and compares all of it; its bots are canonical
players.

62 deliberate breaks of the port were tried one at a time (wrong offsets,
bounds and comparison operators, reordered or dropped calls, changed table
entries, class names and coordinates across every file of the module): each
fails the replay. Ten of them fail only cases that were added for them: the
figure space as a token, the two distance bounds of `walkArray`, a note id of
0, an over-complete offer, a requirement one above a level cap, a plain
player's display name as the repository key, the class name of
`TransitionPulse`, and an entry in a rich and in a poor set (the first corpus
ran only a few of the 152 sets).

```powershell
cargo test -p rs09-server --test bots
cargo xtask bots-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

The generator never downloads. It needs the pinned Kotlin jars under
`runs/kotlin-1.8.20` and the pinned json-simple and ConstLib jars under
`runs/bots-oracle` or another `runs/*-oracle` directory.

## Required services

Every outside call is a method of `bots::Host`, with no default. The plain
fields and getters at the top of the trait are not calls of the original:
locations, ids, names, the active flag, node kinds and directions, item
names, a ground item's dropper, the constants of `ServerConstants`, `Server`
and `GameWorld.settings`, the text of a name file, the class of a script. The
rest are one original call each, and a binding answers them with the system
named:

| Services | Answered by |
| --- | --- |
| `new_player`, `set_location`, `set_direction`, `player_init`, `player_tick`, `player_reset`, `player_finalize_death`, `full_restore` | the canonical player and the login port's `Player.init`/`Entity` members |
| `repository`, and `repository::Host` for `Player.clear` | `world/repository`, `world/disconnection` |
| `random`, `random_between`, `random_double`, `get_random`, `roll`, `linear_decrease_rand`, `normal_rand_dist`, `new_random_next_int`, `new_random_int`, `kotlin_next_int`, `kotlin_random_index` | the world's generators (`RandomFunction.RANDOM`, `Math.random`, a new `java.util.Random`, Kotlin's default) |
| `skills_*` | `player::skills` |
| `container_*`, `equipment_add*` | `inventory`, `equipment`, the bank |
| `item_*` | `definitions::item` and the item configuration (`requirements` in the original map's iteration order: `world::repository::JavaHashMap` reproduces it) |
| `pulse_manager_*`, `pulser_submit`, `pulse_stop`, `script_pulse_super_stop` | `pulses` (manager and runner) |
| `active_script`, `scripts_delay` | `scripts` (the script processor) |
| `pathfinder_find`, `path_walk`, `walking_queue_is_moving`, the movement of a `Pulse::movement` | `world/pathfinding`, `world/movement`, `world/walking` |
| `local_npcs`, `region_object`, `region_for_id`, `region_object_list`, `region_node_list` | `world/region_queries`, `world/region` |
| `interaction*`, `listeners_run`, `listeners_run_use`, `plugin_interaction_handle`, `use_with_run`, `scenery_child` | `world/interaction_listeners`, `world/legacy_interaction`, `world/packet_handlers` |
| `combat_victim`, `combat_delay_next_attack`, `attack`, `in_combat`, `properties_*`, `prayer_toggle`, `skull_*`, `wilderness_level` | `world/combat_pulse`, `player::properties`, `world/prayer`, `world/wilderness_zone` |
| `ground_items_contain`, `botting_drops`, `set_botting_drops` | `world/ground_manager`, the player's attributes |
| `ge_add_bot_offer`, `send_grand_exchange_news` | `world/grand_exchange`, `world/repository` (`send_grand_exchange_news`) |
| `consumable`, `consume` | `world/consumables` |
| `appearance_*`, `settings_set_run_toggled`, `flags_*`, `interface_*`, `dialogue_close`, `send_*`, `visualize`, `lock*`, `unlock`, `face*`, `send_chat`, `register_chat_mask`, `animator_reset`, `impact_set_disabled_ticks` | the canonical player's managers |
| `quest`, `quest_set_stage`, `character_design_randomize`, `script_tick`, `script_new_instance`, `adventurer_state`, `new_pest_control_test_bot` | content |

## Differences from the original

- The statics are a value of the world, not of the process.
- `buyFromGE` launches a coroutine that blocks on a latch until its own pulse
  ran. Natively the block has two halves, `BuyTask::run` up to the wait and
  `BuyTask::resume` after it; the corpus runs them in order on one thread.
  Real coroutine dispatch is not compared.
- A node, location-less or not, is never null; the calls that take a null
  node take an `Option`.
- `Script.toString` ends with the current pulse's `Object.toString`, an
  identity hash for the engine's own pulses; the corpus compares it for no
  pulse and for a stand-in pulse with a text of its own.
- The scanner works on the decoded text of a file; decoding is the host's.

## Not ported

The bot scripts under `content/` and `ImmerseWorld`'s spawning decisions;
`PestControlTestBot` and `PestControlTestBot2`; the reflection that fills
`PlayerScripts.identifierMap`; `AIPBuilder`'s private `ITEMS` list (an
instance field of a class nothing instantiates) and
`CombatBotAssembler.RICH_MELEE_HELMS` beyond its values (nothing reads it);
`ScriptAPI.attackNpcsInRadius`'s second search (unreachable: the first list
is never empty there); `PvMBots.eat`'s return for a constant without a
consumable (no constant is); `IoSession`'s other members for the artificial
session; real coroutines.

## What installing it needs

- A binding of `bots::Host` and `repository::Host` over the live world: the
  plain fields from the canonical player and its actor, the services from the
  libraries in the table above, and `repository::Host::player_clear`
  answered by `ai_player::virtual_clear`.
- The world's player tick calling `AiPlayer::tick`, `reset` and (nothing for)
  `update` for an artificial player, `creator::gbc_tick` as a tick listener,
  and the pulses handed over wrapped as native pulses with their class
  names.
- The places that refuse artificial players today answered from this
  library: `is_artificial` from the canonical flag,
  `AIRepository.addItem`/`getItems` and `botting:drops` (ground items and
  drop tables), `PulseRepository[name]` (trade's doubling-money hand-off, the
  bot command), `clearAllBots` (system manager), `sendBotInfo` (global
  handlers), `handleIncomingChat` (social; it does nothing).
- The world-login and world-trace tests construct their stand-ins by hand:
  a slot taken with `players().add`, uid 0, `artificial`. They become native
  by implementing `bots::Host` for the test binding (`new_player` makes the
  slot, `repository` is the test's repository) and calling
  `creator::create_at` or `AiPlayer::new` once per recorded bot, answering
  the name draw with the recorded name's index. The slot and index then come
  from the constructor's own `NodeList::add`, and the comparison of the
  `players` rows no longer needs to leave artificial players out. Which bots
  the content spawns at tick 1, and with which scripts, stays the content's
  (`ImmerseWorld`) decision, taken from the trace.
