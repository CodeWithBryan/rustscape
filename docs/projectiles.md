# Projectile and ProjectileUpdateFlag

`crates/server/src/world/projectiles.rs` is the native `Projectile` class and
its chunk flag: every factory (`create` in its seven forms, `magic`, `ranged`,
the empty constructor), `getSpeed`, `getLocation`, both `transform` pairs,
`copy`, the three `send`s, the setters (plain fields), and
`ProjectileUpdateFlag.write` with `UpdateFlag`'s `data`, `ordinal`,
`compareTo` and `equals`. The projectile path and line-of-sight queries are a
different library (docs/projectile.md).

A projectile is a shared object (`Projectile<E>` is `Shared<Fields<E>>`):
`transform` changes it and answers the same object, and the chunk flag keeps
that object, so a transform or setter after `send` shows in the bytes. The
world is `projectiles::Host`: the entity kind, `Node.size()`,
`Node.getLocation()` and `Node.getIndex()` as calls, the `size` and `location`
fields `Node.getCenterLocation` reads directly, `RegionManager.getRegionChunk`
and `RegionChunk.flag`. Every service is required and may fail.

What the original does and the port keeps:

- The speed is `(int) (base + distance * factor)` over `Location.getDistance`
  (Int arithmetic, then `sqrt`); Java's cast saturates. `create` and `ranged`
  use 46 and 5, `magic` 46 and 10, `transform` its own two numbers.
- `receiver.getDistance(victim.getLocation())` evaluates the receiver, then
  calls `victim.getLocation()`, then fails on a null receiver: the victim is
  asked for its location even when the source has none.
- `create(source, victim, id, startHeight, endHeight, startDelay, speed)` and
  the shorter forms use `source == null ? 11 : source.size() << 5`; the
  eight-argument form, `magic` and `ranged` use `source.size() << 5` unguarded.
- `transform` writes `source`, then `sourceLocation` (kept when the source has
  no location, because `getCenterLocation` fails first), then `victim` or
  `endLocation`, then the speed, then the distance for an NPC source.
  `transform(source, victim)` treats the source as an NPC when it is one.
- `copy` measures between the two entities' locations (not the source's
  centre), adds the template's speed, and keeps the template's distance.
- `send` asks for the region chunk of the source location (null included)
  and flags it with the same object.
- `write` reads the victim's location first (a location-based projectile reads
  its end location instead; no victim and no end location fails before any
  byte), puts 16, the chunk offsets of the start, the two deltas, the target
  (`-(index + 1)` for a player, `index + 1` otherwise, -1 without a victim),
  then the id, heights, delay, speed, angle and distance as they are at that
  moment. A failure leaves the bytes already put: `write` appends to the
  caller's buffer as it goes.

## Corpus

`cargo xtask projectiles-oracle --check` (Python, JDK 11) compiles
`Projectile.java`, `ProjectileUpdateFlag.java`, `UpdateFlag.java`,
`Location.kt` and `Vector.kt` unchanged, with `Node.getCenterLocation` and
`IoBuffer.put(int)`/`putShort(int)` inserted verbatim; `Node`'s getters,
`RegionManager.getRegionChunk` and `RegionChunk.flag` are recording adapters.
Each case runs in a class loader of its own. 787 cases (575 designed, 212 with
one injected failure) / 4,051 operations / 4,169 recorded calls: every factory
with every source and victim kind, missing sources, victims and locations,
sizes 0 and negative, the coordinate limits, location aliasing, fields changed
after `send`, several flags of one object, and a random walk.
`cargo test -p rs09-server --test projectiles` replays them: recorded calls in
order, every field of every projectile after every operation, the flag list,
results, errors and written bytes (failed prefixes included).

14 of 15 deliberate breaks fail the replay; the fifteenth (reading the
projectile's numbers before the `getIndex` call in `write`) cannot be observed,
because `Node.getIndex` is a plain getter.

Not covered: the order in which `RegionChunk.update` writes a chunk's flags
(chunk-sync-oracle), the client, and anything that makes projectiles (the
swing handlers are combat-swing-oracle and combat-pulse-oracle).

## Replacing the live world's own port

`world/live/combat/projectiles.rs` was written from the pinned sources without
a corpus. To use the library there:

| Live function | Library call |
| --- | --- |
| `Live::projectile_create(source, victim, id, sh, eh, sd, speed, angle)` | `projectiles::create_with_angle(self, Some(source), victim, id, sh, eh, sd, speed, angle)` |
| `Live::projectile_speed_based(.., factor)` | `projectiles::ranged` (factor 5) or `projectiles::magic` (factor 10) |
| `Live::projectile_copy(template, source, victim, multiplier)` | `projectiles::copy(self, &template, Some(source), Some(victim), multiplier)`, the template being a `Projectile` made once from the `RangedConfigLoader` numbers (`Shared::new(Fields { .. })`) |
| `Live::projectile_send(p)` | `projectiles::send(self, &p)` |
| `Live::write_projectile_flag(key, buffer)` | `let mut out = Vec::new(); let r = projectiles::write(self, &p, &mut out); buffer.write().payload.extend(out); r` (extend before returning the error) |
| `Projectiles.flagged: HashMap<usize, Projectile>` | the same map over `projectiles::Projectile<P>` |

`impl projectiles::Host for Live`: `Entity = P`, `Chunk` the chunk handle;
`kind` from the player and NPC tables; `size`/`size_field` the NPC definition's
size (1 for a player); `location`/`location_field`
`state.node_location`; `index` the player slot's index or the actor's wire
index; `region_chunk` is `gm::Context::chunk` (a null location is a failure);
`flag` stores the projectile under a new foreign key and calls
`chunk_sync::flag_with`.

Where the live port differs from the original today: a source without a
location makes a projectile with no source location instead of failing; a
failing `write` leaves no byte instead of the prefix; `create`'s seven shorter
forms, `getSpeed`, the transforms and location-based projectiles do not exist
there (`Projectile.transform` of a switching attack is refused).
