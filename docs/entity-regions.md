# Native entity regions

`world/entity_regions.rs` ports RegionManager.move and its generic local entity
query, RegionChunk's four player/NPC list methods and Region.isTolerated. A
227-scenario JVM corpus executes the original method bodies with explicit
entity, activity, zone, clock and region/chunk lookup services. It compares
events, partial errors, ordered results, lists and aggression tolerance maps.
This is a spatial kernel and live binding, not a complete RegionManager,
RegionChunk, Region, Entity or game-world replacement.

The local query scans chunk x then y, with players before NPCs in mixed queries.
It captures matching entity references before applying inclusive ZoneBorders
filters, reads each entity's location separately for x and y, and retains the
original temporary list through callbacks. Nullable/raw-list entries and
duplicates follow the reference type filtering. Negative distances retain the
original integer/range/border quirks; excessive queries reject after a bounded
65,536-chunk work calculation.

Move compares actual chunk and region identities, rather than deriving equality
only from coordinates. Player removals/additions invoke inactivity/activity
callbacks after their list writes. Cross-region moves remove the old name's
tolerance and record the new timestamp after the original late name/cast reads.
Same-chunk moves still notify the zone using the current footprint. The tolerance
test always consumes both clock samples, even when a map entry is present, and
keeps wrapping subtraction and the strict ten-minute boundary.

Native RegionChunk lists are shared ArrayList-style handles containing canonical
node leases. Exposed lists, duplicate entries and returned query results keep
old nodes alive; reused wire indices cannot rebind them. Each world owns its
node identities independently. Removal drops one matching list occurrence.

`movement_session` and `npc_session` bind these kernels to the existing native
Scene chunks and canonical movement queues. Player admission, queued movement,
NPC initialization and disconnect membership use the same lists. Player and NPC
render candidates now come from the native local query, including the original
same-plane chunk lookup and 15-tile bounds. Regression packets exercise distant
players entering at the boundary and leaving on a plane change.

Region activity/inactivity pulses, viewer startup, concrete zones, dynamic-region
replacement/routing, complete Entity lifecycle, masks and content still require
implementation. The controlled fixture keeps its explicitly preactivated
regions and empty content services. No player-capacity or performance claim
follows from these tests.

```powershell
cargo test -p rs09-server --test entity_regions --test chunk --test region_load --test scene
cargo test -p rs09-server --example movement_session --example npc_session
cargo xtask entity-regions-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```
