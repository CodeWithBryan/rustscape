# Combat pulse, impact and the handler members around a swing

`world::combat_pulse` ports what the pinned server does around and after a
combat swing: whether a swing may happen, the per-tick `CombatPulse`, what a
swing does to the battle state, experience, ammunition and visuals, the impact
pulse, and `ImpactHandler`. The swing calculation itself stays in
`world::combat_swing` (docs/combat-swing.md). It is a library. Nothing in the
controlled sessions calls it; the alpha's hand-written melee stand-in in
`examples/common/combat.rs` is unchanged and is not this port.

The source is the pinned `CombatSwingHandler.kt`, `MeleeSwingHandler.kt`,
`RangeSwingHandler.kt`, `MagicSwingHandler.kt`, `CombatPulse.kt`,
`ImpactHandler.java`, the `effect` and `visualize` members of
`ArmourSet.java`, and the `Pulse.java` fields and `update` the combat pulses
inherit.

## What is ported

| Original | Native |
| --- | --- |
| `canSwing` of the base, melee, range and magic classes; `isAttackable`; `canStepTowards` with `checkStepInterval`; `canMelee`; `usingHalberd` | `handler::can_swing`, `is_attackable`, `can_step_towards`, `can_melee` |
| `isProjectileClipped` | `handler::is_projectile_clipped`, joined to `pathfinding::projectile::can_reach` |
| `adjustBattleState` of all four classes, `getFormattedHit` | `handler::adjust_battle_state`, `formatted_hit` |
| `addExperience` of the base and magic classes | `handler::add_experience` |
| `postSwing`, `useAmmo`, `getDropRate` | `handler::post_swing`, `use_ammo` |
| `visualize`, `visualizeImpact`, `visualizeAudio`, `getAttackAnimation` | `handler::visualize`, `visualize_impact`, `visualize_audio`, `attack_animation` |
| `impact` of the three classes, `onImpact` | `handler::impact`, `on_impact` |
| `ArmourSet.effect` (Ahrim, Guthan, Karil, Torag and the base) and `ArmourSet.visualize` | `handler::armour_set_effect`, `armour_set_visualize` |
| `register`, `getSpecial` | `handler::register`, `special` |
| `CombatPulse`: `pulse`, `setCombatFlags`, `interactable`, `canInteract`, `updateStyle`, `attack`, `setVictim`, `setNextAttack`, `delayNextAttack`, `start`, `stop`, `isAttacking`, `isInCombat`, and the inherited `update` | `pulse::pulse`, `set_combat_flags`, `can_interact`, `update_style`, `attack`, `set_victim`, `set_next_attack`, `delay_next_attack`, `start`, `stop`, `is_attacking`, `is_in_combat`, `update`; fields in `PulseState` |
| The companion `swing` and its anonymous impact pulse; the second-arrow pulse of `RangeSwingHandler.impact`; the delayed `manualHit` pulse | `pulse::swing`, `WorldPulse` with `Task::Impact`, `Task::SecondHit`, `Task::ManualHit` |
| `ImpactHandler`: both `manualHit` forms, `handleImpact`, `handleRecoilEffect`, `getMostDamageEntity`, `setDisabledTicks`, `isHitUpdate`, the queue and both logs | `impact::manual_hit`, `manual_hit_delayed`, `handle_impact`, `handle_recoil_effect`, `most_damage_entity`, `set_disabled_ticks`; fields in `ImpactState` |

State lives where the original keeps it. A `PulseState` and an `ImpactState`
belong to one entity; the host keeps them and hands them out through plain
accessors (`combat_pulse`, `impact_handler`), with the handler instances
(`handler`), each handler's special map (`specials`) and the static
`EXPERIENCE_MOD` (`experience_mod`). The library never holds one of those
borrows across a service call, so a host may route `Entity.attack`, the pulse
manager or the swing slice's `stop_combat_pulse` straight back into these
functions.

## Behaviour that is easy to get wrong

- `pulse()` returns in this order: no victim, attacker dead, victim dead,
  attacker's region inactive, victim's region inactive, not interactable.
  When not interactable it answers false while the attacker is moving;
  otherwise it compares the old time-out counter, increments it, and only then
  reads `combatTimeOut`. After that it faces the victim and swings only when
  `nextAttack <= ticks`.
- `interactable()` stops the pulse for a hidden NPC on either side and for an
  NPC that may not start combat, in that order, before asking `canSwing`. A
  result other than `STILL_INTERACT` asks the movement lock and then requests
  a path, whatever the result was.
- An unattackable victim ends the pulse unless the victim's `re-npc`
  attribute is the attacker itself.
- A refused swing clears the temporary handler, calls `updateStyle` and keeps
  the pulse running. A successful one reads `attackSpeed`; a player casting
  uses 5; `STYLE_RAPID`, or a salamander handler with `STYLE_RANGE_ACCURATE`,
  subtracts one; an active miasmic timer multiplies by 1.5 and truncates,
  never for magic.
- The companion `swing` runs armour set, set assignment, handler swing,
  `adjustBattleState`, `addExperience`, `postSwing`, `visualize`, the impact
  visuals when the delay is at most 1, `visualizeAudio`, the armour set effect
  and its graphic, then submits the impact pulse with `delay - 1`. A pulse
  with delay 0 delivers on its first update. Any other shows the impact
  visuals on its first firing, sets its delay to 1 and delivers a tick later.
  A dark bow's second hit is one more pulse, one tick after the first hit.
- `Pulse.update` catches a failure inside the body, prints it and stops the
  pulse. `stop()` after a finished body is inside the same `try`, so a failing
  stop is printed and stop runs again.
- `EXPERIENCE_MOD` is one value for the whole world. `adjustBattleState` sets
  it to 4.0 (0.1 for the Lumbridge dummies, 0.01 for id 757). `addExperience`
  multiplies it in place by the NPC's multiplier and leaves it multiplied.
- Hitpoints experience is `hit * 1.33`, times the NPC multiplier again.
  Controlled divides `hit * mod` by 3.0 for three skills; long range gives
  `hit * (mod / 2)` twice; defensive cast gives `hit * 1.33` magic and `hit`
  defence without the modifier. A magic handler with a spell hands the total
  to the spell instead.
- `adjustBattleState` recurses into target states through the handler's own
  class, with the outer victim. The magic class adjusts each target against
  that target's own victim. A list holding only the state itself is not
  recursed into.
- `getFormattedHit` truncates the entity's formatted hit, then applies the
  Elysian roll (`random(100) < 71`), the Divine drain (only while prayer
  points exceed `hit * 0.15`), the staff bonus read with two equipment
  lookups, and the summoning prayer, before capping to lifepoints.
- `useAmmo` replaces the ammunition before it looks at the drop tile. The
  drop roll is `randomize((5 * (1 + range * 0.01) * rate).toInt())`. The Ava
  roll is `random(100) < 80`; three torso models turn a kept arrow into a
  full-rate drop with a message.
- The range class calls `handleImpact` even for a hit of -1, which heals one
  lifepoint through `skills.hit`. The melee and magic classes skip negative
  hits.
- `handleImpact` runs: dead owner (clears and stops its pulse), familiar to
  owner, disabled ticks, unfinished tutorial (a queued `MISS`), `skills.hit`,
  hitsplat type, damage logs, `fam-exp`, death or redemption, god mode, the
  queue, ring of life. It reads neither its `state` nor its `secondary`
  argument.
- Kill credit walks a `HashMap`. Player damage is keyed by uid and wins over
  NPC damage whenever any exists. Ties go to the first key in iteration
  order, a null source is a key, and the owner itself is passed over. The NPC
  log is ordered by `Object.hashCode()`, a platform input the host supplies.
  A cleared log keeps its table size.
- `isMovingPulse` counts a combat pulse, so `setCombatFlags` clears the
  victim's pulses only when it is neither moving nor fighting.

## Required services

Everything outside the listed classes is a method of `combat_pulse::Host`,
which extends `combat_swing::Host`: attributes, node activity and indexes,
centre and closest occupied tile, the entity's swing handler and
attackability, facing, animations and graphics, `sendImpact`, `checkImpact`,
`onImpact`, the entity's formatted hit, familiar, degrading, prayer points,
poison, experience, spell effects and visuals, equipment replacement and
containment, clipping and projectile flags, ground items, item
configuration, the walking queue, locks, interface closes, the pulse's
movement pulse, the pulse manager, the world runner (`submit`), ticks, the
clock, death, regions, timers, audio, projectiles, `skills.hit` and `heal`,
uid and player lookup, zone type, ring of life, recoil damage and
`printStackTrace`. None has a default. Auto-retaliation is part of
`Entity.onImpact`, a required service; it is not ported.

`bolt_can_fire` and `bolt_impact` stand for the `BoltEffect` overrides other
than `DIAMOND`. They are not ported, and the reference corpus cannot record
them because the unmodified overrides run there: the calls around them are a
transcription, not a JVM-checked result.

## Reference tests

```powershell
cargo test -p rs09-server --test combat_pulse
cargo xtask combat-pulse-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK_11
```

`fixtures/combat-pulse/` holds **7,377 original JVM cases with 24,619
operations** and the hashes of every source; 2,577 of the cases are single
injected failures. The oracle compiles `CombatPulse.kt`, `ImpactHandler.java`,
`Pulse.java`, the four swing handlers, `BattleState`, `CombatStyle`,
`InteractionType`, `Weapon`, `RangeWeapon`, `Ammunition`, `ArmourSet`,
`BoltEffect`, `EquipmentSlot`, `Direction`, `Point`, `Path`,
`ProjectilePathfinder`, `ClipMaskSupplier` and `ZoneType` unmodified with
Kotlin 1.8.20 on JDK 11. Selected text is taken verbatim at generation time:
`RandomFunction` methods, the `Location` packing and the members combat
reads, the `Pathfinder` constants and `isStandingIn`, `Node.getCenterLocation`,
`Entity.getClosestOccupiedTile`, `getOccupiedTiles` and `attack`. Every other
class is a recording stand-in from `oracle/combat-swing/adapters.txt` and
`oracle/combat-pulse/adapters.txt`; stand-ins for paths the corpus does not
drive throw.

The corpus covers melee reach on a 7x7 grid for plain, halberd, moving,
running and multi-tile combinations and for a victim already fighting the
attacker; walls on the stepped tile, both corner tiles and the projectile
path in each direction; long and oblique approaches; every branch of
`isAttackable`; range and magic reach by distance, interface, stance and
weapon type; projectile footprints; `adjustBattleState` over attackers,
victims, hit boundaries, every poison and shield draw and target shapes;
experience for every attack style, victim kind and modifier; ammunition use;
visuals and audio; impacts; `ImpactHandler` over hitsplat types, lifepoint
thresholds, disabled ticks, delayed hits, recoil and kill credit with
colliding and resizing logs; every early return of `attack`, `pulse` and
`update`; attack speed sources; and 70 fights of 22 ticks with recorded
per-tick decisions for each style and attack speed, plus 38 shorter fights
whose world changes underneath them (4,410 driven ticks in all). In 96 single
operations each service call and draw in turn fails. Expected rows hold the result, the error class, the ordered
service calls and draws, the battle state, and every pulse, impact handler,
queued pulse, handler and `EXPERIENCE_MOD` whenever an operation changed
them. The native test replays every row and compares it whole.

What the stand-ins model is an oracle input, not a port: lifepoints with the
original `hit`/`heal` arithmetic, prayer points, recoil damage, equipment
replacement that empties a slot at amount zero, attributes, death after
`startDeath`, an optional retaliating `onImpact`, a pulse manager that knows
only the combat pulse, and a runner that updates a fixed prefix of one queue
each tick. `System.currentTimeMillis` is not bound in the unmodified pulse;
only the +10000 offset of `combat-time` is recorded. `Entity.hashCode` is a
case input. `SalamanderSwingHandler` is a class-identity stand-in that
inherits the range class; it is only driven where that range swing does not
log under the stand-in's name.

## Differences from the original

- Entities and handlers passed to these functions are never null where the
  original parameter is a Kotlin non-null type, and a combat pulse always has
  an entity. `attack` takes an entity, not any `Node`.
- `NullPointerException` and index failures become `Error::Null` and
  `Error::Index` with the same preceding effects; other exception classes are
  not modelled. `update` catches every failure of the body; the original
  catches `Exception`, not `Error`.
- A handler's class is a field, as in the swing slice. A subclass's
  overriding `swing`, `visualize`, `visualizeImpact`, `impact`,
  `adjustBattleState` and `getFormattedHit` are answered per instance by
  `Host::overrides` (`overrides.rs`); the weapon special attacks
  (`world::special_attacks`, docs/special-attacks.md) are the only subclasses
  joined that way. A subclass overriding `canSwing` or the other members
  cannot be represented yet.
- `BattleState` is an owned value. Target handling reads a target array by
  index and takes one child out while it is being adjusted; a service that
  replaced the array of the state it was handed would be seen sooner than in
  the original's captured array.
- An NPC damage log bucket that the JDK would convert to a tree (nine or more
  colliding sources in a table of 64 or more) is refused with an error; its
  order depends on identity hashes inside the tree.
- Coordinates follow the original fifteen-bit packing, but the corpus only
  uses coordinates inside that range.

## Not ported

`removeFor`; `lastSentAttack`/`lastReceivedAttack` users; `BoltEffect`
overrides other than the `DIAMOND` path; every handler subclass other than
the special attacks (docs/special-attacks.md), `MultiSwingHandler` and
`SalamanderSwingHandler`;
spell effects and spell visuals; `DeathTask` and graves; prayer drain (now
`world::prayer`, docs/prayer.md), poison and degradation themselves; `Entity.onImpact`, `sendImpact`, `checkImpact`,
`isAttackable` and auto-retaliation; `RangedConfigLoader`.

Binding `Host` to the canonical actors, the native pulse manager and runner
(`server::pulses`), skills, equipment, attributes, locks and movement, sending
the queued impacts through the update masks, and replacing the alpha stand-in
are the next steps. Until then this establishes the tested decisions and call
orders only: not playable combat in a session, and no performance claim. All
source inventory rows remain `not_replaced`.
