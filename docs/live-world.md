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
(`processSlotSwitch`), and combat (`world::combat_pulse` and
`world::combat_swing` for every entity, `ImpactHandler` with hit marks in the
player and NPC update masks, the NPC update masks themselves, NPC aggression
through `world::npc_behaviour`, NPC death and drops through `world::npc_death`
and `world::drop_tables`, prayer through `world::prayer` with its tab, and the
engine timers' poison and disease hits; `combat.rs` and `combat/`), a
player's death (`world::player_death`: `Player.commenceDeath`/`finalizeDeath`,
kept items, the death screen; `combat/player_death.rs`), projectiles
(`Projectile.send` as the chunk's `ProjectileUpdateFlag`, so ranged swings
complete; `combat/projectiles.rs`), `useAmmo`'s `equipment.replace`, poison
applied by a swing (the Poison timer of `timers::content` on a player or an
NPC), NPC timers (`timers.rs`: each NPC's own timer manager, processed at the
end of its `Entity.tick`, with `SkillRestore` healing and restoring NPCs), the
weapon special attacks (`world::special_attacks`: the 34 handlers registered
at boot, `Player.getSwingHandler`, the overriding members, the combat tab's
special bar and instant specials; `combat/specials.rs`), the lobby
(`LoginConfiguration.sendLobbyScreen` with `LastLoginInfo`, the welcome
screen's play button and `configureGameWorld`), a dialogue interpreter per
player (`world::dialogue` with the DSLs of `world::dialogue_dsl`, the
Continue packet, the banker conversation), the NPC talk and banker
listeners with the content listeners' walk to an NPC, input prompts
(`world::input_prompt`), the bank at a banker (`inventory::bank` with
`world::bank_interface`), shops (`world::shops`), requests and trading
(`world::trade`), the Attack and Follow player options, eating and drinking
(`world::consumables`), NPCs made at runtime
(`new NPC`, `NPC.init`, `NPC.transform`, `NPC.clear`; `runtime_npcs.rs`),
graves (`world::player_death`'s `Grave`/`GraveController` over a grave NPC
made at runtime, `GraveController.tick` as a tick listener, `Grave.tick`
with the owner's hint icon through `HintIconManager`, `hasGraveAt`;
`graves.rs`, `content.rs`), the NPC class overrides (`world::npc_classes`:
`init`, `tick`, `finalizeDeath`, `onAttack`, `onImpact`, `getSwingHandler`
with MonkNPC's `CombatAction`, their quest and diary reads from the
persisted state; `subclasses.rs`), the dragons (`world::dragonfire`: each
chromatic and metal dragon's MultiSwingHandler with the class's
DragonfireSwingHandler, the protection checks; `dragons.rs`) and the login
plugin `LoginValidationPlugin` (the global handlers'). A client
logs in, enters the world, walks, sees NPCs, players and ground items, uses
its weapon, emote and music tabs, takes, drops, moves, uses and wields items,
examines, uses an NPC's option, climbs a ladder, enters the wilderness,
regains hitpoints over time, toggles a prayer, attacks an NPC and kills it
(hit marks, the death, the drop on the ground), is attacked by an aggressive
NPC, dies and respawns, dies as a regular player and leaves a grave with its
lost items, kills a rat that is a `RatNPC` (its Witch's Potion rat tail),
is breathed on by a green dragon, shoots an arrow (projectile, ammunition),
uses a weapon's special attack, passes the lobby of a returning character,
talks to a banker and continues the conversation, opens the bank, deposits
and withdraws, buys from and sells to a shop (also by amount), trades with
another player, follows another player, eats food and drinks a potion,
casts Wind Strike on an NPC, teleports to Varrock from the magic tab,
logs out, and its character is saved and loaded again.

This is a playable engine slice, **not a complete server**: content (login
listeners, quests, tutorial, content item, NPC and scenery handlers other
than the global handlers, content dialogues other than the banker's, the
bank booths, chests and deposit boxes, the bank PIN, assist sessions), NPC
timers, the spells and teleports listed below as refused, ForceMovement,
commands, quick chat, abuse reports, the Grand Exchange and every handler
listed below as refused are not installed, and every such call is answered
by name below.

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
| appearance | `Appearance.sync`, `Appearance.setAnimations`, `PlayerFlags530.Appearance` | `Inner::appearance_sync` registers the mask (`player.updateAppearance()`) at login and from the EquipmentListener; `live/appearance.rs` runs `player::appearance_writer` when the update phase writes the mask, over the live receivers (the canonical Appearance, equipment, render info and attributes, the profile's Properties and Skills, the SkullManager, the node's size, the configuration's `pvp` and `wild_pvp_enabled`); `prepareBodyData` runs inside the writer; `setAnimations` from the EquipmentListener sets the weapon's animations and render animation |
| engine timers | `TimerRegistry` (the class scan's timers), `addAutoTimers`, `TimerManager.processTimers`, `saveTimers`/`parseTimers`, `Entity.hook`/`unhook`/`dispatch`, `hasTimerActive` | `timers.rs`: the fourteen `timers::content` prototypes in the world's registry (registered at the first timer operation, in scan order); `addAutoTimers` at `Entity.init` (AntiMacro, SkillRestore); `processTimers` for every player at the end of `Entity.tick`, the timer manager of the canonical profile (its three lists aliased); `timers::content::Natives` over `timers::Host` (the live world: native Skills through the live skills and settings contexts, the shared generator, messages, audio, equipment, rights, uids, the zone monitor's random-event restriction, the SkullManager, the per-entity hook lists) and over `timers::Saving` while a save is written or parsed (uids, ticks, the generator, the log; the messages and skull calls of a timer the parse registers are made right after the parse); `hasTimerActive` answers the walking queue (`frozen`), the zones (`teleblock`), the dragons (`dragonfire:immunity`), combat (`miasmic`) and `isPoisonImmune`; the SkullManager's `removeTimer`, `spawnTimer` and `registerTimer` go through the registry. NPCs: `processTimers` at the end of an NPC's `Entity.tick` over its own manager (the start-up's), `SkillRestore` joined at its first timer operation, `GrandExchangeTimer`'s NPC branch, NPC skills from the NPC row, NPC names from the definition, `NPC.task` from `world::slayer_tasks`, `applyPoison`, the specials' timers, `onEntityDeath`, `fullRestore`'s removals and the NPC behaviours' `removeTimer`; `Entity.scripts.removeWeakScripts` from a timer is the canonical script processor's |
| global handlers | `ClassScanner` of the global handler classes, `InterfaceListeners`, `ComponentDefinition` plugins, OptionHandlers, `ClimbActionHandler` | `global.rs`: `registry::register_scanned` over the live InterfaceListeners, the interface definitions' plugins, the native InteractionListeners and the scenery option defaults at boot; the per-definition scenery handlers bound into the scenery definitions; buttons, open and close listeners and component plugins dispatched through `run_button`, `run_open` and `run_plugin` (`buttons.rs`); scenery OptionHandlers through `run_handler` and `handler_destination` (`destination.rs`, over the scene); the bodies' `Task`s submitted to `GameWorld.Pulser` and run with `run_task`; `GHost` is the handlers' `Host` (attributes, messages, interface text, the weapon, emote and music tabs, settings toggles, locks, the animator, the teleport location, scenery geometry, SpecialLadders) |
| scenery options | `processSceneryAction`, scenery `InteractPlugin` | `scenery.rs`: `world::scenery_runtime` (the legacy walk and arrival, `world::scenery_action`) over the cache scene's objects, `scenery_options::Registry` (the scenery tables and handler arena with the global handlers' defaults), `Scenery.getChild` over the scene's definitions, the InteractionListeners lookup for scenery |
| use-with, slot switch | `processUseWith`, `processSlotSwitch` | `items/use_with.rs`, `items/slots.rs`: `world::packet_handlers::use_with` (zone monitor, `InteractionListeners.run_use`, PluginInteractionManager, `UseWithHandler.HANDLERS`, `runUnhandledAction`) and `slot_switch` over the canonical inventory |
| combat | `CombatPulse`, the melee/range/magic `CombatSwingHandler`s, `ImpactHandler`, `Entity.attack`/`onImpact`/`isAttackable`/`checkImpact`/`sendImpact`, `AttackListener` | `combat.rs`, `combat/host.rs`, `combat/behaviour.rs`: the live world is `combat_swing::Host`, `combat_pulse::Host` and `npc_behaviour::Host`; one combat pulse and impact handler per entity that fought (`Inner.combat`); the combat pulse is a `Class::Combat` pulse object current in the entity's canonical pulse manager (STANDARD) and submitted to `GameWorld.Pulser`, whose body is `combat_pulse::pulse::update`; impact, second-arrow and delayed-hit pulses on the same runner, dispatched by identity; NPC option "attack" through the native `InteractionListeners` (`AttackListener`, flagInstant, ported from the pinned AttackListener.kt); auto-retaliation, the NPC's swing handler and attackability through `npc_behaviour::combat`; experience through the native Skills (`LevelUp.levelup` on a level gained) |
| hit marks, NPC masks | `UpdateMasks.prepare`'s impact poll, `NPCFlags530`, `PlayerFlags530.PrimaryHit/SecondaryHit`, NPC `Animator` | `combat/masks.rs`: an `update_masks::Masks` per NPC with the NPCFlags530 providers and encoders of `world::npc_masks` (hits, animation, face entity, spot animation, type swap, force chat, animation sequence, face location; npc-masks-oracle, docs/npc-masks.md; a context of another type is logged and writes nothing), prepared at the end of the NPC's `Entity.tick` and written by `NPCRenderer`; the NPC's animator over them; a player's impacts handed to its prepared masks before `prepare` (`prepared_masks::queue_impact`, the hit encoders added there) |
| aggression | `AggressiveHandler.selectTarget` from `NPC.handleTickActions` | `combat/behaviour.rs`: the NPC's live `Ai` (made from the start-up's: DEFAULT, or WILDERNESS for the NPCs `ZoneBuilder.startup` gave it, with the start-up's radius, chance, tolerance and pause), `Region.isTolerated` over the scene regions' tolerances |
| NPC death, drops | `DeathTask`, `NPC.finalizeDeath`, `NPCDropTables`, the seven shared drop tables | `combat/death.rs`: `npc_death` and `drop` with the live world as host and the ground manager (`ground_items::World`) for `createDrop`; the death task a STRONG pulse of the entity's pulse manager on `GameWorld.Pulser`; the drop tables of the start-up's `DropTableParser`; the shared tables loaded at boot from `<config>/shared_tables` |
| prayer | `Prayer`, `PrayerTabInterface`, `Player.tick`'s `prayer.tick()` | `combat/prayer.rs`: `world::prayer` per player (`PrayerState` in `Inner.combat`), the global handlers' prayer tab services, the drain each `Player.tick`, the combat reads (bonuses, protection, smite, redemption) |
| region zones | `ZoneBuilder.startup` (a StartupListener), `SkullManager` | `region_zones.rs`: `wilderness_zone::startup` at boot (after the NPCs join, as the start-up runs startup listeners after NPCSpawner) registers WildernessZone, MultiwayCombatZone, ModeratorZone, DarkZone, KaramjaZone and BankZone in the scene's regions, where the zone monitors find them (entities already in a region enter, aggressive wilderness NPCs get the WILDERNESS AggressiveHandler); `Live` is `zone_impls::Host`, `dark_zone::Host` and `wilderness_zone::Host`; each player's `SkullManager` (`world::player_managers::skull`) answers the skull services; overlays through the interface manager |
| a player's death | `Player.commenceDeath`/`finalizeDeath`, `DeathTask.getContainers`, `DeathInterface` | `combat/player_death.rs`: `world::player_death` with the live world as `KeptHost`/`Host`, reached from the death task's `player_commence_death`/`player_finalize_death`; the canonical inventory and equipment with their listeners (the container context of the dying player), the ground manager for bones and lost items, the zone monitor's `handleDeath`, type and grave restriction, the SkullManager, the prayer library (Retribution, reset), native Settings (special and run energy), the interface manager (default tabs, the death screen 153, `InterfaceConfig`), `timers.onEntityDeath` (`TimerManager.death`); the death screen's button through the global handlers (`DeathInterface`); a dangerous death that allows a grave reaches the graves below |
| runtime NPCs | `new NPC(id, location)`, `NPC.create`, `NPC.init`, `NPC.transform`, `NPC.clear` | `runtime_npcs.rs`: construction, `Entity.init` (`addAutoTimers`), `initConfig`, `configure`, `setDefaultBehavior`, the child NPCs and `onCreation` through the start-up's `npc_init::Context` (`startup::npcs::Init`) over the live actors and NPC store, the start-up's NPC definition table (archive 18 and the `NPCConfigParser` configurations) and timer registry; `Repository.npcs.add` (the live repository and the start-up's list, same slots), `RegionManager.move` (`entity_regions` with the zone update), `Region.isActive` and `addRenderableNPC` are the live world's. `transform` sets the id, definition, size, `initConfig`, `configure` and the TypeSwap mask (synced unless back to the original id) and the renderer's view; `clear` is `Entity.clear` (inactive, out of its chunk, pulses cleared), `removeRenderableNPC`, `Repository.npcs.remove` and `onRemoval` |
| graves | `Grave`, `GraveController` (`tick`, the four options, `hasGraveAt`), `GraveType`, ContentAPI `registerHintIcon`/`clearHintIcon`, `HintIconManager` | `graves.rs`, `content.rs`: `world::player_death::grave` over a grave NPC made by `new Grave()` (`AbstractNPC(GRAVESTONE_6571, (0,0,0), false)`), kept by its NPC's node; `GraveController.tick` in `GameWorld.tickListeners` (scan order: GraveController before Shops); `Grave.tick` replaces the grave NPC's `NPC.tick` (the hint icon, no `Entity.tick`); the hint icon through `world::player_managers::hint_icons` per player; the four options registered in the native InteractionListeners at boot and dispatched by identity; `hasGraveAt` for DropListener |
| NPC class overrides | the AbstractNPC subclasses' members (ShopNPC, CowNPC, DuckNPC, RatNPC, GuardNPC, MonkNPC, HighwayManNPC, IceGiantNPC) | `subclasses.rs`: `world::npc_classes::classes` with the live world as `npc_classes::Host`: `tick` from the driver's NPC tick, `finalizeDeath` from the death task's stop (`npc_death::task_stop_with`), `onAttack` and `onImpact` from combat, `getSwingHandler` (MonkNPC's static `CombatAction` is a handler slot whose `swing` override is `monk_swing`), `init` of an NPC made at runtime. `getQuest`/`isStarted` read the persisted quest stage of the quests the original registers (Witch's Potion: index 31; 0 when the save has none); the Falador diary is the persisted diary through `world::player_managers::diary` (`isComplete`, `updateTask` with its messages and tab lines) |
| dragons | `ChromaticDragonNPC`, `MetalDragonNPC` (`getSwingHandler`, `getDragonfireProtection`), `DragonfireSwingHandler`, `MultiSwingHandler`, `calculateDragonfireMaxHit` | `dragons.rs`: the live world is `dragonfire::Host`; each class's static `DRAGONFIRE` and each dragon's `combatAction` are handler slots made the first time they are asked for (no draws), whose subclass answers `combat_pulse::Host::overrides` with `dragonfire::overrides`; projectile copies are sent as `Projectile.send`; NPC protection tokens are the dragon classes' (every token) or 0 |
| login plugin | `LoginConfiguration.LOGIN_PLUGINS`: `LoginValidationPlugin` | the global handlers' registration at boot; `login_plugin` runs `registry::run_login` with `GHost` (`unlock`, `SystemConfig.validLogin` (true), `toggleDebug` in dev mode, the fight caves/falconry attributes, Dragon Slayer's saved "repaired", the saved loot-share delay and points, `hasCompletedAll` over the live quest registry) |
| projectiles | `Projectile` (create, ranged, magic, copy), `Projectile.send`, `ProjectileUpdateFlag` | `combat/projectiles.rs`: the projectile fields (source centre, victim, end location, heights, delay, speed, angle, distance) kept per foreign chunk flag (`chunk_sync::Flag::Foreign`) on the source location's chunk; written by `RegionChunk.update` as opcode 16 inside the chunk update (230) with the target index and end location read at write time; dropped by `ChunkUpdateTracker.resetAll`. Ranged swings (`AmmunitionCopy`, the dark bow's `create`, NPC `ranged`), Retribution (location-based) and the specials send through it |
| ammunition, poison | `RangeSwingHandler.useAmmo`, `applyPoison` from a swing | `equipment.replace(item, slot)` over the canonical equipment (`Container.replace` with its listeners); `applyPoison` is `timers::content::apply_poison` over the victim's timer manager (a player; an NPC victim refuses: NPC timers) |
| special attacks | the 34 special handler classes' `newInstance` (`CombatSwingHandler.register`), `Player.getSwingHandler`, the subclass members, `CombatTabInterface`'s special bar and `handleInstantSpec` | `combat/specials.rs`: one handler instance per `Special` in the swing handler table (`HandlerSlot.special`), `register_all` at boot over the melee and range singletons; `combat_pulse::Host::overrides` answers `special_attacks::overrides`, `swing_handler` answers `player_swing_handler`; `special_attacks::Host` over the live world (settings recorded and made after the call, spawned timers through the timer registry, the specials' pulses on `GameWorld.Pulser` dispatched by identity, their projectiles); the global handlers' `special_handler`, `special_instant`, `combat_victim` and `special_swing` |
| dialogue | `DialogueInterpreter` (one per player), `DialoguePlugin`/`DialogueFile`, DialogueLabeller/DialogueBuilder, the ContinueOption branch | `talk.rs`: `world::dialogue` and `world::dialogue_dsl` with the live world as `dialogue::Host`, `dialogue_dsl::Host` and `banker::BankHost` (interface packets through the player's packet counter, the chatbox through the native InterfaceManager, attributes, NPC id, `setDialoguePlayer`, the NPC's walking queue reset and pulse clear, definitions); an interpreter made the first time a player's is asked for; `interface_packets::continue_option` over it (`buttons.rs`); `DialogueInterpreter.close()` from IfAction, the walk request (right after the request returns) and the trade; `DialogueClosed` hooks run from the close event; `Component.setUnclosable`'s close event. The plugin registry holds no content plugin |
| NPC talk, bankers | `NPCTalkListener`, `BankerNPC` (listeners, destination override, `init`'s booth search, companion), `BankerDialogueLabellerFile` | `talk.rs`, `subclasses.rs`: the listeners of `npc_classes::listeners` registered in the native InteractionListeners at boot (BankerNPC, then NPCTalkListener, as scanned), dispatched by `run_interaction` with the NPC class host; `banker_conversation` is `dialogue_dsl::banker::talk_to`; `RandomEvents.randomIDs` and `HolidayRandomEvents.holidayRandomIDs` are the pinned lists; a start-up banker becomes a `BankerNPC` the first time it is interacted with (class, direction, `isWalks = false`, `facing_booth` from `findAdjacentBankBoothLocation` over the scene) |
| the walk to an NPC or player for a listener | `InteractionListeners.run`'s `MovementPulse(player, node, flag, destOverride)` and its arrival | `talk.rs`, `round.rs`: `player_walk::World::listener_motion` (the `Destination` constructor of `world::movement`) with an approach whose option handle identifies the arrival; the arrival (`interaction_listeners::arrival`: zone, face, the handler) runs with the whole world right after the walking call; a destination override is evaluated when the pulse is made |
| input prompts | `RunScript.processInput`, the InputPromptResponse branch, `sendInputDialogue`, `BankUtils.transferX` | `talk.rs`: `world::input_prompt` with `Prompt` as its context; the `runscript` attribute holds the transfer object or a mark for a callback the world keeps (a DSL's, a shop's or a trade's amount script) |
| bank | `BankContainer` (open, close, add, take, tabs, note/insert), the private BankListener, `BankInterface` (762/763/767), `BankCloseEvent` | `bank.rs`: `inventory::bank` on the canonical player's bank with `BankUi` (a `bank::Context` over the interface manager, variables, packets and the canonical inventory binding); `bank_interface::Program` registered in the live InterfaceListeners; its open listener from `InterfaceListeners.runOpen`, its buttons from IfAction, the captured `BankClose` from `InterfaceManager.close`; `openBankAccount` with `restrictForIronman` |
| shops | `Shops` (start-up, `tick`, the NPC listeners, the destination override, the 620/621 listeners), `Shop`, `ShopListener` | `shopping.rs`: `shops::World` built by `controller::startup` at boot from `shops.json`; `ShopHost` (`shops::Host` over the interface context's state, so the shop's open and close listeners run wherever the interface manager is used); `Shops.tick` after `GraveController.tick` in the tick listeners; Buy-X/Sell-X through the input prompts |
| requests, trade | `RequestManager`, `RequestType`, `TradeModule`, `TradeContainer`, `TradeCloseEvent`, `TradeInterfacePlugin` | `trading.rs`: `world::trade` with `TradeHost` over the whole world; the global handlers' RequestOption listeners ("trade with", "req assist") through the native InteractionListeners; TradeInterfacePlugin on 334/335/336 from IfAction; `trade::ui::Ui<TradeHost>` is the interface context of the Close packet and of a walk's interface close; the alert pulse on `GameWorld.Pulser`, dispatched by identity |
| player options | `Option._P_ATTACK`/`_P_FOLLOW.setHandler`, `AttackOptionPlugin`, `FollowOptionPlugin` | `follow.rs`: the handlers bound into the option constants (ids of a player handler arena the legacy runtime resolves for a player target); the legacy walk and arrival, the handler run with the whole world right after; FollowOptionPlugin's `MovementPulse(player, target, FOLLOW_ENTITY)` is a movement of the canonical state whose body (`follow_pulse`: face the target, never finish) runs right after the walking call that reached it |
| spells | `SpellBook.register` of the combat spells, `MagicSpell.castSpell`, `CombatSpell.cast`/`visualize`/`visualizeImpact`/`getMaximumImpact`/`getTargets`/`addExperience`, `meetsRequirements`, `SpellUtils.hasRunes`, `processComponentUseWith`, the magic swing's spell services | `magic.rs`, `items/component_use.rs`: the 53 combat spells of `world::spells::table` in a registry the world owns (`MagicWorld`); `SpellHost` is `world::spells::Host` over the canonical player (skills, inventory, equipment, attributes, the spell book, the persisted autocast spell), the NPC rows, the animators, the live projectiles (`Projectile.transform(...).send()`) and the combat pulse; the combat hosts' spell services answer through `spells::combat`; the spell-on-NPC/player/item/ground item packet (239 and the others of `ComponentUseWith`) runs `world::packet_handlers::component` with the item handlers' context; `WeaponInterface.selectAutoSpell`/`updateInterface` and the weapon tab's `SpellBook.getSpell` lookup answer from the same table. Only spells whose `fireEffect` is the empty base are cast: the strikes, bolts, blasts and waves, Crumble Undead, Iban Blast, Magic Dart and Sonic Wave; no ancient spell (see refused) |
| teleports | `TeleportManager`, `TeleportPulse` (`pulse`, `stop`), `Entity.isTeleporting`, `SpellListeners.run`, ModernListeners' Varrock, Lumbridge and Falador teleports (`requires`, `sendTeleport`, `removeRunes`, `addXP`, `setDelay`) | `magic.rs`: one `teleport::Manager` per entity; its pulses submitted to `GameWorld.Pulser` and dispatched by identity, their `stop()` override run through the driver's `pulse_stop` hook (`TeleportPulse.stop`: unlock, the `tele-pulse` attribute, the end animation); the destination written to the walking queue's teleport location; the zone monitor's teleport check and `isTeleBlocked`; the magic tab (192) reaches the listeners through the global handlers' `tabs::magic`. The three teleports are ported from the pinned `ModernListeners.kt`/`SpellListener.kt` without a differential corpus of the listeners themselves (the teleport and spell libraries are differentially tested) |
| eating, drinking | `ConsumableListener`, `Consumable.consume` of every class, the effects, the potion's dose script | `eating.rs`: `world::consumables` with `Eating` as its `Host` for one player (native Skills taken out of the profile for the call, Settings, the player's clocks, the canonical inventory, the animator, `consumables::join` over the engine timers, the combat pulse's `delayNextAttack`, `manualHit`); the dose message is a SOFT script kept by the world and run at the player's tick once due |
| lobby | `LoginConfiguration.sendLobbyScreen`, `LastLoginInfo`, `LoginInterface` (378), `configureGameWorld` | a returning character with `tutorial:complete`: the login library's lobby with `LastLoginInfo` (164: the IPv4 address, written at once); the welcome screen's play button (140) through the global handlers (`player.locks.lock("login", 2)`, `closeInterface`, `runTask(1, 0)`) and `configure_game_world` over the live world |

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
- The appearance block is written by the original's flag writer
  (`player::appearance_writer` through `live/appearance.rs`) when the mask is
  written, into a buffer of its own that is appended to the mask block (the
  original writes into the block's buffer; the bytes are the same). Its
  `FamiliarManager` receiver is answered without familiars:
  `isUsingSummoning` is the pouch flag and the summoning combat level is the
  profile's. A player with no SkullManager entry yet is read as a new one's
  fields. `transformNPC` is not joined: the services only it reaches
  (`NPCDefinition.forId`, `setSize`, the combat animations) are refused, so a
  transformed player's block is not written. The live join has no JVM
  differential fixture of its own; the writer's has (docs/appearance-writer.md),
  and the new-character login tick is compared with the world trace.
- A reconnect (login operation 18) runs `LoginParser.reconnect` with
  `Player.initReconnect` (`Live::init_reconnect`) in place of `Player.init`,
  as the original does: a new player object from the save, no
  `Entity.init` (so no `addAutoTimers`: AntiMacro and SkillRestore are not
  registered for a reconnected player, as in the original), no lobby and no
  `logged-in-fully` attribute. While the old player is still in the
  repository the reply is AlreadyOnline (`Login.kt`). No world-trace scenario
  covers it; the live test covers one dropped connection.
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
- An NPC's masks are prepared after its tick kernel returns when the tick
  reached `Entity.tick` (its region active and its respawn tick passed).
- `AggressiveHandler.selectTarget` needs the whole world: it runs right after
  the NPC's tick kernel returns (after that tick's walking step), and the
  kernel's query is answered false, so an NPC that just selected a target
  still reaches its force talk that tick. `ImpactHandler.setDisabledTicks`
  and `CombatPulse.stop` from an NPC's tick, and `CombatPulse.stop` from a
  pulse manager (a walk clears the pulses), run right after the walking call
  that asked for them returns.
- The combat pulse's private `MovementPulse.updatePath()` is a path to
  `DestinationFlag.ENTITY`'s tile beside the victim, found with the mover's
  pathfinder and walked, unless the last path went toward the victim's same
  tile and steps remain; the movement pulse's interrupt, truncation and face
  steps are not run.
- `CombatPulse.updateStyle()` from the weapon tab is applied at the next read
  of that player's combat pulse (only combat reads the style).
- A player's hit mark carries the health ratio read when the impact is
  handed to the masks (the end of its `Player.tick`), not when the mask is
  written in the update phase; nothing changes the player's lifepoints in
  between. An NPC's hit mark reads it when written, as the original does.
- `npc_death`'s view of the damage logs is made from the combat pulse's
  `ImpactHandler` at the start of each death call and written back (disabled
  ticks, cleared logs) when it returns.
- `Object.hashCode()` of an entity (the NPC damage log's order) is its node id.
- `playHurtAudio`'s Kotlin `Random.Default` draw comes from a generator of
  the live world's own (not one of the modelled Java generators).
- The seven shared drop tables are loaded at boot after the start-up (the
  original's StartupListeners run inside it): the rare table's positions are
  drawn from the shared generator at that point.
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
- An NPC's `timers.processTimers()` runs right after the walking call of its
  `Entity.tick` returns (when the call reached it), before its masks are
  prepared, as a player's does. `Entity.fullRestore()`'s removal of the
  `poison` and `poison:immunity` timers of an NPC runs right after the same
  walking call returns.
- NPC timers run on the start-up's per-NPC timer manager. The start-up's
  `Entity.init` registered three of the four automatic timers (no port of
  `SkillRestore`'s hooks there); `SkillRestore` is registered on an NPC at
  its first timer operation in the live world (before that operation, so its
  place in the lists and its first run, one interval after the NPC's first
  `processTimers`, are the original's). The start-up's `GrandExchangeTimer`
  on an NPC is answered by its NPC branch: no hooks, its constructor's
  interval as the initial delay, `run` false (removed at its first run).
- NPC skills in the timers are the NPC row's levels and lifepoints, copied
  for one timer operation and written back when it ends; a hit from a timer
  (`manualHit`) first writes back the skills a timer holds of the entity it
  hits (a player's too), so the hit and the timer's heals act on the same
  lifepoints. `Skills.lifepointsUpdate` of an NPC is not kept.
- An NPC's tick or reset that fails is isolated (`faults.rs`): the failure
  is recorded against that NPC and logged (the first failure, the 10th,
  100th, ... in a row, and whenever the error changes; one line when it
  recovers), and the update sequence goes on with the next entity. The
  original has no catch there: an exception from one NPC ends the rest of
  that tick's sequence (`handleTickActions`' catch), which in this world,
  where most failures are refusals of services without a port, stalled the
  whole world whenever one NPC's refusal recurred every tick. A player's
  tick still fails as the original's does.
- The settings calls of a special attack (`drainSpecial`, `toggleSpecialBar`,
  `setSpecialEnergy`: varps 300/301 and the energy message) are recorded
  while the special runs and made when the combat call returns
  (`sync_combat`): they keep their order among themselves but follow the
  swing's other packets of that call.
- ContentAPI `runTask` (the lobby's play button) submits its pulse to
  `GameWorld.Pulser` directly, not through the entity's pulse manager, so a
  walk does not stop it; a body run more than once refuses.
- `LastLoginInfo` parses `details.ipAddress` as an IPv4 literal (the
  original's `Inet4Address.getByName` would resolve a host name; a host name
  refuses).
- The container listeners of a dying player's inventory and equipment answer
  through that player's context (`CombatWorld.death_subject`) for the length
  of `finalizeDeath`.
- `SlayerEquipmentFlags.updateFlags` after a death computes nothing: the
  SlayerManager that keeps the flags is not ported (named skip).- The daily restart is not run: `daily_restart = false` in the pinned
  configuration and its ServerStore daily/weekly clears are store archive
  writes that are not ported. `Server.heartbeat` (watchdog off) and the
  network reachability check are not run.
- A runtime NPC's `Repository.npcs.add` adds it to the live repository and
  to the start-up's NPC list (which the start-up's code still reads); the
  two keep the same slots, and a mismatch refuses.
- The `location` field Kotlin writes in `Grave` is the walking entity's
  location and the NPC kernel's copy, written together.
- `NPC.originalId` is kept for an NPC from its first transform; until then
  its id is the original.
- The dragons' static `DRAGONFIRE` attacks and each dragon's `combatAction`
  are made the first time the dragon is asked for its swing handler, not at
  class load and construction (making them draws nothing; only object
  creation moves). MonkNPC's static `CombatAction` likewise.
- A subclass's `finalizeDeath` is reached through
  `npc_death::task_stop_with` (the library's `DeathTask.stop` with the NPC
  member given by the caller); the base member is
  `npc_death::npc_finalize_death`.
- `getQuestRepository().getQuest(q)` answers the quests the original
  registers (only Witch's Potion is named by the installed classes) as
  present; their stage is the persisted stage, 0 when the save holds none
  (the original's repository puts 0 for every registered quest).
- The persisted diaries are taken out of the player's profile for one
  diary call and put back after it.
- `NPC.getSwingHandler(swing)` of a base NPC does not read `swing`; the live
  world passes true.
- A player's `DialogueInterpreter` is made the first time it is asked for,
  not in the Player constructor; `DialogueInterpreter.close()` asked by a
  walk request runs right after the request returns, answered true
  meanwhile.
- A content listener's arrival (zone check, face, the handler) and a player
  option handler's `handle` run right after the walking call that reached
  them (they need the whole world). A destination override is evaluated
  once, when the movement pulse is made, not at each path search.
- The start-up constructs the NPCs of `BankerNPC.NPC_IDS` as plain NPCs: a
  banker gets its class and the booth part of `BankerNPC.init` the first
  time it is interacted with; `super.init()` is not repeated and BankerNPC's
  prototype is not constructed at registration.
- The follow pulse's `stop()` override (`mover.face(null)` after the
  movement's stop) is not run when another pulse replaces it; the walk
  request that replaces it clears the face itself.
- A bank button's `BankUtils.transferX` (its last call) is made right after
  the button's callback returns; a shop's or trade's `sendDialogue`,
  `sendNPCDialogue` and `sendInputDialogue` of a shop call likewise right
  after the shop call returns.
- `Shops.startup` runs at the live boot, after the start-up (as the shared
  drop tables do); its NPC listeners are registered after BankerNPC's and
  NPCTalkListener's.
- The `runscript` attribute of a prompt whose callback is not the bank's
  transfer object holds a mark; the callback is kept by the world.
- A potion's dose script is kept by the world, not in the canonical script
  queue: it runs at the start of the player's `Player.tick` once due, and
  `getActiveScript()` answers none for it.
- A teleport pulse still running when its player logs out is stopped with
  the base stop at the logout (no `TeleportPulse.stop` override, no ending
  animation); the original's pulse would run on against the stale Player
  object, which the live world no longer holds.
- `ShopHost` keeps the "shop", "shop-cont" and "shop-main" attributes, the
  trade world the `lastRequest` attribute, the RequestManager target and the
  TradeModule extension, by player (they hold objects).

## Answers by name (`world::live::not_ported`)

Skipped (the original runs content or a system without a port; nothing is
sent or changed; the first call of each is logged with `--verbose`):
`addAutoTimers`' GrandExchangeTimer and HolidayRandoms,
`AggressiveHandler`'s tolerance calls from the NPC renderer,
`DynamicSkillLevelChangeEvent`, `InterfaceOpenEvent` and `InterfaceCloseEvent`
dispatch (entity event hooks), `GlobalStats.incrementDeathCount`,
`SlayerEquipmentFlags.updateFlags` after a death, `PlayerMonitor.log` PK
(logged only), `Repository.sendNews` (permadeath),
`ButtonClickEvent` dispatch, `checkForWealthUpdate`,
BankerNPC's prototype construction, `DialogueOpenEvent` and
`DialogueOptionSelectionEvent` dispatch, `ItemShopPurchaseEvent`/
`ItemShopSellEvent` dispatch, `PlayerMonitor.logTrade` (logged only),
`GrandExchange.addBotOffer` from the shops' player stock clear (the items
are dropped without being offered),
`CombatPulse.stop` before an interaction, `Commands.define`,
`ComponentPlugin.open`, `Discord.postPlayerAlert`, entity tick event hooks,
`Entity.hook(UsedWith)`/`unhook` of DarkZone, `Entity.scripts.removeWeakScripts`
in `InteractionListeners.run`, the equip listeners' `removeWeakScripts` and
event, `InteractionEvent`, `PickUpEvent`, `UseWithEvent`, `ItemEquipEvent` and
`ItemUnequipEvent` dispatch, `FamiliarManager.login`, `FriendsListInterface.open`,
`GameWorld.loginListeners`, `GameWorld.logoutListeners`, `GameWorld.tickListeners` other
than GraveController and Shops,
`HouseManager.leave`, `InterfaceListeners.runOpen`, `PlayerMonitor` drop-trade
log and DUPE_ALERT (logged only),
LoginConfiguration's per-item equipment hooks, `Managers.tick`, NPC attributes (return-to-spawn,
lastForceTalk), `Player.tick`'s potion timers,
wealth and off-map check, `PlayerMonitor.log`/`logChat`/`logPrivateChat`/
`logXpGains`, `XPGainPlugins.run`, `XPGainEvent`, `PrayerActivatedEvent`/
`PrayerDeactivatedEvent`, `SelfDeathEvent` and `NPCKillEvent` dispatch,
`PlayerStatsCounter.incrementKills`/`incrementRareDrop`, `core.api.sendNews`
(rare drops) and `Repository.sendNews` (200m experience),
`Properties.updateDefenceAnimation`, the region activity pulse (no unloading),
`RulesAndInfo.openFor`, ServerStore `daily-accounts` `addToList`,
`WalkingQueue.reset` at logout; in saves:
`AttributeSetEvent`, `FamiliarManager.parse`, `HouseManager.parse`,
persist-player content hooks.

The original's answer with no content registered: `AssistSession` (none),
`BrawlingGlovesManager.GloveCharges` (empty: the manager is not ported),
`DialogueInterpreter.open` of an NPC key (false: no content dialogue plugin
is registered), `DialogueInterpreter.close` after an equip or a drop (true),
`AntiMacro.getEventNpc` and `HolidayRandoms.getEventNpc` (null: no random
event content), `hasAwaitingGrandExchangeCollections` (false: no offer
records without the Grand Exchange), `AIRepository.PulseRepository` (no
bot), `FamiliarManager.familiar` for a consumable (null),
`SlayerManager.getTask` (no task), `NPC.getTask` outside the timers (null;
Poison's `onRegister` reads `Tasks.forId(id).levelReq` from
`world::slayer_tasks`), `DegradableEquipment.checkWeaponDegrades`/`checkArmourDegrades`/
`degrade` (nothing: no degradable equipment registered),
`ClueScrollPlugin.ORGANIZED` in a drop roll (none registered),
`FamiliarManager.hasFamiliar` (false), `FamiliarManager.dismiss` after a
death (nothing), `InterfaceListeners.runClose` (true),
the Marathon perk (false), `Player.logoutListeners` (empty),
`QuestRequirements.hasRequirement` (false), ServerStore `daily-accounts`
(empty: the account limit counts nothing across logins) and `flagged-ips`
(empty), skillcape perks (false, `skillcape_perks` off),
`ZoneMonitor.move` (no installed region zone overrides move: true),
`ZoneMonitor.interact` for ground items (false), `ZoneMonitor.useWith` with no
zone (false), `ItemDefinition.getItemPlugin`
(null: no item plugin is registered), `PluginInteractionManager` for ground
items, NPCs, scenery and use-with (false), `InteractionListeners` for ground
items (false) and for a scenery option nothing registers (false),
`UseWithHandler.HANDLERS` (null: no content use-with handler),
`NPCDefinition.getOptionHandler` (null: NPC option handlers are content),
`DegradableEquipment.getDropReplacement` (the item's own id: no degradable
equipment registered),
`SlayerEquipmentFlags.isSlayerEq` (false), `DialogueInterpreter.close` after
an equip (true).

Refused (an error the caller logs; never a success): the routed packets
the driver refuses (commands, quick chat, report abuse, GE); content
item and NPC option handlers and listeners (`InteractionListeners`
functions other than DropListener, EquipHandler, AttackListener,
GraveController's, ConsumableListener, NPCTalkListener's, BankerNPC's,
Shops' and the global handlers' RequestOption listeners; a listener's walk
to an item; `defineInteraction` scripts; a destination override this
world did not register), item plugins (`ItemPlugin.remove`, `canPickUp`), the guild items'
dialogue, `EquipHandler.unequip`'s caller other than the global equipment
tab, equipment plugins' `fireEvent`, brawling gloves, autocast clearing on a
weapon change, `SlayerEquipmentFlags.updateFlags`, the destroy dialogue
(`sendDestroyItem`, `addDialogueAction`: not bound to the interpreter), `FamiliarManager.summon` of a pet
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
port: NPC settings and spell books in timers (only a player's are read),
familiars, SpellbookSwap's logout listener, weapon interface, autocast,
spell book tab and varbit (Lunar spells are not ported), the random event
content of AntiMacro's run (`RandomEvents`), and while a save is written or
parsed every service but uids, ticks, the generator and the log; the global
handlers' services without a binding (each `GHost` method named "global
handler service NAME": the dialogue senders, deposit boxes and the
collection box, quests and diaries, clans, assist sessions, bot info,
familiars, activities, spells and special attacks, interfaces the tabs
open other than through the tab libraries, pulse bodies other than
FollowOptionPlugin's, the settings toggles other than retaliate and the
special bar, item containers' other calls); talking and banking: the bank
PIN entry interface and PIN settings, the deposit box's item rendering,
BankInterface's help and deposit dialogues, a transfer's after callback,
buying the secondary bank account, the Grand Exchange collection box,
`new NPC(id)`, quest stages and the slayer gem in a dialogue DSL, the
Lunar Isle seal, a morphing NPC's shown NPC, random event and holiday NPC
conversations, the barcrawl and gnome cooking dialogues, a
TradeCloseEvent reached by an interface close without the world (deaths
and the other interface paths that close through the plain context);
shops: content item views, tanning, the quest
reads of Siegfried Erkle's, the fur trader's and the candle maker's
listeners; trade: `AssistSession.open`, the money-doubling bots;
consumables: `Player.sendChat`, the Trouble Brewing rum's pulses and
teleport, diary tasks, queued scripts other than the dose message; scenery:
the hardcoded constructed scenery of `processSceneryAction`, varbit- and
varp-configured scenery (morphs) and their debug lookup, the bank booths'
`defineInteraction` scripts (interaction scripts in the player's script
queue) and the bank chest and deposit box listeners (the walk to scenery
for a content listener), scenery option handlers that are not global handlers (doors and other
content); use-with on scenery, the use-with MovementPulse (NPCs, players,
ground items) and item-on-item Pulse with handlers, `UseWithHandler.handle`;
the slot switch's bank calls; content NPC clipping suppliers;
special attack handlers (`instant_spec`), `DialogueInterpreter.sendDialogue`
from the tab libraries and prayer (a locked emote), the SALUTE, GOBLIN_BOW and GIVE_THANKS emotes,
`AchievementDiary.updateTask`, tutorial content, `sendLastLoginInfo` of a host name, ContentAPI `runTask` with more than one repetition, a
saved spawn location or `login_type` attribute, the loot-share pulse, the
management-server session, account creation from social, familiars, region
NPC queries (`Repository.findNPC(location)`), the deferred login world
configuration of CloseIface, the zone services of
teleports, the scene's scenery services (doors, audio), content script
invocation; graves' services without a port: the server store's grave
archive (`ServerStore.getArchive`: graves are neither saved nor read back;
`serializeToServerStore`/`deserializeFromServerStore` are not called),
`GravePurchaseInterface`, `updateGraveType` and the `forcegravedeath`
command; the NPC class bodies' services without a port: DarkWizardNPC (not
registered: `SpellBook.MODERN`, `setAutocastSpell`,
`combatPulse.setStyle`), `IdleAbstractNPC` (none registered), `reTransform`,
`setDirection`, a player's `sendChat`/`faceLocation` from a class body,
the diary interface (`open`, its tab and script) and diary rewards
(containers); the dragonfire shield's own handler and listener
(`updateBonuses` after a charge, its varp, locks, `delayEntity`,
`replaceSlot`, the item plugin and listener) and `Projectile.transform` of
a switching attack; runtime NPC construction from a class body,
`AbstractNPC` subclass `init` of a dragon or an unregistered class at
runtime, a familiar's hp attribute; combat's services without a port:
`Permadeath.permadeath`, equipment plugins' `fireEvent("unequip")`
and item plugins' `getDeathItem` in a death, the specials' services without a port (`Repository.findNPC`
for Chainhit's door supports, the anonymous RSTimers of Hamstring and
Phantom strike, `Clocks.STUN`, Shove's dumb path and `addPath`, a player's
`sendChat`, prayer points of an NPC), the chinchompa and
salamander handlers, `BoltEffect` overrides other than DIAMOND, the duel
arena's ammunition list,
familiars (`Familiar.getOwner`, `getAttackStyle`, the pet test),
`EnchantedJewellery.RING_OF_LIFE`, `settings.updateRunEnergy` from combat
and `Player.fullRestore`, `sendDialogue` from prayer, `Player.sendChat`
(force chat on a player, Vengeance), the safespot rule's `Pathfinder.find`/
`walk`/`moveStep`, item
plugins' `createDrop`/`getItem`, loot share (clan, points,
`Communication.isLootShare`), `GrandExchange.getRecommendedPrice` in a drop,
`AIRepository.addItem`, `KillerwattBehavior.aggressUponKillerwatt`, NPC
chunk moves, the return pulse and `removeRenderableNPC` outside the NPC
tick.

Spells and teleports refused by name (`magic.rs`): the `fireEffect` of the
spell classes with an effect (binds, curses, the god spells, Teleblock and
every ancient spell: Ice, Blood, Shadow, Smoke and Miasmic), refused when a
cast or an autocast would select them; NPC casters' `DefaultCombatSpell`
(`Projectile.magic`); multi-target spells (`findMultihitTargets`); the
non-combat `MagicSpell` plugins (EnchantSpell, EnchantCrossbowSpell,
TelekineticGrabSpell, TeleotherSpells, ChargeSpell, BountyLocateSpell,
AncientTeleportPlugin, and the lunar MagicImbue, Vengeance, StatBoost,
StatRestore and Heal spells); every other `SpellListener` (home teleport,
Camelot (diary task), Ardougne, Watchtower, Trollheim and Ape Atoll (quest
requirements), house teleport, alchemy (and MTA's), superheat, bones to
bananas/peaches, charge orb, the whole lunar book); a combat spell cast on
an item or ground item; spells on scenery (charge orbs: scenery is not a
node of the item handlers); familiar specials; the autocast selection tab
(`openAutocastSelect`'s `openTab`) and the combat pulse's autocast
component close (`interfaceManager.close`, `openTab(weaponInterface)`);
`playAudio` with a location; the spell classes' `openTab`, `setVarbit`,
`setComponentVisibility` and `setAttackStyle`. Skipped by name:
`SpellCastEvent`, `TeleportEvent`, `AttributeSetEvent` and
`AttributeRemoveEvent` dispatch (entity event hooks are not ported) and
`removeWeakScripts` in `SpellListeners.run`.

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
  login tick: of the original's 394 entries after the reply, 109 come from
  calls answered as not ported (pinned by marker and count:
  CropCircleController 1, FriendsListInterface 2, FamiliarManager 3,
  QuestRepository 103 (its first entry, the points varp, is native)); the
  169 entries before `reinitVarps` are equal in order, 123 byte for byte
  (LoginValidationPlugin's "Your debug mode is toggled to true." and
  MusicPlayer.play's track name with the " (62)" debug mode adds are now
  native) and 46 counter-carrying interface packets by opcode and length
  (the skipped calls' packets advance the original's interface packet
  counter); `reinitVarps`' 42 native varps are all among the original's
  116; nothing else follows. The weapon tab (24 entries), the
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
- `traceone` uses the first option of its nearest NPC (`processNpcAction`).
  A "Talk-to" goes through the native InteractionListeners (NPCTalkListener's
  catch-all, the movement pulse and its arrival): the player walks to the
  NPC and `dialogueInterpreter.open(npc.id, npc)` finds no content dialogue
  registered. Any other first option is the NPC's InteractPlugin with the
  legacy walk and arrival: "Nothing interesting happens." (no content
  handler).
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
- `fightone` (a save in the Lumbridge chicken pen at 3232,3294 with attack
  and strength 40) toggles Thick Skin on the prayer tab (IfAction on
  271:5, the global handlers' PrayerTabInterface over `world::prayer`: varp
  83 = 1), then uses Attack (NPC option 2, opcode 3) on the nearest chicken
  (41 or 1017; a chicken the fence keeps out of reach times the combat pulse
  out, and the next is tried): AttackListener, the combat pulse, the melee
  swing, the impact pulse and `ImpactHandler`; the hit marks are read back
  from the NPC update's mask blocks (the RT4 client's reader in
  `rs09_protocol::game::npcs`), the chicken dies (DeathTask) and the tile it
  last stood on holds its drop (bones 526 and raw chicken 2138, through
  `NPCDropTables` and the ground manager); hitpoints experience rises above
  1154.
- `ratbait` (a save next to the Lumbridge swamp's giant rats, 86, aggressive;
  the save sets `allow_admin_aggression`, as the test accounts are
  administrators) is selected by `AggressiveHandler.selectTarget`, and the
  rat's hit mark arrives in its own player update (a miss: the character has
  not completed the tutorial, so `handleImpact` queues MISS).
- `deathone` (a save next to the giant rats with one hitpoint, `tutorial:complete`
  and `allow_admin_aggression`) passes the lobby (the welcome screen with
  `LastLoginInfo` 164; IfAction 378:140 runs `configureGameWorld` a tick
  later), is killed by a rat (the death task, `Player.commenceDeath`,
  `Player.finalizeDeath`): "Oh dear, you are dead!" exactly once, the death
  screen 153, hitpoints restored to 10, respawned at the home location
  (3222,3218); an administrator keeps its items (no grave, no drop).
- `graveone` (an account registered in the world's account storage as a
  regular player before its first login, one hitpoint, `tutorial:complete`,
  six items) is killed by a giant rat: three items are kept, and the other
  three with the bones lie on its death tile, private to it (its ground
  item list shows them; `gravewatch`, standing nearby, does not); it reads
  "Because of your current gravestone, you have 2 minutes to get your items
  back." (colourized), gets the HintIcon packet (217, slot 0, an NPC
  target) from `Grave.tick`, and `gravewatch` sees the grave NPC (6565,
  MEM_PLAQUE) added. Against a running server (no account storage in
  reach) the scenario prints SKIPPED.
- `ratone` (Witch's Potion at stage 10 in its save) kills a rat (47, a
  `RatNPC` of the start-up) west of Draynor: RatNPC's `finalizeDeath` after
  the base death drops the rat tail (300) on the rat's tile.
- `dragonone` (Hitpoints, Defence, Attack and Strength 99) attacks a green
  dragon in the west wilderness (2982,3618): its MultiSwingHandler picks the
  dragonfire and the player reads "You are horribly burnt by the fiery
  breath." (`calculateDragonfireMaxHit`).
- `rangeone` (a shortbow 841 and 100 bronze arrows 882 worn, Ranged 40) shoots
  a chicken: the range swing, the arrow's projectile in the chunk update
  (opcode 16 in 230, target the chicken's index + 1), `useAmmo` (99 arrows
  left) and the hit mark.
- `specone` (a dragon dagger 1215 worn, Attack 60) turns the special bar on
  (the weapon tab's special bar button: varp 301 = 1) and attacks a chicken:
  Puncture (registered at boot) drains 25 energy (varp 300 = 750) and turns
  the bar off (varp 301 = 0).
- `bankone` (in front of Draynor bank's middle booth, a bronze dagger)
  talks to the banker behind it (494; NPC option 1): BankerNPC's "talk-to"
  listener with its destination override (the tile beyond the booth), the
  walk, the arrival, `DialogueLabeller.open` and the banker conversation:
  "Good day, would you like to access your bank account?" in chatbox 241;
  Continue (132) shows the options; the "access" option's component is
  answered and the player speaks it; Continue opens the bank (762). A
  deposit (155 on 763:0, slot 0) moves the dagger to the bank container
  (95), a withdraw (155 on 762:73) brings it back, Close (184) runs the
  captured BankClose (script 571). The banker's Bank option (3) then opens
  the bank directly (`BankerNPC.attemptBank`).
- `shopone` (Lumbridge's general store, 100 coins, a bronze dagger) uses
  Trade on the shop keeper (520): Shops' NPC listener and destination
  override, `Shop.openFor` (620 with the side tab 621, the title text).
  Buy-1 (196 on 620:23) buys a pot (1931) for a coin; Value (155 on 621:0)
  and Sell-1 (196) sell the dagger into the store's player stock for 4
  coins; after switching back to the main stock (620:25), Buy-X (234)
  sends the amount prompt and the answer (INPUT_INT 23: 2) buys two more
  pots through `RunScript.processInput` and `shop::run_amount`.
- `tradeone` (a bronze dagger) and `tradetwo`: Trade with (player option
  3) from one sends "Sending a trade offer..." and the other reads
  "tradeone:tradereq:"; the other's Trade with accepts and both get the
  trade screen (335, side tab 336); the dagger is offered (155 on 336:0),
  both accept (335:16), the second screen (334) opens, both accept
  (334:20) and the dagger is in `tradetwo`'s inventory.
- `followone` uses Follow (player option 2) on `followtwo`: the bound
  FollowOptionPlugin, the walk to the player and the follow pulse; when
  `followtwo` walks six tiles west, `followone` ends beside it.
- `eatone` (4 of 10 hitpoints, shrimps 315, a strength potion (4) 113)
  eats the shrimps (ITEM option 1): "You eat the shrimps.", "It heals some
  health.", hitpoints 7, animation 829; it drinks the potion: "You drink
  some of your strength potion.", a three-dose potion (115) in the slot,
  Strength raised, and two ticks later "You have 3 doses of potion left.".
- `mageone` (magic 40, the tutorial complete, 100 air and mind runes) casts
  Wind Strike (192, button 1) on the nearest chicken with the spell-on-NPC
  packet (239): the spell's projectile (91) toward it, its hit mark, one
  air and one mind rune gone and magic experience added.
- `teleone` (magic 40, the tutorial complete, a fire, three air and a law
  rune) presses Varrock Teleport on the magic tab (192:15): it arrives at
  (3213,3424) four ticks later, the runes are gone and 35 experience is
  added; a second press after the magic delay gets "You don't have enough
  law runes to cast this spell." (`hasRunes` walks its cost map in Java
  HashMap order) and no teleport.
- `lookone` (saved female, body colours 5/3/7/2/4, 40 Attack, Strength and
  Defence, a bronze two-handed sword 1307) and `looktwo`: `looktwo` reads
  `lookone`'s appearance block with the gender bit, the body slots of the
  saved looks, the colours, the name, combat level 38 twice and no
  wilderness level, render animation 1426; `lookone` receives the same
  block for itself. `lookone` wields the sword: the weapon slot is worn and
  the render animation is 124 (`Appearance.setAnimations`).
- `reconone` walks two tiles and its socket is dropped; it logs in again
  with the reconnect operation (18) until admitted (AlreadyOnline while the
  old player is in the repository; 4 such replies and 1.8 s in the measured
  run), is rebuilt where it stood, receives its own appearance block, and
  walks; `recontwo`, online throughout, sees it return with its appearance.
- `RS09_LIVE_COMBAT_ONLY` runs only the combat scenarios (fight, aggression,
  death, grave, subclass, dragon, ranged, magic, special) after the first
  character's save (a shortcut while working on combat);
  `RS09_LIVE_SCENARIOS=a,b` runs only the named ones.
- At least ten ticks, a mean interval between 590 and 700 ms.

`running_server_scenario` (ignored) drives the same client against a
running `rs09-server --world` (`RS09_LIVE_ADDRESS`, `RS09_LIVE_SAVES`).

Measured (2026-11-26, this machine, with the eighth round installed as
well: dialogue, the banker, the bank, shops, trade, follow, eating):
in-process debug test 300 ticks, mean work 32.69 ms, maximum 1,063.55 ms
(the first login: lazy definition loading in a debug build), mean interval
604.8 ms, 36 saves written (189.0 s). Release binary (`rs09-server
--world`, start-up 892 ms, 13,709 NPCs joined), the ignored scenario
against it (144.4 s, every step above but the grave, which needs the
in-process account storage), then idle: 599 ticks, mean work 6.95 ms,
maximum 147.86 ms, mean interval 600.3 ms (`Shops.tick` over the 245
shops runs every tick).
The world-login, tick-driver and world-trace comparisons of the libraries
are unchanged (their tests bind the same libraries with replayed
stand-ins); this test checks the production binding end to end.

## Not covered / still to do

- The remaining routed handlers: the driver's refused packets (commands, quick chat, abuse reports, the GE packet);
  the global handlers' dialogue services (ClimbDialogue, `send_options`,
  the dialogue senders) are still refused although the interpreter is
  installed; `ZoneMonitor.interact`/`useWith` for items inside a zone,
  hint icons and warning messages, the DarkZone's overlay and light sources
  and the multiway overlay.
- The rest of combat: the spells with effects, multi-target and NPC spells,
  autocast selection from the weapon tab, and the spell listeners refused
  above (the next step of spells: the binds' and ancients' `fireEffect`
  over the NPC timers), the specials' services listed as refused, the
  ancient, home, diary- and quest-gated teleports,
  graves in the server store, the equipment registers (chinchompa and salamander handlers, the
  degrader; docs/equipment-registers.md) and the creature behaviours.
- The special attacks, the player's death, graves, the NPC class overrides
  and the dragons are installed without a corpus of the live binding
  itself: their libraries are the differentially tested ports
  (special-attacks-oracle, player-death-oracle, npc-classes-oracle,
  dragonfire-oracle). Ported here from the pinned sources without a
  differential corpus: `Projectile`/`ProjectileUpdateFlag`, `NPC.transform`,
  `NPC.clear` and the runtime half of `NPC.init` (runtime_npcs.rs),
  `GraveController.hasGraveAt`, ContentAPI `registerHintIcon`/
  `clearHintIcon`, `Player.toggleDebug`, `SystemConfig.validLogin` and
  `Projectile.copy` for the metal dragons' breath.
- Left of the install round's first two items: the bank booths
  (`defineInteraction` scripts: the interaction script of the player's
  script queue with its distance, line-of-sight and face services), bank
  chests and deposit boxes (the walk to scenery for a content listener; the
  deposit box's item rendering), the bank PIN (the entry interface and the
  PIN settings; a character with a PIN cannot open its bank here), the
  secondary bank purchase, assist sessions (`world::player_managers`'
  AssistSession), the global handlers' dialogue services, and ForceMovement
  (`world::force_movement`: its Host, its pulses by identity and the
  ForceMove mask encoder, which is not ported; nothing installed calls it).
- The rest of the install round (in order): the rest of spells and
  teleports (above), the teleport jewellery and tablets, the everyday item handlers, zone overlays
  and `ZoneMonitor.interact`/`useWith` inside a zone; commands, quick
  chat, abuse reports and the Grand Exchange; the equipment registers and
  the creatures.
- This round's bindings have no corpus of their own: the libraries are the
  differentially tested ports (dialogue-oracle, dialogue-dsl-oracle,
  input-prompt-oracle, bank-oracle, bank-interface-oracle, shops-oracle,
  trade-oracle, consumables-oracle, npc-classes-oracle,
  interaction-listeners-oracle, global-handlers-oracle). Written here from
  the pinned sources without a differential corpus: the booth part of
  `BankerNPC.init` applied to a start-up banker, ContentAPI
  `hasIronmanRestriction`, `openBankAccount`, `toggleBankAccount` and
  `hasActivatedSecondaryBankAccount`, `RandomEvents.randomIDs` and
  `HolidayRandomEvents.holidayRandomIDs` (id lists read from the pinned
  classes), `player_walk::World::listener_motion` (a constructor wrapper),
  ContentAPI `sendMessage`'s 86-unit split as used by shops, trade and
  consumables, and `Entity.setDirection` for an NPC.
- The per-player or per-world libraries behind other buttons and options:
  gathering listeners, quests and diaries, clans; content listeners and
  doors.
- A transformed player's appearance (`transformNPC`), familiars behind the
  appearance flag's summoning fields; region unloading; the climb animation (828) was not seen in the player
  update by the test (the climb itself is).
- NPC timers have no corpus of the live binding: `timers::content` is the
  differentially tested port (content-timers-oracle); the join (the NPC
  manager, NPC skills, `GrandExchangeTimer`'s NPC branch, the slayer table
  generated from the pinned `Tasks.java`) is checked by
  `tests/live_npc_timers.rs` over the booted world. A runtime NPC that is
  cleared keeps its hook lists and its `SkillRestore` join mark (node ids
  are not reused).
- A failing player tick still ends the rest of that tick's sequence (the
  original's `handleTickActions` catch); only NPC ticks and resets are
  isolated.
- Reconnect beyond one dropped connection (in combat, with open interfaces,
  the client's own state-40 flow against this world), the lobby, AFK logout's
  client side, many players (capacity is
  not measured), concurrent saves of one character.