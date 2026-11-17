//! RegionChunk synchronization/update, item visibility and dirty identity set.
//! Player/session, clipping/debug construction and foreign flag bodies bind to
//! the owning world. No buffer or scene lock is held across these callbacks.
use super::{
    chunk::{ARRAY_SIZE, HandleChunk, SIZE},
    doors::{Handle as Scenery, Location},
    ground_item::{self as ground, Handle as Ground},
    player_walk::NodeHandle,
    scenery::{self, ObjectUpdate},
    scenery_node::Layer,
};
use crate::definitions::item::Shared;
use rs09_protocol::{buffer::Writer, game::Frame};
use std::collections::HashMap;
pub type Buffer = Shared<Frame>;
pub type Flags = Shared<Vec<Option<Shared<Flag>>>>;
pub type Items = Shared<ground::List>;
#[derive(Clone)]
pub enum Flag {
    Object(ObjectUpdate),
    Item {
        item: Ground,
        kind: i32,
        old_amount: i32,
    },
    /// Host registry key, never an actor ID. Bindings and captured node leases
    /// must live as long as every flag handle, including exposed/snapshot copies.
    /// Shared::keeping_from can attach a native capture owner to the flag.
    Foreign(usize),
}
#[derive(Debug)]
pub enum Error {
    Null,
    Ground(ground::Error),
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
impl From<ground::Error> for Error {
    fn from(e: ground::Error) -> Self {
        Self::Ground(e)
    }
}
pub type Result<T> = std::result::Result<T, Error>;
pub trait Context: ground::Context {
    fn last_scene(&mut self, player: &NodeHandle) -> Result<Location>;
    fn send(&mut self, player: &NodeHandle, buffer: &Buffer) -> Result<()>;
    fn attribute(&mut self, player: &NodeHandle, name: &str) -> Result<bool>;
    fn clipping(&mut self, z: i32, x: i32, y: i32) -> Result<i32>;
    fn layer(&mut self, object: &Scenery) -> Result<Option<Layer>>;
    fn foreign_flag(&mut self, id: usize, buffer: &Buffer, player: &NodeHandle) -> Result<()>;
}
fn bytes(b: &Buffer, data: &[u8]) {
    b.write().payload.extend_from_slice(data);
}
fn byte(b: &Buffer, value: i32) {
    bytes(b, &[value as u8]);
}
fn short(b: &Buffer, value: i32) {
    bytes(b, &(value as u16).to_be_bytes());
}
fn offset(l: &Location) -> i32 {
    let [x, y, _] = l.coordinates();
    ((x & 7) << 4) | (y & 7)
}
fn transform(l: &Location, x: i32, y: i32) -> Location {
    let [a, b, z] = l.coordinates();
    Location::new(a.wrapping_add(x), b.wrapping_add(y), z)
}
fn object(b: &Buffer, h: &Scenery, remove: bool) {
    let f = ObjectUpdate {
        object: h.clone(),
        remove,
    }
    .frame();
    byte(b, i32::from(f.opcode));
    bytes(b, &f.payload);
}
pub fn scene_offset(
    ctx: &mut impl Context,
    player: &NodeHandle,
    base: &Location,
) -> Result<[i32; 2]> {
    let scene = ctx.last_scene(player)?;
    let bx = base.coordinates()[0];
    let sx = scene.coordinates()[0];
    let x = bx.wrapping_sub(((sx >> 3).wrapping_sub(6)).wrapping_shl(3));
    let scene = ctx.last_scene(player)?;
    let by = base.coordinates()[1];
    let sy = scene.coordinates()[1];
    let y = by.wrapping_sub(((sy >> 3).wrapping_sub(6)).wrapping_shl(3));
    Ok([x, y])
}
pub fn new_buffer(ctx: &mut impl Context, player: &NodeHandle, base: Location) -> Result<Buffer> {
    let [x, y] = scene_offset(ctx, player, &base)?;
    let mut w = Writer::new();
    w.p1add(y);
    w.p1sub(x);
    Ok(Shared::new(Frame {
        opcode: 230,
        payload: w.into_vec(),
    }))
}
pub fn write_item(buffer: &Buffer, h: &Ground, kind: i32, old_amount: i32) -> Result<()> {
    let l = h.read().location.clone();
    let item = h.read().item.clone();
    if kind == 1 {
        byte(buffer, 240);
        let l = l.ok_or(Error::Null)?;
        byte(buffer, 128i32.wrapping_sub(offset(&l)));
        short(buffer, item.read().id());
    } else if kind == 0 {
        byte(buffer, 33);
        bytes(buffer, &(item.read().id() as u16).to_le_bytes());
        let l = l.ok_or(Error::Null)?;
        byte(buffer, offset(&l));
        let mut w = Writer::new();
        w.p2add(item.read().amount());
        bytes(buffer, w.as_slice());
    } else {
        byte(buffer, 14);
        let l = l.ok_or(Error::Null)?;
        byte(buffer, offset(&l));
        short(buffer, item.read().id());
        short(buffer, old_amount);
        short(buffer, item.read().amount());
    }
    Ok(())
}
pub fn write_flag(
    ctx: &mut impl Context,
    flag: &Shared<Flag>,
    buffer: &Buffer,
    player: &NodeHandle,
) -> Result<()> {
    let f = flag.read().clone();
    match f {
        Flag::Object(o) => {
            object(buffer, &o.object, o.remove);
            Ok(())
        }
        Flag::Item {
            item,
            kind,
            old_amount,
        } => {
            if kind != 1 && ground::dropped_by(ctx, &item, player)? {
                return Ok(());
            }
            write_item(buffer, &item, kind, old_amount)
        }
        Flag::Foreign(id) => ctx.foreign_flag(id, buffer, player),
    }
}
/// Append before notifying the listener, including when that listener fails.
pub fn flag(
    chunk: &HandleChunk,
    flag: Option<Shared<Flag>>,
    listener: &mut impl FnMut(&HandleChunk) -> Result<()>,
) -> Result<()> {
    flag_with(chunk, flag, listener)
}
/// Same enqueue/listener ordering with an owning subsystem's error type.
pub fn flag_with<E>(
    chunk: &HandleChunk,
    flag: Option<Shared<Flag>>,
    listener: &mut impl FnMut(&HandleChunk) -> std::result::Result<(), E>,
) -> std::result::Result<(), E> {
    chunk.read().update_flags.write().push(flag);
    listener(chunk)
}
pub fn reset_flags(chunk: &HandleChunk) {
    chunk.read().update_flags.write().clear();
}
pub fn get_items(chunk: &HandleChunk) -> Items {
    if chunk.read().items.is_none() {
        chunk.write().items = Some(Shared::new(ground::List::default()));
    }
    chunk.read().items.clone().expect("initialized item list")
}
#[derive(Default)]
pub struct DirtyTracker {
    chunks: HashMap<usize, HandleChunk>,
}
impl DirtyTracker {
    pub fn on_flagged(&mut self, c: &HandleChunk) {
        self.chunks.entry(c.identity()).or_insert_with(|| c.clone());
    }
    pub fn len(&self) -> usize {
        self.chunks.len()
    }
    /// The flagged chunks (order not meaningful: the original's set is keyed by identity).
    pub fn chunks(&self) -> Vec<HandleChunk> {
        self.chunks.values().cloned().collect()
    }
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }
    /// Original base RegionChunk.resetUpdateFlags has no callbacks or overrides
    /// in the pinned tree. Clearing distinct chunk identities therefore commutes.
    pub fn reset_all(&mut self) {
        for c in self.chunks.values() {
            reset_flags(c);
        }
        self.chunks.clear();
    }
}
pub fn update(ctx: &mut impl Context, chunk: &HandleChunk, player: &NodeHandle) -> Result<()> {
    if chunk.read().update_flags.read().is_empty() {
        return Ok(());
    }
    let base = chunk.read().current_base.clone();
    let buffer = new_buffer(ctx, player, base)?;
    let flags = chunk.read().update_flags.read().clone();
    for f in flags {
        write_flag(ctx, &f.ok_or(Error::Null)?, &buffer, player)?;
    }
    ctx.send(player, &buffer)
}
/// Complete ClearRegionChunk.send body. The last-scene reference is captured
/// once, then live current-base x/y are read before the original104tile bounds.
pub fn clear(ctx: &mut impl Context, chunk: &HandleChunk, player: &NodeHandle) -> Result<()> {
    let last = ctx.last_scene(player)?;
    let [x, _, _] = chunk.read().current_base.coordinates();
    let [sx, _, _] = last.coordinates();
    let x = x.wrapping_sub(((sx >> 3).wrapping_sub(6)).wrapping_shl(3));
    let [_, y, _] = chunk.read().current_base.coordinates();
    let [_, sy, _] = last.coordinates();
    let y = y.wrapping_sub(((sy >> 3).wrapping_sub(6)).wrapping_shl(3));
    if (0..104).contains(&x) && (0..104).contains(&y) {
        ctx.send(
            player,
            &Shared::new(Frame {
                opcode: 112,
                payload: vec![x as u8, 0u8.wrapping_sub(y as u8)],
            }),
        )?;
    }
    Ok(())
}
fn debug_item(
    ctx: &mut impl Context,
    id: i32,
    base: &Location,
    x: i32,
    y: i32,
    player: &NodeHandle,
) -> Result<Ground> {
    let item = ctx.copy_item(id, 1, 1000)?;
    let l = transform(base, x, y);
    Ok(ground::construct(
        ctx,
        &item,
        Some(l),
        200,
        Some(player.clone()),
    )?)
}
pub fn draw_items(
    ctx: &mut impl Context,
    chunk: &HandleChunk,
    items: Option<Items>,
    player: &NodeHandle,
) -> Result<Vec<Option<Ground>>> {
    let mut total = items.map_or_else(Vec::new, |items| items.read().snapshot());
    if ctx.attribute(player, "chunkdraw")? {
        let l = chunk.read().current_base.clone();
        for x in 0..SIZE as i32 {
            for y in 0..SIZE as i32 {
                if x == 0 || y == 0 || x == 7 || y == 7 {
                    total.push(Some(debug_item(ctx, 13444, &l, x, y, player)?));
                }
            }
        }
    }
    if ctx.attribute(player, "regiondraw")? {
        let l = chunk.read().current_base.clone();
        let [x0, y0, _] = l.coordinates();
        let (local_x, local_y) = (x0 & 63, y0 & 63);
        for x in 0..SIZE as i32 {
            for y in 0..SIZE as i32 {
                if (local_y == 0 && y == 0)
                    || (local_y == 56 && y == 7)
                    || (local_x == 0 && x == 0)
                    || (local_x == 56 && x == 7)
                {
                    total.push(Some(debug_item(ctx, 13444, &l, x, y, player)?));
                }
            }
        }
    }
    if ctx.attribute(player, "clippingdraw")? {
        let l = chunk.read().current_base.clone();
        for x in 0..SIZE as i32 {
            for y in 0..SIZE as i32 {
                let [a, b, z] = l.coordinates();
                let flag = ctx.clipping(z, a.wrapping_add(x), b.wrapping_add(y))?;
                if flag > 0 {
                    total.push(Some(debug_item(ctx, flag, &l, x, y, player)?));
                }
            }
        }
    }
    Ok(total)
}
pub fn append_update(
    ctx: &mut impl Context,
    chunk: &HandleChunk,
    player: &NodeHandle,
    buffer: &Buffer,
) -> Result<bool> {
    let mut updated = false;
    for x in 0..SIZE {
        for y in 0..SIZE {
            for i in 0..ARRAY_SIZE {
                let dynamic = chunk.read().objects.read()[x][y][i].clone();
                let stat = chunk.read().static_objects.read()[x][y][i].clone();
                if stat.is_none() && dynamic.is_none() {
                    continue;
                }
                if let Some(ref stat) = stat {
                    if scenery::equals(stat, dynamic.as_ref()) {
                        continue;
                    }
                    let mut replaced = false;
                    for j in 0..ARRAY_SIZE {
                        if j == i {
                            continue;
                        }
                        let other = chunk.read().objects.read()[x][y][j].clone();
                        if let Some(other) = other {
                            let a = ctx.layer(&other)?;
                            let b = ctx.layer(stat)?;
                            if a == b {
                                replaced = true;
                                break;
                            }
                        }
                    }
                    if !replaced {
                        object(buffer, stat, true);
                        updated = true;
                    }
                }
                if let Some(dynamic) = dynamic {
                    let renderable = dynamic.read().renderable;
                    if renderable {
                        object(buffer, &dynamic, false);
                        updated = true;
                    }
                }
            }
        }
    }
    let items = chunk.read().items.clone();
    let total = draw_items(ctx, chunk, items, player)?;
    for h in total.into_iter().flatten() {
        if !ground::is_active(ctx, &h)? {
            continue;
        }
        let located = h.read().location.is_some();
        if !located {
            continue;
        }
        let private = ground::is_private(ctx, &h)?;
        if !private || ground::dropped_by(ctx, &h, player)? {
            write_item(buffer, &h, 0, 0)?;
            updated = true;
        }
    }
    Ok(updated)
}
pub fn synchronize(ctx: &mut impl Context, chunk: &HandleChunk, player: &NodeHandle) -> Result<()> {
    let base = chunk.read().current_base.clone();
    let buffer = new_buffer(ctx, player, base)?;
    if append_update(ctx, chunk, player, &buffer)? {
        ctx.send(player, &buffer)?;
    }
    Ok(())
}
