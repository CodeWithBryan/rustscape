# The live world (`rs09-server --world`, `world/live`)

`crates/server/src/world/live/` installs the verified libraries in a running
server: the native start-up, the login, the tick driver, the repository and
disconnection queue, social, the canonical players with their own libraries,
the player tab systems (weapon tab, level-up icons, emotes, music, quest tab),
walking on the canonical actor state, the player update with prepared masks,
the start-up's NPCs (tick, random walking, the NPC update), entity regions
and zone monitors with music zones, region activation, the map chunk renderer
with chunk synchronisation, the interface buttons of the player tabs, ground
items (the start-up's spawns, `GroundItemManager`, the ground item packet,
pickup), the region zones of `ZoneBuilder.startup` with the players'
`SkullManager`, the routed item, NPC, player, examine and track-finished
packets (`world/packet_handlers`) with `DropListener` and `EquipHandler` in
the native `InteractionListeners`, `EquipmentContainer.add/remove` and the
appearance block with worn equipment, the engine timers (`timers::content`:
`addAutoTimers` at login, `processTimers` every tick, the timers of the save
file, the entity event hooks and the timer queries of the walking queue,
the zones and the SkullManager), the global handlers
(`world::global_handlers`, registered by `register_scanned`: the tab
interfaces, the game-frame interfaces, the ladder, look-at and search option
handlers), the scenery option packet (`world::scenery_action` through
`world::scenery_runtime`), use-with (`processUseWith`) and the slot switch
(`processSlotSwitch`). A client logs in, enters the world,
walks, sees NPCs, players and ground items, uses its weapon, emote and music
tabs, takes, drops, moves, uses and wields items, examines, uses an NPC's
option, climbs a ladder, enters the wilderness, regains hitpoints over time,
logs out, and its character is saved and loaded again.

This is a playable engine slice, **not a complete server**: content (login
listeners and plugins, quests, tutorial, NPC combat, content item, NPC and
scenery handlers other than the global handlers, dialogue, banking, shops,
prayer, trade), combat, teleports, the dialogue interpreter, input prompts,
component use-with (spells) and every handler listed below as refused are
not installed, and every such call is answered by name below.

```powershell
cargo build --release -p rs09-server
target\release\rs09-server --world --cache <cache> --saves <dir> [--data <world dir>] [--tcp IP:PORT] [--websocket IP:PORT] [--verbose]
```

`--data` is the original's world directory (`worldprops/default.conf` and
`data/configs`); the default is `runs/startup-loaders-oracle/template` (the
world-trace template, which `cargo xtask startup-loaders-oracle` extracts).
`--saves` receives `players/<name>.json`, `serverstore/` and `world/`. Other
modes of `rs09-server` are unchanged.

## What runs

| Part | Original | Live world |
| --- | --- | --- |
| boot | `GameWorld.prompt` | `boot::boot`: `startup::sequence::start` (`ContinuePastRefusals`) over `<saves>/world`, whose `default.conf` is the data directory's with `data_path`, `cache_path`, `store_path` and `save_path` made absolute (nothing else changes); start-up writes are refused outside the saves directory |
| transport | `GameReadEvent`, `LoginReadEvent`, the socket | `runtime::Service` with `server::Gate` as its `session::World`; `World::enter_from` passes the peer address (`IoSession.getAddress`) |
| admission | `Login.proceedWith` â€¦ `LoginParser.initialize` | `login::admission::read` on the worker thread at the start of a tick |
| login pulse | LoginParser's `Pulse(1)` | `login::pulse::run` from `Driver::pulser` (`driver::Host::pulse_body`) |
| login reply | `LoginWriteEvent` | `login::write_response` â†’ `login::Host::write_bytes`, which completes the transport's admission with the same rights and index (the transport writes the identical bytes) |
| worker | `MajorUpdateWorker` | `server::worker_loop`: admissions, the read event, `Driver::tick`, the hand-offs, the management pump and social flush, delivery, the ping timeout (20 s) and the inactive-player failsafe; sleep to 600 ms |
| read event | `IoReadEvent`/`GameReadEvent.read` | `Driver::read` on the bytes of the frames the transport framed (re-encoded with the same size table); end of stream is `Driver::close` |
| packet routes | `PacketProcessor.process` | the driver's table; `Walk` (`player_walk::World::process`, i.e. `world::walk_request`), `Social` (`Social::process`), `DisplayUpdate` (client info and `interfaces::switch_window_mode`), `IfAction` and `CloseIface` (`interface_packets::if_action`/`close_iface`, `buttons.rs`), `GroundItemAction` (`ground_items.rs`), `SceneryAction` (`scenery.rs`), `ItemAction`, `NpcAction`, `PlayerAction`, `UseWith`, `SlotSwitch`, `Examine` and `TrackFinished` (`items.rs`) bound; the rest below |
| pulses | `GameWorld.Pulser.updateAll` | the driver's runner (login pulses, the combat tab's one-tick pulses), then the canonical state's runner (movement pulses) at the start of the tick-listener phase |
| `Player.tick` | `Entity.tick`, music, prayer â€¦ | `player_walk::World::entity_before_masks` (script phases through `script_runtime::phase`, the walking queue) and `prepare_masks`, then `MusicPlayer.tick`; the queue's entity is projected into the render view and the slot |
| player tabs | `WeaponInterface`, `LevelUp`, `EmoteManager`/`Emotes`, `MusicPlayer`, `QuestRepository.syncronizeTab` | `world::player_tabs` with `tabs::Tabs` as their host (the world-login test's binding): the weapon tab is created with the tab array (`create_weapon`), opened by `WeaponInterface.open` and refreshed by `updateInterface` from the equipment listener; `LevelUp.sendFlashingIcons` from `Skills.refresh`; the emote, music and quest tabs at login; the persisted emotes, songs and quest stages joined, not copied |
| tab buttons | `CombatTabInterface`, `EmoteTabInterface`, `MusicTabInterface` | the component plugin of interface 92 and every weapon interface (attack styles through `WeaponInterface.setAttackStyle`, `openAutocastSelect`, `selectAutoSpell`; retaliate and special bar in `Pulse(1, p)`), listeners of 464 (`Emotes.handle`) and 187 (play, loop, the admin unlock); their few content lines are written over the libraries (not differentially tested themselves) |
| NPCs | `NPC.init`, `NPC.tick`, `NPC.reset` | the start-up's NPCs in the live repository (same slots and indices) and in the scene's chunks; `npc_walk::tick` (the `world::npc` kernel on the canonical actor's queue, navigation with the behaviour's pathfinder and clipping supplier, water NPCs' `Pathfinder.findWater` over the scene's water clipping, the shared runner, `RandomFunction.RANDOM` of the start-up's generators); behaviour hooks through `npc_behaviour` with the start-up's NPC store; `NPC.onRegionInactivity` through `npc_behaviour::lifecycle` (`npc_life.rs`) |
| `Player.update` | scene graph, three renderers | the native rebuild writer (XTEA keys from the start-up), `player_render::render` with `RegionManager.getLocalPlayers(location, 15)` candidates, `npc_render::render` with `getLocalNPCs` candidates, `map_chunk_render::render` with `chunk_sync` (synchronise, update, `ClearRegionChunk`) |
| regions | `RegionManager.move`, `Region.flagActive`, `ZoneMonitor` | `entity_regions::move_entity` at login and for every walking move (players and NPCs), over the scene's regions and chunks; `region_activity::flag_active` (load, renderable NPCs) from the move and the map chunk renderer; a zone monitor per entity (`zones::Monitor` with `Live` as its host) with `checkMusicZones` over the start-up's region music and music zones; `Entity.clear`'s chunk and tolerance part; `ZoneMonitor.clear` at logout |
| `Player.reset` | `Entity.reset` â€¦ | masks reset, teleport flag, render information, hitpoints packet, `flagged-for-save`, op counts; `ChunkUpdateTracker.resetAll` clears the scene's chunk update flags |
| `Animator.reset` | `entity.animator.reset()` | `prepared_animation` animators over the canonical masks and clocks, definitions from cache archive 20 |
| collision | `RegionManager` | `world::scene` over cache archive 5, regions loaded around players as they arrive |
| saves | `PlayerParser.saveImmediately`, `SaveTask`, autosave | the save text is built by the canonical player (`Player::save_text`) on the worker thread after the tick; the `PlayerSave` thread writes `<saves>/players/<name>.json`; the autosave every 50 ticks and a final save of every playing character at shutdown |
| account | `details.save()` | the account storage `Auth.configure` made from the configuration (in memory with `persist_accounts = false`) |
| ground items | `GroundItemManager`, `GroundSpawn`, `GroundSpawnLoader` | `ground_items.rs`: the start-up's manager list, spawn registry and `ItemDefinition.OPTION_HANDLERS` (the loader ran in the start-up), each item joined to the scene chunk of its location in manager order; `world::ground_manager` (create, destroy, get, increase, `pulse` from the driver's update sequence) over the scene's chunks and dirty tracker; respawns submitted to `GameWorld.Pulser` (the driver's runner) and dispatched by identity; `GroundSpawn` answers its own privacy and activity in the manager and in chunk synchronisation; a departed dropper's uid and name stay readable |
| ground item packet, pickup | `processGroundItemAction`, `PickupPlugin`, `PickupHandler.take` | `world::ground_runtime` with the walk host as `legacy_runtime::Bindings` (`InteractPlugin.handle` over the canonical legacy scheduler: movement, then `interaction_arrival`), `world::pickup` at arrival over the canonical inventory, attributes, Ironman, pickup types, animator (535) and sound (2582) |
| item, NPC, player, examine, track-finished packets | `processItemAction`, `InteractPlugin.handleItemOption` and `InteractPlugin$1`, `processNpcAction`, `processPlayerAction`, the examine branches, `TrackFinished` | `items.rs`: `world::packet_handlers` over the live world; an inventory item's InteractPlugin is a table made the first time it is asked for (`setDefault` over its definition's actions and `OPTION_HANDLERS`), an NPC's over its definition's options, a player's the one login set up; the item pulse on `GameWorld.Pulser`; `InteractPlugin.handle`/`handleInvalidInteraction` through the legacy runtime (walk, arrival, "Nothing interesting happens." with no content handler); `InteractionListeners` is the native registry (`world::interaction_listeners`) with `DropListener` and `EquipHandler` registered at boot (generic ITEM options) |
| drop, equip | `DropListener`, `EquipHandler`, `EquipmentContainer` | the drop's soft queued script (due a tick later), `Item.getDropItem`, `GroundItemManager.create`, `droppedItem:ID`, the save task; `EquipHandler.handleEquip` with `EquipmentContainer.add` (`inventory::equipment::add_at` over the live equipment context: `ItemDefinition.hasRequirement` (`items::has_requirement`, JDK HashMap key order), the EquipmentListener and the InventoryListener of the canonical containers); the canonical equipment container is an `EquipmentContainer` bound to its player with its listener |
| appearance | `Appearance.sync`, `prepareBodyData` | `Inner::appearance_sync`: the canonical appearance's `prepareBodyData` over the worn equipment, the prepared body copied into the appearance block's data, then the mask; at login and from the EquipmentListener |
| engine timers | `TimerRegistry` (the class scan's timers), `addAutoTimers`, `TimerManager.processTimers`, `saveTimers`/`parseTimers`, `Entity.hook`/`unhook`/`dispatch`, `hasTimerActive` | `timers.rs`: the fourteen `timers::content` prototypes in the world's registry (registered at the first timer operation, in scan order); `addAutoTimers` at `Entity.init` (AntiMacro, SkillRestore); `processTimers` for every player at the end of `Entity.tick`, the timer manager of the canonical profile (its three lists aliased); `timers::content::Natives` over `timers::Host` (the live world: native Skills through the live skills and settings contexts, the shared generator, messages, audio, equipment, rights, uids, the zone monitor's random-event restriction, the SkullManager, the per-entity hook lists) and over `timers::Saving` while a save is written or parsed (uids, ticks, the generator, the log; the messages and skull calls of a timer the parse registers are made right after the parse); `hasTimerActive` answers the walking queue (`frozen`), the zones (`teleblock`) and `isPoisonImmune`; the SkullManager's `removeTimer`, `spawnTimer` and `registerTimer` go through the registry |
| global handlers | `ClassScanner` of the global handler classes, `InterfaceListeners`, `ComponentDefinition` plugins, OptionHandlers, `ClimbActionHandler` | `global.rs`: `registry::register_scanned` over the live InterfaceListeners, the interface definitions' plugins, the native InteractionListeners and the scenery option defaults at boot; the per-definition scenery handlers bound into the scenery definitions; buttons, open and close listeners and component plugins dispatched through `run_button`, `run_open` and `run_plugin` (`buttons.rs`); scenery OptionHandlers through `run_handler` and `handler_destination` (`destination.rs`, over the scene); the bodies' `Task`s submitted to `GameWorld.Pulser` and run with `run_task`; `GHost` is the handlers' `Host` (attributes, messages, interface text, the weapon, emote and music tabs, settings toggles, locks, the animator, the teleport location, scenery geometry, SpecialLadders) |
| scenery options | `processSceneryAction`, scenery `InteractPlugin` | `scenery.rs`: `world::scenery_runtime` (the legacy walk and arrival, `world::scenery_action`) over the cache scene's objects, `scenery_options::Registry` (the scenery tables and handler arena with the global handlers' defaults), `Scenery.getChild` over the scene's definitions, the InteractionListeners lookup for scenery |
| use-with, slot switch | `processUseWith`, `processSlotSwitch` | `items/use_with.rs`, `items/slots.rs`: `world::packet_handlers::use_with` (zone monitor, `InteractionListeners.run_use`, PluginInteractionManager, `UseWithHandler.HANDLERS`, `runUnhandledAction`) and `slot_switch` over the canonical inventory |
| region zones | `ZoneBuilder.startup` (a StartupListener), `SkullManager` | `region_zones.rs`: `wilderness_zone::startup` at boot (after the NPCs join, as the start-up runs startup listeners after NPCSpawner) registers WildernessZone, MultiwayCombatZone, ModeratorZone, DarkZone, KaramjaZone and BankZone in the scene's regions, where the zone monitors find them (entities already in a region enter, aggressive wilderness NPCs get the WILDERNESS AggressiveHandler); `Live` is `zone_impls::Host`, `dark_zone::Host` and `wilderness_zone::Host`; each player's `SkullManager` (`world::player_managers::skull`) answers the skull services; overlays through the interface manager |

## Deviations from the original

- Frames from the client are framed by the transport and re-encoded for the
  read event; a malformed size closes the connection in the transport
  instead of counting as an invalid packet.
- `GameWorld.Pulser` is two runners: the driver's (login and tab pulses) and
  the canonical actor state's (movement pulses), run one after the other in
  the pulse phase position.
- `RegionManager.move` from the walking queue runs right after the walking
  call that asked for it returns (it needs the whole world: zones, the
  repository), still inside the same entity's tick; at login it runs in
  place. `Repository.removeRenderableNPC` from `onRegionInactivity` runs
  likewise right after that NPC's tick call.
- Regions are never unloaded: the region activity pulse (`Region$1`, which
  would `flagInactive` an unviewed region after 50 ticks) is a named skip;
  an activated region stays active.
- The start-up's NPCs are copied into the scene's chunks by location (the
  start-up ran `RegionManager.move` against its own chunk store).
- The appearance block is the prepared publisher's (`appearance::Data`): its
  twelve body slots are copied from the canonical appearance after
  `prepareBodyData` (worn equipment included); the other fields stay the
  prepared data's (the original's full flag writer, `player::appearance_writer`,
  is not joined to the prepared masks).
- `DropListener`'s soft queued script is kept by the live world, not in the
  player's canonical script queue: it runs at the start of the player's
  `Player.tick` once its tick (the next one) is due.
- An item's `InteractPlugin` table uses the item handle's identity as its
  node id (no canonical node is made for an inventory item).
- `ZoneMonitor.interact` for an item option is answered false only when the
  player is in no zone; inside a zone it is refused (the zones' `Target` is a
  canonical node, which an inventory item is not).
- A player in the disconnection queue keeps its slot until it leaves the
  queue (the queue's next update reads it).
- The live world's own CombatTab, Emote and Music button bodies are replaced
  by the global handlers' (their services answer through the same tab
  libraries).
- NPC update masks are not joined: `masks_required` is false for every NPC,
  so no NPC mask block is written (force talk, animations, facing).
- The canonical state's collection (the native stand-in for the JVM's
  garbage collector) runs every 100 ticks: it traces every retained node,
  the joined NPCs included (about 0.8 s in a debug build).
- Save text is built on the worker thread (the original builds it on the
  executor thread); the file write is off the tick thread.
- `Auth.configure` runs a second time with the live repository's name map as
  the online lookup (the start-up's registry looks at the start-up's own
  repository, which holds only NPCs).
- `interfaces::close` asked by the walk request (and `close_chatbox`) runs
  right after the walk request returns, answered true meanwhile.
- The quest registry is empty (quest classes are content): `syncronizeTab`
  sends the quest points varp only, and a returning character's quest points
  become 0 through `syncPoints`, as the original does with no quests
  registered.
- `timers.processTimers()` runs right after the walking call of
  `Entity.tick` returns (it is that call's last step), before the masks are
  prepared; the timers' messages and skull calls of a timer registered by a
  save's parse are made right after the parse returns.
- The global handlers' scenery OptionHandlers (LadderManagingPlugin,
  look-at, search) run right after the walking call that reached them
  returns (they need the whole world); a handler answering false sends
  `Nothing interesting happens.` then. `getDestination` runs inside the
  movement over the scene only.
- `PulseManager.runUnhandledAction` of an unhandled use-with is a world pulse
  (the canonical pulse manager does not take content bodies).
- NPC timers are not installed: an NPC's walking-queue timer query answers
  none and its `processTimers` is a named skip.
- The daily restart is not run: `daily_restart = false` in the pinned
  configuration and its ServerStore daily/weekly clears are store archive
  writes that are not ported. `Server.heartbeat` (watchdog off) and the
  network reachability check are not run.

## Answers by name (`world::live::not_ported`)

Skipped (the original runs content or a system without a port; nothing is
sent or changed; the first call of each is logged with `--verbose`):
`addAutoTimers`' GrandExchangeTimer and HolidayRandoms,
`AggressiveHandler.selectTarget` (no target) and its tolerance calls,
`Appearance.setAnimations`, the Attack and Follow player option handlers
(registered by the global handlers; their binding to the player option
table is not ported), `ButtonClickEvent` dispatch, `checkForWealthUpdate`,
`CombatPulse.updateStyle`, `CombatPulse.stop` and `ImpactHandler.setDisabledTicks`
for NPCs, `CombatPulse.stop` before an interaction, `Commands.define`,
`ComponentPlugin.open`, `Discord.postPlayerAlert`, entity tick event hooks,
`Entity.hook(UsedWith)`/`unhook` of DarkZone, `Entity.scripts.removeWeakScripts`
in `InteractionListeners.run`, the equip listeners' `removeWeakScripts` and
event, `InteractionEvent`, `PickUpEvent`, `UseWithEvent`, `ItemEquipEvent` and
`ItemUnequipEvent` dispatch, `FamiliarManager.login`, `FriendsListInterface.open`,
`GameWorld.loginListeners`, `GameWorld.logoutListeners`, `GameWorld.tickListeners`,
`HouseManager.leave`, `InterfaceListeners.runOpen`, `PlayerMonitor` drop-trade
log and DUPE_ALERT (logged only),
LoginConfiguration's per-item equipment hooks, `LoginConfiguration.LOGIN_PLUGINS`
(the global handlers register LoginValidationPlugin; its services, the
activities, loot share, quest point items and debug toggle, are not bound,
and it is not run), `Managers.tick`, NPC attributes (return-to-spawn,
lastForceTalk), `NPC.sendChat`, NPC update masks (reset and blocks), NPC
timers' `processTimers`, `Player.tick`'s potion timers, prayer drain, wealth
and off-map check, `PlayerMonitor.log`/`logChat`/`logPrivateChat`,
`Properties.updateDefenceAnimation`, the region activity pulse (no unloading),
`RulesAndInfo.openFor`, ServerStore `daily-accounts` `addToList`,
`WalkingQueue.reset` at logout; in saves:
`AttributeSetEvent`, `FamiliarManager.parse`, `HouseManager.parse`,
persist-player content hooks.

The original's answer with no content registered: `AssistSession` (none),
`CombatPulse.isAttacking`/`isInCombat` (false: the combat pulse is not
installed), `DeathTask.isDead` (false: nothing starts a death),
`DialogueInterpreter.close` (true), `DialogueInterpreter.getDialogue` (null),
`Entity.isTeleporting` (false: no teleport pulse runs),
`FamiliarManager.hasFamiliar` (false), `InterfaceListeners.runClose` (true),
the Marathon perk (false), `Player.logoutListeners` (empty),
`QuestRequirements.hasRequirement` (false), ServerStore `daily-accounts`
(empty: the account limit counts nothing across logins) and `flagged-ips`
(empty), skillcape perks (false, `skillcape_perks` off), NPC timer queries
(none: no NPC timer is registered here),
`ZoneMonitor.move` (no installed region zone overrides move: true),
`ZoneMonitor.interact` for ground items (false), `ZoneMonitor.useWith` with no
zone (false), `ItemDefinition.getItemPlugin`
(null: no item plugin is registered), `PluginInteractionManager` for ground
items, NPCs, scenery and use-with (false), `InteractionListeners` for ground
items (false) and for a scenery option nothing registers (false),
`UseWithHandler.HANDLERS` (null: no content use-with handler),
`NPCDefinition.getOptionHandler` (null: NPC option handlers are content),
`DegradableEquipment.getDropReplacement` (the item's own id: no degradable
equipment registered), `GraveController.hasGraveAt` (false: no graves),
`SlayerEquipmentFlags.isSlayerEq` (false), `DialogueInterpreter.close` after
an equip (true).

Refused (an error the caller logs; never a success): routed packets whose
library is ported but not installed here (component actions (spells),
`ContinueOption`, `InputPromptResponse`) and those the driver refuses
(commands, quick chat, report abuse, GE); content item and NPC option
handlers and listeners (`InteractionListeners` functions other than
DropListener, EquipHandler and the global handlers', their movement and
scripts), item plugins (`ItemPlugin.remove`, `canPickUp`), the guild items'
dialogue, `EquipHandler.unequip`'s caller other than the global equipment
tab, equipment plugins' `fireEvent`, brawling gloves, autocast clearing on a
weapon change, `SlayerEquipmentFlags.updateFlags`, the destroy dialogue
(`sendDestroyItem`, `addDialogueAction`), `FamiliarManager.summon` of a pet
(`Pets.forId` is native: the enum's item ids), `NPC.getShownNPC` of a
morphing NPC, `Lock.getMessage`, the scenery examine varbit lookup,
`NPCDefinition.forId` of an id no spawned NPC uses, `ZoneMonitor.interact`
and `ZoneMonitor.useWith` for an item inside a zone; the region zones'
services without a port: the DarkZone darkness overlay, light sources,
insects and tinderbox, the multiway overlay packets, Karamja's inventory
removal, teleports from zones, `restoreTabs`, NPC walk/stacking, familiars
(hasPet, getFamiliar, transform), `NPC.reTransform`, `CombatPulse.attack`,
wilderness loot and news, the player Attack option (wild PvP), the
deep-wilderness listener and risk value; the timers' services without a
port: `ImpactHandler.manualHit` (poison and disease hits: combat is not
installed), `NPC.isPoisonImmune`, NPC names, skills and settings in timers,
`Entity.scripts.removeWeakScripts` from a timer, NPC slayer tasks,
familiars, SpellbookSwap's logout listener, weapon interface, autocast,
spell book tab and varbit (Lunar spells are not ported), the random event
content of AntiMacro's run (`RandomEvents`), and while a save is written or
parsed every service but uids, ticks, the generator and the log; the global
handlers' services without a binding (each `GHost` method named "global
handler service NAME": dialogues, banks and deposit boxes, quests and
diaries, clans, assist, familiars, activities, prayer, spells and special
attacks, interfaces the tabs open other than through the tab libraries,
movement pulses (Follow), requests, the settings toggles other than
retaliate and the special bar, item containers' other calls); scenery:
the hardcoded constructed scenery of `processSceneryAction`, varbit- and
varp-configured scenery (morphs) and their debug lookup, the bank booth,
chest and deposit box InteractionListeners (the bank is not installed),
scenery option handlers that are not global handlers (doors and other
content); use-with on scenery, the use-with MovementPulse (NPCs, players,
ground items) and item-on-item Pulse with handlers, `UseWithHandler.handle`;
the slot switch's bank calls; content NPC clipping suppliers;
special attack handlers (`instant_spec`), `DialogueInterpreter.sendDialogue`
(a locked emote), the SALUTE, GOBLIN_BOW and GIVE_THANKS emotes,
`AchievementDiary.updateTask`, level-up rewards (no experience is gained),
tutorial content, reconnect (opcode 18), `sendLastLoginInfo` (the lobby), a
saved spawn location or `login_type` attribute, the loot-share pulse, the
management-server session, account creation from social, familiars, region
NPC queries (`Repository.findNPC(location)`), the deferred login world
configuration of CloseIface, an NPC's dialogue player, the zone services of
combat and teleports, the scene's scenery services (doors, audio), content
script invocation.

## Verification

```powershell
cargo test -p rs09-server --test live_world
```

`crates/server/tests/live_world.rs` (needs `assets/cache` and the template;
prints SKIPPED without them) starts the world in-process as `--world` does,
behind `runtime::Service`, and drives headless TCP clients built from the
native protocol code (`rs09_net::login::LoginPeer`, the game frame decoder,
the client-side player and NPC update decoders):

- `traceone` (new) logs in: reply 2, index 1, the rebuild (162) and player
  updates. Its login tick is compared with the world-trace new-character
  login tick: of the original's 394 entries after the reply, 110 come from
  calls answered as not ported (pinned by marker and count:
  CropCircleController 1, FriendsListInterface 2, FamiliarManager 3,
  QuestRepository 103 (its first entry, the points varp, is native),
  LoginValidationPlugin 1); the 168 entries before `reinitVarps` are equal in
  order, 122 byte for byte and 45 counter-carrying interface packets by opcode
  and length (the skipped calls' packets advance the original's interface
  packet counter), and one, MusicPlayer.play's track name, differs only by the
  " (62)" the original's debug mode adds (LoginValidationPlugin's
  `toggleDebug`, content); `reinitVarps`' 42 native varps are all among the
  original's 116; nothing else follows. The weapon tab (24 entries), the
  flashing icons, the emote refresh, the music player and the region move
  with its music zone are now in the compared entries.
- A walk three tiles east: three steps in the player updates, the client's
  position reaches the target.
- NPC updates (32) arrive and add NPCs within 15 tiles of the player.
- The weapon tab's button 3 (interface 92) sends the attack style varp 43 = 1;
  the music tab's loop button (187, 11) sends varp 19 = 1; the emote tab's
  YES (464, 2) shows animation 855 in the player update.
- `tracetwo` logs in (index 2): each client sees the other added, with the
  appearance block.
- `tracetwo` closes its connection: `traceone` sees it removed; `traceone`
  closes.
- Both saves are in `<saves>/players`, parse with
  `player::save::parser::parse_text`, and `traceone`'s location is where it
  walked; `traceone` logs in again from its save (the native parser) at
  that location.
- `traceone` uses the first option of its nearest NPC (`processNpcAction`,
  the NPC's InteractPlugin, the legacy walk and arrival) and receives
  "Nothing interesting happens." (no content handler).
- `groundone` (a save made from `traceone`'s, placed on the start-up's bowl
  spawn 1923 at 3208,3214 with a bronze dagger 1205 in slot 0) sees the
  spawn in its chunk synchronisation, takes it (opcode 66: the item enters
  the inventory, the spawn leaves the ground, sound 2582), drops it (opcode
  135: DropListener, the item leaves the inventory and lies on the ground,
  sound 2739; `groundtwo`, logged in next to it, does not see the private
  item), takes it back after the `droppedItem` deadline (the drop/pickup round
  trip over TCP), wields the dagger (opcode 55: equipment slot 3, inventory
  slot 0 empty) and `groundtwo` sees the weapon slot in `groundone`'s
  appearance block; item and NPC examines answer with their texts.
- `wildone` (a save in the wilderness north of Edgeville's ditch) enters
  WildernessZone at login: overlay 381 opens and its level line reads
  "Level: 1" (SkullManager.setLevel).
- `groundone` moves the bowl to inventory slot 5 (opcode 231,
  `processSlotSwitch`) and uses the dagger on it (opcode 27,
  `processUseWith`: no listener, plugin or UseWithHandler takes it):
  "Nothing interesting happens." a tick later (`runUnhandledAction`).
- `climbone` (a save south of the ladder 36768 at 3229,3213 east of
  Lumbridge castle) uses Climb-up (opcode 254): `processSceneryAction`, the
  legacy walk and arrival, the global handlers' LadderManagingPlugin and
  `ClimbActionHandler` (lock, the one-tick pulse, the teleport location);
  the client's plane becomes 1.
- `healone` (a save with 5 of its 10 hitpoints) regains one hitpoint about
  100 ticks after login (SkillRestore from `addAutoTimers`; the client's
  hitpoints skill packet goes from 5 to 6; measured 61.3 to 61.9 s); it runs
  beside the other scenarios and sends PING (93) every five seconds.
- At least ten ticks, a mean interval between 590 and 700 ms.

`running_server_scenario` (ignored) drives the same client against a
running `rs09-server --world` (`RS09_LIVE_ADDRESS`, `RS09_LIVE_SAVES`).

Measured (2026-11-17, this machine, with the timers, the global handlers,
scenery, use-with and the slot switch installed): in-process debug test 132
ticks, mean work 42.5 ms, maximum 1,533 ms (the first login: lazy definition
loading in a debug build), mean interval 608.3 ms, 12 saves written (87.7 s:
the hitpoint restoration waits about a minute). Release binary
(`rs09-server --world`, start-up 907 ms, 13,709 NPCs joined), the ignored
scenario against it (79.0 s, every step above), then idle: 299 ticks, mean
work 5.45 ms, maximum 187.8 ms, mean interval 600.3 ms.
The world-login, tick-driver and world-trace comparisons of the libraries
are unchanged (their tests bind the same libraries with replayed
stand-ins); this test checks the production binding end to end.

## Not covered / still to do

- The remaining routed handlers: component use-with (spells),
  `input_prompt`, `continue_option` and close with a dialogue interpreter
  per player (the global handlers' ClimbDialogue, `send_options` and every
  dialogue service are refused), `ZoneMonitor.interact`/`useWith` for items
  inside a zone, hint icons and warning messages, the DarkZone's overlay and
  light sources and the multiway overlay.
- Combat (CombatPulse and swings, NPC aggression, impact and hitsplats in
  the update masks, prayer and its tab, special attacks, NPC death with drop
  tables, player death), NPC update masks, NPC timers (poison or freezing an
  NPC), teleports (`teleport::Manager`, the spell book teleports, the
  wilderness rule), the global handlers' login plugin (LoginValidationPlugin)
  and the Attack and Follow player options.
- The per-player or per-world libraries behind other buttons and options:
  the bank, trade and requests, shops (`shops::World`, its tick listener),
  gathering listeners, quests and diaries, clans; content listeners and
  doors.
- The full appearance flag writer (`appearance_writer`) instead of the
  prepared data; region unloading, NPC aggression's target selection and
  NPC update masks; the climb animation (828) was not seen in the player
  update by the test (the climb itself is).
- An NPC tick that fails stops the rest of that tick's update sequence (the
  original's `handleTickActions` catch): a refusal reached by every tick (as
  the water paths were before they were bound) stalls the world.
- Reconnect, the lobby, AFK logout's client side, many players (capacity is
  not measured), concurrent saves of one character.