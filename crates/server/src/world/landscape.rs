//! LandscapeParser placement, removal and location-stream decoding. Definition
//! resolution and region loading are supplied by the owning native world.
use super::{
    chunk::{self, HandleChunk},
    clipping,
    doors::{Handle, Location},
    scenery,
};
use crate::definitions::item::Shared;
use rs09_protocol::buffer::Reader;
#[derive(Debug)]
pub enum Error {
    Null,
    Index,
    Chunk(chunk::Error),
    Clipping(clipping::Error),
    Scenery(scenery::Error),
    Decode(rs09_protocol::Error),
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;
impl From<chunk::Error> for Error {
    fn from(e: chunk::Error) -> Self {
        Self::Chunk(e)
    }
}
impl From<clipping::Error> for Error {
    fn from(e: clipping::Error) -> Self {
        Self::Clipping(e)
    }
}
impl From<scenery::Error> for Error {
    fn from(e: scenery::Error) -> Self {
        Self::Scenery(e)
    }
}
impl From<rs09_protocol::Error> for Error {
    fn from(e: rs09_protocol::Error) -> Self {
        Self::Decode(e)
    }
}
/// Landscape consumes the same retained definition handles as the cache library.
pub use crate::definitions::scenery::SceneryDefinition as Definition;
pub trait Context: scenery::Context + clipping::Context {
    /// Must return the definition retained by this scenery's constructor.
    fn definition(&mut self, object: &Handle) -> Result<Shared<Definition>>;
    fn child_has_actions(&mut self, definition: &Shared<Definition>) -> Result<bool>;
    fn load_region(&mut self, location: &Location) -> Result<()>;
    fn region_chunk(&mut self, location: &Location) -> Result<HandleChunk>;
}
pub fn add(ctx: &mut impl Context, object: &Handle, landscape: bool) -> Result<()> {
    let loc = object.read().location.clone();
    let chunk = ctx.region_chunk(&loc)?;
    let [x, y, _] = loc.coordinates();
    flag(ctx, &chunk, x & 7, y & 7, object, landscape, false)
}
pub fn flag(
    ctx: &mut impl Context,
    chunk: &HandleChunk,
    x: i32,
    y: i32,
    object: &Handle,
    landscape: bool,
    store_objects: bool,
) -> Result<()> {
    let base = chunk.read().current_base.clone();
    ctx.load_region(&base)?;
    let def = ctx.definition(object)?;
    scenery::set_active(ctx, object, true)?;
    object.write().renderable = true;
    if store_objects || !landscape || ctx.child_has_actions(&def)? {
        if landscape {
            chunk::add_static_dynamic(ctx, chunk, object, x, y)?;
        } else {
            chunk::add(ctx, chunk, object)?;
        }
    }
    apply_clipping(ctx, chunk, x, y, object)?;
    Ok(())
}
pub fn apply_clipping(
    ctx: &mut impl Context,
    chunk: &HandleChunk,
    x: i32,
    y: i32,
    object: &Handle,
) -> Result<bool> {
    let def = ctx.definition(object)?;
    apply(ctx, chunk, x, y, object, &def, true)
}
fn apply(
    ctx: &mut impl Context,
    chunk: &HandleChunk,
    x: i32,
    y: i32,
    object: &Handle,
    def: &Shared<Definition>,
    add: bool,
) -> Result<bool> {
    let (size_x, size_y) = {
        let d = def.read();
        if object.read().rotation % 2 == 0 {
            (d.size_x, d.size_y)
        } else {
            (d.size_y, d.size_x)
        }
    };
    let shape = object.read().shape;
    if shape == 22 {
        if add {
            chunk.read().flags.clone().mark_landscape(x, y, true)?;
        }
        let block = {
            let d = def.read();
            d.interactable != 0 || d.clip_type == 1 || d.second_bool
        };
        if block && def.read().clip_type == 1 {
            let flags = chunk.read().flags.clone();
            flags.tile_object(ctx, x, y, add)?;
            if def.read().projectile_clipped {
                let flags = chunk.read().projectile_flags.clone();
                flags.tile_object(ctx, x, y, add)?;
            }
        }
    } else if shape >= 9 {
        if def.read().clip_type != 0 {
            let clipped = def.read().projectile_clipped;
            let flags = chunk.read().flags.clone();
            flags.solid_object(ctx, x, y, size_x, size_y, clipped, add)?;
            if def.read().projectile_clipped {
                let clipped = def.read().projectile_clipped;
                let flags = chunk.read().projectile_flags.clone();
                flags.solid_object(ctx, x, y, size_x, size_y, clipped, add)?;
            }
        }
    } else if (0..=3).contains(&shape) {
        if def.read().clip_type != 0 {
            let rotation = object.read().rotation;
            let clipped = def.read().projectile_clipped;
            let flags = chunk.read().flags.clone();
            flags.door(ctx, x, y, rotation, shape, clipped, add)?;
            if def.read().projectile_clipped {
                let rotation = object.read().rotation;
                let clipped = def.read().projectile_clipped;
                let flags = chunk.read().projectile_flags.clone();
                flags.door(ctx, x, y, rotation, shape, clipped, add)?;
            }
        }
    } else {
        return Ok(false);
    }
    Ok(true)
}
pub fn remove(ctx: &mut impl Context, object: &Handle) -> Result<Option<Handle>> {
    if !object.read().renderable {
        return Ok(None);
    }
    let loc = object.read().location.clone();
    ctx.load_region(&loc)?;
    let chunk = ctx.region_chunk(&loc)?;
    let [x, y, _] = loc.coordinates();
    let (x, y) = (x & 7, y & 7);
    let (id, shape) = {
        let o = object.read();
        (o.id, o.shape)
    };
    let index = chunk::get_index(&chunk, x, y, id, shape)?;
    if index < 0 {
        return Ok(None);
    }
    let objects = chunk.read().objects.clone();
    let current = objects.read()[x as usize][y as usize][index as usize]
        .clone()
        .ok_or(Error::Null)?;
    scenery::set_active(ctx, &current, false)?;
    scenery::set_active(ctx, object, false)?;
    // A destruction callback may replace the chunk's array reference.
    chunk.read().objects.write()[x as usize][y as usize][index as usize] = None;
    let def = ctx.definition(object)?;
    // Removal deliberately uses the argument's definition, not current's.
    apply(ctx, &chunk, x, y, object, &def, false)?;
    Ok(Some(current))
}
pub struct ParseRegion {
    pub x: i32,
    pub y: i32,
    pub object_count: i32,
}
pub type Chunks = Shared<[[[Option<HandleChunk>; 4]; 8]; 8]>;
pub type Mapscape = Shared<Box<[[[i8; 64]; 64]; 4]>>;
fn smart(r: &mut Reader<'_>) -> Result<i32> {
    let first = r.g1()?;
    Ok(if first <= 127 {
        first
    } else {
        (first << 8 | r.g1()?) - 32768
    })
}
fn big_smart(r: &mut Reader<'_>) -> Result<i32> {
    let mut value = 0i32;
    let mut current = smart(r)?;
    while current == 32767 {
        current = smart(r)?;
        value = value.wrapping_add(32767);
    }
    Ok(value.wrapping_add(current))
}
pub fn parse(
    ctx: &mut impl Context,
    region: &Shared<ParseRegion>,
    chunks: &Chunks,
    mapscape: &Mapscape,
    r: &mut Reader<'_>,
    store_objects: bool,
) -> Result<()> {
    let mut id = -1i32;
    loop {
        let offset = big_smart(r)?;
        if offset == 0 {
            break;
        }
        id = id.wrapping_add(offset);
        let mut location = 0i32;
        loop {
            let offset = smart(r)?;
            if offset == 0 {
                break;
            }
            location = location.wrapping_add(offset.wrapping_sub(1));
            let y = location & 63;
            let x = (location >> 6) & 63;
            let configuration = r.g1()?;
            let rotation = configuration & 3;
            let shape = configuration >> 2;
            let mut z = location >> 12;
            {
                let mut region = region.write();
                region.object_count = region.object_count.wrapping_add(1);
            }
            if mapscape.read()[1][x as usize][y as usize] & 2 == 2 {
                z = z.wrapping_sub(1);
            }
            if (0..=3).contains(&z) {
                let (rx, ry) = {
                    let region = region.read();
                    (region.x, region.y)
                };
                let l = Location::new(
                    rx.wrapping_shl(6).wrapping_add(x),
                    ry.wrapping_shl(6).wrapping_add(y),
                    z,
                );
                let object = ctx.construct(id, l.clone(), shape, rotation)?;
                let [x, y, _] = l.coordinates();
                let chunk = chunks.read()[((x & 63) >> 3) as usize][((y & 63) >> 3) as usize]
                    [z as usize]
                    .clone()
                    .ok_or(Error::Null)?;
                flag(ctx, &chunk, x & 7, y & 7, &object, true, store_objects)?;
            }
        }
    }
    Ok(())
}
