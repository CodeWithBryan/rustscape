# Canonical ground approach targets

`player_walk::State::ground_target` binds an exact GroundItem reference to a
canonical node identity. Reusing that reference returns the same binding;
another object on the same tile receives a different identity. Removed items
remain available to a captured approach. Locations, inherited node size and
activity remain live. GroundSpawn retains its subtype and always-active override.
Native path dispatch identifies ground items separately from generic nodes,
preserving the original teleport-permission and approach geometry rules.

The state stores an ordinary ground handle or the actual Spawn owner from the
supplied subtype registry. It does not store a leased base view as an internal
owner. Exposed views use ground_item::as_item and ground_spawn::as_ground so they
retain the same logical object and subtype. Unknown leased subtypes and foreign
dropper handles are rejected. Native IDs are never recycled while observable.

Collection counts known ground-owned dropper leases as graph edges, then traces
ground targets, actors, immutable approach captures, current motions, managers,
runner tasks and exposed check arrays together. External ground/Item/subtype
views keep their corresponding graph alive. Detached disconnected cycles are
reclaimed once their last external roots and scheduler references disappear.
The runner still owns a queued pulse until its normal update removes it.

Shared weak views now retain a weak reference to their actual owner. Upgrading
reconstructs ownership of that owner, even when the particular original view
anchor has retired. It never silently upgrades a subtype view into an unowned
base. Unleased internal weak references retain their previous behavior.

The differential corpus executes 172 joined original scenarios using the
unchanged GroundItem, MovementPulse, scheduler, walking and pathfinder bodies.
Original GroundSpawn activity/privacy/auto-spawn override bodies run in an
explicit constructor-only subtype adapter. Construction/default content, player,
regions, clipping, handler destinations, teleport permission and interaction
arrival are explicit adapters. Native replay uses actual ground/spawn objects,
actor state, queues and scheduler. The corpus covers size, decay, removed state,
far-path truncation, permission branches, target/option replacement and callback
mutations. Null exception stack headers are normalized into recorded categories.
Rust-only ownership tests cover duplicate coordinates, retained Item/base/subtype
views, anchor retirement, foreign registries/droppers, detached checks, and
single-actor/cross-actor dropper/approach cycles. JVM GC timing is not compared.

Run `cargo test -p rs09-server --test ground_target` and
`cargo xtask ground-target-oracle --check` with Python3.10+ and JDK11. Shared
regression gates are in AGENTS.md. The previous 190 joined walking observations
remain unchanged. This is a native target/navigation binding; the owning-world
option registries, ground packet-to-approach-to-pickup path, profile inventory and
content services still require integration. Live Take and performance remain
unverified.
