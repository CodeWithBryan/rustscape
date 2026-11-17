# NPC death and the drop

`world/npc_death` and `world/drop_tables/drop` port the path from a lethal hit
on an NPC to its items on the ground: who the kill is credited to, the death
task, the end of the death, the roll, and `createDrop` for every rolled item.
They are libraries. No example or alpha installs them, and the stand-in in
`examples/common/combat.rs` is not this port. Java is a test oracle; nothing
here runs it.

| Original | Rust | Scope |
| --- | --- | --- |
| `NPCDropTables.java` | `world/drop_tables/drop.rs` | `drop`, `createDrop`, `getLooter`, `sendDropMessage`, `handleBoneCrusher` |
| `ContentAPI.kt` | `drop.rs` | `announceIfRare` |
| `Bones.kt` | `drop.rs` (`BONES`, `bone_experience`) | item id and experience of the 31 constants; bonemeal ids are not ported |
| `StringUtils.java` | `drop.rs` (`formatted_number`) | `getFormattedNumber` in the en-US locale |
| `DeathTask.java` | `world/npc_death.rs` | `startDeath`, `isDead`, `start`, `exec`, `stop`, `removeFor`. `getContainers` is in `world/player_death` (docs/player-death.md) |
| `NodeTask.java` | `npc_death.rs` (`Task`, `task_*`) | the pulse `schedule` builds, for the death task only |
| `ImpactHandler.java` | `npc_death.rs` (`ImpactLog`, `log_hit`, `most_damage_entity`, `lethal_impact`, `set_disabled_ticks`) | the two damage logs, `getMostDamageEntity`, `setDisabledTicks`, and the two parts of `handleImpact` that write the logs and start a death. The rest of `handleImpact` is not ported |
| `Entity.java` | `npc_death.rs` | `startDeath`, `finalizeDeath` as an NPC reaches it |
| `NPC.java` | `npc_death.rs` | `commenceDeath`, `finalizeDeath`, `handleDrops`, `getAudio(2)` |
| `NPCDefinition.dropTables` | `definitions/npc` (`drop_tables`), `drop_tables/definitions.rs` | the field, and `DropTableParser`'s assignment onto native definitions |

## What is joined to native pieces

- The ground item is made by the native ground manager (`ground_manager::
  create_at`, the three-argument `GroundItemManager.create`): the native
  `GroundItem` constructor, the tradeability rule, the item plugin's drop hook,
  the chunk list and the manager list.
- The rolled item and its definition are the native `Item` and
  `ItemDefinition`: stackable, noted, note id, name, `tradeable`, `rare_item`.
- The death task is a strong pulse on the native pulse manager and runner.
- `finalizeDeath` writes `respawn_tick` and `respawning` of the native NPC tick
  state, which the native `NPC.tick` then waits on.
- The respawn delay, the death sound and the drop tables are read from the
  native NPC definition.
- `handleDrops` calls the native `drop`, which rolls the native table.

## Evidence

`cargo xtask npc-death-oracle` compiles `NPCDropTables`, `GroundItemManager`,
`GroundItem`, `DeathTask`, `NodeTask`, `Pulse`, `PulseManager`, `PulseType`,
`Bones` and the table classes of the roll unchanged with Kotlin 1.8.20 and
runs them on JDK 11; `PulseRunner` has one added import alias for its clock.
The corpus has 474 cases with 1,400 actions. Across them the original makes
38,185 recorded service calls, creates 1,992 ground items and finishes 86
deaths over 2,428 ticks.

The methods of `Entity`, `NPC`, `ImpactHandler`, `RegionChunk`, `Item`,
`ItemDefinition`, `Definition`, `Location`, `StringUtils` and `ContentAPI`
named in the fixture's `scope` are not compiled as whole classes: their text
is taken from the pinned sources when the oracle is generated and inserted
into recording adapters. Everything else an adapter does is a recorded call
answered from the case.

Each action records its result or error category, the ordered service calls,
and afterwards the state the original holds: every ground item of the manager
(id, amount, charge, location, owner, owner uid, creation tick, decay tick,
private flag, removed flag), the chunk's item list and queued public updates,
each entity's attributes, both damage logs in map order, the disabled-until
tick, the respawn tick and flag, the number of queued pulses and the position
of the three generators. The native replay must produce the same for every
action.

`crates/server/tests/npc_death.rs` also loads the complete pinned
`drop_tables.json` onto native NPC definitions and compares all 2,152 ids and
their shared tables with what the drop-table corpus recorded from the original.

## Behaviour that matters for gameplay

- A kill is credited by `getMostDamageEntity`. With any player in the damage
  log it is the player with the most damage, looked up by account uid; if that
  player has left, nobody is found and the NPC becomes its own killer. With no
  player in the log it is the non-player source with the most damage. Ties go
  to the first in `HashMap` order. An artificial player who lands the last hit
  keeps the kill. A dying player keeps the hitter.
- A killer that is not a player still rolls the table and spends its draws;
  nothing drops.
- `DeathTask.start` runs twice for every death: `schedule` starts the pulse and
  the pulse manager starts it again. The death sound plays twice and the
  behaviour's `onDeathStarted` is called twice.
- If the entity already has a strong pulse, the manager returns before the
  second start and never queues the death pulse. The entity keeps
  `state:death` and never finishes dying (one corpus case).
- The death ends once `tick:death <= ticks - duration`, with the death
  animation's duration replaced by 6 when it is outside 1 to 30.
- `finalizeDeath` restores the NPC, asks the zone, counts the kill for a player
  killer, drops, removes an NPC that does not respawn, and sets the respawn
  tick to the current tick plus `respawn_delay` (17 when absent). The zone can
  end it before the drop; the NPC then has no respawn tick.
- Nothing drops with `disable:drop` set, for an ironman killer when more than
  one player is in the damage log, or when the definition has no drop tables.
- If anything fails while the death finishes (a behaviour callback, a
  configuration value of the wrong type), `Pulse.update` prints the failure and
  stops the pulse again, so the death finishes a second time: the kill is
  counted twice and the table rolls twice. A second failure reaches the pulse
  runner, which logs it. An NPC that fails at its respawn delay both times is
  left without a respawn tick.
- `createDrop` does nothing for a missing item, id 0, a missing location, an
  item named "null" or a killer that is not a player.
- Bones are turned into prayer experience (amount times the bone's experience)
  when the killer has the bone crusher switched on, before any plugin, message
  or ground item.
- An unstackable item with an amount above 1 becomes that many single ground
  items for the killer. They skip the rare announcement, the mystery-box
  message, the drop message and loot share. A noted item is stackable.
- A rare item (`rare_item` true) is announced server-wide and counted, with
  the amount as rolled.
- For the 29 listed NPCs every online player within ten tiles of the killer
  (Euclidean, the plane is not compared) is told what dropped, unless the
  item's lower-cased name contains "bone" or "ashes". Amounts above 1 are
  written with thousands separators.
- Loot share: in a multi-combat zone, for a tradeable item or one whose name
  ends in "charm", when the killer is in a clan with loot share on, meets the
  clan's rank requirement and is not an ironman. Candidates are the clan
  members among the players within 16 tiles of the NPC who also share loot.
  With two or more, each one with more points than the highest so far draws
  `random(10)` and takes the drop on 2 to 9. The winner loses the price and
  regains one share of it; every other candidate gains one share. A charm
  counts as 100; anything else is the Grand Exchange's recommended price of the
  unnoted item. The killer is also sent the text `true` or `false`.
- The ground item belongs to the loot-share winner, is private to them for 100
  ticks and gone after 200. An untradeable item stays private.
- An artificial killer's ground item is registered with the bot repository,
  and a running bot script keeps it in its own list.

## Required services

`drop::Host` and `npc_death::Host` list them, one method per original call:
the NPC (id, location, drop location, multi-combat flag, respawn flag,
`clear`, `transform`), its behaviour callbacks, the zone monitor, attributes,
locks, facing, the walking queue, animation, graphics and audio, the death
animation and spawn locations of `Properties`, skills, timers, the impact
queue, event dispatch, statistics, item plugins, clans and loot-share state,
the Grand Exchange price, messages and news, the lists of local and online
players, the player-by-uid lookup, bots, and a dying player's own
`commenceDeath`/`finalizeDeath`. None has a default. `Host::roll` is the roll
itself: the binding owns the three generators, the shared tables and the
looter's equipment answers and calls `NpcDropTable::roll`; the tested binding
does exactly that.

## Differences from the original

- A split unstackable drop is refused above 65,536 items (`TooManyItems`).
  The original has no bound.
- The behaviour callback edits a list of `RolledItem` (id and amount). The
  original list holds `Item` objects, so content could add an item with a
  charge; that is not representable.
- The NPC damage log is ordered by the identity hash the binding gives for
  each source. Two different sources with one hash are refused
  (`identity hash collision`); the original keeps them in insertion order
  within the bucket.
- Lower-casing uses the JDK's ROOT rules and the number format is en-US. The
  original uses the JVM's default locale, which the oracle sets to en-US.
- The death animation, death graphics and spawn locations are answered by the
  host instead of read from a native `Properties`.
- Attributes are typed (`Bool`, `Int`); a value of another type is `Type`.
- The roll is reached through `Host::roll` rather than called by the library.

## Not covered

A player's death and `DeathTask.getContainers` (docs/player-death.md); the rest of `handleImpact`
(the dead-target check, hit reduction, hit splats, redemption, ring of life);
`NPC.clear`; ground-item removal and expiry in this corpus (the ground-manager
corpus has them); packet bytes; a dying entity that holds other pulses when it
dies (the pulse corpus covers the manager); the real order of pulses and NPC
ticks within a server tick (the corpus runs the pulses, then each NPC's
tick); shared tables and clue slots inside a death (the drop-table corpus
covers the roll); every behaviour, zone and item plugin implementation.

## Reproduction

```powershell
cargo test -p rs09-server --test npc_death --test drop_tables
cargo xtask npc-death-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

The generator does not download: it expects the pinned `json-simple-1.1.1.jar`
and `ConstLib-1.4.28.jar` in `runs/npc-death-oracle`, as other oracles leave
them in their own run directories.

## Remaining work

Binding these libraries to a live world: real `Properties`, attributes, locks,
masks and audio; `ImpactHandler.handleImpact` and combat; a player's death;
`NPC.clear` and the repository; behaviours, zones and item plugins; clans,
loot-share points and the Grand Exchange price; skills experience; the
statistics counter; the bot repository; the loader start-up order that decides
where the shared generator stands; and the playable alpha, which still uses
its own combat stand-in and drops nothing through this path.
