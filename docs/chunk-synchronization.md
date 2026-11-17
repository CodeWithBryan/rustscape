# Chunk synchronization and ground items

chunk_sync ports RegionChunk.synchronize, appendUpdate, drawItems, update, flag,
resetUpdateFlags and the nullable item-list getter/setter behavior. New viewers
compare static and current scenery in x/y/slot order; same-layer replacements
suppress a static removal even if the replacement is not renderable. Retained
viewers use a snapshot of the flag array, with shared flag/object/item references.
New flags queued inside a callback wait for the next snapshot. Filtering every
item flag still sends the original two-byte zone prefix when the queue started
nonempty. Buffers remain shared through callbacks/session handoff.

GroundItem-owned state preserves the original constructor ordering, copied item
fields, captured owner UID, absolute tick argument in the UID overload, wrapping
decay arithmetic and late field reads. removed deliberately makes isActive true,
and droppedBy can rebind the dropper to another player object with the same UID.
Clock draws preserve short circuits and the difference between a decay value
captured before a clock call and a field read after it. Item/Node construction,
default interaction and canonical player properties are explicit owner services.
Ground-item spawning, pickup, decay scheduling and content remain separate.

DirtyTracker records exact chunk identities, retaining them until after all
viewers have flushed. Its base reset operations commute: the pinned source has
no RegionChunk reset override or callback. Scene now puts scenery flags on the
actual chunk queue. Weak registrations keep an old externally retained chunk
distinct from a replacement at the same coordinates without retaining it
forever. Opaque binding tokens never rebind a stale or foreign scene token to a
new chunk, even when coordinates or allocator addresses are reused. Scene.updates
remains the legacy coordinate-keyed diagnostic trace; authoritative viewer
queues are on RegionChunk. Scene.clear_updates resets the
dirty queues as a separate phase, not inside a viewer update or pulse callback.

The 259-scenario oracle compiles the complete unchanged GroundItem,
ItemUpdateFlag, ObjectUpdateFlag, UpdateFlag and ChunkUpdateTracker. It uses
verbatim RegionChunk methods and outgoing writers plus original scenery
equality/layer and IoBuffer integer methods. Item/Node constructors, player
properties/clock, attributes, clipping, foreign flag bodies and sessions are
explicit adapters. The original 212 observations remain byte identical after
adding arithmetic, partial writer and callback-capture cases. Initial adapter
evidence is preserved in runs/chunk-sync-first-adapter.zip; its corrected foreign
mutation lookup now fails before a nonexistent flag can be changed.

Three native regressions cover dropper rebinding and exposed flag ownership,
foreign captures surviving a reentrant live-queue reset, and scene identity
across coordinate replacement/foreign-world rejection/old-chunk release.

```powershell
cargo test -p rs09-server --test chunk_sync
cargo xtask chunk-sync-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

Original region activity, live owning-player viewport/session binding, remaining
animation/graphics/projectile flag payloads, live owning-world ground management and
live delivery to the shared visual/headless ground-item reducer still need
integration; the reducer itself is documented in `ground-items.md`. The manager
and chunk add/remove bodies are ported in `ground-manager.md`. Unknown foreign
bodies must fail explicitly. Hosts must retain their bindings and node
captures for all queued, exposed and snapshot flag handles; keeping_from can
attach capture ownership. No live door click, ground-item gameplay or capacity
claim follows from this corpus. Source inventory statuses remain not_replaced.
