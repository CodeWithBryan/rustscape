# Canonical interaction scheduling

world/legacy_runtime.rs binds the selected original InteractPlugin dispatch and
arrival bodies to player_walk::State. A captured manager includes its original
node lease, so callbacks can replace the live manager without changing the
receiver used by the pending run. Its routing handler is passed to the native
MovementPulse constructor separately from the shared option retained for arrival.

The owning state now registers ordinary delayed and unhandled pulses alongside
movement pulses. They share the same runner, STANDARD manager slot, stop/replace
semantics and original fixed queue prefix. Their immutable bodies keep their
original player, target and option after checks are cleared or a same-coordinate
object/option is replaced. Plain tasks leave Entity.currentMovement unchanged.
The existing unhandled constructor is shared with PulseManager's standalone port.

Collection follows runner, manager, exposed pulse/queue/node handles and captured
references. Original and detached exposed check arrays retain their nodes.
Internal task/manager/node cycles are reclaimable after disconnect; an external
manager or pulse remains usable without rebinding a recycled numeric node ID.
Foreign-world plain tasks and captured manager receivers are rejected.

Node.getLocation/size reads use canonical state without consulting a scenery
definition. Facing uses the already verified reflected-center geometry. Profile
interaction locks, handler methods/default locale, zones, combat, face masks,
events and logs are explicit reentrant Bindings. Host Services delegate movement
and delayed arrivals to legacy_runtime::arrival. No node lock is held during a
service callback.

The joined differential harness compiles unchanged full InteractPlugin, Option,
OptionHandler, MovementPulse, DestinationFlag, Pulse, PulseManager, PulseRunner,
SmartPathfinder and WalkingQueue, plus original Entity.tick and Node/Location
algorithms. Its 323 scenarios compare ordered effects, queue/movement state,
current manager, pending class identities and faces after every command. Cases
include short circuits, reentry, replaced handlers/options/managers, equal-tile
replacement objects, stopped tasks, inactive checks and caught versus delayed
errors. Profile/zone/handler/facing/log services remain explicit adapters.

The shared player-walk generator has an opt-in extension for this composition;
all 190 existing expected records remain byte-for-byte unchanged. Its provenance
updates the generator hash. Logical JSON inputs must round-trip unchanged.

```powershell
cargo test -p rs09-server --test interaction_runtime --test player_walk --test legacy_interaction --test pulses
cargo xtask interaction-runtime-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
cargo xtask player-walk-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
cargo xtask pulses-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

This is a native owning-world binding and joined scheduling proof. The existing
live development endpoints are unchanged. Complete profile/locks/zone/content
services, option/listener/plugin registries, real facing mask publication and
viewport/chunk synchronization remain necessary for live doors. The optional
[shared entity/scenery pulse phase](world-pulses.md) now supplies one queue and
domain-correct dispatch; existing live binaries have not been replaced. Full
class replacement, client/headless/content parity, parallel equivalence and
measured performance/capacity acceptance remain open.
