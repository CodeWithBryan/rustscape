# Item, NPC, player, use-with, spell, slot and examine packets (`world/packet_handlers`)

`crates/server/src/world/packet_handlers/` ports the branches of
`PacketProcessor` that the tick driver used to refuse, and the core handlers
those branches reach. Each is a library over a `Context` trait (a shared
`Base` plus one per branch); every call outside the ported bodies is a
required method with no default. Content (per-item, NPC and object
listeners, plugins, `UseWithHandler` subclasses, familiar specials, pets,
graves) stays content: it is reached through those services, never ported or
fabricated here.

## What is ported

| Original | Rust | Scope |
| --- | --- | --- |
| `processItemAction`, `getLikelyContainerForIface` | `actions.rs`, `mod.rs` | container by interface (149, 762, 667), slot range, id check, the options array, the interaction lock, the second `item.interaction` read, scripts reset, debug |
| `InteractPlugin.handleItemOption` and its pulse (`InteractPlugin$1`) | `item_option.rs` | the lock, `clear(STANDARD)`, the submitted pulse's captures (the plugin's own node), the zone veto, ITEM listeners, the option handler with the default-locale name, "Nothing interesting happens.", the handler debug line, the catch that logs `InteractPlugin$1` plus the message |
| `processNpcAction` | `actions.rs` | the `1 until MAX_NPCS` range, `getShownNPC`, invalid options (handleInvalidInteraction with `Option.NULL`), the six debug lines (spawn location, wrapper varbit), scripts, NPC listeners keyed by the shown id but given the NPC, `PluginInteractionManager`, the NPC's own interaction handling the shown NPC's option |
| `processPlayerAction` | `actions.rs` | range, scripts before the lookup, the active check, the ROOT `lowercase()` option name for PLAYER listeners, then `interaction.handle` |
| `processUseWith`, `NodeUsageEvent`, `UseWithHandler` | `use_with.rs` | all five packets (container lookups, which misses clear the minimap and which return silently), id checks, both locks, scripts, zone, listeners on the node then on a differing shown NPC or scenery child, the item-on-item flip by id, the plugin manager; the handler registry (`addHandler` keys, the PLAYER_TYPE array growth on the shared handler, `nodeAllowed` with `isDynamic` and a null array), `run`'s lookups per target class, the unhandled action, the item-on-item `Pulse(1, ...)` and the two MovementPulses, and their three bodies (`UseWithHandler$1` to `$3`) walking the live handler list |
| `processComponentUseWith`, `SpellUtils.getBookFromInterface`/`validateSpellbookInterface` | `component.rs` | the five targets (ground item "Too late!", NPC range), the id check, `magic:delay` against `GameWorld.ticks`, the spell book check for 192/193/430, `SpellListeners.run`, `MagicSpell.castSpell` (with the native `SpellBook`), the familiar special on 662 |
| `processSlotSwitch`, `switchItem` | `slot_switch.rs` | bank drag to the item area (insert mode), bank tabs (`BankContainer.getArrayIndex` is the native `inventory::bank::array_index`, tab 10 and free slot, the remove-and-add when there is no room, both insert directions, tab start updates), inventory moves; `switchItem`'s bounds, the chatbox close and swapped recursion with refresh, swap or insert, the index writes |
| `ItemExamine`, `SceneryExamine`, `NpcExamine` branches; `Definition.getExamine`, `ItemDefinition.getExamine`, `NPCDefinition.getExamine`, `StringUtils.isPlusN` | `examine.rs` | over the native `definitions::{item, scenery, npc}` handles: the cached examine field, the `examine` handler's `toString`, "It's a(n) NAME.", the note text that overwrites the cache, the NPC configuration's cast and its one-character null, the scenery varbit looked up twice |
| `TrackFinished` branch | `examine.rs` | looping replays, otherwise playing is cleared; `track_finished_native` runs it on `world::player_tabs::music::MusicPlayer` |
| `EquipHandler` | `equip.rs` | registration (`equip`, `wield`, `wear`), `handleEquip` (slot configuration with its unboxing cast, the same-object and "goblin mail" returns, the unequip listener of the current item, the equipment plugin's `fireEvent`, the equip listener, the equipment lock and message, `EquipmentContainer.add(item, slot, true, true)`, brawling gloves, the dialogue close, the configured or default equip sound, the weapon-slot autocast reset, slayer flags, `ItemEquipEvent`), `unequip` |
| `DropListener` | `drop.rs` | registration (`drop`, `destroy`, `dissolve`), `handleDropAction` (pets, graves, the equipment lock, closing interfaces, the soft queued script; destroy, dissolve and the `destroy` configuration with its cast), the script (identity checks, the dupe alert text, the drop item, coin and drop sounds, the ground item, `droppedItem:ID`, the save) and the destroy dialogue action |

The driver routes these packet types to new `Handler`s
(`world/driver/dispatch.rs`): `ItemAction`, `NpcAction`, `PlayerAction`,
`UseWith` (all five), `ComponentUseWith` (all five), `SlotSwitch` (both),
`Examine` (all three) and `TrackFinished`. Each `from_action` turns the
decoded `ClientAction` into the library's request. Binding the handlers to a
live world (`Host::handle`) is the `--world` server's work.

## Behaviour that is easy to get wrong

- Item action: the slot is checked against `capacity()` before `get`, but
  `get` itself answers null outside the array; an option index outside the
  options array fails as the array does. `item.interaction` is read twice;
  the pulse targets the InteractPlugin's own node, not the clicked item.
- The item pulse catches every exception: a null option name fails the
  listeners call (Kotlin's parameter check) and is logged as
  `core.game.interaction.InteractPlugin$1null`.
- NPC action: the listeners get the NPC but the shown NPC's id; the plugin
  manager gets the shown NPC; the NPC's own interaction handles the option.
- Use-with: an item-on-item use whose used id is lower than the target's is
  flipped. NPC, scenery and player misses clear the minimap; item and ground
  item misses return silently. `Repository.npcs[i]`/`players[i]` fail outside
  `1 until length` (NodeList), which use-with does not check first.
- `UseWithHandler.run` swallows every exception (printStackTrace only); its
  pulses do not: a null `allowedNodes` fails there. The item pulse's
  condition is `(!allowed(with) && !allowed(usedItem)) || !handle`; the
  movement pulse's is `!allowed(used) || !handle`. Handlers registered after
  the pulse was scheduled are walked (the list is live).
- `addHandler` with PLAYER_TYPE appends the id to the shared handler's array
  (a null array becomes `[id]`), so one object accumulates ids.
- Component use: a ground-item miss sends the minimap clear and then
  "Too late!" through ContentAPI's `sendMessage`; a mismatched target id
  returns without a clear; `magic:delay` must be strictly greater than ticks.
- Slot switch: dragging onto a tab with no free slot removes and re-adds the
  item; `secondSlot - 1` for a later slot; a missing source item fails
  Kotlin's null check. `switchItem` closes an open chatbox (unless
  `close_c_`) and recurses with the slots swapped, then refreshes.
- Examine: `getExamine` caches into the definition, and the item's note text
  overwrites that cache (a later unnoted read still gives the note text). An
  NPC name of one UTF-16 unit gives a null message. `isPlusN` takes "an" for
  an `h` not followed by `e` ("It's an Hans.").
- Equip: the slot configuration's cast happens before the equipment read; a
  plugin answer other than `true` refuses; unequip's `as Boolean` fails on
  null (Null) and other objects (Cast). The equip sound's configuration is
  read after the add and its cast fails there.
- Drop: only the presence of an equipment lock refuses (not `isLocked`); the
  script compares identities, and its replace fires listeners.

## Required services

`Base`: node fields (id, class, index, active, toString text), the
default-locale lowercase, debug/messages (Player.sendMessage,
PacketDispatch.sendMessage and ContentAPI.sendMessage are distinct), the
minimap clear, scripts reset, locks (interaction, movement, equipment lock and
its message), player location, `getInteraction`, the options array,
`InteractPlugin.handle`/`handleInvalidInteraction` (`world::legacy_interaction`),
`InteractionListeners.run(id, type, option, player, node)`, container
`capacity`/`get`/`replace`, `Repository.npcs`/`players`, `getShownNPC`,
`GroundItemManager.get`, `ServerConstants.MAX_PLAYERS`/`MAX_NPCS`,
`Item.getDefinition()`, `playAudio`, and `InteractionListener.on` at
registration. Each branch's `Context` adds its own: the item pulse's
submission, zone and option handler; NPC spawn/varbit/plugin; use-with's
object/child/zone/listener/plugin, the handler registry (`Registry` is the
native one), pulses and facing; component's object, attribute, spell book,
spell listeners, `castSpell`, familiar; slot switch's container and bank tab
members; examine's definition lookups, varbits and music player; equip's
listeners, plugin `fireEvent`, `EquipmentContainer.add`/`remove`, inventory,
gloves, dialogue, autocast/WeaponInterface, slayer flags, events, strings;
drop's used option, pets, graves, interfaces, queued script, stopExecuting,
PlayerMonitor, drop item, ground items, ticks, attributes, save, destroy
dialogue and `removeItem`.

## Reference tests

```powershell
cargo test -p rs09-server --test packet_handlers --test tick_driver
cargo xtask packet-handlers-oracle --check --java-home <jdk11>
```

`oracle/packet-handlers` compiles the unchanged `UseWithHandler`,
`NodeUsageEvent`, `EquipHandler`, `DropListener` and `DialogueAction` on
JDK 11 with Kotlin 1.8.20 and the pinned ConstLib 1.4.28 (Components, Items
and Sounds constants checked against their names). Original text inserted
into the adapters at generation time: the nine selected `PacketProcessor`
methods and the four `process` arms, `InteractPlugin.handleItemOption`, the
three `getExamine`s and `getConfiguration(key, fail)`,
`SceneryDefinition.getConfigFile`, `StringUtils.isPlusN`,
`BankContainer.getArrayIndex`, `Item.toString`, SpellUtils' two members,
`SpellBook.forInterface` with its constants, the EquipmentContainer slots and
the ItemConfigParser/NPCConfigParser keys. Everything else is a recording
adapter answering from the case. The adapters' fields are named apart from
the getters' Kotlin properties so the original Kotlin goes through the
getters.

`fixtures/packet-handlers/oracle.jsonl`: 3,314 cases (3,047 designed, 267
with one recorded call failing in turn), 6,025 operations, 28,497 recorded
calls. Designed groups: item actions over every container interface, slot
edge and id, option indexes, locks and every pulse outcome (zone, listeners,
both handlers, null and Unicode names, replaced interactions, injected
failures); NPC actions over the index range, wrappers, listeners/plugin,
spawn and varbit lines, late ids; player actions over the range, activity,
names and listeners; every use-with packet's lookups and misses, locks,
zone, listeners on node and child, flips, ten handler registrations against
six targets with three answer patterns, PLAYER_TYPE growth, late
registration, failures in `run` and in the pulses, `run` called directly
with non-item used nodes (the Item cast); component use for five
targets on five interfaces, both spell books, familiars, magic delays and
every miss; bank and inventory slot switches over tabs, free slots, insert
modes, bounds and the chatbox; item, scenery and NPC examines over sixteen
names, configured texts of three types, noted items, varbit answers and
missing definitions; track finished; equips over slot configuration,
current items, names, plugin answers, listeners, locks, gloves, autocast,
slayer and sounds; unequips; drops over every option and destroy flag,
pets, graves, locks, the script's identity checks and dupe alert, the
dialogue action and registration.

`crates/server/tests/packet_handlers.rs` replays every operation through the
native libraries with a recording world that follows the adapters, and
compares the result, the error, every recorded call and the state after
each operation (containers, node indexes, cached examines, handler arrays,
chatbox and autocast flags). Every case matches; there are no pinned gaps.
The same file checks the driver's routes and each `from_action`, runs
`TrackFinished` on the native MusicPlayer and the examine texts on native
definitions. Thirty-five deliberate breaks of the port were tried (re-reads,
listener targets, the ROOT lowercase, the flip, `nodeAllowed` logic,
PLAYER_TYPE growth and key, the Item casts in `run`, the swallowed `run`
exceptions, facing, tab arithmetic and tab 10, replace flags, the swapped
recursion, `isPlusN`, the note text, the double varbit lookup, the equip
sound, same-object and weapon-slot checks, the free-space test, the drop
lock and replace, the destroy button, the magic delay and spell-book list,
the Too-late clear, the NPC and slot ranges, the item pulse's lock, message
and log): 34 fail the replay. The one that does not removes `run`'s
`!(used instanceof Player)` test for an item target; it is unobservable,
because the original casts the used node to `Item` before that test and a
player fails the cast (the direct `run` cases pin that cast).

## Differences from the original

- `switchItem` stops with an error after 1,024 chatbox-closing recursions;
  the original recurses until the chatbox closes (a binding that never
  closes it overflows the JVM stack).
- An `examine` handler value without a native `toString` (anything but
  text, integers and booleans) is an error instead of its Java text.
- Errors carry a category and message, not a Java stack trace;
  `printStackTrace` is not a service.
- In the corpus, `Container.replace` stores the object it is given (the
  real Container stores a copy); the native binding uses
  `inventory::container`, whose own corpus covers the copy.

## Not ported or not joined

- `EquipmentContainer.add`'s body (requirements, two-handed and shield swaps,
  ammunition, the full-inventory refusal, the equipment listener and the
  bonus refresh) is `inventory::equipment` (equipment-oracle); here it is the
  `equipment_add` service, so no corpus runs `EquipHandler` over the real
  EquipmentContainer yet.
- `UseWithHandler.getValidChildren` and `getDestination`, the deprecated
  `InteractionListener` members, and every content listener, plugin and
  `UseWithHandler` subclass.
- Nothing binds these libraries in a live world; the `--world` server binds
  the routed handlers.
