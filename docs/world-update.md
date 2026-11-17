# Packet processing and entity update lifecycle

The native world now has the original PacketProcessor two-buffer queue,
InitializingNodeList deferred admission/removal behavior and UpdateSequence
start/run/end orchestration. The controlled movement_session binds these to the
existing native walking, renderer and 600ms major tick kernel. Complete Player,
NPC, region activity, content and profile initialization remain in progress.

Packet producers append to the next buffer under a short shared lock. Processing
swaps the buffers, releases the lock and invokes handlers. New arrivals wait for
the next pass. Nested processing operates on the same shared buffers, including
the original leftover-buffer behavior after a logging failure. NoProcess clears
both global buffers. One accumulated error trace belongs to each invocation;
ordinary handler errors are logged and processing continues. Logging errors can
escape to the major tick's catch/finally path. The exception text is a host service;
Rust runtime errors do not fabricate JVM stack frames. Original GameReadEvent
filters decoder-produced NoProcess before enqueueing; ordinary incoming packets
must not trigger the global-clear branch. Complete decoder admission, per-opcode
limits, text/social actions and packet dispatch remain separate work.

Initializing list add/remove checks the current list, not the pending queue.
Two adds before synchronization can therefore add the same node twice, and a
remove before the first add is synchronized returns false. Sync consumes the
live queue, including entries produced by initialization callbacks. It sets
active before appending and renderable after appending. Removal attempts the
list mutation before setting renderable false, even if that node was already
removed. Callback failures leave the original partial mutations in place.
Native lists retain identity and support inherited indexed mutations. Player and
lobby iteration uses Kotlin iterator live hasNext size and modCount checks;
it does not use ArrayList.forEach's different final check. NPC iteration retains
the current CopyOnWriteArrayList snapshot. Later NPC iterations see changes to
the same list handle. Ordinary lobby/NPC add/remove applies immediately and
allows duplicate adds; only initializing player membership is deferred.

Sequence.start captures lobby, player and NPC list handles, clears lobby minimap
flags, ticks captured NPCs and then ticks renderable players. Run renders players.
End resets captured players/NPCs, synchronizes player membership, resets dirty
chunks and pulses ground items. Clock differences wrap to Java int telemetry.
The major kernel then advances the global tick and ticks managers; output flush
still runs after an earlier failure. Captured list aliases and mutations during
callbacks are part of the differential corpus.

The unchanged original list/entry/sequence classes and verbatim packet/major tick
methods produce 257 scenarios. Every command compares list contents, pending
entries, node flags, captured aliases, clocks, telemetry, packet buffers, callback
order, results and partial failures. Concrete Node/Player/NPC methods, dispatch,
exception text, output, chunks and ground items are explicit oracle services.
Native tests additionally cover four concurrent producers, 4,000 packet entries,
2,048 deferred initializations and budget release when NoProcess clears both
buffers. These producer tests do not establish parallel gameplay or capacity.

The movement probe now bounds its shared queued/processing frame entries with 4,096
permits, in addition to per-session input and independent bounded output. Stable
node tokens prevent a late frame or close from affecting a new player reusing a
wire index. Player node leases survive deferred removal and failed tick bodies.
Cancelled admissions retire during their first sync without leaving ghost players.
Portable example tests check first-tick admission, next-tick rendering, cancellation,
stale ingress and release of world owners. The test scene archive is unused by
those lifecycle cases; separate live probes use real pinned collision assets.

```powershell
cargo test -p rs09-server --test world_update
cargo test -p rs09-server --example movement_session
cargo xtask world-update-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

Lobby and NPC lists, chunks and ground items are empty services in this controlled
probe. Player tick/update/reset bind only the existing walking and wire-rendering
subset. Complete Player extensions, lobby/world transitions, disconnect policy,
repository UID/name/index management, NPC/content, region activity/unloading,
persistence, parallel replay and measured performance/capacity remain required.
The default production service still requires an explicitly ready native world.
