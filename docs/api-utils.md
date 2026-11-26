# core/api/utils

The eight files of `core/api/utils` and where each is ported:

| Original | Native | Corpus |
| --- | --- | --- |
| `WeightedTable.kt` | `world/api_utils.rs` (`WeightedTable`) | api-utils-oracle |
| `PlayerCamera.kt` | `world/api_utils.rs` (`PlayerCamera`, `CameraContext`, `CameraType`) | api-utils-oracle |
| `CameraUtils.kt` | `world/api_utils.rs` (`CameraShakeType`; the `CameraUtils` object is empty) | api-utils-oracle |
| `WeightBasedTable.kt`, `WeightedItem.kt` | `world/drop_tables.rs` | drop-tables-oracle |
| `Vector.kt` | `world/content_api/api_files.rs` (`Vector`) | content-api-oracle |
| `PlayerStatsCounter.kt` | not ported (`incrementKills`/`incrementRareDrop` are host services of npc_death and drop_tables) | none |
| `Permadeath.kt` | not ported (`permadeath` is a host service of player_death) | none |

## WeightedTable

An `ArrayList<Pair<T?, Double>>` with a running `totalWeight`.
`WeightedTable<T>` keeps the list as `entries`: the inherited `ArrayList`
members (a raw `add(Pair)`, `clear()`, `size`, `get`) work on it and leave the
total as it was, as in the original.

- `add(element, weight)` adds to the total, then appends.
- `remove(element)` removes the first entry whose element equals it (null
  equals null) through `removeAt`, which takes the weight off the total;
  `removeAt` fails for an index outside the list.
- `roll()` answers the only element of a table of one and null for an empty
  table, without a draw. Otherwise it shuffles a copy (Kotlin's `shuffled()`:
  `Collections.shuffle`, which draws `nextInt(i)` for `i` from the size down
  to 2 from the JDK's shared shuffle generator), then draws
  `RandomFunction.random(0.0, totalWeight)` (`min + (max - min) *
  RANDOM.nextDouble()` with `Math.min`/`Math.max`), and answers the first
  shuffled entry that brings the remaining weight to zero or below; null when
  none does (a NaN total, a total larger than the entries' weights).

The two generators are different objects in the original and are two
services of `api_utils::Draws` (`shuffle_int`, `next_double`). The live world
must give `shuffle_int` a generator of its own: the JDK's shuffle generator is
seeded from the clock and shared with every other `Collections.shuffle(list)`
call, so its sequence is a host input, not something a port can reproduce.

## PlayerCamera

`PlayerCamera<P>` holds the player and the last `CameraContext`. Every member
does nothing without a player; otherwise it stores the new context, then sends
it (`CameraHost::send_camera`), so a failed send leaves the new context:

| Member | Type | x, y, height, speed, zoomSpeed |
| --- | --- | --- |
| `setPosition(x, y, height)` | SET (187) | x, y, height, 0, 0 |
| `rotateTo(x, y, height, speed)` | ROTATION (125) | x, y, height, speed, 1 |
| `rotateBy(dx, dy, dh, speed)` | ROTATION (125) | last x + dx, last y + dy, last height + dh (wrapping), speed, 1; nothing without a last context, whatever its type |
| `panTo(x, y, height, speed)` | POSITION (154) | x, y, height, speed, 1 |
| `shake(type, jitter, amplitude, frequency, speed)` | SHAKE (27) | type, jitter, amplitude, frequency, speed |
| `reset()` | RESET (24) | -1, -1, -1, -1, -1 |

The camera packet's encoding (`CameraViewPacket`) is not part of this library
or of its corpus.

## Corpus

`cargo xtask api-utils-oracle --check` (Python, JDK 11) compiles
`WeightedTable.kt`, `PlayerCamera.kt`, `CameraUtils.kt`, `Context.java` and
`CameraContext.java` unchanged, with `RandomFunction.random(double, double)`
inserted verbatim. `Collections.shuffle`'s shared generator (set by
reflection; the JVM runs with `--add-opens java.base/java.util=ALL-UNNAMED`)
and `RandomFunction.RANDOM` are `java.util.Random` objects seeded by the case
that record their draws; the table's members are called from Kotlin, as
content calls them. Each case runs in a class loader of its own. 395 cases
(230 designed, 165 with one injected failure) / 3,925 operations / 6,034
recorded calls: tables of every small size under five seed pairs, sixteen
weight sets (zero, negative, huge, tiny, infinite, NaN, signed zero), null,
int and duplicate elements, every `removeAt` index, the inherited members,
a random walk, and every camera member with and without a player.
`cargo test -p rs09-server --test api_utils` replays them with
`rs09_game_core::random::JavaRandom` under the same seeds: recorded draws and
packets in order, results, errors, the entries and total weight (as bits) and
the camera context after every operation.

22 of 23 deliberate breaks fail the replay. The other (`f64::min` in place
of `Math.min`) is the same behaviour here: the first argument is always 0.0,
and a NaN or negative-zero total reaches the same result through the
maximum.

## What the live world should call

Nothing in `world/live` ports these today. When implings (`ImplingData`'s
`WeightedTable.create`) or cutscene cameras (`PlayerCamera`) are installed,
they call `api_utils::WeightedTable` and `api_utils::PlayerCamera`;
`content_api::Host`'s camera members stay as they are. The activity and
cutscene framework (`world/activities`) keeps `PlayerCamera` behind its
host's `camera_new`, `camera_rotate_to`, `camera_rotate_by` and sibling
services: a binding answers them with one `api_utils::PlayerCamera` per
cutscene.
