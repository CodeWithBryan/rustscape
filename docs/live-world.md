# The live world (`rs09-server --world`, `world/live`)

`crates/server/src/world/live/` installs the verified libraries in a running
server: the native start-up, the login, the tick driver, the repository and
disconnection queue, social, the canonical players with their own libraries,
the player tab systems (weapon tab, level-up icons, emotes, music, quest tab),
walking on the canonical actor state, the player update with prepared masks,
the start-up's NPCs (tick, random walking, the NPC update), entity regions
and zone monitors with music zones, region activation, the map chunk renderer
with chunk synchronisation, and the interface buttons of the player tabs.
A client logs in, enters the world, walks, sees NPCs and other players, uses
its weapon, emote and music tabs, logs out, and its character is saved and
loaded again.

This is a playable engine slice, **not a complete server**: content (login
listeners and plugins, quests, tutorial, NPC aggression and combat, scenery
and item handlers, dialogue, banking, shops, prayer, trade), ground items,
worn equipment in the appearance block and every handler listed below as
refused are not installed, and every such call is answered by name below.

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
| admission | `Login.proceedWith` … `LoginParser.initialize` | `login::admission::read` on the worker thread at the start of a tick |
| login pulse | LoginParser's `Pulse(1)` | `login::pulse::run` from `Driver::pulser` (`driver::Host::pulse_body`) |
| login reply | `LoginWriteEvent` | `login::write_response` → `login::Host::write_bytes`, which completes the transport's admission with the same rights and index (the transport writes the identical bytes) |
| worker | `MajorUpdateWorker` | `server::worker_loop`: admissions, the read event, `Driver::tick`, the hand-offs, the management pump and social flush, delivery, the ping timeout (20 s) and the inactive-player failsafe; sleep to 600 ms |
| read event | `IoReadEvent`/`GameReadEvent.read` | `Driver::read` on the bytes of the frames the transport framed (re-encoded with the same size table); end of stream is `Driver::close` |
| packet routes | `PacketProcessor.process` | the driver's table; `Walk` (`player_walk::World::process`, i.e. `world::walk_request`), `Social` (`Social::process`), `DisplayUpdate` (client info and `interfaces::switch_window_mode`), `IfAction` and `CloseIface` (`interface_packets::if_action`/`close_iface`, `buttons.rs`) bound; the rest below |
| pulses | `GameWorld.Pulser.updateAll` | the driver's runner (login pulses, the combat tab's one-tick pulses), then the canonical state's runner (movement pulses) at the start of the tick-listener phase |
| `Player.tick` | `Entity.tick`, music, prayer … | `player_walk::World::entity_before_masks` (script phases through `script_runtime::phase`, the walking queue) and `prepare_masks`, then `MusicPlayer.tick`; the queue's entity is projected into the render view and the slot |
| player tabs | `WeaponInterface`, `LevelUp`, `EmoteManager`/`Emotes`, `MusicPlayer`, `QuestRepository.syncronizeTab` | `world::player_tabs` with `tabs::Tabs` as their host (the world-login test's binding): the weapon tab is created with the tab array (`create_weapon`), opened by `WeaponInterface.open` and refreshed by `updateInterface` from the equipment listener; `LevelUp.sendFlashingIcons` from `Skills.refresh`; the emote, music and quest tabs at login; the persisted emotes, songs and quest stages joined, not copied |
| tab buttons | `CombatTabInterface`, `EmoteTabInterface`, `MusicTabInterface` | the component plugin of interface 92 and every weapon interface (attack styles through `WeaponInterface.setAttackStyle`, `openAutocastSelect`, `selectAutoSpell`; retaliate and special bar in `Pulse(1, p)`), listeners of 464 (`Emotes.handle`) and 187 (play, loop, the admin unlock); their few content lines are written over the libraries (not differentially tested themselves) |
| NPCs | `NPC.init`, `NPC.tick`, `NPC.reset` | the start-up's NPCs in the live repository (same slots and indices) and in the scene's chunks; `npc_walk::tick` (the `world::npc` kernel on the canonical actor's queue, navigation with the behaviour's pathfinder and clipping supplier, the shared runner, `RandomFunction.RANDOM` of the start-up's generators); behaviour hooks through `npc_behaviour` with the start-up's NPC store; `NPC.onRegionInactivity` through `npc_behaviour::lifecycle` (`npc_life.rs`) |
| `Player.update` | scene graph, three renderers | the native rebuild writer (XTEA keys from the start-up), `player_render::render` with `RegionManager.getLocalPlayers(location, 15)` candidates, `npc_render::render` with `getLocalNPCs` candidates, `map_chunk_render::render` with `chunk_sync` (synchronise, update, `ClearRegionChunk`) |
| regions | `RegionManager.move`, `Region.flagActive`, `ZoneMonitor` | `entity_regions::move_entity` at login and for every walking move (players and NPCs), over the scene's regions and chunks; `region_activity::flag_active` (load, renderable NPCs) from the move and the map chunk renderer; a zone monitor per entity (`zones::Monitor` with `Live` as its host) with `checkMusicZones` over the start-up's region music and music zones; `Entity.clear`'s chunk and tolerance part; `ZoneMonitor.clear` at logout |
| `Player.reset` | `Entity.reset` … | masks reset, teleport flag, render information, hitpoints packet, `flagged-for-save`, op counts; `ChunkUpdateTracker.resetAll` clears the scene's chunk update flags |
| `Animator.reset` | `entity.animator.reset()` | `prepared_animation` animators over the canonical masks and clocks, definitions from cache archive 20 |
| collision | `RegionManager` | `world::scene` over cache archive 5, regions loaded around players as they arrive |
| saves | `PlayerParser.saveImmediately`, `SaveTask`, autosave | the save text is built by the canonical player (`Player::save_text`) on the worker thread after the tick; the `PlayerSave` thread writes `<saves>/players/<name>.json`; the autosave every 50 ticks and a final save of every playing character at shutdown |
| account | `details.save()` | the account storage `Auth.configure` made from the configuration (in memory with `persist_accounts = false`) |

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
- The appearance block is the prepared publisher's (`appearance::Data::naked`
  with the name; worn equipment is not shown), registered by
  `Appearance.sync` and `updateAppearance` at login.
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
- Player timers: no player timer type is ported; saved timers are dropped on
  load (the original's registry would resolve them).
- The daily restart is not run: `daily_restart = false` in the pinned
  configuration and its ServerStore daily/weekly clears are store archive
  writes that are not ported. `Server.heartbeat` (watchdog off) and the
  network reachability check are not run.

## Answers by name (`world::live::not_ported`)

Skipped (the original runs content or a system without a port; nothing is
sent or changed; the first call of each is logged with `--verbose`):
`AggressiveHandler.selectTarget` (no target) and its tolerance calls,
`Appearance.setAnimations`, `ButtonClickEvent` dispatch, `checkForWealthUpdate`,
`CombatPulse.updateStyle`, `CombatPulse.stop` and `ImpactHandler.setDisabledTicks`
for NPCs, `Commands.define`, `ComponentPlugin.open`, `Discord.postPlayerAlert`,
entity tick event hooks, `FamiliarManager.login`, `FriendsListInterface.open`,
`GameWorld.loginListeners`, `GameWorld.logoutListeners`, `GameWorld.tickListeners`,
`GroundItemManager.pulse`, `HouseManager.leave`, `InterfaceListeners.runOpen`,
LoginConfiguration's per-item equipment hooks, `LoginConfiguration.LOGIN_PLUGINS`,
`Managers.tick`, NPC attributes (return-to-spawn, lastForceTalk), `NPC.sendChat`,
NPC update masks (reset and blocks), `Player.tick`'s potion timers, prayer
drain, wealth and off-map check, `PlayerMonitor.log`/`logChat`/`logPrivateChat`,
`Properties.updateDefenceAnimation`, the region activity pulse (no unloading),
`RulesAndInfo.openFor`, ServerStore `daily-accounts` `addToList`,
`Skills.pulse` restoration, `TimerRegistry.addAutoTimers` for players,
`timers.processTimers`, `WalkingQueue.reset` at logout; in saves:
`AttributeSetEvent`, `FamiliarManager.parse`, `HouseManager.parse`,
persist-player content hooks, player timer types.

The original's answer with no content registered: `AssistSession` (none),
`CombatPulse.isAttacking`/`isInCombat` (false: the combat pulse is not
installed), `DeathTask.isDead` (false: nothing starts a death),
`DialogueInterpreter.close` (true), `DialogueInterpreter.getDialogue` (null),
`FamiliarManager.hasFamiliar` (false), `InterfaceListeners.runClose` (true),
the Marathon perk (false), `Player.logoutListeners` (empty),
`QuestRequirements.hasRequirement` (false), ServerStore `daily-accounts`
(empty: the account limit counts nothing across logins) and `flagged-ips`
(empty), skillcape perks (false), walking timer queries (none),
`ZoneMonitor.move` (no region zones registered: true; `ZoneBuilder` is not
run).

Refused (an error the caller logs; never a success): routed packets whose
library is ported but not installed here (`SceneryAction`,
`GroundItemAction`, `ContinueOption`, `InputPromptResponse`) and those the
driver refuses (`ItemAction`, `NpcAction`, `PlayerAction`, use-with,
component actions, slot switches, examines, commands, quick chat, report
abuse, GE, music track finished); content close events and open dispatch,
open listeners of content interfaces, button listeners and component plugins
other than the three tab classes (bank, prayer, trade, shops), autocast
spells (`SpellBook.getSpell`, the autocast tab), special attack handlers
(`instant_spec`), `DialogueInterpreter.sendDialogue` (a locked emote),
the SALUTE, GOBLIN_BOW and GIVE_THANKS emotes, `AchievementDiary.updateTask`,
level-up rewards (no experience is gained), tutorial content, reconnect
(opcode 18), `sendLastLoginInfo` (the lobby), a saved spawn location or
`login_type` attribute, the loot-share pulse, the management-server session,
account creation from social, familiars, region NPC queries
(`Repository.findNPC(location)`), the deferred login world configuration of
CloseIface, an NPC's dialogue player, content NPC clipping suppliers, the
zone services of combat, skulls, timers and teleports, the scene's scenery
services (doors, audio), content script invocation, facing NPCs.

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
- At least ten ticks, a mean interval between 590 and 700 ms.

`running_server_scenario` (ignored) drives the same client against a
running `rs09-server --world` (`RS09_LIVE_ADDRESS`, `RS09_LIVE_SAVES`).

Measured (2026-11-17, this machine, NPCs joined): in-process debug test 26
ticks, mean work 70.5 ms, maximum 1,210 ms (the first login: lazy definition
loading in a debug build), mean interval 624.6 ms. Release binary
(`rs09-server --world`, start-up 945 ms, 13,709 NPCs joined from the
start-up's repository), the ignored scenario against it (15.4 s), then idle:
299 ticks, mean work 3.83 ms, maximum 180 ms (the first login pulse or a
100-tick collection), mean interval 600.3 ms.

The world-login, tick-driver and world-trace comparisons of the libraries
are unchanged (their tests bind the same libraries with replayed
stand-ins); this test checks the production binding end to end.

## Not covered / still to do

- Ground items: `GroundItemManager` (create, decay, `pulse`), drops to the
  ground, `GroundItemAction` and pickup (`ground_action`, `ground_runtime`,
  `pickup`), the start-up's ground spawns.
- The remaining routed handlers (`scenery_action`, `input_prompt`,
  `continue_option`) and the per-player or per-world libraries behind
  interface buttons and options: the dialogue interpreter, prayer and its
  tab, bank, trade and requests, shops (`shops::World`, its tick listener),
  drop tables, gathering listeners, the combat pulse, NPC death and drops,
  teleports, spells; the item, NPC and player option handlers merged in
  `world/packet_handlers` after this copy was made.
- `ZoneBuilder.startup` (wilderness and other region zones), region
  unloading, NPC aggression and NPC update masks.
- The appearance block with worn equipment (`appearance_writer`).
- Reconnect, the lobby, AFK logout's client side, many players (capacity is
  not measured), concurrent saves of one character.
