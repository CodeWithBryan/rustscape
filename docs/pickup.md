# Ground-item pickup

`world::pickup` ports the complete control flow of the pinned PickupHandler.take,
PickupHandler.canTake and PickupPlugin registration/handle/destination methods.
It uses the existing native ground manager, item/subtype state and packet paths.
Player attributes, inventory receivers, restrictions, plugins, dialogue, logging,
animation, audio and event delivery are mandatory host services. No empty
production implementation of those services is supplied.

Take checks location and manager membership before reading the drop deadline.
GroundSpawn is a subtype check, independent of virtual auto-spawn behavior.
Ordinary remain-private items check the cached owner UID. A new Item captures
the ground ID/amount/charge before inventory space checking, while later plugin
callbacks may mutate the ground item. The second inventory receiver is read
after canTake and isActive, immediately before adding the captured new Item.
Inventory membership and ground removal are separate ordered operations.

canTake preserves ironman restrictions, the two guild items, sacred cape and
rune-pouch checks, and separate hasItemPlugin/getPlugin calls. A callback can
clear or replace the plugin between those reads. `pickup_types` ports the original
GodType.forCape/hasAny and RunePouch.forItem lookup bodies over canonical mutable
Item handles. These retain enum declaration order, late cape reads after player
queries, and the small-pouch exception for decayed IDs. Other cape/pouch content
such as prayer, rune storage, charges and dropping is still unported.

After a successful inventory add, the handler reads the late dropper/UID/name
fields for drop-trade logging, checks teleport permission, optionally animates535,
destroys the ground item, plays the pickup sound and dispatches PickUpEvent.
A failure retains its completed prefix. In particular, inventory insertion is
not rolled back when a later permission/log/destroy/audio callback throws.
PickupPlugin removes the pickup attribute only after take returns normally.
Any existing pickup key short-circuits, including false or null values.

The JVM corpus compiles the complete unchanged PickupHandler and PickupPlugin,
joined to the original GroundItem, manager, GroundSpawn, PulseRunner and outgoing
item packets. It contains 800 scenarios covering restrictions, integer/time
boundaries, nulls, duplicates, captured receivers, callback reentry, registration
and failed prefixes. Inventory space/add results and player/content delivery are
explicit recording services. Two native regressions additionally use actual
Container objects and their event flags, checking charged/stackable transfer,
insufficient space, no duplicate transfer from a second actor, failures after add
and respawning the same canonical world item. They do not replace the original
InventoryListener or establish live player-container delivery.

The pinned bundled ConstLib1.4 jar lacks the PICK2_2582 symbol referenced by the
handler; the source POM allows newer external versions. The oracle records that
jar and its Sounds.class hashes in consts-evidence.json and uses an explicit
numeric2582 adapter. This is not verification of the missing external library.
Locking/verifying that dependency remains necessary for full constants parity.

```powershell
cargo test -p rs09-server --test pickup --test inventory --test ground_manager --test ground_spawn
cargo xtask pickup-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

The controlled examples now bind the actual packet, canonical ground target,
default option and movement/arrival path to the real player inventory/listeners,
value attributes, Ironman manager and owning-world ground/Scene operations.
Finite TCP/WS actor/spectator Take runs succeed on a separate native endpoint;
see docs/live-take.md. Full bank/dialogue/item plugins/audit/content, profiles,
Drop and missing original sound-constant provenance remain explicit work.
