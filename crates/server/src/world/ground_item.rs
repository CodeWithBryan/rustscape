//! Original GroundItem-owned state. Item/Node construction and player identity
//! come from the owning world; ground-item spawning/pickup scheduling is separate.
use super::{doors::Location, player_walk::NodeHandle};
use crate::{definitions::item::Shared, inventory::item};

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
pub type Handle = Shared<GroundItem>;
/// ArrayList identity/order, including nulls and duplicates. Structural writes
/// advance modCount even when clearing an empty list; replacement does not.
#[derive(Default)]
pub struct List {
    values: Vec<Option<Handle>>,
    version: i32,
}
impl From<Vec<Option<Handle>>> for List {
    fn from(values: Vec<Option<Handle>>) -> Self {
        Self { values, version: 0 }
    }
}
impl List {
    pub fn len(&self) -> usize {
        self.values.len()
    }
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
    pub fn iter(&self) -> std::slice::Iter<'_, Option<Handle>> {
        self.values.iter()
    }
    pub fn snapshot(&self) -> Vec<Option<Handle>> {
        self.values.clone()
    }
    pub fn version(&self) -> i32 {
        self.version
    }
    pub fn get(&self, index: usize) -> Option<&Option<Handle>> {
        self.values.get(index)
    }
    pub fn push(&mut self, item: Option<Handle>) {
        self.values.push(item);
        self.version = self.version.wrapping_add(1);
    }
    pub fn clear(&mut self) {
        self.values.clear();
        self.version = self.version.wrapping_add(1);
    }
    pub fn set(&mut self, index: usize, item: Option<Handle>) -> bool {
        if let Some(slot) = self.values.get_mut(index) {
            *slot = item;
            true
        } else {
            false
        }
    }
    pub fn remove_first(&mut self, item: Option<&Handle>) -> bool {
        let index = self
            .values
            .iter()
            .position(|candidate| match (candidate.as_ref(), item) {
                (Some(a), Some(b)) => a.same(b),
                (None, None) => true,
                _ => false,
            });
        if let Some(index) = index {
            self.values.remove(index);
            self.version = self.version.wrapping_add(1);
            true
        } else {
            false
        }
    }
}
pub struct GroundItem {
    pub item: item::Handle,
    /// Internal edge. Exposed tables attach the actual ground/subtype owner;
    /// storing an owner lease here would create a self-retaining cycle.
    pub interaction: Option<super::ground_options::Binding>,
    /// Inherited Node.size, independent of quantity and the item's definition.
    pub node_size: i32,
    pub location: Option<Location>,
    pub dropper: Option<NodeHandle>,
    pub dropper_uid: i32,
    pub ticks: i32,
    pub decay_time: i32,
    pub remain_private: bool,
    pub removed: bool,
    pub force_visible: bool,
}
/// Exposed Item views retain their owning ground subtype/location/dropper.
/// The internal Item stays unleased, avoiding a reference cycle.
pub fn as_item(item: &Handle) -> item::Handle {
    item.read().item.keeping_from(item)
}
pub trait Context {
    fn copy_item(&mut self, id: i32, amount: i32, charge: i32) -> Result<item::Handle>;
    fn default_interaction(&mut self, item: &Handle) -> Result<()>;
    fn uid(&mut self, player: &NodeHandle) -> Result<i32>;
    fn username(&mut self, player: &NodeHandle) -> Result<Option<String>>;
    fn ticks(&mut self) -> Result<i32>;
    /// Subtype dispatch is shared by manager updates and chunk/viewer reads.
    /// None continues into the original ordinary GroundItem body below.
    fn virtual_private(&mut self, _: &Handle) -> Result<Option<bool>> {
        Ok(None)
    }
    fn virtual_active(&mut self, _: &Handle) -> Result<Option<bool>> {
        Ok(None)
    }
}

pub fn construct(
    ctx: &mut impl Context,
    input: &item::Handle,
    location: Option<Location>,
    decay: i32,
    player: Option<NodeHandle>,
) -> Result<Handle> {
    construct_with(ctx, input, location, decay, player, |h| h)
}
/// Register subtype identity before the first callback that can expose this
/// reference. A failed base constructor can leave a live subtype with default
/// fields; the returned view must retain that owning subtype without a cycle.
pub(crate) fn construct_with(
    ctx: &mut impl Context,
    input: &item::Handle,
    location: Option<Location>,
    decay: i32,
    player: Option<NodeHandle>,
    wrap: impl FnOnce(Handle) -> Handle,
) -> Result<Handle> {
    let (id, amount, charge) = {
        let i = input.read();
        (i.id(), i.amount(), i.charge())
    };
    let item = ctx.copy_item(id, amount, charge)?;
    item.write().index = -1;
    let h = wrap(Shared::new(GroundItem {
        item,
        interaction: None,
        node_size: 1,
        location,
        dropper: None,
        dropper_uid: 0,
        ticks: 0,
        decay_time: 0,
        remain_private: false,
        removed: false,
        force_visible: false,
    }));
    ctx.default_interaction(&h)?;
    h.write().dropper = player.clone();
    let uid = match player {
        Some(ref p) => ctx.uid(p)?,
        None => -1,
    };
    h.write().dropper_uid = uid;
    let ticks = ctx.ticks()?;
    h.write().ticks = ticks;
    let end = h.read().ticks.wrapping_add(decay);
    h.write().decay_time = end;
    Ok(h)
}
/// The player-UID overload assigns its final argument as an absolute decay tick.
pub fn construct_uid(
    ctx: &mut impl Context,
    input: &item::Handle,
    location: Option<Location>,
    uid: i32,
    ticks: i32,
) -> Result<Handle> {
    let h = construct(ctx, input, location, 200, None)?;
    h.write().dropper_uid = uid;
    h.write().decay_time = ticks;
    Ok(h)
}
pub fn is_private(ctx: &mut impl Context, h: &Handle) -> Result<bool> {
    if let Some(value) = ctx.virtual_private(h)? {
        return Ok(value);
    }
    base_is_private(ctx, h)
}
pub fn base_is_private(ctx: &mut impl Context, h: &Handle) -> Result<bool> {
    if h.read().force_visible {
        return Ok(false);
    }
    if h.read().remain_private {
        return Ok(true);
    }
    let end = h.read().decay_time;
    Ok(end.wrapping_sub(ctx.ticks()?) > 100)
}
pub fn is_active(ctx: &mut impl Context, h: &Handle) -> Result<bool> {
    if let Some(value) = ctx.virtual_active(h)? {
        return Ok(value);
    }
    base_is_active(ctx, h)
}
pub fn base_is_active(ctx: &mut impl Context, h: &Handle) -> Result<bool> {
    if h.read().removed {
        return Ok(true);
    }
    let now = ctx.ticks()?;
    Ok(now < h.read().decay_time)
}
pub fn dropped_by(ctx: &mut impl Context, h: &Handle, p: &NodeHandle) -> Result<bool> {
    let uid = ctx.uid(p)?;
    let matches = uid == h.read().dropper_uid;
    if matches {
        h.write().dropper = Some(p.clone());
        Ok(true)
    } else {
        Ok(false)
    }
}
pub fn set_decay_time(ctx: &mut impl Context, h: &Handle, delay: i32) -> Result<()> {
    let end = ctx.ticks()?.wrapping_add(delay);
    h.write().decay_time = end;
    Ok(())
}
pub fn respawn(_: &Handle) {}
pub fn is_auto_spawn(_: &Handle) -> bool {
    false
}
pub fn java_string(ctx: &mut impl Context, h: &Handle) -> Result<String> {
    let dropper = h.read().dropper.clone();
    let name = match dropper {
        Some(p) => ctx.username(&p)?.unwrap_or_else(|| "null".into()),
        None => "null".into(),
    };
    let g = h.read();
    Ok(format!(
        "GroundItem [dropper={name}, ticks={}, decayTime={}, remainPrivate={}, removed={}]",
        g.ticks, g.decay_time, g.remain_private, g.removed
    ))
}
