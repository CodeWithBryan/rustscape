//! Region.load ordering and state publication. Cache delivery, parser calls and
//! scenery services are supplied by the native owning world context.
use super::{
    chunk::RegionChunk,
    doors::{Handle as Scenery, Location},
    landscape::{Chunks, Mapscape, ParseRegion},
};
use crate::definitions::item::Shared;
use std::collections::HashMap;
#[derive(Debug)]
pub enum Error {
    Null,
    ConcurrentModification,
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Default)]
pub struct Sceneries {
    values: Vec<Scenery>,
    version: i32,
}
impl Sceneries {
    pub fn push(&mut self, h: Scenery) {
        self.values.push(h);
        self.version = self.version.wrapping_add(1);
    }
    pub fn clear(&mut self) {
        self.values.clear();
        self.version = self.version.wrapping_add(1);
    }
    pub fn remove(&mut self, index: usize) -> Option<Scenery> {
        if index >= self.values.len() {
            return None;
        }
        self.version = self.version.wrapping_add(1);
        Some(self.values.remove(index))
    }
    pub fn values(&self) -> &[Scenery] {
        &self.values
    }
}
pub struct Region {
    pub data: Shared<ParseRegion>,
    pub chunks: Option<Chunks>,
    pub loaded: bool,
    pub build: bool,
    pub has_flags: bool,
    pub dynamic_template: Option<i32>,
    pub add: Shared<Sceneries>,
    pub remove: Shared<Sceneries>,
    pub tolerances: Shared<HashMap<String, Option<i64>>>,
    /// Active flag, viewers, activity pulse, zones and music: region_activity.
    pub activity: Shared<super::region_activity::Activity>,
}
pub type Handle = Shared<Region>;
impl Region {
    pub fn new(x: i32, y: i32) -> Self {
        let base = Location::new(x.wrapping_shl(6), y.wrapping_shl(6), 0);
        let [xbase, ybase, _] = base.coordinates();
        let chunks = Shared::new(std::array::from_fn(|x| {
            std::array::from_fn(|y| {
                std::array::from_fn(|z| {
                    Some(Shared::new(RegionChunk::new(
                        Location::new(
                            xbase.wrapping_add(x as i32 * 8),
                            ybase.wrapping_add(y as i32 * 8),
                            z as i32,
                        ),
                        0,
                    )))
                })
            })
        }));
        Self {
            data: Shared::new(ParseRegion {
                x,
                y,
                object_count: 0,
            }),
            chunks: Some(chunks),
            loaded: false,
            build: false,
            has_flags: false,
            dynamic_template: None,
            add: Shared::new(Sceneries::default()),
            remove: Shared::new(Sceneries::default()),
            tolerances: Shared::new(HashMap::new()),
            activity: Shared::new(Default::default()),
        }
    }
    pub fn id(&self) -> i32 {
        let d = self.data.read();
        d.x.wrapping_shl(8) | d.y
    }
}
pub trait Context {
    fn archive_id(&mut self, name: &str) -> Result<i32>;
    fn mapscape_bytes(&mut self, group: i32) -> Result<Option<Vec<u8>>>;
    fn xtea(&mut self, region: i32) -> Result<[u32; 4]>;
    fn landscape_bytes(&mut self, group: i32, key: [u32; 4]) -> Result<Option<Vec<u8>>>;
    fn parse_mapscape(&mut self, region: &Handle, map: &Mapscape, bytes: &[u8]) -> Result<()>;
    fn parse_landscape(
        &mut self,
        region: &Handle,
        chunks: Option<Chunks>,
        map: &Mapscape,
        bytes: &[u8],
        build: bool,
    ) -> Result<()>;
    fn clip_mapscape(
        &mut self,
        region: &Handle,
        chunks: Option<Chunks>,
        map: &Mapscape,
    ) -> Result<()>;
    fn object(&mut self, location: Location, id: i32, shape: i32) -> Result<Option<Scenery>>;
    fn remove(&mut self, scenery: &Scenery) -> Result<()>;
    fn add(&mut self, scenery: &Scenery) -> Result<()>;
    /// None is the outer Throwable.printStackTrace; Some(id) is the wrapped
    /// "Failed parsing region id!" diagnostic from landscape parsing.
    fn exception(&mut self, error: &Error, landscape_region: Option<i32>) -> Result<()>;
}
fn for_each(list: &Shared<Sceneries>, mut action: impl FnMut(Scenery) -> Result<()>) -> Result<()> {
    let expected = list.read().version;
    let mut cursor = 0;
    while cursor != list.read().values.len() {
        let h = {
            let list = list.read();
            if expected != list.version {
                return Err(Error::ConcurrentModification);
            }
            list.values
                .get(cursor)
                .cloned()
                .ok_or(Error::ConcurrentModification)?
        };
        cursor += 1;
        action(h)?;
    }
    Ok(())
}
pub fn load(ctx: &mut impl Context, region: &Handle, build: Option<bool>) -> Result<()> {
    let build = build.unwrap_or_else(|| region.read().build);
    if let Err(error) = load_body(ctx, region, build) {
        ctx.exception(&error, None)?;
    }
    Ok(())
}
fn load_body(ctx: &mut impl Context, region: &Handle, build: bool) -> Result<()> {
    {
        let r = region.read();
        if r.loaded && r.build == build {
            return Ok(());
        }
    }
    region.write().build = build;
    let (dynamic, id) = {
        let r = region.read();
        (
            r.dynamic_template.is_some(),
            r.dynamic_template.unwrap_or_else(|| r.id()),
        )
    };
    let (x, y) = ((id >> 8) & 255, id & 255);
    let map_id = ctx.archive_id(&format!("m{x}_{y}"))?;
    if map_id < 0 && !dynamic {
        region.write().loaded = true;
        return Ok(());
    }
    let map: Mapscape = Shared::new(Box::new([[[0; 64]; 64]; 4]));
    for x in 0..8 {
        for y in 0..8 {
            for z in 0..4 {
                let chunks = region.read().chunks.clone().ok_or(Error::Null)?;
                let chunk = chunks.read()[x][y][z].clone().ok_or(Error::Null)?;
                chunk.write().flags.landscape =
                    super::clipping::ChunkFlags::new(0, 0, 0, false).landscape;
            }
        }
    }
    if map_id >= 0 {
        let bytes = ctx.mapscape_bytes(map_id)?.ok_or(Error::Null)?;
        ctx.parse_mapscape(region, &map, &bytes)?;
    }
    {
        let mut r = region.write();
        r.has_flags = dynamic;
        r.loaded = true;
    }
    let landscape_id = ctx.archive_id(&format!("l{x}_{y}"))?;
    if landscape_id >= 0 {
        let key = ctx.xtea(id)?;
        let bytes = ctx.landscape_bytes(landscape_id, key)?;
        let Some(bytes) = bytes.filter(|b| b.len() >= 4) else {
            return Ok(());
        };
        region.write().has_flags = true;
        let chunks = region.read().chunks.clone();
        if let Err(error) = ctx.parse_landscape(region, chunks, &map, &bytes, build) {
            ctx.exception(&error, Some(id))?;
        }
    }
    let chunks = region.read().chunks.clone();
    ctx.clip_mapscape(region, chunks, &map)?;
    let remove = region.read().remove.clone();
    for_each(&remove, |h| {
        let (l, id, shape) = {
            let h = h.read();
            (h.location.clone(), h.id, h.shape)
        };
        if let Some(actual) = ctx.object(l, id, shape)? {
            ctx.remove(&actual)?;
        }
        Ok(())
    })?;
    let add = region.read().add.clone();
    for_each(&add, |h| ctx.add(&h))?;
    Ok(())
}
