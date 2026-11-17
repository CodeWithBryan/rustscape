//! Original Pathfinder.find overloads: destination geometry and supplier choice.
//! Node IDs retain their identity while a call is in progress. Metadata lookups
//! are ordinary field reads; supplier/teleport callbacks may mutate those fields.
use super::{Error, Request, Result};
use crate::world::doors::Location;

pub type Node = usize;
pub type Finder = usize;
pub const SMART: Finder = 0;
pub const DUMB: Finder = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Other,
    Entity,
    Npc,
    Scenery,
    GroundItem,
}
#[derive(Clone)]
pub struct Scenery {
    pub kind: i32,
    pub rotation: i32,
    /// None is the original null definition, rather than a default definition.
    pub definition: Option<Definition>,
}
#[derive(Clone, Copy)]
pub struct Definition {
    pub size: [i32; 2],
    pub walking_flag: i32,
}
#[derive(Clone)]
pub struct View {
    pub kind: Kind,
    pub location: Option<Location>,
    pub size: i32,
    pub scenery: Option<Scenery>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Supplier {
    World,
    Water,
    Custom(usize),
}
#[derive(Clone)]
pub struct Invocation {
    pub finder: Option<Finder>,
    pub supplier: Option<Supplier>,
    pub start: Option<Location>,
    pub end: Option<Location>,
    pub mover_size: i32,
    pub size: [i32; 2],
    pub rotation: i32,
    pub kind: i32,
    pub walking_flag: i32,
    pub near: bool,
}
impl Invocation {
    pub fn request(&self, bfs: bool, max_distance: i32) -> Request {
        Request {
            start: self.start.clone(),
            end: self.end.clone(),
            mover_size: self.mover_size,
            size: self.size,
            rotation: self.rotation,
            kind: self.kind,
            walking_flag: self.walking_flag,
            near: self.near,
            bfs,
            max_distance,
            has_supplier: self.supplier.is_some(),
        }
    }
}
pub trait Context {
    type Output;
    fn node(&self, node: Node) -> Result<View>;
    fn npc_supplier(&mut self, npc: Node) -> Result<Option<Supplier>>;
    fn teleport_permitted(&mut self, location: Option<Location>) -> Result<bool>;
    fn invoke(&mut self, invocation: Invocation) -> Result<Self::Output>;
}
fn view(ctx: &impl Context, n: Option<Node>) -> Result<View> {
    ctx.node(n.ok_or(Error::Null)?)
}

/// Entity overload with the original NPC behavior supplier and world fallback.
pub fn entity<C: Context>(
    ctx: &mut C,
    mover: Option<Node>,
    destination: Option<Node>,
    near: bool,
    finder: Option<Finder>,
) -> Result<C::Output> {
    let npc = mover
        .map(|n| ctx.node(n).map(|v| v.kind == Kind::Npc))
        .transpose()?
        .unwrap_or(false);
    let supplier = if npc {
        ctx.npc_supplier(mover.unwrap())?
    } else {
        None
    };
    entity_with_supplier(
        ctx,
        mover,
        destination,
        near,
        finder,
        Some(supplier.unwrap_or(Supplier::World)),
    )
}

pub fn entity_water<C: Context>(
    ctx: &mut C,
    mover: Option<Node>,
    destination: Option<Node>,
    near: bool,
    finder: Option<Finder>,
) -> Result<C::Output> {
    entity_with_supplier(ctx, mover, destination, near, finder, Some(Supplier::Water))
}

/// This overload bypasses NPC behavior even for an explicitly null supplier.
pub fn entity_with_supplier<C: Context>(
    ctx: &mut C,
    mover: Option<Node>,
    destination: Option<Node>,
    near: bool,
    finder: Option<Finder>,
    supplier: Option<Supplier>,
) -> Result<C::Output> {
    let start = view(ctx, mover)?.location;
    let size = view(ctx, mover)?.size;
    find(ctx, start, size, destination, near, finder, supplier)
}

pub fn location<C: Context>(
    ctx: &mut C,
    start: Option<Location>,
    destination: Option<Node>,
) -> Result<C::Output> {
    find(
        ctx,
        start,
        1,
        destination,
        true,
        Some(SMART),
        Some(Supplier::World),
    )
}

/// Main original Location/mover-size/Node overload. In particular, the walking
/// flag rotation uses Java shift masking, including unusual signed rotations.
pub fn find<C: Context>(
    ctx: &mut C,
    start: Option<Location>,
    mover_size: i32,
    destination: Option<Node>,
    near: bool,
    finder: Option<Finder>,
    supplier: Option<Supplier>,
) -> Result<C::Output> {
    let kind = destination
        .map(|n| ctx.node(n).map(|v| v.kind))
        .transpose()?;
    let (size, rotation, kind, walking_flag) = if kind == Some(Kind::Scenery) {
        let object = view(ctx, destination)?.scenery.ok_or(Error::Null)?;
        if matches!(object.kind, 10 | 11 | 22) {
            let definition = object.definition.ok_or(Error::Null)?;
            let [x, y] = definition.size;
            let size = if object.rotation % 2 != 0 {
                [y, x]
            } else {
                [x, y]
            };
            let mut flag = definition.walking_flag;
            if object.rotation != 0 {
                flag = (flag.wrapping_shl(object.rotation as u32) & 15)
                    .wrapping_add(flag.wrapping_shr(4_i32.wrapping_sub(object.rotation) as u32));
            }
            (size, 0, 0, flag)
        } else {
            ([0, 0], object.rotation, object.kind.wrapping_add(1), 0)
        }
    } else {
        let size = if matches!(kind, Some(Kind::Entity | Kind::Npc)) {
            view(ctx, destination)?.size
        } else if kind == Some(Kind::GroundItem)
            && !ctx.teleport_permitted(view(ctx, destination)?.location)?
        {
            1
        } else {
            0
        };
        ([size, size], 0, 0, 0)
    };
    // The target location is read after the ground-item callback, as in Java.
    let end = view(ctx, destination)?.location;
    ctx.invoke(Invocation {
        finder,
        supplier,
        start,
        end,
        mover_size,
        size,
        rotation,
        kind,
        walking_flag,
        near,
    })
}
