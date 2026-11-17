//! Original MapChunkRenderer traversal over retained chunk/region identities.
//! The owning world supplies chunk synchronization, packet and activity services.
use super::{chunk::HandleChunk, doors::Location, player_walk::NodeHandle, region};
use crate::definitions::item::Shared;

pub const BUILD_AREA_DEPTH: i32 = 6;
pub const BUILD_AREA_SIZE: usize = 13;
pub type Row = Shared<Vec<Option<HandleChunk>>>;
pub type Viewport = Shared<Vec<Option<Row>>>;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Index,
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

pub fn empty_viewport() -> Viewport {
    Shared::new(
        (0..BUILD_AREA_SIZE)
            .map(|_| Some(Shared::new(vec![None; BUILD_AREA_SIZE])))
            .collect(),
    )
}

/// Callbacks may reenter the renderer or replace viewport rows. No shared lock
/// is held across a callback. Captured player/chunk/region handles keep their
/// original identities even if an owning registry is changed during a call.
pub trait Context {
    fn viewport(&mut self, player: &NodeHandle) -> Result<Viewport>;
    fn last_scene(&mut self, player: &NodeHandle) -> Result<Option<Location>>;
    fn location(&mut self, player: &NodeHandle) -> Result<Location>;
    fn chunk(&mut self, location: Location) -> Result<HandleChunk>;
    fn region(&mut self, location: Location) -> Result<region::Handle>;
    fn clear_chunk(&mut self, player: &NodeHandle, chunk: &HandleChunk) -> Result<()>;
    fn update_chunk(&mut self, player: &NodeHandle, chunk: &HandleChunk) -> Result<()>;
    fn synchronize_chunk(&mut self, player: &NodeHandle, chunk: &HandleChunk) -> Result<()>;
    fn name(&mut self, player: &NodeHandle) -> Result<String>;
    fn decrement_view(&mut self, region: &region::Handle, name: String) -> Result<()>;
    fn increment_view(&mut self, region: &region::Handle, name: String) -> Result<()>;
    fn is_active(&mut self, region: &region::Handle) -> Result<bool>;
    fn check_inactive(&mut self, region: &region::Handle) -> Result<()>;
    fn flag_active(&mut self, region: &region::Handle) -> Result<()>;
}

fn row(last: &Viewport, x: usize) -> Result<Row> {
    last.read()
        .get(x)
        .ok_or(Error::Index)?
        .clone()
        .ok_or(Error::Null)
}
fn unique(regions: &mut Vec<region::Handle>, r: region::Handle) {
    if !regions.iter().any(|previous| previous.same(&r)) {
        regions.push(r);
    }
}

pub fn render(ctx: &mut impl Context, player: &NodeHandle) -> Result<()> {
    let last = ctx.viewport(player)?;
    let mut updated: Vec<HandleChunk> = Vec::new();
    let anchor = match ctx.last_scene(player)? {
        Some(l) => l,
        None => ctx.location(player)?,
    };
    let [x, y, _] = anchor.coordinates();
    let z = ctx.location(player)?.coordinates()[2];
    let center = Location::new(x, y, z);
    let [x, y, z] = center.coordinates();
    let mut current = Vec::with_capacity(BUILD_AREA_SIZE);
    for dx in -BUILD_AREA_DEPTH..=BUILD_AREA_DEPTH {
        let mut chunks = Vec::with_capacity(BUILD_AREA_SIZE);
        for dy in -BUILD_AREA_DEPTH..=BUILD_AREA_DEPTH {
            chunks.push(ctx.chunk(Location::new(
                x.wrapping_add(dx * 8),
                y.wrapping_add(dy * 8),
                z,
            ))?);
        }
        current.push(chunks);
    }
    let mut previous_regions = Vec::new();
    let mut current_regions = Vec::new();
    let size_x = last.read().len();
    for x in 0..size_x {
        let size_y = row(&last, x)?.read().len();
        for y in 0..size_y {
            let previous = row(&last, x)?.read().get(y).ok_or(Error::Index)?.clone();
            let Some(previous) = previous else { continue };
            let base = previous.read().current_base.clone();
            unique(&mut previous_regions, ctx.region(base)?);
            if current.iter().flatten().any(|c| c.same(&previous)) {
                updated.push(previous);
            } else {
                ctx.clear_chunk(player, &previous)?;
            }
        }
    }
    for (x, chunks) in current.iter().enumerate() {
        for (y, chunk) in chunks.iter().enumerate() {
            let base = chunk.read().current_base.clone();
            unique(&mut current_regions, ctx.region(base)?);
            if updated.iter().any(|c| c.same(chunk)) {
                ctx.update_chunk(player, chunk)?;
            } else {
                ctx.synchronize_chunk(player, chunk)?;
            }
            *row(&last, x)?.write().get_mut(y).ok_or(Error::Index)? = Some(chunk.clone());
        }
    }
    for r in &previous_regions {
        if !current_regions.iter().any(|c| c.same(r)) {
            let name = ctx.name(player)?;
            ctx.decrement_view(r, name)?;
            if ctx.is_active(r)? {
                ctx.check_inactive(r)?;
            }
        }
    }
    for r in &current_regions {
        if !previous_regions.iter().any(|c| c.same(r)) {
            let name = ctx.name(player)?;
            ctx.increment_view(r, name)?;
        }
        if !ctx.is_active(r)? {
            ctx.flag_active(r)?;
        }
    }
    Ok(())
}
