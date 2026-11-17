# Inventory listener

server::inventory::listener ports all three original InventoryListener methods
over the native Container, ContainerEvent and Item handles. It constructs actual
full (105) or partial (22) inventory frames for interface 149, child 0, container 93.
Player settings, familiar manager, appearance and session delivery are required
owning-world services. They are not replaced with empty default callbacks.

Refresh and event update send the packet before calling the shared update
routine. Update gets the current Settings receiver, recalculates weight, reads
the current familiar's prior hasPouch value, scans the live container, gets the
current familiar again, writes hasPouch and synchronizes appearance only when
the captured prior value differs. Callbacks can replace arrays or managers,
reenter the listener, or fail. Completed mutations remain after failure.

Packet inputs retain the actual item array. Event slots are captured in
ascending order before send. Clearing a container can replace its array without
replacing a retained packet's array. Partial writes distinguish the owning
world's NULL_ITEM by reference identity; a separate Item with the same numeric
fields is not that sentinel. Full writes retain the original separate rule for
nonnull NULL_ITEM. The bounded native wire encoder can reject oversized arrays,
payloads or invalid smart slots outside the original inventory's 28 slots.

Pouch membership uses a generated sorted table of 83 original map keys, with
binary lookup and no runtime hash-map allocation. The generator reads the
pinned enum declarations, including its alternate boolean constructor.
This ports only SummoningPouch.get key membership; familiar creation, pouch
crafting, enum metadata, mutable item arrays and summoning gameplay remain open.
No throughput or total-memory improvement is claimed from the table alone.

Run:

~~~powershell
cargo test -p rs09-server --test inventory_listener --test inventory --test settings
cargo xtask inventory-listener-oracle --check --python PYTHON_PATH --java-home JDK11_PATH
python tools/port_summoning_pouch_ids.py --check
~~~

The 1,468 scenarios compile the complete unchanged InventoryListener,
SummoningPouch, ContainerContext, ContainerPacket and IoBuffer. They use complete
unchanged Item/Container/Event classes freshly reproduced by the existing
inventory oracle's 389 cases. Original packet bytes, real Container.update callback
dispatch, retained arrays/slots, charge/amount boundaries, two manager receivers,
recursive updates, null receivers and failed prefixes match native results.
The original map is also queried over all 65,536 representable Item IDs, producing
exactly the 83 native keys.

The JVM Player/Settings/Familiar/Appearance/Session/PacketRepository adapters
record controlled services. ISAAC returns zero for plaintext packet comparison;
unused external string codecs throw. These observations do not prove actual
weight computation, complete Player/profile/familiar/content, session delivery,
listener installation, authoritative pickup or live inventory/Take. Those
services must bind to the same canonical player/world and native listener in
the continuing integration work. Existing sealed client/server checkpoints are
preserved.

The subsequent [owning-player binding](player-inventory.md) now joins this
listener to real Settings weight calculation and the live fresh-player gameframe.
Its complete profile/content/Take boundaries remain explicit there.
