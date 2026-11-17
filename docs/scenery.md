# Native scenery replacement and server collision

`server::world::scenery` ports SceneryBuilder's replacement, addition, removal,
update and four captured pulse bodies. `server::world::clipping` ports the
server ChunkFlags masks and mutation order. `world::chunk` supplies RegionChunk
scenery storage, copies and rebasing. `world::landscape` applies placement/removal
and decodes location streams over the native storage and clipping. These run
entirely in Rust. The native [scene](scene.md) connects these kernels with region
loading, door rules and update packets. World startup, authoritative player
movement and live door actions remain separate integration work.

## Replacement and restoration

The builder keeps original wrapper identity and constructed-object behavior.
Clipped replacements remove the current landscape object, stop and clear its
restoration pulse, and either restore its previous object or add the new
constructed object. Temporary replacements retain the removed handle and assign
the restore pulse before submission. Errors keep all prior effects. Negative,
zero and positive restoration delays preserve their different source branches.
Client-only replacements queue removal/addition updates without changing clipping.

Addition, expiration and respawn preserve their distinct captured references.
Expiration checks retain the original unwrapped object; ground-item arrays remain
shared and are read per callback. A respawn resolves its captured object's wrapper
when the body runs. Deactivation calls an existing destruction pulse directly
before assigning the active flag, so callbacks may fail or mutate the object.

Object update records retain the object handle and the removal boolean captured
when queued. Packet serialization reads current ID, location, type and rotation.
Chunk selection happens before reading the active flag. The original outgoing
writers and enclosed-zone writer are used by the oracle; its scene-coordinate
adapter is explicit and does not establish player-relative scene origins.

## Collision flags

Server movement storage starts unknown (-1); projectile storage starts clear.
Adding a mask uses max(0, old) OR mask. Removal first checks the old bits and then
uses max(0, old) AND NOT mask. This differs from client CollisionMap defaults.
Flat indexes use Java wrapping and x*8+y. Border operations forward absolute
coordinates and preserve callback order; actual neighboring-region routing is
the context's responsibility.

Straight, corner and diagonal walls preserve rotation, neighbor masks and the
complete movement-mask phase before the projectile-mask phase. Solid objects,
floor blocking and landscape marker aliases are retained. Invalid door shapes
and rotations are no-ops. Native object rectangles exceeding 65,536 writes are
rejected to bound malformed work; original huge loops are not oracle inputs.

## Verification

```powershell
cargo test -p rs09-server --test scenery --test clipping --test chunk --test landscape
cargo xtask scenery-oracle --check --python <python> --java-home <jdk11>
cargo xtask clipping-oracle --check --python <python> --java-home <jdk11>
cargo xtask chunk-oracle --check --python <python> --java-home <jdk11>
cargo xtask landscape-oracle --check --python <python> --java-home <jdk11>
```

There are 240 scenery, 1,244 server-clipping, 269 chunk-storage and 179 landscape
scenarios. They compare
state, shared references, exact update bytes, callback order and partial errors.
The scenery oracle compiles complete unchanged SceneryBuilder, Scenery,
Constructed, Pulse, ObjectUpdateFlag and original packet writers with explicit
landscape/chunk/definition/entity/ground-spawn adapters. The clipping oracle
compiles complete unchanged ChunkFlags with explicit flag-storage/border adapters.
Snapshot arrays are copied at observation time so later mutations cannot rewrite
earlier expected observations. Fixtures pin original and harness source hashes;
CI reproduces these corpora. Reference sources remain unmodified.

The chunk oracle uses verbatim constructor, storage, index, copy, rebase and
rotation-coordinate methods. Static insertion retains its argument and creates
a fresh dynamic copy; duplicate static IDs are ignored. Dynamic insertion either
stores a constructed object or reactivates the existing equal instance. Capacity
is four per tile. Copying resets flags and defaults while retaining locations
until rebasing replaces those references. Constructor failures preserve an
already-written static slot.

The landscape oracle compiles unchanged LandscapeParser/ChunkFlags plus the
original storage methods and smart decoders. Placement activates the argument,
optionally stores it, then clips. Removal locates the current stored object by
ID (ignoring type for nonnegative IDs), deactivates current and argument, clears
the slot, and unclips using the argument's definition. Floor landscape markers
remain set on removal. Parsing preserves object-count overflow, bridge-plane
adjustments, skipped planes and partial buffer positions. The native definition
view now uses the shared native cache definition type described in
[scenery-definitions.md](scenery-definitions.md).
Region loading/routing and child actions remain explicit adapters. Reentrant
landscape/world callbacks are not covered by this initial landscape corpus.

The native `world::mapscape` terrain and floor-clipping loops add 343 separate
JVM scenarios. Every map byte, both source/destination chunk tables and landscape
arrays, and every allocated movement flag enter a canonical big-endian SHA-256
observation. Getter/flag counts and read positions are compared too. This covers
all 256 terrain byte values, bridge planes, unknown flag normalization, existing
wall bits, null regions/chunks/landscapes, shared marker arrays, separate source
and destination chunks, and callback mutation/failure prefixes. The oracle uses
unchanged MapscapeParser/ChunkFlags with explicit region/storage/location records.
Terrain parsing consumes null chunks' bytes and discards their marker writes.
Floor clipping normalizes unknown tiles without erasing existing wall/object
bits. Region/cache delivery and ownership remain separate native integration.

```powershell
cargo test -p rs09-server --test mapscape
cargo xtask mapscape-oracle --check --python <python> --java-home <jdk11>
```

Native constructors, Region.load and the joined scene now connect these kernels
with door rules, builder pulses and update packets. Their separate differential
corpora and real-cache audit are described in [scene.md](scene.md). The earlier
kernel corpora retain explicit adapters for their narrower isolation tests.
Full Scenery/Node interaction APIs, dynamic-region rotation, item/entity lists,
chunk synchronization, walking, entity startup, the 600ms world driver and live
world entry remain incomplete.
