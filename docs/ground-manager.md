# Server ground-item management

`server/world/ground_manager.rs` ports every GroundItemManager entry point and
the original RegionChunk ground-item add/remove bodies. It uses the existing
GroundItem state, canonical chunk lists, dirty flags and original wire writers.
An owning world supplies region lookup, actual plugins, player/AI services,
subclass respawn behavior and packet delivery.

Creation checks the native item definition's trade restrictions, calls the
drop plugin through its original two lookups, resets `removed`, adds to the
resolved chunk, then appends to the manager list. Errors retain completed
mutations. The constructor overload taking a UID retains the original absolute
decay tick. The array overload reads each live array slot in order; failure at
a later null entry leaves earlier creations in place.

Private additions send directly to the current dropper. Public additions append
an ItemUpdateFlag. Removal first removes the matching chunk reference; a private
clear also requires a playing owner within the original Euclidean distance 15
on the same plane. It must not use a square distance check. Destroy removes only
the first identical reference from both lists and performs virtual respawn
after removal; it does not itself set the item's `removed` field.

Lookup captures the original chunk list and uses ArrayList's live-size and
modCount checks. Replacing the chunk's list does not replace that captured
iterator. Replacing a slot is nonstructural; clear/add/remove are structural,
including an add/remove pair with unchanged final contents. An early successful
return does not perform an extra modification check. Nulls and duplicate
references remain observable.

Increase preserves owner checks, wrapping addition and Item.setAmount's clamp
before sending the amount change. Pulse processes a fixed array snapshot in
order while retaining live item fields. It skips autospawns, cleans the first AI
list reference for an expired artificial owner's item, and publishes at exactly
`decayTime - ticks == 100`. Ordinary removed items retain the original active
quirk. No lifetime or economic rules have been corrected for convenience.

`inventory::item::definition_tradeable` implements the original destroy/dissolve
guards, case-sensitive `impling jar` exception, Boolean configuration fallback
and wrong-type errors. GroundItem's shared virtual active/private hooks are used
by both the manager and chunk synchronization, so supported subtype bindings do
not disappear or become private during a new viewer's rebuild. Concrete
GroundSpawn loading/scheduling and content subclass bodies remain separate.

`direct_frames` preserves the private packet's fixed zone-origin26 followed by
item33/240/14 in the same original IoBuffer. It returns two valid native Frames
for the bounded session writer. Shared `scene_offset` retains both original
last-scene reads and callback order without allocating an intermediate encoded
packet. Queued flags retain GroundItem handles; exposed Item views must use
`ground_item::as_item` to retain the owning subtype and dropper without a cycle.

The JVM oracle compiles the complete unchanged GroundItemManager and GroundItem,
verbatim chunk bodies, complete private packet send bodies and BuildItemContext,
original location-distance statements and item tradeability/action bodies.
Dependency adapters explicitly cover definition fields/configuration/plugins,
region lookup, actors/AI, virtual subclass dispatch, disabled ISAAC and sessions.
It records 405 scenarios / 2,755 command snapshots. Earlier 246 manager cases,
all 259 chunk-sync records and all 1,831 item-definition observations remain
byte-for-byte unchanged after extending the harness.

```powershell
cargo test -p rs09-server --test ground_manager --test chunk_sync
cargo xtask ground-manager-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

Native regressions additionally deliver manager packets byte-by-byte through
the shared client decoder/reducer and check owner-only changes and queued
public changes. An ownership regression retires an actual canonical actor while
manager/chunk/flag/Item views retain it, then verifies release after the final
view is dropped. These are recording and component tests, not live world-entry,
inventory pickup, ground-spawn population or player-capacity evidence. Original
player/activity/plugin services, subtype scheduling, all-viewer fanout/reset and
the authoritative Take/inventory path still need live bindings.
