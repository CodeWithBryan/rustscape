# The live world (`rs09-server --world`, `world/live`)

`crates/server/src/world/live/` installs the verified libraries in a running
server: the native start-up, the login, the tick driver, the repository and
disconnection queue, social, the canonical players with their own libraries,
walking on the canonical actor state and the player update with prepared
masks. A client logs in, enters the world, walks, sees other players, logs
out, and its character is saved and loaded again.

This is a playable engine slice, **not a complete server**: content (login
listeners and plugins, quests, music, emotes, the weapon tab, tutorial,
NPC behaviour, scenery and item handlers) is not installed, NPCs are not
joined to the live repository, and every such call is answered by name below.

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
| packet routes | `PacketProcessor.process` | the driver's table; `Walk` (`player_walk::World::process`, i.e. `world::walk_request`), `Social` (`Social::process`), `DisplayUpdate` (client info and `interfaces::switch_window_mode`) bound; the rest below |
| pulses | `GameWorld.Pulser.updateAll` | the driver's runner (login pulses), then the canonical state's runner (movement pulses) at the start of the tick-listener phase |
| `Player.tick` | `Entity.tick`, music, prayer … | `player_walk::World::entity_before_masks` (script phases through `script_runtime::phase`, the walking queue) and `prepare_masks`; the queue's entity is projected into the render view and the slot |
| `Player.update` | scene graph, three renderers | the native rebuild writer (XTEA keys from the start-up), `player_render::render` with the candidates below and the canonical player's `PreparedMasks` as the mask writer |
| `Player.reset` | `Entity.reset` … | masks reset, teleport flag, render information, hitpoints packet, `flagged-for-save`, op counts |
| `Animator.reset` | `entity.animator.reset()` | `prepared_animation` animators over the canonical masks and clocks, definitions from cache archive 20 |
| collision | `RegionManager` | `world::scene` over cache archive 5, regions loaded around players as they arrive |
| saves | `PlayerParser.saveImmediately`, `SaveTask`, autosave | the save text is built by the canonical player (`Player::save_text`) on the worker thread after the tick; the `PlayerSave` thread writes `<saves>/players/<name>.json`; the autosave every 50 ticks and a final save of every playing character at shutdown |
| account | `details.save()` | the account storage `Auth.configure` made from the configuration (in memory with `persist_accounts = false`) |

## Deviations from the original

- Frames from the client are framed by the transport and re-encoded for the
  read event; a malformed size closes the connection in the transport
  instead of counting as an invalid packet.
- `GameWorld.Pulser` is two runners: the driver's (login pulses) and the
  canonical actor state's (movement pulses), run one after the other in the
  pulse phase position.
- Player update candidates are the live players within 15 tiles in
  repository order; the original walks entity regions (chunk order), which
  are not joined.
- The appearance block is the prepared publisher's (`appearance::Data::naked`
  with the name; worn equipment is not shown), registered by
  `Appearance.sync` and `updateAppearance` at login.
- Save text is built on the worker thread (the original builds it on the
  executor thread); the file write is off the tick thread.
- `Auth.configure` runs a second time with the live repository's name map as
  the online lookup (the start-up's registry looks at the start-up's own
  repository, which holds only NPCs).
- `interfaces::close` asked by the walk request (and `close_chatbox`) runs
  right after the walk request returns, answered true meanwhile.
- The quest registry is empty (quest classes are content): a returning
  character's quest points become 0 through `syncPoints`, as the original
  does with no quests registered.
- Player timers: no player timer type is ported; saved timers are dropped on
  load (the original's registry would resolve them).
- The daily restart is not run: `daily_restart = false` in the pinned
  configuration and its ServerStore daily/weekly clears are store archive
  writes that are not ported. `Server.heartbeat` (watchdog off) and the
  network reachability check are not run.

## Answers by name (`world::live::not_ported`)

Skipped (the original runs content or a system without a port; nothing is
sent or changed; the first call of each is logged with `--verbose`):
`Appearance.setAnimations`, `checkForWealthUpdate`, `ChunkUpdateTracker.resetAll`,
`Commands.define`, `ComponentPlugin.open`, `Discord.postPlayerAlert`,
`EmoteManager.refresh`, entity tick event hooks, `Entity.clear` region chunk
and tolerances, `FamiliarManager.login`, `FriendsListInterface.open`,
`GameWorld.loginListeners`, `GameWorld.logoutListeners`, `GameWorld.tickListeners`,
`GroundItemManager.pulse`, `HouseManager.leave`, `InterfaceListeners.runOpen`,
`LevelUp.sendFlashingIcons`, LoginConfiguration's per-item equipment hooks,
`LoginConfiguration.LOGIN_PLUGINS`, `Managers.tick`, `MapChunkRenderer.render`,
`MusicPlayer.init`, `NPCRenderer.render` (no NPC update is sent),
`Player.tick`'s music, potion timers, prayer drain, wealth and off-map check,
`PlayerMonitor.log`/`logChat`/`logPrivateChat`, `Properties.updateDefenceAnimation`,
`QuestRepository.syncronizeTab`, `RegionManager.move` (entity regions, zones,
music zones; the scene region is loaded), `RulesAndInfo.openFor`,
ServerStore `daily-accounts` `addToList`, `Skills.pulse` restoration,
`TimerRegistry.addAutoTimers` for players, `timers.processTimers`,
`WalkingQueue.reset` at logout, `WeaponInterface.open` (the attack tab is not
sent), `WeaponInterface.updateInterface`; in saves: `AttributeSetEvent`,
`FamiliarManager.parse`, `HouseManager.parse`, persist-player content hooks,
player timer types.

The original's answer with no content registered: keyed `ActionLocks`
(false), `AssistSession` (none), `DeathTask.isDead` (false: nothing starts a
death), `DialogueInterpreter.close` (true), `FamiliarManager.hasFamiliar`
(false), `InterfaceListeners.runClose` (true), the Marathon perk (false),
`Player.logoutListeners` (empty), `QuestRequirements.hasRequirement` (false),
ServerStore `daily-accounts` (empty: the account limit counts nothing across
logins) and `flagged-ips` (empty), skillcape perks (false), walking timer
queries (none), `ZoneMonitor.clear` and `ZoneMonitor.move` (no zones: true).

Refused (an error the caller logs; never a success): routed packets whose
library is ported but not installed here (`SceneryAction`,
`GroundItemAction`, `IfAction`, `CloseIface`, `ContinueOption`,
`InputPromptResponse`) and those the driver refuses (`ItemAction`,
`NpcAction`, `PlayerAction`, use-with, component actions, slot switches,
examines, commands, quick chat, report abuse, GE, music track finished);
content close events and open dispatch, autocast spells, tutorial content,
reconnect (opcode 18), `sendLastLoginInfo` (the lobby), a saved spawn
location or `login_type` attribute, keyed locks, the loot-share pulse, the
management-server session, account creation from social, familiars, NPC
tick/reset and region NPC queries, the scene's scenery services (doors,
audio), content script invocation, the safety-first emote, facing NPCs.

## Verification

```powershell
cargo test -p rs09-server --test live_world
```

`crates/server/tests/live_world.rs` (needs `assets/cache` and the template;
prints SKIPPED without them) starts the world in-process as `--world` does,
behind `runtime::Service`, and drives headless TCP clients built from the
native protocol code (`rs09_net::login::LoginPeer`, the game frame decoder,
the client-side player update decoder):

- `traceone` (new) logs in: reply 2, index 1, the rebuild (162) and player
  updates. Its login tick is compared with the world-trace new-character
  login tick: of the original's 394 entries after the reply, 187 come from
  calls answered as not ported (pinned by marker and count:
  CropCircleController 1, WeaponInterface 24, FriendsListInterface 2,
  LevelUp 1, FamiliarManager 3, QuestRepository 104, EmoteManager 6,
  LoginValidationPlugin 1, RegionManager.move 23, MusicPlayer.init 22); the
  91 entries before `reinitVarps` are equal in order, 56 byte for byte and 35
  counter-carrying interface packets by opcode and length (the skipped
  calls' packets advance the original's interface packet counter);
  `reinitVarps`' 16 native varps are all among the original's 116 (the
  others were set by the skipped content); nothing else follows.
- A walk three tiles east: three steps in the player updates, the client's
  position reaches the target.
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

Measured (2026-11-17, this machine): in-process debug test 23 ticks, mean
work 66.8 ms, maximum 1,507 ms (the first login: lazy definition loading in a
debug build), mean interval 641.5 ms. Release binary
(`rs09-server --world`, start-up 456 ms, 21,976 NPC rows spawned and not
joined), the ignored scenario against it, then idle: 199 ticks, mean work
1.55 ms, maximum 157 ms (the first login pulse), mean interval 600.3 ms.

The world-login, tick-driver and world-trace comparisons of the libraries
are unchanged (their tests bind the same libraries with replayed
stand-ins); this test checks the production binding end to end.

## Not covered / still to do

- Install the player tab systems of `world/player_tabs` (WeaponInterface,
  LevelUp, EmoteManager, MusicPlayer, quest tab sync; merged after this copy
  was made) in place of the skipped answers above.
- Bind the remaining routed handlers (`scenery_action`, `ground_action` /
  `ground_runtime`, `interface_packets`, `input_prompt`) with their
  owning-world contexts.
- NPCs: the start-up spawns them into its own repository and canonical
  state; joining them to the live repository (`Repository::renderable_npcs`),
  `NPC.tick`/`reset` (`world::npc_tick`) and the NPC renderer
  (`world::npc_render`) is not done; no NPC update is sent.
- Entity regions (`world::entity_regions`) and zones for local players and
  region moves; map chunk synchronisation and ground items.
- Reconnect, the lobby, AFK logout's client side, many players (capacity is
  not measured), concurrent saves of one character.
