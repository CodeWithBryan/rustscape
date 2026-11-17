# Player and NPC repository and the disconnection queue

`world/repository` and `world/disconnection` port the original's lists of
players and NPCs, the name and uid maps, and the queue a leaving player waits
in until the server may remove it. They are libraries. Nothing installs them:
the playable alpha still gives out player indices with `(1..2000).find` in an
example and decides "already online" by user name, and neither is what the
original does. Java is a test oracle; nothing here runs it.

| Original | Rust | Scope |
| --- | --- | --- |
| `NodeList.java` | `world/repository/node_list.rs` (`NodeList`) | every method: allocation, removal, the bulk calls, `size`, `length`, `toArray`, `iterator` |
| `NodeListIterator.java` | `node_list.rs` (`NodeListIter`) | `hasNext`, `next`, `remove` over the copy made when it starts |
| `Repository.kt` | `world/repository.rs` (`Repository`) | the lists and maps, `addPlayer`, `removePlayer`, `getPlayerByName`, `getPlayerByUid`, the three `sendNews` forms and `sendGrandExchangeNews`, `add`/`removeRenderableNPC`, both `findNPC`, `lobbyPlayers`, `disconnectionQueue` |
| `DisconnectionQueue.kt` | `world/disconnection.rs` (`DisconnectionQueue`) | every member: `add`, `update`, `finish`, `clear`, `get`, `contains`, `remove`, `isEmpty`, `save` |
| `Player.java` | `disconnection.rs` (`player_clear`, `finish_clear`, `allow_removal`) | `clear`, `finishClear` with `sendLogoutEvents`, `allowRemoval`, as ordered calls to required services |
| `PacketDispatch.java` | `disconnection.rs` (`send_logout`) | `sendLogout` |
| `IoSession.java` | `disconnection.rs` (`session_disconnect`, `session_player`) | `disconnect` and `getPlayer` |
| `TickUtils.kt` | `disconnection.rs` (`seconds_to_ticks`) | `secondsToTicks` |
| `java.util.HashMap` (JDK 11) | `world/repository/hash_map.rs` (`JavaHashMap`) | put, get, remove, clear and the walk, with tree bins, resizes and splits |

## What the original does that this keeps

- Slot 0 of a list is never used. `add` takes the first free slot from 1 up and
  fails when there is none; a freed slot is the next one taken. The index is
  the node's own field: removal does not reset it, and `indexOf` only reads it.
- `remove(null)` empties the first free slot, which is already empty, and still
  takes one off the size. The size can then fall below the number of nodes or
  below zero, after which `toArray`, `iterator` and `findNPC(id)` fail.
  `contains(null)` is true while any slot is free.
- An iterator walks a copy. Changes to the list after it starts are not seen;
  its `remove` takes the node out of the list if the list still holds it.
- `addPlayer` looks for a player with the same uid only in slots
  `1 .. size - 1`. Slot `size` and anything after a hole are never compared,
  so a second player with the same uid can stay in the list. A player found
  is taken out and cleared before the new one is added. A full list does not
  stop the maps from being written. A null name fails with a
  `NullPointerException` after the list and the uid map were written.
- The name map is keyed by `getName()`. `getPlayerByName` lowers the case in
  the default locale and turns spaces into underscores; nothing else is
  folded. The last player put under a name or uid wins, and removing any
  player removes the entry under its current name and uid, whoever it points
  to. `removePlayer` ends with the session's `disconnect`.
- `sendNews` walks the name map in its order when the call starts and skips
  muted players for Grand Exchange news. It does nothing unless global chat is
  enabled.
- `addRenderableNPC` and `removeRenderableNPC` ignore revenants. Removal
  clears the flag even for an NPC that was not in the list.
- The queue is keyed by name and walked in HashMap order. `update` does
  nothing unless the queue has an entry and `GameWorld.ticks % 3 == 0`
  (Kotlin's remainder, so -3 is a multiple and -1 is not). It walks the
  entries present when it starts.
- `allowRemoval` asks, in order and stopping at the first yes: in combat,
  lifepoints below 1, dead, teleporting, a soft script queued.
- A finished entry leaves the queue, but its wait counter stays under the
  name. Only `remove(name)` drops it, so the next wait under that name starts
  where the last one stopped. `clear()` keeps the counters too.
- A waiting entry's counter grows by 3 at each gate. At 50 for an AFK logout
  (`secondsToTicks(30)`) or 500 otherwise (`secondsToTicks(300)`) the player
  is cleared without the checks, the logout packet, the clan or the save:
  `finishClear`, then in a `finally` `removePlayer` and `remove(name)`, then
  the warning. Because the counter only takes multiples of 3, the clear comes
  at 51 and 501. `isAfkLogout` is read again at every gate.
- `finish` sends the logout (which queues the player again first), runs
  `finishClear` catching any `Throwable`, removes the player, leaves the clan
  catching only an `Exception`, stops for an artificial player, and then
  hands the save to `TaskExecutor.executeSQL` (normal) or saves at once
  (forced, from `clear()`). `save` prints a failure and always answers false.
- `Player.clear` clears an artificial player at once; any other is removed
  from the queue and added again with `clear = true`, then its details are
  saved. `IoSession.disconnect` with a failing socket close prints the
  `IOException`, clears the player without a catch and leaves `object` set.

## Evidence

`cargo xtask repository-oracle` compiles `NodeList`, `NodeListIterator`,
`Repository`, `DisconnectionQueue`, `TaskExecutor`, `TickUtils`,
`LogoutListener` and `ContentInterface` unchanged with Kotlin 1.8.20 and runs
them on JDK 11, each case in a fresh class loader so that the `Repository`
object and its maps start empty. The members of `Player`, `PacketDispatch`,
`IoSession`, `Node` and `NPC` named in the fixture's `scope` are original text
inserted into recording adapters at generation time. Everything else an
adapter does is a recorded call answered from the case. `kotlinx.coroutines`
is a stand-in that keeps a launched block until the case runs it.

The corpus has 625 cases: 501 scenarios with 7,876 actions, in which the
original makes 26,255 recorded calls, finishes 194 forced clears and meets 214
injected failures; and 124 cases with 59,670 steps that drive `HashMap` itself
over plain lists, one-hash and one-bucket key families, null and empty keys,
tree bins, splits and `clear()`. The scenarios cover allocation, reuse and
the pinned capacities of 2,000 players and 32,000 NPCs, duplicate uids and
names, removal during iteration, the gate over long runs and across the wrap
of the tick counter, each `allowRemoval` condition both ways, the forced
clear before, at and after its deadline, calls back into the queue and the
repository from inside each service, and both scenarios of the original's
`SessionClearingTests` and `SessionDirtyDisconnectTests`.

Each action records its result or failure, the calls in order and the state
afterwards: both lists slot by slot with their size counters, both maps and
the queue and its counters in iteration order, the renderable NPC and lobby
lists, every node's index, playing and renderable fields, every session, the
renderable players, the waiting saves, the tick counter and the thread name.
`crates/server/tests/repository.rs` replays every case on the native library
and compares all of it.

```powershell
cargo test -p rs09-server --test repository
cargo xtask repository-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

The generator never downloads. It needs the pinned Kotlin jars under
`runs/kotlin-1.8.20` and the pinned json-simple jar under
`runs/repository-oracle`.

## Required services

Every outside call is a method of `repository::Host`, with no default:
the uid, name, user name, index, artificial, AFK and playing fields; the
session and its key, socket and object; the virtual `player.clear()`; the
renderable players; combat, lifepoints, death, teleport and the script queue;
the logout packet; both kinds of logout listener; the familiar, interfaces,
`Entity.clear`, zones and house; the two management messages; the clan and the
world id; the wealth check; `details.save()`; the save hand-off, the thread
name and `PlayerParser.saveImmediately`; the log and stack traces; revenants,
NPC ids, the region chunk and locations; the news mute and messages; the tick
counter, global chat and the default-locale lower case. The binding owns these
and must not answer them from native state that is not the real system.

## Differences from the original

- Each method of a list, map or the queue takes its lock alone; no lock is
  held while a service runs, so services may call back in. The original's
  `NodeList` methods are `synchronized` and nothing else is; concurrent
  callers are not compared.
- A `NodeList` above 2^20 slots is refused (`Error::Capacity`); the original
  has no bound. Capacity -1 makes a list with no slots, as in the original.
- Players and NPCs are never null. The calls that take null in the original
  (`contains`, `remove`) take an `Option`. A null map key is `None`.
- `HashMap` lookups walk the bin's chain rather than its tree. They find the
  same entry and change nothing. Its iterators, views and modification check
  are not ported.
- `TaskExecutor.executeSQL` is a hand-off: the binding receives a
  `SaveTask` and runs it later, off the tick. Real coroutine dispatch and
  threads are not part of this corpus.
- `Player.clear` is a host call because it is virtual; the plain `Player`
  body is `player_clear`. `AIPlayer.clear` is not ported.
- A failure of a service is `Io`, `Exception` or `Throwable`, so that each
  `catch` stops what it stops in the original. Stack traces are a host call.

## Not ported

`AIPlayer.clear`; `checkForWealthUpdate`'s body; `PlayerSaver` as a save hand-off
target (it is the player-save port's); `LoginParser`, `LoginConfiguration` and
the lobby; `MajorUpdateWorker`'s call site (the tick driver in the corpus is
"update, then increment the counter"); `SystemTermination`; real sockets,
packets and management messages; a null entry in the queue.

## What the next steps need from this

The login step needs `Repository::add_player` at the point the original calls
it, `contains_uid` for the online check of `Login.kt`, and
`DisconnectionQueue::remove` by user name as `LoginParser` does it; the
example's index search and name check must then go. The tick-driver step
needs `DisconnectionQueue::update` in the `disconnects` phase of
`tick::handle`, with the real `GameWorld.ticks`, and a binding of `Host` to the
canonical player: its fields, its session, `renderablePlayers` of the update
sequence, and real or explicitly unavailable services for the rest.
