//! Scene streaming. Natively JS5 reads, decoding and geometry preparation run off
//! the render thread. Only the render thread installs meshes or touches the GPU.
//! A browser page has one thread: there the same preparation runs on it, when
//! the scene asks whether its request is ready (see `Stream::new`).
use super::{Geometry, NpcBody, Scene, render};
use macroquad::prelude::{Material, Texture2D, vec2};
#[cfg(not(target_arch = "wasm32"))]
use rs09_cache::store::StorageMode;
use rs09_cache::{Limits, archive::Archive, store::Store};
use rs09_client::{
    animation::{Base, Frame, Sequence},
    floor::{Overlay, Underlay},
    interaction::LocationType,
    model::Model,
    npc, on_demand,
    terrain::Region,
    walking::ClippingDefinition,
    world::{self, Location, Slot},
};
use rs09_protocol::{buffer::Reader, game::rebuild::NormalRebuild};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::Path,
    sync::{Arc, mpsc},
    time::Duration,
};

/// `std::time::Instant` has no clock inside a browser module; this reads the
/// page's, which counts in milliseconds.
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, PartialEq, PartialOrd)]
struct Instant(f64);
#[cfg(target_arch = "wasm32")]
impl Instant {
    fn now() -> Self {
        Self(macroquad::miniquad::date::now())
    }
    fn elapsed(&self) -> Duration {
        Duration::from_secs_f64((Self::now().0 - self.0).max(0.0))
    }
}
#[cfg(target_arch = "wasm32")]
impl std::ops::Add<Duration> for Instant {
    type Output = Self;
    fn add(self, duration: Duration) -> Self {
        Self(self.0 + duration.as_secs_f64())
    }
}

pub type Key = [u32; 4];
type File = Arc<Vec<u8>>;
type Tile = [i32; 2];

pub struct Cache {
    maps: Archive,
    locations: Archive,
    models: Archive,
    configs: Archive,
    sequences: Archive,
    frames: Archive,
    bases: Archive,
    npcs: Archive,
    spots: Archive,
    spawns: BTreeMap<Tile, BTreeSet<u32>>,
}

#[derive(serde::Deserialize)]
struct SpawnRow {
    npc_id: String,
    loc_data: String,
}

fn authored_types() -> Result<BTreeMap<Tile, BTreeSet<u32>>, String> {
    let rows: Vec<SpawnRow> =
        serde_json::from_str(include_str!("../../../fixtures/npc-spawns/npc_spawns.json"))
            .map_err(|e| e.to_string())?;
    let mut regions = BTreeMap::<Tile, BTreeSet<u32>>::new();
    for row in rows {
        let id = row.npc_id.parse::<u32>().map_err(|e| e.to_string())?;
        for coord in row.loc_data.split('-').filter(|r| !r.is_empty()) {
            let mut fields = coord.trim_matches(['{', '}']).split(',');
            let mut region = [0; 2];
            for axis in &mut region {
                *axis = fields
                    .next()
                    .ok_or("Missing NPC coordinate")?
                    .trim()
                    .parse::<i32>()
                    .map_err(|e| e.to_string())?
                    >> 6;
            }
            regions.entry(region).or_default().insert(id);
        }
    }
    Ok(regions)
}

impl Cache {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(store) = rs09_client::cache_live::store_for(path) {
            return Self::from_store(store);
        }
        let limits = Limits::default();
        let store = Store::open(path, StorageMode::File, limits).map_err(|e| e.to_string())?;
        Self::from_store(store)
    }
    /// The same over an opened store: the browser's, which is in memory.
    pub fn from_store(store: Store) -> Result<Self, String> {
        let limits = Limits::default();
        let archive =
            |id| Archive::open(store.clone(), id, None, limits).map_err(|e| e.to_string());
        Ok(Self {
            maps: archive(5)?,
            locations: archive(16)?,
            models: archive(7)?,
            configs: archive(2)?,
            npcs: archive(18)?,
            sequences: archive(20)?,
            frames: archive(0)?,
            bases: archive(1)?,
            spots: archive(21)?,
            spawns: authored_types()?,
        })
    }
    /// A group still being downloaded reads as absent; the build that met
    /// it is done again (`build_when_present`).
    fn read(archive: &Archive, group: u32, file: u32) -> Result<Option<File>, String> {
        on_demand::arriving(|| archive.file(group, file, [0; 4])).map_err(|e| e.to_string())
    }
    fn map(&self, name: String, key: Key) -> Result<Option<File>, String> {
        let Some(id) = self.maps.index.group_by_ascii_name(&name) else {
            return Ok(None);
        };
        on_demand::arriving(|| self.maps.file(id, 0, key)).map_err(|e| format!("{name}: {e}"))
    }
    /// Whether the map archive has a group of this name (`n39_44`, ...).
    pub fn has_map(&self, name: &str) -> bool {
        self.maps.index.group_by_ascii_name(name).is_some()
    }
    /// A region's NPC file (`n{x}_{y}`). The title screen's regions are the
    /// only ones that have one; the original reads it, like their other map
    /// files, with an all-zero key.
    pub fn title_spawns(&self, [x, y]: Tile) -> Result<Option<File>, String> {
        self.map(format!("n{x}_{y}"), [0; 4])
    }
    pub fn npc(&self, id: u32) -> Result<Option<npc::Definition>, String> {
        Self::read(&self.npcs, id >> 7, id & 127)?
            .map(|raw| npc::Definition::decode(&raw).map_err(|e| format!("npc/{id}: {e}")))
            .transpose()
    }
    /// `NpcTypeList.get`'s read, telling a file the archive lacks from one
    /// whose group has not arrived yet (`rs09_client::on_demand`).
    pub fn npc_file(&self, id: u32) -> Result<on_demand::Read<npc::Definition>, String> {
        use on_demand::Read;
        let read = on_demand::classify(self.npcs.file(id >> 7, id & 127, [0; 4]))
            .map_err(|e| format!("npc/{id}: {e}"))?;
        Ok(match read {
            Read::File(raw) => {
                Read::File(npc::Definition::decode(&raw).map_err(|e| format!("npc/{id}: {e}"))?)
            }
            Read::NoFile => Read::NoFile,
            Read::NotYet => Read::NotYet,
        })
    }
    /// Every spot animation file (archive 21, `SpotAnimTypeList`), by id.
    /// The archive is one of the config archives the start-up sequence
    /// holds whole before the title (`client.mainLoad` state 70).
    pub fn spot_files(&self) -> Result<Vec<(u32, File)>, String> {
        let mut files = Vec::new();
        for info in &self.spots.index.groups {
            let Some(group) = self
                .spots
                .group(info.id, [0; 4])
                .map_err(|e| format!("spot group {}: {e}", info.id))?
            else {
                continue;
            };
            for (file, data) in group.iter() {
                files.push(((info.id << 8) + file, Arc::new(data.to_vec())));
            }
        }
        Ok(files)
    }
    pub fn location(&self, id: u32) -> Result<Option<File>, String> {
        Self::read(&self.locations, id >> 8, id & 255)
    }
    /// `LocTypeList.get`'s read (`rs09_client::on_demand`).
    pub fn location_file(&self, id: u32) -> Result<on_demand::Read<File>, String> {
        on_demand::classify(self.locations.file(id >> 8, id & 255, [0; 4]))
            .map_err(|e| format!("loc/{id}: {e}"))
    }
    pub fn model(&self, id: u32) -> Result<Option<Model>, String> {
        Self::read(&self.models, id, 0)?
            .map(|bytes| Model::decode(&bytes).map_err(|e| format!("model/{id}: {e}")))
            .transpose()
    }
    pub fn bas(&self, id: i32) -> Result<Option<npc::Bas>, String> {
        if id == -1 {
            return Ok(Some(npc::Bas::decode(&[0]).map_err(|e| e.to_string())?));
        }
        Self::read(&self.configs, 32, id as u32)?
            .map(|bytes| npc::Bas::decode(&bytes).map_err(|e| format!("bas/{id}: {e}")))
            .transpose()
    }
    pub fn sequence(&self, id: u32) -> Result<Option<Sequence>, String> {
        Self::read(&self.sequences, id >> 7, id & 127)?
            .map(|bytes| Sequence::decode(&bytes).map_err(|e| format!("seq/{id}: {e}")))
            .transpose()
    }
}

pub fn placements(region: Tile, bytes: &[u8]) -> rs09_protocol::Result<Vec<[i32; 6]>> {
    let mut r = Reader::new(bytes);
    let mut objects = Vec::new();
    let mut id = -1i32;
    loop {
        let offset = r.g_var_smart()?;
        if offset == 0 {
            return Ok(objects);
        }
        id = id.checked_add(offset).ok_or(rs09_protocol::Error::new(
            r.position(),
            "landscape id overflow",
        ))?;
        let mut coordinate = 0;
        loop {
            let offset = r.gsmarts()?;
            if offset == 0 {
                break;
            }
            coordinate += offset - 1;
            if coordinate >= 4 * 64 * 64 {
                return Err(rs09_protocol::Error::new(
                    r.position(),
                    "invalid landscape coordinate",
                ));
            }
            let flags = r.g1()?;
            objects.push([
                id,
                region[0] * 64 + (coordinate >> 6 & 63),
                region[1] * 64 + (coordinate & 63),
                coordinate >> 12,
                flags >> 2,
                flags & 3,
            ]);
        }
    }
}

pub struct Prepared {
    pub center: Tile,
    pub regions: Vec<Region>,
    pub scene: Scene,
    pub slots: BTreeMap<Slot, Location>,
    pub lights: render::PreparedLights,
    pub geometry: Geometry,
    pub types: BTreeMap<u32, LocationType>,
    pub clips: BTreeMap<u32, ClippingDefinition>,
    pub models: BTreeMap<u32, Model>,
    pub sequences: BTreeMap<u32, Sequence>,
    pub frames: BTreeMap<u32, Frame>,
    pub bases: BTreeMap<u16, Base>,
    pub npc_bases: BTreeMap<i32, npc::Bas>,
    pub npc_bodies: BTreeMap<u32, NpcBody>,
    pub diagnostics: Vec<String>,
    pub shadows: std::collections::VecDeque<([usize; 2], Vec<u8>)>,
}

struct Request {
    center: Tile,
    keys: BTreeMap<Tile, Key>,
    origin: Tile,
    corrected_lights: bool,
    instance: Option<Arc<Instance>>,
    /// A title-screen scene: the only regions whose files are read.
    title: Option<Arc<TitleScene>>,
}

/// The title screen's scene as `LoginManager.setupLoadingScreenRegion` and
/// `loadRegion` set it up (`rs09_client::title_scene`).
pub struct TitleScene {
    /// `regionBitPacked` in the original's order, each with whether its files
    /// are read (it has an NPC file).
    pub regions: Vec<(Tile, bool)>,
    /// `Camera.originX/Y`.
    pub base: Tile,
    /// `SceneGraph.centralZoneX/Y`.
    pub zone: Tile,
}
impl TitleScene {
    fn listed(&self, region: Tile) -> bool {
        self.regions.contains(&(region, true))
    }
}

/// An instanced scene from REBUILD_REGION: the 4 x 13 x 13 chunk table and
/// the absolute tile of scene tile (0, 0).
struct Instance {
    chunks: Vec<i32>,
    base: Tile,
}
impl Instance {
    /// The regions the chunks are copied from.
    fn sources(&self) -> BTreeSet<Tile> {
        self.chunks
            .iter()
            .filter_map(|&packed| rs09_client::instance::Chunk::unpack(packed))
            .map(|chunk| {
                let id = chunk.region();
                [id >> 8, id & 255]
            })
            .collect()
    }
    /// The region of the scene's middle tile. One build covers the whole
    /// instanced scene, so it is requested under this fixed centre.
    fn center(&self) -> Tile {
        self.base.map(|v| (v + 52) >> 6)
    }
}

/// Terrain and locations of an instanced scene, at absolute coordinates.
fn build_instance(
    cache: &Cache,
    request: &Request,
    instance: &Instance,
    diagnostics: &mut Vec<String>,
) -> Result<(Vec<Region>, Vec<[i32; 6]>), String> {
    use rs09_client::instance::{Terrain, environments, locations, raw_tiles};
    let mut terrain = BTreeMap::new();
    let mut atmosphere = BTreeMap::new();
    let mut lights = BTreeMap::new();
    let mut placed = BTreeMap::new();
    for [x, z] in instance.sources() {
        let id = (x << 8) | z;
        if let Some(raw) = cache.map(format!("m{x}_{z}"), [0; 4])? {
            terrain.insert(id, raw_tiles(&raw).map_err(|e| format!("m{x}_{z}: {e}"))?);
            // The environment follows the tile stream of the same file.
            let region = Region::decode(x, z, &raw).map_err(|e| format!("m{x}_{z}: {e}"))?;
            atmosphere.insert(id, region.environment);
            lights.insert(id, region.point_lights);
        }
        let key = request.keys.get(&[x, z]).copied().unwrap_or([0; 4]);
        match cache.map(format!("l{x}_{z}"), key) {
            // Region (0, 0) keeps the placements region-local.
            Ok(Some(data)) => {
                placed.insert(
                    id,
                    placements([0, 0], &data).map_err(|e| format!("l{x}_{z}: {e}"))?,
                );
            }
            Ok(None) => (),
            Err(e) => diagnostics.push(e),
        }
    }
    let mut sizes = BTreeMap::new();
    let objects = locations(&instance.chunks, &placed, |id| {
        *sizes.entry(id).or_insert_with(|| {
            // A missing or undecodable type is the original's default 1 x 1.
            u32::try_from(id)
                .ok()
                .and_then(|id| cache.location(id).ok().flatten())
                .and_then(|raw| LocationType::decode_with_visual(&raw).ok())
                .map_or([1, 1], |(_, _, clip)| clip.size)
        })
    })
    .into_iter()
    .map(|[id, x, y, level, shape, rotation]| {
        [
            id,
            instance.base[0] + x,
            instance.base[1] + y,
            level,
            shape,
            rotation,
        ]
    })
    .collect();
    let mut regions = Terrain::compose(&instance.chunks, &terrain)
        .regions(instance.base, &environments(&instance.chunks, &atmosphere));
    // The scene's lights are kept on its first region, in the order the
    // original adds them, measured from that region's corner.
    if let Some(first) = regions.first_mut() {
        let offset = [
            instance.base[0] - first.x * 64,
            instance.base[1] - first.z * 64,
        ];
        match rs09_client::instance_lights::rebased(
            rs09_client::instance_lights::lights(&instance.chunks, &lights),
            offset,
        ) {
            Some(records) => first.point_lights = records,
            None => diagnostics.push("Instanced scene lights lie outside the scene".into()),
        }
    }
    Ok((regions, objects))
}

struct Assets {
    textures: render::Textures,
    underlays: Vec<Underlay>,
    overlays: Vec<Overlay>,
}

async fn build(cache: &Cache, request: Request, assets: &Assets) -> Result<Prepared, String> {
    let mut diagnostics = Vec::new();
    let mut regions = Vec::new();
    let mut objects = Vec::new();
    let mut region_ids = window(request.center);
    if let Some(instance) = &request.instance {
        (regions, objects) = build_instance(cache, &request, instance, &mut diagnostics)?;
        region_ids = regions.iter().map(|r| [r.x, r.z]).collect();
    }
    let mut without_terrain = BTreeSet::new();
    for &[x, z] in region_ids.iter().filter(|_| request.instance.is_none()) {
        rs09_client::slice::yield_point().await;
        // LoginManager.setupLoadingScreenRegion: a title scene reads none of
        // the files of a region without an NPC file (`title_window`).
        if request
            .title
            .as_ref()
            .is_some_and(|title| !title.listed([x, z]))
        {
            regions.push(rs09_client::title_window::unread(x, z));
            continue;
        }
        let raw = cache.map(format!("m{x}_{z}"), [0; 4])?;
        let blank = [0; 4 * 64 * 64];
        let terrain = raw.as_deref().map_or(blank.as_slice(), Vec::as_slice);
        if raw.is_none() && request.title.is_some() {
            regions.push(rs09_client::title_window::unread(x, z));
            without_terrain.insert([x, z]);
        } else {
            regions.push(Region::decode(x, z, terrain).map_err(|e| format!("m{x}_{z}: {e}"))?);
        }
        // The underwater map and its locations, neither encrypted.
        let (under, under_locs) = (
            cache.map(format!("um{x}_{z}"), [0; 4])?,
            cache.map(format!("ul{x}_{z}"), [0; 4])?,
        );
        regions.last_mut().unwrap().underwater = rs09_client::underwater::Source::decode(
            [x, z],
            under.as_deref().map(Vec::as_slice),
            under_locs.as_deref().map(Vec::as_slice),
        )
        .map_err(|e| format!("um{x}_{z}: {e}"))?;
        let key = request.keys.get(&[x, z]).copied().unwrap_or([0; 4]);
        match cache.map(format!("l{x}_{z}"), key) {
            Ok(Some(data)) => {
                objects.extend(placements([x, z], &data).map_err(|e| format!("l{x}_{z}: {e}"))?)
            }
            Ok(None) => (),
            Err(e) => diagnostics.push(e),
        }
    }
    // A title scene is the original's 104 by 104 tiles of these regions.
    let window = request.title.as_ref().map(|title| {
        let window = rs09_client::title_window::Window {
            base: title.base,
            zone: title.zone,
            unread: title
                .regions
                .iter()
                .filter(|&&(region, listed)| !listed || without_terrain.contains(&region))
                .map(|&(region, _)| region)
                .collect(),
        };
        window.clear_heights(&mut regions);
        objects.retain(|&[_, x, z, ..]| window.keeps_location(x, z));
        window
    });

    let mut definitions = BTreeMap::new();
    let mut types = BTreeMap::new();
    let mut clips = BTreeMap::new();
    let mut models = BTreeMap::new();
    let mut sequences = BTreeMap::new();
    let mut missing_models = BTreeSet::new();
    let mut location_ids: BTreeSet<_> = objects.iter().map(|row| row[0] as u32).collect();
    location_ids.extend(
        regions
            .iter()
            .filter_map(|region| region.underwater.layer())
            .flat_map(|layer| &layer.locations)
            .filter_map(|row| u32::try_from(row[0]).ok()),
    );
    while let Some(id) = location_ids.pop_first() {
        rs09_client::slice::yield_point().await;
        if types.contains_key(&id) {
            continue;
        }
        let Some(raw) = cache.location(id)? else {
            diagnostics.push(format!("Missing scenery definition {id}"));
            continue;
        };
        let (interaction, mut definition, clip) =
            LocationType::decode_with_visual(&raw).map_err(|e| format!("loc/{id}: {e}"))?;
        if let Some(children) = &interaction.children {
            location_ids.extend(children.iter().filter_map(|id| u32::try_from(*id).ok()));
        }
        let complete = add_models(cache, &mut models, &mut missing_models, &definition.models)?;
        if !complete {
            diagnostics.push(format!("Scenery {id} has a missing model"));
            definition.models.clear();
            definition.shapes = None;
        }
        if ![-1, 65535].contains(&definition.animation) {
            let id = definition.animation as u32;
            if let std::collections::btree_map::Entry::Vacant(entry) = sequences.entry(id) {
                if let Some(sequence) = cache.sequence(id)? {
                    entry.insert(sequence);
                }
            }
            if !sequences.contains_key(&id) {
                definition.animation = -1;
            }
        }
        definitions.insert(id.to_string(), definition);
        types.insert(id, interaction);
        clips.insert(id, clip);
    }
    objects.retain(|row| types.contains_key(&(row[0] as u32)));
    // An instanced scene is populated like the regions it was copied from.
    let spawn_regions = match &request.instance {
        Some(instance) => instance.sources().into_iter().collect(),
        None => region_ids.clone(),
    };
    let mut npc_ids: BTreeSet<_> = spawn_regions
        .iter()
        .filter_map(|r| cache.spawns.get(r))
        .flatten()
        .copied()
        .collect();
    // The NPCs a title scene places come from its regions' own files.
    let listed = request.title.iter().flat_map(|title| &title.regions);
    for &(region, _) in listed.filter(|&&(_, listed)| listed) {
        if let Some(data) = cache.title_spawns(region)? {
            npc_ids.extend(
                data.chunks_exact(4)
                    .map(|record| u32::from(u16::from_be_bytes([record[2], record[3]]))),
            );
        }
    }
    let mut visited = BTreeSet::new();
    let mut npc_bases = BTreeMap::new();
    let mut npc_bodies = BTreeMap::new();
    while let Some(id) = npc_ids.pop_first() {
        rs09_client::slice::yield_point().await;
        if !visited.insert(id) {
            continue;
        }
        let Some(raw) = Cache::read(&cache.npcs, id >> 7, id & 127)? else {
            continue;
        };
        let definition = npc::Definition::decode(&raw).map_err(|e| format!("npc/{id}: {e}"))?;
        if let Some(children) = &definition.children {
            npc_ids.extend(children.iter().filter_map(|id| u32::try_from(*id).ok()));
        }
        if let std::collections::btree_map::Entry::Vacant(entry) = npc_bases.entry(definition.bas) {
            if let Some(bas) = cache.bas(definition.bas)? {
                entry.insert(bas);
            }
        }
        if !add_models(cache, &mut models, &mut missing_models, &definition.models)?
            || definition.models.is_empty()
        {
            continue;
        }
        if let Some(bas) = npc_bases.get(&definition.bas) {
            if let Ok(mut model) = definition.assemble_with_bas(&models, &bas.movement) {
                model.draw_order =
                    rs09_client::surface::face_order(&model, &assets.textures.materials);
                model.surfaces = rs09_client::surface::prepare(&model, &assets.textures.materials);
                npc_bodies.insert(id, NpcBody { definition, model });
            }
        }
    }
    for id in npc_bases.values().flat_map(|bas| bas.movement.sequences()) {
        if let Ok(id) = u32::try_from(id) {
            if let std::collections::btree_map::Entry::Vacant(entry) = sequences.entry(id) {
                if let Some(sequence) = cache.sequence(id)? {
                    entry.insert(sequence);
                }
            }
        }
    }
    let mut frames = BTreeMap::new();
    let mut bases = BTreeMap::new();
    for &id in sequences.values().flat_map(|s| &s.frames) {
        if id == 65535 || frames.contains_key(&id) {
            continue;
        }
        rs09_client::slice::yield_point().await;
        let Some(bytes) = Cache::read(&cache.frames, id >> 16, id & 65535)? else {
            continue;
        };
        let base = u16::from_be_bytes(
            bytes
                .get(..2)
                .ok_or("Truncated scene frame")?
                .try_into()
                .unwrap(),
        );
        if let std::collections::btree_map::Entry::Vacant(entry) = bases.entry(base) {
            let Some(raw) = Cache::read(&cache.bases, u32::from(base), 0)? else {
                continue;
            };
            entry.insert(Base::decode(base, &raw).map_err(|e| e.to_string())?);
        }
        frames.insert(
            id,
            Frame::decode(&bytes, &bases[&base]).map_err(|e| e.to_string())?,
        );
    }

    let scene = Scene {
        name: String::new(),
        start: request.center.map(|r| r * 64 + 32),
        regions: region_ids,
        locations: objects,
        definitions,
        avatar_kits: vec![],
        npc_types: vec![],
        npc_spawns: vec![],
        window,
    };
    let mut geometry = Geometry::build_sliced(
        &scene,
        &regions,
        &models,
        &sequences,
        &assets.textures,
        &types,
        &assets.underlays,
        &assets.overlays,
    )
    .await?;
    // Map::updates, a tile at a time.
    let mut shadows = std::collections::VecDeque::new();
    for tile in 0..geometry.shadows.map.tiles() {
        rs09_client::slice::yield_point().await;
        shadows.extend(geometry.shadows.map.update(tile));
    }
    let slots = scene
        .locations
        .iter()
        .filter_map(|&[id, x, y, plane, shape, rotation]| {
            Some((
                Slot {
                    plane: plane as usize,
                    tile: [x, y],
                    layer: world::layer(shape as u8)?,
                },
                Location {
                    id,
                    shape: shape as u8,
                    rotation: rotation as u8,
                },
            ))
        })
        .collect();
    let lights =
        render::PreparedLights::new(&regions, request.origin, request.corrected_lights, |id| {
            let raw = Cache::read(&cache.configs, 31, id as u32)
                .map_err(|_| rs09_protocol::Error::new(0, "Could not read custom light"))?
                .ok_or(rs09_protocol::Error::new(0, "Missing custom light"))?;
            rs09_client::lighting::Flicker::decode(&raw)
        })?;
    Ok(Prepared {
        center: request.center,
        regions,
        scene,
        slots,
        lights,
        geometry,
        types,
        clips,
        models,
        sequences,
        frames,
        bases,
        npc_bases,
        npc_bodies,
        diagnostics,
        shadows,
    })
}

/// A build that met a group still being downloaded (`Store::live_recording`
/// has asked for it): it is built again a little later, as the original's
/// `LoginManager` waits for its map and model files.
pub const NOT_YET: &str = "the scene's files are still arriving";

/// [`build`], answering [`NOT_YET`] when it read any group that had not
/// arrived (its result would lack what is still on its way).
#[cfg(not(target_arch = "wasm32"))]
fn build_when_present(
    cache: &Cache,
    request: Request,
    assets: &Assets,
) -> Result<Prepared, String> {
    let before = rs09_cache::store::misses_on_this_thread();
    let result = rs09_client::slice::block_on(build(cache, request, assets));
    if rs09_cache::store::misses_on_this_thread() != before {
        return Err(NOT_YET.into());
    }
    result
}

/// How much of each frame the page gives a scene build (milliseconds). The
/// rest of the frame (the cycles with their keep-alives, the music feed,
/// the redraw) goes on as usual; a slice ends at the first yield point after
/// this much, so it may run over by one step of the build. `?slice=MS` on
/// the start page sets another; `?slice=0` builds each scene in the one frame
/// that asks for it, as before 2026-11-17 (for comparison).
#[cfg(target_arch = "wasm32")]
const SLICE_MS: f64 = 12.0;

#[cfg(target_arch = "wasm32")]
fn slice_ms() -> f64 {
    thread_local! {
        static CHOSEN: std::cell::OnceCell<f64> = const { std::cell::OnceCell::new() };
    }
    CHOSEN.with(|chosen| {
        *chosen.get_or_init(
            || match crate::cache_host::launch_value(8).trim().parse::<f64>() {
                Ok(ms) if ms == 0.0 => f64::INFINITY,
                Ok(ms) if ms > 0.0 => ms,
                _ => SLICE_MS,
            },
        )
    })
}

/// A digest of what a build prepared, for comparing a build spread over
/// frames with one done at once: every chunk's mesh fingerprint and pass,
/// the counts, the shadow tiles and the placed slots.
#[cfg(target_arch = "wasm32")]
fn digest(prepared: &Prepared) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let geometry = &prepared.geometry;
    for chunk in &geometry.chunks {
        (chunk.fingerprint, chunk.plane, chunk.blend, chunk.floor).hash(&mut hasher);
    }
    (
        geometry.faces,
        geometry.objects,
        geometry.pick_objects.len(),
    )
        .hash(&mut hasher);
    (
        geometry.animated_locations.len(),
        geometry.static_shadows.len(),
    )
        .hash(&mut hasher);
    prepared.shadows.hash(&mut hasher);
    for (slot, location) in &prepared.slots {
        (
            slot.plane,
            slot.tile,
            location.id,
            location.shape,
            location.rotation,
        )
            .hash(&mut hasher);
    }
    hasher.finish()
}

/// The page's clock in milliseconds.
#[cfg(target_arch = "wasm32")]
fn page_ms() -> f64 {
    macroquad::miniquad::date::now() * 1000.0
}

/// [`build_when_present`] a slice at a time: the build under way, and the
/// groups it has met that had not arrived. Only the misses of its own
/// slices count; the page reads the store between them too.
#[cfg(target_arch = "wasm32")]
struct Building {
    sliced: rs09_client::slice::Sliced<Result<Prepared, String>>,
    misses: u64,
}

#[cfg(target_arch = "wasm32")]
impl Building {
    fn new(cache: Arc<Cache>, request: Request, assets: std::rc::Rc<Assets>) -> Self {
        Self {
            sliced: rs09_client::slice::Sliced::new(async move {
                build(&cache, request, &assets).await
            }),
            misses: 0,
        }
    }

    /// Runs on until `deadline`; the result once the build has ended.
    fn run_until(&mut self, deadline: f64) -> Option<Result<Prepared, String>> {
        let before = rs09_cache::store::misses_on_this_thread();
        let done = self.sliced.run_until(deadline, page_ms);
        self.misses += rs09_cache::store::misses_on_this_thread().wrapping_sub(before);
        let result = done?;
        let sliced = &self.sliced;
        macroquad::logging::info!(
            "Scene build: {:.0} ms in {} frames, longest slice {:.0} ms, region {:?}, digest {:016x}",
            sliced.total_ms,
            sliced.slices,
            sliced.longest_ms,
            result.as_ref().map_or([-1; 2], |prepared| prepared.center),
            result.as_ref().map_or(0, digest)
        );
        Some(if self.misses != 0 {
            Err(NOT_YET.into())
        } else {
            result
        })
    }
}

fn add_models(
    cache: &Cache,
    models: &mut BTreeMap<u32, Model>,
    missing: &mut BTreeSet<u32>,
    ids: &[u32],
) -> Result<bool, String> {
    let mut complete = true;
    for &id in ids {
        if missing.contains(&id) {
            complete = false;
        } else if let std::collections::btree_map::Entry::Vacant(entry) = models.entry(id) {
            if let Some(model) = cache.model(id)? {
                entry.insert(model);
            } else {
                missing.insert(id);
                complete = false;
            }
        }
    }
    Ok(complete)
}

pub fn window(center: Tile) -> Vec<Tile> {
    ((center[0] - 1).max(0)..=(center[0] + 1).min(255))
        .flat_map(|x| ((center[1] - 1).max(0)..=(center[1] + 1).min(255)).map(move |y| [x, y]))
        .collect()
}

fn anticipated_region(tile: Tile, direction: Tile) -> Tile {
    std::array::from_fn(|axis| ((tile[axis] + direction[axis] * 24) >> 6).clamp(0, 255))
}

#[derive(serde::Deserialize)]
struct KeyRow {
    #[serde(rename = "regionId")]
    region_id: String,
    keys: String,
}
#[derive(serde::Deserialize)]
struct Keys {
    xteas: Vec<KeyRow>,
}

fn read_keys(path: &Path) -> Result<BTreeMap<Tile, Key>, String> {
    let raw = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let rows: Keys = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    let mut keys = BTreeMap::new();
    for row in rows.xteas {
        let id = row.region_id.parse::<i32>().map_err(|e| e.to_string())?;
        let key: Vec<u32> = row
            .keys
            .split(',')
            .map(|word| word.parse::<i32>().map(|v| v as u32))
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        keys.insert(
            [id >> 8, id & 255],
            key.try_into().map_err(|_| "Invalid map key length")?,
        );
    }
    Ok(keys)
}

pub struct Stream {
    /// Carries out the queued jobs where there is no worker thread.
    #[cfg(target_arch = "wasm32")]
    worker: Box<dyn FnMut()>,
    requests: mpsc::Sender<WorkerJob>,
    results: mpsc::Receiver<Result<Prepared, String>>,
    keys: BTreeMap<Tile, Key>,
    pending: Option<Tile>,
    instance: Option<Arc<Instance>>,
    title: Option<Arc<TitleScene>>,
    /// Counts scene replacements; a build requested under an older value is
    /// for a scene that no longer exists.
    generation: u64,
    pending_generation: u64,
}
enum WorkerJob {
    Build(Request),
    RetirePrepared(Box<Prepared>),
    RetireAssets(Box<AssetBatch>),
}

struct Staged {
    center: Tile,
    regions: Vec<Region>,
    slots: BTreeMap<Slot, Location>,
    lights: render::PointLights,
    chunks: Vec<super::Chunk>,
    warm: VecDeque<usize>,
    reusable: Vec<(usize, usize)>,
    faces: usize,
    objects: usize,
    shadows: render::Shadows,
    shadow_tiles: VecDeque<([usize; 2], Vec<u8>)>,
    animated_locations: Vec<super::AnimatedLocation>,
    static_shadows: Vec<super::ShadowCaster>,
    pick_objects: Vec<super::PickObject>,
    multilocs: Vec<(Slot, Location)>,
    assets: AssetBatch,
}

struct AssetBatch {
    types: BTreeMap<u32, LocationType>,
    clips: BTreeMap<u32, ClippingDefinition>,
    definitions: BTreeMap<String, rs09_client::location::Definition>,
    models: BTreeMap<u32, Model>,
    sequences: BTreeMap<u32, Sequence>,
    frames: BTreeMap<u32, Frame>,
    bases: BTreeMap<u16, Base>,
    npc_bases: BTreeMap<i32, npc::Bas>,
    npc_bodies: BTreeMap<u32, NpcBody>,
}
impl AssetBatch {
    fn deliver(&mut self, world: &mut super::Workbench) -> bool {
        let end = Instant::now() + Duration::from_micros(800);
        let mut remaining = 384;
        publish(
            &mut self.npc_bases,
            &mut world.npc_bases,
            end,
            &mut remaining,
        ) && publish(
            &mut self.sequences,
            &mut world.sequences,
            end,
            &mut remaining,
        ) && publish(
            &mut self.npc_bodies,
            &mut world.npc_types,
            end,
            &mut remaining,
        ) && publish(&mut self.bases, &mut world.bases, end, &mut remaining)
            && publish(&mut self.frames, &mut world.frames, end, &mut remaining)
            && publish(&mut self.models, &mut world.models, end, &mut remaining)
            && publish(
                &mut self.types,
                &mut world.location_types,
                end,
                &mut remaining,
            )
            && publish(&mut self.clips, &mut world.clips, end, &mut remaining)
            && publish(
                &mut self.definitions,
                &mut world.definitions,
                end,
                &mut remaining,
            )
    }
}
fn publish<K: Ord, V>(
    pending: &mut BTreeMap<K, V>,
    current: &mut BTreeMap<K, V>,
    end: Instant,
    remaining: &mut usize,
) -> bool {
    while *remaining != 0 && Instant::now() < end {
        let Some((id, asset)) = pending.pop_first() else {
            return true;
        };
        current.entry(id).or_insert(asset);
        *remaining -= 1;
    }
    pending.is_empty()
}

pub struct WorldMap {
    stream: Stream,
    active: Option<Tile>,
    staged: Option<Staged>,
    shadow_texture: Option<Texture2D>,
    light_textures: Option<[Texture2D; 2]>,
    retired: Vec<super::Chunk>,
    failed: Option<Tile>,
    /// A build met files still arriving: no new one before this time.
    not_yet_until: Option<Instant>,
    previous: Option<Tile>,
    direction: Tile,
    switches: u32,
    max_frame_ms: f64,
    last_switch_ms: f64,
    uncovered_frames: u32,
    travel_frames: u32,
    slow_frames: u32,
    crossings: Vec<serde_json::Value>,
    last_work: &'static str,
    last_work_ms: f64,
    slow_samples: Vec<serde_json::Value>,
}

impl WorldMap {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(path: &Path, keys: &Path, world: &mut super::Workbench) -> Result<Self, String> {
        Self::open_with(Cache::open(path)?, keys, world)
    }
    pub fn open_with(
        cache: Cache,
        keys: &Path,
        world: &mut super::Workbench,
    ) -> Result<Self, String> {
        let cache = Arc::new(cache);
        let (underlays, overlays) = super::scene_floors(&world.pack)?;
        let stream = Stream::new(cache.clone(), keys, &world.textures, underlays, overlays)?;
        // Spot animation types are read from the cache from now on.
        world.attach_spot_catalog(&cache)?;
        world.world_cache = Some(cache);
        let side = 192 * 16;
        // The two large shadow textures are allocated during login and reused.
        let shadow_texture = Some(Texture2D::from_rgba8(
            side,
            side,
            &vec![0; usize::from(side).pow(2) * 4],
        ));
        Ok(Self {
            stream,
            active: None,
            staged: None,
            shadow_texture,
            light_textures: Some(render::PointLights::spare_textures()),
            retired: vec![],
            previous: None,
            direction: [0; 2],
            failed: None,
            not_yet_until: None,
            switches: 0,
            max_frame_ms: 0.0,
            last_switch_ms: 0.0,
            uncovered_frames: 0,
            travel_frames: 0,
            slow_frames: 0,
            crossings: vec![],
            last_work: "idle",
            last_work_ms: 0.0,
            slow_samples: vec![],
        })
    }

    pub fn rebuild(&mut self, rebuild: &NormalRebuild) -> Result<(), String> {
        if self.stream.apply_rebuild(rebuild)? {
            // Leaving an instance: the scene around the player is rebuilt even
            // though the player's region has not changed.
            self.restart();
        }
        Ok(())
    }
    /// The title screen's scene (`LoginManager.setupLoadingScreenRegion`):
    /// from now on only its listed regions are read, without keys, until a
    /// rebuild from the server replaces the scene.
    pub fn title(&mut self, title: TitleScene) {
        self.stream.instance = None;
        self.stream.title = Some(Arc::new(title));
        self.stream.generation += 1;
        self.restart();
    }
    /// The scene installed is the title scene asked for last, built around
    /// the region `center`.
    pub fn title_installed(&self, center: Tile) -> bool {
        self.stream.title.is_some() && self.active == Some(center) && self.staged.is_none()
    }
    /// The build around the region `center` failed and is not asked for
    /// again.
    pub fn build_failed(&self, center: Tile) -> bool {
        self.failed == Some(center)
    }
    /// REBUILD_REGION: every instanced rebuild replaces the scene.
    pub fn rebuild_instance(
        &mut self,
        rebuild: &rs09_protocol::game::events::InstanceRebuild,
    ) -> Result<(), String> {
        self.stream.apply_instance(rebuild)?;
        self.restart();
        Ok(())
    }
    /// `ScriptRunner.refreshGameStateAfterSettingsChange`: the scene is built
    /// again from the same maps, with the preferences as they are now. A
    /// build already under way was started under the old ones and is dropped.
    pub fn refresh(&mut self) {
        self.stream.generation += 1;
        self.restart();
    }
    fn restart(&mut self) {
        self.active = None;
        self.failed = None;
        if let Some(staged) = self.staged.take() {
            self.shadow_texture = Some(staged.shadows.texture);
            self.light_textures = Some(staged.lights.into_textures());
            self.retired.extend(staged.chunks);
            let _ = self
                .stream
                .requests
                .send(WorkerJob::RetireAssets(Box::new(staged.assets)));
        }
    }

    pub fn metrics(&self) -> serde_json::Value {
        serde_json::json!({
            "active": self.active, "switches": self.switches, "peak_frame_ms": self.max_frame_ms,
            "last_switch_ms": self.last_switch_ms, "uncovered_frames": self.uncovered_frames,
            "travel_frames": self.travel_frames, "frames_over_33ms": self.slow_frames,
            "crossings": self.crossings,
            "slow_samples": self.slow_samples,
            "prefetch": self.stream.pending,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        world: &mut super::Workbench,
        tile: Tile,
        frame_time: f64,
        renderer: Option<&super::static_render::Renderer>,
        materials: [&Material; 2],
        corrected_lights: bool,
        ui: Option<&mut rs09_client::ui::state::State>,
        diagnostics: &mut Vec<String>,
    ) -> Result<(), String> {
        let here = self.stream.region(tile);
        if self.active.is_some() {
            self.max_frame_ms = self.max_frame_ms.max(frame_time * 1000.0);
            self.travel_frames = self.travel_frames.saturating_add(1);
            if frame_time > 1.0 / 30.0 {
                self.slow_frames = self.slow_frames.saturating_add(1);
                if self.slow_samples.len() < 64 {
                    self.slow_samples.push(serde_json::json!({
                        "tile": tile, "frame_ms": frame_time * 1000.0, "stream_action": self.last_work, "stream_cpu_ms": self.last_work_ms,
                    }));
                }
            }
        }
        self.last_work = "idle";
        let pacing_work = Instant::now();
        let covered = world.regions.iter().any(|r| [r.x, r.z] == here);
        if !covered {
            self.uncovered_frames = self.uncovered_frames.saturating_add(1);
        }
        let previous = self.previous.replace(tile);
        if let Some(last) = previous.filter(|last| last.map(|v| v >> 6) != here) {
            if self.crossings.len() < 128 {
                self.crossings.push(serde_json::json!({
                    "from": last, "to": tile, "covered": covered, "frame_ms": frame_time * 1000.0,
                }));
            }
        }
        if let Some(old) = previous.filter(|&old| old != tile) {
            self.direction = std::array::from_fn(|i| (tile[i] - old[i]).signum());
        }
        let anticipated = if self.stream.instance.is_some() || self.stream.title.is_some() {
            here
        } else {
            anticipated_region(tile, self.direction)
        };
        let target = if self.active != Some(here) {
            here
        } else {
            anticipated
        };
        if let Some(prepared) = self.stream.poll() {
            match prepared {
                Ok(data) => {
                    if (data.center == target || data.center == here) && self.staged.is_none() {
                        self.staged = Some(self.stage(world, data, tile, diagnostics));
                        self.last_work = "stage";
                    } else {
                        let _ = self
                            .stream
                            .requests
                            .send(WorkerJob::RetirePrepared(Box::new(data)));
                        self.last_work = "discard";
                    }
                }
                // Asked for again shortly; the groups it met are on their way.
                Err(error) if error == NOT_YET => {
                    self.not_yet_until = Some(Instant::now() + Duration::from_millis(250));
                }
                Err(error) => {
                    self.failed = Some(target);
                    return Err(error);
                }
            }
        }
        if let Some(staged) = &mut self.staged {
            let covered = world.regions.iter().any(|r| [r.x, r.z] == here);
            let ready_to_switch = staged.center == here
                || (!covered && staged.regions.iter().any(|r| [r.x, r.z] == here));
            let stale = self.active.is_some()
                && !ready_to_switch
                && staged.center != target
                && self.active != Some(target);
            if stale {
                let staged = self.staged.take().unwrap();
                self.shadow_texture = Some(staged.shadows.texture);
                self.light_textures = Some(staged.lights.into_textures());
                self.retired.extend(staged.chunks);
                let _ = self
                    .stream
                    .requests
                    .send(WorkerJob::RetireAssets(Box::new(staged.assets)));
                self.last_work = "discard";
            } else {
                let assets_ready = staged.assets.deliver(world);
                let start = Instant::now();
                let budget = Duration::from_micros(1200);
                let bytes_per_frame = 1_500_000;
                let mut submitted = 0;
                let mut operations = 0;
                while operations < 8 && submitted < bytes_per_frame && start.elapsed() < budget {
                    if let Some((offset, bytes)) = staged.shadow_tiles.pop_front() {
                        submitted += bytes.len();
                        staged.shadows.upload_tile(offset, bytes);
                    }
                    if let Some(index) = staged.warm.pop_front() {
                        if let Some(renderer) = renderer {
                            let chunk = &mut staged.chunks[index];
                            let bytes = mesh_bytes(chunk);
                            if submitted + bytes > bytes_per_frame && operations != 0 {
                                staged.warm.push_front(index);
                                break;
                            }
                            renderer.warm(chunk);
                            submitted += bytes;
                        }
                    } else if staged.shadow_tiles.is_empty() {
                        break;
                    }
                    operations += 1;
                }
                if submitted > 0 && self.last_work == "idle" {
                    self.last_work = "upload";
                }
                if ready_to_switch
                    && assets_ready
                    && staged.warm.is_empty()
                    && staged.shadow_tiles.is_empty()
                {
                    let staged = self.staged.take().unwrap();
                    let start = Instant::now();
                    self.activate(world, staged, materials, corrected_lights, ui)?;
                    self.last_switch_ms = start.elapsed().as_secs_f64() * 1000.0;
                    self.last_work = "install";
                }
            }
        }
        let mut retired_bytes = 0;
        for _ in 0..8 {
            let Some(chunk) = self.retired.pop() else {
                break;
            };
            retired_bytes += mesh_bytes(&chunk);
            if retired_bytes >= 1_500_000 {
                break;
            }
        }
        if self.staged.is_none()
            && self.active != Some(target)
            && self.failed != Some(target)
            && self
                .not_yet_until
                .is_none_or(|until| Instant::now() >= until)
        {
            self.not_yet_until = None;
            self.stream
                .request(target, world.navigation.origin, corrected_lights);
        }
        self.last_work_ms = pacing_work.elapsed().as_secs_f64() * 1000.0;
        Ok(())
    }

    fn stage(
        &mut self,
        world: &mut super::Workbench,
        mut data: Prepared,
        tile: Tile,
        diagnostics: &mut Vec<String>,
    ) -> Staged {
        for message in data.diagnostics.drain(..) {
            if diagnostics.len() < 32 && !diagnostics.contains(&message) {
                diagnostics.push(message);
            }
        }
        let assets = AssetBatch {
            types: data.types,
            clips: data.clips,
            definitions: data.scene.definitions,
            models: data.models,
            sequences: data.sequences,
            frames: data.frames,
            bases: data.bases,
            npc_bases: data.npc_bases,
            npc_bodies: data.npc_bodies,
        };
        let geometry = data.geometry;
        let chunks: Vec<_> = geometry
            .chunks
            .into_iter()
            .map(super::PreparedChunk::activate)
            .collect();
        let center = vec2(tile[0] as f32, tile[1] as f32);
        let mut resident: BTreeMap<_, _> = world
            .chunks
            .iter()
            .enumerate()
            .filter_map(|(index, chunk)| {
                chunk.gpu.as_ref()?;
                Some((mesh_key(chunk)?, index))
            })
            .collect();
        let mut reusable = Vec::new();
        let mut warm: Vec<_> = chunks
            .iter()
            .enumerate()
            .filter(|(_, chunk)| chunk.plane == 0 || chunk.center.distance(center) < 72.0)
            .filter_map(|(index, chunk)| {
                if let Some(old) = mesh_key(chunk).and_then(|key| resident.remove(&key)) {
                    reusable.push((index, old));
                    None
                } else {
                    Some((chunk.center.distance_squared(center) as u32, index))
                }
            })
            .collect();
        warm.sort_unstable();
        Staged {
            center: data.center,
            regions: data.regions,
            slots: data.slots,
            lights: render::PointLights::with_textures(data.lights, self.light_textures.take()),
            chunks,
            warm: warm.into_iter().map(|(_, index)| index).collect(),
            reusable,
            faces: geometry.faces,
            objects: geometry.objects,
            shadows: render::Shadows::with_texture(geometry.shadows, self.shadow_texture.take()),
            shadow_tiles: data.shadows,
            animated_locations: geometry.animated_locations,
            static_shadows: geometry.static_shadows,
            pick_objects: geometry.pick_objects,
            multilocs: geometry.multilocs,
            assets,
        }
    }

    fn activate(
        &mut self,
        world: &mut super::Workbench,
        mut staged: Staged,
        materials: [&Material; 2],
        corrected_lights: bool,
        ui: Option<&mut rs09_client::ui::state::State>,
    ) -> Result<(), String> {
        if corrected_lights != staged.lights.corrected {
            staged.lights.place(&staged.regions, corrected_lights);
        }
        let inside = |slot: &Slot| {
            staged
                .regions
                .iter()
                .any(|r| r.x == slot.tile[0] >> 6 && r.z == slot.tile[1] >> 6)
        };
        let mut old_clocks: BTreeMap<_, _> = world
            .animated_locations
            .iter()
            .map(|loc| {
                (
                    (
                        loc.target.plane,
                        loc.target.tile,
                        loc.target.shape,
                        loc.target.id,
                        loc.sequence,
                        // The two entities of a corner wall or a double
                        // decoration differ in their rotation.
                        loc.rotation,
                    ),
                    &loc.clock,
                )
            })
            .collect();
        for loc in &mut staged.animated_locations {
            if let Some(clock) = old_clocks.remove(&(
                loc.target.plane,
                loc.target.tile,
                loc.target.shape,
                loc.target.id,
                loc.sequence,
                loc.rotation,
            )) {
                loc.clock = clock.clone();
            }
        }
        world.world.changes.retain(|change| inside(&change.slot));
        let retained: Vec<_> = world
            .world
            .changes
            .iter()
            .filter(|change| change.set_loops == -1)
            .map(|change| (change.slot, change.replacement))
            .collect();
        let mut slots = staged.slots;
        for &(slot, replacement) in &retained {
            if replacement.id < 0 {
                slots.remove(&slot);
            } else {
                slots.insert(slot, replacement);
            }
        }
        #[cfg(target_os = "windows")]
        {
            let covered = |slot: &Slot, regions: &[Region]| {
                regions
                    .iter()
                    .any(|region| region.x == slot.tile[0] >> 6 && region.z == slot.tile[1] >> 6)
            };
            let audible = |id: i32| {
                world.location_types.get(&(id as u32)).is_some_and(|kind| {
                    kind.has_area_sound(|child| {
                        world
                            .location_types
                            .get(&(child as u32))
                            .is_some_and(|c| c.ambient.audible())
                    })
                })
            };
            world.audio_changes.extend(
                world
                    .world
                    .locations
                    .iter()
                    .filter(|(slot, loc)| !covered(slot, &staged.regions) && audible(loc.id))
                    .map(|(&slot, _)| {
                        (
                            slot,
                            Location {
                                id: -1,
                                shape: 0,
                                rotation: 0,
                            },
                        )
                    }),
            );
            world.audio_changes.extend(
                slots
                    .iter()
                    .filter(|(slot, loc)| !covered(slot, &world.regions) && audible(loc.id))
                    .map(|(&slot, &loc)| (slot, loc)),
            );
        }
        world.world.locations = slots;
        for (new, old) in staged.reusable {
            let target = &mut staged.chunks[new];
            let previous = &mut world.chunks[old];
            if mesh_key(target) == mesh_key(previous) {
                target.gpu = previous.gpu.take();
            }
        }
        self.retired
            .extend(std::mem::replace(&mut world.chunks, staged.chunks));
        self.shadow_texture = Some(std::mem::replace(&mut world.shadows, staged.shadows).texture);
        world.shadow_roofs = true;
        world.regions = staged.regions;
        world.animated_locations = staged.animated_locations;
        world.static_shadows = staged.static_shadows;
        world.pick_objects = staged.pick_objects;
        world.multilocs = staged
            .multilocs
            .into_iter()
            .map(|(slot, loc)| (slot, super::MultiLoc::new(loc, None)))
            .collect();
        self.light_textures =
            Some(std::mem::replace(&mut world.point_lights, staged.lights).into_textures());
        world.objects = staged.objects;
        world.faces = staged.faces;
        for (slot, replacement) in retained {
            world.dynamic_location(slot, replacement, None)?;
        }
        world.rebuild_navigation();
        world.map_revision = world.map_revision.wrapping_add(1);
        world.scene_revision = world.scene_revision.wrapping_add(1);
        world.refresh_ground(ui)?;
        super::bind_scene_materials(world, materials);
        self.active = Some(staged.center);
        self.switches += 1;
        Ok(())
    }
}

fn mesh_key(chunk: &super::Chunk) -> Option<([i32; 2], usize, u64)> {
    Some((
        [chunk.center.x as i32, chunk.center.y as i32],
        chunk.plane,
        chunk.fingerprint?,
    ))
}
fn mesh_bytes(chunk: &super::Chunk) -> usize {
    std::mem::size_of_val(chunk.mesh.vertices.as_slice())
        + std::mem::size_of_val(chunk.mesh.indices.as_slice())
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use rs09_protocol::buffer::Writer;

    #[test]
    fn landscape_deltas_preserve_positions_planes_and_rotation() {
        let mut bytes = Writer::default();
        bytes.p2(65_535); // An extended object id delta.
        bytes.p1(5);
        bytes.p1(66); // Plane 0, local x=1,z=1, plus one.
        bytes.p1(42);
        bytes.p2(32_768 + 4097); // The same tile on plane 1.
        bytes.p1(89);
        bytes.p1(0);
        bytes.p1(0);
        assert_eq!(
            placements([45, 54], &bytes.into_vec()).unwrap(),
            [[32771, 2881, 3457, 0, 10, 2], [32771, 2881, 3457, 1, 22, 1]]
        );
    }

    /// With every chunk copied unrotated from its own position, the instanced
    /// path must give exactly what the normal region decoder gives inside the
    /// scene. This uses the pinned cache and recorded keys; it is skipped when
    /// either is absent.
    #[test]
    fn an_identity_instance_reproduces_the_normal_scene() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (Ok(cache), Ok(keys)) = (
            Cache::open(&root.join("assets/cache")),
            read_keys(&root.join("runs/player-session/xteas.json")),
        ) else {
            eprintln!("skipped: no cache or map keys");
            return;
        };
        // Lumbridge; the base (3168, 3168) is not region aligned.
        let zone = [402, 402];
        let base = zone.map(|z| (z - 6) * 8);
        let mut chunks = vec![];
        for level in 0..4 {
            for x in 0..13 {
                for y in 0..13 {
                    chunks
                        .push((level << 24) | ((zone[0] - 6 + x) << 14) | ((zone[1] - 6 + y) << 3));
                }
            }
        }
        let instance = Instance { chunks, base };
        assert_eq!(instance.center(), [50, 50]);
        let sources = instance.sources();
        assert_eq!(sources.len(), 9);
        let request = Request {
            center: instance.center(),
            keys: keys.clone(),
            origin: base,
            corrected_lights: false,
            instance: None,
            title: None,
        };
        let mut diagnostics = vec![];
        let (regions, mut objects) =
            build_instance(&cache, &request, &instance, &mut diagnostics).unwrap();
        assert_eq!(diagnostics, Vec::<String>::new());
        assert_eq!(regions.len(), 9);

        let inside =
            |x: i32, y: i32| (0..104).contains(&(x - base[0])) && (0..104).contains(&(y - base[1]));
        let mut expected = vec![];
        let (mut tiles, mut derived) = (0, 0);
        for region in &regions {
            let (x, z) = (region.x, region.z);
            let raw = cache.map(format!("m{x}_{z}"), [0; 4]).unwrap().unwrap();
            let normal = Region::decode(x, z, &raw).unwrap();
            let commands = rs09_client::instance::raw_tiles(&raw).unwrap();
            for plane in 0..4 {
                for lx in 0..64 {
                    for lz in 0..64 {
                        let (ax, ay) = (x * 64 + lx as i32, z * 64 + lz as i32);
                        let got = region.tile(plane, lx, lz);
                        if inside(ax, ay) {
                            assert_eq!(region.environment_at(lx, lz), normal.environment);
                            tiles += 1;
                            let mut want = normal.tile(plane, lx, lz);
                            // The one intended difference: ground without a
                            // stored height takes its noise at the scene tile
                            // in an instance (`readTile(0, 0, ...)`) and at the
                            // absolute tile otherwise. Upper levels are
                            // relative, so the whole column shifts with it.
                            if commands[lx * 64 + lz].height
                                == rs09_client::instance::RawHeight::Derived
                            {
                                derived += usize::from(plane == 0);
                                want.height += -rs09_client::terrain::tile_height(
                                    ay - base[1] + 556_238,
                                    ax - base[0] + 932_731,
                                ) * 8
                                    - normal.tile(0, lx, lz).height;
                            }
                            assert_eq!(
                                format!("{got:?}"),
                                format!("{want:?}"),
                                "tile {ax},{ay} plane {plane}"
                            );
                        } else {
                            assert_eq!(
                                format!("{got:?}"),
                                format!("{:?}", rs09_client::terrain::Tile::default())
                            );
                        }
                    }
                }
            }
            // A region absent from the recorded keys is unencrypted or empty.
            let key = keys.get(&[x, z]).copied().unwrap_or([0; 4]);
            let Some(data) = cache.map(format!("l{x}_{z}"), key).unwrap() else {
                continue;
            };
            // The instanced path drops locations on the scene's outer ring.
            expected.extend(placements([x, z], &data).unwrap().into_iter().filter(
                |&[_, ax, ay, ..]| {
                    (1..103).contains(&(ax - base[0])) && (1..103).contains(&(ay - base[1]))
                },
            ));
        }
        assert_eq!(tiles, 4 * 104 * 104);
        // Most of Lumbridge stores its heights; some ground is procedural.
        assert!(derived > 0 && derived < 104 * 104, "{derived}");
        assert!(expected.len() > 1000, "{}", expected.len());
        expected.sort_unstable();
        objects.sort_unstable();
        assert_eq!(objects, expected);
    }

    #[test]
    fn running_preloads_before_a_region_boundary_and_keeps_overlap() {
        let next = anticipated_region([3193, 3271], [1, 0]);
        assert_eq!(next, [50, 51]);
        let old = window([49, 51]);
        let new = window(next);
        assert_eq!(old.iter().filter(|r| new.contains(r)).count(), 6);
        assert!(old.contains(&[50, 51]));
        assert_eq!(anticipated_region([4, 4], [-1, -1]), [0, 0]);
        assert_eq!(window([0, 0]).len(), 4);
    }
}

impl Stream {
    pub fn new(
        cache: Arc<Cache>,
        keys: &Path,
        textures: &render::Textures,
        underlays: Vec<Underlay>,
        overlays: Vec<Overlay>,
    ) -> Result<Self, String> {
        // Without a key file the server's rebuild messages supply the keys.
        let keys = if keys.exists() {
            read_keys(keys)?
        } else {
            BTreeMap::new()
        };
        let assets = Assets {
            textures: textures.worker_copy(),
            underlays,
            overlays,
        };
        let (requests, jobs) = mpsc::channel();
        let (output, results) = mpsc::channel();
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::Builder::new()
            .name("map-stream".into())
            .spawn(move || {
                while let Ok(job) = jobs.recv() {
                    match job {
                        WorkerJob::Build(request) => {
                            if output
                                .send(build_when_present(&cache, request, &assets))
                                .is_err()
                            {
                                break;
                            }
                        }
                        WorkerJob::RetirePrepared(data) => drop(data),
                        WorkerJob::RetireAssets(assets) => drop(assets),
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        // A page has no second thread. The same jobs are carried out when the
        // scene polls for its result, once a frame, a slice of at most about
        // SLICE_MS at a time (`rs09_client::slice`), so a build no longer
        // holds a frame up for seconds. (The original builds its scene in one
        // blocking call on the client's thread and pings the server from
        // inside it; here the frames, and with them the cycles, go on.)
        #[cfg(target_arch = "wasm32")]
        let worker: Box<dyn FnMut()> = {
            let assets = std::rc::Rc::new(assets);
            let mut building: Option<Building> = None;
            Box::new(move || {
                let deadline = page_ms() + slice_ms();
                loop {
                    if building.is_none() {
                        match jobs.try_recv() {
                            Ok(WorkerJob::Build(request)) => {
                                building =
                                    Some(Building::new(cache.clone(), request, assets.clone()));
                            }
                            Ok(WorkerJob::RetirePrepared(data)) => drop(data),
                            Ok(WorkerJob::RetireAssets(assets)) => drop(assets),
                            Err(_) => break,
                        }
                    }
                    let Some(build) = &mut building else {
                        continue;
                    };
                    let Some(result) = build.run_until(deadline) else {
                        break;
                    };
                    building = None;
                    if output.send(result).is_err() || page_ms() >= deadline {
                        break;
                    }
                }
            })
        };
        Ok(Self {
            #[cfg(target_arch = "wasm32")]
            worker,
            requests,
            results,
            keys,
            pending: None,
            instance: None,
            title: None,
            generation: 0,
            pending_generation: 0,
        })
    }

    /// REBUILD_REGION: remember the chunk table and the keys of the regions
    /// the chunks are copied from. Keys follow the first-seen region order.
    pub fn apply_instance(
        &mut self,
        rebuild: &rs09_protocol::game::events::InstanceRebuild,
    ) -> Result<(), String> {
        let ids = rebuild.regions();
        if ids.len() != rebuild.keys.len() {
            return Err("Instanced scene region/key count mismatch".into());
        }
        for (id, key) in ids.into_iter().zip(&rebuild.keys) {
            self.keys
                .insert([id >> 8, id & 255], key.map(|value| value as u32));
        }
        self.title = None;
        self.instance = Some(Arc::new(Instance {
            chunks: rebuild.chunks.clone(),
            base: rebuild.base(),
        }));
        self.generation += 1;
        Ok(())
    }

    /// Returns whether an instanced scene was in effect before this rebuild.
    pub fn apply_rebuild(&mut self, rebuild: &NormalRebuild) -> Result<bool, String> {
        // Leaving the title screen's scene is leaving a scene of another
        // kind too: what was built or asked for under it is void.
        let was_instanced = self.instance.take().is_some() | self.title.take().is_some();
        if was_instanced {
            self.generation += 1;
        }
        self.apply_normal(rebuild).map(|()| was_instanced)
    }

    fn apply_normal(&mut self, rebuild: &NormalRebuild) -> Result<(), String> {
        let ids = rebuild.regions();
        if ids.len() != rebuild.keys.len() {
            return Err("Scene region/key count mismatch".into());
        }
        for (id, key) in ids.into_iter().zip(&rebuild.keys) {
            self.keys
                .insert([id >> 8, id & 255], key.map(|value| value as u32));
        }
        Ok(())
    }

    pub fn request(&mut self, center: Tile, origin: Tile, corrected_lights: bool) {
        if self.pending.is_some() || center.iter().any(|&v| !(0..=255).contains(&v)) {
            return;
        }
        // An instanced scene reads its source regions, wherever they are.
        let wanted: Vec<Tile> = match &self.instance {
            Some(instance) => instance.sources().into_iter().collect(),
            None => window(center),
        };
        // The title scene's files have no keys (`regionsXteaKeys` is zero).
        let keys = wanted
            .into_iter()
            .filter(|_| self.title.is_none())
            .filter_map(|id| self.keys.get(&id).map(|key| (id, *key)))
            .collect();
        if self
            .requests
            .send(WorkerJob::Build(Request {
                center,
                keys,
                origin,
                corrected_lights,
                instance: self.instance.clone(),
                title: self.title.clone(),
            }))
            .is_ok()
        {
            self.pending = Some(center);
            self.pending_generation = self.generation;
        }
    }

    pub fn poll(&mut self) -> Option<Result<Prepared, String>> {
        #[cfg(target_arch = "wasm32")]
        (self.worker)();
        let result = self.results.try_recv().ok()?;
        self.pending = None;
        if self.pending_generation != self.generation {
            // Built for a scene an instanced rebuild has since replaced.
            if let Ok(data) = result {
                let _ = self
                    .requests
                    .send(WorkerJob::RetirePrepared(Box::new(data)));
            }
            return None;
        }
        Some(result)
    }

    /// The region a build is requested and activated under for a player on
    /// `tile`: the player's own region, or the fixed centre of an instance.
    fn region(&self, tile: Tile) -> Tile {
        match &self.instance {
            Some(instance) => instance.center(),
            None => tile.map(|tile| tile >> 6),
        }
    }
}

/// Drives real walk and scenery clicks for a live rendering regression capture.
#[cfg(not(target_arch = "wasm32"))]
pub struct TravelCapture {
    route: VecDeque<serde_json::Value>,
    pending: Option<(Option<Tile>, u64)>,
}
#[cfg(not(target_arch = "wasm32"))]
impl TravelCapture {
    pub fn open(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let route = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        Ok(Self {
            route,
            pending: None,
        })
    }
    pub fn finished(&self) -> bool {
        self.pending.is_none() && self.route.is_empty()
    }
    pub fn next(&mut self, tile: Tile, tick: u64) -> Result<Option<Vec<u8>>, String> {
        use rs09_protocol::game::{Direction, actions};
        if let Some((target, sent)) = self.pending {
            if let Some(target) = target {
                if target != tile {
                    if tick - sent > 40 {
                        return Err(format!(
                            "Travel stopped at {tile:?} on the way to {target:?}"
                        ));
                    }
                    return Ok(None);
                }
            } else if tick - sent <= 5 {
                return Ok(None);
            }
            self.pending = None;
        }
        let Some(next) = self.route.pop_front() else {
            return Ok(None);
        };
        if next.is_array() {
            let target: Tile = serde_json::from_value(next).map_err(|e| e.to_string())?;
            self.pending = Some((Some(target), tick));
            let packet = actions::worldspace_walk(target[0], target[1], true);
            let mut wire = Vec::new();
            packet
                .write(Direction::ClientToServer, None, &mut wire)
                .map_err(|e| e.to_string())?;
            Ok(Some(wire))
        } else {
            let [x, y]: Tile =
                serde_json::from_value(next["tile"].clone()).map_err(|e| e.to_string())?;
            let id = next["id"].as_i64().ok_or("Missing gate definition id")? as i32;
            self.pending = Some((None, tick));
            Ok(actions::location_packet(42, id, [x, y]))
        }
    }
}
