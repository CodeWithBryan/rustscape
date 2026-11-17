# Weapon special attacks

`world::special_attacks` ports the weapon special attack handlers of the
pinned server: the 34 `@Initializable` classes of
content/global/handlers/item/equipment/special (`AncientMace`, `Backstab`,
`Chainhit`, `Cleave`, `Clobber`, `DescentOfDarkness`, `EnergyDrain`,
`Excalibur`, `Feint`, `Hamstring`, `HealingBlade`, `IceCleave`, `Impale`,
`Judgement`, `Liquefy`, `PhantomStrike`, `Powershot`, `Powerstab`,
`Puncture`, `QuickSmash`, `Rampage`, `SaradominsLightning`, `Seercull`,
`Sever`, `Shatter`, `Shove`, `SliceAndDice`, `Smash`, `Snapshot`, `Snipe`,
`SpearWall`, `Sweep`, `Warstrike`, `Weaken`), their registration, the lookup
`Player.getSwingHandler` makes, their `fireEvent` answers with the combat
tab's instant special, the anonymous timers and pulses they create, and
core/game/node/entity/combat/MultihitTargets.kt. It is a library: nothing in
a session or `--world` installs it yet (see Joins).

`ChinchompaSwingHandler`, `DragonfireSwingHandler` and
`SalamanderSwingHandler` live in the same folder but are not special attacks;
the dragonfire handler is `world::dragonfire` (docs/dragonfire.md), the other
two are not ported.

## What is ported

| Original | Native |
| --- | --- |
| Each class: constructor flags, `SPECIAL_ENERGY`, `newInstance` (every `register` call, the `&&` chains that stop at the first refusal) | `Special::instance`, `energy`, `registration`, `new_instance`, `register_all` |
| `swing` of all 34 classes (Clobber's authentic and OSRS bodies, Chainhit's door supports, QuickSmash's own impact and experience) | `swing`, `melee::swing`, `range::swing` |
| The overridden `visualize`, `visualizeImpact`, `impact`, `adjustBattleState` (IceCleave, Sweep) and `getFormattedHit` (AncientMace) | `melee::*`, `range::*`, joined through `combat_pulse::Overrides` |
| `fireEvent(identifier)` (`instant_spec`, `ncspec`, Kotlin `Unit`) and `CombatTabInterface.handleInstantSpec` (`swing(p, victim, null)`) | `Special::fire_event`, `instant_swing` |
| `Player.getSwingHandler(swing)`: spell/autocast, the special registered for the weapon on the pulse style's handler while the bar is on, the "Unhandled special attack" message, chinchompa and salamander instances | `player_swing_handler` |
| Chainhit's `handleHit`, `getVictimsList` and its three anonymous pulses; Snapshot's second-arrow pulse | `range::handle_hit`, `SpecialPulse`, `SpecialTask` |
| Hamstring's and Phantom strike's anonymous `RSTimer`s (`onRegister`, `run`) | `SpecialTimer`, `TimerBody` |
| ContentAPI `stun(entity, ticks)` | `melee::stun` |
| `findMultihitTargets`, `findMultihitTargetsForDragonHalberd`, `findMultihitTargetsForD2h`, `findMultihitTargetsForChinchompa` and the private `isAttackable` | `multihit::*` |

## How the subclasses reach the native swing

A special handler is an instance of the melee or range swing handler class
with its own `type` and flags: the host keeps a `combat_swing::Handler` for it
like for the three `CombatStyle` singletons, and answers which subclass it is
(`Host::special_of`). The members the subclass overrides are the original's
virtual calls. `combat_pulse::Host::overrides` answers, per handler instance,
a table of the overriding bodies ([`overrides`] builds it for a special), and
`pulse::swing`, `handler::visualize`, `visualize_impact`, `impact`,
`adjust_battle_state` and `formatted_hit` run the overriding body when there
is one. The base class bodies a subclass reaches with `super` are
`handler::*_super` and `combat_swing::Handler::swing`. The recursion of the
base `adjustBattleState` into target states is virtual, as in the original.

The lookup is the original's: `CombatSwingHandler.register` fills each base
handler's `specialHandlers` map (`combat_pulse::handler::register`), and
`Player.getSwingHandler` asks `getSpecial(weaponId)` on the handler of the
pulse's style (`player_swing_handler`); `CombatPulse.pulse` reaches it through
`combat_pulse::Host::swing_handler`.

## Behaviour that is easy to get wrong

- `Settings.drainSpecial` switches the bar off (with varp 301) before it
  compares the energy, so a refused special still turns the bar off and sends
  "You do not have enough special attack energy left." Liquefy's
  `isSpecialToggled = false` is `setSpecialToggled(false)`, which flips the
  bar whatever its argument.
- Weaken answers its hit as the swing delay: a miss submits the impact pulse
  with delay -1, which delivers on its first update.
- Excalibur, Rampage and Clobber's OSRS body answer -1 after their effects,
  so the pulse treats them as refused swings. Clobber's Kotlin safe casts
  answer -1 for a non-player, a null victim or a null state without failing.
- QuickSmash checks reach before energy: without a victim it uses the last
  victim within two ticks of `combat-stop`; it delivers its own hit through
  `handleImpact` (0.6 against a protecting player, 0 against an NPC) and
  answers 1, after which `CombatPulse.swing` runs the rest of the swing with an
  empty state.
- Chainhit stores the local NPC or player list in the `chain-hit_v`
  attribute (the attacker and the victim removed from it), and each bounce
  pops the list's head until an attackable multi-zone entity is found; every
  bounce costs 10 energy through `setSpecialEnergy`, and an exhausted list or
  energy below 10 removes the attribute.
- Sweep keeps the state itself as its only target outside a multi zone; its
  large victims get a second roll at 0.75 accuracy. Its `adjustBattleState`
  adjusts each target against that target's own victim.
- Slice and dice leaves the main state's style unset and puts its four hits in
  target states; `random(a, b)` is `min(a, b) + random(|b - a|)`.
- Warstrike's drain walks defence, strength, prayer points
  (`(int) -(points + left)`), attack, magic and range with what each
  `updateLevel` call answers.
- Descent of darkness builds its own weapon: `hasAmmo` may turn the double
  shot into a default shot on the same weapon object, which the second hit
  then reads. The dragon-arrow check reads the ammunition definition's name.
- Shove's direction comes from the four location reads; the same tile gives
  no direction, a stun and no path.
- `getFormattedHit` of AncientMace is `formatHit` alone: no shield,
  staff or familiar adjustments.
- The multi-target finders filter the whole local list before `take`; the
  halberd looks one tile to each side across the attack direction
  (`Direction.getDirection`), the 2h sword keeps only entities within
  distance 1 of the attacker (diagonals are farther).

## Required services

Everything outside the listed classes is a method of `special_attacks::Host`
(which extends `combat_pulse::Host`) or of the combat slices' hosts, called in
the original order, with no default: multi-zone and local entity queries,
`Repository.findNPC`, original ids, prayer points (set, increment), chat,
ContentAPI `playAudio` and `sendMessage`, delayed global audio, timer spawn
and registration, rights, protection prayers and their toggles,
`continueAttack`, the `combat-stop` and `chain-hit_v` attributes, movement
locks, the stun clock, the dumb pathfinder, walking paths, item definition
names, the world runner for the specials' pulses and projectiles.
`Host::settings` hands out the player's native `Settings` with its context.
`ServerConstants.DRAGON_AXE_USE_OSRS_SPEC` and `Animation(id, priority)` are
plain accessors.

## Joins

- **Swing and pulse.** `combat_pulse::Host::overrides` answers
  `special_attacks::overrides(special)` for a special instance;
  `combat_pulse::Host::swing_handler` answers `player_swing_handler` for a
  player. The test host does both.
- **Special energy.** The native `player::settings::Settings`
  (`drain_special`, `toggle_special_bar`, `set_special_toggled`,
  `set_special_energy`), bound through `Host::settings` to the player's
  varps and messages. The weapon tab's special bar
  (`world/player_tabs/weapon.rs`) toggles the same `Settings`.
- **Combat tab.** A world answers `global_handlers`' `special_handler`,
  `special_instant` and `special_swing` services with
  `combat_pulse::handler::special` on the melee singleton,
  `Special::fire_event(.., "instant_spec") == Fired::True` and
  `instant_swing`. No test drives that join yet.
- **Start-up.** A world makes one handler per [`Special::ALL`] entry with
  `Special::instance` and calls `register_all` with the melee and range
  singletons.
- **Timers and pulses.** `register_special_timer` receives the anonymous
  timer; the world's timer manager calls `on_register` and `run`.
  `submit_special` receives a `SpecialPulse` the world runner updates once a
  tick.

## Reference tests

```powershell
cargo test -p rs09-server --test special_attacks
cargo xtask special-attacks-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK_11
```

`fixtures/special-attacks/` holds **3,705 original JVM cases (427 designed,
3,278 single injected failures) with 7,410 operations and 294,775 recorded
calls** and the hashes of every source. The oracle compiles the 34 handler
classes, MultihitTargets.kt, Plugin, Initializable, TimerFlag and Clocks
unmodified with the combat slice the combat-pulse oracle compiles
(CombatPulse, ImpactHandler, Pulse, the four swing handlers, BattleState,
CombatStyle, InteractionType, Weapon, RangeWeapon, Ammunition, ArmourSet,
BoltEffect, EquipmentSlot, Direction, Point, Path, ProjectilePathfinder,
ClipMaskSupplier, ZoneType) on Kotlin 1.8.20 and JDK 11 with the published
ConstLib 1.4.28 (every Sounds and Items name checked against its suffix).
Original text inserted at generation time: `Player.getSwingHandler`, the
special energy fields and members of `Settings`, ContentAPI `stun`,
`registerTimer`, `spawnTimer`, `getWorldTicks` and `getAttribute`, and the
selections the combat-pulse oracle inserts. The stand-ins are
oracle/combat-swing/adapters.txt, oracle/combat-pulse/adapters.txt and
oracle/special-attacks/adapters.txt in that order.

The corpus covers the registry (every registered item on both handlers,
unregistered ids, repeated `newInstance` with its logs and stopped chains),
`fireEvent` of every class for three identifiers with and without the OSRS
dragon axe, `Player.getSwingHandler` over weapons, bar, pulse style, spell
and autocast, every special swung through `CombatPulse.swing` at NPCs and
players with four seeds and delivered by the runner, too little energy and
the bar off, every special's `swing`/`visualize`/`visualizeImpact`/`impact`/
`adjustBattleState`/`getFormattedHit` called directly (null state, null
victim), QuickSmash's instant paths (reach, last victim and its tick window,
death, `continueAttack`, protection prayers, energy), Excalibur's two
weapons with a null timer, Rampage and Clobber's OSRS body, the multi-target
specials with 0, 3 and 16 neighbours and refusals, Sweep's large victims,
the four target finders at every direction (chinchompa with players,
familiars, owners and wilderness refusals), Chainhit's door supports and
bounces through NPCs and players until list or energy runs out, Descent of
darkness with dragon/rune/one/no arrows and no bow, Snapshot at one to seven
tiles, Snipe at a fighting or idle victim, Hamstring's and Phantom strike's
timers registered and run five times at NPCs and players, Sever's prayers, Warstrike's drain chain, every Slice
and dice band, Shove in nine directions, a large victim and a wall, Liquefy,
and eight whole fights where `CombatPulse` reaches the special through
`Player.getSwingHandler`. In marked operations every recorded call and draw
fails once in turn. Expected rows hold the result, the error class, the
ordered calls and draws, the battle state and every pulse, impact handler,
queued pulse, settings field and `chain-hit_v` list whenever an operation
changed them. The native test replays every row and compares it whole;
there is no pinned gap. 20 of 20 deliberate breaks of the port fail the
replay: a dropped registration, a chain registered as separate calls, the
special bar ignored by the lookup, the Cleave modifier, Rampage's boost,
Weaken's delay, a Shove direction, a Slice and dice band, Ice cleave's
freeze on a miss, Hamstring's interval, Seercull's drain test, the dragon
arrow minimum, the Powershot projectile speed, Chainhit's energy cost,
Snapshot's reach, the 2h reach, the Vesta spear count, Phantom strike's
bleed, `drainSpecial`'s order in the native Settings and the impact override
dispatch.

## Differences from the original

- A non-player attacker of a special that casts `(Player) entity` fails with
  `Error::Binding` ("ClassCastException") instead of the JVM exception; the
  corpus does not drive it (Clobber's Kotlin safe cast is driven).
- The anonymous timers and pulses are values handed to the host; the
  original shares the `BattleState` object between the impact pulse and
  Chainhit's pulse, which neither changes afterwards, so a copy is captured.
- `ImpactHandler.handleImpact`'s `secondary` argument (Puncture's second
  hit) is not read by the original, so it is not observable here either.

## Not ported

`ChinchompaSwingHandler`, `SalamanderSwingHandler` (`DragonfireSwingHandler`,
`SwitchAttack`, `FireType`, `MixedSwingHandlers` and the chromatic and metal
dragons are `world::dragonfire`, docs/dragonfire.md), the equipment registers
(`BarrowsEquipment`, `BarrowsEquipmentRegister`, `CrystalEquipmentRegister`,
`EquipmentDegrader`, `HastaEquipListener`, `PVPEquipmentRegister`); the timer manager's scheduling of the anonymous timers;
every binding to a live session.
