# Wilderness and dark zones

`world/wilderness_zone.rs` ports `core.game.world.map.zone.impl.WildernessZone`
and `ZoneBuilder.startup`; `world/dark_zone.rs` ports
`core.game.world.map.zone.impl.DarkZone` (with its DARKNESS_OVERLAY component
and the anonymous insect pulse) and `content.data.LightSource`. With
`zone_impls.rs` this completes the six zones of the `impl` package.

These are libraries. Nothing installs them: no example, alpha or session
configures a zone, and SkullManager, the InterfaceManager overlay, familiars
and every other system they reach are required services. This is not a
complete wilderness, PvP system, darkness system or world.

## What is ported

WildernessZone:

- The INSTANCE borders (2944,3525)-(3400,3975), (3070,9924)-(3135,10002) and
  regions 12192, 12193 and 11937; `configure` registers them in that order;
  name "Wilderness", overlappable, RANDOM_EVENTS restricted. The world makes
  the INSTANCE once (`wilderness_zone`) and keeps it: `isInZone(Entity)` and
  `isInZone(Location)` read its borders (`WildernessZone::is_in_zone`, which
  keeps the original's unused first location read).
- `getWilderness`: `(y - 3520) / 8 + 1`, or `(y - 9920) / 8 + 1` above 6400,
  Java truncating division, one location read.
- `enter`: a real player gets `show` (unless wilderness is disabled: overlay
  381, the level set from the location, "Level: n" on 381/1 read back through
  the skull manager, the attack option only when `wild_pvp_enabled`, then the
  wilderness flag); an artificial player only gets the flag and the level. A
  familiar that is not a pet is transformed and the appearance synced. An NPC
  (familiars included) with an attack option that is aggressive gets the
  WILDERNESS aggressive handler.
- `leave` (not on logout): a fresh component 381 is closed through the
  manager (the manager's overlay field is left as it was), the attack option
  removed, the flag and level reset; a combat familiar is re-transformed; the
  appearance synced.
- `locationUpdate`: a non-artificial player's level follows the location.
- `teleport`: administrators always; otherwise `checkTeleport` against 20, or
  30 when the node is an Item whose name contains "glory" or "slaying" (the
  name is read again for the second test). `checkTeleport` refuses above the
  level unless wilderness is disabled, with MapZone.message.
- `continueAttack`: player against player uses the lower skull level (the
  target's read twice when it is lower) and refuses when `combat - level >
  target` or `combat + level < target`, with wrapping arithmetic and the
  message only when asked; player against an NPC whose name contains
  "Revenant" uses the lower of the skull level and the NPC's wilderness level
  against the NPC definition's combat level.
- `interact`: a revenant target is attacked through the combat pulse
  (ClassCast for a non-player), anything else falls to MapZone.
- `death`: an NPC killed by a player rolls the revenant/Chaos Elemental loot:
  `getNewDropRate` (`(int) (44044.5491 - 7360.19548 * ln(level))`), the deep
  wilderness halving above the 100,000 risk threshold, the glove rate (50 for
  the Chaos Elemental, 100 deep, otherwise the 16-roll formula), the
  brawling-glove draw `random(1, 14)` cast to a byte, then the 17 PvP items
  each rolled in order with 15-50 javelins and throwing axes; each drop goes
  to the NPC's drop location and sends news (the glove's name lower-cased, a
  null gear name printed as "null"). The NPC id and name are re-read for each
  news line. The result is always false.

DarkZone:

- `configure`: (1728,5120)-(1791,5247), regions 12693, 12948, 12949,
  (3222,9600)-(3306,9661) and (3717,9346)-(3841,9473).
- `enter`: for a player the active light source is looked up before the
  constant-glow perk is checked; with the perk the entry ends there (no
  hook). No source opens DARKNESS_OVERLAY, a source with an interface opens a
  fresh component 97/98, a bright source opens nothing; every entity is then
  hooked for UseWith.
- `leave`: a player's overlay is closed, then the entity unhooked.
- DARKNESS_OVERLAY.open: nothing while the stored pulse still runs; otherwise
  a new insect pulse (delay 2, the player as its node) is submitted, stored as
  the extension, then the component opens. Its close stops the stored pulse
  only after the component closed.
- The insect pulse: "You hear tiny insects..." on its first run, "Tiny biting
  insects swarm all over you!" on its sixth, a 1 hit on every run after; it
  never finishes by itself.
- `updateOverlay`: the light source, the perk (queueing the SOFT glow script),
  two overlay reads, darkness when there is no source, otherwise the stored
  pulse stops and the overlay changes to the source's (closed for -1).
- `checkDarkArea` walks the monitor's live list to the first DarkZone; the
  tinderbox UseWith hook runs it two ticks later through runTask.
- `continueAttack` refuses only a player whose overlay is DARKNESS_OVERLAY
  itself; `interact` refuses dropping or extinguishing a lit light source
  (option lower-cased, ClassCast for a non-player).

LightSource: the ten sources with level, raw and lit item, open flag and
interface, `forId`, `forProductId`, `getName`, `getStrength`, `getRawIds`
(whose last entry stays 0) and `getActiveLightSource` (inventory snapshot,
then equipment).

ZoneBuilder.startup: configure of the wilderness INSTANCE, the multiway
INSTANCE, a new ModeratorZone, DarkZone, KaramjaZone and BankZone.

## Verification

```powershell
cargo xtask wilderness-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
cargo test -p rs09-server --test wilderness
```

The oracle compiles the unchanged WildernessZone, DarkZone, LightSource,
BrawlingGloves, Event and EventHook with the zone framework of the zones
oracle (ZoneBorders, Zone, ZoneType, ZoneRestriction, RegionZone, ZoneBuilder,
MapZone, ZoneMonitor, the four other zones, MusicZone, Region.java,
MapDistance, Pulse, PulseRunner) on JDK 11 with Kotlin 1.8.20 and ConstLib
1.4.28. Original text inserted at generation time: RandomFunction's RANDOM,
`random(int)`, `random(int, int)`, `roll` and `normalRandDist`;
SkullManager.DEEP_WILD_DROP_RISK_THRESHOLD; Option._P_ATTACK; QueueStrength;
UseWithEvent; Event.UsedWith; the Skills ids; Entity.isTeleBlocked and the
Location/RegionManager/RegionChunk/Pathfinder/Log texts of the zones oracle.
Everything else is a recording adapter under `oracle/wilderness/` (built on
`oracle/zones/adapters.py`).

The designed corpus, `oracle/wilderness/cases.py`, has 696 cases, 6,627
actions and 23,203 recorded calls; 337 operations fail (104 injected
failures, 225 NullPointerException, 2 ClassCast, 2 ConcurrentModification,
4 out-of-range script/task indices):

| Group | Cases | What it drives |
| --- | ---: | --- |
| levels | 32 | getWilderness at both sides of every 8-tile level line above and below ground, 6399-6401, negative and wrapped y; isInZone(Entity) and isInZone(Location) at every corner and edge (±1) of the five INSTANCE borders on planes 0 and 3; getNewDropRate over 82 combat levels including 0, negative, 396-400 and the int extremes; getBorders, the constants, BrawlingGloves.forIndicator -1..15, zone info and configure of the INSTANCE and of custom borders |
| enter | 89 | enter, level change and leave through ZoneMonitor.updateLocation for real, artificial and wilderness-disabled players, familiars (none, pet, non-combat, combat, missing), PvP on, off and without settings, NPCs, familiars and other entities with every attack-option/aggression pair, logout through clear(); every hook with each recorded call failing at its first and second occurrence; a refusing overlay; the INSTANCE across its south edge, north edge and the underground borders; callbacks that clear the monitor or register a zone mid-walk |
| attack | 82 | continueAttack player against player over skull-level pairs and combat levels at -1/0/+1 of both range ends, with and without the message; wrapping at the int extremes and negative levels; revenants (name variants, null name, familiars) at five wilderness positions over fifteen combat levels; NPC attackers, null, item and other targets; failures; ZoneMonitor.continueAttack with the entity rule, the wilderness flags and checkMulti |
| teleport | 42 | the zone rule for every rights value, wilderness disabled or not, skull levels 0/19/20/21/29/30/31/56/-1/MAX, null, glory, slaying, both, neither, null-named, entity and other nodes; checkTeleport at 20, 30 and the int extremes; failures; ZoneMonitor.teleport (types -1, 0, 1, 2) with the tele-block timer, the teleport lock and the jewellery rule |
| death | 323 | the Chaos Elemental, revenants, other NPCs and a null name over combat levels 1-1,000, deep wilderness with risk below/at/above the threshold, 90 seeded multi-kill cases (Chaos Elemental, high-level revenants, revenants whose glove rate is a few rolls) hitting gloves and gear, null user and item names, NPC and null killers, familiars, every recorded call failing in turn, ZoneMonitor.handleDeath |
| interact | 8 | the revenant attack option (players, NPCs, null names, failures, through the monitor); DarkZone's drop/extinguish refusal for every lit and unlit source with ten option spellings, null options, NPC actors and non-item targets |
| dark | 107 | enter/leave through the monitor with ten light holdings and the perk; the insect pulse over twenty ticks, an inactive node, failing hits and messages; DARKNESS_OVERLAY open twice, close refused, failures; updateOverlay over every holding and five current overlays with the glow script run; checkDarkArea and the tinderbox hook with every name combination and a non-player; continueAttack with each overlay; each enter/update/leave call failing; a callback clearing the monitor during checkDarkArea |
| light | 10 | the LightSource table and getRawIds, forId/forProductId over every raw and lit id, getActiveLightSource and hasActiveLightSource over inventory and equipment holdings, a failing equipment snapshot |
| startup | 3 | ZoneBuilder.startup with the six real zones; DarkZone.configure; entry at each dark area's corners |

`cargo test -p rs09-server --test wilderness` replays every case through the
native libraries and compares results, the ordered call record, every
monitor's zone list, every player's overlay and stored pulse (with its running
flag), every region's zone list, the pulse queue, the queued script and task
counts and the next draw of the shared generator. The zones, monitors,
regions, the darkness overlay's open/close bodies, the insect pulse and the
pulse runner in that replay are the real native ones. A second test pins the
fixture hash and the source hashes.

The oracle needs `json-simple-1.1.1.jar` and `ConstLib-1.4.28.jar` in
`runs/wilderness-oracle/`, checked against their pins; it never downloads
them.

## How the other systems join

- Teleports: `teleport::send` calls `ZoneMonitor.teleport(type, null)` through
  its `zone_teleport` service, which an owning world answers with
  `zones::Monitor::teleport`. With the wilderness INSTANCE registered, that
  monitor walk reaches `WildernessZone::teleport` natively; the corpus drives
  `ZoneMonitor.teleport` with a null node (as TeleportManager passes) for
  types -1, 0, 1 and 2, tele-blocked and not, and with jewellery items.
- Skull and tele-block state: SkullManager is not ported. Its getters and
  setters (`skull_level`, `skull_wilderness`, `set_skull_level`,
  `set_skull_wilderness`, `skull_wilderness_disabled`,
  `skull_deep_wilderness`) and the "deepwild-value-risk" attribute are
  required services; the real `setLevel` also re-derives deep wilderness, the
  skull icon and the teleport lock, which is where the skull icon and the
  level-20 teleport lock come from. The tele-block timer reaches the zones
  through `zones::Host::has_timer(entity, "teleblock")` and the teleport lock
  through `teleport_locked`; a timers port answers the first, ActionLocks the
  second.
- Combat: CombatPulse's attackability check reaches `victim.isAttackable`,
  which calls `zoneMonitor.continueAttack` (npc_behaviour's
  `zone_continue_attack` service); `zones::Monitor::continue_attack` walks
  the zones, so the level range and the darkness refusal run natively there.
  npc_behaviour's `wilderness_in_zone` service is answered by
  `WildernessZone::is_in_zone` on the INSTANCE, and its `wild_pvp_enabled`
  is the same setting as here.
- The interface manager: `dark_zone::Overlays` is the manager's overlay
  surface. A binding to the native `interfaces` manager must make
  DARKNESS_OVERLAY one component object (id 96) whose open and close call
  `darkness_open` and `darkness_close`, and every `Component::Id` a fresh
  component.
- Pulses: the insect pulse is a native `pulses::Pulse`; the world's
  `pulses::Context` must dispatch its handle by identity to `insect_pulse`.
  The glow script and the tinderbox task are typed objects for the script
  queue and the entity pulse manager.

## Required services

`wilderness_zone::Host` (on top of `zone_impls::Host` and
`dark_zone::Overlays`): the SkullManager getters and setters, the deep-wild
risk attribute, sendString, the PvP setting, the attack option set/remove,
the familiar manager (has familiar, has pet, the familiar, transform, combat
familiar, reTransform), appearance sync, NPC aggression and the WILDERNESS
handler, NPC name and definition combat level, the current combat level, the
combat pulse attack, Item names, the drop location, GroundItemManager.create,
the username and Repository.sendNews. `dark_zone::Overlays`: openOverlay,
closeOverlay, close(Component), getOverlay and `instanceof Item`.
`dark_zone::Host`: the inventory and equipment snapshots, the constant-glow
perk, option names, hook/unhook, the DarkZone extension get/add, the world
pulser, Component's base open/close, manualHit, queueScript, stopExecuting,
runTask and getItemName. The RANDOM draws go through
`zones::Host::random_next_int`. None has a default; no lock is held across a
service or zone callback.

## Deviations

- The INSTANCE is a value the world keeps, not a static: isInZone takes it.
- DARKNESS_OVERLAY is `Component::Darkness`; every `new Component(id)` is
  `Component::Id(id)`, so only DARKNESS_OVERLAY has an identity.
- The insect pulse's `count` is an atomic of the native object (the JVM field
  is a plain int); the values are the same.

## Harness limits

The interface manager adapter keeps one overlay field and calls the
components' close/open in the original order; the definition-type repair,
the packets and the rest of InterfaceManager are not modelled (the
interfaces corpus covers the manager). Queued scripts and runTask tasks run
when the case says so, not on a script queue or entity pulse manager.
Event dispatch to `process` is not driven; the case calls it. SkullManager
is a recording adapter, so the deep-wilderness skull icons and the
teleport lock its setLevel derives are not in this corpus.
BrawlingGloves' static initializer reads item names through an adapter that
records nothing (it runs once per JVM). Item, NPC and user names are case
inputs.

## Not ported

SkullManager (deep wilderness, skull icons, skull causes, the teleport lock
in setLevel), the Familiar transform bodies, AggressiveHandler's use of the
handler (npc_behaviour has it), the InterfaceManager binding, the event hook
dispatch, the content that lights light sources (LightSourceLighter,
SkillcapePerks' activation), MapZone.cleanItems and every other content
zone.
