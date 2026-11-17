# Typed component callbacks and captures

Components and definitions now store actual native ComponentPlugin and CloseEvent
objects in `server/world/component_objects.rs`, rather than observation numbers.
Handles preserve object identity, typed payloads, weak upgrades and captured
receivers. Different instances may have identical labels or state. Clones retain
the same object. Runtime dispatch downcasts the captured payload; it does not
resolve an observer ID or a recycled player's wire index.

PIN programs and close events use these same headers. Converting between the PIN
API and component API preserves the exact object. A PIN close event has one real
manager field; its manager has one player field. `BankClose` similarly retains
the actual bank receiver. The stateless Component.setUnclosable event captures
no player: its reserved attribute is read from the player passed to close.

The typed-object milestone had 351 original JVM scenarios (now preserved within
the 628-case complete-manager oracle; see interface-manager.md). The first 238 are
independently pinned and byte-identical. It compiles the original full Component
with only a constructor observation recorder, the full ComponentPlugin and Plugin
defaults, and the existing complete definition/type/config and packet writers.
The 113 added cases compare distinct equal-label objects, aliases, cached versus
late plugin lookup, old definitions, callback replacement and retirement, nested
execution, late callback results, setUnclosable, typed failures and completed
prefixes. Snapshot IDs are diagnostic identities, never dispatch inputs.

The canonical capture tracer is `player/owned/components.rs`. It traverses the
actual manager fields, tab arrays, components, definitions, callbacks and PIN
managers once per object. Incoming physical fields count separately; sharing a
tab array or callback does not duplicate the fields inside it. Callback captures
of ordinary nodes, PIN managers and containers join the existing actor/container/
listener graph. No field guard spans capture metadata inspection or gameplay
callbacks. Capture metadata must report every supported strong field and must
not execute gameplay behavior.

Seven native ownership regressions cover internal PIN cycles, exposed component/
event/array roots, mutual player captures, actual bank receiver fields, multiple
node fields and equal/aliased callbacks, stateless noncapture, global definition
roots and shared arrays retaining only their actual targets. External components
retain players only through fields that really capture them. Clearing the last
external root collects disconnected actor/UI/PIN cycles in these supported graphs.

Both controlled live examples exercise the typed PIN program's inherited empty
open body, original captured close body and unclosable attribute guard using real
canonical player fields. Foreign PIN callbacks are rejected before mutation;
old local captures stay bound to the original manager. Unknown plugin/close
payloads still return an explicit unavailable-service error.

InterfaceManager lifecycle and physical owner fields are now joined and tested
(see interface-manager.md). Remaining work includes complete listeners and
content registration, PIN entry packet services, profile persistence and
playable banking. Arbitrary opaque plugin graphs, unsupported object-field kinds,
cross-world cycles, general interface casts/subclass/reflection APIs and exposed
definition-map iteration are not established here. Extend capture metadata and
collection for concrete new payloads before installing them; never hide owner
anchors inside their own graph. This is not full UI/client/server completion or
a performance/capacity claim.

```powershell
cargo test -p rs09-server --test interfaces --test bank_pin --test owned_player
cargo xtask interfaces-oracle --check --java-home runs/jdk11/jdk-11.0.32.1+1
```

Captured BankClose execution now uses canonical bank/UI services in the controlled
composition; see bank-runtime.md. Live content/admission remains required.
