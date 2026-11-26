//! Native bindings between MovementPulse, Pathfinder dispatch, Smart/DumbPathfinder
//! and the canonical WalkingQueue. The owning world supplies node metadata and
//! collision/services; this module never substitutes a precomputed route.
use super::{
    doors::Location,
    movement::{self, NodeView, Route, RouteHandle, Target, WalkView},
    pathfinding::{
        self, Dumb, Error, Path, Result, Smart,
        dispatch::{self, Invocation, Supplier, View},
    },
    walking::{self, Point, Queue},
};
use crate::definitions::item::Shared;

pub trait Context {
    fn node(&self, node: usize) -> Result<View>;
    fn npc_supplier(&mut self, npc: usize) -> Result<Option<Supplier>>;
    fn teleport_permitted(&mut self, location: Option<Location>) -> Result<bool>;
    fn clipping(&mut self, supplier: Supplier, z: i32, x: i32, y: i32) -> Result<i32>;
    /// Sized reach deliberately bypasses the selected supplier in the source.
    fn world_clipping(&mut self, z: i32, x: i32, y: i32) -> Result<i32>;
}
pub struct Navigator {
    smart: Smart,
    dumb: Dumb,
    pub bfs: bool,
    pub max_distance: i32,
}
impl Default for Navigator {
    fn default() -> Self {
        Self {
            smart: Smart::default(),
            dumb: Dumb::default(),
            bfs: false,
            max_distance: 25,
        }
    }
}
impl Navigator {
    /// Original Entity/Node find overload, used by MovementPulse with near=true.
    /// SMART and DUMB preserve their distinct algorithms. Other finders fail explicitly.
    pub fn find<C: Context>(
        &mut self,
        ctx: &mut C,
        mover: Option<usize>,
        target: Target,
        finder: Option<usize>,
        near: bool,
    ) -> Result<Path> {
        self.search(ctx, mover, target, finder, near, false)
    }

    pub fn find_water<C: Context>(
        &mut self,
        ctx: &mut C,
        mover: usize,
        destination: Location,
    ) -> Result<Path> {
        self.search(
            ctx,
            Some(mover),
            Target::Location(destination),
            Some(dispatch::DUMB),
            true,
            true,
        )
    }

    fn search<C: Context>(
        &mut self,
        ctx: &mut C,
        mover: Option<usize>,
        target: Target,
        finder: Option<usize>,
        near: bool,
        water: bool,
    ) -> Result<Path> {
        let target_id = match &target {
            Target::Node(n) => Some(*n),
            Target::Location(_) => None,
        };
        // Only mover and destination are read by this overload. Select a private
        // location ID different from the mover, including at usize::MAX.
        let destination = target_id.unwrap_or_else(|| mover.map_or(0, |n| n.wrapping_add(1)));
        let mut bridge = Dispatch {
            ctx,
            navigator: self,
            location: match target {
                Target::Location(l) => Some((destination, l)),
                _ => None,
            },
        };
        if water {
            dispatch::entity_water(&mut bridge, mover, Some(destination), near, finder)
        } else {
            dispatch::entity(&mut bridge, mover, Some(destination), near, finder)
        }
    }
    pub fn movement<C: Context>(
        &mut self,
        ctx: &mut C,
        movement: &movement::Movement,
        target: Target,
        finder: usize,
    ) -> movement::Result<Option<RouteHandle>> {
        let path = self
            .find(ctx, movement.mover(), target, Some(finder), true)
            .map_err(movement_error)?;
        Ok(Some(Shared::new(Route {
            successful: path.successful,
            move_near: path.move_near,
            points: Shared::new(
                path.points
                    .into_iter()
                    .map(|[x, y]| Shared::new(Point::origin(x, y)))
                    .collect(),
            ),
        })))
    }
}
struct Dispatch<'a, C> {
    ctx: &'a mut C,
    navigator: &'a mut Navigator,
    location: Option<(usize, Location)>,
}
impl<C: Context> dispatch::Context for Dispatch<'_, C> {
    type Output = Path;
    fn node(&self, node: usize) -> Result<View> {
        if let Some((id, l)) = &self.location
            && node == *id
        {
            return Ok(View {
                kind: dispatch::Kind::Other,
                location: Some(l.clone()),
                size: 1,
                scenery: None,
            });
        }
        self.ctx.node(node)
    }
    fn npc_supplier(&mut self, npc: usize) -> Result<Option<Supplier>> {
        self.ctx.npc_supplier(npc)
    }
    fn teleport_permitted(&mut self, location: Option<Location>) -> Result<bool> {
        self.ctx.teleport_permitted(location)
    }
    fn invoke(&mut self, i: Invocation) -> Result<Path> {
        let r = i.request(self.navigator.bfs, self.navigator.max_distance);
        let mut collision = Collision {
            ctx: self.ctx,
            supplier: i.supplier,
        };
        match i.finder {
            None => Err(Error::Null),
            Some(dispatch::SMART) => self.navigator.smart.find(&mut collision, &r),
            Some(dispatch::DUMB) => self.navigator.dumb.find(&mut collision, &r),
            Some(f) => Err(Error::Binding(format!(
                "native pathfinder {f} is not ported"
            ))),
        }
    }
}
struct Collision<'a, C> {
    ctx: &'a mut C,
    supplier: Option<Supplier>,
}
impl<C: Context> pathfinding::Context for Collision<'_, C> {
    fn clipping(&mut self, z: i32, x: i32, y: i32) -> Result<i32> {
        self.ctx
            .clipping(self.supplier.ok_or(Error::Null)?, z, x, y)
    }
    fn world_clipping(&mut self, z: i32, x: i32, y: i32) -> Result<i32> {
        self.ctx.world_clipping(z, x, y)
    }
}
fn movement_error(e: Error) -> movement::Error {
    match e {
        Error::Null => movement::Error::Null,
        Error::Index => movement::Error::Index,
        Error::Limit => movement::Error::Limit,
        Error::Binding(s) => movement::Error::Binding(s),
    }
}
pub fn walking_error(e: walking::Error) -> movement::Error {
    match e {
        walking::Error::Null => movement::Error::Null,
        walking::Error::Type => movement::Error::Type,
        walking::Error::Limit => movement::Error::Limit,
        other => movement::Error::Binding(format!("walking: {other:?}")),
    }
}
/// Read all movement-facing queue fields from the same live queue handle.
pub fn view(mut node: NodeView, queue: &Queue) -> NodeView {
    let entity = queue.entity();
    node.location = entity.location;
    node.direction = entity.direction;
    node.walking = WalkView {
        points: queue.points(),
        running: queue.running(),
        moving: queue.is_moving(),
        has_path: queue.has_path(),
    };
    node
}
pub fn reset(
    ctx: &mut impl walking::Context,
    queue: &Queue,
    running: Option<bool>,
) -> movement::Result<()> {
    queue.clone().reset(ctx, running).map_err(walking_error)
}
pub fn add(ctx: &mut impl walking::Context, queue: &Queue, x: i32, y: i32) -> movement::Result<()> {
    queue
        .clone()
        .add_path(ctx, x, y, None)
        .map_err(walking_error)
}
