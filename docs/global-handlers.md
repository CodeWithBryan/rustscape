# Global handlers (`world/global_handlers`)

`crates/server/src/world/global_handlers/` ports the global handlers a plain
playable world needs from `content/global/handlers/{player,iface,scenery,npc}`.
Each Kotlin lambda, function reference, ComponentPlugin and OptionHandler the
original registers is a typed native value (`registry::Listener`,
`Interaction`, `ComponentPlugin`, `OptionPlugin`, `LoginPlugin`); its body is a
function over the `Host` trait, where every call the original makes outside
the ported bodies is a required method with no default. Deferred bodies
(pulses, tasks, input callbacks, close events) are `Task` values the world
stores and runs through `run_task`/`stop_task`.

## What is ported

| Original | Rust | Scope |
| --- | --- | --- |
| `AttackOptionPlugin` | `player::attack`, `OptionPlugin::Attack` | `Option._P_ATTACK.setHandler`, `player.attack(node)`, `isWalk`/`isDelayed` false |
| `FollowOptionPlugin` | `player::follow`, `follow_pulse`, `follow_stop` | the Java `(Player)` cast (null passes), the FOLLOW_ENTITY MovementPulse run as STANDARD, its body (face the target, never finish) and stop (the MovementPulse stop, then `face(null)`) |
| `RequestOption` | `player::trade`, `player::assist` | the deprecated catch-all `on(option, PLAYER)` registrations; Kotlin's `as Player` (null is `Null`, another class `Cast`); the bot info before an assist request |
| `LoginValidationPlugin` | `player::login_validation` | unlock, artificial and valid-login returns, dev mode, the fight caves/falconry activities (attribute casts), the Dragon Slayer varp, the loot share decay `(int)(p - p * 0.10)` with both reads, the one-day delay, the quest point cape/hood return with every location branch and the `null` text when nothing was removed |
| `LogoutTabInterface`, `SettingsTabInterface` (with `restrictForIronman`), `StatsTabInterface` (SkillConfig, ADVANCE_CONFIGS), `EquipmentTabInterface` (items kept on death, worn items, equipment screen and its inventory), `CombatTabInterface` (with both pulses), `MagicTabInterface`, `PrayerTabInterface`, `QuestTabInterface`, `ClanTabInterface` (with the prefix input), `EmoteTabInterface`, `MusicTabInterface` | `tabs` | every listener body and branch |
| `MainGameInterface`, `LoginInterface`, `DeathInterface`, `CharacterDesignInterface`, `WorldMapInterface`, `ExperienceInterface`, `FriendsListInterface.open`, `RulesAndInfo`/`RulesListener`, `ScrollInterface.scrollSetup`, `GenericItemSelect`, `AutocastSelectPlugin`, `TeleotherInterface`, `RequestAssistInterface`, `BankDepositBoxInterface`, `BankChargeInterface` | `ifaces` | every listener/plugin body and helper |
| `BankBoothListener` (use-quickly/bank, use, collect, the note conversion and the banker search), `BankChestListener`, `BankDepositBoxListener`, `LookAtOptionPlugin`, `SearchOptionPlugin`, `LadderManagingPlugin` | `scenery` | every body (the ladder handler calls the native `climb`) |
| `NPCDepositListener`, `DummyAttackPlugin` | `npc` | both bodies |
| `ClimbActionHandler` (climbLadder, getDestination with its private four-direction search, climb and its pulse, getLadder, findLadder, isLadder), `ClimbDialogue`, `SpecialLadders` | `climb`, `special_ladders` | every body; the SpecialLadders table (60 constants with their diary and Zogre achievement overrides, first constant per location wins) is written from the unchanged enum by `oracle/global-handlers/port_special_ladders.py` (`--check` compares it) |
| the classes' registrations | `registry::define`, `register_scanned`, `Native` | each class's `defineInterfaceListeners`/`defineListeners`/`newInstance(null)` in the original's order; `register_scanned` walks the start-up manifest (content interfaces, then `@Initializable` plugins, RequestOption's second entry doing nothing as it is no Plugin, LoginValidationPlugin added to the login plugins) |

Already native elsewhere and not repeated here: `TradeInterfacePlugin`
(`world/trade`), `BankInterface` (`world/bank_interface.rs`),
`BankPinInterface` (`player/bank_pin.rs`), `BankUtils.transferX`
(`world/input_prompt.rs`), `DoorManagingPlugin` (`world/door_option.rs`,
`door_runtime.rs`, with DoorActionHandler in `world/doors.rs`).

## Behaviour that is easy to get wrong

- Registration: one ComponentPlugin object for all its ids (CombatTab: every
  WeaponInterfaces id then 92 again; AutocastSelect: 797, 319, 310, 406);
  MagicTab registers one callback per spell book; the booth's use-with
  listener only when `BANK_BOOTH_NOTE_ENABLED`.
- Java generic reads cast where they are assigned (`WeaponInterface w =
  getExtension(...)`, the item-select callback, `skillMenu`, `rules:pin`),
  Kotlin `when (getAttribute(..., ""))` casts to String; a stored value of
  another type is `Cast` at that point, before the next call.
- Eager defaults: `RulesAndInfo.openFor` draws `RandomFunction.random(1000,
  9999)` before reading `rules:pin`; TeleotherInterface reads the player's
  location before the attribute.
- MusicTabInterface reads `unlocked[slot]` twice; ClanTabInterface reads each
  requirement back twice (communication, then the MS packet); the prefix
  input renames on the MS server before it checks the clan (a null clan fails
  after that call).
- EquipmentTabInterface: `DeathTask.getContainers` twice, the kept count read
  once or twice by the skull short circuit, the log only for more than four
  kept items outside a safe zone, `operate` falls back to the operate handler
  and its answer.
- MainGameInterface: the STAFF check is an identity comparison with the enum
  constant, so only the constant itself opens the tab, and the second
  `getExtension<WeaponInterface>` then fails the cast. The world map hash reads
  the location three times.
- TeleotherInterface answers `interfaceManager.close()`'s result.
- The banker search: the first NPC found in the four directions ends the
  linear search even when it is no banker; the square search builds
  `Location(x, y)` on plane 0.
- The note conversion's free-slot rule keeps the amount when it is exactly one
  more than the free slots.
- DummyAttackPlugin: controlled style gives 5/3 to attack, strength and
  defence, in that order.
- Climbing: a name starting with "Stair" climbs without an animation; a
  SpecialLadders location climbs straight to its destination (the achievement
  after the pulse is submitted); "climb" opens the dialogue when both or
  neither ladder exists. `getLadder` moves the plane with Kotlin's
  `(z + dz) % 4`, so going down from plane 0 looks on plane -1 before the
  `-6400`/`+6400` basement offset; a non-ladder of the same name above is
  stepped over once; `findLadder` scans x outer from -5 to 5. `isLadder` needs
  an option containing "Climb" or the name Trapdoor. `getDestination` swaps
  the sizes for any odd rotation (Java's `-1 % 2`), tries the walking flag's
  direction (Java shift counts) or the rotation's, and turns clockwise at most
  three times; `canMove` tests the target tile's own clipping flag against
  every traversal mask.

## Required services and joins

The `Host` methods name the original call each answers. The world joins them
to: native Settings toggles; the InterfaceManager; native prayer
(`world::prayer::PrayerType`, `toggle`); native diaries
(`world::player_managers::diary::for_child`, `diary_data::TYPES`); the
MusicPlayer of `world::player_tabs::music`; the QuestRepository of
`world::player_tabs::quests`; Emotes (`player_tabs::emotes::handle`);
`world::trade::RequestManager` with `RequestType`; the assist session of
`world::player_managers::assist`; EquipHandler.unequip
(`world::packet_handlers::equip`); the InteractionListeners registry for the
operate/equip runs; the input prompt (`world::input_prompt::transfer_x`);
`world::social` for clans and `social::names::format_display_name` (called
natively here); `world::combat_swing::WeaponInterfaces` and
`world::spells::SpellBook` (used natively for registration); the
RegionManager object, teleport-permission and clipping-flag queries
(`world::region_queries`, `world::clipping`) and the diary `finishTask`; the
SpecialLadders maps are `climb::Ladders` (the world keeps one and answers
`special_destination`/`special_ladder` from it). Content stays a
named service: `QuestTabUtils.showRequirementsInterface` (the quest
requirement tables), `BankerNPC.checkLunarIsleRestriction`,
`ZogreFleshEatersListeners.ladderMakesSithikTurnIntoOgre`, `CharacterDesign`,
`ActivityManager.start`, `LoginConfiguration.configureGameWorld`, the special
attack handlers and the `caller`/item-select callback objects.

`registry::Native` installs the typed values into the native registries: the
InterfaceListeners registry (`Callback::button`/`open`), the component
definitions' plugins (`interfaces::Registry::for_id`), the
InteractionListeners registry (`add_ids`, the deprecated catch-all
`add_generic`, `add_metadata`, `add_any`) and the scenery option defaults
(`option_handler::set_option_handler`). Player option handlers
(`Option._P_ATTACK`/`_P_FOLLOW`), per-definition scenery handlers
(`option:look at`, `option:attack`) and login plugins are collected for the
world to bind into its option tables, definitions and login sequence.

## How the live world registers them

At start-up, after the registries exist: build `registry::Native` over the
world's InterfaceListeners registry, its `interfaces::Registry`, its
InteractionListeners registry with the world's casing, and the scenery option
defaults of `scenery_options::Registry::defaults()`; call
`registry::register_scanned(&mut native)`. Then bind `native.player_options`
into the `interaction_options::Constants` handles (Attack, Follow),
`native.scenery_handlers` into the scenery definitions' `option:` handler
properties, and run `native.login_plugins` from the login sequence
(`registry::run_login`). The interface button path (`world/live/buttons.rs`'s
`invoke_button`/`invoke_open`/`handle_plugin`) dispatches a callback's
`Listener`/`ComponentPlugin` value through `registry::run_button`,
`run_open`, `run_plugin`; option handlers through `run_handler` and
`handler_destination`; InteractionListeners functions through
`run_interaction`. Each `Task` the bodies hand over is stored with its pulse,
task, input or close event and run with `run_task`/`stop_task`. The live
world's own CombatTab, Emotes and Music button bodies are then replaced by
these.

## Reference tests

```powershell
cargo test -p rs09-server --test global_handlers
cargo xtask global-handlers-oracle --check --java-home <jdk11>
```

`oracle/global-handlers` compiles the originals listed in `UNCHANGED` of
generate.py unchanged on JDK 11 with Kotlin 1.8.20 and the pinned ConstLib
1.4.28 (Components, Items, NPCs, Scenery, Sounds and Animations constants
checked against their names), with the unchanged OptionHandler,
ComponentPlugin, CloseEvent, Pulse, Plugin, InterfaceListener, IntType,
core.api.Container, IfaceSettingsBuilder, ClimbActionHandler,
SpecialLadders, LadderAchievementCheck and Direction. Original text inserted at
generation time: the WeaponInterfaces constants with their component ids, the
SpellBook constants, the PrayerType and DiaryType constant names, the Skills
ids, `StringUtils.formatDisplayName` and `ContentAPI.restrictForIronman`.
Everything else is a recording adapter answering from the case; the adapters'
fields are named apart from the getters' Kotlin properties.

`fixtures/global-handlers/oracle.jsonl`: 1,766 cases (629 designed, 1,137
with one recorded call failing in turn), 3,558 operations, 28,091 recorded
calls, in 40 groups (registration, each listener, the login plugin, every
SpecialLadders constant and the climbing geometry, re-entry hooks). The test replays every operation through the native bodies
and registrations with a recording host that follows the adapters and
compares the result, the error, every recorded call (registrations included,
with the original class names and object identities) and the attributes and
values after each operation. Every case matches; there are no pinned gaps. The
same file checks that `register_scanned` over the start-up manifest produces
exactly the oracle's scan-order registration, and that `Native` installs the
typed objects into the native registries.

Forty-three deliberate breaks of the port were tried (check orders and
short circuits, ids, thresholds, re-reads, eager defaults, casts, the
registration order, class names and plugin identity, the banker search order
and plane, the note rule, the climbing plane modulo, scan orders, option
matching, walk-flag order, offsets, attempts, achievements, locks, the stair
prefix, the negative rotation swap and the dialogue end): every one fails the
replay. Two went undetected at first (the clan prefix's null clan read after
the MS rename, the size swap for a negative rotation); the cases that pin them
were added.

## Differences from the original

- A Kotlin lambda that captures nothing is one object however often its class
  registers; the port makes one callback object per registration (the
  original defines each class once at start-up, so only a repeated definition
  could tell them apart).
- The booth's `defineInteraction` handlers refuse a null node before their
  body (the registry never dispatches one).
- `GlobalData.setLootShareDelay` is compared relative to the clock: the
  original reads `System.currentTimeMillis()` itself; here the clock is the
  `current_time_millis` service.
- Errors carry a category, not a Java stack trace.

## Not ported or not joined

- `QuestTabUtils.showRequirementsInterface` (the `QuestRequirements` tables
  and their evaluation are quest content).
- `ClimbActionHandler.climbRope`/`climbTrapdoor` are empty in the original;
  `SpecialLadders.add` is ported (`Ladders::add`) but nothing calls it here.
- `NPCTalkListener`, `BankerNPC`, `ShopNPC` (an AbstractNPC; shops themselves
  are native in `world/shops`).
- Nothing installs the libraries in `rs09-server --world` yet (see above for
  how); skill-specific and creature-specific handlers are content.
