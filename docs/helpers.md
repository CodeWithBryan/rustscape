# AttackListener, the debug members and hasGraveAt

`crates/server/src/world/helpers.rs` holds small original members the live
world had written for itself:

- `AttackListener` (`content/global/skill/AttackListener.kt`):
  `define_listeners` (`flagInstant()`, then `on(IntType.NPC, "attack")`) and
  `attack`, the handler. The magic dummy (4474) refuses a swing handler whose
  type is not MAGIC ("You can only attack this with magic."), the melee dummy
  (7891) one whose type is not MELEE ("You must use the training sword to
  attack this."); the handler is asked only for a dummy, and a handler
  without a type is refused by both. A killerwatt (3201, 3202) is cast to
  `NPC` and provoked through `KillerwattBehavior.aggressUponKillerwatt`
  first. Then `player.attack(npc)`. The handler always answers true.
- `Player.toggleDebug()`, `Player.debug(text)` and `Player.isDebug()`.
  `getAttribute("debug", false)` is unboxed as a `Boolean`: a stored value of
  another type is a `ClassCastException`. `toggleDebug` writes the attribute,
  then sends "Your debug mode is toggled to true." or "... false.".
  `isDebug` reads the attribute only for an administrator.
- `GraveController.hasGraveAt(loc)`: any active grave whose location equals
  `loc` by its packed coordinates; a grave without a location matches
  nothing, and a null `loc` fails Kotlin's parameter check.

Each has its own host trait (`Listeners`, `AttackHost`, `DebugHost`);
`has_grave_at` takes the graves' locations. Every service is required.

## Corpus

`cargo xtask helpers-oracle --check` (Python, JDK 11) compiles
`AttackListener.kt`, `IntType.kt`, `Location.kt` and `Vector.kt` unchanged
against the published ConstLib 1.4.28 and inserts `Player.toggleDebug`,
`debug`, `isDebug` and `GraveController.hasGraveAt` verbatim; the listener
registration, `getSwingHandler`, ContentAPI `sendMessage`, the killerwatt
behaviour, `Entity.attack`, the attribute map, the packet dispatch and the
rights are recording adapters. Each case runs in a class loader of its own.
355 cases (178 designed, 177 with one injected failure) / 2,161 operations /
2,419 recorded calls. `cargo test -p rs09-server --test helpers` replays them:
recorded calls in order with their arguments, results, errors and the
attribute map after every operation.

18 of 18 deliberate breaks fail the replay.

Not covered here, and corpus-backed elsewhere: `CombatPulse.attack`
(combat-pulse-oracle), ContentAPI `sendMessage`'s line splitting
(content-api-oracle). The killerwatt behaviour is content and stays a service.

## What the live world should call

| Live code | Library call |
| --- | --- |
| `world/live/items.rs`, `AttackListener`'s body (`attack_listener`, the dummies, the refused killerwatt, `player.attack`) | `helpers::attack(&mut host, &player, &npc)` |
| `world/live/items.rs`, the registration (`flagInstant`, `Function::new(FunctionKind::On, ATTACK_LISTENER, ..)`) | `helpers::define_listeners(&mut registrar)` with `helpers::ATTACK_LISTENER` |
| `world/live/global.rs`, `GHost::toggle_debug` (the attribute flipped, then the message) | `helpers::toggle_debug(&mut host, &player)` |
| `world/live/graves.rs`, `Live::has_grave_at(at)` | `helpers::has_grave_at(graves' NPC locations, at.as_ref())` |
| `world/live/global.rs`, `GHost::valid_login` (a literal `true`) | `system_manager::Config::valid_login()` of the world's `SystemConfig` (already ported; command-sets-oracle replays `validLogin`) |
| `world/live/content.rs`, `Live::register_hint_icon` and `Live::clear_hint_icon` | `content_api::ui::register_hint_icon` and `content_api::ui::clear_hint_icon` (already ported; content-api-oracle), which need `content_api::Host` bound for the members `HintIconManager.registerHintIcon`, `HintIconManager.removeHintIcon`, `Entity.removeAttribute` and the attribute read and write |

Where the live code differs from the original today: `has_grave_at(None)`
answers true for a grave without a location (the original fails); a
`hinticon` attribute of another type is a binding error there and a
`ClassCastException` in the original; `attack_listener` casts the node to an
NPC before anything else (the original casts only a killerwatt) and refuses
the killerwatt branch outright (in the library that refusal is the
`aggress_upon_killerwatt` service's to make).
