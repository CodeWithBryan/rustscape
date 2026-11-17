# The familiar framework

`crates/server/src/world/familiars` ports the summoning familiar framework of
content/global/skill/summoning: what every familiar and pet shares, not the
individual familiars.

| Original | Native |
| --- | --- |
| `familiar/FamiliarManager.java` (summon and renew, pets, morphing, feeding, picking up, login, parse with the v0/v1 migrations, battle-state and boost hooks, details lists, ownership) | `manager::Manager` |
| `FamiliarManager.FAMILIARS` and `Familiar.newInstance` (the registry) | `manager::Registry` |
| `familiar/Familiar.java` (constructor, init/spawn, the tick, attack rules both ways, call/teleport, dismiss, specials, the hit pulse, boosts, varps) | `familiar::Familiar` |
| `familiar/BurdenBeast.java`, `BurdenContainerListener.java`, `Forager.java` | `burden` |
| `familiar/FamiliarSpecial.java` | `special::FamiliarSpecial` |
| `pet/Pet.java` (hunger, growth, warnings, running away, growing up) | `pet` |
| `pet/Pets.java` (the 85 constants, `forId`'s three maps, stages, NPC ids, cats, `hasPet`) | `pets` |
| `pet/PetDetails.java` | `player::persisted::familiars::PetDetails` |

## Model

A familiar is an NPC subclass object. [`Familiar`] keeps the class's own fields
(owner, timer, special points, pouch, special cost, combat handler, boosts,
attack style, the drain per tick and its fraction, `firstCall`) and the
subclass part (`Kind`: a plain familiar, a beast of burden with its container
and a forager's items and passive delay, or a pet with its item id, shared
`PetDetails`, growth rate, `Pets` constant and warning level). Its NPC is a
host node made by `new NPC(id, null)`, the base constructor: familiars are
not `AbstractNPC` subclasses, `NPC.create` never makes one, and they are not
in `npc_classes::Class` or `creatures::classes::WorldClass` (registering them
there would change what `NPC.create` builds for their ids). The live world
answers `Host::new_npc` with `npc_classes::Factory::new_npc(None, id, None,
Some(Direction::South))`. Content classes build their objects with
`Familiar::new(host, class, owner, Spec, Shape)` (`Shape::Plain`,
`Burden { container_size }` or `Forager { items }` is the class it extends)
and pets with `Familiar::new_pet`.

Objects are shared like the original's: a `Familiar` or `Manager` clone is
the same object (`same` is `==`), so a familiar keeps working after it was
dismissed (a pet's tick continues after `growNextStage` replaced it). Every
field access takes the object's lock briefly; no lock is held across a
service, so services may reenter (the corpus dismisses and summons from
inside `specialMove`, `getText`, `handleFamiliarTick` and `construct`).

The pet details are the persisted ones. `player::persisted::familiars::Manager`
now holds a `PetStore` (the original's `HashMap<Integer, ArrayList<PetDetails>>`
of shared `PetDetails` objects, iterated in the JDK's order); a manager made
with `Manager::new(player, persisted.familiars.pets.clone())` works on that
same map, so parse, summoning, growth, running away and `removeDetails` change
exactly what the save writes. The current familiar is read off the manager
when saving (`Manager::saved_active`: a pet's item id, or the original id,
ticks, special points, a non-empty beast of burden's container and the NPC's
lifepoints, as `PlayerSaver.saveFamiliarManager` reads them).

## Services

Everything outside the classes is a required `Host` service with no default:
the NPC and entity members, the player's skills, messages, varps, interfaces,
quests and skull, containers and item definitions, combat pulses and battle
states, the pulser and pulse manager, `RandomFunction`, `GameWorld.getTicks`,
`RegionManager.getSpawnLocation`, the packet repository, and the content:
`SummoningPouch.get`, `SummoningScroll.forPouch`, `SkillcapePerks`, and each
content class's `construct`, `specialMove`, `getIds`, `configureFamiliar`,
`getText`, `handleFamiliarTick` (its own body; a forager's inherited body runs
after it), `adjustPlayerBattle` and a forager's `getRandom`. A pet has no
content class: the host answers `familiar_tick` and `adjust_player_battle`
for it with the base's empty bodies. The getters the original reads as fields
(location, ids, name, visibility, `instanceof`, the inventory, the save
version, `getFamiliarManager`, `FAMILIARS`, the pet class) are the unrecorded
accessors at the top of the trait.

## Behaviour kept

- The constructor: `new_npc`, then `SummoningPouch.get(pouchId)`, then the
  definition name of `getOriginalId() + 1` against the NPC's own name (a
  combat familiar has a Wilderness form with the same name); the drain per
  tick `(level - cost + 1) / ticks` as a double, 0 for pouch id -1, and a
  missing pouch of any other id throws. A beast of burden then makes and
  registers its container; a forager sets its items, then `setRandomPassive`
  (the tick read before the 100..440 draw).
- The tick: the timer drops, the fraction grows and a point drains only while
  `fracDrain > 1.0` and ticks remain; every 50 ticks special points rise by 15
  (capped at 60) and `getText` is read twice when not empty; the timer varbits
  4534/4290; the minute and half-minute warnings; at 0 the vanish or
  dropped-items message (only a non-empty beast of burden) and `dismiss`.
  Assisting the owner reads the owner's victim twice (the second against the
  familiar itself), falls back to `combat-attacker`, and needs every multi
  zone, a combat, non-burden, non-peaceful familiar (a missing pouch throws
  there). Recall when visible and more than 12 tiles away, or hidden and
  `ticks % 25 == 0` (never for a pet's -1); a failed call hides it; otherwise
  follow again when no pulse runs; then `handleFamiliarTick`.
- `call`: no spawn tile answers false; hidden in a follower-free zone unless
  `enable_summoning` is locked; teleport, then (not for pets) the summon sound
  (firstCall is cleared only after it plays) and the large or small graphic by
  size; follow while attacking or face the owner; an unrendered NPC of an
  active owner updates its walking queue and masks.
- `dismiss`: a beast of burden closes interface 671 when open, drops each item
  on its tile for 500 ticks (the location re-read per item) and clears; then
  clear, pulses, tab 7, the manager's familiar, varps 448, 1176, 1175, 1174,
  appearance, tab 3.
- Specials: the special node equal to the familiar answers false; points,
  scroll, scroll in inventory and 15 tiles in that order; a refused content
  `specialMove` still answers true without effects; the delay attribute is
  set before the scroll is removed, sound, visualization, points (unless the
  infinite attribute) and experience. The hit pulse: delay `2 + floor(d/2)`,
  next attack 4, then accuracy, `randomize`, the slayer task read twice
  (neutralize, poison -1, and -1 for a magic state), the impact and graphic.
- Attack rules: `isAttackableBy` before the NPC's own; the player tests in the
  original order with each message's recipient; `continueAttack` checks the
  familiar's own victim first; `canAttack` reads the owner's style before the
  target test, then distance 8, multi for players, interaction lock, the
  target's MAGIC test, the combat flag.
- Beasts of burden: `isAllowed` looks the pouch up again for each pouch test
  (and throws without one) and lowercases with the root locale; `transfer`
  refuses a forager's store first, clamps to the source amount and the
  target's room, moves one unstackable item by slot then the rest by
  remove/add; `withdrawAll` reads the capacity every round; `openInterface`
  refuses an empty forager, throws when `open` answers null, and generates
  both item grids.
- Pets: hunger 0.025 (baby) or 0.018 per tick unless no food, a grown cat or
  pet mastery, with `petrate` read up to twice (0 stops it, 2 multiplies by
  100); the warnings at 75..90 and 90 once each; running away at 100 (only
  with food and a growth rate) dismisses, drops the last details and clears
  varp 1175; growth uses the value read before the update, so the stage
  grows the tick after it reaches 100; `growNextStage` (kittens count
  `cats_raised`) moves the details, then morphs through the manager, whose
  current familiar must be a pet (`morphPet` reads its warning level).
- Summoning: renew only for the same pouch; the zone check before the pet
  branch; levels, points, the registry (`Invalid familiar`), `construct`,
  the spawn tile, the inventory removal, then points, experience, spawn or
  `refreshTimer`, appearance. Pets: ids 8851..8899 and unknown items answer
  false; the level dialogue; a new pet's details start at growth 100 when
  its rate is 0; the familiar is set before the animation and removal, so a
  failed removal leaves it set and unspawned.
- Parse: `currentPet` first; versions below 2 put an empty list under every
  stage of every pet (replacing lists already there), read hashes
  (`>> 16 & 0xFFFF`) or v0 baby ids with stages, and unhash a current pet
  above 65536; version 2 reads the keys in the JSON object's HashMap order.
  A current pet without saved details gets a detached `PetDetails(0)`; a
  familiar is set on the manager before its ticks are parsed; the inventory
  is cast to an array before the familiar is cast to a beast of burden; every
  failure is the original's class (null, cast, number).
- `Registry::new_instance`: `getIds()` once, `configureFamiliar()` after each
  put, and the first id already registered logs and answers null leaving the
  earlier ids registered.
- `Pets.forId`: baby, then grown, then overgrown; grown and overgrown ids only
  when positive, so -1 is never a key. `hasPet` walks the three maps in the
  JDK's iteration order.

## Oracle

`cargo xtask familiars-oracle --check` (oracle/familiars, JDK 11) compiles
FamiliarManager, Familiar, BurdenBeast, BurdenContainerListener, Forager,
FamiliarSpecial, Pet, Pets, PetDetails, Plugin and CloseEvent unchanged with
recording stand-ins named after the `Host` services (WeaponInterface's
defensive style, the Skills ids, the Log constants, `ItemConfigParser.BANKABLE`
and Location.getDistance inserted from the originals) and three content
classes of the corpus extending the unchanged abstract classes. Each case runs
in a class loader of its own. The fixture holds 565 cases (124 designed, 441
single injected failures over 34 swept operations) with 2,186 operations and
24,371 recorded calls; the generator rejects any failure an operation does not
name in `expect`, and any `expect` that does not happen; `cargo test -p rs09-server --test familiars` replays
every event, result, failure class and the snapshot after each operation (the
manager and its pet details with object identity, the registry, every familiar
seen with its NPC's fields, the player's attributes). The `Pets` table is
compared constant by constant with the original enum.

Of 18 deliberate breaks of the port (the drain test, the hunger rate, the call
sound order, the v1 hash, the per-round capacity, the second pouch lookup,
configure per id, a dismiss varp, a new pet's growth, a pet's food, the
captured growth, the second victim read, the special's answer, the second task
read, the familiar set before parse finishes, `hasPet`'s order, the morph
warning level, the second `getText`), all 18 fail the replay; `>` against `>=`
on the drain fraction failed only after `tick-drain-exact` (a fraction of
exactly 1.0) was added. The script is not part of the repository; the breaks
were restored.

## Join points

The other libraries declare the familiar services by name; a live world
answers them from the player's native `Manager`:

| Service | Answer |
| --- | --- |
| `world::login` `familiar_login` (`FamiliarManager.login`) | `manager.login(host)` |
| `player::save` `parse_familiars` / `familiars` | `manager.parse(host, data)` / `Familiars { pets: store.saved(), active: manager.saved_active(host)? }` |
| `world::repository` `has_familiar`, `familiar_clear` (logout) | `manager.has_familiar()`; the current familiar's NPC `clear` |
| `world::player_death` `dismiss_familiar` | `manager.dismiss(host)` |
| `world::wilderness_zone` `has_familiar`, `has_pet`, `familiar`, `familiar_transform`, `familiar_is_combat` | `has_familiar()`, `has_pet()`, `familiar().npc()`, `Familiar::transform`, `Familiar::is_combat_familiar` |
| `world::zones`, `world::npc_behaviour`, `timers::content` `familiar_owner` | `Familiar::owner()` of the familiar the node is |
| `world::combat_pulse` `familiar_adjust_battle_state`, `familiar_owner`, `familiar_attack_style`, `has_familiar`, `familiar_is_pet` | `adjust_battle_state`, `owner()`, `attack_style()`, `has_familiar()`, `is_pet()` |
| `world::combat_swing` `familiar_pouch` | the current familiar's `pouch_id()` |
| `world::packet_handlers::component` `has_familiar`, `familiar_special` | `has_familiar()`; `execute_special_move(&FamiliarSpecial::with(...))` |
| `world::dynamic_region` `is_familiar`, `recall_familiar` | whether the NPC is a familiar object; its flagInactive block stays that library's |
| `world::consumables` `familiar`, `update_special_points` | `familiar()`, `update_special_points` |
| `world::interfaces`, trade, shops, Grand Exchange `has_familiar` | `has_familiar()` |
| `inventory::listener` `familiar`, `has_pouch`, `set_has_pouch`; `appearance_writer` `using_summoning`, `summoning_combat_level` | the manager and its `has_pouch`/`set_has_pouch`, `is_using_summoning`, `summoning_combat_level` |
| `world::packet_handlers::drop` `is_pet` | `pets::Pet::for_id(id).is_some()` |

`world/live/items.rs` answers `is_pet` from a sorted `PET_ITEMS` list that
includes -1; the original's `Pets.forId(-1)` is null (no grown or overgrown
id of -1 is a key), so the live world should answer with `Pet::for_id`.

`tests/world_login.rs` replays `FamiliarManager.login` as a three-entry
content segment (varbits 4280, 4281 and 4282 for a player without a
familiar). To make it native the test's world keeps one `Manager` per player
on the persisted `PetStore` and answers `familiar_login` with
`manager.login(host)`, its `Host` reading `wolf_whistle_complete` from the
native quest repository (`world::player_tabs::quests`) and writing the
varbits through the native varbit writer; the segment then leaves the stand-in
list (54 stand-in calls become 51) and the three varbits come from the port.

## Not ported

The individual familiar classes (their specials, dialogues, `configureFamiliar`
listeners), the summoning pouches, scrolls, obelisks and the summoning tab,
remote viewing, the incubator, the familiar dialogue/feed/item/NPC option
plugins, and `Familiar.getCombatStyle` overrides (the port answers MAGIC, the
base; no content class of the corpus overrides it). Nothing installs the
library in `rs09-server --world`; `PlayerSaver.saveFamiliarManager` is checked
by the native `saved_active` test only, not by this corpus.
