# Canonical player field ownership

The owning State actor now stores one native player-field record. This record
contains the currently bound inventory/equipment data containers, Settings,
definition table, interface manager/counter, fresh familiar flag and prepared
body/masks. Binding happens once at native admission. Reads require the original
canonical node lease and actor domain; matching numeric IDs or wire indices do
not substitute a new player. UI rows retain views of this record.

A retained InventoryListener can recover these fields after disconnect and UI
metadata retirement. The actual callback services live in the library and are
shared by tests and the live examples. Operations capture receivers and release
record guards before callbacks. A listener belonging to another player in the
same world resolves that exact player's fields. Foreign-world receivers fail
before field mutation.

The original listener has an unusual valid-input distinction. Its packet array
and familiar-pouch scan use the container supplied to the callback. Weight reads
the listener's player's own inventory/equipment; counter and appearance changes
also belong to that player. The new 88-case JVM corpus compares both players'
fields and the exact recipient-tagged original packets, including callbacks on
equipment data containers, negative/fractional/nonfinite weights, wrong types,
counter wrapping and failed prefixes. The 184 previous joined cases stay intact.
Full original EquipmentContainer behavior is not substituted by these data tests.

The actor collector traces the supported player graph alongside existing
movement, task, scenery and ground/dropper edges. It distinguishes internal node
fields from external roots and counts each actual listener/mask object once.
An external inventory/list reference follows its current listeners. An empty
base inventory or list does not by itself retain a player. A captured listener,
typed receiver, Settings/interface/mask view or native player record does retain
the original fields. Disconnected mutual listener cycles and self-facing mask
cycles release after their last external root drops.

Listener implementations report all stored strong node fields through pure
ownership metadata. No record/list lock spans that inspection. Appearance
payloads retain the actual body data as an internal edge; copying the returned
view's owner anchor into that payload would create a permanent native cycle.
The regressions exercise this update-then-disconnect case in the real UI binding.

Run owned_player, shared_ownership, container_listeners/container_ownership,
player_inventory/inventory_listener/settings, actor/ground/mask tests, headless
and all three example targets. Reproduce owned-player-oracle and affected
existing corpora; broader gates are specified in AGENTS.md.

The record also holds a once-bound profile (details, skills, saved data, timers,
Properties, spell book, persisted state, Player's scalar fields) from which the
save file is written and into which it is loaded; see canonical-player.md.

This record is the supported native admission component, not the complete Java
Player class or profile loader/save system. Full Settings/movement and skills/
Animator field joins, equipment/bank subclasses and listeners, other plugin
graphs, familiar gameplay, combat/content and authoritative item actions remain
open. Collection assumes serialized owning-world mutation; concurrent weak
upgrades/parallel simulation and world-shutdown lifetime parity are unproven.
No measured performance or capacity improvement is claimed by this change.
