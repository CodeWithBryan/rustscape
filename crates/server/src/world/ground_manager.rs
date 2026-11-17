//! Original GroundItemManager and RegionChunk ground-item add/remove bodies.
//! Concrete definition/plugin, actor, subtype, region and packet services belong
//! to the owning world. Lists/flags retain canonical item/player references.
use super::{
    chunk::HandleChunk,
    chunk_sync::{self, Flag, Items},
    doors::Location,
    ground_item::{self as ground, Handle},
    player_walk::NodeHandle,
};
use crate::{
    definitions::item::{Handle as Definition, Shared},
    inventory::item,
};

#[derive(Debug)]
pub enum Error {
    Null,
    Index,
    Type,
    ConcurrentModification,
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
    fn from(error: ground::Error) -> Self {
        Self::Ground(error)
    }
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Packet {
    Construct,
    Clear,
    Amount,
}

pub trait Context: ground::Context {
    type Plugin;
    fn tradeable(&mut self, definition: &Definition) -> Result<bool> {
        item::definition_tradeable(definition).map_err(|error| match error {
            crate::inventory::Error::Null => Error::Null,
            crate::inventory::Error::Type => Error::Type,
            error => Error::Binding(format!("{error:?}")),
        })
    }
    fn plugin(&mut self, definition: &Definition) -> Result<Option<Self::Plugin>>;
    fn plugin_remove(
        &mut self,
        plugin: Self::Plugin,
        player: Option<NodeHandle>,
        item: &Handle,
        option: i32,
    ) -> Result<()>;
    fn chunk(&mut self, location: Location) -> Result<HandleChunk>;
    fn flagged(&mut self, chunk: &HandleChunk) -> Result<()>;
    fn player_location(&mut self, player: &NodeHandle) -> Result<Option<Location>>;
    fn playing(&mut self, player: &NodeHandle) -> Result<bool>;
    fn artificial(&mut self, player: &NodeHandle) -> Result<bool>;
    fn ai_items(&mut self, player: &NodeHandle) -> Result<Option<Items>>;
    fn packet(
        &mut self,
        kind: Packet,
        player: Option<NodeHandle>,
        item: &Handle,
        old_amount: i32,
    ) -> Result<()>;
    // Actual GroundSpawn/content subclasses override these services. Default
    // dispatch preserves the independently ported ordinary GroundItem bodies.
    fn auto_spawn(&mut self, item: &Handle) -> Result<bool> {
        Ok(ground::is_auto_spawn(item))
    }
    fn respawn(&mut self, item: &Handle) -> Result<()> {
        ground::respawn(item);
        Ok(())
    }
}

#[derive(Clone)]
pub struct Manager {
    items: Items,
}
impl Default for Manager {
    fn default() -> Self {
        Self {
            items: Shared::new(ground::List::default()),
        }
    }
}
impl Manager {
    pub fn get_items(&self) -> Items {
        self.items.clone()
    }
}
pub type ItemArray = Shared<Box<[Option<item::Handle>]>>;

fn inner(item: &Handle) -> item::Handle {
    item.read().item.clone()
}
fn definition(item: &Handle) -> Result<Definition> {
    inner(item).read().definition.clone().ok_or(Error::Null)
}
fn enqueue(ctx: &mut impl Context, chunk: &HandleChunk, item: &Handle, kind: i32) -> Result<()> {
    chunk_sync::flag_with(
        chunk,
        Some(Shared::new(Flag::Item {
            item: item.clone(),
            kind,
            old_amount: 0,
        })),
        &mut |chunk| ctx.flagged(chunk),
    )
}

pub fn add_chunk(ctx: &mut impl Context, chunk: &HandleChunk, item: Option<Handle>) -> Result<()> {
    chunk_sync::get_items(chunk).write().push(item.clone());
    let item = item.ok_or(Error::Null)?;
    if ground::is_private(ctx, &item)? {
        let has_dropper = item.read().dropper.is_some();
        if has_dropper {
            let player = item.read().dropper.clone();
            ctx.packet(Packet::Construct, player, &item, 0)?;
        }
        return Ok(());
    }
    enqueue(ctx, chunk, &item, 0)
}

fn within(left: Location, right: Option<Location>) -> Result<bool> {
    let right = right.ok_or(Error::Null)?;
    if right.coordinates()[2] != left.coordinates()[2] {
        return Ok(false);
    }
    let x = left.coordinates()[0].wrapping_sub(right.coordinates()[0]);
    let y = left.coordinates()[1].wrapping_sub(right.coordinates()[1]);
    Ok((x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y)) as f64).sqrt() <= 15.0)
}
pub fn remove_chunk(
    ctx: &mut impl Context,
    chunk: &HandleChunk,
    item: Option<Handle>,
) -> Result<()> {
    let item = item.ok_or(Error::Null)?;
    let location = item.read().location.clone();
    if !chunk_sync::get_items(chunk)
        .write()
        .remove_first(Some(&item))
    {
        return Ok(());
    }
    if ground::is_private(ctx, &item)? {
        let has_dropper = item.read().dropper.is_some();
        if has_dropper {
            let player = item.read().dropper.clone().ok_or(Error::Null)?;
            if ctx.playing(&player)? {
                let player = item.read().dropper.clone().ok_or(Error::Null)?;
                let current = ctx.player_location(&player)?.ok_or(Error::Null)?;
                if within(current, location)? {
                    let player = item.read().dropper.clone();
                    ctx.packet(Packet::Clear, player, &item, 0)?;
                }
            }
        }
        return Ok(());
    }
    enqueue(ctx, chunk, &item, 1)
}

pub fn create(ctx: &mut impl Context, manager: &Manager, item: Option<Handle>) -> Result<Handle> {
    let item = item.ok_or(Error::Null)?;
    let definition = definition(&item)?;
    if !ctx.tradeable(&definition)? {
        item.write().remain_private = true;
    }
    let has_dropper = item.read().dropper.is_some();
    if has_dropper {
        let first = inner(&item).read().definition.clone();
        let has_plugin = match first {
            Some(def) => ctx.plugin(&def)?.is_some(),
            None => false,
        };
        if has_plugin {
            let second = inner(&item).read().definition.clone();
            let plugin = match second {
                Some(def) => ctx.plugin(&def)?,
                None => None,
            };
            let player = item.read().dropper.clone();
            ctx.plugin_remove(plugin.ok_or(Error::Null)?, player, &item, 1)?;
        }
    }
    item.write().removed = false;
    let location = item.read().location.clone().ok_or(Error::Null)?;
    let chunk = ctx.chunk(location)?;
    add_chunk(ctx, &chunk, Some(item.clone()))?;
    manager.items.write().push(Some(item.clone()));
    Ok(item)
}

pub fn create_at(
    ctx: &mut impl Context,
    manager: &Manager,
    input: Option<item::Handle>,
    location: Option<Location>,
    player: Option<NodeHandle>,
) -> Result<Handle> {
    let input = input.ok_or(Error::Null)?;
    let item = ground::construct(ctx, &input, location, 200, player)?;
    create(ctx, manager, Some(item))
}
pub fn create_player(
    ctx: &mut impl Context,
    manager: &Manager,
    input: Option<item::Handle>,
    player: Option<NodeHandle>,
) -> Result<Handle> {
    let player = player.ok_or(Error::Null)?;
    let location = ctx.player_location(&player)?;
    create_at(ctx, manager, input, location, Some(player))
}
pub fn create_uid(
    ctx: &mut impl Context,
    manager: &Manager,
    input: Option<item::Handle>,
    location: Option<Location>,
    uid: i32,
    ticks: i32,
) -> Result<Handle> {
    let input = input.ok_or(Error::Null)?;
    let item = ground::construct_uid(ctx, &input, location, uid, ticks)?;
    create(ctx, manager, Some(item))
}
pub fn create_many(
    ctx: &mut impl Context,
    manager: &Manager,
    items: Option<ItemArray>,
    location: Option<Location>,
    player: Option<NodeHandle>,
) -> Result<()> {
    let items = items.ok_or(Error::Null)?;
    let mut index = 0;
    while index < items.read().len() {
        let item = items.read()[index].clone().ok_or(Error::Null)?;
        create_at(ctx, manager, Some(item), location.clone(), player.clone())?;
        index += 1;
    }
    Ok(())
}
pub fn destroy(
    ctx: &mut impl Context,
    manager: &Manager,
    item: Option<Handle>,
) -> Result<Option<Handle>> {
    let Some(item) = item else {
        return Ok(None);
    };
    manager.items.write().remove_first(Some(&item));
    let location = item.read().location.clone().ok_or(Error::Null)?;
    let chunk = ctx.chunk(location)?;
    remove_chunk(ctx, &chunk, Some(item.clone()))?;
    if ctx.auto_spawn(&item)? {
        ctx.respawn(&item)?;
    }
    Ok(Some(item))
}
pub fn get(
    ctx: &mut impl Context,
    id: i32,
    location: Option<Location>,
    player: Option<NodeHandle>,
) -> Result<Option<Handle>> {
    let chunk = ctx.chunk(location.clone().ok_or(Error::Null)?)?;
    let items = chunk_sync::get_items(&chunk);
    let version = items.read().version();
    let mut cursor = 0;
    loop {
        let item = {
            let list = items.read();
            if cursor == list.len() {
                return Ok(None);
            }
            if version != list.version() {
                return Err(Error::ConcurrentModification);
            }
            list.get(cursor)
                .cloned()
                .ok_or(Error::Index)?
                .ok_or(Error::Null)?
        };
        cursor += 1;
        let id_matches = inner(&item).read().id() == id;
        if id_matches {
            let query = location.as_ref().ok_or(Error::Null)?;
            let matches = item
                .read()
                .location
                .as_ref()
                .is_some_and(|current| current.coordinates() == query.coordinates());
            let eligible = matches && !item.read().removed;
            if eligible {
                if !ground::is_private(ctx, &item)? {
                    return Ok(Some(item));
                }
                if let Some(ref player) = player {
                    if ground::dropped_by(ctx, &item, player)? {
                        return Ok(Some(item));
                    }
                }
            }
        }
    }
}
pub fn increase(
    ctx: &mut impl Context,
    manager: &Manager,
    input: Option<item::Handle>,
    location: Option<Location>,
    player: Option<NodeHandle>,
) -> Result<Handle> {
    let input = input.ok_or(Error::Null)?;
    let id = input.read().id();
    let existing = get(ctx, id, location.clone(), player.clone())?;
    if let Some(item) = existing {
        let owner = player.as_ref().ok_or(Error::Null)?;
        let eligible = ground::dropped_by(ctx, &item, owner)?
            && ground::is_private(ctx, &item)?
            && !item.read().removed;
        if eligible {
            let old_amount = inner(&item).read().amount();
            let amount = old_amount.wrapping_add(input.read().amount());
            inner(&item).write().set_amount(amount);
            ctx.packet(Packet::Amount, player, &item, old_amount)?;
            return Ok(item);
        }
    }
    create_at(ctx, manager, Some(input), location, player)
}

/// The original private packet is a fixed two-byte zone-origin frame followed
/// by an item frame in the same IoBuffer. Return two valid native Frames so the
/// normal bounded writer reproduces that stream without an invalid fixed size.
pub fn direct_frames(
    ctx: &mut impl chunk_sync::Context,
    kind: Packet,
    player: Option<NodeHandle>,
    item: &Handle,
    old_amount: i32,
) -> chunk_sync::Result<Vec<rs09_protocol::game::Frame>> {
    use rs09_protocol::{buffer::Writer, game::Frame};
    let location = item
        .read()
        .location
        .clone()
        .ok_or(chunk_sync::Error::Null)?;
    let [x, y, z] = location.coordinates();
    let base = Location::new(x & !7, y & !7, z);
    let player = player.ok_or(chunk_sync::Error::Null)?;
    let [x, y] = chunk_sync::scene_offset(ctx, &player, &base)?;
    let mut origin = Writer::new();
    origin.p1neg(x);
    origin.p1(y);
    let buffer = Shared::new(Frame {
        opcode: 26,
        payload: Vec::new(),
    });
    chunk_sync::write_item(
        &buffer,
        item,
        match kind {
            Packet::Construct => 0,
            Packet::Clear => 1,
            Packet::Amount => 2,
        },
        old_amount,
    )?;
    let mut data = buffer.read().payload.clone();
    let opcode = data.remove(0);
    Ok(vec![
        Frame {
            opcode: 26,
            payload: origin.into_vec(),
        },
        Frame {
            opcode,
            payload: data,
        },
    ])
}
pub fn pulse(ctx: &mut impl Context, manager: &Manager) -> Result<()> {
    let snapshot = manager.items.read().snapshot();
    for item in snapshot {
        let item = item.ok_or(Error::Null)?;
        if ctx.auto_spawn(&item)? {
            continue;
        }
        if !ground::is_active(ctx, &item)? {
            manager.items.write().remove_first(Some(&item));
            let has_dropper = item.read().dropper.is_some();
            if has_dropper {
                let player = item.read().dropper.clone().ok_or(Error::Null)?;
                if ctx.artificial(&player)? {
                    let player = item.read().dropper.clone().ok_or(Error::Null)?;
                    if let Some(items) = ctx.ai_items(&player)? {
                        items.write().remove_first(Some(&item));
                    }
                }
            }
            let removed = item.read().removed;
            if !removed {
                let location = item.read().location.clone().ok_or(Error::Null)?;
                let chunk = ctx.chunk(location)?;
                remove_chunk(ctx, &chunk, Some(item.clone()))?;
            }
        } else {
            let remain_private = item.read().remain_private;
            if !remain_private {
                let end = item.read().decay_time;
                if end.wrapping_sub(ctx.ticks()?) == 100 {
                    let location = item.read().location.clone().ok_or(Error::Null)?;
                    let chunk = ctx.chunk(location)?;
                    enqueue(ctx, &chunk, &item, 0)?;
                }
            }
        }
    }
    Ok(())
}
