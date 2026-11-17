//! RegionManager's landscape, clipping, water and teleport/reach predicates.
//! Region/cache loading and actual chunk/flag identity are owning-world bindings.
use super::{chunk::HandleChunk, clipping::Flags, doors::Location, region::Handle};

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
pub trait Context {
    fn region(&mut self, location: &Location) -> Result<Handle>;
    fn load(&mut self, region: &Handle) -> Result<()>;
    fn chunk(&mut self, location: &Location) -> Result<HandleChunk>;
    fn flags(&mut self, key: i32, projectile: bool) -> Result<Flags>;
}
pub fn clipping_flag(
    ctx: &mut impl Context,
    location: Option<&Location>,
    projectile: bool,
) -> Result<i32> {
    let location = location.ok_or(Error::Null)?;
    let region = ctx.region(location)?;
    ctx.load(&region)?;
    if !region.read().has_flags {
        return Ok(-1);
    }
    // Original getFlagIndex uses absolute coordinates, independently of a
    // mutable/rebased ChunkFlags.base. Capture key/index before getFlags callbacks.
    let [x, y, z] = location.coordinates();
    let key = z.wrapping_shl(22) | (x >> 3).wrapping_shl(11) | (y >> 3);
    let index = ((x & 7) * 8 + (y & 7)) as usize;
    let flags = ctx.flags(key, projectile)?;
    let value = flags.read()[index];
    Ok(value)
}
pub fn is_landscape(ctx: &mut impl Context, location: Option<&Location>) -> Result<bool> {
    let location = location.ok_or(Error::Null)?;
    let region = ctx.region(location)?;
    ctx.load(&region)?;
    if !region.read().has_flags {
        return Ok(false);
    }
    let chunk = ctx.chunk(location)?;
    if chunk.read().flags.landscape.is_none() {
        return Ok(false);
    }
    // The source repeats loc.chunk and flags.landscape after its null test.
    // Do not use the first receiver if a callback replaced it in between.
    let chunk = ctx.chunk(location)?;
    let landscape = chunk.read().flags.landscape.clone().ok_or(Error::Null)?;
    let x = (location.coordinates()[0] & 7) as usize;
    let row = landscape
        .read()
        .get(x)
        .ok_or(Error::Index)?
        .clone()
        .ok_or(Error::Null)?;
    let y = (location.coordinates()[1] & 7) as usize;
    let value = *row.read().get(y).ok_or(Error::Index)?;
    Ok(value)
}
const MASKS: [i32; 4] = [0x12c0102, 0x12c0108, 0x12c0120, 0x12c0180];
pub fn teleport_permitted(ctx: &mut impl Context, location: Option<&Location>) -> Result<bool> {
    let location = location.ok_or(Error::Null)?;
    if !is_landscape(ctx, Some(location))? {
        return Ok(false);
    }
    let flag = clipping_flag(ctx, Some(location), false)?;
    Ok(MASKS.iter().any(|mask| flag & mask == 0))
}
pub fn is_clipped(ctx: &mut impl Context, location: Option<&Location>) -> Result<bool> {
    let location = location.ok_or(Error::Null)?;
    if !is_landscape(ctx, Some(location))? {
        return Ok(true);
    }
    let flag = clipping_flag(ctx, Some(location), false)?;
    Ok(MASKS.iter().any(|mask| flag & mask != 0))
}
pub fn water_flag(ctx: &mut impl Context, location: Option<&Location>) -> Result<i32> {
    let location = location.ok_or(Error::Null)?;
    // Capture the first flag before the independent landscape/clipping queries.
    let flag = clipping_flag(ctx, Some(location), false)?;
    Ok(if !is_clipped(ctx, Some(location))? {
        flag | 0x100
    } else {
        flag & !0x200000
    })
}
