//! DynamicRegion (`core.game.world.map.build`): instanced copies of map
//! regions, their reserved areas, chunk replacement and rotation
//! (RegionChunk.rotate), linking, the multiway and music fields and the
//! `flagInactive` override, joined to the native scene (region map, clipping
//! arrays, Region.load, LandscapeParser, SceneryBuilder) and to region
//! activity through `region_activity::Kind`.
//!
//! The region map is `Scene::regions` (RegionManager's cache) and the
//! reserved areas are `Scene::reserved_areas`. Building takes the scene's
//! `World` and a `Build` host; activation and removal take a `Host`. Every
//! system without a port here (NPC creation and clearing, familiars, ground
//! items, player scene updates, logging, the multiway zone object) is a
//! required service with no default, and no lock is held across one.
use super::{
    chunk::{self, HandleChunk, RegionChunk},
    chunk_sync,
    doors::{Handle as Scenery, Location},
    ground_item,
    landscape::{self, Chunks},
    player_walk::NodeHandle,
    region,
    region_activity::{self, Kind},
    scene::{Assets, Scene, Services, World},
    scenery, scenery_node, update,
    zones::{self, Borders, List, MusicZone, RegionZone, Zone},
};
use crate::definitions::item::{Shared, WeakShared};

pub use zones::{Error, Result};

/// Region.SIZE.
pub const SIZE: i32 = 64;
pub const CHUNKS: usize = 8;
pub const PLANES: usize = 4;
/// A native bound on findZoneBorders' search; the original loops until it
/// finds a free area.
pub const MAX_AREA_PROBES: u64 = 1 << 24;

/// RESERVED_AREAS: an ArrayList of ZoneBorders, removed by `equals`.
pub type Reserved = List<Borders>;

/// DynamicRegion's own fields.
pub struct Instance {
    /// `regionId`: the copied region (-1 for an empty instance).
    pub region_id: i32,
    pub borders: Option<Borders>,
    pub multicombat: bool,
    pub permanent: bool,
    pub linked: Option<Vec<region::Handle>>,
    /// `parentRegion`. The parent holds its linked regions strongly; the
    /// child's view is weak so a family is not an Arc cycle. An expired
    /// parent is a binding error, never read as null.
    pub parent: Option<WeakShared<region::Region>>,
    /// The public `NPCs` list that `clear()` empties.
    pub npcs: List<NodeHandle>,
}

fn bind(error: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{error:?}"))
}
pub fn instance(region: &region::Handle) -> Option<Shared<Instance>> {
    region.read().activity.read().dynamic.clone()
}
fn fields(region: &region::Handle) -> Result<Shared<Instance>> {
    instance(region).ok_or(Error::ClassCast)
}
/// `region instanceof DynamicRegion`.
pub fn is_dynamic(region: &region::Handle) -> bool {
    region.read().dynamic_template.is_some()
}
/// `getRegionId()`: the copied region's id for a DynamicRegion.
pub fn region_id(region: &region::Handle) -> i32 {
    let r = region.read();
    r.dynamic_template.unwrap_or_else(|| r.id())
}
pub fn borders(region: &region::Handle) -> Result<Option<Borders>> {
    Ok(fields(region)?.read().borders.clone())
}
pub fn set_borders(region: &region::Handle, borders: Option<Borders>) -> Result<()> {
    fields(region)?.write().borders = borders;
    Ok(())
}
pub fn is_multicombat(region: &region::Handle) -> Result<bool> {
    Ok(fields(region)?.read().multicombat)
}
pub fn set_multicombat(region: &region::Handle, multicombat: bool) -> Result<()> {
    fields(region)?.write().multicombat = multicombat;
    Ok(())
}
pub fn is_permanent(region: &region::Handle) -> Result<bool> {
    Ok(fields(region)?.read().permanent)
}
pub fn set_permanent(region: &region::Handle, permanent: bool) -> Result<()> {
    fields(region)?.write().permanent = permanent;
    Ok(())
}
/// `getParent()`.
pub fn parent(region: &region::Handle) -> Result<Option<region::Handle>> {
    let parent = fields(region)?.read().parent.clone();
    match parent {
        None => Ok(None),
        Some(weak) => weak
            .upgrade()
            .map(Some)
            .ok_or_else(|| Error::Binding("expired parent region".into())),
    }
}
pub fn linked(region: &region::Handle) -> Result<Option<Vec<region::Handle>>> {
    Ok(fields(region)?.read().linked.clone())
}
pub fn npcs(region: &region::Handle) -> Result<List<NodeHandle>> {
    Ok(fields(region)?.read().npcs.clone())
}
/// `getBaseLocation()`.
pub fn base_location(region: &region::Handle) -> Location {
    let r = region.read();
    let d = r.data.read();
    Location::new(d.x.wrapping_shl(6), d.y.wrapping_shl(6), 0)
}
fn chunks(region: &region::Handle) -> Result<Chunks> {
    region.read().chunks.clone().ok_or(Error::Null)
}
fn chunk_at(region: &region::Handle, x: usize, y: usize, z: usize) -> Result<HandleChunk> {
    let chunks = chunks(region)?;
    let chunk = chunks.read()[x][y][z].clone().ok_or(Error::Null)?;
    Ok(chunk)
}
fn transform(l: &Location, dx: i32, dy: i32, dz: i32) -> Location {
    let [x, y, z] = l.coordinates();
    Location::new(
        x.wrapping_add(dx),
        y.wrapping_add(dy),
        z.wrapping_add(dz) % 4,
    )
}

/// RegionManager.addRegion.
pub fn add_region<A: Assets>(scene: &mut Scene<A>, id: i32, region: &region::Handle) {
    scene.regions.insert(id, region.clone());
}

/// `new DynamicRegion(regionId, x, y)`: Region's constructor, then every
/// tile of the new region's chunk keys invalidated (-1) for movement and
/// cleared (0) for projectiles.
pub fn construct(
    ctx: &mut impl super::clipping::Context,
    region_id: i32,
    x: i32,
    y: i32,
) -> Result<region::Handle> {
    let region = Shared::new(region::Region::new(x, y));
    region.write().dynamic_template = Some(region_id);
    region.read().activity.write().dynamic = Some(Shared::new(Instance {
        region_id,
        borders: None,
        multicombat: false,
        permanent: false,
        linked: None,
        parent: None,
        npcs: List::new(),
    }));
    for cx in 0..CHUNKS {
        for cy in 0..CHUNKS {
            for cz in 0..PLANES {
                let chunk = chunk_at(&region, cx, cy, cz)?;
                for ox in 0..8 {
                    for oy in 0..8 {
                        let flags = chunk.read().flags.clone();
                        flags.invalidate(ctx, ox, oy).map_err(bind)?;
                        let projectile = chunk.read().projectile_flags.clone();
                        projectile.clear(ctx, ox, oy).map_err(bind)?;
                    }
                }
            }
        }
    }
    Ok(region)
}
/// `new DynamicRegion(ZoneBorders)`: an empty instance (-1) at the borders'
/// region, given the borders and registered.
pub fn with_borders<A: Assets, S: Services<A>>(
    world: &mut World<'_, A, S>,
    borders: Borders,
) -> Result<region::Handle> {
    let region = construct(
        world,
        -1,
        borders.south_west_x() >> 6,
        borders.south_west_y() >> 6,
    )?;
    set_borders(&region, Some(borders))?;
    let id = region.read().id();
    add_region(world.scene, id, &region);
    Ok(region)
}

/// The services building an instance reaches.
pub trait Build<A: Assets> {
    /// `player.updateSceneGraph(false)` for a player already on a new region.
    fn update_scene_graph(&mut self, scene: &mut Scene<A>, player: &NodeHandle) -> Result<()>;
    /// `npc.getLocation()` of an NPC of the copied chunk.
    fn npc_location(&mut self, scene: &mut Scene<A>, npc: &NodeHandle) -> Result<Location>;
    /// `NPC.create(id, location, direction)` from the template NPC, its
    /// aggressive, respawn and walks flags copied, then `init()`.
    fn copy_npc(
        &mut self,
        scene: &mut Scene<A>,
        npc: &NodeHandle,
        location: Location,
    ) -> Result<()>;
    /// `log(RegionChunk.class, Log.ERR, message)`.
    fn log_error(&mut self, scene: &mut Scene<A>, message: &str) -> Result<()>;
}

/// `findZoneBorders(sizeX, sizeY)` (sizes in chunks): the first area, in
/// steps of a region along x and fifteen probes per row, none of whose four
/// corners lies inside a reserved area.
pub fn find_zone_borders(reserved: &Reserved, size_x: i32, size_y: i32) -> Result<Borders> {
    let (mut x, mut y) = (0i32, 0i32);
    let mut count = 0i32;
    let width = ((size_x >> 3) << 6).max(SIZE);
    let height = ((size_y >> 3) << 6).max(SIZE);
    for _ in 0..MAX_AREA_PROBES {
        let end_x = x.wrapping_add(width);
        let end_y = y.wrapping_add(height);
        let mut taken = false;
        let mut cursor = reserved.iter();
        while cursor.has_next(reserved) {
            let b = cursor.next(reserved)?;
            if b.inside_xy(x, y)?
                || b.inside_xy(end_x, end_y)?
                || b.inside_xy(x, end_y)?
                || b.inside_xy(end_x, y)?
            {
                taken = true;
                break;
            }
        }
        if !taken {
            return Ok(Borders::new(x, y, end_x, end_y));
        }
        count = count.wrapping_add(1);
        if count % 15 == 0 {
            y = y.wrapping_add(SIZE);
            x = 0;
        } else {
            x = x.wrapping_add(SIZE);
        }
    }
    Err(Error::Limit)
}
/// `reserveArea(sizeX, sizeY)`.
pub fn reserve_area(reserved: &Reserved, size_x: i32, size_y: i32) -> Result<Borders> {
    let borders = find_zone_borders(reserved, size_x, size_y)?;
    reserved.add(borders.clone());
    Ok(borders)
}
/// `RESERVED_AREAS.remove(borders)`: the first equal entry.
pub fn release_area(reserved: &Reserved, borders: Option<&Borders>) -> Result<bool> {
    let Some(borders) = borders else {
        // ArrayList.remove(null) finds no null entry: none is ever added.
        return Ok(false);
    };
    for (index, candidate) in reserved.values().iter().enumerate() {
        if borders.equals(candidate)? {
            reserved.remove_at(index)?;
            return Ok(true);
        }
    }
    Ok(false)
}

/// `create(regionId)`.
pub fn create<A: Assets, S: Services<A>>(
    world: &mut World<'_, A, S>,
    host: &mut impl Build<A>,
    region_id: i32,
) -> Result<region::Handle> {
    let x = (region_id >> 8) << 6;
    let y = (region_id & 0xFF) << 6;
    let regions = create_area(
        world,
        host,
        &Borders::new(x, y, x.wrapping_add(SIZE), y.wrapping_add(SIZE)),
    )?;
    regions.into_iter().next().ok_or(Error::Null)
}
/// `create(regionOne, regionTwo)`: both and everything between; the first.
pub fn create_between<A: Assets, S: Services<A>>(
    world: &mut World<'_, A, S>,
    host: &mut impl Build<A>,
    region_one: i32,
    region_two: i32,
) -> Result<region::Handle> {
    let x = (region_one >> 8) << 6;
    let y = (region_one & 0xFF) << 6;
    let x1 = (region_two >> 8) << 6;
    let y1 = (region_two & 0xFF) << 6;
    let regions = create_area(
        world,
        host,
        &Borders::new(x, y, x1.wrapping_add(SIZE), y1.wrapping_add(SIZE)),
    )?;
    regions.into_iter().next().ok_or(Error::Null)
}
/// `create(ZoneBorders copy)`: reserve an area, copy every region of the
/// borders into it (each given the one shared borders object and registered
/// right after its copy), then send a scene update to every player found on
/// the new regions.
pub fn create_area<A: Assets, S: Services<A>>(
    world: &mut World<'_, A, S>,
    host: &mut impl Build<A>,
    copy_borders: &Borders,
) -> Result<Vec<region::Handle>> {
    let (sx, sy) = (copy_borders.south_west_x(), copy_borders.south_west_y());
    let (nx, ny) = (copy_borders.north_east_x(), copy_borders.north_east_y());
    let base_x = sx >> 6;
    let base_y = sy >> 6;
    let reserved = world.scene.reserved_areas.clone();
    let border = find_zone_borders(
        &reserved,
        nx.wrapping_sub(sx) >> 3,
        ny.wrapping_sub(sy) >> 3,
    )?;
    reserved.add(border.clone());
    let l = Location::new(border.south_west_x(), border.south_west_y(), 0);
    let mut regions = Vec::new();
    for x in (sx >> 6)..(nx >> 6) {
        for y in (sy >> 6)..(ny >> 6) {
            let id = (x << 8) | y;
            let to = transform(&l, (x - base_x) << 6, (y - base_y) << 6, 0);
            let dest = copy(world, host, id, &to)?;
            set_borders(&dest, Some(border.clone()))?;
            let dest_id = dest.read().id();
            add_region(world.scene, dest_id, &dest);
            regions.push(dest);
        }
    }
    for r in &regions {
        for z in 0..PLANES {
            for player in assemble_player_list(r, z)?.into_iter().flatten() {
                host.update_scene_graph(world.scene, &player)?;
            }
        }
    }
    Ok(regions)
}
/// `assemblePlayerList(z)`: chunk x, y order, each list copied.
pub fn assemble_player_list(region: &region::Handle, z: usize) -> Result<Vec<Option<NodeHandle>>> {
    let mut list = Vec::new();
    for x in 0..CHUNKS {
        for y in 0..CHUNKS {
            list.extend(chunk_at(region, x, y, z)?.read().players.values());
        }
    }
    Ok(list)
}
/// `assembleNpcList(z)`.
pub fn assemble_npc_list(region: &region::Handle, z: usize) -> Result<Vec<Option<NodeHandle>>> {
    let mut list = Vec::new();
    for x in 0..CHUNKS {
        for y in 0..CHUNKS {
            list.extend(chunk_at(region, x, y, z)?.read().npcs.values());
        }
    }
    Ok(list)
}

/// `copy(regionId, to)`: a new instance at `to`'s region holding a copy of
/// every chunk of the loaded template, each placed with `replaceChunk`, then
/// given rotation 0 and the template tile as its base. Not registered.
pub fn copy<A: Assets, S: Services<A>>(
    world: &mut World<'_, A, S>,
    host: &mut impl Build<A>,
    region_id: i32,
    to: &Location,
) -> Result<region::Handle> {
    let region_x = ((region_id >> 8) & 0xFF) << 6;
    let region_y = (region_id & 0xFF) << 6;
    let [tx, ty, _] = to.coordinates();
    let region = construct(world, region_id, (tx >> 3) >> 3, (ty >> 3) >> 3)?;
    let base = world.scene.region(region_id);
    region::load(world, &base, None).map_err(Error::from)?;
    for offset_x in 0..CHUNKS {
        for offset_y in 0..CHUNKS {
            let x = region_x + ((offset_x as i32) << 3);
            let y = region_y + ((offset_y as i32) << 3);
            for plane in 0..PLANES {
                let source = chunks(&base)?.read()[offset_x][offset_y][plane].clone();
                let c = match source {
                    None => {
                        let c = Shared::new(RegionChunk::new(Location::new(0, 0, 0), 0));
                        chunks(&region)?.write()[offset_x][offset_y][plane] = Some(c.clone());
                        c
                    }
                    Some(source) => {
                        let c = chunk::copy(world, &source).map_err(bind)?;
                        replace_chunk(world, host, &region, plane, offset_x, offset_y, &c, &base)?;
                        c
                    }
                };
                let mut c = c.write();
                c.rotation = 0;
                c.base = Location::new(x, y, plane as i32);
            }
        }
    }
    Ok(region)
}

/// `replaceChunk(z, x, y, replacement, fromRegion)`: this region is loaded
/// (a first load parses the template's map and landscape onto its own
/// chunks), the replacement is stored, the source region loaded, and the
/// replacement rebased to this region's tile, given fresh clipping flags
/// rebuilt from its objects and the source chunk's landscape, and the source
/// chunk's NPCs are copied onto it.
#[allow(clippy::too_many_arguments)]
pub fn replace_chunk<A: Assets, S: Services<A>>(
    world: &mut World<'_, A, S>,
    host: &mut impl Build<A>,
    region: &region::Handle,
    z: usize,
    x: usize,
    y: usize,
    replacement: &HandleChunk,
    from: &region::Handle,
) -> Result<()> {
    if z >= PLANES || x >= CHUNKS || y >= CHUNKS {
        return Err(Error::Binding("ArrayIndexOutOfBounds".into()));
    }
    region::load(world, region, None).map_err(Error::from)?;
    chunks(region)?.write()[x][y][z] = Some(replacement.clone());
    region::load(world, from, None).map_err(Error::from)?;
    let orig = chunks(from)?.read()[x][y][z].clone().ok_or(Error::Null)?;
    let current_base = transform(
        &base_location(region),
        (x as i32) << 3,
        (y as i32) << 3,
        z as i32,
    );
    replacement.write().current_base = current_base.clone();
    chunk::rebase_objects(replacement);
    replacement.write().reset_clipping_flags();
    rebuild_clipping_flags(world, replacement, &orig)?;
    let npcs = orig.read().npcs.clone();
    let mut failure = None;
    let walked = npcs.each(|npc| {
        let result = (|| {
            let npc = npc.ok_or(Error::Null)?;
            let [nx, ny, _] = host.npc_location(world.scene, &npc)?.coordinates();
            let location = transform(&current_base, nx & 7, ny & 7, 0);
            host.copy_npc(world.scene, &npc, location)
        })();
        result.map_err(|error| {
            failure = Some(error);
            update::Error::Binding(String::new())
        })
    });
    walked.map_err(|error| {
        failure.take().unwrap_or(match error {
            update::Error::ConcurrentModification => Error::ConcurrentModification,
            update::Error::Null => Error::Null,
            other => bind(other),
        })
    })
}
/// RegionChunk.rebuildClippingFlags(from): per tile, both landscape marks
/// taken from the source chunk, then every stored dynamic object flagged
/// again (LandscapeParser.flagScenery, not as landscape, storing all).
pub fn rebuild_clipping_flags(
    ctx: &mut impl landscape::Context,
    chunk: &HandleChunk,
    from: &HandleChunk,
) -> Result<()> {
    for x in 0..chunk::SIZE {
        for y in 0..chunk::SIZE {
            for projectile in [false, true] {
                let mark = {
                    let from = from.read();
                    let flags = if projectile {
                        &from.projectile_flags
                    } else {
                        &from.flags
                    };
                    landscape_mark(flags, x, y)?
                };
                let flags = if projectile {
                    chunk.read().projectile_flags.clone()
                } else {
                    chunk.read().flags.clone()
                };
                flags
                    .mark_landscape(x as i32, y as i32, mark)
                    .map_err(bind)?;
            }
            for i in 0..chunk::ARRAY_SIZE {
                let object = chunk.read().objects.read()[x][y][i].clone();
                if let Some(object) = object {
                    landscape::flag(ctx, chunk, x as i32, y as i32, &object, false, true)
                        .map_err(bind)?;
                }
            }
        }
    }
    Ok(())
}
fn landscape_mark(flags: &super::clipping::ChunkFlags, x: usize, y: usize) -> Result<bool> {
    let rows = flags.landscape.clone().ok_or(Error::Null)?;
    let row = rows.read().get(x).cloned().flatten().ok_or(Error::Null)?;
    let mark = row.read().get(y).copied().ok_or(Error::Null)?;
    Ok(mark)
}

/// RegionChunk.rotate(direction), given `Direction.toInteger()`: refused
/// with a log once rotated; otherwise both object layers are taken out (the
/// dynamic objects removed through SceneryBuilder), new layers made, the
/// rotation chosen (a diagonal falls back with a log) and every object put
/// back at its rotated tile: static ones into the first free slot, dynamic
/// ones added through SceneryBuilder.
pub fn rotate<A: Assets, S: Services<A>>(
    world: &mut World<'_, A, S>,
    host: &mut impl Build<A>,
    chunk: &HandleChunk,
    direction: i32,
) -> Result<()> {
    if chunk.read().rotation != 0 {
        return host.log_error(world.scene, "Region chunk was already rotated!");
    }
    type Layer = [[[Option<Scenery>; chunk::ARRAY_SIZE]; chunk::SIZE]; chunk::SIZE];
    let empty =
        || -> Layer { std::array::from_fn(|_| std::array::from_fn(|_| Default::default())) };
    let mut old_stat = empty();
    let mut old_dyn = empty();
    for x in 0..chunk::SIZE {
        for y in 0..chunk::SIZE {
            for i in 0..chunk::ARRAY_SIZE {
                old_stat[x][y][i] = chunk.read().static_objects.read()[x][y][i].clone();
                let object = chunk.read().objects.read()[x][y][i].clone();
                if let Some(object) = object {
                    old_dyn[x][y][i] = Some(object.clone());
                    scenery::remove(world, Some(&object)).map_err(bind)?;
                }
            }
        }
    }
    {
        let mut c = chunk.write();
        c.static_objects = Shared::new(empty());
        c.objects = Shared::new(empty());
    }
    let rotation = match direction {
        0..=3 => direction,
        other => {
            let rotation = (other + if other % 2 == 0 { 2 } else { 0 }) % 4;
            chunk.write().rotation = rotation;
            // The original logs from inside its switch, after the assignment.
            host.log_error(
                world.scene,
                &format!(
                    "Attempted to rotate a chunk in a non-cardinal direction ({other}) - using fallback rotation code. This should be investigated!"
                ),
            )?;
            rotation
        }
    };
    chunk.write().rotation = rotation;
    for x in 0..chunk::SIZE {
        for y in 0..chunk::SIZE {
            for i in 0..chunk::ARRAY_SIZE {
                if let Some(stat) = old_stat[x][y][i].clone() {
                    let object = rotated(world, &stat, x, y, rotation)?;
                    let [nx, ny] = object.1;
                    let statics = chunk.read().static_objects.clone();
                    let mut statics = statics.write();
                    let slots = &mut statics[nx][ny];
                    if let Some(slot) = slots.iter_mut().find(|slot| slot.is_none()) {
                        *slot = Some(object.0);
                    }
                }
                if let Some(dynamic) = old_dyn[x][y][i].clone() {
                    let (object, _) = rotated(world, &dynamic, x, y, rotation)?;
                    scenery::add(world, Some(&object), -1, None).map_err(bind)?;
                }
            }
        }
    }
    Ok(())
}
/// `obj.transform(id, (rotation + chunkRotation) % 4, location.transform(newX - x, newY - y, 0))`
/// at `getRotatedPosition` of the definition's (unrotated) size.
fn rotated<A: Assets, S: Services<A>>(
    world: &mut World<'_, A, S>,
    object: &Scenery,
    x: usize,
    y: usize,
    rotation: i32,
) -> Result<(Scenery, [usize; 2])> {
    let definition = scenery_node::definition(object).map_err(bind)?;
    let (size_x, size_y) = {
        let d = definition.read();
        (d.size_x, d.size_y)
    };
    let (id, shape, own, location) = {
        let o = object.read();
        (o.id, o.shape, o.rotation, o.location.clone())
    };
    let [nx, ny] = chunk::rotated_position(x as i32, y as i32, size_x, size_y, own, rotation);
    if !(0..chunk::SIZE as i32).contains(&nx) || !(0..chunk::SIZE as i32).contains(&ny) {
        return Err(Error::Binding("ArrayIndexOutOfBounds".into()));
    }
    let location = transform(&location, nx - x as i32, ny - y as i32, 0);
    let object = scenery::Context::construct(world, id, location, shape, (own + rotation) % 4)
        .map_err(bind)?;
    Ok((object, [nx as usize, ny as usize]))
}

/// The services of activating, deactivating and removing instances.
pub trait Host: region_activity::Host {
    /// The host type of the region zone list.
    type Zones: zones::Host;
    /// RESERVED_AREAS (the scene's list).
    fn reserved_areas(&mut self) -> Result<Reserved>;
    /// `MultiwayCombatZone.getInstance()`.
    fn multiway_zone(&mut self) -> Result<Zone<Self::Zones>>;
    /// RegionManager.removeRegion's map removal; the region removed.
    fn remove_region(&mut self, id: i32) -> Result<Option<region::Handle>>;
    /// `npc instanceof Familiar`.
    fn is_familiar(&mut self, npc: &NodeHandle) -> Result<bool>;
    /// flagInactive's familiar block: invisible, moved to its owner's tile
    /// (RegionManager.move, setLocation) and the strong recall script queued.
    fn recall_familiar(&mut self, npc: &NodeHandle) -> Result<()>;
    /// `npc.clear()`.
    fn clear_npc(&mut self, npc: &NodeHandle) -> Result<()>;
    /// `GroundItemManager.destroy(item)`.
    fn destroy_ground_item(&mut self, item: Option<&ground_item::Handle>) -> Result<()>;
}
/// The region class of a host that builds instances: DynamicRegion's
/// override is reached through Region's methods.
pub struct Dynamic;
impl<H: Host> Kind<H> for Dynamic {
    const DYNAMIC: bool = true;
    fn flag_inactive(host: &mut H, region: &region::Handle, force: bool) -> Result<bool> {
        flag_inactive(host, region, force)
    }
}
pub fn check_inactive<H: Host>(host: &mut H, region: &region::Handle) -> Result<bool> {
    region_activity::is_inactive_in::<Dynamic, H>(host, region, true)
}
pub fn is_inactive<H: Host>(host: &mut H, region: &region::Handle, run: bool) -> Result<bool> {
    region_activity::is_inactive_in::<Dynamic, H>(host, region, run)
}
pub fn flag_active<H: Host>(host: &mut H, region: &region::Handle) -> Result<()> {
    region_activity::flag_active_in::<Dynamic, H>(host, region)
}
pub fn unload<H: Host>(host: &mut H, region: &region::Handle, force: bool) -> Result<bool> {
    region_activity::unload_in::<Dynamic, H>(host, region, force)
}
/// The activity pulse's body: `flagInactive()`, the override for an instance.
pub fn pulse_body<H: Host>(host: &mut H, region: &region::Handle) -> Result<bool> {
    region_activity::pulse_body_in::<Dynamic, H>(host, region)
}
pub fn clear_chunk<H: Host>(host: &mut H, chunk: &HandleChunk) -> Result<()> {
    region_activity::clear_chunk_in::<Dynamic, H>(host, chunk)
}
/// RegionManager.removeRegion(id): out of the map, then `flagInactive(true)`.
pub fn remove_region<H: Host>(host: &mut H, id: i32) -> Result<()> {
    if let Some(region) = host.remove_region(id)? {
        <Dynamic as Kind<H>>::flag_inactive(host, &region, true)?;
    }
    Ok(())
}

/// `flagInactive(force)`: Region's for a plain region. An instance that is
/// not permanent defers to an active parent (checked for inactivity) and
/// waits for active linked regions; once Region's own flagInactive succeeds
/// its familiars are recalled, other NPCs cleared, ground items destroyed,
/// its area released, multiway combat switched off and every linked region
/// flagged inactive too. A permanent instance answers true untouched.
pub fn flag_inactive<H: Host>(host: &mut H, region: &region::Handle, force: bool) -> Result<bool> {
    let Some(fields) = instance(region) else {
        return region_activity::base_flag_inactive::<Dynamic, H>(host, region, force);
    };
    if fields.read().permanent {
        return Ok(true);
    }
    if let Some(parent) = parent(region)?
        && region_activity::is_active(&parent)
    {
        check_inactive(host, &parent)?;
        return Ok(false);
    }
    if let Some(linked) = linked(region)? {
        for r in &linked {
            if !is_inactive(host, r, false)? {
                return Ok(false);
            }
        }
    }
    if !region_activity::base_flag_inactive::<Dynamic, H>(host, region, force)? {
        return Ok(false);
    }
    for x in 0..CHUNKS {
        for y in 0..CHUNKS {
            for z in 0..PLANES {
                let chunk = chunk_at(region, x, y, z)?;
                // `new ArrayList<>(chunk.getNpcs())`.
                let npcs = chunk.read().npcs.values();
                for npc in npcs {
                    let npc = npc.ok_or(Error::Null)?;
                    if host.is_familiar(&npc)? {
                        host.recall_familiar(&npc)?;
                    } else {
                        host.clear_npc(&npc)?;
                    }
                }
                let items = chunk_sync::get_items(&chunk).read().snapshot();
                for item in items {
                    host.destroy_ground_item(item.as_ref())?;
                }
            }
        }
    }
    let reserved = host.reserved_areas()?;
    release_area(&reserved, fields.read().borders.clone().as_ref())?;
    if fields.read().multicombat {
        toggle_multicombat(host, region)?;
    }
    let mut all_linked_inactive = true;
    if let Some(linked) = linked(region)? {
        for r in &linked {
            all_linked_inactive &= flag_inactive(host, r, force)?;
        }
    }
    Ok(all_linked_inactive)
}

/// `link(regions)`: each becomes a child of this one, the list is kept and
/// this region is flagged active.
pub fn link<H: Host>(
    host: &mut H,
    region: &region::Handle,
    regions: &[region::Handle],
) -> Result<()> {
    for r in regions {
        fields(r)?.write().parent = Some(region.downgrade());
    }
    fields(region)?.write().linked = Some(regions.to_vec());
    flag_active(host, region)
}
/// `toggleMulticombat()`: on, a multiway RegionZone over the borders is
/// added; off, every RegionZone of the multiway zone is removed.
pub fn toggle_multicombat<H: Host>(host: &mut H, region: &region::Handle) -> Result<()> {
    let fields = fields(region)?;
    let zones = region_activity::region_zones::<H::Zones>(region)?;
    if fields.read().multicombat {
        let mut cursor = zones.iter();
        while cursor.has_next(&zones) {
            let zone = cursor.next(&zones)?;
            let multiway = host.multiway_zone()?;
            if zone.zone().same(&multiway) {
                cursor.remove(&zones)?;
            }
        }
        fields.write().multicombat = false;
        return Ok(());
    }
    let multiway = host.multiway_zone()?;
    let borders = fields.read().borders.clone().ok_or(Error::Null)?;
    zones.add(RegionZone::new(multiway, borders));
    fields.write().multicombat = true;
    Ok(())
}
/// `setMusicId(musicId)`: a music zone over the borders.
pub fn set_music_id(region: &region::Handle, music_id: i32) -> Result<()> {
    let borders = fields(region)?.read().borders.clone().ok_or(Error::Null)?;
    let list = region.read().activity.read().music_zones.clone();
    list.add(MusicZone::new(music_id, borders));
    Ok(())
}
/// `clear()`: every NPC of the public list cleared, then the list emptied.
pub fn clear<H: Host>(host: &mut H, region: &region::Handle) -> Result<()> {
    let list = npcs(region)?;
    let mut cursor = list.iter();
    while cursor.has_next(&list) {
        let npc = cursor.next(&list)?;
        host.clear_npc(&npc)?;
    }
    list.clear();
    Ok(())
}

impl From<landscape::Error> for Error {
    fn from(error: landscape::Error) -> Self {
        bind(error)
    }
}
