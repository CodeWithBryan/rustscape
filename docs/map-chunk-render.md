# Original map viewport traversal

map_chunk_render ports the complete pinned MapChunkRenderer.render traversal.
Its 13 by 13 area uses last-scene x/y and the current player's plane. The
renderer captures the previous viewport array, retains chunks by identity,
clears chunks that leave, updates retained chunks and synchronizes new ones.
Slot publication follows its callback. Ordered region sets preserve first
encounter order for viewer membership and activity calls.

The shared outer and row arrays remain observable across callbacks. Replacing
the player's current viewport does not redirect an already captured array;
replacing a row is observed by subsequent indexed reads/writes. All locks are
released before world callbacks, so reentrant mutations preserve partial
publication and exceptions. Player, chunk and region identities are retained
handles, with the owning context responsible for canonical player services.

The differential fixture runs the complete unchanged Kotlin renderer with the
original packed Location and Region viewer-list method bodies. Its 104 cases
exercise repeated frames, movement with/without a held scene anchor, plane
changes, coordinate wrapping, replaced/aliased/rebased chunks, row replacement,
whole-viewport replacement, callback failures and mutations. The adapters use
distinct backing field names so Kotlin invokes the recorded getters rather
than directly reading public Java fields. Initial adapter results and failures
are preserved in runs/map-chunk-render-first-adapter.zip.

```powershell
cargo test -p rs09-server --test map_chunk_render
cargo xtask map-chunk-render-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

Chunk update/synchronize serialization and dirty identity/reset now have native
kernels in chunk_sync (see chunk-synchronization.md). Outgoing clear packets,
concrete region activity and the live owning-player binding remain explicit
services. This port does not yet connect live scenery delivery. It must be
joined with chunk_sync and an after-all-viewers owner reset;
neither a radius broadcast nor an immediate door-ID shortcut supplies those
semantics. All source inventory statuses remain not_replaced.
