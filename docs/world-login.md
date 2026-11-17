# Login into the world (`world/login`)

`crates/server/src/world/login.rs` and `world/login/` port what the original
does between an authenticated login block and a player standing in the world
with the client configured. It is a library and a test. Nothing installs it:
`main.rs`, the examples and the endpoint are unchanged, and the native server
still answers a valid login with "world unavailable".

| Original | Rust | Scope |
| --- | --- | --- |
| `LoginReadEvent.read` after authentication | `admission::read` | the `catch (Exception)` that prints and writes UnexpectedError |
| `Login.proceedWith` | `admission::proceed_with` | uid check, session and UIDInfo, flagged IPs, `new Player`, IP log, bypass or account limit |
| `canBypassAccountLimitCheck`, `checkAccountLimit` | `admission` | rights, exceptions file (one service), daily-accounts archive |
| `proceedWithAcceptableLogin`, `sendMSEvents` | `admission` | `Repository.addPlayer`, last ping, `initialize`, social `login_events`, catch and `session.disconnect` |
| `LoginParser.initialize`, `validateRequest`, `flag` | `admission::initialize`, `flag` | inactive session, updating, banned; `PlayerParser.parse` and its failure path; the pulse submission |
| LoginParser's `Pulse(1)`, `reconnect` | `pulse::run` | spawn location, login listeners, content hooks, session object, last used IP, Success, `Player.init`, `reinitVarps`; the `Throwable` recovery |
| `LoginWriteEvent.write` | `write_response` | reply bytes (`rs09_protocol::login::reply`), rights and index from `session.getPlayer()`, the game producer |
| `Player.init`, `Entity.init` | `configuration::player_init`, `entity_init` | log, session object, active, auto timers, lobby, `logged-in-fully` |
| `LoginConfiguration` | `configuration` | `configureLobby`, `sendLobbyScreen`, `configureGameWorld`, `sendGameConfiguration`, `welcome`, `config`, `getMessageChild`, `autoSelect`, `getLastLogin`; `SpellBookManager.update` |

It runs on the native repository (`addPlayer`, the uid map, `removePlayer`,
the lobby list, `DisconnectionQueue.remove`, `IoSession.disconnect` and
`getPlayer`), social (`Social::login_events`), the interface manager
(windows pane, chatbox, default tabs, info bars, close, interface config, tab
open), `PlayerDetails` and the login reply writer. Every other call is a
method of `login::Host`, called in the original order, with no default. The
host binds the canonical player's own libraries (containers and listeners,
skills, Settings, variables, the save file, interaction options, the rebuild
writer) behind those methods; content and systems without a port are reached
only through methods named for them: login listeners and login plugins (by
class name, as an input list in the original's registration order), content
hooks, timers, ServerStore, Discord, PlayerMonitor, FamiliarManager,
QuestRepository, EmoteManager, MusicPlayer, RegionManager, WeaponInterface,
interface listeners and component plugins, quest requirements, RulesAndInfo,
the update sequence's renderable list and update masks.

The world pulse that finishes a login is submitted through
`Host::submit_login_pulse`; the tick driver ([tick-driver.md](tick-driver.md))
runs `pulse::run` when its world pulse runner reaches it, one tick later.

## Packet origins: `oracle/world-login`

`cargo xtask world-login-oracle [--check]` (Python 3.10+, JDK 11, the pinned
cache) builds on `oracle/world-trace` as the start-up and canonical-player
oracles do: the same unchanged compiled server (3,487 files), the same pinned
call sites, world directory and scenario script. Its harness,
`oracle.worldtrace.LoginOrigins`, runs new-character, two-characters and
returning-character statement for statement as `WorldTrace.main` does; after
the boot record it replaces the two `LinkedList`s of the original's
`PacketWriteQueue` with lists that remember the stack of whoever queued each
packet, and attributes the entries a flush writes to the packet polled
before them. Entries written without the queue (the login reply, the
handshake, UpdateSceneGraph, `Settings.updateChatSettings`, the update
sequence) are attributed `direct`.

A run is accepted only if two fresh JVM passes agree byte for byte **and**
every record it writes for the three scenarios is byte-identical to the
same record of `fixtures/world-trace/trace.jsonl` (the boot records by
digest). They are: recording origins did not change the trace. The
world-trace generator, harness and fixture are unchanged.

`fixtures/world-login/origins.jsonl`: one line per record and session with
one origin per entry, then the table of 154 distinct stacks
(`class.method:line` from the caller of `PacketRepository.send` or
`PacketWriteQueue.push` down to the harness). 1,911 entries, 202 of them
direct. `provenance.json` holds the pins, hashes and counts.

## Verification

```powershell
cargo test -p rs09-server --test world_login
cargo xtask world-login-oracle --check --python <python> --java-home <jdk11>
```

`crates/server/tests/world_login.rs` drives the scripted input of each login
scenario through the native code: the recorded login block is decoded by
`rs09_protocol::login::decode` with the reference private key, authenticated
by the native development authenticator over `InMemoryStorage`, prepared by
`player::prepare` and admitted by `admission::read`; at the next tick
`pulse::run` finishes it. The configuration is the pinned `default.conf` with
the world-trace's one override; XTEA keys and interface configuration are
the pinned files (each checked against the world-trace provenance). For
returning-character the character file new-character wrote is loaded by the
canonical player.

What matches (all four logins: new-character, both characters of
two-characters, returning-character):

- **Every entry the original handed to the session during the login ticks,
  byte for byte and in order**: nothing at the login block's tick, then 395,
  395, 396 and 371 entries at the tick the pulse runs (tick 1, 1, 6 and 1).
- **206 of them are written by native code in each login**: the login reply,
  the rebuild (162) and the chat settings (232) written at once; the windows
  pane; the chatbox (three interfaces and varp 334); 24 tab openings over the
  two `openDefaultTabs` (tabs 1 to 6 and 9 to 14 in each); the welcome
  message; inventory and equipment container packets with their weight
  texts; the 24 skill levels; `Settings.update` (14 varps and run energy);
  the three player options; run energy; both `config` texts; the five info
  bars; the interface config of 226; the spell book's tab and varbit 357;
  and `reinitVarps` (116 varps, in the original's HashMap order, after a
  load for the returning character).
- **The repository** after the login block and after the login tick: slot,
  index (1, 1, 5 and 1), name, location, playing and active of every
  character.
- **Social**: the events `login_events` publishes (PlayerStatusUpdate,
  RequestContactInfo) are the first events the original handled.

### Stand-ins and the named gaps

Where the original reaches content or a system that is not ported, the test
binding replays the packets the original queued from that call: the
original's entries at the session's current queue position whose stack
contains the call's frame. It advances the interface packet counter by the
counter-carrying packets among them and puts the varps they carry into the
canonical variable store, without sending (what `setVarp` does besides
sending). This is a test input, not a port; the native packets that follow
each replayed segment are compared exactly, which checks the counter and the
varp map. The test pins every segment by name and size, and the number of
stand-in calls (61 per login); a gap that closes or a new difference fails
it.

| Segment | Entries | Why |
| --- | --- | --- |
| `ManagementEvents.handleEvent` | 2 (197, 126) | the handlers of the events social published; not joined in this test |
| login listeners | 1 (2 for tracetwo) | content: CropCircleController, ShootingStarPlugin messages |
| `WeaponInterface.open` (twice) | 9 + 9 | core, not ported: tab 0's interface, access masks and weapon name |
| `FriendsListInterface.open` | 2 | content component |
| `WeaponInterface` (equipment refresh) | 6 | core, not ported |
| `LevelUp.sendFlashingIcons` | 1 | core, not ported |
| `FamiliarManager.login` | 3 | content |
| `QuestRepository.syncronizeTab` | 104 | core repository with content `updateVarps` |
| `EmoteManager.refresh` | 6 (3 returning) | core, not ported |
| `LoginValidationPlugin` | 1 | content login plugin |
| `RegionManager.move` | 23 (2 returning) | entity regions and zones not joined; ZoneMonitor's music zone |
| `MusicPlayer.init` | 22 | core, not ported |

Other stand-ins answer state queries for systems that are not ported:
ServerStore's flagged IPs (empty), `SystemManager.isUpdating` (false),
`hasFamiliar` (false), `hasRequirement` (false: these characters have
started no quest and the original wrote no varbit 3909), and the varbit 357
definition, pinned as the cache defines it (varp 439, bits 0..1;
`pinned_varbit_matches_the_cache` checks it when `assets/cache` is present).
Set `RS09_WORLD_LOGIN_VERBOSE=1` to print the save differences field by field.
Auto timers, appearance synchronisation, the renderable list, update masks
and the content hooks' parse are recorded, not run.

Artificial players: the original's content spawned three AIPlayers during
tick 1 (into slots 2 to 4 through `NodeList.add`, not `addPlayer`). The test
puts stand-ins into the same slots of the native list before the second
login, which is why tracetwo's index is 5 natively too.

### Saves

The canonical player's save right after the native login is compared with
the original's save written at logout (tick 22, 28 for tracetwo: 21 or 22
ticks after the login tick). The top-level
keys that differ are pinned: for new characters `attributes`, `barCrawl`,
`core_data`, `emoteData`, `ge-history`, `globalData`, `slayer`,
`treasureTrails`, `unlockedMusic`; for the returning character `barCrawl`,
`core_data`, `ge-history`, `slayer`, `treasureTrails`. They come from content
hooks that write nothing here, attributes set by content and the wealth
check, the emote and music state of replayed calls, the AntiMacro timer
and a saved varp 442 set during the ticks, and the loot-share delay; the
original's file is written after ticks the native world does not run.

## Deviations from the original

- `checkAccountLimit`'s exceptions-file block (existence, reload, TOML
  lookup, its `catch`) is one service; the rest is ported.
- `LoginParser`'s `lateinit` parser is an `Option`; reaching it unset is the
  original's `UninitializedPropertyAccessException`, an `Exception`.
- `sendGameConfiguration` calls the interface primitives itself, in the
  order of `interfaces::send_game_configuration`, instead of that function's
  `LoginContext` callbacks.
- Writes at once and queued packets are the host's two channels; the
  library does not model `PacketWriteQueue`.
- Errors carry a category (`Exception`, `Throwable`, or `Native` for a
  native bound nothing catches) and text, not a Java stack trace.
- `colorize` is ported only for the two spell-book messages.

## Not covered

Reconnect (opcode 18), the lobby screen, the account limit, flagged IPs,
bans, updates, wrong passwords, duplicate logins, parse failures and the
`Throwable` recovery are ported but no world-trace scenario reaches them; the
handshake reply (its server key comes from the original's `Math.random`
position) and decoding are the transport's (login corpus). Nothing after the
login tick is compared here: the tick driver's test compares the later ticks
([tick-driver.md](tick-driver.md)).

## What the tick driver and the `--world` step need

- A world binding of `login::Host` (the test binding in
  `tests/world_login.rs` is its shape): the canonical State, repository,
  social, interface registry, XTEA keys, varbit definitions, sessions with
  two output channels (written at once; queued and flushed at the end of the
  tick, after the social queue's packets), the GameMessage and
  InteractionOption writers.
- The world pulse queue running `pulse::run` in `Pulser.updateAll`, one tick
  after the admission, and the flush after `Managers.tick`.
- The management-event pump (social handlers) between steps, as the
  world-trace harness pumps.
- The update sequence (renderable list, rebuild flag, appearance), entity
  regions and zones (`RegionManager.move`), timers (`addAutoTimers`),
  `DisconnectionQueue.update` and the 50-tick autosave.
- Ports or explicit services for WeaponInterface, LevelUp, QuestRepository,
  EmoteManager, MusicPlayer, FamiliarManager, interface listeners and
  content listeners before a native world can stand without stand-ins.
