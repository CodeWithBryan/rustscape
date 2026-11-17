# Combat swing calculation

`world::combat_swing` ports the calculations a combat swing makes before
anything is sent or applied: accuracy, defence and maximum hit for the melee,
range and magic handlers, the hit decision, and the `swing` bodies that fill a
`BattleState`. It is a library. Nothing in the controlled sessions calls it yet;
the alpha's hand-written melee stand-in in `examples/common/combat.rs` is
unchanged and is not this port.

The source is the pinned `CombatSwingHandler.kt`, `MeleeSwingHandler.kt`,
`RangeSwingHandler.kt`, `MagicSwingHandler.kt`, `BattleState.java`,
`CombatStyle.java`, `InteractionType.java` and, as far as those reach them,
`Weapon.java`, `RangeWeapon.java`, `Ammunition.java`, `ArmourSet.java`,
`BoltEffect.java` and the attack-style table of `WeaponInterface.java`.

## What is ported

| Original | Native |
| --- | --- |
| `calculateAccuracy`, `calculateHit`, `calculateDefence`, `getSetMultiplier` of all three handlers, with `checkUndead` and the familiar bonus helpers | `Handler::calculate_accuracy`, `calculate_hit`, `calculate_defence`, `set_multiplier`, `check_undead` |
| `isAccurateImpact`, three overloads | `Handler::is_accurate_impact`, `is_accurate_impact_typed`; `hit_chance` exposes the probability |
| `swing` of all three handlers, `configureRangeData`, `hasAmmo` | `Handler::swing`, `configure_range_data`, `has_ammo` |
| `getArmourSet`, `ArmourSet.isUsing` | `Handler::armour_set`, `ArmourSet::is_using` |
| `getCombatDistance`, `formatHit`, `getDragonfireMessage` | `Handler::combat_distance`, `format_hit`, `dragonfire_message` |
| `BattleState` fields, both constructors, `neutralizeHits`, `getTotalDamage` | `BattleState` |
| `Weapon` constructors and `WeaponType` | `Weapon`, `WeaponType` |
| `BoltEffect` table, `forId`, and the base `canFire`/`impact` that `DIAMOND` inherits | `BoltEffect`, `can_fire_base`, `impact_base` |
| `ArmourSet` rows and end graphics, `WeaponInterfaces` styles and interface ids, `SwingHandlerFlag`, `CombatStyle`, `InteractionType` | enums of the same names |
| `RangeWeapon.get`, `Ammunition.get` and the fields `swing` reads | `RangeWeapon`, `Ammunition`, `RangeData` |

A handler is a value: its class (`HandlerKind`), its mutable `type` and its
mutable flag list. The original keeps one instance per combat style and per
special attack; in `world::combat_pulse` the host owns the instances and names
each by a reference.

## Behaviour that is easy to get wrong

- Two generators are involved. The hit decision is one `Math.random()` draw.
  Damage, the Verac roll and the bolt-effect roll are `RandomFunction.RANDOM`
  draws. They are separate `java.util.Random` objects in the original, so the
  host supplies them separately (`math_random`, `next_int`).
- `RandomFunction.random(n)` draws nothing when `n <= 0`. A maximum hit of -1
  therefore costs no damage draw. Magic draws `random(max)`, not `max + 1`, so a
  spell never rolls its own maximum; melee and range draw `random(max + 1)`.
- A melee or range swing that is not accurate draws once. A Verac roll comes
  first and replaces the accuracy draw when it succeeds or when the state
  already carries the Verac effect. A dark bow draws accuracy twice, and its
  second arrow copies the first arrow's damage when the second draw misses.
- Each `properties.attackStyle` in the source is its own read. A player's
  stance is read once when it matches the first comparison and twice otherwise;
  a defender's up to three times. The victim's lifepoints are read again
  inside each cap that applies. The port keeps every read.
- Floating point follows the source step by step: prayer boost, floor, stance,
  the constant 8, set multiplier, floor, style bonus, then the salve or slayer
  factor, with the final conversion truncating toward zero and saturating.
  Integer steps wrap. The magic defence blend is
  `defence * 0.3 + magic * 0.7 + 8` in that order.
- `Weapon`'s constructor ignores its type argument, so a melee swing leaves the
  weapon type unset. Only `configureRangeData` assigns it.
- An unhandled range weapon logs and leaves the state's weapon untouched, which
  stops the combat pulse and answers -1. A dark bow with one arrow becomes a
  `DEFAULT` weapon inside `hasAmmo`.
- A diamond bolt that fires adds `IGNORE_STAT_REDUCTION` to the handler, takes
  1.15 times the maximum hit, and removes the flag. If the calculation between
  fails, the flag stays in the handler; later swings see it.
- Exceptions thrown while reading the familiar are swallowed by the original
  `try`. A failing `familiar_pouch` service therefore means "no familiar".
- The dragon slayer gloves factor applies to accuracy only. The melee handler's
  berserker and void bonuses read the armour set and void state even for
  skills they do not affect.
- The magic swing clears the manual spell after the hit rolls and stops the
  pulse when no autocast remains. A spell that fails its requirements clears
  the spell, resets autocast through the weapon interface when the player has
  one, and answers -1 with the style already set to `MAGIC`.
- Tick counts use the straight-line distance: `1 + ceil(d * 0.3)` for range,
  `2 + ceil(d * 0.5)` for magic, one more for a `BLITZ` spell. Its squared term
  wraps as a Java `int`.

## Required services

Everything outside the listed classes is a method of `Host`, called in the
original order and able to fail: bonuses, attack style, armour set, combat
pulse victim/stop/style update, manual and autocast spell, levels (with and
without the assist flag), static level, lifepoints, maximum lifepoints, prayer
skill bonus, the summoning and protection prayers, skillcape perks, void,
name, id, size, location, the entity's level modifier, equipment reads, the
item bonus configuration, slayer task and slayer equipment bonus, familiar
pouch, the NPC's slayer task and combat distance, messages, the error log,
global audio, graphics, the weapon-interface extension and its
`selectAutoSpell`, the spell's requirement check, base damage, targets and
type, and both random draws. `range_weapon` and `ammunition` are the two static
maps. Entity, item and spell handles are the host's own types and keep their
identity inside a retained `BattleState`.

None of these has a default. Prayer state, equipment effects, NPC combat
definitions, familiars, slayer, perks and spells are not ported here; the
values they would supply are inputs. `world::prayer::combat` holds the
answers to the three prayer services from native prayer state
(docs/prayer.md); no binding uses them yet.

## Reference tests

```powershell
cargo test -p rs09-server --test combat_swing
cargo xtask combat-swing-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK_11
```

`fixtures/combat-swing/` holds **4,461 original JVM cases with 20,762
operations** and the hashes of every source. The oracle compiles the twelve
slice classes and `EquipmentSlot.kt` unmodified with Kotlin 1.8.20 on JDK 11.
`RandomFunction`'s selected methods, `Location.getDistance`, the `Graphics`
constructors and `WeaponInterface`'s `AttackStyle`/`WeaponInterfaces`
declarations are taken verbatim from the pinned text at generation time.
Every other class is a recording stand-in in `oracle/combat-swing/adapters.txt`
that answers from the case and records the call; stand-ins for paths the
corpus does not drive throw.

The corpus covers level, bonus and style grids for each handler in both
directions; every attack style of every weapon interface; each prayer's skill
bonuses alone, in pairs and in ordered triples, summed the way
`Prayer.getSkillBonus` sums them; void, Dharok, berserker and the other sets
alone and combined; perks, familiars, salve amulets, slayer tasks and every
flag combination; player, NPC, familiar and untyped entities on both sides;
integer wrapping and degenerate divisions; swings with seeded generators and
with scripted draws at the extremes; and 615 single swings in which each
service call or draw in turn fails. Expected rows hold the result, the error
class, the ordered service calls and draws, the handler's flags and the whole
state after the swing. The native test replays every row and compares it
whole.

The hit probability is compared bit for bit. The oracle finds, by bisection
over scripted draws through the unmodified `isAccurateImpact`, the least draw
that is not accurate; that value is the probability. The sign of a zero
probability cannot be observed this way and is not compared. In 1,146 of
those operations the oracle also makes three real decisions, one unit in the
last place below the threshold, at it, and above it.

`Math.random()` has no seed parameter. The oracle replaces the private
generator inside `java.lang.Math` once at start with one that reads the case.
Scripted integer draws are reduced modulo the requested bound, so every one is
a value `nextInt(bound)` can return. Threshold probes use draws outside
`[0, 1)`; they are probes, not values `Math.random()` produces.

The pinned bundled `ConstLib-1.4.jar` supplies `Items`, `NPCs` and
`Components`. It has no `Sounds.HUMAN_ATTACK_2564`; one numeric stand-in lets
`visualizeAudio` compile, and that method is not driven.

## Differences from the original

- `BattleState` is an owned value. A target array can hold `Target::This` for
  the state itself, the one alias the ported methods distinguish. One target
  object shared between two arrays cannot be expressed.
- `NullPointerException` and index failures become `Error::Null` and
  `Error::Index` with the same preceding effects. A null `state` argument
  cannot be passed. Other exception classes are not modelled.
- A handler's class is a field, so subclasses and special-attack handlers are
  not represented. The unhandled-weapon log names `RangeSwingHandler`; the
  original names the runtime class.
- A failing familiar service is swallowed whatever its cause. The original
  swallows `Exception`, not `Error`.
- The magic target loop reports a null dereference for `Target::This`. A spell
  is never handed the state, so the original cannot produce that array.

## Not ported

The remaining members of these handlers (`canSwing`, `isAttackable`,
`adjustBattleState`, `getFormattedHit`, `addExperience`, `postSwing`,
`useAmmo`, `impact`, the visuals, `onImpact`, special attack registration and
`ArmourSet.effect`), `CombatPulse` and `ImpactHandler` are ported in
`world::combat_pulse`; see docs/combat-pulse.md. Its host routes this slice's
`pulse_victim`, `stop_combat_pulse` and `update_combat_style` services to that
pulse.

Still not ported: every `BoltEffect` override; `WeaponInterface` behaviour
beyond its style table; `RangedConfigLoader` and the loading of the
animation, graphics and projectile fields of the range tables. `DeathTask`,
graves, `MultiSwingHandler`, special attack handlers, spell effects, prayer
drain, poison and degradation are outside both slices.

Binding `Host` to the canonical player, NPC, equipment, skills and prayer
state, choosing the world's two generators, and joining the result to the
alpha are the next steps. Until then this establishes the
tested calculations only: not playable combat, not ranged or magic combat in a
session, and no performance claim. All source inventory rows remain
`not_replaced`.
