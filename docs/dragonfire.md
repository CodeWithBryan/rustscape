# Dragonfire and switching NPC attacks

`world::dragonfire` ports, from the pinned server:

- content/global/handlers/item/equipment/special/DragonfireSwingHandler.java
  (`handler`);
- core/game/node/entity/combat/equipment/FireType.java (`fire_type`);
- core/game/node/entity/combat/equipment/SwitchAttack.java (`SwitchAttack`);
- core/game/node/entity/combat/MultiSwingHandler.kt and
  MixedSwingHandlers.kt (`multi`);
- the dragonfire protection checks: ContentAPI `calculateDragonfireMaxHit`
  and `hasDragonfireShieldProtection`, and `Player.getDragonfireProtection`
  (`api`);
- content/global/handlers/item/DragonfireShieldListener.kt with its anonymous
  `DragonfireSwingHandler` and `DFSItemPlugin` (`shield`);
- content/global/handlers/npc/ChromaticDragonNPC.java, MetalDragonNPC.java and
  ChromaticDragonBehavior.kt (`dragons`).

It is a library: nothing in a session or `--world` installs it yet (see
Joins). The other global creature classes and the equipment registers are
not ported (see Not ported).

## How the classes reach the native swing

`DragonfireSwingHandler` and `MultiSwingHandler` extend the abstract
`CombatSwingHandler` directly. A host keeps a `combat_swing::Handler` for each
instance (the mutable `type` and flags: MAGIC for dragonfire, RANGE for a
switching handler until its first swing) and the subclass's own fields in
`Subclass` (`Host::subclass`). `dragonfire::overrides` answers
`combat_pulse::Host::overrides` with every member the class overrides, and the
override table now also carries `canSwing`, `addExperience`, `postSwing`,
`getArmourSet`, `calculateAccuracy`, `calculateHit`, `calculateDefence` and
`getSetMultiplier`, so `CombatPulse`, the impact pulse and the base bodies
reach these classes exactly where the original's virtual calls do
(`combat_pulse::handler::{can_swing, add_experience, post_swing, armour_set,
calculate_accuracy, calculate_hit, calculate_defence, set_multiplier,
is_accurate_impact}`). The members the classes inherit from
`CombatSwingHandler` itself are `handler::base_adjust`,
`handler::base_add_experience`, `formatted_hit_super`, an empty `postSwing`
and a null `getArmourSet`; every kind-dependent base member is overridden,
so the `HandlerKind` a host gives these instances is never read.

The MixedSwingHandlers objects override `calculateDefence` only, which the
base `swing` reaches through `isAccurateImpact`. They are instances of the
melee or range class made with `combat_swing::Handler::mixed`, whose
`defence_of` field answers `calculateDefence` with the named singleton's
(`multi::Mixed`).

`MultiSwingHandler` keeps `current` and `next` as indices into its attacks:
the original only ever assigns elements of that array to them. Its
`isAccurateImpact` is the base method with the virtual `calculateAccuracy`
and `calculateDefence`, which answer the current attack's handler's.

## Behaviour that is easy to get wrong

- `calculateDragonfireMaxHit` asks a player's shield (elemental and mind
  shields only against wyvern fire, the anti-dragon shield only against
  dragonfire, the dragonfire shield against both), the antifire potion (never
  against wyvern fire) and the magic prayer, sends its message (the potion
  message always says fiery), and takes a non-player's 0x2/0x4/0x8 tokens
  from `getDragonfireProtection(!wyvern)`. The prayer's 60% applies only
  without shield and potion; shield 90% and potion 10% add up; the result is
  `max(unprotectable, (int) effective)`.
- `Player.getDragonfireProtection` stores its answer as `fire_resistance`;
  fire resistance only counts for dragonfire.
- `swing` draws `random(max + 1)`, sets the state's style to MAGIC and
  answers 1 in melee range, otherwise `2 + floor(distance * 0.5)` with
  `fireBreath` set to the impact tick plus 2. `adjustBattleState` puts the
  state's style back after the base adjustment; icy breath above 10 against an
  elemental or mind shield is redrawn as `random(10)` first.
- `visualizeImpact` of an NPC's breath at a player wearing a dragonfire shield
  only charges it (an uncharged shield is replaced by a charged one set to 0
  charges) by 20 while below 1000 and returns: no freeze, no defence
  animation. Item charges are the original packed `idHash`, so a negative or
  oversized charge changes the id (`Host::set_item_charge`).
- `MultiSwingHandler.swing` makes `next` current (in melee-distance mode the
  first non-melee attack when the melee singleton cannot swing), sets its own
  `type`, draws the next attack with `randomize(attacks.size)` before
  swinging, and answers the current attack's handler's swing when the attack
  uses its handler; otherwise an accuracy roll, `random(max)` (not
  `max + 1`) and `1 + ceil(distance * (0.5 magic, 0.3 otherwise))` ticks for a
  non-melee style. `canSwing` asks the range singleton in melee-distance
  mode, otherwise the next attack's handler. A single melee attack turns
  melee distance off.
- FireType's breath effects: shocking lowers `random(3)` by 5 when
  `random(10) < 3`; toxic poisons at 40 with `n[0]` as source (an empty
  array is an index failure, a null source a null failure); icy registers a
  seven-tick `frozen` timer. Every `exec` answers true.
- The dragon classes: one static dragonfire attack per class, made the first
  time the class is used and shared by every instance; each instance's own
  switching handler over two melee attacks (animations 80 and 91) and that
  attack, without melee distance for chromatic dragons (their breath is in
  melee range) and with it for metal dragons (projectile 54 from afar);
  `getSwingHandler` answers it without asking anything, and
  `getDragonfireProtection` answers every token (0x2 | 0x4 | 0x8), so a dragon
  hit by dragonfire takes `max(unprotectable, 0)`.
- `ChromaticDragonBehavior.onDropTableRolled` removes every dragon egg, then a
  killer with static summoning of at least 99 who rolls `EGG_RATE` (a mutable
  static, 1000; `roll` is true for a rate of 1 or less) gets the egg of the
  dragon's colour, dragon bones for the lava dragon.
- The dragonfire shield's "operate" cancels a prepared attack (the bar's varp
  reset only when the bar is off), refuses an uncharged shield or one below 20
  charges and a recharging one, then prepares a new anonymous handler that
  swings at once while attacking within reach or replaces the next swing. Its
  `swing` (only for a player attacker) needs the charged shield worn (-1
  otherwise), spends 20 charges, replaces the shield in the shield slot with
  an uncharged one below 20, delays the player 3 ticks and locks
  `dfs_recharge` for 50 ticks with `BETTER_DFS`, 200 otherwise (read when the
  listener is made).

## Required services

Everything outside the listed classes is a method of `dragonfire::Host`
(which extends `special_attacks::Host`) or of the combat slices' hosts, called
in the original order, with no default: `hasTimerActive` by identifier and
for `DragonFireImmunity`, a non-player's `getDragonfireProtection`,
`hasFireResistance`, attributes, `EquipmentContainer.updateBonuses`,
`faceLocation`, projectile templates' `copy`/`transform` with `send`,
ContentAPI `setVarp`, `delayEntity`, `replaceSlot`, the `locks` key
members, `inventory.get`, `DropListener.drop`, `ItemPlugin.register` and the
listener registration. Plain accessors: the subclass fields, the
`CombatStyle` singletons, a new handler instance, `Item.setCharge`/
`getSlot` and `ServerConstants.BETTER_DFS`.

## Joins

- **Swing and pulse.** A world answers `combat_pulse::Host::overrides` with
  `dragonfire::overrides(subclass)` for an instance of these classes, and
  its NPCs' `getSwingHandler` with the instance (the dragon classes' own,
  `dragons::combat_action`). The test host does both.
- **NPC classes.** `dragons::DragonClass` implements `npc_classes::Abstract`:
  a world registers it with `npc_classes::registry::define` beside
  `npc_classes::Class` (a world enum over both), makes the class's
  `dragons::dragonfire` once and each constructed dragon's
  `dragons::combat_action`, answers the dragon's `getSwingHandler` and
  `getDragonfireProtection` from them, and registers
  `ChromaticDragonBehavior` for `dragons::behaviour::ids()` with its drop
  hook (`dragons::on_drop_table_rolled`). `npc_classes::Class` itself does not
  list the dragons and the start-up world does not register them.
- **Items.** A world registers the shield's four options on items 11283 and
  11284 (`shield::Listener::define_listeners`, `Listener::handle`) and the
  death item of 11283 (`shield::death_item`).
- **Timers.** `spawn_timer`/`register_timer` receive the frozen timers;
  `timers::content` owns `DragonFireImmunity` and `Frozen`.

## Reference tests

```powershell
cargo test -p rs09-server --test dragonfire
cargo xtask dragonfire-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK_11
```

`fixtures/dragonfire/` holds **1,264 original JVM cases (416 designed, 848
single injected failures) with 6,330 operations and 244,781 recorded calls**
and the hashes of every source. The oracle compiles DragonfireSwingHandler,
FireType, SwitchAttack, MultiSwingHandler.kt, MixedSwingHandlers.kt,
DragonfireShieldListener.kt, ChromaticDragonNPC, MetalDragonNPC and
ChromaticDragonBehavior.kt unmodified, with NodeTask, TickUtils.kt,
IntType.kt, Container.kt, Plugin, Initializable, TimerFlag and the combat
slice the combat-pulse oracle compiles (CombatPulse, ImpactHandler, Pulse, the
four swing handlers, BattleState, CombatStyle, InteractionType, Weapon,
RangeWeapon, Ammunition, ArmourSet, BoltEffect, EquipmentSlot, Direction,
Point, Path, ProjectilePathfinder, ClipMaskSupplier, ZoneType), on Kotlin
1.8.20 and JDK 11 with the published ConstLib 1.4.28. Original text inserted
at generation time: ContentAPI `calculateDragonfireMaxHit`,
`hasDragonfireShieldProtection`, `registerTimer`, `spawnTimer`,
`removeAttribute`, `visualize` and `getAttribute`,
`Player.getDragonfireProtection` and `getSwingHandler`, `Entity.isPlayer`,
`Node.asItem`, and the selections the special-attacks oracle inserts. The
stand-ins are oracle/combat-swing/adapters.txt,
oracle/combat-pulse/adapters.txt, oracle/special-attacks/adapters.txt and
oracle/dragonfire/adapters.txt in that order.

The corpus covers every protection combination (seven shields, potion,
prayer, fire and wyvern, message, unprotectable minimum, the sixteen
non-player tokens, extreme maximums, `getDragonfireProtection` with fire
resistance), every dragonfire handler member called directly (the six- and
seven-argument `get`, the handler-less constructor, melee range, icy breath)
at players and NPCs at four distances with four shields, shield charging from
uncharged, empty, nearly full, full and beyond the packed range, the freeze
draw with and without immunity or a timer, the icy redraw bounds, the
chromatic and metal dragons' switching tables at four distances with scripted
next-attack draws, a table with projectiles, a fixed maximum hit, the style
constructor and a mixed handler, single-attack and null-handler tables, the
three mixed handlers through `CombatPulse.swing`, the four breaths' effects,
the shield's four options at five charges, operate's refusals and cancels,
the prepared attack in whole fights at three distances with and without
`BETTER_DFS`, the shield taken off or emptied in between, 28 whole fights
where `CombatPulse` reaches switching and dragonfire handlers, and the dragon
classes (five chromatic and metal ids, their ids, tokens and handlers, 60
whole fights against three shields with and without antifire, two dragons of
a class sharing its breath) and the egg drops (nine dragons, summoning 98/99,
three draws, every egg removed, rates 1, 2, 0 and -3 kept between calls). In
marked operations every recorded call and draw fails once in turn. Expected rows hold the result, the
error class, the ordered calls and draws, the battle state and every pulse,
impact handler, queued pulse, settings field, equipment and inventory item
(packed id and charge), handler type, switching index and stored attribute
whenever an operation changed them. The native test replays every row and
compares it whole; there is no pinned gap. 25 of 25 deliberate breaks of the
port fail the replay: the prayer share, the anti-dragon shield and the potion
against wyvern fire, the fire resistance token, the breath delay, the shield
charge cap, the freeze draw, the icy redraw bound, dragonfire's and the
switch's hit bounds, the next attack kept, the melee-distance fallback, the
switch's 0.3 delay factor (first missed: the corpus then had no gap of 3 or 8
tiles, now added), the switching handler's postSwing dispatch, the shock
chance (first missed: no first draw of 3, now added), the recharge lock, the
uncharge threshold, the mixed handlers' defence, the canSwing override
dispatch, chromatic melee distance, the egg colour, the
summoning level, the dragons' tokens, the chromatic breath graphic (first
missed: no chromatic dragon fought within melee range, now added) and the egg
removal.

## Differences from the original

- Projectiles are recorded calls: `copy` and `transform` with `send` are one
  service each (`DragonProjectile`); the original's in-place change of a
  shared template by `transform` is not modelled.
- `SwitchAttack.canSelect` is the base class's `true`; the content
  subclasses that override it are not ported.
- A `MultiSwingHandler` swing with a null state is not supported (the
  original's `state!!` fails only on the non-handler path); the corpus only
  swings a dragonfire handler with a null state.

## Not ported

The dragon classes in `npc_classes::Class` and the start-up registration;
`SheepBehavior`, `ChinchompaDeathListener`, `ShooAwayStrayDogListener`, `CowMilkingPlugin`,
`DecantListener`; the equipment registers (`BarrowsEquipment`,
`BarrowsEquipmentRegister`, `CrystalEquipmentRegister`, `EquipmentDegrader`,
`HastaEquipListener`, `PVPEquipmentRegister`); the dragon classes elsewhere
(KBD, Elvarg, wyverns, brutal and mithril dragons) that use these handlers;
every binding to a live session.
