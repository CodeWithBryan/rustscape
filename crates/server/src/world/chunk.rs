//! RegionChunk storage and shared entity/item/update lists. Synchronization is
//! in chunk_sync; dynamic rotation and ground-item management remain separate.
use super::{
    clipping::ChunkFlags,
    doors::{Handle, Location},
    player_walk::NodeHandle,
    scenery,
    update::List,
};
use crate::definitions::item::Shared;

pub const SIZE: usize = 8;
pub const ARRAY_SIZE: usize = 4;
pub type Objects = Shared<[[[Option<Handle>; ARRAY_SIZE]; SIZE]; SIZE]>;
pub type HandleChunk = Shared<RegionChunk>;
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Index,
    Capacity(String),
    Scenery(scenery::Error),
}
pub type Result<T> = std::result::Result<T, Error>;
impl From<scenery::Error> for Error {
    fn from(e: scenery::Error) -> Self {
        Self::Scenery(e)
    }
}
pub struct RegionChunk {
    pub base: Location,
    pub current_base: Location,
    pub rotation: i32,
    pub static_objects: Objects,
    pub objects: Objects,
    pub flags: ChunkFlags,
    pub projectile_flags: ChunkFlags,
    pub players: List<Option<NodeHandle>>,
    pub npcs: List<Option<NodeHandle>>,
    pub update_flags: super::chunk_sync::Flags,
    pub items: Option<super::chunk_sync::Items>,
}
impl RegionChunk {
    pub fn new(base: Location, rotation: i32) -> Self {
        let [x, y, z] = base.coordinates();
        let objects = || {
            Shared::new(std::array::from_fn(|_| {
                std::array::from_fn(|_| std::array::from_fn(|_| None))
            }))
        };
        Self {
            base: base.clone(),
            current_base: base,
            rotation,
            static_objects: objects(),
            objects: objects(),
            flags: ChunkFlags::new(x, y, z, false),
            projectile_flags: ChunkFlags::new(x, y, z, true),
            players: List::plain(),
            npcs: List::plain(),
            update_flags: Shared::new(Vec::new()),
            items: Some(Shared::new(super::ground_item::List::default())),
        }
    }
    pub fn reset_clipping_flags(&mut self) {
        let [x, y, z] = self.current_base.coordinates();
        self.flags = ChunkFlags::new(x, y, z, false);
        self.projectile_flags = ChunkFlags::new(x, y, z, true);
    }
}
fn offsets(x: i32, y: i32) -> Result<(usize, usize)> {
    if (0..SIZE as i32).contains(&x) && (0..SIZE as i32).contains(&y) {
        Ok((x as usize, y as usize))
    } else {
        Err(Error::Index)
    }
}
pub fn get_index(chunk: &HandleChunk, x: i32, y: i32, id: i32, shape: i32) -> Result<i32> {
    let (x, y) = offsets(x, y)?;
    for i in 0..ARRAY_SIZE {
        let object = chunk.read().objects.read()[x][y][i].clone();
        if let Some(object) = object {
            let object = object.read();
            if if id < 0 {
                shape < 0 || object.shape == shape
            } else {
                object.id == id
            } {
                return Ok(i as i32);
            }
        }
    }
    Ok(-1)
}
fn transform(ctx: &mut impl scenery::Context, object: &Handle) -> scenery::Result<Handle> {
    let (id, location, shape, rotation) = {
        let o = object.read();
        (o.id, o.location.clone(), o.shape, o.rotation)
    };
    ctx.construct(id, location, shape, rotation)
}
pub fn add_static_dynamic(
    ctx: &mut impl scenery::Context,
    chunk: &HandleChunk,
    object: &Handle,
    x: i32,
    y: i32,
) -> Result<()> {
    let (x, y) = offsets(x, y)?;
    for i in 0..ARRAY_SIZE {
        let current = chunk.read().static_objects.read()[x][y][i].clone();
        if let Some(current) = current {
            if current.read().id == object.read().id {
                return Ok(());
            }
        } else {
            chunk.read().static_objects.write()[x][y][i] = Some(object.clone());
            // The static write precedes constructor callbacks/failures.
            let copy = transform(ctx, object)?;
            chunk.read().objects.write()[x][y][i] = Some(copy);
            return Ok(());
        }
    }
    Err(Error::Capacity(format!(
        "RC addStatDyn insufficient array length for storing object {}",
        display(object)
    )))
}
pub fn add(ctx: &mut impl scenery::Context, chunk: &HandleChunk, object: &Handle) -> Result<i32> {
    let [x, y, _] = object.read().location.coordinates();
    let (x, y) = ((x & 7) as usize, (y & 7) as usize);
    for i in 0..ARRAY_SIZE {
        let current = chunk.read().objects.read()[x][y][i].clone();
        if let Some(current) = current {
            if scenery::equals(&current, Some(object)) {
                current.write().renderable = true;
                scenery::set_active(ctx, &current, true)?;
                return Ok(i as i32);
            }
        } else {
            let constructed = scenery::as_constructed(ctx, object)?;
            chunk.read().objects.write()[x][y][i] = Some(constructed);
            object.write().renderable = true;
            scenery::set_active(ctx, object, true)?;
            return Ok(i as i32);
        }
    }
    Err(Error::Capacity(format!(
        "RC add insufficient array length for storing object {}",
        display(object)
    )))
}
fn display(object: &Handle) -> String {
    let o = object.read();
    let [x, y, z] = o.location.coordinates();
    format!(
        "[Scenery {}, [{x}, {y}, {z}], type={}, rot={}]",
        o.id, o.shape, o.rotation
    )
}
fn copy_objects(
    ctx: &mut impl scenery::Context,
    source: &Objects,
    destination: &Objects,
) -> Result<()> {
    for x in 0..SIZE {
        for y in 0..SIZE {
            for i in 0..ARRAY_SIZE {
                let Some(object) = source.read()[x][y][i].clone() else {
                    continue;
                };
                let copy = transform(ctx, &object)?;
                let copy = if object.read().constructed {
                    scenery::as_constructed(ctx, &copy)?
                } else {
                    copy
                };
                destination.write()[x][y][i] = Some(copy.clone());
                let active = object.read().active;
                scenery::set_active(ctx, &copy, active)?;
                copy.write().renderable = object.read().renderable;
            }
        }
    }
    Ok(())
}
pub fn copy(ctx: &mut impl scenery::Context, source: &HandleChunk) -> Result<HandleChunk> {
    let (base, rotation) = {
        let source = source.read();
        (source.base.clone(), source.rotation)
    };
    let dest = Shared::new(RegionChunk::new(base, rotation));
    let stat = source.read().static_objects.clone();
    let target = dest.read().static_objects.clone();
    copy_objects(ctx, &stat, &target)?;
    let objects = source.read().objects.clone();
    let target = dest.read().objects.clone();
    copy_objects(ctx, &objects, &target)?;
    Ok(dest)
}
pub fn rebase_objects(chunk: &HandleChunk) {
    fn rebase(chunk: &HandleChunk, objects: &Objects) {
        for i in 0..ARRAY_SIZE {
            for x in 0..SIZE {
                for y in 0..SIZE {
                    let Some(object) = objects.read()[x][y][i].clone() else {
                        continue;
                    };
                    let [a, b, z] = chunk.read().current_base.coordinates();
                    object.write().location =
                        Location::new(a.wrapping_add(x as i32), b.wrapping_add(y as i32), z);
                }
            }
        }
    }
    let objects = chunk.read().static_objects.clone();
    rebase(chunk, &objects);
    let objects = chunk.read().objects.clone();
    rebase(chunk, &objects);
}
pub fn rotated_position(
    x: i32,
    y: i32,
    mut size_x: i32,
    mut size_y: i32,
    rotation: i32,
    chunk_rotation: i32,
) -> [i32; 2] {
    if rotation & 1 == 1 {
        std::mem::swap(&mut size_x, &mut size_y);
    }
    let rx = 8i32.wrapping_sub(x).wrapping_sub(size_x);
    let ry = 8i32.wrapping_sub(y).wrapping_sub(size_y);
    match chunk_rotation {
        1 => [y, rx],
        2 => [rx, ry],
        3 => [ry, x],
        _ => [x, y],
    }
}
