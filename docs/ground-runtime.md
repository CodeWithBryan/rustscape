# Ground request and canonical arrival

`world/ground_runtime.rs` connects the existing original ground packet algorithm
to actual `ground_options::Captured` tables and the canonical legacy scheduler.
Its required `Bindings` extend `legacy_runtime::Bindings`. The owning world must
provide real `GroundItemManager.get`, option capture, plugin and listener services;
there is no default empty content implementation or immediate pickup shortcut.

The packet algorithm's new `Lookup` interface exposes its two actual dependencies:
player location and item lookup. Existing JVM replay adapters call the same native
manager implementation through it; the packet body and fixture bytes are unchanged.
The bridge rejects foreign and nonplayer request handles before callbacks. Captured
tables must refer to the same real canonical ground target before scheduling.

Lookup precedes script reset. A missing item resets scripts and clears the minimap.
Invalid options schedule the original invalid path with the world's actual
`Option.NULL`; the already captured target avoids an extra interaction getter.
Plugins precede GROUNDITEM and ITEM listeners. Valid dispatch re-reads the receiver
after callbacks while retaining the originally selected option. Movement and arrival
use the existing shared runner and retain the exact item even after an equal ID/tile
replacement. A send/decoded request never means that an item was taken.

Composition regressions use a real original Take frame, native manager/chunk lookup,
actual default option registration and the canonical movement/legacy kernels. They
cover callback order, short circuits, invalid-option identity, foreign handles,
listener table replacement, item replacement before arrival and partial-table index
failure with its completed request prefix. Their typed handler
invocation is recorded: these tests do not claim a live transfer to a player.
The initial table fixture remained private for its original 100-tick interval even
without a dropper; the public-item fixture now explicitly sets force_visible.
Authoring expectations about completion minimap clearing and short replacement
arrays were corrected to the existing original behavior; runtime behavior was not
changed to satisfy those expectations.

The next assembly now binds canonical player value attributes/Ironman, actual
inventory/listeners and owning-world Pickup/Scene/audio into the controlled host.
Real TCP/WS actor/spectator runs observe inventory changes and both ground removals
together; see docs/live-take.md. Complete profiles, object-valued attributes and
all remaining restriction/content services remain open. The previous immutable
door demo remains available alongside the separate new Take endpoint.
