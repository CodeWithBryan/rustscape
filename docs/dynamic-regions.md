# Instanced (dynamic) regions

`world/dynamic_region.rs` ports `core.game.world.map.build.DynamicRegion`
and the instancing parts of RegionChunk and Region it uses; `world/dynamic_scene.rs`
ports the instanced scene packets. Both are joined to the native scene
(`world/scene.rs`), region loading (`region.rs`), landscape and mapscape
parsing, clipping, SceneryBuilder (`scenery.rs`), the chunk store
(`chunk.rs`) and region activity (`region_activity.rs`).

- The region map is `Scene::regions` (RegionManager's cache: `forId` creates
  a plain region on a miss, `add_region` is RegionManager.addRegion) and the
  instance areas are `Scene::reserved_areas` (RESERVED_AREAS, removed by
  ZoneBorders equality).
- `construct` is `new DynamicRegion(regionId, x, y)`: Region's constructor,
  then every tile of the new region's chunk keys invalidated (-1) for
  movement and cleared (0) for projectiles in the global flag arrays.
  `with_borders` is `new DynamicRegion(ZoneBorders)`.
- `find_zone_borders`, `reserve_area`, `create`, `create_between` and
  `create_area` are the static factories: the original search (four corners
  against every reserved area, a region per step, fifteen probes per row), one
  shared borders object per area, each copy registered right after it is
  built, then a scene update for any player already on the new regions.
- `copy` loads the template and copies every chunk with RegionChunk.copy,
  places it with `replace_chunk`, then sets rotation 0 and the template tile as
  its base. `replace_chunk` keeps the original order: this region is loaded
  first (the first load parses the template's map and landscape onto the
  region's own, soon replaced, chunks and leaves their clipping in the flag
  arrays), the replacement stored, the source loaded, the replacement rebased,
  given fresh ChunkFlags, `rebuild_clipping_flags` from the source's landscape
  marks and its own objects (LandscapeParser.flagScenery storing all, not as
  landscape), and every NPC of the source chunk copied onto the instance tile.
  A copy is not registered while it is built, so the region lookups of its
  own load, of border flags and of the copied NPCs reach a plain placeholder
  region at the instance's id, exactly as in the original.
- `rotate` is RegionChunk.rotate: refused with a log once rotated; both layers
  taken out (dynamic objects removed through SceneryBuilder), new layer arrays,
  the rotation (a diagonal direction falls back with the original's formula and
  log), and every object put back at `getRotatedPosition` of its definition's
  size: static objects into the first free slot (dropped when none), dynamic
  ones added through SceneryBuilder.
- `link`, `toggle_multicombat` (a RegionZone of the multiway zone over the
  borders; off removes every RegionZone of that zone object), `set_music_id`,
  `clear`, the borders/multicombat/permanent/parent accessors.
- `flag_inactive` is the override: a permanent instance answers true; an
  active parent is checked for inactivity instead; any linked region that is
  not inactive keeps it; then Region's own flagInactive, and only when that
  succeeds: familiars recalled, other NPCs cleared, ground items destroyed
  (the chunks were cleared first, so the list is a new empty one, as in the
  original), the area released, multiway combat switched off and every linked
  region flagged inactive (all of them, `&=`).
- `region_activity::Kind` carries the region class through Region's methods.
  The unsuffixed region_activity functions keep refusing a DynamicRegion
  (`Error::Dynamic`); `dynamic_region::{check_inactive, is_inactive,
  flag_active, unload, pulse_body, clear_chunk, remove_region}` accept both
  classes: DynamicRegion's override is reached from the activity pulse,
  `unload` skips `onRegionInactivity` for an instance and RegionChunk.clear
  empties and drops an instance chunk's item list.
- `dynamic_scene::send` is BuildDynamicScene (214): the local position, zone
  x and plane, one bit per chunk of the 13 by 13 build area on four planes
  (set with rotation, plane and zone of the copied chunk for a chunk of a
  DynamicRegion whose base is not negative; every region of the area is looked
  up, and so created, through the scene's map), the XTEA keys of the copied
  regions in first-use order, the zone y; the last scene location is published
  after the write. `update_scene_graph` is Player.updateSceneGraph's choice
  between it and the normal rebuild (`rebuild::send`, opcode 162).
  `minimap_state` (192) and `clear_minimap_flag` (153) are the two minimap
  packets.

NPC creation and clearing, familiar recall, ground item destruction, player
scene updates, logging and the multiway zone object are required services
(`Build` while building, `Host` while activating and removing), with no
defaults; no lock is held across one. Diagnostic ids never select behaviour.

## Verification

`cargo xtask dynamic-regions-oracle --check` (Python and JDK 11) exports from
the pinned cache the map and landscape bytes of every region the corpus
reaches, the scenery definitions of every object in them with the closure of
their children, and the pinned XTEA keys, into
`fixtures/dynamic-regions/cache.json`. The regions reached are found by the
original itself: the JVM oracle runs in a discovery mode naming every map or
landscape archive it looked up but was not given, and the export grows until
nothing is missing (52 regions). It then compiles the unchanged DynamicRegion,
ChunkFlags, LandscapeParser, MapscapeParser, Region, RegionChunk,
RegionManager.kt, Direction, Point, ZoneBorders, Pulse, Scenery, Constructed,
SceneryBuilder, SceneryDefinition, the object update flag and writers,
BuildDynamicScene, UpdateSceneGraph, MinimapState and ClearMinimapFlag with
their contexts, with Player.updateSceneGraph, XteaParser's lookup,
MapChunkRenderer's build-area constants, isPlusN and Location's members
inserted verbatim. Entities, the zone monitor, NPC creation, familiars, map
zones, ground items, the pulser (a fixed-order queue), PacketRepository and
the session are recording adapters (`oracle/dynamic-regions/adapters.py`).

The corpus (`oracle/dynamic-regions/cases.py`) has 36 cases and 756 actions
over Lumbridge (12850), the Fight Caves (9551), Pest Control (10536), the
Fishing Trawler (8011), 11573, 12854 and Clan Wars (13114/13115): instances
created from every template with objects and clipping read back on two
planes, players entering and leaving, scene packets inside, at the edges and
outside instances, the minimap packets, the 50-tick deactivation; template
NPCs copied (landing on the unregistered placeholder region) and instance
NPCs and familiars cleared or recalled; allocation order, rows of narrow and wide areas, release and
reuse, multi-region and odd-sized areas, empty and unregistered instances;
house-style chunks replaced and rotated in every direction, twice and
diagonally with build on and off; linked house/dungeon pairs and a chain whose first linked region refuses,
viewers and players refusing unloading, permanent instances, multiway combat and music,
ground items, removal of an occupied instance; injected failures of NPC
creation, NPC init, familiar recall, NPC clearing and the session write
(caught and logged by PacketRepository) midway, a null NPC and
the content's DynamicRegion casts. After every action the replay compares the
result or error, every region of the map (class, template, coordinates, load
state, object count, activity, viewers), each instance's fields, region zones,
music zones and NPC list, a hash of every stored static and dynamic object, a
hash of every chunk's bases, rotation, entity lists, item list, update flags
and landscape marks, a hash of every movement and projectile flag array, the
reserved areas, the pulse queue, the players' last scene, recorded events and
the written packet bytes. Set `DYNAMIC_REGIONS_DUMP` (oracle) or
`DYNAMIC_REGIONS_NATIVE_DUMP` (test) to a directory to write the hashed text
of each step.

`cargo test -p rs09-server --test dynamic_regions` replays it against the
port with the real native scene over the exported cache. One difference is
pinned: toggleMulticombat on an instance that was never given borders stores a
RegionZone with null borders in the original (which fails at the next zone
check); the native RegionZone cannot hold null borders, so the toggle is
refused with `Null`, and that case is compared up to that step.

Fourteen deliberate breaks of the port were run against the replay (search
row length, construction flag values, the replacement stored before the first
load, the projectile landscape copy, the area release, the diagonal fallback,
the chunk rotation bits, onRegionInactivity for instances, the instance item
list, `&=` over linked regions, the NPC copy offsets, multiway switched off,
the last scene before the write, the copied chunk's base plane); 13 fail it.
The projectile landscape copy is unobservable: nothing in the original ever
marks a projectile ChunkFlags landscape, so the copied marks are always false.

## Not ported or not joined

- Nothing installs the library in `rs09-server --world`: the live world's
  `update_scene_graph` still writes the normal rebuild, its region activation
  calls the static region_activity functions, its NPC host answers
  `region_dynamic` false and NPC.clear is unbound. Installing means: the live
  host implements `dynamic_region::Host` (reserved areas and region removal
  from its scene, the multiway zone of its zone host, familiar recall, NPC
  clear, ground item destruction) and `Build`; its update_scene_graph calls
  `dynamic_scene::update_scene_graph`; region activation, the activity pulse
  body, check_inactive and unload go through `dynamic_region::*`; and
  `region_dynamic` answers `dynamic_region::is_dynamic`.
- The region specification DSL (`core.api.regionspec`), HouseManager and
  the content that builds instances are not ported. The activity and
  cutscene framework is `world/activities` (docs/activities.md).
- A copied NPC's own creation (NPC.create, init) and the familiar recall
  script are services; the recall script never runs in the corpus.
- `create_area`'s scene update loop is ported but unreachable in the original
  as well as here: every chunk of a new instance is a fresh copy with empty
  entity lists, so `Build::update_scene_graph` is never called. The null
  template chunk branch of `copy` is likewise unreachable (Region's
  constructor fills every chunk).
- `toggle_multicombat` and `set_music_id` refuse an instance without borders
  (see the pinned difference); content always gives borders first.
- `find_zone_borders` stops after 2^24 probes with `Limit` (the original
  loops until it finds an area).
