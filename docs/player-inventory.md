# Owning-player inventory integration

The native inventory listener now binds to the actual inventory, equipment,
Settings fields and canonical player. The live gameframe invokes that listener
for inventory refresh, and its shared interface manager emits the weight-text
counter. Inventory changes use the same container callback path. No duplicate
item array, definition table or cached weight is maintained by the binding.

The library is crates/server/src/inventory/player_binding.rs; its required
services retain familiar, appearance, session and content receiver ownership.
Its Capture component now retains the actual canonical owner, inventory,
equipment, Settings, NULL_ITEM and InventoryListener. Container is itself a
shared object handle; captured references observe later array and event
replacement. The live UI binds the same component together with its actual
body masks, familiar flag, definition table and packet counter. Getter views
retain this component without creating internal owner cycles. No mutable player
row, field or registry guard spans callbacks.
Settings.updateWeight reads each current Item's cached definition and typed
weight property. It does not look up the current item ID again. This preserves
unmasked charge changes that alter an ID while retaining its old definition.
Missing/null properties default to zero; wrong property types fail after the
inventory packet has already been sent. The private weight adapter retains the
exact current Item handle without allocating a token registry.

The world publishes one loaded Arc registry to both ground items and the UI.
The table is fixed after loading; individual definition handles and their
configuration remain mutable and shared. This removes a registry guard from
callback paths. Reloading/replacing the table and original map views remain
separate work. It is not a measured throughput or player-capacity improvement.

The 184 joined JVM scenarios use actual original Container/Item/Event and
InventoryListener, verbatim Settings.updateWeight, original familiar accessors,
Appearance.sync, interface counter, PacketDispatch.sendString and complete
container/string packet writers. They compare actual packet bytes, weight bits,
event identity, array replacement, configuration mutations, recursive refresh,
failed sends, wrong types, nonfinite weights and integer counter wrapping.
Other Settings APIs, full Player/session startup and full familiar gameplay are
explicit external boundaries of that corpus.

~~~powershell
cargo test -p rs09-server --test player_inventory --test inventory_listener --test settings
cargo xtask player-inventory-oracle --check --python PYTHON_PATH --java-home JDK11_PATH
cargo test -p rs09-server --example movement_session --example npc_session
cargo test -p rs09-headless
~~~

Both movement/NPC example targets exercise the live UI binding. Initial inventory
105 precedes weight text171 and skill updates. Real container additions emit22
then171, count a stack's definition weight once, skip negative inventory weights,
include signed equipment weights in slots0 through10, and ignore equipment
slots11 through13. Pouch transitions update the actual appearance data and flags
before rendering. This fresh-player probe has no active familiars or equipment
actions; EquipmentListener, summoning/pet lifecycle and complete profile/body
preparation still need implementation. Its internal inventory listener lease
is released on disconnect when no external captures remain. Captured inventory
and Settings views now survive removal of the metadata row and retain the
original fields/body/counter. A native live-example regression replaces the
wire-index occupant, updates the old inventory through its actual listener,
then proves the replacement's fields, counter and output remain untouched.
The host checks the canonical node before frame delivery, and captures reject
other worlds even when their numeric IDs match. The actor now owns the supported
fields and traces listener/list/container/mask edges, including disconnected
cycles. A retained listener alone can reach its original fields after metadata
retirement. Callbacks on another same-world player's container use the listener's
actual owner for weight/counter/body and the callback container for packet/pouch
reads. See owned-player.md. Complete Player/profile lifecycle and arbitrary plugin
graph collection remain pending.

The headless observer now returns bounded inventoryEvents using the visual
client's shared packet decoder: full, partial, clear and weight-label messages.
The history stores exact IDs, amounts, slots, pointer/container IDs and label
bytes. A clear message is an observed wire event; no client container/CS2 state
is guessed. Weight text is the server's integer-truncated display label, not
precise mass. This supports data-only observation while complete shared UI state
and agent action support continue to be implemented.

Authoritative ground option/navigation/arrival/Pickup binding, live Take/Drop,
complete saved profiles, all inventory/bank/equipment actions and full client/
server gameplay remain open. Native primitive and joined-library evidence does
not complete those acceptance gates.
