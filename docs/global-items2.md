# Everyday item handlers (`world/global_items2`)

`crates/server/src/world/global_items2/` ports the everyday item options and
the item-on-object basics of `content/global/handlers/item`, the second pass
of that slice after the teleport items of `world/global_items`
(docs/global-items.md). The global books (`book/`) are `world/books`
(docs/books.md).

Each Kotlin lambda and OptionHandler the original registers is a typed native
value (`registry::Interaction`, `registry::InterfaceFn`,
`registry::OptionPlugin`, `registry::ComponentPlugin`); its body is a function
over the `Host` trait, where every call the original makes outside the ported
bodies is a required method with no default. Pulses, queued scripts and close
events are `Task` values and the two dialogue files `Dialogue` values; the
world stores them and runs them with `registry::run_task` and
`registry::run_dialogue`.

## What is ported

| Original | Rust | Scope |
| --- | --- | --- |
| `EmptyOptionListener` (and its `EmptyItem`) | `everyday::empty`, `tables::EMPTY_ITEMS`, `empty_item`, `empty_item_list` | the 51 rows, the finished and unfinished potion products (the herblore enums' `potion` ids), the legacy name scan of the item definitions, the handler's two branches |
| `LightSourceListeners` | `everyday::drop_light`, `active_light_source` | the Temple of Ikov refusal (both `getActiveLightSource` calls), the removal and drop, the lit mining helmet |
| `SpadeDigOptionPlugin`, `SpadeDigListener`, `core.game.global.action.DigSpadeHandler` | `dig::spade`, `dig`, `dig_pulse`, `register`, `register_listener`, `run_listener` | the sound, the lookup before the animation and lock, a listener before an action, the action's world pulse, `putIfAbsent`, the logged refusal of a taken location |
| `CasketListener` | `everyday::casket`, `casket_table` | the table built (with its insertion draws) when the class initialises, one `roll` of the native `drop_tables::WeightBasedTable`, the article |
| `BirdNestScript`, `BirdNestPlugin`, `content.data.tables.BirdNest` | `everyday::nest_script`, `nest_plugin`, `search`, `random_nest`, `drop_nest`, `tables::BirdNest` | every nest and its loot, `forNest`, `getRandomNest` (identity of the chosen nest item), `drop`, `search` (the egg nests' first loot, the draw for the others and Wyson's) |
| `LampPlugin`, `content.data.Lamps` | `everyday::lamp`, `lamp_close`, `lamp_select`, `tables::LAMPS` | the 23 lamps, the attributes, the skill interface with its close event, the hidden tabs, `handleSelectionCallback` |
| `StarterPackPlugin`, `RingUnchargePlugin`, `RunePackPlugin`, `HerbBoxPlugin` | `everyday::starter_pack`, `ring_uncharge`, `rune_pack`, `herb_box` | every option and refusal; the herb box's charge rule and draws |
| `ItemQuestRequirementListener` | `toys::quest_equip`, `tables::quest_equipment` | the 26 `onEquip` lambdas and their 166 ids, the quest cape's points check |
| `MorphItemListener` | `toys::morph_equip`, `morph_button`, `morph_close` | the equip lambda (Kotlin `random()`), the interface's button and close listeners |
| `NewspaperListener`, `CadavaPotionListener`, `DrinkBlamishOilListener` | `everyday::newspaper`, `flyer`, `cadava`, `blamish_oil` | the pages, the flyer, the item and player dialogues |
| `ToyListeners`, `ToyHorseListener` | `toys::toy`, `toy_task`, `horsey` | all nine toys (the snow globe's two `runTask`s, the yo-yo and zombie head reading the option before the lock), the horsey's phrase |
| `EnchantedGemListener` and `EnchantedGemDialogue` | `dialogues::gem_activate`, `gem_handle`, `slayer_master_location` | every stage, the topics with `firstRun`, the task progress (pluralise, varp 2502) |
| `PetRock` (`PetRockListener`, `PetRockDialogue`) | `dialogues::pet_rock_interact`, `pet_rock_handle`, `pet_rock_script` | the five options, the queued script, the projectile |
| DialogueFile's `npc`, `npcl`, `player`, `options`, `end`, `showTopics` | `dialogues::npc`, `npcl`, `player`, `options`, `end`, `show_topics` | as the two files call them |
| `toys/DiangoOptionHandler`, `DiangoReclaimInterface`, `SnowGlobePlugin` | `diango::holiday_items`, `open`, `eligible_items`, `reclaim_handle`, `snow_globe_handle`, `snow_globe_pulse` | the 38 holiday items (appended on every `newInstance`), the eligibility filter, take and examine, the snow globe's two buttons and pulse |
| `withobject/WaterSourceListener`, `SandSourceListener`, `ChestKeyListener`, `HatStand`, `SackOnHay` | `withobject::water`, `water_pulse`, `format_message`, `sand`, `sand_task`, `chest_key`, `hats`, `hat_stand`, `sack_on_hay` | every vessel and source, the Falador diary task, the sand task's shared trigger, the hat stand's slot scan |
| `equipment/brawling_gloves` (`BrawlingGloves`, `BrawlingGlovesManager`) | `gloves::BrawlingGloves`, `login`, `instance`, `register`, `register_with`, `update_charges`, `experience_bonus`, `parse_player`, `save_player` | the enum and its three lookups, the login listener, the save hooks, the charges |
| `equipment/fistofguthixgloves` (`FOGGlovesListener`, `FOGGlovesManager`) | `gloves::fog_inspect`, `fog_update` | the inspect message, `updateCharges` |
| `RandomFunction.random`, `random(a, b)`, `getChanceItem`, `ChanceItem.getRandomItem`, Kotlin's `Array.random()` | `random::random`, `random_range`, `get_chance_item`, `ChanceItem`, `kotlin_random`, `JavaGenerators` | the three generators with their JDK 11 algorithms |
| ContentAPI `visualize`, `animate`, `runTask`, `getUsedOption`, `sendChat`, `getItemFromEquipment`, `face` (location), `getSlayerMasterLocation`; `colorize`, `StringUtils.isPlusN` | in `mod.rs` and `dialogues.rs` | the bodies these handlers call |
| the classes' registrations | `registry::define`, `register_scanned`, `Native` | ClassScanner's order per class (login listener, interaction listeners, interface listeners, save hook), plugins' `newInstance(null)`; `register_scanned` walks the start-up manifest |

## Behaviour that is easy to get wrong

- Registration: `EmptyItem.emptyItemList` is the rows, then the potion
  products not yet listed (finished, then unfinished), then every item
  definition whose name has `potion`, `brew`, `poison`, `cure`, `mix`, `balm`
  or `Super ` (case sensitive) or `serum` (lower-cased), in the definitions
  HashMap's walk. Blamish oil is a finished potion, so it can be emptied.
  HatStand's "hats" are every definition whose `equipment_slot` is 0 *or
  missing* (the default 0 is the head's ordinal). WaterVessel's inputs are its
  HashMap's key walk (table 32: `[1825, 1923, 1827, 1925, 229, ...]`), and the
  array overload of `onUseWith` (no in-use validation) registers them;
  ChestKey, SackOnHay and SandSource use the validating scalar one. A Kotlin
  lambda that captures nothing is one object across registrations (everything
  but BirdNestScript's bound `::handleNest`, MorphItemListener's equip and
  close lambdas, ToyHorseListener's, WaterSourceListener's and both cadava
  lambdas). BirdNestPlugin is not `@Initializable`: nothing registers it.
  DiangoReclaimInterface appends its holiday items to the static `ITEMS` on
  every `newInstance`. The casket's table draws its insertion positions from
  `RandomFunction.RANDOM` when CasketListener initialises (once).
- Random draws: `RandomFunction.random(int)` and `random(a, b)` use
  `RandomFunction.RANDOM`; `getChanceItem` draws `random((int) total)` and
  then shuffles a copy with `Collections.shuffle`'s own generator; the weighted
  table roll draws `ThreadLocalRandom.nextDouble(0, total)`; Kotlin's
  `random()` of an array is `Random.Default`, which on JDK 8+ is
  `ThreadLocalRandom.current().nextInt(size)`. The corpus seeds all three per
  case and compares their raw states after every operation.
- The empty option reads the slot through `node.asItem()` before building the
  empty item and casts again for the current item; an NPC whose name looks
  like a potion fails the cast. The emptied item is `Item(empty)` (amount 1).
- The light source: both `getActiveLightSource` calls walk the inventory's
  array, then the equipment's; nothing lit fails at `.product`; the region
  test is `x >> 6 << 8 | y >> 6 == 10648`; the product's name is the constant
  item's definition name, lower-cased with `Locale.ROOT`.
- DigSpadeHandler looks the action up before the animation and lock, runs a
  SpadeDigListener first (Kotlin's parameter check fails a null location
  there), and runs the action from a world pulse of delay 1.
- Lamps: `handleSelectionCallback` builds `Item(2528)` before reading the
  attribute; the level requirement is read after the static level; the
  message indexes `SKILL_NAME`; the genie lamp gives ten times the static
  level read again after the removal.
- The herb box removes a box outside 0..=10 herbs before locking; `take-one`
  re-reads the charge it just wrote; `bank-all` draws every herb before the
  removal.
- Brawling gloves: `getInstance` builds a default manager (no player) before
  reading the attribute and never stores it; `registerGlove(id)` catches the
  `requireNonNull` failure and prints it; `updateCharges` announces every
  fifty counting the charges *before* this use, less one; the save walks
  `GloveCharges` in HashMap order with string values; the parse fails on an
  unknown indicator at `getId` and on non-numbers in `parseInt`.
- The Fist of Guthix gloves write the capped charge back before testing it,
  and answer the capped charge when they crumble.
- The enchanted gem's dialogue reads the slayer master at every `handle`;
  `showTopics` offers the `IfTopic`s by `firstRun`, which any answered topic
  clears.
- The pet rock's "Fetch" reads `Projectile.getLocation(player)` three times
  (the face, the projectile's source, the projectile's destination).

## Required services and joins

The `Host` methods name the original call each answers. The world joins them
to: the native containers (`container_*`, `remove_item`, `replace_slot`,
`add_item`, `amount_in_inventory`, `in_inventory`, `in_equipment`); attributes
(GameAttributes' "/save:" rule); action locks (`lock`, `unlock`,
`lock_interactions`); the walking queue (`stop_walk`); the Animator; the
pulse managers and world pulser (`pulse_run`, `submit_world`, running the
`Task`s with `registry::run_task`); the script processor (`queue_script`,
`stop_executing`); the InterfaceManager (`open_component` with the close
event, `open_default_tabs`, `remove_tabs`, `opened_component`,
`close_interface`); the dialogue interpreter (`open_dialogue`,
`send_dialogues`, `add_topic`, `send_options`, `interpreter_close`, the item
and player dialogues); packets (messages, interface text, audio, jingles,
projectiles, `InterfaceContainer.generateItems`); ground items; scenery
replacement; diaries; skills (`static_level`, `add_experience`); the skull
manager; the item definitions (`definitions` in the table's HashMap order,
`item_name`, `item_examine`); `random` (the three generators, see
`random::JavaGenerators`); `statics` (the casket table and the dig
registries). Content stays a named service: the quest tables
(`has_requirement`, `quests`, `quest_points`), the slayer manager
(`slayer_*`, `set_varp`), the POH toy box (`storage_contains`),
`Location.getRandomLocation`, and the dig callbacks other content registers
(`run_spade_listener`, `run_dig_action` over the world's `Callback` type).

## How the live world registers them

At start-up, after the InteractionListeners and InterfaceListeners
registries exist: build `registry::Native::new(&interactions, &interfaces,
&casing, definitions, &mut generators)` (definitions in the definitions
table's HashMap order) and call `registry::register_scanned(&mut native)`.
Then put each of `native.item_handlers` into the item definition's handlers
map and each of `native.npc_handlers` into the NPC definition's
(`definitions::option_handler`), bind `native.components` as the component
plugins of interfaces 468 and 659, add `native.login_listeners` to the login
listeners and `native.persist_players` to the save hooks (`gloves::login`,
`save_player`, `parse_player`), and keep `native.casket` and
`native.diango_items` in the world's `Statics`. The item option packet path
dispatches a registered `Interaction` through `registry::run_interaction`,
an equip through the same with the equipped item, an `InterfaceFn` through
`registry::run_interface`, an `OptionPlugin` through `registry::run_handler`
(with the handler object's identity), the lamp's skill choice through
`registry::run_selection`, and a component button through
`registry::run_component`. Each `Task` handed over is stored with its pulse,
script or close event and run with `registry::run_task`; each opened
`Dialogue` with `registry::run_dialogue`. Nothing in `--world` installs these
yet.

## Reference tests

```powershell
cargo test -p rs09-server --test global_items2
cargo xtask global-items2-oracle --check --java-home <jdk11>
```

`oracle/global-items2` compiles the originals listed in `UNCHANGED` of
generate.py unchanged on JDK 11 with Kotlin 1.8.20 and the pinned ConstLib
1.4.28 (Items, Sounds, Scenery, Animations, NPCs and Components constants
checked against their names, LightSource's static `Items` imports too), with
the unchanged OptionHandler, Plugin, Pulse, IntType, core.api.Container,
Script, DialogueConst, EquipmentSlot, ApiExtensions, Globals, LoginListener,
PersistPlayer, RandomFunction, ChanceItem, WeightedChanceItem, DropFrequency,
DialUtils, DialogueTopic, ComponentPlugin, StorableFamily, WeightBasedTable,
WeightedItem, DigSpadeHandler, the herblore enums, Quests, LightSource,
Lamps and BirdNest. Original text inserted at generation time: the Skills ids
and `SKILL_NAME`, `StringUtils.isPlusN`, `NPCDropTables.DROP_RATES`,
`SlayerUtils.pluralise`, DialogueFile's `npc`, `player` (both), `npcl`,
`end`, `options` and `showTopics`, and the ContentAPI functions `visualize`,
`animate`, `runTask`, `getUsedOption`, `sendChat`, `getItemFromEquipment`,
`face` and `getSlayerMasterLocation`. Everything else is a recording adapter
answering from the case; the adapters' fields are named apart from the
getters' Kotlin properties. Every case runs in a class loader of its own
(`Boot.java`), so the originals' static state (listener maps, the casket
table, `ITEMS`) starts fresh; items built while a class initialises (the
tables' constants) are not recorded and carry the key `const:<id>`.

`fixtures/global-items2/oracle.jsonl`: 1,430 cases (552 designed, 878 with
one recorded call failing in turn), 4,191 operations, 79,447 recorded calls,
in 30 groups (registration with the scan order and every class registered
twice; emptying; light sources; the spade; caskets by seed; bird nests by
seed; lamps; starter pack, rings and rune packs; the herb box; the newspaper,
cadava potion and blamish oil; equip requirements; morphing rings; toys and
the toy horsey; water and sand sources; chest key, hat stand and sack; the
gloves; the enchanted gem; the pet rock; Diango and the snow globe; the
generators' own draws, doubles as raw bits). The test
replays every operation through the native bodies and registrations with a
recording host that follows the adapters and compares the result, the error,
every recorded call (registrations included, with the original class names
and object identities), the printed stack traces, the attributes, the
dialogue stages and the three generators' raw states after each operation.
Every case matches; there are no pinned gaps. The same file checks that
`register_scanned` over the start-up manifest produces exactly the oracle's
scan-order registration (and the casket's draws), that `Native` installs the
typed objects into the native InteractionListeners and InterfaceListeners
registries, and the water vessels' HashMap walk.

Forty-two deliberate breaks of the port were tried (the dig lock, pulse delay
and taken-location answer; the empty option's `Super ` and audio sentinel;
the Ikov region; the casket's article and coin weight; the nest ordinal; the
shuffle bound, ThreadLocalRandom's power-of-two path, rejection and double
bits, `random(a, b)`'s wrapping; the genie factor and lamp tabs; the starter
pack's slots; the herb box's emptying charge; the quest cape's comparison;
the marionette's lock order; the phrase count; the well rule, the message's
parenthesis and the sand trigger; the FOG cap write; the brawling message
count and charges; the glove map's capacity and growth; the shared water
lambda; the gem's varp shift and topic title; the pet rock's script strength;
the staff of the raven; the snow globe's delay; the HashMap growth; a
message; a reward; the rune pack's lock order; colorize; the slayer
location fallback; the second diary fetch): 41 fail the replay. Three of
them (`Super `, the power-of-two path, the HashMap growth) passed until cases
were added for them (a "Super" name without the space, the generators'
direct draws, a twelve-definition table). The one that does not is the glove
map's growth threshold (2/3 instead of 3/4): the thirteen glove ids walk in
the same order in tables of 16 and 32, so no reachable state tells them apart.

## Differences from the original

- The registry never dispatches a null node to an `on`/use-with lambda; the
  bodies take the node by reference (the option handlers still take an
  optional node and option).
- `getSlayerMaster` and the other ContentAPI functions besides the ported
  ones are services at their ContentAPI level.
- A constant item of a table is a host node (`constant_item`); its name is
  read when the body needs it (`item_name`), not when the class initialised.
- Errors carry a category, not a Java stack trace; a caught and printed stack
  trace is `Host::stack_trace` with the exception class.

## Not ported or not joined

- `book/` (GeneralRuleBook, GoblinBook, GrimDiary, SecurityBookPlugin,
  StrongholdNotes) and BookInterface are `world/books` (docs/books.md).
- The content services named above (quest tables, slayer, POH storage,
  random reachable locations, the content's dig callbacks).
- Nothing installs the library in `rs09-server --world` yet (see above for
  how).
