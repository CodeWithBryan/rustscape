# NPC behaviour: aggression, behaviour hooks and the combat members

`world::npc_behaviour` ports what makes an NPC act on its own once the
`world::npc` tick kernel reaches it: aggression (`AggressiveHandler`,
`AggressiveBehavior`), the `NPCBehavior` hooks content overrides, the
`NPC.java` members `tick` leaves to its callers, and the members of
`NPC.java`, `Entity.java` and `Player.java` on the combat path (attackability,
`continueAttack`, `checkImpact`, `onImpact` with auto-retaliation,
`sendImpact`, `inCombat`, the NPC's swing handler). It is a library. No
example, alpha or session installs it, and the alpha's combat stand-in in
`examples/common/combat.rs` is not this port. This is not a complete NPC,
Entity or Player class and not playable combat.

## What is ported

| Original | Native |
| --- | --- |
| `AggressiveHandler`: fields and defaults, `selectTarget`, `setPauseTicks`, `removeTolerance`, `isAllowTolerance` | `aggression::Handler` (`select_target`, `set_pause_ticks`, `remove_tolerance`, `is_allow_tolerance`) |
| `AggressiveBehavior` base members `canSelectTarget`, `ignoreCombatLevelDifference`, `getPriorityFlag`, `getPossibleTargets`, `getLogicalTarget`; `DEFAULT`; `WILDERNESS`'s `canSelectTarget` | `aggression::AggressiveBehaviour` (trait defaults), `DefaultAggression`, `Wilderness`, `base_can_select_target` |
| `NPC.setDefaultBehavior`, `configureBossData` | `aggression::set_default_behaviour`, `configure_boss_data` |
| `NPCBehavior` (every hook with its default), `forId`, `register`, `StandardClipMaskSupplier` | `Behaviour` trait, `DefaultBehaviour`, `Registry`, `ClipSupplier` |
| The overridable `NPC` members `canAttack`, `canStartCombat`, `isIgnoreAttackRestrictions`, `onRespawn`, `isAttackable`, and `Entity.canSelectTarget` | `NpcClass` trait with those bodies as defaults, `PlainNpc` |
| `NPC.isInvisible`, `isHidden`, `continueAttack`, `isAttackable`, `checkImpact`, `getSwingHandler` | `combat::npc_is_invisible`, `npc_is_hidden`, `npc_continue_attack`, `npc_is_attackable`, `npc_check_impact`, `npc_swing_handler` |
| `Entity.inCombat`, `isAttackable`, `checkImpact` (safespot rule), `onImpact` (retaliation), `sendImpact`, `isIgnoreMultiBoundaries` | `combat::in_combat`, `entity_is_attackable`, `entity_check_impact`, `entity_on_impact`, `send_impact`, `is_ignore_multi_boundaries` |
| `Player.isAttackable`, `continueAttack`, `onImpact` (vengeance, ring of recoil, degradation) | `combat::player_is_attackable`, `player_continue_attack`, `player_on_impact` |
| Runtime-class dispatch of `isAttackable`, `checkImpact`, `onImpact`, `isInvisible` | `combat::is_attackable`, `check_impact`, `on_impact`, `is_invisible` |
| `NPC.onRegionInactivity`, `Entity.fullRestore` | `lifecycle::on_region_inactivity`, `full_restore` |
| The tick's behaviour, aggression, respawn, restore and inactivity steps | `tick::query`, `tick::event` (see below) |

State lives where the original keeps it: `Ai` holds the NPC's
`aggressiveHandler`, `behavior`, runtime class, slayer `task` and
`Entity.invisible`; `world::npc::Fields` keeps the fields the tick kernel
already owned (`aggressive`, `walkRadius`, `respawnTick`, the return-to-spawn
attributes). `set_default_behaviour` keeps `Fields.has_handler` equal to the
handler's presence. An `aggression::Handler` is a shared object: a running
`selectTarget` keeps the handler it was called on, as the original does.

## What is joined to native pieces

- `tick::query` and `tick::event` plug into the existing `world::npc` tick
  kernel (`NPC.tick`/`handleTickActions`): `Behavior` is the behaviour's
  `tick`, `SelectTarget` the NPC's handler's `selectTarget`, `Attacking` and
  `InteractionLocked` the two services; `Inactivity` is
  `onRegionInactivity`, `BehaviorRespawn` the behaviour's `onRespawn`,
  `Respawn` the class's `onRespawn`, `Restore` `fullRestore` and
  `PauseAggression` the handler's `setPauseTicks`. The corpus's tick runs go
  through the unchanged kernel this way.
- `onRegionInactivity` runs the return pulse's arrival body through the
  binding's `return_arrival`, which the replay answers with the existing
  `world::npc::State::return_arrival`.
- Tolerance: the replay answers `is_tolerated` with the native
  `entity_regions::is_tolerated` over a real native `region::Region`'s
  tolerance map, so the two clock reads and the name lookup are native.
- `BattleState` is the native `combat_swing::BattleState`; the error type is
  the combat slices' (`Null`, `Index`, `Binding`).

Everything else is a required `Host` service, listed in `npc_behaviour/mod.rs`
in the original's terms: the combat pulse (`isAttacking`, `getVictim`,
`setVictim`, `attack`, `stop`, `getNextAttack`, `getStyle`, the last sent and
received attack ticks), `DeathTask.isDead`, locks, attributes, definition
configuration and actions, `getShownNPC`, `RegionManager.forId` and
`getLocalPlayers`, `WildernessZone.isInZone`, rights, the skull manager,
`GameWorld.getSettings`, the zone monitors' `continueAttack` and `isPvPable`,
skills, the familiar owner, messages, `Pathfinder.find`/`walk` for the
safespot rule, `moveStep`, walking and pulse queries, equipment, `sendChat`,
`manualHit`, `handleRecoilEffect`, the armour degrader, walking/pulse/mask
resets, chunk membership, the repository, `NPC.clear`, skills restore, timers,
the clipping flag, and the two generators (`RandomFunction.RANDOM.nextInt`,
`Math.random`). None has a default.

## Behaviour that matters for gameplay

- `selectTarget` order: pause (`pauseTicks > ticks`, an absolute tick), the
  interaction lock, an existing fight unless target switching, the NPC's own
  death, then one `RANDOM.nextInt(10)` that must not exceed the chance ratio
  (2 by default, 6 for bosses; a ratio of 10 or more always passes, below 0
  never). Then the local players within the radius (4 by default, the
  `agg_radius` configuration, 64 for bosses) are filtered by
  `canSelectTarget`, and the target is the lowest priority flag plus one
  `Math.random()` each, ties to the later candidate (`<=`). A chosen player
  with `ignore_aggression`, or an administrator without
  `allow_admin_aggression`, stops it. The target gets `aggressor` set; the
  NPC sets its victim and faces it when already fighting, otherwise attacks.
  With no target the answer is whether the NPC is attacking.
- `canSelectTarget`: inactive or dead targets are refused; a target in
  combat (`combat-time` later than now) is refused outside a multi-way zone;
  a player is refused while the region tolerates them (strictly more than ten
  minutes since their entry), but only when the NPC has a handler that allows
  tolerance (field and the definition's `can_tolerate`, read first) and is not
  in the wilderness; and a target whose current combat level exceeds twice the
  NPC's (an `int` shift that wraps) is refused. `WILDERNESS` adds
  invisibility, the class's `canSelectTarget` and the skull's wilderness
  flag, and drops tolerance and the level rule.
- Priority flag: one each for being in combat, being interaction-locked, and
  having an `aggressor` whose victim is this target.
- `isAttackable` of an NPC: invisible (inactive or before its respawn tick),
  the slayer level message (it does not refuse), the behaviour's
  `canBeAttackedBy` (a player needs the definition's `attack` or `smash-ice`
  option), death, the attacker's zone monitor, then the victim's. A player
  victim refuses an NPC without an `attack` option unless its class ignores
  the restriction, and applies the wilderness and PvP rules to players.
- `checkImpact`: the last received attack is the current tick; more than 10
  ticks since the NPC's last sent attack on a `safespot` NPC paths both sides
  towards each other, more than 40 also steps and stops both. A slayer task
  above the attacker's (or familiar owner's) slayer level neutralises the
  hits and poison, and a magic estimate becomes -1.
- `onImpact` retaliates when retaliating, not attacking, not locked and the
  next attack tick is before now; a player must also have no path and no
  moving pulse, an NPC retaliates regardless. A dead victim's hits are
  neutralised first. A player then takes vengeance (75% of the hit,
  truncated), recoil on the estimate and the secondary hit with ring 2550,
  and the degrader check.
- `onRegionInactivity` resets walking, pulses and masks; an NPC returning to
  spawn is moved to its spawn chunk at once, its return pulse's arrival body
  runs and that pulse stops; then the NPC leaves the renderable list and is
  cleared in a dynamic region.
- Kotlin refuses null arguments of the `NPCBehavior` hooks before any hook
  body runs; the native call sites fail with `Error::Null` at the same point.

## Evidence

```powershell
cargo test -p rs09-server --test npc_behaviour
cargo xtask npc-behaviour-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

`cargo xtask npc-behaviour-oracle` compiles `AggressiveHandler.java`,
`AggressiveBehavior.java`, `NPCBehavior.kt`, `ContentInterface.kt` and
`ClipMaskSupplier.java` unchanged with Kotlin 1.8.20 on JDK 11. The members
named in the fixture's `scope` (the NPC, Entity and Player members above, and
`NPC.tick`, `handleTickActions`, `walkToNextDest`, `setNextWalk`,
`getMovementDestination`; `Region.isTolerated`; `BattleState.neutralizeHits`;
`RandomFunction`, `Location` and `Vector` members) are original text inserted
into the adapters of `oracle/npc-behaviour/adapters.py` at generation time.
Everything else is a recording adapter answering from the case.

`fixtures/npc-behaviour/` holds **1,243 original JVM cases with 8,625
actions and 118,676 recorded calls**, 370 of them single injected failures
(every recorded call of a marked operation fails once in turn). Groups:
selection (seeds covering every first draw against each chance-ratio
boundary, pause and lock boundaries, switching, death, rights and attributes,
radius values, the wilderness behaviour), several candidates and priority
flags against seeded `Math.random`, tolerance at and around the ten-minute
boundary with every gate and the wrapping level rule, the behaviour hooks
with the default and a registered content behaviour and the registry,
attackability and `continueAttack` for NPC, familiar and player victims,
`checkImpact`, `onImpact` and retaliation grids, `sendImpact` and `inCombat`
at their boundaries, region inactivity, `fullRestore`, the default behaviour
and boss data, and 27 designed tick runs (112 cases with their failure
variants) through the unchanged tick text with the real `AggressiveHandler`: aggression over 30 ticks with sticky and
non-sticky combat, three runs of about a thousand ticks crossing the
tolerance boundary (the NPC stops choosing the player from the first tick
strictly after ten minutes), multi versus single combat over 12 ticks,
respawn timing with content hooks, inactivity while returning to spawn,
wandering and return-to-spawn pauses.

Each row records the result or error class, the ordered recorded calls and
draws, and the whole state afterwards (attributes, the combat-pulse model,
locations, the NPC's fields, its handler's fields and its behaviour). The
native test replays every row and compares it whole, and checks the fixture
hash and that the six main sources are pinned.

What the stand-ins model is an oracle input, not a port: a combat pulse that
records `attack` and, unless the case turns that off, then reports attacking
that victim until `stop`; answers for locks, death, zones, definitions and
skills; a case clock and tick counter. The harness clears `NPCBehavior`'s
private id map before each case. Reads of the tick kernel's own four
attributes (`no-spawn-return`, `return-to-spawn`, `return-to-spawn-pulse`,
`lastForceTalk`) are not recorded because `world::npc` keeps them as fields;
attribute writes happen and are then recorded, as in the npc-tick oracle.

## Differences from the original

- `playerTolerance` is only ever written with zero, so its elements are not
  stored; its length is kept for `removeTolerance`'s bounds. A caller that
  wrote into the array returned by `getPlayerTolerance` is not representable.
- Attributes are typed by the host; a value of the wrong type is a host
  error, not a `ClassCastException` (not driven by the corpus).
- A behaviour, aggressive behaviour or NPC class is a trait object; dispatch
  by runtime class is the binding's choice of object, not reflection.
- `NPC.getSwingHandler` reads only the style's handler; the `swing` argument
  is unused in the original too.

## Not ported

`AbstractNPC` (its mapping and duplicate-name warning, which compares
`getSimpleName()` strings by identity) and `IdleAbstractNPC` are
`world::npc_classes` (docs/npc-classes.md); `NPC.init`,
`clear`, `transform`, `openShop`, `getShownNPC`, `initConfig`/`configure`
(their own slice is `world::npc_init`), `getAudio` and the poison members;
`Player.fullRestore`; the `Entity` members not listed above. The real
`CombatPulse`, `ImpactHandler` and `DeathTask` are services here; their own
corpora (`docs/combat-pulse.md`, `docs/npc-death.md`) verify them, and **no
corpus yet runs a whole fight through the real combat pulse, impact handler,
death task and these members together** (first sight to respawn). That joined
corpus needs the combat-pulse oracle's stand-in world merged with these
adapters and is the next step.

## What a binding needs

A tick driver keeps one `Ai` per NPC next to its `world::npc::State`, builds
the behaviour from a `Registry` (`for_id(id)`) and the class from the NPC's
content, and in its `world::npc::Context` hands every query and event to
`tick::query`/`tick::event` first. Its `npc_behaviour::Host` answers the
combat-pulse services with `combat_pulse::pulse::{attack, set_victim, stop,
is_attacking}` and the `PulseState` fields, `is_dead` with
`npc_death::is_dead`, `is_tolerated` with `entity_regions::is_tolerated`,
`local_players` with `entity_regions::local(.., Select::Players)`, the zone
monitor calls with `zones::Monitor`, and `return_arrival` with
`world::npc::State::return_arrival`. In the other direction,
`combat_pulse::Host::entity_attackable`, `entity_on_impact`, `check_impact`,
`send_impact`, `swing_handler` (for an NPC), `npc_hidden`,
`npc_can_start_combat`, `npc_xp_multiplier` and `before_attack_finalized`,
and `npc_death`'s behaviour callbacks, are answered by this module. NPC
initialisation calls `aggression::set_default_behaviour` for
`npc_init::Context::default_behavior` and the behaviour's `on_creation` for
`behavior_created`. None of these bindings exists yet.
