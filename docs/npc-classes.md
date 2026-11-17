# NPC classes (`world/npc_classes`)

`crates/server/src/world/npc_classes/` ports the NPC subclass framework of
`core/game/node/entity/npc` (`AbstractNPC`, `IdleAbstractNPC`, the subclass
half of `NPC.create`) and the global NPC handlers of
`content/global/handlers/npc` that a plain playable world meets: the generic
shop keepers, bankers and creatures, and the NPC "talk-to" listener. Each
body is a function over the `Host` trait (and its construction half,
`Factory`), where every call the original makes outside the ported bodies is
a required method with no default. An NPC's subclass is data the world keeps
with the NPC (`Class`); the members a subclass overrides are dispatched by
that class in `classes` (`init`, `tick`, `tick_actions`, `finalize_death`,
`on_attack`, `on_impact`, `swing_handler`, `walk_radius`).

## What is ported

| Original | Rust | Scope |
| --- | --- | --- |
| `AbstractNPC` (`mapping`, `newInstance`, `forId`, the `(id, location[, autowalk])` constructors) and `NPC.create` | `registry` (`Registry`, `define`, `create`, `construct_npc`), `Abstract`, `Constructor` | the prototype construction, the id registration with its warning, the class-identity test, `construct` (and a null `construct` falling back to a base NPC), the Kotlin null check of `BankerNPC.construct` |
| `ShopNPC` | `Class::Shop`, `classes::init`, `classes::walk_radius` | its ids, `init` (`setWalks(true)` after `super.init()`), `getWalkRadius() = 3` |
| `CowNPC`, `DuckNPC` | `classes::tick` | `random(45) == 5` chat (not for the calf 1766), then `NPC.tick` |
| `RatNPC`, `HighwayManNPC`, `IceGiantNPC` | `classes::finalize_death`, `classes::on_attack` | the rat tail for Witch's Potion, the Falador diary tasks (180 only; within 100 of (3052, 9573, 0)), "Stand and deliver!" |
| `GuardNPC` | `Abstract for Class` | `setAggressive(false)` in both constructors |
| `MonkNPC` | `classes::swing_handler`, `monk_swing` | the static `CombatAction` (one object for every monk): heal 2, animation 709, next attack 4 and `-1` when hurt and `randomize(10) < 2`, else the melee swing |
| `DarkWizardNPC` | `classes::init`, `classes::on_impact` | the prototype's `setAggressive(true)`, the magic style and default spell (6 for 172, 4 for 174), the switch with `random(6) > 4` and `random(2)` over `SPELLS`, the `(CombatSpell)` cast |
| `BankerNPC` | `banker` | the three listeners, the destination override (ogress, Magnus, 6532-6535 by direction, the booth search for a BankerNPC), `init`'s booth search (south, west, north, east), the companion's Lunar Isle rule, Odovacar's fee, `attemptBank`/`attemptCollect` |
| `NPCTalkListener` | `talk` | the barcrawl handler (with `BarcrawlType.forId`) and the catch-all handler: random event and holiday NPCs, the holiday extra NPC, `facing_booth`, the combat refusals, gnome cooking deliveries (with `GnomeCookingJob`), the NPC's dialogue |
| `IdleAbstractNPC` | `idle` | `handleTickActions`, `isAttackable`, `inDisturbingRange`, `canDisturb`, `disturb`, `goIdle`, the fields' initial values |
| `NPCDisturbPlugin`, `HairCutPlugin` | `idle::disturb_option`, `disturb_option_walks`, `hair_cut` | both handlers and their `isWalk` |
| the classes' registrations | `listeners` (`define_listeners`, `define_plugin`, `register_scanned`, `Native`) | the content-interface path (`defineListeners` then `defineDestinationOverrides`; BankerNPC's construction is an NPC's) and the `@Initializable` path (`newInstance(null)`), in the start-up manifest's order |

## Behaviour that is easy to get wrong

- `newInstance` compares `getSimpleName()` strings with `!=`. JDK 11 caches a
  class's simple name, so this is class identity: the same class registered
  again replaces its prototype silently; another class keeps the id and the
  new one logs `[X] - Warning: Mapping already contained NPC id N! (Y)` once
  per id, even when both classes have the same simple name.
- Every AbstractNPC constructor sets `walks` (the two-argument one to
  `true`), so a prototype and every constructed NPC start walking until the
  spawner or `init` says otherwise. `construct` ignores the direction
  `NPC.create` was given (a subclass's NPC faces south).
- `BankerNPC.construct` is a Kotlin override with a non-null location: a
  null location fails before the constructor.
- The booth search reads the location again for every side, `transform`s
  through `Location`'s 15-bit packing (y 16383 north is -16384), and an
  object that is no booth is passed over.
- The destination override reads the direction only for 6532-6535 (a null
  direction fails there); only a `BankerNPC` object searches for a booth;
  any other NPC answers its own location.
- `NPCTalkListener` reads `AntiMacro.getEventNpc` (and
  `HolidayRandoms.getEventNpc`) up to four times, identity-compares it with
  the node, reads `finalized` through the getter, and reads the gnome
  cooking ordinal twice; an ordinal outside the 15 jobs fails with an index
  error after both reads.
- `IdleAbstractNPC.isAttackable` sends the slayer message but does not
  return false for it; `getTask()` is read twice.
- HighwayManNPC and IceGiantNPC read `getDiary(FALADOR)` again before
  `updateTask`.

## Required services and joins

The `Host` methods name the original call each answers. The world joins
them to its NPC actors (walks, aggression, direction and attributes are the
canonical NPC's), the native `npc_init`/`npc_behaviour` members for the
`base_*` calls (`NPC.init`, `tick`, `handleTickActions`, `finalizeDeath`,
`onImpact`, `getSwingHandler`), the combat pulse and swing handlers, skills,
the dialogue interpreter (`world::dialogue`), the bank and collection box,
the scenery of loaded regions, and the content it does not have yet as named
services: the banker conversation (`BankerDialogueLabellerFile`, ported in
`world::dialogue_dsl::banker`), the random event and holiday NPCs, the barcrawl
manager, the gnome cooking completion dialogue, quests and diaries.
`banker::check_lunar_isle_restriction` answers the `lunar_restriction`
service of `world::global_handlers` (the bank booth listeners).

`npc::Fields::walk_radius_override` carries a class's `getWalkRadius()`
override (ShopNPC: 3) to the one place the original calls the method rather
than reading the field (`getMovementDestination`).

## The start-up world

`startup::npcs` joins the registry to `npc_init::create`: the class scan
(`ClassScanner.loadSideEffectfulPlugins`) constructs the prototypes of the
classes in `STARTUP_NPC_CLASSES` (ShopNPC, CowNPC, DuckNPC, RatNPC, GuardNPC,
MonkNPC, HighwayManNPC, IceGiantNPC: `NPC(id, null)` with its definition
lookups) and registers their ids; `NPCSpawner`'s `NPC.create` answers
`AbstractNPC.forId` from that registry, constructs the class over a canonical
NPC tagged with it, and runs the class's own `init` statements after
`NPC.init`. `tests/startup.rs` checks `npcs.subclasses`: for every registered
class, the spawner NPCs the native start-up constructs as that class are
exactly the ones the original constructed as it (count and ids, from
`fixtures/startup-draws`), and every other boot-record comparison is
unchanged. `BankerNPC` (whose `init` needs the scenery of loaded regions) and
`DarkWizardNPC` (an autocast spell) are not registered in the start-up world,
whose regions hold no objects yet; their NPCs stay base NPCs there and the
class scan step names them as gaps.

## How the live world installs it

At start-up, after the InteractionListeners registry exists: build
`listeners::Native` over it with the world's casing and NPC factory and call
`listeners::register_scanned(&mut native, &mut registry, accept)` with the
world's AbstractNPC registry; bind `native.npc_handlers` into the NPC
definitions' `option:` handlers and `native.npc_options` into the default
NPC options. Dispatch an InteractionListeners function holding an
`Interaction` through `listeners::run_interaction`, a `Destination` through
`run_destination`, and the NPC option handlers through `run_handler` and
`handler_walks`. Run `classes::init` where the NPC's `init` runs (BankerNPC
needs the regions around its spawn loaded first), `classes::tick` and
`tick_actions` in the NPC tick, `finalize_death`, `on_attack`, `on_impact`
and `swing_handler` from combat and death. The live world is another slice's
(`world/live`); nothing here installs the library there yet.

## Reference tests

```powershell
cargo test -p rs09-server --test npc_classes
cargo xtask npc-classes-oracle --check --java-home <jdk11>
cargo test -p rs09-server --test startup
```

`oracle/npc-classes` compiles the originals listed in `UNCHANGED` of
generate.py unchanged on JDK 11 with Kotlin 1.8.20 and the pinned ConstLib
1.4.28 (NPCs, Items and Scenery constants checked against their names), with
the unchanged OptionHandler, Plugin, IntType, core.api.Container and
Direction. Original text inserted at generation time: `NPC.create` (both
overloads), the Skills ids, `BankBoothListener.BANK_BOOTHS`, the
`BarcrawlType` constants' NPC ids and `BarcrawlType.forId`; `GnomeCookingJob`
and `GnomeCookingConstants` are compiled unchanged. Two probe classes (a class
claiming ids another holds, and `oracle.probe.ShopNPC`) exercise the
registry's conflict path; `oracle.IdleProbe` is a concrete IdleAbstractNPC.
Everything else is a recording adapter answering from the case; the
adapters' fields are named apart from the getters' Kotlin properties. The
clock IdleAbstractNPC reads itself is recorded relative to it, in whole
seconds.

`fixtures/npc-classes/oracle.jsonl`: 815 cases (218 designed, 597 with one
recorded call failing in turn), 2,178 operations, 5,026 recorded calls, in
12 groups (registration, create, init, tick, death, attacked, impact, swing,
talk, banker, idle, handlers). The test replays every operation through the
native bodies and registrations with a recording host that follows the
adapters and compares the result, the error, every recorded call (NPC
construction and registrations included, with the original class names and
object identities) and the attributes and NPC fields after each operation.
Every case matches; there are no pinned gaps. The same file checks that
`register_scanned` over the start-up manifest produces exactly the oracle's
scan-order registration and that `listeners::Native` installs the functions
into the native InteractionListeners registry.

Twenty-four deliberate breaks of the port were tried (the registry's class
test and warning text, the Kotlin null check, Guard's aggression, the
ShopNPC radius, MonkNPC's prototype id, the calf id, the monk heal and
wizard switch thresholds, the second diary read, the ice giant distance, the
booth search order, the 6532-6535 range, the Lunar Isle region, the banker's
`faceLocation(null)`, the second gnome cooking read, the holiday class test,
the first event NPC read, the idle timeout unit, the slayer message
condition, the disturb order, the catch-all options, a handler class name and
HairCutPlugin's `isWalk`): every one fails the replay.

## Differences from the original

- A Kotlin lambda that captures nothing is one object however often its
  class registers; the port makes one function object per registration (the
  original defines each class once at start-up).
- Errors carry a category, not a Java stack trace; the class scan's own
  catch-and-log around a failing class is the caller's (the native start-up
  refuses the step).
- The IdleAbstractNPC clock is the `current_time_millis` service.

## Not ported or not joined

- The banker conversation (`BankerDialogueLabellerFile`) is ported in
  `world::dialogue_dsl::banker` (docs/dialogue-dsl.md);
  `Host::banker_conversation` stays the join point: a live host answers it
  with `dialogue_dsl::banker::talk_to`. Nothing binds it yet.
- `ChromaticDragonNPC`, `MetalDragonNPC` and `ChromaticDragonBehavior` are
  `world::dragonfire::dragons` (docs/dragonfire.md): `DragonClass` implements
  `Abstract`, so `registry` registers and constructs them; `Class` does not
  list them, but the start-up world registers them through
  `world::creatures::classes::WorldClass` (both kinds in one enum).
  `SheepBehavior`, `ChinchompaDeathListener`, `ShooAwayStrayDogListener`,
  `CowMilkingPlugin` and `DecantListener` are `world::creatures`
  (docs/equipment-registers.md); `QuestTabUtils.showRequirementsInterface`
  is not ported in this pass.
- The start-up world registers BankerNPC and DarkWizardNPC only once its
  regions hold objects and spells (see above); NPCTalkListener, BankerNPC's
  listeners and the two option handlers are not registered there (the
  start-up world has no InteractionListeners registry).
- Nothing installs the libraries in `rs09-server --world` yet.
