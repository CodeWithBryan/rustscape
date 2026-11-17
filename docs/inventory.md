# Native item and container state

`server::inventory` ports Item-owned state, Container mutations, ContainerEvent
changes and synchronous listener ordering. The code uses Rust and the native
definition registry. Transport, rendering and AI inference are outside these rules.

Container now uses a shared object identity. Clone retains that same object,
including replaced item arrays/events and the actual exposed listener list.
See [container-ownership.md](container-ownership.md) for the 207 original JVM
alias/reentry scenarios and actual retained player-inventory binding.

This is a library milestone. Core inventory/bank/equipment profile sections are
now covered in [core-profiles.md](core-profiles.md). Actual Inventory/Bank/Equipment
actions/listeners, player lifecycle, packets, ground-item spawning, degradation
tables, startup application of item configurations and concrete plugins still need integration. The
full original Node and its interaction/world behavior remain unported. All source
inventory entries remain `not_replaced` until full runtime acceptance.

The native [InventoryListener](inventory-listener.md) now supplies the original
listener control flow and inventory packet construction with differential
coverage. The bounded owning-player weight/body/gameframe binding is covered in
[player-inventory.md](player-inventory.md). Complete profile/content, bank and
equipment integration remain separate work.

## Item behavior

- IDs and charges retain the original packed signed integer, including unmasked
  charge writes that can change the ID bits. Changing charge does not refresh the
  cached definition. Setting an ID refreshes the definition without changing the
  Node name. Missing definitions retain the registry's original defaults.
- Constructors accept raw negative amounts. The amount setter clamps negatives
  to zero. Default Item has a null definition and index zero; a regular Item has
  index minus one. Copying creates a regular Item with current ID/amount/charge,
  resetting its name, definition and slot through the original constructor rules.
- Note conversion returns the original shared instance when already unnoted.
  Drop conversion calls the explicit degradation binding, rechecks the current
  ID after that call, and resets charge to 1000 only when creating a replacement.
- Value/alchemy calculations preserve lower bounds, note redirection, configured
  integer overrides, JDK ties-to-even rounding and the original error behavior.
  Definition high/low-alchemy lookup is implemented as a native helper here;
  it does not imply completion of the rest of ItemDefinition's configuration API.
- Operate-handler and plugin lookups use required `Bindings` methods. Their
  concrete implementations remain a world/content integration task. The test
  adapter supplies explicit opaque objects and records degradation decisions;
  it does not execute item plugins or spawn ground items.

## Container behavior

The four container types and ID/hash sort rules remain distinct. Adds preserve
partial acceptance, preferred slots, signed count wrapping, constructor/copy
behavior and notification timing. Hash-based bank stacking still uses ID-based
amount/maximum checks where the source does. Removes preserve their separate
stacking test, slot/amount checks and charge selection. Replace/insert, self-add,
self-copy, different-capacity copies, clear and shift retain source ordering.

Queries preserve the differences between ID, hash, quantity and at-least-one
overloads. In particular, the empty/zero/negative/overflow cases are compared
against the actual methods. SQL-format output uses native JDK Integer HashMap
insertion/resize/tree-bin iteration and wrapping amount totals. Its output is
built in one Rust string buffer; no throughput improvement is claimed without
a workload measurement.

Container JSON loading reads all four row fields before validating IDs/slots,
uses definition-map size, skips magic carpet ID 5614 and preserves raw amounts
and partial writes. It also performs the original value calculation even though
its accumulated total is unused: an error there occurs after the slot was written.
This method is not a complete savefile/profile loader.

Items, arrays and events retain shared identity through Arc/RwLock handles.
External retained arrays continue to refer to the old array after clear/copy.
Events contain shared item handles, so later item mutations remain observable.
A `Factory` owns one mutable NULL_ITEM placeholder per world; it must be created
at the matching world initialization point with the correct definition registry.

Listeners are actual native objects supplied to a required `Context`. They may
mutate the container, retain events/items, recurse or return an error. Listener
registration/removal preserves the relevant ArrayList iterator checks, including
the case where shrinking to the cursor ends iteration. Each callback sees the
current event field; a recursive refresh/update can change the event seen by the
next listener. Normal and forced updates preserve their different retained clear
flags. Failed callbacks keep all mutations performed before the failure.

These are ordered state transitions. Shared locks do not prove deterministic
parallel gameplay; serial/parallel world replay remains a separate acceptance gate.
Arbitrary Java List operations, diagnostic object-address strings and all inherited
Node APIs are outside this library's compatibility claim.

## Reference and regression tests

The committed corpus contains **389 original JVM scenarios**, including over
4,800 commands and six integer-map traces. Five native tests compare:

- Item construction, copying, IDs, charge bits, quantities, values, alchemy,
  notes and explicit plugin/degradation boundaries.
- All container types/sorts, stacking/overflow boundaries, preferred slots,
  insert/replace/remove, quantity queries, formatting, copy and self-operations.
- Complete visible state and shared identities after each action, retained
  item/array/event references, mutable placeholders and raw array changes.
- Listener ordering, recursive updates/refreshes, registration/removal,
  injected errors, malformed profile rows and partial mutation.
- Fixture/source hashes and bounded native rejection of oversized containers
  and cyclic noted-item references.

The oracle compiles complete, unchanged Item.java, Container.java,
ContainerEvent.java, ContainerListener.java and both enums, plus unchanged
Definition.java. Selected original Node/ItemDefinition fields and methods compile
verbatim. Explicit adapters isolate location/interaction, opaque handlers/plugins,
ground spawning and degradation decisions. Each scenario resets the mutable
static placeholder to isolate world state. The adapter does not replace the
original container algorithms with a model of their expected result.

```powershell
cargo test -p rs09-server --test inventory
cargo xtask inventory-oracle --check --java-home PATH_TO_JDK_11
```

Python 3.10+ and JDK 11 reproduce the oracle. Ordinary native tests read committed
fixtures and need neither. The original source checkout is never modified.
Container construction rejects negative sizes and capacities above one million;
cyclic note alchemy returns a native error instead of overflowing the JVM stack.
Those safety differences do not change valid pinned gameplay.

The [item configuration layer](item-configuration.md) now has separate native
coverage. Next work is player inventory/bank/equipment subclasses and persistence
sections, followed by actual listeners/packet encoding
and world integration. Playable login and complete profile loading remain pending.
