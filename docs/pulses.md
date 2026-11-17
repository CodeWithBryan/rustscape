# Native pulses and scheduling queues

`server::pulses` implements base Pulse state/update behavior, the entity
PulseManager, and the world PulseRunner queue. Content, nodes, scripts, packet
dispatch, death state, clock and telemetry are explicit native `Context<S>`
bindings. This is a scheduling library; the running world, 600ms driver and
concrete movement/combat/skill/bot/cutscene pulses still need integration.

## Pulse behavior

A pulse starts running, with zero elapsed ticks and its constructor delay. The
default constructor's delay is one. Checks can be null, empty or a shared fixed
node array. Null entries are skipped. Each iteration reloads the checks field;
changing that field during a node callback can therefore expose a new array,
null, or bounds failure after earlier callbacks have run.

Update checks inactive nodes before running state, increments elapsed ticks with
signed i32 wrapping, resets the counter before invoking the body when due, and
stops finished pulses. Exceptions from the body or its first stop call are
reported before another stop call. Failures from initial checks, initial stop,
initial running query, or the final running query propagate to the runner.
Restart resets only the counter; start/stop do not reset timing. The Runnable
entry point calls the virtual update and ignores its result, as in the source.

Default context methods implement actual base behavior. Concrete content can
override update/start/stop/running/check/removeFor hooks and call the base
helpers explicitly. Delay/counter/check accessors preserve base state behavior;
complete concrete subclass behavior remains a content-porting task. In
particular, removeFor is not consulted by the original manager's run method.

## Entity slots and interruption

The four priority slots are STANDARD, STRONG, CUSTOM_1 and CUSTOM_2. Current-pulse
queries scan that declaration order. Clearing all stops non-strong entries but
retains them in the map. A stopped STANDARD entry can consequently mask a running
custom or strong pulse in status queries until a later run removes stopped
entries. This behavior is preserved.

Starting a pulse first queries every current non-null pulse, then removes the
stopped entries. An existing strong pulse blocks new work. Clearing the selected
slot invokes weak-script removal, normal-script removal, then stop/removal for
that slot. Starting STRONG also clears other slots, repeating the script-removal
calls in source order. The new handle is stored before virtual start, then queued
only if the virtual running query succeeds. Failures preserve preceding changes.
Unhandled actions create the original one-tick checked task that sends
`Nothing interesting happens.`; actual player packet delivery is a binding.

Java enum identity hashes determine HashMap callback iteration order, separately
from the declaration order used by status queries. `Manager::new` requires four
explicit world/replay hash inputs. Its small bucket map preserves insertion,
replacement, removal, retained entry links and HashMap.forEach's modification
check at the end of traversal. The map holds at most four enum keys plus null,
so the original default table never resizes or becomes a tree. Reentrant
callbacks observe live entries, rather than a cloned snapshot of the slot map.
Default world hash policy and startup remain pending; no declaration-order
replacement for Java hash iteration is silently assumed.

## World queue and telemetry

The queue accepts concurrent submissions and retains handle identity and
duplicates. At update entry, the runner captures the queue size. It processes
exactly that many head entries. Tasks submitted during callbacks wait for a later
update, while unfinished running tasks are appended at the tail. Running queries,
clock samples, caught errors, logs and queue mutations keep their original order.

Elapsed wall-clock subtraction wraps as i64, then metric values/totals cast or
wrap as i32. Bot subtype and case-sensitive `ScriptAPI` name classification,
30/100ms warnings, per-pulse metrics and conditional final totals follow the
original. Metric/provider failures are outside the pulse-execution catch and can
leave a partial tick. The native telemetry binding must reproduce the provider's
enabled/uninitialized behavior; actual Grafana/SQLite storage is not ported here.

The queue uses a Mutex/Condvar and releases its lock before executing content.
Like LinkedBlockingQueue.take, removal waits if another consumer drains it.
The world runner is intended for the original single-consumer update model;
arbitrary concurrent consumers, recursive drains that empty an outer tick's
prefix, JVM thread interruption and parallel game simulation are outside this
milestone. Parallel producers are tested, without changing tick ordering or rules.

## Verification

```powershell
cargo test -p rs09-server --test pulses
cargo xtask pulses-oracle --check --python <python> --java-home <jdk11>
```

262 original scenarios cover base state, virtual callbacks, shared checks,
signed boundaries, slot priorities, strong/failed/reentrant interruption, hash
collisions and traversal mutations, queue prefixes, duplicates, nested empty
updates, failed metrics, clock jumps and warning thresholds. Snapshots include
all handles, array identities, slot/queue order, callbacks, messages, clocks,
telemetry and partial errors after each command.

The complete original Pulse.java and PulseManager.java compile unchanged. The
complete PulseRunner class is compiled with one added import alias that binds
System clock calls to controlled samples; its remaining source is verbatim.
PulseType is represented by an identity-equal class adapter with the original
values and controlled hashCode, because JVM enum identity hashes are not a
portable fixture constant. Nodes, entity/scripts, packet dispatch, death and
concrete movement/combat/bot types and telemetry are explicit adapters.
Reference source remains unmodified and fixture provenance pins every input.

A native test also submits 512 tasks from four synchronized producer threads,
verifies each producer's FIFO/no-loss behavior, and compares execution with a
fresh serial replay of the observed queue order. Four child tasks remain queued
after the first update and execute on the next. This establishes queue behavior
for that workload, not throughput, player capacity or parallel world fidelity.
Full gameplay/world integration and all source-file replacement acceptance
remain pending.
