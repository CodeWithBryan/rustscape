//! Original singleton DumbPathfinder fields and traversal. Shared atomic fields
//! allow callback reentry without holding a lock; the owning world orders calls.
use super::{Context, Error, Path, Reach, Request, Result};
use crate::world::doors::Location;
use rs09_game_core::direction::Direction;
use std::{
    num::Wrapping as I,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicI32, Ordering::Relaxed},
    },
};
#[path = "dumb_generated.rs"]
mod generated;
#[derive(Default)]
pub struct Fields {
    pub x: AtomicI32,
    pub y: AtomicI32,
    pub z: AtomicI32,
    pub found: AtomicBool,
}
#[derive(Clone, Default)]
pub struct Dumb {
    pub fields: Arc<Fields>,
}
impl Dumb {
    pub fn state(&self) -> [i32; 4] {
        [self.x().0, self.y().0, self.z().0, i32::from(self.found())]
    }
    fn x(&self) -> I<i32> {
        I(self.fields.x.load(Relaxed))
    }
    fn y(&self) -> I<i32> {
        I(self.fields.y.load(Relaxed))
    }
    fn z(&self) -> I<i32> {
        I(self.fields.z.load(Relaxed))
    }
    fn found(&self) -> bool {
        self.fields.found.load(Relaxed)
    }
    fn set_found(&self, b: bool) {
        self.fields.found.store(b, Relaxed);
    }
    fn bump_x(&self, n: i32) {
        self.fields.x.store(self.x().0.wrapping_add(n), Relaxed);
    }
    fn bump_y(&self, n: i32) {
        self.fields.y.store(self.y().0.wrapping_add(n), Relaxed);
    }
    pub fn find(&self, ctx: &mut impl Context, r: &Request) -> Result<Path> {
        let start = r.start.as_ref().ok_or(Error::Null)?;
        self.fields.z.store(start.coordinates()[2], Relaxed);
        self.fields.x.store(start.coordinates()[0], Relaxed);
        self.fields.y.store(start.coordinates()[1], Relaxed);
        let end = r.end.as_ref().ok_or(Error::Null)?;
        let mut path = Path {
            successful: true,
            move_near: false,
            points: Vec::with_capacity(20),
        };
        let mut ctx = Supplier {
            ctx,
            present: r.has_supplier,
        };
        let mut steps = 0;
        while self.x().0 != end.coordinates()[0] || self.y().0 != end.coordinates()[1] {
            steps += 1;
            if steps > 65_536 {
                return Err(Error::Limit);
            }
            let (mut directions, len) = directions(self.x().0, self.y().0, end);
            let reach = || Reach {
                from: [self.x().0, self.y().0, self.z().0],
                mover: [r.mover_size; 2],
                to: [end.coordinates()[0], end.coordinates()[1]],
                size: r.size,
                kind: r.kind.wrapping_sub(1),
                rotation: r.rotation,
                walking_flag: r.walking_flag,
            };
            if r.kind != 0 {
                if (r.kind < 5 || r.kind == 10) && reach().door(&mut ctx)? {
                    break;
                }
                if r.kind < 10
                    && (Reach {
                        kind: r.rotation,
                        rotation: r.kind.wrapping_sub(1),
                        ..reach()
                    })
                    .decoration(&mut ctx)?
                {
                    break;
                }
            }
            if r.size[0] != 0 && r.size[1] != 0 {
                if reach().rectangle(&mut ctx)? {
                    break;
                }
                if len > 1 {
                    let (dx, dy) = directions[0].step();
                    if self.x().0.wrapping_add(dx) == end.coordinates()[0]
                        && self.y().0.wrapping_add(dy) == end.coordinates()[1]
                    {
                        directions.swap(0, len - 1);
                    }
                }
            }
            self.set_found(true);
            match r.mover_size.cmp(&2) {
                std::cmp::Ordering::Less => {
                    self.single(&mut ctx, &mut path.points, &directions[..len], r.mover_size)?
                }
                std::cmp::Ordering::Equal => {
                    self.double(&mut ctx, &mut path.points, &directions[..len], r.mover_size)?
                }
                std::cmp::Ordering::Greater => {
                    self.variable(&mut ctx, &mut path.points, &directions[..len], r.mover_size)?
                }
            }
            if !self.found() {
                path.move_near =
                    self.x().0 != start.coordinates()[0] || self.y().0 != start.coordinates()[1];
                path.successful = false;
                break;
            }
        }
        Ok(path)
    }
}
fn directions(x: i32, y: i32, end: &Location) -> ([Direction; 3], usize) {
    use Direction::*;
    let [ex, ey, _] = end.coordinates();
    let list = if x == ex {
        match y.cmp(&ey) {
            std::cmp::Ordering::Greater => [South, South, South],
            std::cmp::Ordering::Less => [North, North, North],
            std::cmp::Ordering::Equal => return ([North; 3], 0),
        }
    } else if y == ey {
        if x > ex { [West; 3] } else { [East; 3] }
    } else if x < ex && y < ey {
        [NorthEast, East, North]
    } else if x < ex && y > ey {
        [SouthEast, East, South]
    } else if x > ex && y < ey {
        [NorthWest, West, North]
    } else {
        [SouthWest, West, South]
    };
    (list, if x == ex || y == ey { 1 } else { 3 })
}
struct Supplier<'a, C> {
    ctx: &'a mut C,
    present: bool,
}
impl<C: Context> Context for Supplier<'_, C> {
    fn clipping(&mut self, z: i32, x: i32, y: i32) -> Result<i32> {
        if !self.present {
            return Err(Error::Null);
        }
        self.ctx.clipping(z, x, y)
    }
    fn world_clipping(&mut self, z: i32, x: i32, y: i32) -> Result<i32> {
        self.ctx.world_clipping(z, x, y)
    }
}
