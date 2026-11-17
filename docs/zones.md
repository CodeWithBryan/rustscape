# Zones and region activity

`world/zones.rs` ports `core.game.world.map.zone`: ZoneBorders, ZoneType,
ZoneRestriction, RegionZone, MapZone's base behaviour and every overridable
member with its default, ZoneBuilder.configure, MusicZone and ZoneMonitor.
`world/region_activity.rs` ports the parts of Region.java that region.rs did
not hold: the active flag, the viewing players, the activity pulse with its
inactivity and unload decisions, Region.add/remove of region zones, the music
fields, and RegionChunk.clear as unloading uses it. `world/zone_impls.rs`
ports four of the six concrete zones: BankZone, KaramjaZone, ModeratorZone and
MultiwayCombatZone. The other two, WildernessZone and DarkZone, and
ZoneBuilder.startup are in world/wilderness_zone.rs and world/dark_zone.rs,
verified by their own corpus (docs/wilderness.md).

These are libraries. Nothing installs them: no example, alpha or session uses
them, and the three fabricated answers named below are still in the examples.
This is not a complete zone system, Region, RegionManager or world.

## Verification

`cargo xtask zones-oracle --check` compiles the unchanged ZoneBorders, Zone,
ZoneType, ZoneRestriction, RegionZone, ZoneBuilder, MapZone, ZoneMonitor,
BankZone, KaramjaZone, ModeratorZone, MultiwayCombatZone, MusicZone,
Region.java, MapDistance and Pulse on JDK 11, with PulseRunner given one
import alias for its clock. RegionManager.forId (over its own cache map and
lock), RegionChunk.clear, Entity.isTeleBlocked, Pathfinder.isStandingIn,
RandomFunction.RANDOM with normalRandDist(max, intensity), the Log enum and
Location's packed hash, region id forms, transform, toString, equals and
hashCode are original text inserted at generation time. Everything else is a
recording adapter under `oracle/zones/` (see `adapters.py`).

The designed corpus, `oracle/zones/cases.py`, has 1,019 cases, 15,261 actions,
41,772 recorded calls and 589 failures (Null, ClassCast, injected Failure,
ConcurrentModification, IllegalArgument, Arithmetic, IllegalState):

| Group | Cases | What it drives |
| --- | ---: | --- |
| borders | 60 | inside tests at and around every edge on planes 0-3 and out-of-range planes, both plane rules, nested and repeated exceptions, equals/hashCode field by field, toString, the Location and region-id constructors, region-id lists, node and region forms |
| random | 98 | seeded getWeightedRandomLoc draws including zero and negative intensity, overflowing widths, getRandomLoc and getRandomWalkableLoc with every retry count up to and past twenty |
| zone | 40 | restriction flags, names and uid (including null, non-ASCII and surrogate names), equals across classes, RegionZone equality, every MapZone member's default on players, NPCs, familiars and other entities, message |
| registration | 23 | register/unregister/registerRegion forms, structural removal, Region.add/remove walking every chunk with refusals, failures, null list entries and list changes from callbacks |
| moves | 96 | enter/leave/locationUpdate across one, two and nested zones, planes, regions, teleports and logout; dynamic zones; refusals and failures at every callback; callbacks that change the lists being walked |
| music | 16 | region track, tile music zones, the monitor's own list, artificial players and NPCs |
| queries | 165 | every ZoneMonitor query both ways, failing midway, with a zone removing itself, null arguments, type/restriction/random-event/name queries |
| teleport | 58 | tele-block by timer, lock and restriction, the mid-wilderness jewellery rule, refusing zones and failures |
| combat | 337 | continueAttack and MapZone.checkMulti for every player/NPC/familiar pairing, multi flags, other attackers, combat times and the 1613/1614 exemption |
| activity | 39 | viewer counts, the 50-tick inactivity pulse over runner ticks, region timeouts, duplicate submissions, unload refusals, forced unloads, reactivation, Region.load's lookup and its failure, NPC handling, failures and callbacks midway |
| concrete | 87 | configure of the four ported zones, Karamja rum, ModeratorZone entry/toggle/home, MultiwayCombatZone interfaces and the NPC-stacking rule, isStandingIn |

`cargo test -p rs09-server --test zones` replays every case through the
native libraries with the same inputs as required services, comparing results,
the ordered call record, every monitor's zone and music lists, every region's
activity state and zone list, every non-empty chunk's lists, update-flag count
and object count, the pulse queue and the next draw of the shared generator.
The regions, chunks, entity lists, `region::load`, the activity pulse and the
pulse runner in that replay are the real native ones. A second test pins the
fixture hash and the source hashes.

```powershell
cargo xtask zones-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
cargo test -p rs09-server --test zones
```

The oracle needs `json-simple-1.1.1.jar` and `ConstLib-1.4.28.jar` in
`runs/zones-oracle/`, checked against their pins; it never downloads them.

## What is kept

ZoneMonitor walks its live list with the ArrayList iterator: a callback that
changes the list fails the walk with ConcurrentModification exactly where the
original does, and removal of the last element can end a walk silently.
updateLocation checks music only for a non-artificial player, calls the
entity's own updateLocation, leaves in list order (passing over dynamic zones,
stopping at the first refusal with the earlier leaves kept), then enters in
the region's registration order, calling locationUpdate after each entry and
for each zone already entered. A refused entry ends the update; everything
before it stays. clear() leaves the lists alone when a zone refuses.
teleport() checks the tele-block (timer, lock, restriction, in that order)
before the zones, and the jewellery rule reads the skull level twice.
continueAttack calls the entity's own rule, the zones, the wilderness rule and
checkMulti in that order. checkMulti resolves familiars to their owners on
both sides, compares the attacker read with the entity as default, and returns
true for NPC ids 1613 and 1614 only on the second test.

ZoneBorders: the zero-plane rule, the plane rule, inclusive corners, an
exception snapshot (`toArray`) per test, region ids x outer and y inner,
getUid from the name hash while hashCode uses the stored uid field, and
ZoneBuilder.configure setting that field before the zone's own configure.
getWeightedRandomLoc draws RandomFunction.RANDOM through the host, nothing for
a zero-width axis, and divides by the intensity after the draws.

Region activity: isInactive is false while viewed or while any chunk holds a
player; otherwise a stopped pulse is rewound, started and submitted again
even while an earlier submission is still queued (both copies then run).
flagActive stops the pulse and, only when the region was inactive, loads it
through the native Region.load and hands every active NPC to the repository.
unload refuses while viewed or populated, logging and flagging the region
active again; otherwise every NPC is told (over a copy of each list) and every
chunk is cleared, then `loaded` is reset and the pulse stopped. Zones stay on
the region through unload and reactivation, and no monitor is told.
Region.add skips a null player entry; Region.remove does not.

## Required services

`zones::Host`: entity kind and monitor lookup, RegionManager.forId, location
reads, artificial flag, Entity.updateLocation, setInvisible,
Entity.continueAttack, skull wilderness and level, the two message channels,
the teleblock timer, teleport lock, item id of a teleport node, the wall clock,
multi-zone flag, Entity.isIgnoreMultiBoundaries, familiar owner, the two
combat attributes, isActive, getId, the music player, RandomFunction.RANDOM,
`new Random()`, isTeleportPermitted. `zone_impls::Host` adds inventory,
interface/packet, multi-zone writes, NPC-walkable, local NPC and player
queries, NPC visibility/attack-option/stacking, size, rights, the home
location, the teleporter and restoreTabs. `region_activity::Host` adds pulse
submission, NPC activity, Repository.addRenderableNPC,
NPC.onRegionInactivity, the error log and the chunk's region lookup, plus the
existing `region::Context` for Region.load. None has a default. No lock is
held across a host or zone callback.

## Deviations

- A DynamicRegion is refused with `Error::Dynamic` by isInactive, flagActive,
  unload and RegionChunk.clear. clear has already emptied the update flags when
  it refuses (the original clears them first too, then clears the item list).
- Native bounds the original lacks: 1,024 nested exceptions (the original
  overflows the stack on a cycle), 65,536 region ids per borders, 65,536 draws
  in normalRandDist.
- ModeratorZone's `open` is a static of the class in the original; here it
  belongs to the zone object and its clones.
- RegionManager.forId's lock-timeout path (a blank region and a log line) is
  not ported: the host's region lookup either answers or fails.
- The region-zone list is stored type-erased on the region; a region used with
  two different host types is a binding error rather than a second list.
- A ZoneMonitor holds a lease on its entity's node, as the original holds the
  entity; an owning world must drop the monitor with the entity.

## Harness limits

MapZone.checkMulti reads the wall clock; the corpus keeps every combat time
far from it, and the native replay answers a fixed time. getRandomLoc draws
from unseeded generators: only facts every draw shares are recorded (inside
the borders, the plane, and value sets over 96 draws of a span of at most
three). The first generated fixture also recorded the value set of a single
draw; the unchanged original did not reproduce it on `--check`, so the
summary was corrected on both sides and the fixture regenerated, after which
it reproduced on two further checks. The Java Familiar adapter casts its
stored owner to Player before recording the call; the native recording host
does the same. Region.load is driven only to its archive lookup (no map
archive exists in this corpus); its body is verified by the region-load
corpus. ZoneBuilder compiles against placeholder WildernessZone and DarkZone
classes; startup() is not run here (the wilderness corpus runs it with the
real zones).

## Not ported

MapZone.cleanItems. (WildernessZone, DarkZone and ZoneBuilder.startup are
ported in world/wilderness_zone.rs and world/dark_zone.rs; see
docs/wilderness.md.)
Region's assemble lists, getBaseLocation, the DynamicRegion paths,
RegionManager.apply/addRegion/removeRegion. Every concrete content zone
elsewhere in the server.

## What a real binding needs

The examples still fabricate three answers. To replace them an owning world
must:

1. `check_inactive` in `examples/common/spatial.rs`: call
   `region_activity::check_inactive` with a `region_activity::Host` whose
   `submit_pulse` submits to the world's own `pulses::Runner`, whose runner
   context dispatches that region's pulse (found by pulse identity, never by
   class name) to `region_activity::pulse_body`, whose NPC services reach the
   real NPC repository and NPC.onRegionInactivity (not ported), and whose
   `region::Context` is the Scene's real loader. Its `flag_active` should call
   `region_activity::flag_active` instead of the fixture's own activation.
   MapChunkRenderer's increment/decrement/is_active/check_inactive/flag_active
   services in `map_chunk_render::Context` map one-to-one onto this module.
2. `zone_update` in the same file: keep one `zones::Monitor` per canonical
   actor and call its `update_location` with the footprint; the original
   ignores the result. That needs a `zones::Host` answered from the actor
   (kind, location, artificial flag), Entity.updateLocation, and the music
   player, which is not ported. Regions must hold the registered zones, which
   means running each zone's `configure` at startup.
3. `ZoneMove => true` in `examples/movement_session.rs`: answer it with the
   actor's `Monitor::move_entity(from, to)`, which WalkingQueue calls once for
   the walk point and again for a run point. With no zone registered this
   returns true honestly; with
   MultiwayCombatZone registered it needs the local NPC query
   (`entity_regions::local`), NPC visibility, attack option, stacking rule
   and sizes.
