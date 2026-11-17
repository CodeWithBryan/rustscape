//! Original movement-pulse orchestration and destination selection. Node/entity,
//! pathfinder, queue, content and packet services belong to the owning world.
mod destination;
mod pulse;
use super::{doors::Location, pathfinding, walking::Step};
use crate::{definitions::item::Shared, pulses};
pub use destination::Flag;
pub use pulse::{Constructor, Fields, Movement};
use rs09_game_core::direction::Direction;
use std::collections::VecDeque;
pub type Node = usize;
pub type Finder = usize;
pub const SMART: Finder = 0;
pub const DUMB: Finder = 1;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Null,
    Type,
    Index,
    NoSuchElement,
    Limit,
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Player,
    Npc,
    Entity,
    Scenery,
    Location,
    Other,
}
impl Kind {
    pub fn entity(self) -> bool {
        matches!(self, Self::Player | Self::Npc | Self::Entity)
    }
}
#[derive(Clone)]
pub struct WalkView {
    pub points: Shared<VecDeque<Step>>,
    pub running: bool,
    pub moving: bool,
    pub has_path: bool,
}
impl Default for WalkView {
    fn default() -> Self {
        Self {
            points: Shared::new(VecDeque::new()),
            running: false,
            moving: false,
            has_path: false,
        }
    }
}
#[derive(Clone)]
pub struct NodeView {
    pub kind: Kind,
    pub location: Option<Location>,
    pub size: i32,
    pub direction: Option<Direction>,
    pub active: bool,
    pub walking: WalkView,
    pub never_walks: bool,
    pub combat_running: bool,
    pub victim: Option<Node>,
    pub draw_intersect: bool,
    pub scenery_kind: i32,
    pub rotation: i32,
    pub dimensions: [i32; 2],
    pub walking_flag: i32,
}
impl NodeView {
    pub fn new(kind: Kind, location: Option<Location>) -> Self {
        Self {
            kind,
            location,
            size: 1,
            direction: Some(Direction::South),
            active: true,
            walking: WalkView::default(),
            never_walks: false,
            combat_running: false,
            victim: None,
            draw_intersect: false,
            scenery_kind: 10,
            rotation: 0,
            dimensions: [1, 1],
            walking_flag: 0,
        }
    }
}
pub struct Route {
    pub successful: bool,
    pub move_near: bool,
    pub points: Shared<VecDeque<Step>>,
}
pub type RouteHandle = Shared<Route>;
#[derive(Clone)]
pub enum Target {
    Node(Node),
    Location(Location),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DestinationService {
    Option(usize),
    UseWith(usize),
    Override(usize),
    Custom(usize),
}
/// Lookups return live field views. No node/route/base-pulse lock is held across
/// callbacks, which may mutate Movement, locations, paths or world state.
pub trait Context {
    fn clipping(&mut self, z: i32, x: i32, y: i32) -> Result<i32>;
    fn world_clipping(&mut self, z: i32, x: i32, y: i32) -> Result<i32>;
    fn node(&self, node: Node) -> Result<NodeView>;
    fn region(&mut self, location: &Location) -> Result<Option<bool>>;
    fn npc_finder(&mut self, node: Node) -> Result<Option<Finder>>;
    fn current_movement(&self, node: Node) -> Result<Option<usize>>;
    fn stop_previous(&mut self, movement: usize) -> Result<()>;
    fn assign_current(&mut self, node: Node, movement: usize) -> Result<()>;
    fn active(&mut self, movement: &mut Movement, node: Node) -> Result<bool>;
    fn scripts(&mut self, movement: &mut Movement, node: Node, weak: bool) -> Result<()>;
    fn face(&mut self, movement: &mut Movement, node: Node, target: Option<Node>) -> Result<()>;
    fn pulse(&mut self, movement: &mut Movement) -> Result<bool>;
    fn destination(
        &mut self,
        mover: Option<Node>,
        target: Option<Node>,
        service: DestinationService,
    ) -> Result<Option<Location>>;
    fn find(
        &mut self,
        movement: &mut Movement,
        target: Target,
        finder: Finder,
    ) -> Result<Option<RouteHandle>>;
    fn walk_reset(
        &mut self,
        movement: Option<&mut Movement>,
        node: Node,
        running: Option<bool>,
    ) -> Result<()>;
    fn walk_add(&mut self, movement: &mut Movement, node: Node, x: i32, y: i32) -> Result<()>;
    fn message(&mut self, movement: &mut Movement, node: Node, text: &str) -> Result<()>;
    fn clear_minimap(&mut self, movement: &mut Movement, node: Node) -> Result<()>;
    fn exception(&mut self, movement: &mut Movement, error: &Error) -> Result<()>;
    fn hint_clear(&mut self, movement: &mut Movement, node: Node) -> Result<()>;
    fn hint_add(
        &mut self,
        movement: &mut Movement,
        node: Node,
        location: Option<Location>,
        kind: i32,
    ) -> Result<()>;
    fn teleport_permitted(&mut self, location: &Location) -> Result<bool>;
    fn door_destination(
        &mut self,
        mover: Option<Node>,
        scenery: Option<Node>,
    ) -> Result<Option<Location>>;
    fn max_distance(&self) -> i32;
}
fn binding(e: pathfinding::Error) -> Error {
    match e {
        pathfinding::Error::Null => Error::Null,
        pathfinding::Error::Index => Error::Index,
        pathfinding::Error::Limit => Error::Limit,
        pathfinding::Error::Binding(s) => Error::Binding(s),
    }
}
fn rectangle(ctx: &mut impl Context, request: pathfinding::Reach, standing: bool) -> Result<bool> {
    struct Bridge<'a, C> {
        ctx: &'a mut C,
        error: Option<Error>,
    }
    impl<C: Context> pathfinding::Context for Bridge<'_, C> {
        fn clipping(&mut self, z: i32, x: i32, y: i32) -> pathfinding::Result<i32> {
            match self.ctx.clipping(z, x, y) {
                Ok(v) => Ok(v),
                Err(e) => {
                    self.error = Some(e);
                    Err(pathfinding::Error::Binding("movement clipping".into()))
                }
            }
        }
        fn world_clipping(&mut self, z: i32, x: i32, y: i32) -> pathfinding::Result<i32> {
            match self.ctx.world_clipping(z, x, y) {
                Ok(v) => Ok(v),
                Err(e) => {
                    self.error = Some(e);
                    Err(pathfinding::Error::Binding("movement clipping".into()))
                }
            }
        }
    }
    let mut bridge = Bridge { ctx, error: None };
    let result = if standing {
        request.standing(&mut bridge)
    } else {
        request.rectangle(&mut bridge)
    };
    if let Some(e) = bridge.error {
        return Err(e);
    }
    result.map_err(binding)
}
fn node(ctx: &impl Context, n: Option<Node>) -> Result<NodeView> {
    ctx.node(n.ok_or(Error::Null)?)
}
fn kind(ctx: &impl Context, n: Option<Node>) -> Result<Option<Kind>> {
    n.map(|n| ctx.node(n).map(|n| n.kind)).transpose()
}
fn location(ctx: &impl Context, n: Option<Node>) -> Result<Location> {
    node(ctx, n)?.location.ok_or(Error::Null)
}
fn transformed(l: &Location, dx: i32, dy: i32) -> Location {
    let [x, y, z] = l.coordinates();
    Location::new(x.wrapping_add(dx), y.wrapping_add(dy), z)
}
fn distance(a: &Location, b: &Location) -> f64 {
    let [x, y, _] = a.coordinates();
    let [xx, yy, _] = b.coordinates();
    let dx = x.wrapping_sub(xx);
    let dy = y.wrapping_sub(yy);
    (dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as f64).sqrt()
}
fn same(a: &Option<Location>, b: &Option<Location>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.same(b),
        _ => false,
    }
}
fn equal(a: &Option<Location>, b: &Option<Location>) -> bool {
    a.as_ref()
        .zip(b.as_ref())
        .is_some_and(|(a, b)| a.coordinates() == b.coordinates())
}
fn logical(a: &Location, b: &Location) -> Direction {
    let [x, y, _] = a.coordinates();
    let [xx, yy, _] = b.coordinates();
    Direction::logical_delta(xx.wrapping_sub(x), yy.wrapping_sub(y))
}
fn can_move(ctx: &mut impl Context, l: &Location, d: Direction) -> Result<bool> {
    let [x, y, z] = l.coordinates();
    let flag = ctx.clipping(z, x, y)?;
    Ok(d.traversal().iter().all(|&(_, _, mask)| flag & mask == 0))
}
fn transform_vector(l: &Location, x: f64, y: f64) -> Location {
    let [lx, ly, _] = l.coordinates();
    Location::new(
        lx.wrapping_add(x.floor() as i32),
        ly.wrapping_add(y.floor() as i32),
        0,
    )
}
fn between(a: &Location, b: &Location) -> [f64; 2] {
    let [x, y, _] = a.coordinates();
    let [xx, yy, _] = b.coordinates();
    [xx.wrapping_sub(x) as f64, yy.wrapping_sub(y) as f64]
}
fn magnitude(v: [f64; 2]) -> f64 {
    (v[0].powf(2.0) + v[1].powf(2.0)).sqrt()
}
/// Verbatim truncateLoc geometry: equality counts as truncated and vector
/// Location.transform resets plane to zero, including the zero-length case.
pub fn truncate(
    ctx: &impl Context,
    mover: Node,
    destination: &Location,
) -> Result<(bool, Location)> {
    let from = location(ctx, Some(mover))?;
    let vector = between(&from, destination);
    let mag = magnitude(vector);
    let multiplier = if ctx.node(mover)?.kind == Kind::Npc {
        14.0
    } else {
        ctx.max_distance() as f64
    };
    // Kotlin min propagates NaN; Rust f64::min does not.
    let clamped = if multiplier.is_nan() || mag.is_nan() {
        f64::NAN
    } else {
        multiplier.min(mag)
    };
    let from = location(ctx, Some(mover))?;
    Ok((
        multiplier == clamped,
        transform_vector(&from, vector[0] / mag * clamped, vector[1] / mag * clamped),
    ))
}
