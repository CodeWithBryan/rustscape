# NPCs made and changed at runtime

`crates/server/src/world/npc_runtime.rs` holds the `NPC` and `Entity` members
that run after the start-up: the three `NPC` constructors (`construct`,
`new_npc_at`, `new_npc`), `NPC.transform(id)`, `transformWithHpCarryover(id)`,
`reTransform()`, `NPC.clear()`, and `Entity.init()`/`Entity.clear()` (which a
player reaches too).

`NPC.init()` is not here: its body is `world::npc_init::init` (npc-init-oracle),
whose `super.init()` is `npc_runtime::entity_init` and whose object is made by
`npc_runtime::construct`. The members `init` reaches in the world
(`Repository.getNpcs().add`, `RegionManager.move`, `Region.isActive`,
`Repository.addRenderableNPC`) are bindings of the repository,
entity-regions and region-activity libraries, each with its own corpus.

The world is `npc_runtime::Host`. Field writes are accessors without a result;
every outside call is a required service that may fail.

What the original does and the port keeps:

- The constructor looks the definition up twice: once for the name the entity
  is made with (no definition fails before the entity exists), once for the
  `definition` field, whose size it then reads. It writes `id` and
  `originalId` before the second lookup, then the size and direction, makes
  the `InteractPlugin`, and asks `NPCBehavior.forId(id)` last.
- `transform` writes the id before the lookup, the definition (null included)
  before reading its name, then name and size, makes a new `InteractPlugin`,
  runs the virtual `initConfig()` and `configure()`, then the new plugin's
  `setDefault()`. Back at the original id it asks
  `EntityFlags.getOrdinal(NPC, TypeSwap)` and unregisters that synced slot;
  it always registers the TypeSwap mask, synced unless the id is the original.
- `transformWithHpCarryover` reads the lifepoints before and writes them after.
- `reTransform` does nothing at the original id.
- `Entity.clear` sets `active` false first; with a location a player leaves
  its chunk and the region's tolerances, an NPC its chunk; the pulse manager
  is cleared either way. `NPC.clear` then leaves the renderable list and the
  repository and tells its behaviour (a null behaviour fails there, after the
  removals).

## Corpus

`cargo xtask npc-runtime-oracle --check` (Python, JDK 11) inserts those
members verbatim from `NPC.java` and `Entity.java` into adapters and compiles
`NodeList.java`, `NodeListIterator.java`, `Location.kt`, `Vector.kt` and
`EntityFlag.kt` unchanged; `Repository.getNpcs()` is the real `NodeList`.
Each case runs in a class loader of its own. 912 cases (362 designed, 550 with
one injected failure) / 5,678 operations / 17,328 recorded calls: every
constructor over present and missing definitions, locations and directions;
init and clear for NPCs and players, placed or not, in or out of a full or
empty repository; transforms between every pair of ids, back, repeated, with
each ordinal answer, a missing definition, a table that changes under a live
NPC or between two lookups; and a random walk.
`cargo test -p rs09-server --test npc_runtime` replays them over a host whose
repository is the native `NodeList`: recorded calls in order, every field of
every entity after every operation, the repository's slots, results and
errors.

19 of 20 deliberate breaks fail the replay (one only after the cases that
change the definition table between the constructor's lookups were added);
the other (writing the direction before the size) cannot be observed.

Not covered: `NPC.init` (npc-init-oracle), `NodeList` and the renderable list
bodies (repository-oracle), `RegionManager.move` (entity-regions-oracle), the
update masks (update-masks-oracle, npc-masks-oracle), `AbstractNPC`
subclasses' overrides (npc-classes-oracle).

## Replacing the live world's own port

`world/live/runtime_npcs.rs` wrote these bodies from the pinned sources
without a corpus:

| Live function | Library call |
| --- | --- |
| `Live::npc_transform(node, id)` | `npc_runtime::transform(&mut host, &node, id)` |
| `Live::npc_clear(handle)` | `npc_runtime::clear(&mut host, &node)` |
| `Live::npc_new(id, location, direction)` (`npc_init::Context::base`) | `npc_runtime::construct(&mut host, id, location, direction)` |
| `Runtime::entity_init` (`npc_init::Context::entity_init`) | `npc_runtime::entity_init(&mut host, &node)` |
| a player's `Entity.clear` at logout | `npc_runtime::entity_clear(&mut host, &player)` |

The host over the live world: `definition_for_id` is `Live::npc_definition`;
`allocate` makes the canonical actor and NPC row; `new_interact_plugin` drops
the NPC's cached InteractPlugin table (the live world builds it when first
asked for); `init_config` is `npc_init::init_config`; `configure` and
`interact_default` are the `Runtime` members; `type_swap_ordinal` answers the
NPCFlags530 TypeSwap ordinal (5); `unregister_synced` and
`register_type_swap` are the NPC's `update_masks::Masks`; `chunk_remove_npc`
is `Live::region_clear`; `pulse_manager_clear` is `world.clear_pulses`;
`remove_renderable` and `repository_remove` are the live repository's (and
the start-up list's); `behavior_removed` is `npc_behaviour`'s `on_removal`.

Where the live port differs from the original today: `transform` does not
write the name; `originalId` is kept in a side map from the first transform
(equivalent while only `transform` changes the id); the TypeSwap ordinal is a
constant.
