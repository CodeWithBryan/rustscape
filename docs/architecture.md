# Native replacement architecture and acceptance

## Reference boundary

The locked server and RT4 sources define the target. Configuration is part of
that contract: this pair disables opcode ISAAC by default, sends an extra
cache-index CRC, and has a 22050 Hz default audio rate. Although the client sets
LOGIN_USE_STRINGS=true, its actual LoginManager still writes the username as a
Base37 long; the server reads that format. The flag does not establish string
login behavior. Do not
replace these defaults with assumptions from a different RuneScape revision.

The migration covers Kotlin as well as Java. The existing parent LostCity/289
workload is a separate implementation and cannot serve as this revision's
gameplay oracle.

## Components

- protocol owns packet and stream primitives, validation, and revision-specific
  wire framing. It does not own a game world or render a frame.
- cache owns JS5 storage, index/group/file recovery and decryption. Storage,
  reference tables and file recovery are implemented. Server definition loaders
  live above it in server::definitions; the item-cache decoder and ordered
  templates are implemented before content configuration. Other server definition
  families and their gameplay bindings remain pending.
- game-core owns rules and state transitions independently of transport,
  graphics, and model inference. Current code implements small tested subsets.
- net provides TCP and binary WebSocket I/O plus a native cache peer. Browser
  network bindings remain pending; the current WebSocket implementation runs
  in native clients and accepts browser-compatible binary frames.
- protocol also owns signed BigInteger-compatible RSA, Base37 login names,
  original client request fields and Kotlin parser responses. The native login
  peer retains bytes arriving after the reply for subsequent game decoding.
  The shared game codec now provides pinned sizes, bounded framing, numeric
  client actions and typed scene/interface/inventory update fields. It remains
  separate from authoritative action handling and the live session/world driver.
- server currently owns a bounded cache/lobby endpoint, connection admission,
  request/reply queues, byte budgets, timeouts and worker limits. Native
  account records, password rules, provider selection and memory/SQL storage are
  connected through configured endpoints and separate bounded account workers.
  Player/communication metadata setup now runs in those workers after account
  acceptance; profile loading and social communication actions remain pending.
  Supporting profile libraries now cover activity/quest/global records,
  persistent attributes and skills/XP; complete player/world integration remains pending.
  Server item definitions now load archive 19 natively, preserving template aliases
  and defaults. Native Item/Container state libraries preserve copying, stacking,
  charges, shared events/arrays and listener callbacks. The complete item
  configuration loader now creates typed native properties from the original
  data. Live inventory/bank/equipment, profile, content and world bindings remain pending.
  Native core character profile sections now cover inventory, both banks,
  equipment, tabs, location, varps and persisted timer data. Entity timer state
  and queue/callback ordering have their own native library and composite JVM
  checks. A native timer registry now handles registered factories, automatic
  installation and spawn/get-or-start wrappers with JDK ROOT identifier casing.
  Base pulses, entity pulse slots and the shared world queue now preserve task
  timing, interruption, callback and telemetry order, with differential checks.
  Native entity-script processing now covers queue strengths, movement phases,
  approach/operable scripts and scripting API helpers. Its content executors,
  nodes, paths, interfaces and other world effects are explicit bindings.
  Native door handling now preserves configuration order, opening/closing
  decisions, geometry and captured autowalk pulse bodies. The native scenery scene
  joins region loading, retained constructors, terrain/landscape, chunk collision,
  builder pulses and update packets. Original/native real-cache Lumbridge door
  traces cover opening and 500-tick restoration; see scene.md. Player/entity
  movement and live world delivery remain pending.
  Concrete listeners/content timers, registry/entity startup binding, full file
  persistence, world tick scheduling and authoritative partitions remain pending.
  The endpoint validates login packets in the bounded compute
  pool, returns account rejection codes, and reports world entry unavailable
  after successful account validation. See server.md
  and login.md for verified scope.
- client owns a shared game-facing core plus presentation. Windows desktop is
  the first visual target; GPU/window/audio integration should retain a browser
  path. The headless target should reuse decoded state and action rules.
- headless currently runs a real cache-transport probe. Its game-playing state,
  login and structured agent observations have not been implemented.
- A future experiment runner will own seeds, scenario setup, snapshots, replay,
  logs, agent interfaces, and result collection. Its controls must stay outside
  the default game's rules.

## Fidelity before parallel execution

First establish the serial world's tick and update ordering from the original
server. The reference game uses 600 ms ticks; exact subphase scheduling, queued
actions, delayed tasks, and save behavior still need integration verification.

Use bounded ingress/egress queues and keep socket work, compression, persistence,
and model inference outside the authoritative simulation phase. Network overload
must produce measured backpressure rather than an unbounded chat queue.

For candidate parallel work, derive reads from a stable phase snapshot, compute
in workers, and apply results in the reference order at defined boundaries.
Cross-region combat, item ownership, transactions, timers, and RNG consumption
need explicit ownership and ordering. Verify outputs against the serial
implementation; race freedom alone is insufficient.

Do not adopt new tick intervals, XP tables, drops, path selection, or reordered
RNG to obtain a throughput number. Any experimental rule change requires an
explicit alternate configuration and is outside default parity.

## Test layers

1. Differential primitives: run unmodified original classes, record outputs
   and hashes, then test native functions against the committed corpus.
2. Data fixtures: load actual cache groups and compare decoded definitions,
   scripts, models, maps, animation frames, and audio samples.
3. Game scenarios: feed the same actions and controlled RNG/events through
   original/native systems; compare state changes and outgoing messages.
4. End-to-end traces: connect the original visual client, native visual client,
   and native headless client to the native server and replay equivalent actions.
5. Long runs: compare snapshots, inventory conservation, economy, saves, and
   restart/recovery across many ticks. Test differing thread counts and overload.
6. Presentation: compare reference/native scenes, animation timing, interfaces,
   camera/picking behavior, sound samples, and music scheduling.

Test adapters must identify their scope and cannot establish full subsystem
parity. JagString/Location and RegionManager adapters permit isolated compilation;
each corpus records whether unsupported paths throw or use explicit storage,
region-load and routing adapters. Native chunk storage and landscape/clipping
kernels have separate differential corpora. The native scene now joins region
loading with doors, builder pulses and packet output, including a real pinned
Lumbridge comparison. Entity scheduling, synchronization and live world delivery
remain required.

## Performance evaluation

Record release settings, hardware, inputs, repetitions, warmup, checksums, and
raw timings. The first benchmarks are primitive measurements, not tick or player
capacity tests. Keep normal gameplay, busiest regions, chat bursts, scheduled
tasks, and saving in future world measurements.

Report p50/p95/p99 tick duration, missed deadlines, allocations, resident memory,
network bytes, persistence lag, and queue depth. Separate connected, idle,
scripted-active, and externally model-driven players. Compare like-for-like
behavior and verify correctness before accepting a performance result.
