# Doors in the live world (`world/live/items/doors.rs`)

Scope: the pinned `DoorManagingPlugin` and `DoorActionHandler`, already
ported as `world::door_option` and `world::doors` (docs/doors.md, corpus
`fixtures/doors`), bound to the live world. docs/live-doors.md describes
the older controlled probe (`npc_session`), not this binding.

| original | here |
|---|---|
| `DoorManagingPlugin.newInstance` (the default handler of "open", "close", "shut", "go-through") | `Live::install_doors`, when the class is in the scan |
| `DoorManagingPlugin.handle` | `Live::door_handle` over `door_option::handle` |
| `DoorManagingPlugin.getDestination` | `SceneryPlugin::Door` in the walk host's `destination`, over `doors::destination` |
| the two anonymous pulses of `DoorActionHandler` | world pulses, `Live::door_pulse_body` over `doors::pulse_body` |
| `player.logoutListeners["autowalk"]` | `DoorWorld::logout`, run by `Live::door_logout_listener` at logout |

`Opener` is the context of both libraries: the start-up's door
configurations, the cache scene (`RegionManager.getObject`,
`Scenery.transform`, `SceneryBuilder.replace` with its restore pulses), the
player's location, lock, synth sounds, messages, animator and teleport
location (trapdoors), `Pathfinder.find(..).isMoveNear()` and the walking
queue's `reset` and `addPath`, which do not ask the lock
(`player_walk::World::queue_add`).

## Options that belong to other content

DoorManagingPlugin is only the default. In the original a content listener
for the scenery id, a `PluginInteraction` for the id or a handler on the
scenery's definition takes the option first, and most of that content is
not installed in this world. Opening such scenery as an ordinary door
would give a player a quest door or a toll gate for free, so:

- `fixtures/door-claims/claims.txt` lists every scenery id and option pair
  (626 rows: 203 listeners, 331 definition handlers, 92 rows for 23
  `PluginInteraction` ids) that the booted original registered for the four
  option names. It is recorded by `oracle/door-claims/generate.py` (`cargo
  xtask door-claims-oracle`), which boots the whole unmodified server as
  oracle/world-trace compiles it and reads the registries by reflection;
  two fresh JVM runs must agree.
- `world/door_claims.txt` is that file, embedded in the server
  (`world::door_claims`); `cargo test -p rs09-server --test door_claims`
  checks the two are byte for byte the same.
- `Live::door_handle` refuses a listed pair by name (a live log line, no
  message to the player) before the door rules run. A listed pair whose
  content this world does install never reaches the refusal: a listener
  runs before any option handler, and a handler on a definition (the
  thieving chests) wins over the default in the option registry.

What the list does not establish (see `scope` in
`fixtures/door-claims/provenance.json`): what the content does, anything
registered after start-up, scenery that reaches an option through a morph
(child id), and zone or region restrictions (`ZoneMonitor.interact`),
which are not registrations. The generator is not in `oracle/ci.toml`: CI
compares the embedded table with the committed fixture but does not boot
the original.

## Not ported, answered by name

- `AchievementDiaryManager.finishTask` for doors 2112, 35549, 35551 and
  2406: a named skip.
- No NPC opens a door: every entity of this binding is a player.

## Kept behaviour worth knowing

- An opened door is the configured replacement for 500 ticks, then the
  closed door again; "close" swaps it back at once.
- A scenery whose name has "door", "gate", "fence", "wall", "exit" or
  "entrance" and no door configuration is walked through
  (`handleAutowalkDoor`): a four tick lock, the open and (delayed) close
  sounds, the door open for two ticks and a step to the other side. A
  player who logs out inside it is put back where the walk started.
- Drawers, wardrobes and cupboards become the next id for 80 ticks;
  a trapdoor teleports 6400 tiles north if that tile permits it, else
  "This doesn't seem to go anywhere."
- Any other scenery with one of the four options answers "Nothing
  interesting happens." (the handler's false).

## Evidence, and what it does not show

- The libraries are replayed against `fixtures/doors` (docs/doors.md). The
  live binding has no JVM corpus.
- The live world's `gather` scenario (`doorman`): the closed Lumbridge
  door 36846 at 3234,3207 blocks a walk to the tile behind it; "Open"
  lets the walk through; "Close" on the open door 36848 blocks the direct
  two-tile walk again (the house has a second door, so the walk goes
  around); a second "Open" from inside lets the player straight out. Run
  in the cloud, it passes.
- No live check covers a walk-through door, a double door, a gate or
  fence, the sounds, the 500 tick restore, drawers, wardrobes, cupboards,
  trapdoors, a refused pair, a logout inside a door or a second viewer.
