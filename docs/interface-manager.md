# Native interface lifecycle

`world/interfaces.rs` now ports the pinned InterfaceManager's open/close, main
interface, chatbox, single-tab, overlay, tab removal/restoration/view selection,
lookup and existing gameframe/window operations. The oracle compiles the complete
unmodified original InterfaceManager class, rather than a selected-method class.
Its Player, event, content, weapon/friends/tutorial and packet dispatch boundaries
remain explicit adapters. The original CloseInterface, RunScriptPacket and
InterfaceConfig writers produce the compared bytes.

The interface corpus contains 628 cases. Independent locks preserve both the
first 238 metadata/gameframe observations and all 351 typed-component observations
byte-for-byte. The 277 new cases compare every interface type, null/short/shared
tab arrays, dynamic array replacement, hidden tabs, duplicate IDs and lookup
priority, vetoes, callback reentry and completed failure prefixes. A default
single-tab close event is the same stateless lambda object at its Java call site;
the Rust object likewise captures no player and shares one header.

Preserved behaviors include:

- Main close uses the component present after its close callback, then reads the
  current field again for the close event after sending. Dispatch can change the
  value returned by open. No field guard spans any callback.
- Main close normally sends no packet for a walkable component; ID 14 is an
  exception. `isWalkable` only returns false for ID 389, even if another opened
  definition is marked non-walkable.
- Tab removal/default-tab closure discards a tab even when its close vetoes.
  Single-tab close always returns true, including a veto that leaves it open.
- Restore uses the original fixed child offsets for hidden tabs in either window
  mode, sends both config packets, then unhides the current array's entry.
- A null window causes lookup to fail before searching tabs or special fallback
  components. A missing tab leaves viewed-tab state unchanged.
- The `runscript` attribute is removed before reset script 101 sends; packet
  counters wrap and advance before sink failures.

Each canonical manager now owns one real, immutable player handle. Startup binds
it once and rejects a different receiver before constructing replacement player
fields. Exposed manager views clone that actual object; they no longer attach a
separate owner anchor. The capture tracer counts the physical player field once,
follows nullable tab arrays and single/overlay/wilderness component fields, and
collects disconnected internal cycles. Components and raw tab arrays only retain
players actually captured by their contents.

The controlled native UI context validates the actual manager/player identity
before metadata, attributes, packets or callbacks change. Both movement and NPC
examples exercise real PIN close state, reset script and interface close bytes,
and reject foreign manager receivers without changing their attributes or local
component metadata. The fixture explicitly has empty game-event listeners; this
does not prove full event dispatch or banker/PIN entry gameplay.

Later [bank runtime](bank-runtime.md) and [bank content](bank-interface.md) stages
join captured BankClose, BankInterface registration and basic live transactions.
Full content/event dispatch, banker/booth admission, PIN/input/dialogue services,
profiles and remaining packet handlers remain required. Whole-source parity rows
remain `not_replaced`; this kernel is not complete UI/server/game replacement or
a performance claim.

```powershell
cargo test -p rs09-server --test interfaces --test bank_pin --test owned_player
cargo test -p rs09-server --example movement_session --example npc_session --example player_session
cargo xtask interfaces-oracle --check --java-home runs/jdk11/jdk-11.0.32.1+1
```
