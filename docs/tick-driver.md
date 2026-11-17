# The tick driver (`world/driver`)

`crates/server/src/world/driver/` is one native world tick:
`MajorUpdateWorker.handleTickActions` with the verified libraries installed in
its phases, the read event that queues incoming packets, `PacketProcessor`'s
per-type table, `PacketWriteQueue`, `GameWorld.pulse` and the logout path from
a closed connection. It is a library and a test. Nothing installs it: `main.rs`,
the examples and the endpoint are unchanged, and the native server still
answers a valid login with "world unavailable".

| Phase | Original | Installed |
| --- | --- | --- |
| packets | `PacketProcessor.processQueue` | `packet_queue::Queue` with `dispatch` (below) |
| disconnections | `Repository.disconnectionQueue.update()` | `disconnection::DisconnectionQueue::update` on the driver's `Repository` |
| pulses | `GameWorld.Pulser.updateAll()` | `pulses::Runner::update_all`; a pulse's body is `Host::pulse_body` |
| tick listeners | `GameWorld.tickListeners` | `Host::tick_listeners` |
| update sequence | `UpdateSequence.start/run/end` | `update::Sequence` over the repository's lobby and renderable NPC lists and the driver's `renderablePlayers`; `Player.update` (scene graph when flagged, player, NPC and map chunk renderers) and `Player.reset` (entity reset, scene flag, render information, the hitpoints `SkillLevel`, `flagged-for-save`, `opCounts`) are ported in the driver, their parts are services |
| world pulse | `GameWorld.pulse()` | `Ticks` (wrapping `Int`) and the `Autosave` hand-off when the counter is a multiple of 50; `Autosave::run` is the block (`Repository.players`' stream, `!isArtificial && isPlaying`, `disconnectionQueue.save(p, false)`, `catch (Throwable)`) |
| managers | `Managers.tick()` | `Host::managers_tick` |
| flush | `PacketWriteQueue.flush()` in `finally` | `write_queue::WriteQueue::flush` |

The order, the `catch (Exception)` around the phases and the flush in
`finally` are `world::tick::handle`'s (verified by the world-update oracle). An
`Exception` from a phase is printed and ends the phases; a `Throwable` escapes
after the flush. Errors keep their category through the libraries' own error
types (the adapters stash the driver error and return it).

- `Driver::read` is `IoReadEvent.run` with `GameReadEvent.read`: the reading
  queue (`queueBuffer`, the 100,000-byte room), `PACKET_SIZES`, the size
  byte or short, `size == -1` (unknown size or a missing size byte: the
  player's invalid packet count, `clear()` at five, the rest dropped), the
  header bytes written back for an incomplete packet, `setLastPing`, the
  authentic limit `opCounts[opcode]++ >= 10` in a `byte[255]` (it wraps; the
  limit returns from the read and drops the rest), and the three decode
  outcomes (`UnhandledOp` and `DecodingError` logged, `NoProcess` dropped).
- `dispatch::decode` is `Decoders530.process` (the 83 decoders, checked
  against the pinned enum's text): the numeric client actions decode through
  `rs09_protocol::game::ClientAction`, the social opcodes through
  `world::social::chat::decode`; COMMAND, QUICKCHAT and REPORT_ABUSE have no
  native decoder and become `Packet::Undecoded`.
- `dispatch::route` is the `when` of `PacketProcessor.process`, by `Packet`
  class (checked against the pinned `Packet.kt`):

| Route | Packet types |
| --- | --- |
| native handler (`Host::handle`) | `WorldspaceWalk`, `MinimapWalk`, `InteractWalk` (walk_request); `SceneryAction` (scenery_action); `GroundItemAction` (ground_action / ground_runtime); `IfAction` (interface_packets; bank, prayer tab and trade buttons are interface listeners behind it); `CloseIface`, `ContinueOption` (interface_packets); `InputPromptResponse` (input_prompt); `AddFriend`, `RemoveFriend`, `AddIgnore`, `RemoveIgnore`, `JoinClan`, `SetClanRank`, `KickFromClan`, `ChatMessage`, `ChatSetting`, `PrivateMessage` (social); `TrackingDisplayUpdate` (client info and `interfaces::switch_window_mode`) |
| ported here | `Ping` (last ping), `PacketCountUpdate`, `TrackingAfkTimeout` (`isAfkLogout`, `sendLogout`), and the original's empty branches: `TrackingFocus`, `TrackingCameraPos`, `TrackingMouseClick`, `PlayerPrefsUpdate` |
| refused, not ported (logged, recorded in `Driver::refusals`, never a success) | `ItemAction`, `NpcAction`, `PlayerAction` (player options and trade requests), the five `UseWith*`, the five `Component*Action` (spells, familiar specials), both `SlotSwitch*`, `ItemExamine`, `SceneryExamine`, `NpcExamine`, `GESetOfferItem`, `TrackFinished`, `Command`, `QuickChat`, `ReportAbuse` |

- `Driver::close` is `IoSession.disconnect` (then `Player.clear`: the queue
  and `details.save()`); the disconnection queue finishes the player at the
  next tick whose counter is a multiple of three (`allowRemoval`,
  `sendLogout` with the immediate `Logout` packet, `finishClear` with the
  logout events, `removePlayer`, the clan, `TaskExecutor.executeSQL` of the
  save), all in `world::disconnection`.
- `write_queue` is `PacketWriteQueue`: `UpdateSceneGraph`,
  `BuildDynamicScene`, `InstancedLocationUpdate`, `Logout` and
  `ClearRegionChunk` are sent at once, everything else queued; a null context
  is logged and dropped; `flush` swaps the lists under the lock and writes in
  order with one `StringWriter` per flush; a non-`Exception` stops it with the
  rest still queued.

Not ported: the worker thread and its 600 ms sleep, `Server.heartbeat`, the
20-second ping timeout, the inactive-player failsafe, the daily restart and
store reset, `tickOffline`, Grafana itself (telemetry is a service), and the
bodies of `Player.tick`, `NPC.tick`, `NPC.reset`, the three renderers' world
queries, chunk reset and the ground item pulse.

## Verification against the world trace

```powershell
cargo test -p rs09-server --test world_login tick_driver
cargo test -p rs09-server --test tick_driver
```

`crates/server/tests/world_login/tick_driver.rs` (a child module of the
world-login test, on its binding, unchanged) drives the scripted steps of
new-character, two-characters and returning-character: each `recv` through the
native login, each `tick` as one `Driver::tick` (the login pulse is a
`Pulse(1)` in the native runner), each `close` as `Driver::close`, and after
every step the hand-offs the harness pumps (`SaveTask::run`, the autosave).
No new oracle was needed: the per-entry origins of `fixtures/world-login`
cover every tick of the three scenarios.

Compared exactly, at every step: every entry each session received, bytes and
dropped flag, in order (1,907 entries over four sessions); the repository's
characters (slot, index, name, location, active, playing, renderable, queued
for disconnection), the lobby and the tick counter (95 steps: every step but
the handshakes); the management
events published; the files written. Result lines of the run:

```text
test tick_driver::tick_driver_runs_the_world_trace_scenarios ... ok
test tick_driver::tick_driver_hands_off_the_autosave_at_tick_fifty ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

| Session | Entries | Native, equal | Replayed | Differ |
| --- | --- | --- | --- | --- |
| new-character traceone | 482 | 303 | 177 | 2 |
| two-characters traceone | 482 | 300 | 177 | 5 |
| two-characters tracetwo | 485 | 304 | 179 | 2 |
| returning-character traceone | 458 | 279 | 177 | 2 |

Natively written beyond the login's own (282 for a new character, 258 for
the returning one, with the player-tab systems of
[player-tabs.md](player-tabs.md)): the rebuild of the first
update after login (`Player.update`'s scene graph), every idle player update
(225) from the native renderer (including the two characters seeing each
other, `e100020080`, and tracetwo removing the logged-out traceone,
`e1000200f0`), and the `Logout` packet (86) the disconnection queue writes at
once to the closed session (dropped, as the original's).

Replayed (stand-ins; every segment pinned by marker and size, per session):
the login's segments of [world-login.md](world-login.md); `NPCRenderer.render`
(one NPC update, 32, per player update: the NPC world is not joined);
`MapChunkRenderer.render` (the two chunk updates, 230, of the first update);
what `Player.tick` queued (walking's region move and its music, four entries);
the content world pulses (37 entries: tutorial island's `TutorialLoginCheck`
two ticks after login, and the stage dialogue its `runTask` opens ten ticks
later), replayed at the end of the pulse phase.

The eleven natively written entries that differ are all player updates
(225), pinned by tick and position: the first update after the login
(the original carries the appearance block `updateAppearance` registered at
login; update masks and the appearance writer are not joined in this binding),
the next one (tutorial content teleported the player), and traceone's updates
at ticks 6 to 8 of two-characters (the original adds tracetwo with its
appearance and then sends its placement and teleport masks).

Also pinned: the saves written at logout differ from the original's in the
top-level keys world-login.md lists (content hooks, content attributes and the
wealth check, the AntiMacro timer, varp 442,
the loot-share delay) and nothing else; the published events equal the
original's except `SendContactInfo` at each login (its handler is not joined);
the native pulse runner is empty after every tick (the original's holds 14 to
50 content pulses); the 21 kinds of stand-in calls (tick listeners, managers,
chunk reset, ground items, `Player.tick`, the seven logout listeners by class
name, `DeathTask.isDead`, the soft-script check, familiar, walking reset,
zones, house, wealth, `Entity.clear`'s chunk and pulse manager parts, the
player's logout listeners); the log lines; no refusal and no printed error.
A gap that closes or a new difference fails the test.

The world trace ends before tick 50: `tick_driver_hands_off_the_autosave_at_tick_fifty`
ticks the native world (without client input or originals) instead of the
close and checks the hand-off at 50 and the native save it writes. It checks
the native code, not the original.

`crates/server/tests/tick_driver.rs` checks what no scenario reaches against
the pinned sources' text and bodies (there is no JVM corpus for them): the
decoder table, every route, framing, queueing and the wrapping per-tick limit,
the write queue, the phase order with catch and finally, the counter's wrap
and the autosave block.

## Deviations from the original

- Decoding errors carry the native error text, not a JVM stack trace; the
  three decoders without a port are refused at dispatch, not decoded.
- Refused packets are logged and recorded; the original would run content.
- `Entity.update` (empty) is not called.
- The reading queue's 100,000-byte room is checked as a length.
- The flush's log names the class as `core.net.packet.out.<simple name>`.

## Not covered

Any client input (the scenarios send none), so the read event, the dispatch
table and the refusals are checked only by the unit tests; the lobby; NPCs
(none are in this world); a death, combat or a soft script at logout; a forced
logout after the queue's wait; the autosave against the original; the real-time
loop.

## What the `--world` step needs

- A production `driver::Host` and `repository::Host` over the canonical world
  (the test binding's shape): sessions with a written-at-once channel and the
  flushed queue; the logout path as the test binds it (native `Player.clear`,
  the driver's renderable list, `Logout` through the write queue,
  `details.save()` into the account storage, `SaveTask` and `Autosave` on an
  off-tick executor, `save_immediately` through the canonical save file).
- The login binding (`login::Host`) of world-login.md, with
  `renderable_players_add` on `Driver::renderable_players` and
  `submit_login_pulse` into `Driver::pulser`.
- A 600 ms loop calling `Driver::tick`, with the worker loop's checks (ping
  timeout, inactive failsafe, daily restart) and `Server.heartbeat`; the game
  producer feeding `Driver::read`; end of stream calling `Driver::close`; the
  management-event pump.
- Bindings of every `dispatch::Handler` to its library on the canonical
  player.
- The native start-up's world (NPCs, regions, definitions) with NPC tick and
  render, map chunk render, chunk reset, the ground item pulse, entity regions
  for the renderer's candidates, update masks and the appearance writer.
- Ports or explicit services for `Player.tick`'s systems (walking, music,
  prayer, timers), tick listeners, managers and the content the login and the
  tutorial reach.
