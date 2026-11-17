# Native region and scenery integration

`server::world::scene` connects the native cache provider, retained scenery
definitions and constructors, Region.load, terrain/landscape parsers, chunk
storage, movement/projectile collision, door rules, builder pulses and outgoing
object updates. It is a simulation library; server world entry remains pending.

The scene has one simulation owner. Entity/path/audio/ground-item services are
explicit callbacks and may reenter the scene. Object/cache/queue locks are released
before those callbacks. Region-border clipping loads the destination region and
uses its hasFlags state. A missing map returns unknown collision for both movement
and projectiles. Actual 600ms scheduling, entity ownership, chunk fanout and
network world delivery still need implementation.

Scenery factories retain the constructor's definition, name, size and direction
with the original live-versus-captured behavior. Morph children are distinct
objects even for duplicate definition IDs. Owned child views retain their family;
weak parent links avoid cycles. The public child view is read-only. General
reflection/mutable child-array APIs, concrete interaction dispatch, and malformed
recursive/cyclic families are outside the current port. Depth and allocation
bounds reject malformed construction rather than exhausting the runtime.

Scene pulse metadata uses weak object references. Queued pulses and public
restore-pulse views explicitly own their captures, while raw object restore fields
avoid ownership cycles. Tests cover removal before expiry, retention beyond queue
execution, release after dropping the last view, and pruning completed metadata.

The following independent original JVM corpora cover this integration:

| Corpus | Scenarios | Scope |
| --- | ---: | --- |
| scenery-node | 176 | Complete original Scenery/Constructed with retained definition, morph, geometry and config observations |
| region-load | 206 | Verbatim Region.load overloads with explicit cache/parser/service callbacks, failures and nested mutations |
| scene | 37 | Joined original parsers, scenery, doors, builder, collision and outgoing writers over synthetic cache bytes |

The joined corpus compares stored static/dynamic scenery, retained named handles,
all allocated movement/projectile flags, region state, pulse state, service events
and complete wire bytes. It opens doors across chunk/region boundaries, verifies
tick 499 versus 500, closes early, and tests removal, respawn, expiry and client-only
replacement. The JVM uses original classes and verbatim region/chunk methods;
single-owner region routing, fixed-prefix pulse scheduling, player/path/audio
services and dirty-chunk delivery are explicit harness adapters. Separate kernel
corpora retain mutation/error coverage beyond these successful joined traces.

```powershell
cargo test -p rs09-server --test scenery_node --test region_load --test scene
cargo xtask scenery-node-oracle --check --python <python> --java-home <jdk11>
cargo xtask region-load-oracle --check --python <python> --java-home <jdk11>
cargo xtask scene-oracle --check --python <python> --java-home <jdk11>
cargo run --release -p xtask -- scene-audit --python <python> --java-home <jdk11>
```

`scene-audit` loads all native definitions from strict archive 16 and provides real
terrain/landscape bytes through strict archive 5, using pinned XTEA data. It
compares the actual Lumbridge door at (3207,3217,0), opening to ID 10327 at
(3207,3218,0) and restoring after 500 ticks, against original JVM methods over the
same decrypted bytes. Nearby region inputs are supplied for border loading.
Terrain groups must have exactly file zero to match the original container-read
path. Cache decompression/XTEA itself has its separate complete JVM cache audit.

A successful audit writes input/output observations and source/build/asset hashes
to `runs/scene-oracle/audit.json`. It exports server-produced ordinary revision-530
frames without ISAAC in `lumbridge-door.s2c`, with action/tick/chunk/offset/length
metadata in `lumbridge-door.json`. This supports client replay of the exact server
output. It does not establish a live visual/headless session, authoritative player
movement or audio delivery, dynamic regions, complete content or player capacity.

The Windows visual client has replayed this exact 26-byte native trace. GPU
captures and bounded scene readbacks under `runs/visual/server-door-*` show
ID 12348 at (3207,3217), rotation 1, changing to ID
10327 at (3207,3218), rotation 2, then restoring. Collision values at the three
affected tiles match the audited server state, including the existing neighboring
wall. Client capture frames 25/75/125 use accelerated packet receipt for inspection;
the authoritative trace metadata retains server ticks 0/500. This proves packet
consumption/rendering for this door, not live network sessions or server-driven
player movement. Browser/WASM captures under `runs/visual/wasm-door-phase-*`
also visibly show closed/open/restored states using the same unchanged server
bytes embedded in a separate replay pack. Default pack inputs remain unchanged;
the optional repeat is presentation playback. `tools/check_door_replay.py`
checks native readbacks, and `tools/prepare_door_replay.py` verifies the exported
wire SHA before embedding it for either client target.
For a fresh isolated report, JVM build and wire export, use
`scene-audit --output runs/NEW_AUDIT_DIRECTORY`. An explicitly selected output
must be empty, preserving prior review evidence. The existing default path
remains compatible.
