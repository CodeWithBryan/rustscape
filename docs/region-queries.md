# Native landscape and clipping queries

`world::region_queries` ports the original RegionManager getClippingFlag,
isLandscape, isTeleportPermitted, isClipped and getWaterClipFlag control flow.
`Scene::World` binds these functions to its actual Region.load, chunks, terrain
landscape arrays and shared walking/projectile flag stores. The movement host
now supplies this real permission function for ground-target navigation.

Clipping reads use absolute Location chunk coordinates. They do not follow a
ChunkFlags base which may have been changed or rebased. Region loading occurs
before hasFlags is tested. Missing flags short-circuit without allocating a flag
array; absent walking arrays default to -1 and projectile arrays to zero.

Landscape lookup repeats loc.chunk after testing its landscape array for null.
Callbacks can change the current region/chunk/array or mutate the retained
Location. The first region receiver and each array receiver have their original
lifetimes, while later reads observe current state. No guard spans a callback.
Null and jagged landscape arrays retain their original failure positions.

Teleport permission uses OR across four zero-mask checks. Clipped uses OR across
four nonzero-mask checks. Both may be true on one tile. Water first captures a
clipping value, then independently queries isClipped; callbacks in the latter
must not replace the already captured value used in the final bit operation.

1,002 original JVM scenarios compare outcomes, map contents, actual arrays,
receiver/lookup/load order, changed locations and partial callback failures.
The oracle compiles unchanged selected RegionManager bodies, original Location
getters/hash and complete ChunkFlags. Region lookup/load and chunk arrays are
explicit recording services. Snapshots copy flag values after each action.
Native integration tests use actual Scene terrain loading, underlay marks,
mutable chunk bases, missing regions and both flag stores.

Run `cargo test -p rs09-server --test region_queries` and
`cargo xtask region-queries-oracle --check --java-home runs/jdk11/jdk-11.0.32.1+1`,
plus the related gates in AGENTS.md. Full RegionManager lifecycle, dynamic-region
ownership, water-NPC supplier selection and authoritative Take are still open.
The next pickup binding must also preserve synced FaceLocation update masks and
the actual captured inventory/profile receiver; an immediate item transfer would
skip required original movement, facing and restriction behavior.
