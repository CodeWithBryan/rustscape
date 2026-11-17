# Native container listener objects

Container and its exposed list retain actual native ContainerListener handles.
Each object has one immutable Arc allocation containing a typed native value.
Cloning a handle preserves identity. Equal recording labels or equal player
fields do not make separately constructed objects equal. Removal selects only
the first matching object reference, preserving the pinned concrete listeners'
default Java equality. Null entries remain representable and fail when invoked.

Typed projection clones that same Arc allocation before dispatch. It neither
locks the object nor constructs a new receiver from a registry number. A weak
handle can still upgrade while a captured typed receiver remains alive; it
retires after the final actual object reference drops. Listener implementations
may use shared mutable native fields, whose guards must end before callbacks.

The actual player binding now dispatches the InventoryListener stored in the
container. Its captured canonical player controls Settings, familiar, appearance
and packet operations; there is no parallel primary-listener ID or separate
listener substitute. Foreign canonical owners still fail before mutating this
binding. The bounded UI registers this real object at admission. Core save-state
construction also receives actual listener objects from its caller.

The new oracle compiles the unchanged complete original Container, Item, event,
listener interface and enums. Only the recording driver adds named listener
variables. Distinct instances can share a label; aliases can retain an instance
after variable replacement or removal. The 104 scenarios / 2,066 command
snapshots cover selective removal, live list replacement, nulls, direct refresh,
recursive callbacks and failed prefixes. The 389 original inventory scenarios
and 207 shared-container scenarios stay byte-identical. The native adapter's
numeric labels belong solely to recording objects and never select game logic.

Native regressions also exercise exact payload identity, weak upgrades through
typed captures, list-only retention after container retirement and recursive
dispatch after replacing a later list entry. Run container_listeners,
container_ownership, inventory, inventory_listener, player_inventory and core_data
tests and reproduce container-listeners-oracle. The associated UI/example,
world/headless and broader verification requirements are in AGENTS.md.

This is not complete Player/profile construction or general listener graph
collection. The owning actor now retains the currently supported player fields;
retained listeners reach them after UI metadata retirement. The collector traces
their actual object/list/node edges and releases detached cycles. See
owned-player.md. Complete original Player fields and backreferences remain
integration work. Bank, equipment, trade, shop and other concrete listeners, subclass virtual
dispatch, content/plugin behavior and authoritative item actions remain open.
No measured speedup or player-capacity claim follows from this object layout.
