# Prepared-player mask binding

The actual player_session, movement_session and npc_session Actors now own
`PreparedMasks`, which uses the verified UpdateMasks core. Separate animation/
spot fields and hand-built header assembly are removed. Prepared appearance
fields live in one shared object, read by both the inventory/skills bindings and
the retained mask context. Captured masks retain their actual owner/body even
after another player receives the same wire index. The renderer rejects a stale
owner rather than resolving it to the current index occupant.

The movement host's original tick-phase Event::Masks calls prepare on the same
mask instance used by the renderer. All viewers render before reset clears the
current slots. Persistent slots/stamp survive reset and are supplied to a new
viewer. Actor construction registers appearance; inventory familiar changes
register it through this same core. Controlled animation/spot probes explicitly
register appearance on their scheduled events to preserve their prior previews.
Gameplay animation registration itself adds only the animation flag.

FaceLocation retains the shared Location and uses original transformed shorts.
Clearing it unregisters only its persistent slot. FaceEntity retains the actual
canonical NodeHandle and the actual player/NPC render object; index changes are
read from that captured view. Player client indices use OR with 0x8000. A clear
registers a null context only if a persistent face was removed. During updating,
that new registration can be rejected while the old current context remains.
These captures release on reset/unregister as in the original; private host
views are removed during actual deferred retirement, including self-facing.

The 440 joined JVM scenarios compile complete UpdateMasks with a controlled
clock, unchanged Entity.face/faceLocation and Entity/Player getClientIndex,
the actual selected 530 encoder subclasses, complete IoBuffer and original
LocationHash/getters. Byte/status histories cover signed/wrapping coordinates,
indices, extended masks, animation/spot order, persistent state, target changes,
and clearing/rejected registration during preparation. Registry and immutable
animation/graphics field records are explicit adapters. The independent 396
core scenarios still cover reentrant registries, writers, impacts and failures.
Native tests add captured-body identity, canonical target retention/release,
new-viewer synchronization, stale-index rejection and actual tick/retirement.

Run `cargo test -p rs09-server --test prepared_masks --test update_masks` plus
all three example targets, and `cargo xtask prepared-masks-oracle --check` with
the oracle Python/JDK11 selections. AGENTS.md lists related checks.

This world supplies an actual, initially empty owned impact queue. Combat
production and its concrete hit encoders remain unbound; unsupported payloads
return errors. Prepared appearance encoding still depends on its supplied body
fields. Complete equipment/familiar/body preparation, all chat/force-movement/
hit providers, mutable registry startup, canonical full Player/profile lifecycle
and authoritative arrival/Pickup are not established by these tests. Actual
Animator/definition/clock binding is covered separately in docs/animator.md. Existing
endpoints and sealed visual packages are not changed by source-only tests.
