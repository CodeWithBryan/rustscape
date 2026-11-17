# Equipment registers, chinchompas, salamanders and the global creatures

`world::equipment_registers` ports, from the pinned server's
content/global/handlers/item/equipment:

- `BarrowsEquipment.kt` (`barrows`): the 24 pieces with brother, slot type,
  name, base repair cost and six degradation states, the lookups,
  `getAllEquipmentSets`, `getAllRepairableBarrowsIds`, `getRepairCost` and
  `graveDeathDurabilityReduction` (the state table is the one
  `world::player_death::barrows` already uses for a death);
- `EquipmentDegrader.kt` with `BarrowsEquipmentRegister.kt`,
  `PVPEquipmentRegister.kt` and `CrystalEquipmentRegister.kt` (`degrader`);
- `HastaEquipListener.kt` (`hasta`);
- special/`ChinchompaSwingHandler.java` (`chinchompa`) and
  special/`SalamanderSwingHandler.kt` (`salamander`).

`world::creatures` ports, from content/global/handlers/npc,
`SheepBehavior.kt` (`sheep`), `ChinchompaDeathListener.kt`
(`chinchompa_death`), `ShooAwayStrayDogListener.kt` (`stray_dog`),
`CowMilkingPlugin.java` with its `BucketHandler` (`cow_milking`) and
`DecantListener.kt` with its `DecantingDialogue` (`decant`), and the world
enum of AbstractNPC classes (`classes::WorldClass`: `npc_classes::Class`
and `dragonfire::dragons::DragonClass`) that the start-up registers.

The ArmourSet set effects (Ahrim, Dharok, Guthan, Karil, Torag, Verac) are
`combat_swing`/`combat_pulse` (`ArmourSet`, `handler::armour_set_effect`);
their lists already hold every degraded state but the broken one, which is
what the registers degrade through.

They are libraries: a world installs them (see Joins); `--world` does not
yet.

## How the classes reach the native swing

`ChinchompaSwingHandler` extends `RangeSwingHandler`; `SalamanderSwingHandler`
extends the abstract `CombatSwingHandler` with type RANGE and keeps a
private, mutable `style`. A host keeps a `combat_swing::Handler` for each
singleton (`Subclass::instance`) and answers the subclass
(`Host::equipment_subclass`, the salamander's current `style` with it);
`equipment_registers::overrides` answers `combat_pulse::Host::overrides`
with the members the class overrides, so `CombatPulse`, the impact pulse and
the base bodies reach them where the original's virtual calls do. The
chinchompa's `super` calls are the range class's own bodies
(`handler::adjust_battle_state_super`, `impact_super`, the cs-level range
`swing`); the salamander's inherited members are `CombatSwingHandler`'s
(`base_adjust`, an empty `postSwing`, a null `getArmourSet`), and its
delegations go to the current style's singleton through the override table
(`style.swingHandler.impact(...)` is that singleton's virtual `impact`).
`combat_pulse::Host::is_salamander` answers true for the salamander
singleton; `special_attacks::player_swing_handler` already answers
`Chosen::Chinchompa` / `Chosen::Salamander`, which a world maps to its two
singletons.

## Behaviour that is easy to get wrong

- `EquipmentDegrader.degrade` reads the item's packed `idHash` after every
  write: a charge above the registered charges is first set to them, the
  decrement of 0 writes -1 (id and charge both become 0xFFFF, so nothing
  degrades), and the first state of a set loses its whole charge unless its
  name contains "100". A set longer than two removes the item, says it
  degraded, then either adds the next state to the inventory or the ground
  (`addItemOrDrop`, "has broken") when that next state is the last one, or
  adds it with full charges through `EquipmentContainer.add(item, slot, false,
  false)` (the equipment slot passed as the inventory slot) and refreshes; a
  two-item set turns into dust from its last state. A broken Barrows piece
  is in the lists too: after 1000 checks its own id goes to the inventory.
  `getNext` of the last element is the element itself.
- The registers: Barrows (22,500 charges per tier), then PvP (6,000, two-item
  sets), then crystal (250, both sets ending in the crystal seed, so the seed
  is in the first set found). `checkArmourDegrades` checks slots 0 to 12 but
  3; `checkWeaponDegrades` slot 3; both before anything else read the
  player.
- `getRepairCost` is the lost share of the base cost, truncated; the grave's
  reduction takes a fifth of the remaining durability, truncated, and makes
  the piece of that durability (broken, with the default charge, at 0).
- `HastaEquipListener` collects every definition whose lower-cased (ROOT)
  name contains "hasta" in the definition map's iteration order (a world
  input); a null name fails the scan. Below stage 2 the equip is refused
  with the dialogue.
- The chinchompa outside multi combat is the range swing with the state as
  its only target (set even when the swing answers -1). Inside multi combat
  (both entities' properties) a non-player thrower is a cast failure; no
  range weapon answers -1 without stopping; no ammunition stops the pulse.
  One accuracy roll for all; each target answered by
  `findMultihitTargetsForChinchompa` gets its own state with
  `random(calculateHit + 1)` when accurate; the ammunition is used once (no
  drop location); the delay is `2 + ceil(distance * 0.3)`. Experience sums
  the targets' positive hits: 1.33 hitpoints each, range and defence 2
  (long range) or range 4. The impact graphic is (157, 96) on the victim,
  then every target's victim plays its defence animation.
- The salamander's `checkStyle` (defensive cast: magic, range accurate:
  range, otherwise melee) runs in `canSwing`, `calculateAccuracy` and
  `calculateHit` (after the null attacker's 0), never in `calculateDefence`
  or `getSetMultiplier`; `swing` uses the field as it is. Its reach is one
  tile (two at a moving victim, a moving interaction). Magic damage is
  `floor((0.5 + ceil(level * setMultiplier * (bonus[13] + salamander bonus +
  64)) / 640) * modifier)`, the multiplier through the override; its tar is
  removed from the equipment with `new Item(tar, 1)`; the hit is capped to
  the victim's lifepoints after the location check. `CombatPulse` attacks a
  tick sooner with it in range accurate style.
- `SheepBehavior.tick` draws `random(35)` and bleats on 5 unless attacking or
  dead; always true. Shearing faces the sheep first; the penguin sheep (3579)
  always escapes (marking the quest attribute only while Sheep Shearer is
  started); then shears, no "attack" option and a free slot; `random(1, 5)`
  other than 4 shears (locked 3, movement 2, transformed to 5153, wool added
  through ContentAPI or false, re-transformed by an 80-tick pulse), 4 backs
  away. Backing away (3 steps) and the stray dog's run (12 steps) go the
  "opposite" way of `Direction.getDirection(self, player)`: `getOpposite` is
  `get(value + 2 & 3)`, so a diagonal turns cardinal, and no difference is
  north.
- `ChinchompaDeathListener` explodes for a player killer unless "hunter" is
  set (graphic and sound at the NPC's tile) and hits a killer on one of the
  eight surrounding tiles (never the NPC's own) with `random(0, 3)` through
  ContentAPI `impact` (the killer's own manual hit, then the hurt sound);
  `onRespawn` removes "hunter".
- `CowMilkingPlugin.handle`: "steal-cowbell" in any case (Java
  `equalsIgnoreCase`) opens dialogue 70099; otherwise the node must be a
  scenery (or null) and milking needs a bucket or a pail (dialogue 3807
  otherwise); the 8-tick pulse turns a bucket (else a pail) into a bucket of
  milk and goes on while one is left; its `stop` resets the animation, also
  when the player goes inactive. `ClassScanner.definePlugin` prints and
  swallows a failure of the bucket handler's registration.
- `DecantListener`: the NPC option removes and adds the decanted potions,
  then kegs (a refused removal answers false), then opens the dialogue
  (`when (stage++)`: "There you go!", then "Thanks!" and `END_DIALOGUE`). By
  hand: kegs go to `CalquatDecant.combineKegs`; both items must be the same
  potion object (the table row), neither a four-dose; a full potion goes to
  the target, the leftover to the target (no full) or the used slot, and the
  used slot becomes a vial unless both happen. Barbarian decanting needs the
  herblore attribute, a four-dose and answers `ids[2]` in both slots.

## Required services

Everything outside the listed classes is a method of
`equipment_registers::Host` (which extends `dragonfire::Host`) or
`creatures::Host` (which extends `equipment_registers::Host`), called in the
original order with no default: `EquipmentContainer.remove/add/refresh`,
`addItemOrDrop`, `getAttribute` of an Int, `sendDialogue`, `onEquip`, the
ContentAPI chat, animation, lock, unlock, force-walk, add/remove item,
graphics, free slots and option calls, the quest repository, the inventory's
contains/remove/add, `NPC.transform/reTransform`, the two pulse submissions,
the dialogue interpreter and `DialogueFile` lines, `decantContainer`,
`CalquatDecant`, the listener and plugin registrations. Plain accessors: the
subclass and the salamander's field, the potion rows
(`Consumables.getConsumableById(id).consumable as? Potion`, the identity of
the row and its ids), `Consumables.potions` and `CalquatDecant.allKegIds`.

## Joins

- **Swing and pulse.** A world answers `combat_pulse::Host::overrides` with
  `equipment_registers::overrides(subclass)` for its chinchompa and
  salamander singletons, `is_salamander` for the salamander, and maps
  `special_attacks::Chosen::Chinchompa`/`Salamander` from
  `player_swing_handler` to them. The test host does all three.
- **Degrading.** A world keeps one `degrader::Registry::startup()` and
  answers `combat_pulse::Host::check_weapon_degrades` with
  `degrader::check_weapon_degrades` and `npc_behaviour::Host`'s
  `check_armour_degrades` with `degrader::check_armour_degrades`; the
  container calls are the native `inventory::equipment` container
  (`remove`, `add_at` with the slot as its inventory slot, `refresh`). The
  test host answers the weapon check through the port, so
  `CombatSwingHandler.adjustBattleState` reaches it.
- **Listeners and behaviours.** The class scan registers
  `HastaEquipListener`, `SheepBehavior`, `ShooAwayStrayDogListener`,
  `DecantListener` (`define_listeners`) and `CowMilkingPlugin`
  (`cow_milking::new_instance`); a world's interaction handlers call
  `hasta::on_equip`, `sheep::shear`, `stray_dog::shoo_away`,
  `decant::{decant, combine, pour, barbarian}`, `cow_milking::{handle,
  bucket_handle}`; its `npc_behaviour::Behaviour` for `sheep::IDS` calls
  `sheep::tick`, for `chinchompa_death::IDS` `on_death_finished` and
  `on_respawn`, and the stray dogs' (`stray_dog::IDS`) is an empty behaviour;
  `CreaturePulse`s run on its pulse runner (`update` once a tick).
- **Start-up.** `startup::npcs::STARTUP_NPC_CLASSES` is now a list of
  `WorldClass` with the two dragon classes; the scan registers them with
  `npc_classes::registry::define` and the spawner constructs their NPCs. The
  start-up test's `npcs.subclasses` check confirms the original's NPCs per
  class (46 chromatic, 23 metal dragons). A combat world still makes each
  class's dragonfire and each dragon's combat action (docs/dragonfire.md).

## Reference tests

```powershell
cargo test -p rs09-server --test equipment_registers
cargo xtask equipment-registers-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK_11
```

`fixtures/equipment-registers/` holds **687 original JVM cases (310 designed,
377 single injected failures) with 2,779 operations and 22,201 recorded
calls** and the hashes of every source. The oracle compiles the thirteen
classes unmodified with ApiExtensions.kt, MultihitTargets.kt,
ContentInterface.kt, StartupListener.kt, DialogueConst.kt and the dragonfire
oracle's unmodified slice, on Kotlin 1.8.20 and JDK 11 with the published
ConstLib 1.4.28. Original text inserted at generation time: the dragonfire
oracle's, ContentAPI `impact`, `setAttribute`, `inInventory`, `freeSlots`,
`lock`, `unlock` and `stopWalk`, `Location.getSurroundingTiles`, the
`Quests` constant, `SheepShearer`'s and `BarbarianTraining`'s attributes,
`Player`'s degrader field, `Potion.getDose`, `UseWithHandler`'s types and
every Potion/BarbarianMix row's ids of `Consumables` (the native test answers
those rows from the native consumables table, and checks the two agree). The
stand-ins are the four earlier layers' adapters.txt and
oracle/equipment-registers/adapters.txt.

The corpus covers every Barrows state at nine charges, the lists, the
registries in other orders and repeated, every weapon state at seven charges
twice, the "100" names, unregistered items, null players, a whole armour set
over three rounds, the PvP and crystal sets to dust and seed, refused adds,
the weapon check reached through `adjustBattleState`, the hasta scan with
odd names and a null one at six training stages, the swing handler lookup by
weapon and style, the chinchompa's swing, whole `CombatPulse.swing` and fights
in and out of multi combat at three distances with every target kind, more
than ten candidates, no range weapon or ammunition, and every member called
directly; the salamander's members in the four style groups with the four
salamanders, a wrong tar and none, its magic maximum at six levels and four
modifiers, reach at four distances still and moving, experience per style,
null attackers and states, a non-player, the lifepoints cap at and around
the hit and whole fights in three styles; the sheep's bleat, every
shearing outcome and the backing away from nine sides; the explosion around
fifteen killer positions, non-player and other-plane killers, the hunter
attribute; the stray dog from nine sides; the cow plugin's registration and
every milking path; the decant option, fourteen pairs by hand, the keg pour
both ways and barbarian decanting. In marked operations every recorded call
and draw fails once in turn. Expected rows hold the result, the error class,
the ordered calls and draws, the battle state and every pulse, impact
handler, queued pulse, equipment and inventory item (packed id and charge),
handler type, the salamander's style and the stored attributes whenever an
operation changed them. The native test replays every row and compares it
whole; there is no pinned gap. 30 of 30 deliberate breaks of the port fail
the replay: the repair cost share, the grave's fifth, the "100" name, the
next-last test, the PvP charges, the armour slot range, the chinchompa's
delay factor, its target hit bound, its long range experience, its single
target list and its range weapon animation, the salamander's style mapping,
its magic damage bonus (first missed: no level crossed a boundary, now six
levels), a style check in its defence, its armour set override and its
lifepoints cap (first missed: no hit just above the lifepoints, now added),
the shear chance, the bleat draw, the diagonal opposite, the explosion
tiles, the hunter attribute, the dog's steps, the milking delay,
definePlugin's catch, the leftover dose, the same-potion test, the glass
order, the barbarian two-dose id and the hasta stage and lower case. A
charge cap of `>=` instead of `>` is equivalent (it writes the same charge)
and is not counted.

## Differences from the original

- The casts `(Player) entity` of the chinchompa in multi combat, `(Scenery)
  node` of the cow plugin and `node.asNpc()` of the decant option are
  binding failures for the wrong kind (the corpus never casts wrongly).
- `CalquatDecant`, `decantContainer` and the dialogue interpreter are
  services; their own content is another slice.

## Not ported

`BrawlingGloves`, the fist of Guthix gloves and every other
content/global/handlers/item/equipment class not listed above; repairing
(`BobRepairItem`, `ArmourStand`, `BobDialogue`) beyond the repair cost;
`CalquatDecant` and the consumables effects; the dragon classes elsewhere
(KBD, Elvarg, wyverns, brutal and mithril dragons); every binding to a live
session (`world/live` installs nothing of this yet).
