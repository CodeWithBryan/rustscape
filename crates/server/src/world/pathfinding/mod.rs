//! Original server route selection and reach rules; owning-world services remain
//! explicit. These are server collision masks, distinct from RT4 CollisionMap.
pub mod dispatch;
mod dumb;
pub mod projectile;
mod reach_generated;
mod smart;
pub use dumb::Dumb;
pub use smart::{Path, Request, Smart, State};
use std::num::Wrapping as I;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Null,
    Index,
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
pub trait Context {
    fn clipping(&mut self, z: i32, x: i32, y: i32) -> Result<i32>;
    /// The original sized reach helper uses RegionManager directly, even when a
    /// different ClipMaskSupplier was passed to its caller.
    fn world_clipping(&mut self, z: i32, x: i32, y: i32) -> Result<i32>;
}
#[derive(Clone, Copy, Debug)]
pub struct Reach {
    pub from: [i32; 3],
    pub mover: [i32; 2],
    pub to: [i32; 2],
    pub size: [i32; 2],
    pub kind: i32,
    pub rotation: i32,
    pub walking_flag: i32,
}
impl Reach {
    pub fn door(self, ctx: &mut impl Context) -> Result<bool> {
        let [x, y, z] = self.from;
        let [dx, dy] = self.to;
        reach_generated::canDoorInteract(
            I(x),
            I(y),
            I(self.mover[0]),
            I(dx),
            I(dy),
            I(self.kind),
            I(self.rotation),
            I(z),
            ctx,
        )
    }
    pub fn decoration(self, ctx: &mut impl Context) -> Result<bool> {
        let [x, y, z] = self.from;
        let [dx, dy] = self.to;
        reach_generated::canDecorationInteract(
            I(x),
            I(y),
            I(self.mover[0]),
            I(dx),
            I(dy),
            I(self.rotation),
            I(self.kind),
            I(z),
            ctx,
        )
    }
    pub fn standing(self, ctx: &mut impl Context) -> Result<bool> {
        let [x, y, _] = self.from;
        let [dx, dy] = self.to;
        let [sx, sy] = self.size;
        reach_generated::isStandingIn(
            I(x),
            I(y),
            I(self.mover[0]),
            I(self.mover[1]),
            I(dx),
            I(dy),
            I(sx),
            I(sy),
            ctx,
        )
    }
    pub fn rectangle(self, ctx: &mut impl Context) -> Result<bool> {
        let [x, y, z] = self.from;
        let [dx, dy] = self.to;
        let [sx, sy] = self.size;
        reach_generated::canInteract(
            I(x),
            I(y),
            I(self.mover[0]),
            I(dx),
            I(dy),
            I(sx),
            I(sy),
            I(self.walking_flag),
            I(z),
            ctx,
        )
    }
    pub fn sized(self, ctx: &mut impl Context) -> Result<bool> {
        let [x, y, z] = self.from;
        let [dx, dy] = self.to;
        let [sx, sy] = self.size;
        reach_generated::canInteractSized(
            I(x),
            I(y),
            I(self.mover[0]),
            I(self.mover[1]),
            I(dx),
            I(dy),
            I(sx),
            I(sy),
            I(self.walking_flag),
            I(z),
            ctx,
        )
    }
}
