# A player's death, kept items and graves

`world/player_death` ports what happens when a player dies: the items kept on
death, `Player.commenceDeath`/`finalizeDeath`, where the lost items go (a grave
or the ground), the grave with its timer, blessing, repair, reading and
demolition, the server-store copy of active graves, and the death-screen
button. They are libraries; `rs09-server --world` installs `commenceDeath`,
`finalizeDeath`, `getContainers` and the death screen
(world/live/combat/player_death.rs, docs/live-world.md); graves refuse there
by name (no grave NPC is built at runtime). Java is only the test oracle.

| Original | Rust | Scope |
| --- | --- | --- |
| `DeathTask.java` | `player_death/kept.rs` | `getContainers`. `start`, `exec`, `stop` are `world/npc_death`'s, now exercised over a player too |
| `Player.java` | `player_death/death.rs` | `commenceDeath`, `finalizeDeath` |
| `Entity.java` | `npc_death::entity_finalize_death` | `finalizeDeath`, reached from a player's death unchanged |
| `Grave.kt` | `player_death/grave.rs` | the whole class except `construct`/`getIds` (NPC factory) |
| `GraveController.kt` | `grave.rs`, `store.rs` | `tick`, the four option handlers, `produceGrave`, `allowGenerate`, `getGraveType`, `shouldCrumble`, `shouldRelease`, `checkTransform`, `serializeToServerStore`, `deserializeFromServerStore`. Not `updateGraveType`, `hasGraveAt` or the `forcegravedeath` command |
| `GraveType.kt` | `grave.rs` (`GraveType`) | every constant, `ids` |
| `BarrowsEquipment.kt` | `player_death/barrows.rs` | the 24 pieces, `getDefinition`, `isBarrowsItem`, `isFullyRepaired`, `isBroken`, `graveDeathDurabilityReduction`. Not the repair cost |
| `DeathInterface.kt` | `death.rs` (`death_interface_button`) | the button handler |
| `TickUtils.kt`, `Globals.kt` | `player_death/mod.rs` | `secondsToTicks`, `ticksToSeconds`, `colorize(line)` |
| `Permadeath.kt` | not ported | a required service (`Host::permadeath`) |
| `GravePurchaseInterface.kt` | not ported | interface content |

## What is joined to native pieces

- The worn, kept and lost containers and the player's inventory and equipment
  are native `inventory::container::Container`s with their listeners;
  `getContainers` uses the native `add`/`addAll`/`replace` and alchemy value.
- Every ground item (bones, drops, grave items, store graves) is made, looked
  up and removed by the native ground manager (`create_at`, `create_uid`,
  `get`, `destroy`, `pulse`) and `ground_item` state (`set_decay_time`,
  privacy, activity).
- The death task over a player is `npc_death::task_start`/`task_pulse`/
  `task_stop`; their `player_commence_death`/`player_finalize_death` services
  call `commence_death` and `finalize_death`. `finalizeDeath`'s
  `super.finalizeDeath` is `npc_death::entity_finalize_death`.
- Item ids, names, options, notes, tradeability and alchemy values come from
  native item definitions; `Mode` is `player::ironman::Mode`; the active-grave
  map keeps Java `HashMap` order through `java_map::int_key_layout`, archive
  keys through `string_key_order`; item names are lowered with the native ROOT
  rules.
- The grave type is the `/save:gravetype` attribute read through
  `attribute_int`; a binding answers it from the canonical attributes that
  `player::persisted::graves` saves and parses.

## Evidence

`cargo xtask player-death-oracle` compiles `DeathTask`, `Item`, `Container`,
`ContainerEvent`, `ContainerListener`, `ContainerType`, `SortType`,
`Definition`, `NodeTask`, `Pulse`, `PulseType`, `GroundItem`,
`GroundItemManager`, `Grave`, `GraveController`, `GraveType`,
`BarrowsEquipment`, `DeathInterface`, `TickUtils`, `Globals`, `Quests`,
`IronmanMode`, `Rights`, `ZoneType`, `ZoneRestriction`, `MapDistance` and
`StatAttributeKeys` unchanged with Kotlin 1.8.20 and runs them on JDK 11.
`Player.commenceDeath`/`finalizeDeath`, `Entity.commenceDeath`/
`finalizeDeath`, `RegionChunk.add`/`remove(GroundItem)`, the `Location`
string, parse and distance members, the `ImpactHandler` accessors, the
`Animator` reset constants and the `ItemDefinition` fields and item methods
are taken from the pinned sources when the oracle is generated and inserted
into recording adapters (the fixture's `scope` names them all). Everything else
an adapter does is a recorded call answered from the case.

The corpus has 938 cases with 3,216 actions; the original makes 62,576
recorded service calls, creates 1,987 ground items and 161 graves and runs
9,101 driver ticks:

- 708 kept-item selections: 21 fixed inventories (ties, stacks, notes, charges,
  zero, negative and maximum values, configured alchemy) under all 20
  combinations of skull, Protect Item and ironman mode (null, none, standard,
  hardcore, ultimate); inventory plus equipment with duplicates; full
  inventory and equipment; 260 seeded random sets.
- 122 death sequences (281 actions): killer absent, self, player (active,
  inactive, same name), NPC, artificial victims; retribution; zone handling,
  safe flag, safe and other zone types, rights; every ironman mode with and
  without `permadeath`; every reason a grave is refused with player, NPC and
  no killer; magic levels around 55; original location; all 13 grave types
  and out-of-range or mistyped saved types; plugins without a death item;
  graves with nothing left; the whole `DeathTask` start/exec/stop over a
  player with death animations of 0, 1, 4, 30 and 31 ticks; second deaths.
- 86 grave lifetimes (1,292 actions): every type to expiry from four start
  ticks with owner, other-player and relogged-owner lookups at each boundary;
  three other driver orders; the decay tick near `Integer.MAX_VALUE`;
  logout and relog with takes; blessing and repair refusals and amounts;
  `addTime` around both threshold pairs; reading every type and formatted
  durations from -101 to `Integer.MAX_VALUE`; demolition by others, owners and
  absent owners; six graves in map order including a grave at -1; ground drops
  without a grave at ticks 0, 99, 100, 101, 199, 200 and 201 after death.
- 10 server-store cases (40 actions): parse, save and run with missing,
  mistyped, out-of-range and escaped fields and colliding key hashes.
- 12 static cases (895 actions): `shouldCrumble`/`shouldRelease` around every
  listed id, `checkTransform`, the `GraveType` table, all 202 ids from 4706 to
  4761 and 4854 to 4999 at four charges through `BarrowsEquipment`, the death
  screen button and the saved grave type.

Each action records its result or error category (`Null`, `Type`, `Index`,
`Number`), the ordered service calls and afterwards: every player's inventory
and equipment slots, attributes, disabled-until tick and prayer points; the
ground manager list and chunk list; every ground item ever seen (id, amount,
charge, location, dropper, dropper uid, creation and decay ticks, private and
removed flags); the active graves in map order; every grave's NPC id,
location, owner, username, ticks remaining and since death, type, items and
attributes. `crates/server/tests/player_death.rs` replays every case natively
and must produce the same; it also checks the case and action counts.

## Behaviour that matters for gameplay

- Three items are kept, none when skulled, one more with Protect Item (one
  when skulled and praying), none for an ultimate ironman; a null ironman mode
  is not ultimate. Inventory then equipment go into one always-stacking
  container, so equal ids merge (the later charge is kept). The selection
  pass runs once per kept slot; each item goes in front of the first kept slot
  whose high-alchemy value is lower or equal, so of equal values the later
  item wins, the item pushed out goes back and one of the item is taken. Kept
  items are single items with their charge. The rest are the lost items, with
  unstackable stacks split into single items.
- `commenceDeath` starts Retribution when it is on. `finalizeDeath` always
  sets special energy 100, takes 100 run energy, sends "Oh dear, you are
  dead!", counts the death and plays the death music, and at the end clears
  the skull, combat time, prayers and `original-loc`, opens the default tabs,
  shows the logout buttons, restores the entity, syncs the appearance and
  opens the death screen unless it was turned off. A player killer is told
  "You have defeated ..." unless the names are equal. An artificial player
  killed by an NPC stops before all of that; killed by a player it gets the
  `dead` attribute.
- Nothing is lost when the zone handles the death, in a safe zone (flag or
  zone type 1) or for an administrator. A hardcore ironman with the
  `permadeath` attribute gets the news line and the permadeath reset and the
  method ends there, skipping every reset after it. A null ironman mode fails
  the dangerous death with a null pointer after the zone checks.
- Bones drop on every dangerous death for the killer (an active player) or
  the player, at `/save:original-loc` when set.
- No grave when skulled, wilderness-skulled, in the wilderness zone, a
  hardcore ironman or where graves are restricted. Then the killing player
  gets the items (private for 100 ticks, public until 200), untradeable items
  become coins for them (half the high-alchemy value below Magic 55, minus
  250, never negative, one coin pile when the sum is positive), escaping and
  crumbling items vanish, every non-broken Barrows piece drops broken, plugin
  items become their death item, and the kill is logged. Killed by an NPC or
  nobody, the items are the player's and untradeables stay private.
- A grave holds every lost item except chinchompas and unnoted impling jars
  ("has escaped", with "jar" removed from the name), crumbling items (the
  ectophial, pouches 5509-5515, anything with a destroy option: "has crumbled
  to dust"), damaged Barrows pieces lose a fifth of their durability, the rest
  become their plugin's death item. Grave items are private to the owner's uid
  for their whole life and never become public; anyone else's lookup finds
  nothing. A grave with no items is cleared and never registered. A second
  death collapses the owner's earlier grave.
- Grave types last 2, 2, 2, 3, 4 x 8 and 5 minutes (200 to 500 ticks). With
  150 and 50 ticks left the NPC turns into the next id. The items decay when
  the timer would end; the grave collapses one controller tick after reaching
  0 (with the order graves, ground, NPCs the items are gone on the tick before
  the collapse). Near `Integer.MAX_VALUE` the decay tick wraps and the items
  disappear on the next ground pulse while the grave stays.
- A grave whose counter is -1 ends the controller's tick for every grave after
  it in map order.
- Bless: once per grave, not one's own, not an online ironman's, Prayer 70,
  adds `min(60, prayer points - 10)` minutes and costs that many points; the
  online owner is told. Repair: once, Prayer 2, at least one point, adds
  `min(5, points)` minutes. `addTime` sets every item's decay from now and
  shows the NPC states at 30 and 90 ticks (not 50 and 150).
- Demolish: the owner only; their items decay 75 ticks later and stay
  private. An owner who has left gets nothing done.
- A logged-out owner's grave keeps counting; the hint icon is sent each NPC
  tick only while the owner is online. After a relog the new player object
  finds the items by uid (and becomes their dropper).
- The server store holds each grave's ticks, location text, type, username and
  items; reading it back recreates graves whose items decay that many ticks
  from the time of reading, private once registered. Missing fields read as
  "null" and fail as numbers; a missing `ticksSinceDeath` is 0.

## Required services

`player_death::KeptHost` and `player_death::Host` list them, one method per
original call, on top of `npc_death::Host`, `drop::Host` and the ground
manager's `Context`: the player's containers, skull, prayers, ironman mode,
details (uid, username, name and the `name` field, rights), activity,
artificial and gender flags; settings energy; the packet dispatch's messages
and music, ContentAPI messages, `Player.sendMessage`; the zone monitor's
`handleDeath`, safe flag, type and grave restriction; `WildernessZone`; the
attributes (typed reads, presence, location values); the location field;
statistics, news, total level and `permadeath`; unequip listeners and
equipment plugins; item names and definitions, stat levels, item plugins'
death items; the PK monitor, slayer flags, familiars, tabs, component
visibility, appearance, death screen; the grave NPC (construction,
transform, init, clear, id, location getter and field, walking flags),
`Repository.uid_map`, hint icons, varbits, interfaces, audio, animation,
prayer points and the store archive. None has a default.

## Differences from the original

- Each method's Java `HashMap` order is reproduced for a fresh table; the
  store archive's table size kept across `clear()` is not modelled (a
  difference only after an archive of more than 12 graves).
- `toInt()` accepts ASCII digits only; the JDK also accepts other Unicode
  decimal digits. Kotlin `trim()` beyond ASCII is approximated by Rust
  whitespace. A store value that is a JSON double, array or object is written
  back in Rust's form, not Java's `toString`.
- Ground-item and grave locations are native packed locations; coordinates
  outside the packed range are not compared.
- A Barrows durability above every tier is an `Index` error; the corpus never
  reaches it.
- Grave NPC ids, locations and walking flags live in the binding's NPC;
  Kotlin writes and reads the `location` field directly (no `setLocation`),
  so those are separate silent services.

## Not covered

`PulseManager`/`PulseRunner` scheduling of the death task over a player (the
NPC corpus covers it), `DeathTask.startDeath`/`isDead` and kill credit over a
player, the rest of `handleImpact`, `Permadeath.kt` itself,
`GravePurchaseInterface`, `updateGraveType`, `hasGraveAt`, the
`forcegravedeath` command, real `EquipmentContainer`/inventory listeners
(the oracle's containers have none), item and equipment plugin bodies,
packet bytes, the real order of the controller, ground pulse and NPC ticks
within a server tick (a case input here), and every zone, content and
interface implementation.

## Reproduction

```powershell
cargo test -p rs09-server --test player_death
cargo xtask player-death-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

The generator never downloads: it expects the pinned `json-simple-1.1.1.jar`
and `ConstLib-1.4.28.jar` in `runs/player-death-oracle` and otherwise copies
them from another `runs/*/` directory after checking their pinned hashes; the
Kotlin compiler comes from `runs/kotlin-1.8.20` as for the other oracles.

## Remaining work

Binding these libraries to a live world: the tick driver must call
`Controller::tick`, the ground manager's `pulse` and each registered grave's
`npc_tick` in the original server's order; real containers with their
listeners, attributes, zone monitor, prayers, skull, settings, packets and
interfaces; the grave as a real NPC; the server store at start-up and save;
`Permadeath`, purchase interface, Retribution, plugins and the PK monitor.
