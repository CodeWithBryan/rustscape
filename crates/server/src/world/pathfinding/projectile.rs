//! Revision 530 projectile traversal and the combat footprint check.
use super::{Error, Result};
use crate::world::doors::Location;
use rs09_game_core::direction::Direction::{self, *};

pub trait Context {
    fn flag(&mut self, z: i32, x: i32, y: i32, projectile: bool) -> Result<i32>;
}

pub trait Footprint {
    fn size(&self) -> Result<i32>;
    fn location(&self) -> Result<Option<Location>>;
}

impl Footprint for Location {
    fn size(&self) -> Result<i32> {
        Ok(1)
    }

    fn location(&self) -> Result<Option<Location>> {
        Ok(Some(self.clone()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
    pub direction: Direction,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Path {
    pub successful: bool,
    pub move_near: bool,
    pub points: Vec<Point>,
}

pub fn find(
    ctx: &mut impl Context,
    start: Option<&Location>,
    end: Option<&Location>,
) -> Result<Path> {
    let mut points = Vec::new();
    let run = traverse(ctx, start, end, |point| points.push(point))?;
    Ok(Path {
        successful: run.successful,
        move_near: run.move_near,
        points,
    })
}

/// The original tries every tile pair in x-then-y order and stops at the
/// first qualifying path. Each range takes its size at entry. The path uses
/// the source plane, including when the destination is on another plane.
pub fn can_reach(
    ctx: &mut impl Context,
    source: &impl Footprint,
    victim: Option<&dyn Footprint>,
    check_close: bool,
) -> Result<bool> {
    for x1 in 0..source.size()? {
        for y1 in 0..source.size()? {
            let from = offset(source, x1, y1)?;
            let victim = victim.ok_or(Error::Null)?;
            for x2 in 0..victim.size()? {
                for y2 in 0..victim.size()? {
                    let to = offset(victim, x2, y2)?;
                    // Combat needs only the path result and length. The same
                    // traversal can emit Points for callers that need the path.
                    let path = traverse(ctx, Some(&from), Some(&to), |_| {})?;
                    if path.successful && (!check_close || path.steps <= 1) {
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}

fn offset(node: &(impl Footprint + ?Sized), dx: i32, dy: i32) -> Result<Location> {
    let [x, y, z] = node.location()?.ok_or(Error::Null)?.coordinates();
    Ok(Location::new(x.wrapping_add(dx), y.wrapping_add(dy), z))
}

struct Traversal {
    successful: bool,
    move_near: bool,
    steps: usize,
}

fn traverse(
    ctx: &mut impl Context,
    start: Option<&Location>,
    end: Option<&Location>,
    mut point: impl FnMut(Point),
) -> Result<Traversal> {
    let start = start.ok_or(Error::Null)?;
    let [mut x, mut y, z] = start.coordinates();
    let end = end.ok_or(Error::Null)?;
    let mut run = Traversal {
        successful: true,
        move_near: false,
        steps: 0,
    };
    // Any fixed pair of packed locations takes at most 65534 iterations.
    // A caller mutating the destination in a clip callback cannot spin forever.
    for _ in 0..65536 {
        let [ex, ey, _] = end.coordinates();
        if (x, y) == (ex, ey) {
            return Ok(run);
        }
        let [ex, ey, _] = end.coordinates();
        if (x, y) == (ex, ey) {
            continue;
        }
        // getDirection lists two additional choices for diagonals, but the
        // labeled break on collision and the break on success both leave the
        // loop after its first direction. No cardinal fallback is attempted.
        let direction = Direction::from_delta(ex - x, ey - y);
        let (dx, dy) = direction.step();
        let (nx, ny) = (x + dx, y + dy);
        const EAST: i32 = 0x12c0180;
        const WEST: i32 = 0x12c0108;
        const NORTH: i32 = 0x12c0120;
        const SOUTH: i32 = 0x12c0102;
        let mask = match direction {
            North => NORTH,
            NorthEast => 0x12c01e0,
            East => EAST,
            SouthEast => 0x12c0183,
            South => SOUTH,
            SouthWest => 0x12c010e,
            West => WEST,
            NorthWest => 0x12c0138,
        };
        if (dx != 0
            && dy != 0
            && (flagged(ctx, z, nx, y, if dx > 0 { EAST } else { WEST })?
                || flagged(ctx, z, x, ny, if dy > 0 { NORTH } else { SOUTH })?))
            || flagged(ctx, z, nx, ny, mask)?
        {
            let [sx, sy, _] = start.coordinates();
            run.successful = false;
            run.move_near = (x, y) != (sx, sy);
            return Ok(run);
        }
        (x, y) = (nx, ny);
        point(Point { x, y, direction });
        run.steps += 1;
    }
    Err(Error::Limit)
}

fn flagged(ctx: &mut impl Context, z: i32, x: i32, y: i32, mask: i32) -> Result<bool> {
    const PROJECTILE_BLOCK: i32 = 0x20000;
    Ok(ctx.flag(z, x, y, true)? & (mask | PROJECTILE_BLOCK) != 0
        || ctx.flag(z, x, y, false)? & PROJECTILE_BLOCK != 0)
}
