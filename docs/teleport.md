# Teleports

`world/teleport.rs` ports `core.game.node.entity.player.link.TeleportManager`
(its three `send` overloads, `fireRandom`, `getType`, the teleport-type
field, `WILDY_TELEPORT`, `TeleportSettings` and all fifteen `TeleportType`
constants with their anonymous pulses and `stop`/`start` overrides) and
`TeleportPulse`, plus `Entity.isTeleporting`.

The fifteen types are the ones the original defines: NORMAL, ANCIENT, LUNAR,
TELETABS, OBELISK, TELE_OTHER, FAIRY_RING, PURO_PURO, ECTOPHIAL, CHRISTMAS,
CABBAGE, ENTRANA_MAGIC_DOOR, MINIGAME, INSTANT and RC_GUILD. There is no
home-teleport or jewellery type in this class: the home teleport is its own
content pulse and jewellery calls `send(location, NORMAL, 1)`; both are
content and not ported here.

This is a library. Nothing installs it: no session, example or world owns a
`Manager`, and no packet reaches `send`.

## What runs where

- Each teleport pulse is a native `pulses::Pulse` (delay 1, no checks, class
  name `TeleportManager$TeleportType$N$1`) submitted to the world
  `pulses::Runner`. The owning world's `pulses::Context` must dispatch the
  handle, by identity, to `TeleportPulse::pulse`, `stop` and `start`;
  `pulses::update_base` does the rest (the caught failing body, the stop after
  it, the stop after `true`).
- Animations and graphics are native `animation::Animation`/`Graphics`
  objects handed to the entity's native `Animator` (`forceAnimation`,
  `animate`, `graphics`); `Animation.RESET` is the world factory's shared
  object.
- The destination is written to the teleport location the native
  `walking::Queue` consumes in `update_teleport` (WalkingQueue.updateTeleport:
  reset, move, scene flag, RegionManager.move, footprint, teleporting flag).
- `send`'s checks go through `ZoneMonitor.teleport(type, null)` and
  `Entity.isTeleBlocked`, which an owning world answers with the native
  `zones::Monitor::teleport` and `zones::is_tele_blocked`.
- Every other call is a required `teleport::Host` service, in the original
  order with the original arguments. None has a default.

## The checks, in the original order

`send(location, type, teleportType)`:

1. WILDY_TELEPORT or OBELISK: refused while a `teleblock` timer is active, with
   no message. Nothing else is checked.
2. Otherwise `ZoneMonitor.teleport(teleportType, null)`: for any type but -1,
   `isTeleBlocked` (timer, then the teleport lock, then a TELEPORT restriction
   on any zone) refuses with "A magical force has stopped you from
   teleporting." for a player; then every zone's `teleport` in list order,
   the first refusal ending the walk (no message from the monitor).
3. Then, again for any type but -1, `isTeleBlocked` a second time, with the
   same message for a player.

There is no combat check and no wilderness-level check in TeleportManager. The
wilderness rule lives in WildernessZone.teleport, a zone the monitor walks
(`world/wilderness_zone.rs`, see docs/wilderness.md: with the INSTANCE
registered, `zones::Monitor::teleport` reaches it natively); a combat check,
where one exists, is in the content that calls `send`.

After the checks: for a non-forced player the script delay becomes
`ticks + 12` and the dialogue closes; a `true` "tablet-spell" attribute turns
any type into TELETABS; the type field is set; the walking queue resets; the
type builds its pulse (FAIRY_RING shows graphic 569 here); the pulse manager
clears; the entity locks for 12 ticks; impacts are disabled for 12 ticks (5 for
a forced teleport); the pulse is submitted; a player's interfaces close.

## Verification

```powershell
cargo xtask teleport-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
cargo test -p rs09-server --test teleport
```

The oracle compiles the unchanged TeleportManager, TeleportPulse, Pulse,
Animator, Animation, Graphics, EntityFlag, Clocks, ActionLocks, Lock,
LockElapse, WalkingQueue, Point, Direction, ClipMaskSupplier, ZoneRestriction
and Audio on JDK 11 with Kotlin 1.8.20 and ConstLib 1.4.28. Original text
inserted into adapters at generation time: the Entity lock, unlock, graphics,
animate, asPlayer, isPlayer, isTeleBlocked, isTeleporting and attribute
members; GameAttributes set/get/remove; the Properties teleport members;
ImpactHandler.setDisabledTicks; ZoneMonitor's jewellery set, isRestricted,
teleport and canTeleportByJewellery; MapZone.isRestricted; the
AnimationDefinition duration members; ContentAPI hasTimerActive/getTimer and
the audio signatures; the Location class pieces; the two attribute events; the
last statement of RegionManager.move. Everything else is a recording adapter
under `oracle/teleport/` (see `adapters.py`). The generator checks the six
Sounds constants and the seventeen jewellery item ids against the jar.

The corpus (`oracle/teleport/cases.py`) has 1,027 cases, 24,229 actions and
40,372 recorded calls, 199 of them single injected failures:

| Group | Cases | What it drives |
| --- | ---: | --- |
| complete | 75 | every type to completion and two ticks past, tick by tick, as a player (type 0, 1, 2 and -1) and as an NPC, with isTeleporting after every walking phase |
| default type, table, far | 14 | `send(location)` over spell books 192/193/430/0 and an NPC; getType; the settings table; teleports far enough to flag a scene rebuild |
| interruptions | 454 | after every pulse phase of every type: `getCurrentTeleport().stop()` (player and NPC), a second send of the same type, a send of another type; PulseManager.clear mid-teleport; `start()` mid-teleport and after completion; stopping the older of two running pulses; stop/start with no teleport |
| blocking | 194 | tele-block timer, teleport lock (active and expired), TELEPORT and other restrictions, zone answers in every order, the second isTeleBlocked (a zone tele-blocking during its own check), a zone removing itself or a later zone from the live list, the tablet-spell attribute as true/false/null/Integer/absent, a null type, a null destination for every type, a null entity location, each as player and NPC and for types 0, 1, 2, 3, -1 and WILDY_TELEPORT |
| zones | 15 | out of a refusing zone, arrival handing the entity a restricting zone, arrival starting, stopping or tele-blocking a teleport, a zone starting a teleport from inside its own check, two entities teleporting in the same ticks |
| mutate, priority, clock | 34 | the destination object changed before and after arrival; a running high-priority end animation against the next type's animations; world clocks of 0, 2147483640 and -5 |
| failure sweeps | 241 | `send`, every pulse step of five types, `stop` and `start`, each with every recorded call failing once in turn (199 of these are the failing copies) |

Each operation records its result or failure, its ordered calls (every world
clock read, timer lookup, attribute get/set/remove and event, zone check,
message, script delay, dialogue/interface close, pulse-manager clear,
submission, audio, animation-definition lookup, update-mask registration,
region move, zone update, log and stack trace), every entity's location (and
whether it is the destination object itself), teleport location, teleporting
flag, footprint, walking points, three lock expirations, animator state and
clock, disabled ticks, script delay, scene flags, attributes, current pulse
and type and zone list, every pulse's class/running/ticks/counter, the runner
queue and the clock. No random generator is reached by these sources, so the
corpus has no draws.

`cargo test -p rs09-server --test teleport` replays every case through the
native library with a host whose pulse runner and `update_base`, Animator,
ActionLocks, walking queue, zone monitor walk and `is_tele_blocked`, script
processor delay field and GameAttributes store are the real native ones, and
compares every field above. A second test pins the fixture and source hashes.
The joined suites are pulses, animator, prepared_masks, update_masks,
action_locks, walking, zones, scripts and attributes.

## What is kept

- A second `send` replaces only the field: the earlier pulse keeps running
  and both set the teleport location. PulseManager.clear does not reach a
  teleport pulse (it is never in the manager). Only `stop()` ends one early.
- `send` never calls `start()`, so "tele-pulse" is not set by a teleport;
  `stop()` removes it (and dispatches the remove event) every time. During a
  spell teleport isTeleporting is true only between the pulse that sets the
  teleport location and the walking queue that consumes it.
- Each type's exact step ticks, sounds (with ANCIENT's seven loops), animation
  priorities (end animations HIGH, TELETABS' second animation HIGH, LUNAR's
  graphic height 120), the stop overrides (unlock and lock 4, 5 or 2, the reset
  animation and graphic) and the types without one.
- FAIRY_RING counts with `++delay`, animates with MID priority (so a running
  HIGH animation rejects both of its animations) and stores the destination
  object itself: mutating it later moves the entity. Every other type copies
  the destination at the step that sets it, so a mutation before that step
  counts and one after does not.
- Teleport type 1 while tele-blocked fails with a NullPointerException in the
  jewellery rule (the node is null); the jewellery set is never consulted from
  TeleportManager.
- A null type fails at `getPulse`, after the delay, dialogue close, type field
  and walking reset; a null destination fails when the pulse copies it and the
  failure is caught by Pulse.update (stack trace, then the stop override).
- RegionManager.move hands the zone monitor the old footprint.
- Lock and clock arithmetic wraps; the corpus runs at clocks near
  `Integer.MAX_VALUE` and below zero.

## Required services

`teleport::Host`: the player check and teleporter lookup, spell book, timers,
`ZoneMonitor.teleport`, `isTeleBlocked`, messages, the world clock, the script
delay, dialogue and interface close, the "tablet-spell" read, the
"tele-pulse" attribute set/remove/read (with their events), the walking reset,
PulseManager.clear, lock/lock()/unlock, setDisabledTicks, the submission, the
entity's location and teleport location, playAudio/playGlobalAudio, the three
Animator calls and `Animation.RESET`. None has a default; none is called with a
lock held.

## Deviations

- The anonymous pulse's counter is an atomic field of the native pulse body;
  the JVM field is a plain int. The values are the same (no type counts far
  enough to wrap).
- The destination copy (`Location.create(location)`) re-packs the decoded
  coordinates into a new native Location; the original copies the packed
  hash. Both give the same packed value.
- `lastTeleport` is kept as a field that is never assigned, as in the
  original.
- `fireRandom` performs its two reads (player check, teleporter type) and
  nothing else, as the original body does.

## Harness limits

The per-tick driver (the runner, then every walking queue, then the reset of
the teleporting and scene flags with the last scene set where a rebuild was
flagged, then the clock) is an oracle input, not a port; it follows
MajorUpdateWorker's order but leaves out every other phase. The harness runner
repeats PulseRunner.updateAll without its telemetry; the native replay gives
the runner a constant clock so no telemetry is produced. The animation
definition lookup is recorded but never fails (the original forId catches
everything). Zone entry on arrival (ZoneMonitor.updateLocation) and the chunk
part of RegionManager.move are recorded services; the zones and
entity-regions corpora verify their bodies. Walking steps, run energy and
route drawing are not driven (the walking corpus covers them). Players have
full run energy and no walking path.

## Not ported

The content that calls `send` (spell handlers, teleport tabs, jewellery,
obelisks, fairy rings, minigame portals, the home teleport pulse, the
`::tele` commands, which set the teleport location directly), every content
zone's `teleport` rule other than the six core zones', the random-event body that
`fireRandom` once had, and Player.isTeleporting's callers.

## What a real binding needs

1. A `Manager` per canonical actor (the Entity's final `teleporter` field)
   and a `teleport::Host` answered from that actor: its attributes store for
   "tablet-spell" and "tele-pulse" (with AttributeSetEvent/RemoveEvent
   dispatch), its script processor's delay, its ActionLocks, its prepared
   Animator, its walking queue, its `zones::Monitor`, its timers, its
   ImpactHandler (`combat_pulse::impact::set_disabled_ticks`), its dialogue
   interpreter and interface manager, its spell book and the audio and message
   packets.
2. The world's `pulses::Context` dispatching teleport handles by identity (a
   registry of live `TeleportPulse`s, never by class name) and the 600 ms tick
   driver running the runner before the entity ticks, and the entity reset
   after rendering.
3. The content that reaches `send`: the magic tab button handlers, item and
   object options, each zone's teleport rule.
