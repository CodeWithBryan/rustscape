# Player walking through the native session

The player-walk assembly joins real native walk requests, MovementPulse,
PulseManager/PulseRunner, destination dispatch, SmartPathfinder, WalkingQueue and
the Entity.tick sequence. A separate live probe supplies pinned cache collision
and original player/rebuild encoders. The shared headless client can send an
absolute destination and report authoritative positions without graphics.

`walk_request.rs` preserves the original handler's lock/interface/dialogue order,
strict distance test, floored vector truncation and plane-zero transform. An
interaction-walk packet changes the queue's running flag and returns; it does
not start movement toward its coordinates. Worldspace/minimap walks clear facing,
reset scripts, construct a movement pulse and schedule it. Arrival captures the
packet's original player and run flag independently of later pulse-field changes.

`tick.rs` ports MajorUpdateWorker.handleTickActions and Entity.tick order. Major
phases process packets and disconnects, then pulses/listeners, sequence start/run/
end, the global tick and manager plugins. A failure stops the remaining body, but
the write flush still runs. Entity phases run scripts/events/skills, copy the old
location, advance walking, run post-movement scripts/timers and prepare masks.
These kernels keep concrete content services explicit. Native repository membership
and update orchestration are described in [world-update.md](world-update.md).

`player_walk.rs` owns canonical queues, stable node identities, current movement
handles, original pulse slots and a reused search workspace. It dispatches base
pulses by an indexed identity map. Collection follows connected players, runner
entries, current/manager/check references, externally retained handles and explicit
node leases. Retired walk targets and disconnected cycles are reclaimed; a reused
network player index cannot change an old pulse's mover. Raw IDs are owner-scoped;
content retaining a node independently uses retain_node. The native strong-count
helpers support ownership accounting, not Java-visible gameplay values. Collection
requires the owning world to serialize graph mutation. Recursive entry into the
same smart-search workspace remains explicitly unsupported.

The JVM gates cover 978 walk-handler scenarios, 141 major/entity tick scenarios
and 190 joined player-walk scenarios. The joined gate compiles the unchanged
MovementPulse, DestinationFlag, PulseManager, PulseType, PulseRunner,
SmartPathfinder, WalkingQueue, Path, Point, Direction, Vector and Option, with verbatim
walk-handler, Entity.tick, dispatch/reach/truncation bodies. Mutable player metadata,
lock/interface/dialogue/script/region/zone/timer/skill/mask/packet/clock/telemetry
services remain explicit. Rust tests compare state and callbacks after every
command, including replacement, runner prefixes, reentrant requests, energy,
arrival and failures. Additional regressions exercise 512 replacements, retained
handle lifetimes, wire-index reuse, walk/run updates and scene rebasing through
the shared player decoder. The absolute-walk writer reproduces four canonical
original decoder inputs from the existing gameplay-packet corpus.

The 79 added scenery scenarios use canonical retained Scenery targets and the
same native movement/queue/scheduler assembly. Shape/rotation/definition geometry
feeds original object destination and path dispatch. Arrival retains its original
player, target and option independently of mutable movement fields. The handler
used for routing is captured at construction; arrival sees later mutations to
the selected option's handler. Four additional native regressions cover retained
captures, duplicate coordinates, 192 replacement/collection cycles and foreign
world scheduling. Content/door destinations, permitted tiles and the legacy
arrival body remain explicit services. See [scenery-actions.md](scenery-actions.md)
for the exact scope and remaining live door work.

```powershell
cargo test -p rs09-server --test walk_request --test tick --test player_walk
cargo test -p rs09-server --example movement_session
cargo xtask walk-request-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
cargo xtask tick-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
cargo xtask player-walk-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11

cargo run -p rs09-server --example movement_session -- --keys runs/player-session/xteas.json
cargo run -p rs09-headless -- --world --address 127.0.0.1:43600 --username walker --password testing --rounds 12 --walk 3211,3215
cargo run -p rs09-headless -- --world --websocket --address 127.0.0.1:43601 --username runner --password testing --rounds 12 --walk 3211,3215 --run
```

The controlled probe listens separately on TCP 43600 / WebSocket 43601; ordinary
and animation/audio endpoints keep their existing behavior. It loads 42,005 raw
scenery definitions and 17 initial native cache regions. A bounded shared ingress
queue uses the original two-buffer packet rules; disconnects retain their immutable
node token. Deferred repository membership activates new players at the end of
the update sequence. Network/auth work uses the existing native runtime. The owner
runs the original major and update sequence kernels every 600ms, prepares 162/225 updates and
forwards energy/minimap/message packets. The shared wire renderer is reused from
player_session. Windows runs a copied executable under runs/movement-probe/bin
so Cargo can relink its target safely.

Live TCP and WebSocket traces are recorded under runs/movement-probe. Reachable
walk/run destinations are reached, and actual cache obstacles can produce the
original near result and message. These are controlled player-walking sessions,
with empty interfaces, scripts, event hooks, skill actions, timers, zones and
content plugins. Original complete Player initialization/extensions, profiles,
complete repository/world transitions, region activity/unloading, full packet dispatch
and content remain pending. The shared packet queue supports original NoProcess
semantics. Original network input filters that marker before enqueueing; decoder
admission and complete dispatch remain separate work.
NPC/scenery actions and interactions, remaining
finders, Path.walk, persistence, parallel replay and measured performance/capacity
are still required for the complete replacement. Default production startup
continues to require an explicit ready world.
