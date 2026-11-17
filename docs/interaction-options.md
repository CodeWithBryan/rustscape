# Interaction options and ground clicks

`world::interaction_options` ports the option-table methods of InteractPlugin
and Option.defaultHandler against the pinned server. Tables begin with eight
slots. init replaces the array but preserves initialized; setDefault publishes
initialized only after its entire operation succeeds. Player defaults share the
world's canonical Follow, Trade with and Req Assist handles. Item defaults choose
ground versus inventory names by the presence of a location, independently of
the GroundItem subtype.

The current options array and the input names array have separate retained
identities. A callback can replace the table during initialization; the outer
loop uses the new table's length and the original input names. Failed operations
retain their completed prefix. Public array handles represent fixed-length Java
arrays: hosts may replace slots but must not resize those arrays in place.

`Context` requires native node-class/ID, definition/name, default-locale casing,
handler-registry and player option-packet services. Tables use opaque node IDs;
hosts must keep those bindings alive while exposed and never recycle an ID to a
different node. Registry IDs follow the same rule. These contracts still require
owning-world bindings; this module does not supply empty production services.

`world::ground_action` ports PacketProcessor.processGroundItemAction over the
actual native GroundItemManager. Lookup uses the player's current plane and
canonical retained actor handle before scripts reset. A missing item clears the
minimap. A missing option invokes the required invalid-interaction service,
then clears the minimap. Plugins run before ground-item listeners, then item
listeners. Each listener receives the late item ID and the retained selected
option's name. The final approach dispatch re-reads the interaction receiver.
It never skips movement/arrival and directly transfers an item.

The option corpus compiles unchanged complete InteractPlugin/Option/OptionHandler
classes and runs 730 scenarios. ROOT locale, definitions, registries and packet
delivery are explicit recording adapters; other approach bodies are not executed
by this corpus. The ground-click corpus executes the unchanged Kotlin method in
323 scenarios joined to original ground manager/item/spawn/chunk code. The Rust
replay uses its real manager, canonical actors/chunks and shared option tables;
plugins/listeners/invalid/approach/minimap remain recording services. Reentrant
replacement/removal, privacy, duplicate items, selected-option retention,
short circuits and failure prefixes are covered. Original property getter access
is preserved, and snapshots copy mutable arrays.

Real ground default tables and the typed item handler registry are now joined
by `ground_options`; see [the binding scope](ground-options.md). Live Take still
needs owning-world ground navigation
and approach/arrival binding, PickupHandler world/inventory/profile services and
packet-loop binding. These tests do not establish live Take, full interactions,
complete classes, performance improvements or player capacity.

Run `cargo test -p rs09-server --test interaction_options --test ground_action`
and `cargo xtask interaction-options-oracle --check` /
`cargo xtask ground-action-oracle --check` with Python3.10+ and JDK11. Shared
legacy, ground and packet regression gates are listed in AGENTS.md.
