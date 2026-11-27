# Command sets, the system manager and their packets

Native bodies for three of the `core.game.system.command.sets` classes, the
system manager the `::update`, `::cancelupdate` and `::shutdown` commands
reach, and five outgoing packet writers. The libraries are
`world::commands::sets` (`misc`, `spawn`, `stats`), `world::system_manager`
and `world::command_packets`. The registry (`world::commands::registration`)
now gives every define of `MiscCommandSet`, `SpawnCommandSet` and
`StatsCommandSet` a native body (`NativeBody::Set`); dispatch still goes only
through the registry (`commands::run` hands the body the world as
`commands::Host::sets`). `rs09-server --world` routes commands and answers
`sets::Host` (world/live/commands.rs); the system manager is not installed
there, so `::update`, `::cancelupdate` and `::shutdown` are refused, and the
other refusals are listed in docs/live-world.md.

## What is ported

| Original | Native |
| --- | --- |
| `MiscCommandSet`: all 49 commands (debug, calcaccuracy, anmacs, setsaveversion, loc, pos, coords, calcmaxhit, calcdefence, empty, announce, players, botinfo, shop, ge, commands, commandsearch, reply, xface, completediaries, permadeath, log, logdone, rolltrawlerloot, fillbank, emptybank, setconfig, getobjectvarp, define_varbit, setvarbit, setvarbits, getvarbit, setvarp, setvarc, grow, finishbins, resetbins, diseasecrops, addcredits, getnpcparent, infinitespecial, infinitespecialmove, allowaggro, rules, confirmrules, setplayerstrong, setplaqueread, brewkit, brew), `resolveBooleanToggle`, `setBooleanAttribute`, `setupScrollInterface`, `displayCommandInScroll`, `chunkText`, `setPlaqueReadStatus`, `showGeBuy/Sell/Bots/Botsearch`, `showOffers`, `showGeInputDialogue`, `showGeBook` and its `display` callback, the reply and GE input handlers, the getnpcparent coroutine | `commands::sets::misc` |
| `SpawnCommandSet` (npc, item, items, object, objectgrid, removeobject, objects) | `commands::sets::spawn` |
| `StatsCommandSet` (stats) and its book `display(player, pageNum, buttonId)` | `commands::sets::stats` |
| `Player.toggleDebug`, `Rights.getChatIcon`, `Scenery.toString`, the Location members the sets read | inside the sets |
| `SystemManager` (state, `flag`, the `is*` queries), `SystemUpdate` (`setCountdown`, `schedule`, `notifyPlayers`, `pulse`, `cancel`, the backup flag, `Pulse.update` for it), `SystemTermination` (`terminate`, `save`), `SystemShutdownHook.run`, `SystemConfig` | `world::system_manager` |
| `SystemUpdatePacket`, `CameraViewPacket` (POSITION, ROTATION, SET, SHAKE, RESET), `CSConfigPacket`, `UpdateAreaPosition` (`getBuffer`, `getChunkUpdateBuffer`), `AnimateObjectPacket` | `world::command_packets` |

Joins to native libraries: page indices and the command walks are the native
`Registry` (`CommandMapping`); the GE views' maps are
`world::repository::hash_map::JavaHashMap` (Java `HashMap` order);
`formatDisplayName` is `world::social::names`; `colorize`, `reject`,
`notify`, rights and the Kotlin parsers are `world::commands`; the updater's
fields are a native `pulses::Pulse` handle; `UpdateAreaPosition` is the
native `zone_origin` frame; the ::update/::cancelupdate/::shutdown bodies
(`world::commands::system`) reach `system_manager::Manager` through the
commands host's `set_update_countdown`, `flag_updating` and `cancel_update`
(the replay binds them so).

Every other call is a required `sets::Host` or `system_manager::Host`
method with no default: attributes (the host keeps GameAttributes' key
rules), interfaces, packets the sets send (strings, varc, interface config),
varps/varbits, containers, definitions, regions, chunks and scenery
construction, NPC construction/configuration/spawn, the combat calculations
(world/combat_swing is native but not bound to a session), the Grand
Exchange offer queries, books, rules text, private messages, news, timers
(farming, compost, brewing), the cutscene, diaries, permadeath, trawler
loot, slayer/clue/quest/kill/drop counters, bots, the clipboard, standard
output, the clock; for the manager: the tick worker, the pulse runner, the
repository's player list, the system update packet, networking (running
flag, reactor, websocket), bots, shutdown listeners, world stores,
PlayerMonitor, the disconnection queue, the file system and `Thread.sleep`.

## Evidence

`cargo xtask command-sets-oracle --check` (Python, JDK 11) reproduces
fixtures/command-sets/: Command, CommandSystem, CommandPlugin, the legacy
CommandSet enum, Privilege, sets.CommandSet, the Misc, Spawn, Stats and
System command sets, StatAttributeKeys, SystemManager, SystemUpdate,
SystemTermination, SystemShutdownHook, SystemConfig, SystemState, Pulse,
GameAttributes, Rights, UserAccountInfo, StringUtils, IoBuffer and the five
packet writers with their contexts are compiled unchanged (the inserted
original texts are listed in oracle/command-sets/generate.py) over
recording adapters, each case in a fresh class loader, headless
(`java.awt.headless=true`) under a security manager that turns
`System.exit` into an outcome; wall clock reads are case inputs; `println`
and `printStackTrace` are recorded; the working directory is written
`<work>`. Every recorded call of the marked operations is then made to fail
once in turn (long scroll loops: their first twelve and last five calls).
691 cases (156 designed, 535 injected failures), 1,270 operations and 26,811
recorded calls drive all 60 commands (the 57 ported here and the three
system paths), the manager's every transition and the five writers' bytes;
the counts are in fixtures/command-sets/provenance.json.

`cargo test -p rs09-server --test command_sets` replays every case natively
and compares every recorded call, result, error and the state after each
operation (accounts, every player's attributes with the saved-key list,
version, test stage and plaques, the manager's state and updater fields,
the tick worker, pending pulses/inputs/jobs/books, directories); it checks
that every define of the three sets has a native body.

The comparison is not vacuous: 16 deliberate breaks of the port were tried
(the hit-chance comparison, the BigDecimal rounding, the next-page number,
the NPC amount limit, the easy-clue index, the backup countdown, the
ServerStore saving last, the "/save:" strip on removal, chunkText's break
character, the chat icon offset, the 5000 ms save limit, the CS config id
transform, confirmrules' dialogue before unlock, notifyPlayers' single
catch, showOffers' inclusive line loop, the static-object slots). The replay
failed for 14 of them (the save limit by never ending, as the original
would not). The hit-chance comparison was unobserved: accuracy equal to
defence gives the same value in exact arithmetic, so a case with both 2
(where the two double formulas differ in the last bit) was added and the
break then failed. showOffers' inclusive bound is unobservable (its extra
iteration always finds no selling entry and continues).

## Differences and limits

- `getItemName` is a definition-table read: answered by the case, not
  recorded (its call count follows the sort, which a stable native sort does
  not reproduce call for call; the order of the result is the same).
- The clipboard is a host service; the corpus runs headless, so `loc`
  rejects with its NOTE and `logdone`/`npc` stop with `HeadlessException`
  (`sets::HEADLESS`), as on a server. A desktop clipboard is not driven.
- Kotlin's `equals/contains(ignoreCase = true)` use single-character case
  mappings; the command line only holds U+0001..U+007F and U+FF80..U+FFFF.
- `SystemConfig.split` matches its regex literally and refuses one with a
  metacharacter (no caller passes one); `parseConfig` is private and never
  called, so the config maps stay empty as in the original.
- `SystemUpdate.schedule`'s own thread (when the tick worker is not running)
  is `system_manager::Host::start_update_thread` and is not driven (it would
  race the case); `::shutdown` is `commands::Host::exit_process`, which a
  world must answer by running `Manager::shutdown_hook` (the JVM's
  `ServerConstants.SHUTDOWN_HOOK`) and exiting.
- The packet writers return frames with the plain opcode; the session's ISAAC
  stream is the transport's. The interface manager's packet counter is read
  (and advanced) by the caller.
- Not ported here: QuestCommandSet, SlayerCommandSet, the development sets
  (Development, Fun, Animation, Audio, Camera, CameraViewer, Config, If,
  ModelViewer, Botting), MapDumpCommand, `oldsys/` and `rottenpotato/`, and
  DisplayModel; their commands stay content services (docs/commands.md).
