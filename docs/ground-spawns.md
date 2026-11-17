# Ground spawn lifecycle

`world/ground_spawn.rs` implements the original GroundSpawn rate initialization,
population-dependent respawn delay and captured pulse body. Its registry lets
the owning world's GroundItem callbacks identify this subtype: private is false,
active and autospawn are true, and respawn submits through the existing global
Runner. Configuration loading and the diagnostic string are now implemented;
see [ground-spawn-loader.md](ground-spawn-loader.md) for their validation and I/O
limits. Concrete live-world services remain separate.

The original setter `min or max shl 16` associates left-to-right in Kotlin. It
stores `(min | max) << 16`, including its surprising zero minimum. Init's separate
normalization only duplicates the low rate when the signed high half is zero.
The duration retains the original two double divisions, including zero players,
invalid/zero maxima, over-capacity populations, NaN and saturating conversion.
The server's original maximum remains an owning-world configuration value.

Respawn captures the current world runner before reading the population. Later
replacement of the runner does not move that pending submission. The anonymous
pulse uses the shared canonical scheduler's fixed queue prefix, recreates the
same item reference and finishes. No second spawn clock advances this task.

GroundItem construction now has an internal subtype-registration hook before
the first exposing callback. A constructor failure can leave the original
GroundSpawn reference reachable with its default rate zero; later virtual
dispatch still uses that subtype. The subtype owns an unleased ground handle.
Exposed ground/Item views and queued/exposed pulse handles retain their owners;
weak registries release the graph after the final actual reference disappears.

The JVM corpus compiles the complete unchanged nested Kotlin GroundSpawn class,
GroundItemManager/GroundItem, Java Pulse and Kotlin PulseRunner. Only the runner's
clock import is replaced for deterministic telemetry. Existing region/plugin/
packet/definition adapters and explicit population/maximum/runner services are
recorded; no-op metrics and stack-trace stderr are outside the observations.
108 scenarios cover rates, population boundaries, failures, callback mutation,
subtype visibility, duplicate destroys and newly submitted pulse ordering.
Native regressions additionally verify canonical task and Item ownership release.

```powershell
cargo test -p rs09-server --test ground_spawn --test ground_manager --test chunk_sync
cargo xtask ground-spawn-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

Authoritative Take/inventory and concrete owning-world activity/plugin/session
services remain open. Existing production endpoints have not been replaced by
these component tests.
