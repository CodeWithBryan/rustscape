# Magic spell framework

`world::spells` ports the framework that magic combat and spell casting run
on: rune requirements with combination runes and elemental staves, the cast
pipeline, the spell books with their registered combat spells, spell blocks
and autocast selection. It is a library. The live world (`--world`)
installs it for the combat spells whose `fireEffect` is the empty base (the
standard book's strikes, bolts, blasts and waves, Crumble Undead, Iban
Blast, Magic Dart and Sonic Wave), cast with the spell-on-entity packets and
from the combat pulse; the rest is refused there by name
(docs/live-world.md). Nothing in the controlled sessions calls it.

The sources are the pinned `MagicSpell.java`, `CombatSpell.java`,
`DefaultCombatSpell.java`, `SpellType.java`, `Runes.java`,
`CombinationRune.java`, `MagicStaff.java`, `SpellBlocks.java`,
`SpellBookManager.java`, `SpellUtils.kt`, the autocast members of
`WeaponInterface.java`, and the seventeen combat spell classes of the
standard and ancient books: `AirSpell`, `WaterSpell`, `EarthSpell`,
`FireSpell`, `BindSpell`, `CurseSpells`, `GodSpells`, `CrumbleUndead`,
`IbanBlast`, `MagicDart`, `TeleblockSpell`, `SonicWaveSpell`, `IceSpells`,
`BloodSpells`, `ShadowSpells`, `SmokeSpells` and `MiasmicSpells`.

## What is ported

| Original | Native |
| --- | --- |
| `Runes` (ids, `forId`, `isInfinite`), `CombinationRune` (`types`, `eligibleFor`), `MagicStaff` (`forId`, staves) | `runes::Rune`, `is_infinite`, `CombinationRune`, `MagicStaff` |
| `MagicSpell.usingStaff`, `SpellUtils.usingStaff`, `SpellUtils.hasRunes` | `runes::using_staff`, `has_runes` |
| `MagicSpell.meetsRequirements` (with `GodSpells`' override), `checkLevelRequirement`, `addExperience` (with `CurseSpells`' override), `getDelay`, `getExperience` | `cast::meets_requirements`, `check_level_requirement`, `add_experience`, `delay`, `experience` |
| `MagicSpell.castSpell` | `cast::cast_spell` |
| `CombatSpell.cast`, `visualize`, `visualizeImpact`, `getTargets`, and the overrides of each class; every class's `getMaximumImpact` | `cast::cast`, `visualize`, `visualize_impact`, `get_targets`, `maximum_impact` |
| `SpellType` (accuracy modifiers, `getImpactAmount` of every type) | `SpellType::accuracy_mod`, `impact_amount` |
| `DefaultCombatSpell` (constructor, `visualize`, `getMaximumImpact`) | `cast::default_combat_spell` and the `Default` class |
| `SpellBookManager` (`setSpellBook`, `update`, `getSpellBook`), `SpellBook` (`forInterface`, `register`, `getSpell`), `SpellbookChangeSource` | `book::SpellBookManager`, `SpellBook`, `Registry`, `SpellbookChangeSource` |
| `SpellBlocks` | `book::SpellBlocks` |
| `WeaponInterface.getAutospellId`, `selectAutoSpell`, `canAutocast` and the four autocast spell tables | `autocast::autospell_id`, `select_auto_spell`, `can_autocast` |
| The 53 combat spell registrations (33 standard including Sonic wave at 63232, 20 ancient): level, experience, runes, type, animation, start and end graphics, cast and impact audio, projectile templates | `table::combat_spells` |

A spell is an object the host owns: `Spell` holds the `MagicSpell` and
`CombatSpell` fields and the concrete class, whose overridden members the
functions dispatch on. Two fields change after construction, as in the
original: `spell_id` (set by `register`) and `projectile` (replaced by
`DefaultCombatSpell.visualize`).

## Behaviour that is easy to get wrong

- `hasRunes` checks the staff once per listed rune (one `equipment.get(3)`
  each). Combination runes are used first, in declaration order: each takes
  up to the largest remaining cost of its types, records an entry even when
  it uses none (amount 0), and lowers every one of its types by that usage,
  inserting keys that were not costs. The leftover walk follows the
  `HashMap<Integer, Integer>` table order (JDK 11, resizes included), so the
  missing rune reported and the removal order are the JDK's. Sums wrap.
- The removal list is the `spell:runes` attribute. It is written only when
  every rune is present and is left stale otherwise; `meetsRequirements`
  reads it with an empty-list default, removes each item, then removes the
  attribute. A staff of light (14726) negates the runes when
  `(int) (Math.random() * 101) < 13`, before any rune is checked or removed.
- `meetsRequirements` checks the autocast spell, not the spell being cast:
  with Magic Dart (31) or Claws of Guthix (42) on autocast and no Slayer's
  staff or Void knight mace, any spell fails, and the message is sent
  whatever the message flag. The weapon is read twice; an empty weapon slot
  with an autocast spell fails as a null.
- Bind (12), Snare (30) and Entangle (56) compare `entangleDelay` with the
  tick and answer through `Player.sendMessage`, not the packet dispatch.
- God spells: an NPC caster always passes. A player with fewer than 100
  casts outside the mage arena is told (the count is read again for the
  message); the staff message has two spaces before "to cast". Claws of
  Guthix accept the Void knight mace. `visualizeImpact` counts the cast, and
  on a splash plays all three failure sounds, because the three projectile
  constants it compares with are all null; the impact sound has delay 0.
- `CombatSpell.visualize` animates 1162 (1576 for Magic Dart) when the
  entity's autocast spell is this spell; an NPC with that autocast and no
  animation fails the original's cast to `Player`. An NPC's magic animation
  is read twice. The ancient and teleblock visualizations cast the target to
  an entity and never transform the projectile as an NPC's.
- `visualizeImpact` on a splash plays the failure sound before the splash
  graphic; on a hit it shows the end graphic before the impact sound. Ice
  spells show the barrage orb (1677) when the state is frozen.
- `addExperience` gives `hit * 1.33` hitpoints experience, and on defensive
  cast `(EXPERIENCE_MOD * hit) / 2.0` defence and magic; curses use the
  two-argument `addExperience` and nothing for the hit.
- Crumble undead reads the NPC id once per Zogre and Skogre id until a match
  and fails as a null for a target that is not an NPC. Teleblock and the
  miasmic spells check the requirements twice (their own check and the
  inherited `cast`).
- `castSpell` reads the target's location three times, uses the Euclidean
  `withinDistance(15)` on the same plane, and reads the tick a third time
  for the new `magic-delay`.
- `getMaximumImpact`: strike against NPC 205 is `8 + base`; bolt with chaos
  gauntlets (777) `11 + base`; god strike 30 only with the spell-charge timer
  and a god cape (2412 to 2414); Magic Dart `10 + magic / 10`; the trident
  type `(magic - 75) / 3 + base` with Java truncation; the miasmic base is
  read from the animation id. `DefaultCombatSpell` computes
  `(int) (14 + level + bonus / 8 + level * bonus * 0.016865) / 10 + 1` with
  wrapping integer steps; its spell id is 0 and its rune array is empty, not
  null.
- `SpellBlocks.isBlocked` reads both names for every listed node and never
  stops early; a null name of the queried node fails at the first element.
- `selectAutoSpell` reads the weapon name up to five times, the autocast
  spell after picking the table, sends the "on" configs of the old spell
  before forgetting, and for the Slayer's staff and Void knight mace tables
  sends nothing for a config outside 66..84.

## Required services

Everything outside the ported classes is a method of `spells::Host`, called
in the original order: the tick, development mode, node names and
locations, levels (with and without the assist flag), both
`addExperience` forms, worn items (`get` and `getNew`), inventory amounts and
removals, both message paths, interface configs, the cast event, facing,
graphics, animations and forced animations, global audio,
`Projectile.magic` and projectile transform-and-send, attributes, the
autocast and current spell, the combat pulse members `cast` calls, the
attack style and its index, magic and attack animations, multi-combat zones,
bonuses, zones, timers, god casts, NPC ids, slayer tasks and combat
graphics, multi-hit targets, tabs, varbits, component visibility,
`setAttackStyle` and `Math.random`. None has a default. Plain accessors (not
calls): node kind and identity, the spell object, the registry, item
definition names, `EXPERIENCE_MOD`, the player's spell book field, whether
the weapon interface is the staff one, and the god-cast array write.

Each spell's `fireEffect` (freeze, stat drains, Blood healing, Smoke
poison, binding, teleblock and miasmic timers) is content: not ported, and a
required service of the combat host (`spell_fire_effect`).

## Joined to the combat slices

`spells::combat` gives the answers, in the combat slices' shapes and error
type, to `combat_swing::Host::spell_meets_requirements`,
`spell_maximum_impact`, `spell_targets`, `spell_is_blitz` and
`select_auto_spell`, and to `combat_pulse::Host::spell_id`,
`spell_add_experience`, `spell_visualize` and `spell_visualize_impact`. A
second replay runs the whole corpus with those operations answered there
(14,401 operations). No corpus runs `MagicSwingHandler` or `CombatPulse`
with the real spell classes; the combat corpora still use their recording
spells, and no combat host in the repository calls these answers yet.

`table::persisted_registry` fills the canonical player's save registry
(`player::persisted::spells::Registry`) from the table; it holds only the
combat spells.

## Reference tests

```powershell
cargo test -p rs09-server --test spells
cargo xtask spells-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK_11
```

`fixtures/spells/` holds **8,600 original JVM cases (5,238 designed and
3,362 single injected failures) with 31,181 operations and 142,715 recorded
calls**, and the hashes of every source. The oracle compiles the framework
classes, `Audio.kt`, `SpellUtils.kt` and the seventeen combat spell classes
unmodified with Kotlin 1.8.20 on JDK 11 and the published ConstLib 1.4.28,
whose `Sounds`, `Items`, `NPCs` and `Components` values are checked against
the suffixes of the names used. The classes register themselves once through
their own `newInstance`. `RandomFunction.getRandom`, the `Location` packing
and distance, the `Animator.Priority` constants, the equipment slot and skill
constants, the `SpellCastEvent` declaration, the Zogre and Skogre id arrays,
four `ContentAPI` attribute functions and `WeaponInterface`'s autocast tables,
constants and three methods are original text inserted at generation time;
`Math.random`'s generator is replaced by the case's script. Every other class
is a recording stand-in in `oracle/spells/adapters.py`; stand-ins for paths
the corpus does not drive throw.

The corpus covers the static tables; `isInfinite` for every rune against 21
weapons and 11 type arrays; every `SpellType` impact over entity kinds,
levels and integer extremes; `usingStaff` for 33 weapons and every rune and
combination id; `hasRunes` for every spell exactly, one short in each rune,
with staves, with every combination rune at 0, 1 and 100, 600 random
requirement/inventory/weapon sets and wrapping sums; `meetsRequirements` of
every spell below, at and above its level with both flags, missing runes,
NPC casters, autocast staves, the binding delay, staff-of-light draws at the
13/101 boundary and god spells over casts, arena and staves; repeated casts
draining split and combination stacks and a stale `spell:runes`; experience
for every spell, style and modifier; visualizations and impacts for every
spell, caster kind, autocast and target; maximum hits; targets in and out of
multi-combat; every cast gate; `castSpell` over the magic delay, books,
distance and planes; book switching and updates; spell blocks; autocast
selection over both books, the four tables, both styles and every button;
and `DefaultCombatSpell` over combat-graphics shapes, levels and bonuses.
Expected rows hold the result, the error class, the ordered calls and draws,
the static `EXPERIENCE_MOD`, the draw count and the whole state (attributes,
autocast, inventory, spell book, god casts) whenever an operation changed it.
The native test replays every row and compares it whole.

The inventory removal is a stand-in (from the first slot holding the id,
emptied slots leave): the corpus establishes what is removed and in which
order, not `Container.remove` itself, which the inventory port covers.

## Differences from the original

- Spell objects are host-owned values with a class field. Spell subclasses
  other than these eighteen classes cannot be represented.
- `NullPointerException`, index failures and `ClassCastException` become
  `Error::Null`, `Error::Index` and `Error::Cast` with the same preceding
  effects; other exception classes are not modelled. In the combat slices'
  error type a cast failure travels as `Binding("java.lang.ClassCastException")`.
- The registry is built from the table, not by the plugin scan running each
  class's `newInstance`; a registry built from it has no non-combat spells.
- Rune arrays cannot hold null elements; the original's array of a combat
  spell never does.
- Attribute values are `Int` or an item list; another stored type read as an
  int or a list fails as a cast.
- `SpellBlocks` is a value the world owns, not a process-wide static.
- `projectile.transform(...).send()` is one service naming the template by
  the field that holds it; the original mutates that shared template's
  source, speed and (for NPCs) distance, which is the projectile engine's
  state and not modelled here.

## Not ported

Every `fireEffect`; the non-combat spells of the standard book (teleports,
alchemy, enchanting, charge, telekinetic grab, bounty locate, teleother) and
the whole lunar book; `SpellListener`/`SpellListeners` and the modern
listener-based spells; `SpellUtils.validateSpellbookInterface` and
`getBookFromInterface`; spell packets; `WeaponInterface` beyond the three
autocast members (`openAutocastSelect`, `restoreAutocast`, styles, tabs);
`PCTorcherNPC`'s spell; the projectile engine. Binding `Host` to the canonical
player (attributes, containers, skills, properties, the spell-book field,
saved god casts), installing a registry and spell blocks in a world, and
answering the combat hosts' spell services from these functions in a session
are the next steps. Until then this establishes the tested decisions and call
orders only; all source inventory rows remain `not_replaced`.
