# Ground items in the owning world

`world::ground_world` joins the ported ground item, manager, spawn and loader
implementations to the existing canonical `player_walk::State` and `Scene`.
`Parts::new` rejects actors and scenes backed by different global pulse runners.
Actor handles are checked by retained identity before profile or location access;
an equal numeric node from another world is not accepted.

The assembly uses the real Scene chunk item lists and dirty tracker. It renders
each player's retained 13 by 13 viewport with the original MapChunkRenderer
traversal, serializes chunk flags through chunk_sync, and sends typed protocol
frames. This includes items more than 15 tiles away inside the 104 tile build
area. Ground-item publication and the player's local character list have different
visibility rules. Private updates preserve the adjacent opcode26 and item frames
in one batch. Scene flags are reset after all viewers, and the manager pulse runs
after reset, matching UpdateSequence.end. Respawns enter the same fixed-prefix
runner as actor and scenery tasks.

The host supplies account identity, item interactions/plugins, player state,
transport, retained viewports, region activity and logging. These are mandatory
service contracts, not fallback game implementations. Accounts remain available
while a disconnected actor is retained by a ground item, flag or exposed Item
view. Profile metadata itself must not retain a canonical actor lease. Cached UID
matching can rebind the dropper to a new canonical player and release the old one.

The movement_session and npc_session examples opt into this assembly with
`--ground-spawns FILE --item-config FILE`. They load the actual native strict item
definitions and original item configuration, then the pinned177row/717spawn file.
Their real player render/rebuild handles, account metadata, actor repository,
Scene and packet queues are shared with the ground services. Previously running
endpoints keep their original binaries and behavior.

These examples still use explicit fixture boundaries: no registered item plugins,
no accepted Take/Drop actions, and resident
regions without inactivity transitions. They do not establish complete Player
initialization, saved inventories, item/content dispatch, region activity,
production startup or player capacity. Native visibility/scheduling integration
does not make those services complete.

ClearRegionChunk.send is separately ported to `chunk_sync::clear`. The unchanged
complete original class and ClearChunkContext generate136 JVM observations for
104tile boundaries, retained last-scene coordinates, live chunk bases, callback
reentry and failed prefixes. The native replay also reruns all prior chunk and
ground corpora; nullable scene state is represented explicitly in its test adapter.

The example regressions send actual typed frames through the native writer and a
byte-at-a-time decoder into the shared client ground reducer. They cover private
adds/increases, owner exclusion on public publication, expiry, two-viewer flag
lifetime, distant visible chunks, scene rebases, plane changes, clear-before-sync,
shared-runner respawns, foreign-world rejection and disconnected-owner release.
Synthetic test metadata and height zero are scoped decoder inputs; they do not
prove real terrain placement, live inventory pickup or render fidelity.

```powershell
cargo test -p rs09-server --test clear_chunk --example movement_session --example npc_session
cargo xtask clear-chunk-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
cargo run --release -p rs09-server --example npc_session -- --keys runs/player-session/xteas.json --tcp 127.0.0.1:43614 --websocket 127.0.0.1:43615 --gameframe --ground-spawns fixtures/ground-spawn-loader/ground_spawns.json --item-config runs/item-config-oracle/item_configs.json
```

The configuration and keys in runs are local pinned inputs, not assumed present
in a fresh checkout. Ports must be free before starting an isolated live probe.

