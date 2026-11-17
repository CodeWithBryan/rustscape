# core.api (ContentAPI and the other core/api files)

`crates/server/src/world/content_api/` is the native `core.api` package:
every top-level function of `ContentAPI.kt` and the other files of
`core/api` that content is written against. Its corpus is
`fixtures/content-api` (`cargo xtask content-api-oracle [--check]`), which
compiles `ContentAPI.kt` and the other core/api files **unchanged** on JDK 11
with Kotlin 1.8.20 and replays every case against the Rust bodies
(`cargo test -p rs09-server --test content_api`).

## Scope

- **Every function is a native body over `content_api::Host`.** A call the
  original makes on another class (Player, Entity, the containers, the
  interface manager, dialogue interpreter, packet dispatch, skills, timers,
  the script processor, the repository and region manager, scenery builder,
  ground items, slayer, Grand Exchange, shops, POH storage, requirements...)
  is `Host::call(Member, args)`: one `Member` per original member, named
  `Class.member` as the corpus records it, with its arguments as `Arg`s and
  a typed `Reply`. World state the original reads without a call (node ids,
  names, locations, item amounts and charges, the item definition table,
  clocks, `GameWorld.ticks`, script fields, the player's container
  objects, `useSecondaryBank`, `Shops.shopsByNpc`...) is a plain `Host`
  method. Content callbacks, pulses and queued scripts are host nodes; the
  pulse and script bodies ContentAPI itself builds (`poofClear`,
  `transformNpc`, `runTask`, `runWorldTask`, `sendChat` with a delay,
  `transformNpcWithHpCarryover`, `forceMove`) are `PulseBody`/`ScriptBody`
  values run by `state::run_pulse`/`state::run_script`, and
  `sendSkillDialogue`'s handler is a `SkillHandler` run by
  `ui::skill_create`/`ui::skill_get_all`. Every service is required; none
  has a default.
- **Varps and varbits** are thin wrappers (`content_api::vars`) over
  `crate::player::variables`, which already holds the original bodies over
  the player's real varp maps; the corpus replays them through the wrappers.
- **Content-only callees stay services**: `SkillingTool`, `SlayerManager`
  (and `Tasks`, `Master`, `SlayerFlags`, `SlayerEquipmentFlags`),
  `StockMarket`, `GrandExchangeRecords`, `Shops`, `StorageInterface`
  (bookcase bitfields, storables, storage containers), `Consumables`,
  `QuestRepository`/`Quest`, the requirement classes' `evaluate`,
  `InteractionListeners.run`, `DoorActionHandler`, `SpecialLadders`,
  `PlayerStatsCounter`, `PlayerMonitor`, `HintIconManager`,
  `AchievementDiaryManager`, `CombatSwingHandler.isProjectileClipped`,
  `Pathfinder`, `ForceMovement`, `Projectile`. Data tables of content are
  host reads: `Quests.toString()` (`quest_name`), `Master.npc`
  (`master_npc`), `DiaryType.levelNames`. The `QuestRequirements` lookup
  (constant to `Quests`) is native (`quests::QUEST_REQUIREMENTS`), checked
  against the original table the generator extracts into the provenance.
- **Nothing installs the library in `rs09-server --world`**: no binding of
  `content_api::Host` to the live world's native libraries (containers,
  skills, timers, scripts, interfaces) exists yet. That binding, and the
  content ported on top of this surface, are the next steps.

## Corpus

1,635 cases (974 designed, 661 single injected failures), 330 entry points
(every overload and default form of the 236 public ContentAPI functions is
its own entry point, plus the other core/api files), 13,357 recorded calls.
Each case runs in a class loader of its own, seeds the thread's
ThreadLocalRandom (Kotlin's `Random.Default`, drawn by `playHurtAudio`) and
records its raw state afterwards. The recorded record per case: every call
with its arguments, the result, the error class, the follow-up steps (a
recorded pulse's `pulse()`, a queued script's execution at a stage, the
skill dialogue handler's `create`/`getAll`, a tick change) and a state
snapshot (varp and save maps, logout listener keys, `useSecondaryBank`,
entity clocks, NPC invisibility, item id/amount/charge after `Item`'s
`idHash` packing, scenery charge, script `nextExecution`/`state`, the
poison timer's severity and damage source). Printed stack traces inside a
catch (`runcs2`, `forceMove`'s callback) are recorded at the point they are
printed; `logWithStack`'s stack frames are the platform's and are replaced
by `<frames>` on both sides.

Original text inserted into adapters at generation time:
`Skills.SKILL_NAME` and the skill ids, `Direction.getDirection(int, int)`,
`Item.toString()` and the `QuestRequirements` enum. Compiled unchanged with
ContentAPI: the other core/api files listed below, `Location.kt` (with its
15-bit `LocationHash` packing), `Script.kt`, `Clocks.kt`, `Globals.kt`
(`colorize`), `TickUtils.kt` (`cyclesToTicks`), `DiaryLevel.kt`,
`ForceMoveCtx.kt`, `Quests.kt` and `StorableFamily.kt`.

Pinned behaviour the corpus covers (all reproduced): the
`useSecondaryBank` choice of `player.bank`; `getAmountInBank`'s first stack;
`amountInEquipment`'s `equipment_slot` configuration (a missing definition
fails, another configuration class fails the cast); `removeItem`'s
null/Item/Int/other forms, the bank's secondary fallback and the BoB
container's second familiar read; `replaceSlot`'s 65535/zero-amount clear,
the DUPE_ALERT log with `Item.toString()` and the other-container
fallback, the BoB cast; `replaceAllItems`' stackable and bank paths;
`addItemOrDrop` one at a time for unstackable amounts; `addItemOrBank`'s
ironman reads per branch and `colorize`d messages; `hasAnItem`'s search
order (active then inactive bank, then the ten storable families);
`decantContainer`'s dose regex, HashMap order of the potions by identity
hash (an explicit host input) and empty vials; `dumpContainer`'s space
check that stops, `canAdd` that skips, equipment unequip listeners and
slayer flags, `unnote` after the removal; `animate`/`visualize`'s
Int-or-object arguments and IllegalStateException messages;
`playHurtAudio`'s Kotlin `random()`; `replaceScenery`'s direction table;
`hasOption`'s NPC/scenery/item dispatch and IllegalArgumentException;
`face`'s `asNpc()` ClassCastException for a location with a duration;
`truncateLoc`'s vector clamp (NaN for a zero vector) and plane-0 result;
`forceMove`'s tick arithmetic (`cyclesToTicks`), the mask registered once,
the animation while moving, the swallowed callback failure and the
original `dest`; `getPathableRandomLocalCoordinate`'s
`center.transform(.., center.z)` plane quirk and attempt loop;
`setTitle`'s exception after both packets; `sendNPCDialogueLines`'
component id; `sendDoubleItemDialogue`'s casts; the input prompt types;
`openDialogue`'s close first and the log for other objects; `runcs2`'s
reversed arguments and printed IllegalArgumentException;
`sendItemSelect`'s nine options and settings 1022; the skill dialogue's
item count check, types and default callbacks; hint icons;
`getAttribute` defaults and `removeAttribute`'s `/save:` strip; the timer
glue with the reified casts (`getTimer<Poison>` of another timer fails the
cast) and `spawnTimer<T>`'s `!!`; `applyPoison`'s immunity, severity and
damage-source order; `restartScript`/`stopExecuting`'s second active-script
read; `delayAttack`'s fixed 3; `stun`; `setCurrentScriptState`'s dev-mode
failure; `hasRequirement`'s quest point arithmetic, the two message forms
and the `QuestRequirements` lookup for every `Quests` constant;
`getDiaryLevelIndex`'s capitalised name; the slayer chain's casts and
`!!`; `hasIronmanRestriction`'s ordinal compare; the
`restrictForIronman` openers; `hasAwaitingGrandExchangeCollections`'
first-record return; `activateSecondaryBankAccount`'s coin split and
failure message; `storeBookInHouse`'s game-book path; credits;
`announceIfRare`; `calculateDragonfireMaxHit` for players (shield, potion,
prayer, messages) and NPCs (protection tokens).

Deliberate breaks: 12 of 12 fail the replay (the bank's secondary
amount, `keepRunning`'s next tick, `sendMessage`'s 86-character limit
(only after the boundary cases were added), `cyclesToTicks`' ceiling,
`hasRequirement`'s quest point comparison, dragonfire's prayer share,
`replaceScenery`'s direction table, decant's HashMap order,
`stopExecuting`'s persist check, `colorize`'s trailing space,
`dumpContainer`'s space check, the pathable radius).

## Duplicates

Before this slice, ContentAPI bodies were ported inside other slices over
their own hosts (they are re-exported in `content_api::ported`). Each is the
same original text; `content_api` has its own body over its host so content
has one surface, replayed against the whole compiled file. Pairs (the
earlier one keeps its own corpus and remains the binding its slice uses):

| ContentAPI | Earlier port | Note |
| --- | --- | --- |
| `animate` | world/global_items, world/global_items2 | two earlier ports of the same body |
| `getUsedOption` | world/global_items2, world/consumables | two earlier ports |
| timer glue (`getOrStartTimer`, `spawnTimer`, `hasTimerActive`, `getTimer`, `registerTimer`, `removeTimer`, `applyPoison`, `isPoisoned`, `curePoison`) | timers/content (content-timers, 2,311 cases), world/consumables/join.rs | timers/content is the broader corpus and binds the native timer manager |
| `openDialogue`, `sendInputDialogue` | world/dialogue_dsl/api.rs (3,600 cases) | over the native dialogue interpreter |
| `hasDragonfireShieldProtection`, `calculateDragonfireMaxHit` | world/dragonfire/api.rs (1,264 cases) | |
| `stun` | world/special_attacks/melee.rs (3,705 cases) | |
| `hasAwaitingGrandExchangeCollections` | world/grand_exchange/records.rs | over the native records |
| `teleport`, `delayEntity`, `resetAnimator` | world/global_items | |
| `visualize`, `runTask`, `sendChat` (no delay), `getItemFromEquipment`, `getSlayerMasterLocation`, `face` (inlined) | world/global_items2 | the runTask bodies are that slice's |
| `replaceSlot`, `queueScript`, `stopExecuting`, `delayAttack` | world/consumables | |
| `announceIfRare` | world/drop_tables/drop.rs | |
| `restrictForIronman` | world/global_handlers/tabs.rs | |
| `removeAttributes` | world/player_tabs (crate-private) | |
| `hasLevelDyn`, `getDynLevel`, `inInventory`, `addItem` | world/gathering/binding.rs (inline over native skills/containers) | |
| `splitLines` (DialUtils) | world/plain_dialogue.rs | wrapped, not duplicated (`ui::split_lines`) |
| `EquipmentSlot.slotByName` | world/commands/system.rs | wrapped, not duplicated |
| varps/varbits | player/variables.rs | wrapped, not duplicated |

## Inventory

261 top-level functions (237 names; `getAmountInBank` is private; the
reified `getOrStartTimer`, `spawnTimer`, `hasTimerActive`, `getTimer` and
`removeTimer` are inline). "Content files" counts the files under
`content/` that call the name. Every function has a native body; 10 are
thin wrappers over player/variables, the rest are bodies over
`content_api::Host`. 46 names had an earlier port (column 5).

| Line | Function | Content files | Native body (world::content_api::) | Earlier port | Corpus cases |
| ---: | --- | ---: | --- | --- | ---: |
| 98 | `getTool` | 2 | `items::get_tool` |  | 2 |
| 109 | `hasLevelDyn` | 19 | `services::has_level_dyn` | world/gathering/binding.rs (inline) | 3 |
| 120 | `hasLevelStat` | 26 | `services::has_level_stat` |  | 2 |
| 130 | `amountInInventory` | 73 | `items::amount_in_inventory` |  | 1 |
| 141 | `amountInBank` | 2 | `items::amount_in_bank` |  | 6 |
| 153 | `getAmountInBank (private)` | 0 | `items::amount_in_one_bank (private)` |  | via callers |
| 165 | `amountInEquipment` | 1 | `items::amount_in_equipment` |  | 6 |
| 178 | `inInventory` | 238 | `items::in_inventory` | world/gathering/binding.rs (inline) | 2 |
| 189 | `inBank` | 11 | `items::in_bank` |  | 3 |
| 200 | `inEquipment` | 55 | `items::in_equipment` |  | 2 |
| 210 | `anyInInventory` | 6 | `items::any_in_inventory` |  | 4 |
| 223 | `inEquipmentOrInventory` | 9 | `items::in_equipment_or_inventory` |  | 4 |
| 232 | `allInEquipment` | 8 | `items::all_in_equipment` |  | 2 |
| 244 | `anyInEquipment` | 13 | `items::any_in_equipment` |  | 3 |
| 256 | `getItemFromEquipment` | 18 | `items::get_item_from_equipment` | world/global_items2 | 3 |
| 277 | `hasAnItem` | 41 | `items::has_an_item` |  | 18 |
| 288 | `hasAnItem` | 41 | `items::has_an_item` |  | 18 |
| 336 | `hasGodItem` | 3 | `items::has_god_item` |  | 9 |
| 346 | `shouldRemoveNothings` | 1 | `items::should_remove_nothings` |  | 3 |
| 357 | `removeItem` | 296 | `items::remove_item` |  | 19 |
| 384 | `removeItemsIfPlayerHasEnough` | 11 | `items::remove_items_if_player_has_enough` |  | 8 |
| 404 | `addItem` | 198 | `items::add_item` | world/gathering/binding.rs (inline) | 17 |
| 429 | `replaceSlot` | 30 | `items::replace_slot` | world/consumables | 21 |
| 467 | `replaceAllItems` | 1 | `items::replace_all_items` |  | 31 |
| 508 | `addItemOrDrop` | 192 | `items::add_item_or_drop` |  | 14 |
| 526 | `addItemOrBank` | 2 | `items::add_item_or_bank` |  | 25 |
| 544 | `poofClear` | 34 | `world::poof_clear` |  | 7 |
| 567 | `freeSlots` | 53 | `items::free_slots` |  | 1 |
| 576 | `getAnimation` | 16 | `visuals::get_animation` |  | 1 |
| 586 | `getAnimationWithPriority` | 0 | `visuals::get_animation_with_priority` |  | 1 |
| 594 | `resetAnimator` | 13 | `visuals::reset_animator` | world/global_items | 2 |
| 603 | `renderAnimation` | 1 | `visuals::render_animation` |  | 3 |
| 612 | `resetRenderAnimation` | 1 | `visuals::reset_render_animation` |  | 4 |
| 623 | `animationDuration` | 14 | `visuals::animation_duration` |  | 1 |
| 627 | `animationCycles` | 2 | `visuals::animation_cycles` |  | 1 |
| 638 | `rewardXP` | 101 | `services::reward_xp` |  | 2 |
| 649 | `replaceScenery` | 47 | `visuals::replace_scenery / replace_scenery_facing` |  | 22 |
| 670 | `addScenery` | 8 | `visuals::add_scenery / add_existing_scenery` |  | 5 |
| 676 | `addScenery` | 8 | `visuals::add_scenery / add_existing_scenery` |  | 5 |
| 684 | `removeScenery` | 10 | `visuals::remove_scenery` |  | 1 |
| 696 | `replaceScenery` | 47 | `visuals::replace_scenery / replace_scenery_facing` |  | 22 |
| 724 | `getItemName` | 19 | `visuals::get_item_name` |  | 3 |
| 732 | `removeGroundItem` | 4 | `visuals::remove_ground_item` |  | 1 |
| 741 | `isValidGroundItem` | 0 | `visuals::is_valid_ground_item` |  | 2 |
| 751 | `hasSpaceFor` | 24 | `items::has_space_for` |  | 1 |
| 758 | `getWorldTicks` | 21 | `world::get_world_ticks` |  | 1 |
| 767 | `playJingle` | 17 | `visuals::play_jingle` |  | 2 |
| 777 | `impact` | 57 | `visuals::impact` |  | 5 |
| 787 | `itemDefinition` | 2 | `visuals::item_definition` |  | 2 |
| 801 | `hasOption` | 2 | `visuals::has_option` |  | 4 |
| 813 | `animateScenery` | 17 | `visuals::animate_scenery_for / animate_scenery` |  | 5 |
| 820 | `animateScenery` | 17 | `visuals::animate_scenery_for / animate_scenery` |  | 5 |
| 829 | `produceGroundItem` | 15 | `visuals::produce_ground_item / produce_ground_item_at` |  | 7 |
| 841 | `produceGroundItem` | 15 | `visuals::produce_ground_item / produce_ground_item_at` |  | 7 |
| 848 | `spawnProjectile` | 11 | `visuals::spawn_projectile / spawn_projectile_at` |  | 7 |
| 863 | `spawnProjectile` | 11 | `visuals::spawn_projectile / spawn_projectile_at` |  | 7 |
| 892 | `face` | 80 | `world::face` | world/global_items2 (inlined in dialogues) | 6 |
| 910 | `resetFace` | 2 | `world::reset_face` |  | 3 |
| 920 | `openInterface` | 86 | `ui::open_interface` |  | 1 |
| 929 | `openOverlay` | 20 | `ui::open_overlay` |  | 1 |
| 937 | `closeOverlay` | 16 | `ui::close_overlay` |  | 1 |
| 946 | `emote` | 2 | `visuals::emote` |  | 1 |
| 959 | `sendMessage` | 378 | `ui::send_message` |  | 6 |
| 968 | `sendChat` | 110 | `ui::send_chat` | world/global_items2 (no-delay form) | 13 |
| 982 | `sendDialogue` | 206 | `ui::send_dialogue` |  | 4 |
| 991 | `sendDialogueLines` | 22 | `ui::send_dialogue_lines` |  | 1 |
| 1001 | `sendDialogueOptions` | 17 | `ui::send_dialogue_options` |  | 1 |
| 1011 | `animate` | 226 | `visuals::animate` | world/global_items; world/global_items2 | 5 |
| 1035 | `playAudio` | 134 | `visuals::play_audio` |  | 4 |
| 1048 | `playGlobalAudio` | 55 | `visuals::play_global_audio` |  | 5 |
| 1059 | `playHurtAudio` | 3 | `visuals::play_hurt_audio` |  | 8 |
| 1075 | `openDialogue` | 243 | `ui::open_dialogue` | world/dialogue_dsl/api.rs | 8 |
| 1088 | `closeDialogue` | 5 | `ui::close_dialogue` |  | 3 |
| 1098 | `findNPC` | 15 | `world::find_npc / find_npc_near` |  | 5 |
| 1108 | `getScenery` | 33 | `visuals::get_scenery / get_scenery_at` |  | 2 |
| 1116 | `getScenery` | 33 | `visuals::get_scenery / get_scenery_at` |  | 2 |
| 1126 | `findNPC` | 15 | `world::find_npc / find_npc_near` |  | 5 |
| 1136 | `findLocalNPC` | 20 | `world::find_local_npc` |  | 2 |
| 1146 | `findLocalNPCs` | 9 | `world::find_local_npcs_at / find_local_npcs` |  | 3 |
| 1156 | `findLocalNPCs` | 9 | `world::find_local_npcs_at / find_local_npcs` |  | 3 |
| 1166 | `findLocalNPCs` | 9 | `world::find_local_npcs_at / find_local_npcs` |  | 3 |
| 1174 | `getRegionBorders` | 12 | `world::get_region_borders` |  | 1 |
| 1184 | `getAttribute` | 303 | `state::get_attribute` |  | 3 |
| 1194 | `setAttribute` | 250 | `state::set_attribute` |  | 3 |
| 1198 | `removeAttribute` | 104 | `state::remove_attribute` |  | 2 |
| 1202 | `removeAttributes` | 15 | `state::remove_attributes` | world/player_tabs (pub(crate)) | 3 |
| 1211 | `registerTimer` | 22 | `state::register_timer` | world/consumables/join.rs | 2 |
| 1223 | `getOrStartTimer` | 14 | `state::get_or_start_timer / get_or_start_timer_of` | timers/content (get_or_start) | 12 |
| 1237 | `getOrStartTimer (inline reified)` | 14 | `state::get_or_start_timer / get_or_start_timer_of` | timers/content (get_or_start) | 12 |
| 1250 | `spawnTimer` | 14 | `state::spawn_timer / spawn_timer_of` | timers/content (spawn); world/consumables/join.rs (spawn_timer) | 3 |
| 1260 | `spawnTimer (inline reified)` | 14 | `state::spawn_timer / spawn_timer_of` | timers/content (spawn); world/consumables/join.rs (spawn_timer) | 3 |
| 1270 | `hasTimerActive (inline reified)` | 14 | `state::has_timer_active / has_timer_active_of` | timers/content (has, has_identifier) | 2 |
| 1280 | `getTimer (inline reified)` | 9 | `state::get_timer / get_timer_of` | timers/content (get) | 3 |
| 1289 | `removeTimer (inline reified)` | 13 | `state::remove_timer / remove_timer_instance` | world/consumables/join.rs; world/live/timers.rs | 3 |
| 1299 | `hasTimerActive` | 14 | `state::has_timer_active / has_timer_active_of` | timers/content (has, has_identifier) | 2 |
| 1309 | `getTimer` | 9 | `state::get_timer / get_timer_of` | timers/content (get) | 3 |
| 1318 | `removeTimer` | 13 | `state::remove_timer / remove_timer_instance` | world/consumables/join.rs; world/live/timers.rs | 3 |
| 1327 | `removeTimer` | 13 | `state::remove_timer / remove_timer_instance` | world/consumables/join.rs; world/live/timers.rs | 3 |
| 1336 | `lock` | 120 | `state::lock` |  | 1 |
| 1345 | `lockInteractions` | 10 | `state::lock_interactions` |  | 1 |
| 1353 | `unlock` | 43 | `state::unlock` |  | 1 |
| 1363 | `transformNpc` | 7 | `world::transform_npc` |  | 4 |
| 1379 | `transformNpcWithHpCarryover` | 1 | `world::transform_npc_with_hp_carryover` |  | 7 |
| 1395 | `location` | 35 | `world::location` |  | 5 |
| 1402 | `inBorders` | 22 | `world::in_borders / in_corners` |  | 2 |
| 1409 | `inBorders` | 22 | `world::in_borders / in_corners` |  | 2 |
| 1416 | `heal` | 4 | `services::heal` |  | 1 |
| 1420 | `getVarp` | 16 | `vars::get_varp` | player/variables.rs (wrapped by content_api::vars) | 2 |
| 1424 | `getVarbit` | 34 | `vars::get_varbit / get_varbit_definition` | player/variables.rs (wrapped) | 4 |
| 1430 | `getVarbit` | 34 | `vars::get_varbit / get_varbit_definition` | player/variables.rs (wrapped) | 4 |
| 1436 | `setVarp` | 82 | `vars::set_varp` | player/variables.rs (wrapped) | 5 |
| 1443 | `saveVarp` | 1 | `vars::save_varp` | player/variables.rs (wrapped) | 1 |
| 1447 | `unsaveVarp` | 0 | `vars::unsave_varp` | player/variables.rs (wrapped) | 1 |
| 1452 | `setVarbit` | 99 | `vars::set_varbit / set_varbit_definition` | player/variables.rs (wrapped) | 5 |
| 1460 | `setVarbit` | 99 | `vars::set_varbit / set_varbit_definition` | player/variables.rs (wrapped) | 5 |
| 1471 | `setVarc` | 1 | `vars::set_varc` | player/variables.rs (wrapped) | 2 |
| 1476 | `reinitVarps` | 0 | `vars::reinit_varps` | player/variables.rs (wrapped) | 6 |
| 1488 | `forceWalk` | 23 | `world::force_walk` |  | 7 |
| 1504 | `truncateLoc` | 0 | `world::truncate_loc` |  | 7 |
| 1529 | `forceMove` | 20 | `world::force_move (+ run_force_move)` |  | 29 |
| 1583 | `stopWalk` | 15 | `world::stop_walk` |  | 2 |
| 1590 | `getChildren` | 0 | `visuals::get_children` |  | 3 |
| 1599 | `adjustCharge` | 0 | `visuals::adjust_charge` |  | 5 |
| 1612 | `getCharge` | 3 | `visuals::get_charge` |  | 3 |
| 1626 | `setCharge` | 3 | `visuals::set_charge` |  | 3 |
| 1639 | `getUsedOption` | 8 | `state::get_used_option` | world/global_items2; world/consumables | 3 |
| 1650 | `visualize` | 118 | `visuals::visualize` | world/global_items2 | 4 |
| 1670 | `submitWorldPulse` | 19 | `state::submit_world_pulse` |  | 2 |
| 1679 | `runWorldTask` | 1 | `state::run_world_task` |  | 3 |
| 1697 | `teleport` | 136 | `world::teleport` | world/global_items | 5 |
| 1711 | `setTempLevel` | 1 | `services::set_temp_level` |  | 1 |
| 1721 | `getStatLevel` | 30 | `services::get_stat_level` |  | 1 |
| 1731 | `getDynLevel` | 55 | `services::get_dyn_level` | world/gathering/binding.rs (inline) | 1 |
| 1741 | `adjustLevel` | 4 | `services::adjust_level` |  | 4 |
| 1751 | `removeAll` | 6 | `items::remove_all` |  | 30 |
| 1785 | `setInterfaceText` | 47 | `ui::set_interface_text` |  | 1 |
| 1796 | `setComponentVisibility` | 15 | `ui::set_component_visibility` |  | 1 |
| 1807 | `setTitle` | 2 | `ui::set_title` |  | 8 |
| 1854 | `sendString` | 3 | `ui::set_interface_text` |  | 1 |
| 1881 | `removeTabs` | 1 | `ui::remove_tabs` |  | 2 |
| 1889 | `restoreTabs` | 1 | `ui::restore_tabs` |  | 1 |
| 1897 | `closeInterface` | 38 | `ui::close_interface` |  | 1 |
| 1905 | `closeTabInterface` | 3 | `ui::close_tab_interface` |  | 1 |
| 1913 | `closeAllInterfaces` | 12 | `ui::close_all_interfaces` |  | 4 |
| 1926 | `sendPlayerDialogue` | 33 | `ui::send_player_dialogue` |  | 2 |
| 1936 | `sendPlayerOnInterface` | 1 | `ui::send_player_on_interface` |  | 1 |
| 1948 | `sendNPCDialogue` | 62 | `ui::send_npc_dialogue` |  | 2 |
| 1960 | `sendNPCDialogueLines` | 2 | `ui::send_npc_dialogue_lines` |  | 4 |
| 1972 | `sendAnimationOnInterface` | 3 | `ui::send_animation_on_interface` |  | 1 |
| 1981 | `registerLogoutListener` | 5 | `ui::register_logout_listener` |  | 1 |
| 1990 | `clearLogoutListener` | 6 | `ui::clear_logout_listener` |  | 2 |
| 2002 | `sendItemOnInterface` | 4 | `ui::send_item_on_interface` |  | 2 |
| 2014 | `sendItemZoomOnInterface` | 7 | `ui::send_item_zoom_on_interface` |  | 2 |
| 2024 | `sendItemDialogue` | 64 | `ui::send_item_dialogue` |  | 5 |
| 2043 | `sendDoubleItemDialogue` | 11 | `ui::send_double_item_dialogue` |  | 5 |
| 2057 | `sendInputDialogue` | 29 | `ui::send_input_dialogue / send_input_dialogue_typed` | world/dialogue_dsl/api.rs (both overloads) | 14 |
| 2069 | `sendInputDialogue` | 29 | `ui::send_input_dialogue / send_input_dialogue_typed` | world/dialogue_dsl/api.rs (both overloads) | 14 |
| 2094 | `flee` | 1 | `world::flee` |  | 5 |
| 2109 | `submitIndividualPulse` | 37 | `state::submit_individual_pulse` |  | 1 |
| 2116 | `runTask` | 39 | `state::run_task` | world/global_items2 (slice bodies) | 5 |
| 2131 | `getQuestPoints` | 5 | `quests::get_quest_points` |  | 1 |
| 2138 | `getQuestStage` | 253 | `quests::get_quest_stage` |  | 1 |
| 2145 | `setQuestStage` | 138 | `quests::set_quest_stage` |  | 5 |
| 2153 | `isQuestInProgress` | 22 | `quests::is_quest_in_progress` |  | 5 |
| 2157 | `isQuestStarted` | 1 | `quests::is_quest_started` |  | 3 |
| 2164 | `isQuestComplete` | 78 | `quests::is_quest_complete` |  | 3 |
| 2174 | `getQuest` | 1 | `quests::get_quest` |  | 2 |
| 2182 | `startQuest` | 7 | `quests::start_quest` |  | 5 |
| 2193 | `finishQuest` | 32 | `quests::finish_quest` |  | 1 |
| 2202 | `sceneryDefinition` | 0 | `visuals::scenery_definition` |  | 2 |
| 2211 | `registerMapZone` | 2 | `world::register_map_zone` |  | 3 |
| 2223 | `animateInterface` | 5 | `visuals::animate_interface` |  | 1 |
| 2232 | `addClimbDest` | 8 | `world::add_climb_dest` |  | 1 |
| 2240 | `sendNews` | 6 | `ui::send_news` |  | 1 |
| 2249 | `sendGraphics` | 23 | `visuals::send_graphics` |  | 3 |
| 2262 | `runcs2` | 0 | `ui::runcs2` |  | 4 |
| 2290 | `sendItemSelect` | 2 | `ui::send_item_select` |  | 7 |
| 2304 | `announceIfRare` | 1 | `services::announce_if_rare` | world/drop_tables/drop.rs | 4 |
| 2311 | `hasRequirement` | 56 | `quests::has_requirement / has_quest_requirement` |  | 170 |
| 2346 | `hasRequirement` | 56 | `quests::has_requirement / has_quest_requirement` |  | 170 |
| 2356 | `getMasteredSkillNames` | 1 | `services::get_mastered_skill_names` |  | 27 |
| 2383 | `dumpContainer` | 2 | `items::dump_container` |  | 42 |
| 2433 | `dumpBeastOfBurden` | 2 | `items::dump_beast_of_burden` |  | 26 |
| 2479 | `getFamiliarBoost` | 5 | `services::get_familiar_boost` |  | 1 |
| 2491 | `note` | 1 | `items::note` |  | 4 |
| 2509 | `unnote` | 1 | `items::unnote` |  | 4 |
| 2522 | `hasSealOfPassage` | 1 | `items::has_seal_of_passage` |  | 2 |
| 2531 | `hasHouse` | 3 | `services::has_house` |  | 1 |
| 2535 | `Player.getCutscene` | 1 | `services::get_cutscene` |  | 3 |
| 2547 | `storeBookInHouse` | 28 | `services::store_book_in_house / store_book_node_in_house` |  | 28 |
| 2596 | `storeBookInHouse` | 28 | `services::store_book_in_house / store_book_node_in_house` |  | 28 |
| 2601 | `Player.getCutsceneStage` | 0 | `services::get_cutscene_stage` |  | 2 |
| 2605 | `getServerConfig` | 0 | `services::get_server_config` | config (Boolean access) | 2 |
| 2609 | `getPathableRandomLocalCoordinate` | 6 | `world::get_pathable_random_local_coordinate` |  | 7 |
| 2633 | `getPathableCardinal` | 1 | `world::get_pathable_cardinal` |  | 5 |
| 2651 | `getSlayerTask` | 0 | `services::get_slayer_task` |  | 2 |
| 2661 | `getSlayerTaskName` | 2 | `services::get_slayer_task_name` |  | 2 |
| 2671 | `getSlayerTaskKillsRemaining` | 2 | `services::get_slayer_task_kills_remaining` |  | 1 |
| 2681 | `getSlayerMaster` | 1 | `services::get_slayer_master` |  | 6 |
| 2691 | `getSlayerMasterLocation` | 1 | `services::get_slayer_master_location` | world/global_items2/dialogues.rs | 6 |
| 2708 | `getSlayerTip` | 1 | `services::get_slayer_tip` |  | 9 |
| 2722 | `getSlayerTaskFlags` | 1 | `services::get_slayer_task_flags` |  | 3 |
| 2726 | `getSlayerFlags` | 2 | `services::get_slayer_flags` |  | 1 |
| 2736 | `hasSlayerTask` | 1 | `services::has_slayer_task` |  | 1 |
| 2747 | `hasIronmanRestriction` | 5 | `services::has_ironman_restriction` |  | 5 |
| 2759 | `restrictForIronman` | 3 | `services::restrict_for_ironman` | world/global_handlers/tabs.rs | 4 |
| 2771 | `openGrandExchange` | 0 | `services::open_grand_exchange` |  | 4 |
| 2783 | `hasAwaitingGrandExchangeCollections` | 3 | `services::has_awaiting_grand_exchange_collections` | world/grand_exchange/records.rs | 6 |
| 2804 | `openGrandExchangeCollectionBox` | 6 | `services::open_grand_exchange_collection_box` |  | 5 |
| 2819 | `openBankAccount` | 9 | `services::open_bank_account` |  | 4 |
| 2833 | `openDepositBox` | 2 | `services::open_deposit_box` |  | 4 |
| 2847 | `openBankPinSettings` | 4 | `services::open_bank_pin_settings` |  | 4 |
| 2866 | `activateSecondaryBankAccount` | 1 | `services::activate_secondary_bank_account` |  | 26 |
| 2909 | `hasActivatedSecondaryBankAccount` | 4 | `services::has_activated_secondary_bank_account` |  | 1 |
| 2922 | `toggleBankAccount` | 4 | `services::toggle_bank_account` |  | 6 |
| 2938 | `isUsingSecondaryBankAccount` | 2 | `services::is_using_secondary_bank_account` |  | 2 |
| 2951 | `getBankAccountName` | 2 | `services::get_bank_account_name` |  | 4 |
| 2966 | `openNpcShop` | 37 | `services::open_npc_shop` |  | 3 |
| 2991 | `sendSkillDialogue` | 7 | `ui::send_skill_dialogue (+ skill_create, skill_get_all)` |  | 12 |
| 3049 | `registerHintIcon` | 23 | `ui::register_hint_icon_at / register_hint_icon` |  | 9 |
| 3058 | `registerHintIcon` | 23 | `ui::register_hint_icon_at / register_hint_icon` |  | 9 |
| 3068 | `clearHintIcon` | 20 | `ui::clear_hint_icon` |  | 6 |
| 3079 | `equipSlot` | 1 | `items::equip_slot` |  | 4 |
| 3091 | `updateCredits` | 0 | `services::update_credits` |  | 5 |
| 3107 | `getCredits` | 0 | `services::get_credits` |  | 1 |
| 3118 | `requireQuest` | 4 | `quests::require_quest` |  | 4 |
| 3132 | `isPlayer` | 1 | `world::is_player` |  | 2 |
| 3141 | `addDialogueAction` | 12 | `ui::add_dialogue_action` |  | 1 |
| 3151 | `log` | 49 | `services::log` |  | 5 |
| 3161 | `logWithStack` | 0 | `services::log_with_stack` |  | 1 |
| 3174 | `exceptionToString` | 0 | `services::exception_to_string` |  | 1 |
| 3184 | `finishedMoving` | 4 | `state::finished_moving` |  | 6 |
| 3192 | `delayScript` | 67 | `state::delay_script` |  | 2 |
| 3200 | `delayEntity` | 6 | `state::delay_entity` | world/global_items | 3 |
| 3205 | `apRange` | 0 | `state::ap_range` |  | 3 |
| 3210 | `hasLineOfSight` | 2 | `world::has_line_of_sight` |  | 1 |
| 3214 | `animationFinished` | 3 | `state::animation_finished` |  | 6 |
| 3218 | `clearScripts` | 3 | `state::clear_scripts` |  | 1 |
| 3223 | `restartScript` | 2 | `state::restart_script` |  | 7 |
| 3231 | `keepRunning` | 18 | `state::keep_running` |  | 2 |
| 3236 | `stopExecuting` | 110 | `state::stop_executing` | world/consumables | 7 |
| 3244 | `queueScript` | 114 | `state::queue_script (+ run_script)` | world/consumables | 4 |
| 3258 | `delayClock` | 14 | `state::delay_clock` |  | 4 |
| 3272 | `clockReady` | 14 | `state::clock_ready` |  | 3 |
| 3276 | `delayAttack` | 2 | `state::delay_attack` | world/consumables | 2 |
| 3281 | `stun` | 5 | `state::stun` | world/special_attacks/melee.rs | 10 |
| 3285 | `stun` | 5 | `state::stun` | world/special_attacks/melee.rs | 10 |
| 3300 | `isStunned` | 2 | `state::is_stunned` |  | 6 |
| 3311 | `applyPoison` | 12 | `state::apply_poison` | timers/content (apply_poison) | 19 |
| 3332 | `isPoisoned` | 5 | `state::is_poisoned` | timers/content | 1 |
| 3336 | `curePoison` | 5 | `state::cure_poison` | timers/content | 6 |
| 3344 | `setCurrentScriptState` | 1 | `state::set_current_script_state` |  | 6 |
| 3359 | `modPrayerPoints` | 1 | `services::mod_prayer_points` |  | 3 |
| 3370 | `decantContainer` | 2 | `items::decant_container` |  | 15 |
| 3409 | `hasDragonfireShieldProtection` | 0 | `services::has_dragonfire_shield_protection` | world/dragonfire/api.rs | 14 |
| 3429 | `calculateDragonfireMaxHit` | 3 | `services::calculate_dragonfire_max_hit` | world/dragonfire/api.rs | 48 |
| 3476 | `openSingleTab` | 6 | `ui::open_single_tab` |  | 1 |
| 3487 | `finishTask` | 9 | `quests::finish_task` |  | 3 |
| 3498 | `getDiaryLevelIndex` | 0 | `quests::get_diary_level_index` |  | 12 |
| 3514 | `handleDoor` | 4 | `world::handle_door` |  | 2 |
| 3528 | `handleAutowalkDoor` | 53 | `world::handle_autowalk_door` |  | 4 |

Extension functions in ContentAPI.kt: `Player.getCutscene()`,
`Player.getCutsceneStage()` (in the table). Classes and enums declared in
ContentAPI.kt: `ContainerisedItem` (`items::ContainerisedItem`), `GAMETAB`
(`GameTab`), `SecondaryBankAccountActivationResult`
(`services::SecondaryBankActivation`), `SkillDialogueBuilder`
(`ui::send_skill_dialogue`'s arguments and `SkillHandler`).

## The other core/api files

| File | Here | Corpus |
| --- | --- | --- |
| ApiExtensions.kt | `api_files::{as_item, progression_to_int_array, flatten_int_arrays, is_last, get_next, is_next_last, try_pop, parse_enum_entry}` | yes |
| Commands.kt | `api_files::{reject, notify, define}` (CommandMapping.register a service) | yes |
| Container.kt | `Container` | yes |
| ContentInterface.kt, LoginListener, LogoutListener, PersistPlayer, PersistWorld, ShutdownListener, StartupListener, TickListener | `api_files` traits | interfaces only |
| DialUtils.kt | `ui::split_lines` over `world::plain_dialogue::split_lines` | yes (18 cases) |
| EquipmentSlot.kt | `EquipmentSlot` (slotByName over world/commands/system.rs) | yes |
| Event.kt | `api_files::EVENTS` | yes |
| God.kt | `God` with `valid_items` (ConstLib values) | yes |
| IfaceSettingsBuilder.kt | `api_files::IfaceSettingsBuilder` | yes |
| InputType.kt | `InputType` | yes |
| MapArea.kt | `api_files::MapArea` trait, `map_area_zone`/`set_map_area_zone` over `Host::zone_maps` | yes |
| regionspec/ | `api_files::{build_region, RegionContract, ChunkContract, FillChunk}` over DynamicRegion/Region/RegionChunk services (`world/dynamic_region.rs` is the native DynamicRegion; binding it is the host's) | yes (empty, copy, using, fill, delegate, lateinit) |
| utils/Vector.kt | `api_files::Vector` | yes |
| utils/WeightBasedTable.kt, WeightedItem.kt, NPCDropTable.kt | world/drop_tables.rs (drop-tables corpus) | earlier slice |
| utils/WeightedTable.kt | compiled unchanged; **not ported** | no |
| utils/PlayerCamera.kt, CameraUtils.kt | compiled unchanged; **not ported** | no |
| utils/PlayerStatsCounter.kt | **not ported** (an SQLite statistics store; a service of `announceIfRare`) | no |
| utils/Permadeath.kt | **not ported** (resets most of a player; content) | no |
| utils/Exceptions.kt | `ConfigParseException`: not needed natively | no |

## Gaps

- No live binding: nothing implements `content_api::Host` over the native
  world (containers, skills, timers, scripts, interfaces, dialogue). The
  earlier slices' own bindings keep using their ports.
- Not ported: `WeightedTable`, `PlayerCamera`, `PlayerStatsCounter`,
  `Permadeath`.
- `decantContainer`'s HashMap order models buckets and insertion order,
  not JDK tree bins (eight or more potions colliding in one bucket of a
  table of 64 or more).
- `IntProgression.toIntArray`'s NegativeArraySizeException is an
  `Error::Argument`; the corpus does not reach it.
- `Error` messages are compared only through printed stack traces; the
  corpus records the exception class of a thrown error, not its message.
