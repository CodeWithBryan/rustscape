# One world pulse phase

The canonical player_walk state and native Scene can now share one PulseRunner.
State::with_runner attaches the queue at construction; State::runner exposes its
shared handle. The controlled movement/NPC probe constructs its state from the
scene's queue. Existing running binaries are unchanged.

The owner calls state.context(host).pulses once per original pulse phase. The
movement bridge dispatches movement and ordinary interaction tasks, and delegates
foreign pulse bodies, inactive checks and stops to their owning domain. Scene's
pulse_body, pulse_inactive and stop_pulse reject another scene's pulse identity.
Scenery checks use their scenery identities, not the canonical actor node IDs.
Base update, exception handling, stopped-state checks, timing and telemetry stay
in the common runner/bridge. An inactive-check failure propagates to the runner;
a body failure is caught by the original base Pulse update.

Scene.collect_pulses reaps expired task/check registrations after that phase. It
does not update the queue, tick a restore again or clear chunk flags. Do not also
call the standalone Scene::World.tick when the global owner drives this queue.
Foreign tasks that capture canonical actor IDs must retain their node leases for
their observable lifetime; numeric foreign checks are never interpreted as actor
IDs. Concrete profile/door services still need those bindings.

The mixed JVM harness compiles unchanged complete SceneryBuilder and Constructed,
InteractPlugin/Option/OptionHandler, Pulse/manager/runner, movement, pathfinding
and walking code with original Entity.tick. Its 61 scenarios and 794 command
snapshots include restores submitted before/after player tasks or inside a
handler, zero/one/multiple/500-tick delays, ground callbacks submitting player
actions, stopped/restored/inactive objects and check/body exceptions. The JVM's
landscape/flag environment is a single-cell adapter that preserves original
chunk.add constructed-copy behavior. Native scenery uses the real native
region/landscape/collision stack with pinned definition samples and synthetic
terrain/land data. Profile, handler, ground and flag observations are explicit.

Three native regressions verify shared queue identity and one update per phase,
scenery completion after disconnected actor graphs are collected, and foreign
scene-pulse rejection without mutating it. Both controlled example targets also
verify restoration through their actual global pulse callback. The prior 190
player-walk, 323 interaction-runtime and 37 scene expected records stay byte for
byte unchanged; only the factored interaction test setup's provenance changes.

```powershell
cargo test -p rs09-server --test world_pulses --test interaction_runtime --test player_walk --test scene --example movement_session --example npc_session
cargo xtask world-pulses-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

This connects native task domains and the controlled probe's owner phase. Live
object-option registries/profile locks/face masks, full MapChunkRenderer viewport
retention/synchronization, chunk-flag fanout and content handlers remain pending.
No door ID or immediate click shortcut has been added. Full gameplay/client/
headless parity, deterministic parallel simulation and measured performance/
capacity acceptance remain open; source inventory statuses stay not_replaced.
