# Prayer: the manager, the prayer types and their combat reads

`world::prayer` ports the player's prayers as the pinned server runs them:
switching a prayer on and off, the conflicts between prayers, the drain each
tick, the head icon, the varps and sounds, the reset, and the redemption and
retribution effects. `world::prayer::combat` answers the prayer reads of the
combat slices (`world::combat_swing`, `world::combat_pulse`) from the same
native state. It is a library. Nothing in a session or the alpha calls it
yet; it is not playable prayer in a world.

The source is the pinned `Prayer.java`, `PrayerType.java`,
`PrayerCategory.java`, `PrayerTabInterface.kt` (the prayer tab's button
listener), `Player.commenceDeath` and `Player.fullRestore` (the two members
that start prayer effects), and, for the combat reads,
`Player.hasProtectionPrayer`, `NPC.hasProtectionPrayer`,
`Entity.getFormattedHit` and `Entity.isIgnoreProtection`.

## What is ported

| Original | Native |
| --- | --- |
| `PrayerType` constants (level, drain, varp, button, both restrictions, sound, defence requirement, skill bonuses), `get(button)`, `getByBonus`, `getMeleeTypes`/`getRangeTypes`/`getMagicTypes` | `PrayerType`, `Row`, `PrayerType::row`, `for_button`, `by_bonus`, `melee_types`, `range_types`, `magic_types` |
| `PrayerCategory` | `Category` |
| `PrayerType.permitted`, `toggle`, `flag`, `iconify`, `findNextIcon`, `getIcon`, `hasIcon`, `ICON_CACHE` | `type_permitted`, `type_toggle`, `flag`, `iconify`, `find_next_icon`, `PrayerState::icon`, `has_icon` |
| `Prayer.toggle`, `reset` (with its one-tick head-icon pulse), `startRedemption`, `startRetribution`, `tick`, `getSkillBonus`, `get`, the private `permitted` | `toggle`, `reset`, `head_icon_pulse`, `start_redemption`, `start_retribution`, `tick`, `PrayerState::skill_bonus`, `PrayerState::get` |
| `PrayerTabInterface`'s handler on component 271 | `tab_button`, `PRAYER_TAB` |
| `Player.commenceDeath`, `Player.fullRestore` | `commence_death`, `full_restore` |
| `Player`/`NPC.hasProtectionPrayer`, `Entity.getFormattedHit`, `Entity.isIgnoreProtection` | `combat::player_has_protection_prayer`, `npc_has_protection_prayer`, `formatted_hit`, `is_ignore_protection` |
| `Location` packing, `transform`, `withinDistance`, `getDistance` as retribution uses them | `location`, `transform`, `within_distance`, `distance` |

State lives where the original keeps it: a `PrayerState` is one `Prayer`
object (the active `ArrayList` in order, `prayerActiveTicks`, and the list's
`modCount` that its iterators check); the host keeps one per player and
hands it out through `Host::prayer`. Prayer points, static levels and
lifepoints are the native `player::skills::Skills` the host hands out with
`Host::skills`: `decrementPrayerPoints`, `setPrayerPoints` and `heal` are the
native `Skills::decrement_prayer`, `set_prayer` and `heal` with their own
packet and damage-sponge calls.

## Behaviour that matters for gameplay

- Switching: `Prayer.toggle` refuses (no effect, no call) when prayer points
  are not above zero, then asks `PrayerType.permitted`, which reads the
  static Prayer and Defence levels and sends one dialogue naming what is
  missing. Chivalry (Defence 65) and Piety (Defence 70) also need King's
  Ransom and set varbit 3909 to 8 when it is not 8. This check runs for
  switching off too, so a player below a prayer's level cannot switch it off
  through the tab and a player with no points cannot switch anything.
- Switching on: the varp is set to 1 first, then `flag` switches off every
  conflicting active prayer, then the prayer is added, the head icon set, the
  prayer's sound played and `PrayerActivatedEvent` dispatched. Switching off:
  varp 0, removal, sound 2663, `findNextIcon`, `PrayerDeactivatedEvent`.
- Conflicts (`flag`): the same first category, the same non-null second
  category (Magenta: summoning protection, retribution, redemption, smite),
  the same skill bonus, a melee bonus against a ranged or magic bonus either
  way. Protection prayers, retribution, redemption and smite share Light
  Brown, so they exclude each other. An active prayer is listed once per
  conflicting bonus pair and switched off once per listing: activating
  Sharp Eye over Piety switches Piety off twice (two varp writes, two cancel
  sounds, two icon checks, two events). The list holds `active.size() + 10`
  entries; overflowing it is the original's index failure.
- Head icon: `getIcon` maps redemption 5, retribution 3, smite 4, magic 2,
  melee 0, missiles 1, and 10/8/9 for those three with summoning protection,
  which is 7 alone. `findNextIcon` clears the icon when no active prayer has
  one and re-applies icons when summoning protection goes off.
- Drain, once a tick from `Player.tick`: the active-tick counter counts while
  any prayer is on and drops to 0 when none is. Every second counted tick,
  if points are exactly 0 the drain sound, the "run out" message and `reset`
  come first; otherwise each active prayer adds `0.6 / (drain * (1 + b))`
  with `b = (1/30f) * bonuses[12]` computed in float and widened (the bonus
  array is read once per active prayer). A Divine Favor perk and
  `random(100) <= 10` cancel the drain; the decrement (and its packet)
  happens even when the amount is 0. A prayer bonus of -30 makes the drain
  infinite (points go to 0); below -30 the drain is negative and points rise
  without a cap. Points that reach 0 by draining are reset on the next
  counted even tick.
- `reset`: varp 0 and a deactivation event for each active prayer, the list
  cleared, and a `Pulse(1)` submitted that clears the head icon and syncs the
  appearance when it fires. It does not touch the active-tick counter.
- Redemption: sound 2681, graphic 436, heal `(int)(static prayer * 0.25)`,
  points set to 0, reset. Retribution: eight projectiles 438 around the
  player, sound 159, graphic 437, then `1 + randomize(max)` with
  `max = (int)(static prayer * 0.25) - 1` on the killer when it is not the
  player and stands within Euclidean distance 1 on the same plane, and in a
  multi-way zone on each local NPC (an NPC killer) or player (otherwise)
  other than the player and the killer that is attackable by magic. One
  `RANDOM.nextInt` draw per hit, none for a maximum below 1.
- Combat reads: `getSkillBonus` sums every active prayer's bonuses for the
  skill in list order; the swing formulas use `floor(level + bonus * level)`.
  A player's protection is the style's protection prayer, false for a null
  style; an NPC's is `protectStyle == style`, true for two nulls.
  `Entity.getFormattedHit` reads the state's attacker, victim and style;
  without Verac, a protected hit is multiplied by 0.6 between two players
  and by 0 otherwise, through an int compound assignment that truncates
  toward zero. No class of the pinned server overrides `isIgnoreProtection`.
- Not in the pinned server: quick prayers. Logging out resets nothing; a new
  `Prayer` is made with each `Player`, and prayer varps are written with
  `save = false`.

## What answers what

`combat` maps the combat slices' services to this state:
`combat_swing::Host::prayer_skill_bonus` is `combat::skill_bonus`,
`protects_from_summoning` is `combat::protects_from_summoning`,
`has_protection_prayer` is `player_has_protection_prayer` or
`npc_has_protection_prayer`; `combat_pulse::Host::prayer_active` (smite,
redemption) is `combat::active`, `start_redemption` is
`prayer::start_redemption`, `entity_formatted_hit` is
`combat::formatted_hit`; `prayer_points` and `decrement_prayer_points` are
the native `Skills` field and method. No world binding of those Host traits
exists yet, so these are the answers a binding uses, checked against the
original members in this corpus, not a joined fight.

## Required services

Everything else is a method of `prayer::Host`, called in the original order
with no default: `setVarp`, `getVarbit`/`setVarbit` (3909), `playAudio` with
its defaults, `dispatch` of the two events, the appearance head icon and
`sync`, `sendDialogue`, `hasRequirement(KINGS_RANSOM)`, the pulse runner's
`submit`, `sendMessage`, the bonus array, `SkillcapePerks.isActive`,
`RandomFunction.RANDOM.nextInt`, `graphics`, locations, `Projectile...send`,
`manualHit`, `isMultiZone`, `getLocalNPCs`/`getLocalPlayers`, `isAttackable`,
`isPlaying`, the superclass `commenceDeath`/`fullRestore` and the two
`Settings` energy setters. A binding answers `set_varp`/`set_varbit` with
`player::variables`, `manual_hit` with `combat_pulse::impact::manual_hit`,
`is_attackable` with `npc_behaviour::combat::is_attackable`, the local
queries with `entity_regions::local`, and runs `head_icon_pulse` from
`server::pulses` a tick after `submit_head_icon_pulse`.

## Reference tests

```powershell
cargo test -p rs09-server --test prayer
cargo xtask prayer-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

`cargo xtask prayer-oracle` compiles `Prayer.java`, `PrayerType.java`,
`PrayerCategory.java`, `SkillBonus.java`, `Audio.kt` and
`PrayerTabInterface.kt` unchanged with Kotlin 1.8.20 on JDK 11, with the
published ConstLib 1.4.28 pinned in `oracle/bank/constants.lock.json` for
`Sounds` and `Components`: the bundled `ConstLib-1.4.jar` has none of the
prayer sound names. The generator checks that every `Sounds` constant the
two classes name has its numeric suffix as its value. The members listed in
the fixture's `scope` (the `Skills` prayer-point, static-level and healing
members, the three `Player` members, `NPC.hasProtectionPrayer`, the two
`Entity` members, `hasLevelStat`, `setVarp`, `formatDisplayName`,
`random`/`randomize`, the `Location` members, `HitsplatType`, the
`CombatStyle` protection prayers, the `ArmourSet` names and the two event
declarations) are original text inserted into the adapters of
`oracle/prayer/adapters.py`. Everything else is a recording adapter.

`fixtures/prayer/` holds **3,145 original JVM cases with 48,429 actions and
59,759 recorded calls**, 1,127 of them single injected failures (every
recorded call of a marked operation fails once in turn). Groups: the whole
constant table and lookups; every prayer below, at and above its level, the
defence requirement and King's Ransom/varbit paths, the points gate
(0, -0, the least positive double, NaN); every ordered pair of the 27
prayers with the resulting state, triples and the widest conflict
activation; 240 seeded random switching sequences through toggles, tab
buttons and direct `PrayerType.toggle`; head icons for every ordered
combination of up to three overhead prayers and `findNextIcon` from each
starting icon; drain for ten active sets at fifteen prayer bonuses
(-31, -30, -29, -15, -1, 0, 1, 2, 15, 29, 30, 31, 60, `Integer.MAX_VALUE`,
`MIN_VALUE`), runs until points run out (one of 700 ticks and one of 2,000),
points at exactly 0, null and short bonus arrays, Divine Favor with twelve
seeds; reset, the head-icon pulse, `fullRestore` and `commenceDeath`;
redemption at prayer and lifepoint boundaries with and without the damage
sponge; retribution by distance, plane, killer kind, null killer and
locations, multi-way zones with listed and duplicate neighbours; every tab
button; protection for each style on players and NPCs, 11,664
`getFormattedHit` calls over attacker, victim, style, armour and hit
boundaries, and the boosted level `floor(level + bonus * level)` for 21
bonus sets, five skills and 45 levels. Each row records the result or
error, the ordered calls and draws, and every entity's prayer state,
points, lifepoints, head icon and varps afterwards. The native test replays
every row on native `Skills` and the native `variables::set_varp`, compares
it whole, and checks the fixture hash and the source pins.

What the stand-ins model is an oracle input, not a port: the head icon as a
field, varbit 3909 as a stored value, the queued pulse fired when the case
asks, answers for perks, King's Ransom, multi-way, local entities,
attackability and `isPlaying`.

## Differences from the original

- An entity handle is the host's type; `killer instanceof NPC` is
  `Host::is_npc`. Null killers and locations fail with `Error::Null` at the
  original's point.
- A dispatch hook that changes the active list during `reset`, `tick` or
  `findNextIcon` fails with `Binding("ConcurrentModificationException")`,
  as the list's iterator would; no corpus case drives a reentrant hook.
- Locations are only driven inside the fifteen-bit packing range.

## Not ported

Prayer training (burying bones, altars, the ectofuntus), `SkillRestore`'s
prayer event hooks (rapid heal and rapid restore change restore timers
there), `DeathTask.getContainers`'s protect-item count,
`Player.getDragonfireProtection`, `Player.finalizeDeath` (which calls
`reset`), prayer content in minigames, bosses and special attacks, and the
binding of `prayer::Host` and the combat Host traits to canonical players, a
session or the tick driver. All source inventory rows remain `not_replaced`.
