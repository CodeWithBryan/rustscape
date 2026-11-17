use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    Location,
    Entity,
    FollowEntity,
    Combat,
    Item,
    Object,
    Custom(usize),
}
fn walk_direction(mut flag: i32, rotation: i32) -> Option<Direction> {
    if rotation != 0 {
        flag = (flag.wrapping_shl(rotation as u32) & 15)
            .wrapping_add(flag.wrapping_shr(4i32.wrapping_sub(rotation) as u32));
    }
    if flag > 0 {
        for (mask, d) in [
            (8, Direction::West),
            (2, Direction::East),
            (4, Direction::South),
            (1, Direction::North),
        ] {
            if flag & mask == 0 {
                return Some(d);
            }
        }
    }
    None
}
impl Flag {
    pub fn destination(
        self,
        ctx: &mut impl Context,
        mover: Option<Node>,
        target: Option<Node>,
    ) -> Result<Option<Location>> {
        match self {
            Self::Location => Ok(node(ctx, target)?.location),
            Self::Combat => Ok(None),
            Self::Custom(token) => {
                ctx.destination(mover, target, DestinationService::Custom(token))
            }
            Self::Entity => {
                let suggestion = transformed(&location(ctx, target)?, 0, -1);
                let mut l = self.closest(ctx, mover, target, suggestion)?;
                if node(ctx, mover)?.size > 1 {
                    if l.coordinates()[0] < location(ctx, target)?.coordinates()[0] {
                        l = transformed(
                            &l,
                            node(ctx, mover)?.size.wrapping_sub(1).wrapping_neg(),
                            0,
                        );
                    }
                    if l.coordinates()[1] < location(ctx, target)?.coordinates()[1] {
                        l = transformed(
                            &l,
                            0,
                            node(ctx, mover)?.size.wrapping_sub(1).wrapping_neg(),
                        );
                    }
                }
                Ok(Some(l))
            }
            Self::FollowEntity => {
                let dir = node(ctx, target)?.direction.ok_or(Error::Null)?;
                let (dx, dy) = dir.step();
                let mut l = transformed(
                    &location(ctx, target)?,
                    dx.wrapping_neg(),
                    dy.wrapping_neg(),
                );
                if !self.traversal(ctx, &l, dir)? {
                    l = self.closest(ctx, mover, target, l)?;
                }
                if node(ctx, mover)?.size > 1 {
                    if l.coordinates()[0] < location(ctx, target)?.coordinates()[0] {
                        l = transformed(
                            &l,
                            node(ctx, mover)?.size.wrapping_sub(1).wrapping_neg(),
                            0,
                        );
                    }
                    if l.coordinates()[1] < location(ctx, target)?.coordinates()[1] {
                        l = transformed(
                            &l,
                            0,
                            node(ctx, mover)?.size.wrapping_sub(1).wrapping_neg(),
                        );
                    }
                }
                Ok(Some(l))
            }
            Self::Item => {
                if !ctx.teleport_permitted(&location(ctx, target)?)? {
                    let suggestion = transformed(&location(ctx, target)?, 1, 0);
                    self.closest(ctx, mover, target, suggestion).map(Some)
                } else {
                    Ok(node(ctx, target)?.location)
                }
            }
            Self::Object => {
                let object = node(ctx, target)?;
                if object.kind != Kind::Scenery {
                    return Err(Error::Type);
                }
                if object.scenery_kind < 4 || node(ctx, target)?.scenery_kind == 9 {
                    return ctx.door_destination(mover, target);
                }
                if node(ctx, target)?.scenery_kind == 4 || node(ctx, target)?.scenery_kind == 5 {
                    return Ok(node(ctx, target)?.location);
                }
                let mut sx = node(ctx, target)?.dimensions[0];
                let mut sy = node(ctx, target)?.dimensions[1];
                if node(ctx, target)?.rotation % 2 != 0 {
                    std::mem::swap(&mut sx, &mut sy);
                }
                let d =
                    walk_direction(node(ctx, target)?.walking_flag, node(ctx, target)?.rotation);
                if let Some(d) = d {
                    return self.object(ctx, mover, target, [sx, sy], d, 3);
                }
                self.object(
                    ctx,
                    mover,
                    target,
                    [sx, sy],
                    logical(&location(ctx, target)?, &location(ctx, mover)?),
                    0,
                )
            }
        }
    }
    pub fn traversal(self, ctx: &mut impl Context, l: &Location, d: Direction) -> Result<bool> {
        if self == Self::Object {
            return Ok(ctx.teleport_permitted(l)? && can_move(ctx, l, d)?);
        }
        let d = Direction::from_code((d.code() + 2) % 4).ok_or(Error::Index)?;
        can_move(ctx, l, d)
    }
    pub fn closest(
        self,
        ctx: &mut impl Context,
        mover: Option<Node>,
        target: Option<Node>,
        mut suggestion: Location,
    ) -> Result<Location> {
        let nl = location(ctx, target)?;
        let [sx, sy, _] = suggestion.coordinates();
        let [nx, ny, _] = nl.coordinates();
        let dx = sx.wrapping_sub(nx);
        let dy = sy.wrapping_sub(ny);
        let mut d = if dx < 0 {
            Direction::East
        } else if dx >= node(ctx, target)?.size {
            Direction::West
        } else if dy >= node(ctx, target)?.size {
            Direction::South
        } else {
            Direction::North
        };
        let mut best = 9999.9;
        let mut result = suggestion.clone();
        let mut budget = 0usize;
        for _ in 0..4 {
            let mut i = 0i32;
            while i < node(ctx, target)?.size.wrapping_add(1) {
                for j in 0..if i == 0 { 1 } else { 2 } {
                    budget += 1;
                    if budget > 65_536 {
                        return Err(Error::Limit);
                    }
                    let current = Direction::from_code((d.code() + if j == 1 { 3 } else { 1 }) % 4)
                        .ok_or(Error::Index)?;
                    let (dx, dy) = current.step();
                    let l = transformed(&suggestion, dx.wrapping_mul(i), dy.wrapping_mul(i));
                    let [x, y, _] = l.coordinates();
                    let [nx, ny, _] = nl.coordinates();
                    if d.code() % 2 == 0 {
                        if x < nx || x > nx.wrapping_add(node(ctx, target)?.size).wrapping_sub(1) {
                            continue;
                        }
                    } else if y < ny || y > ny.wrapping_add(node(ctx, target)?.size).wrapping_sub(1)
                    {
                        continue;
                    }
                    if self.traversal(ctx, &l, d)? {
                        let dist = distance(&location(ctx, mover)?, &l);
                        if dist < best {
                            best = dist;
                            result = l;
                        }
                    }
                }
                i = i.wrapping_add(1);
            }
            d = Direction::from_code((d.code() + 1) % 4).ok_or(Error::Index)?;
            let (dx, dy) = d.step();
            let ox = dy.wrapping_mul(node(ctx, target)?.size >> 1).wrapping_abs();
            let oy = dx.wrapping_mul(node(ctx, target)?.size >> 1).wrapping_abs();
            suggestion = if d.code() < 2 {
                transformed(
                    &location(ctx, target)?,
                    dx.wrapping_neg().wrapping_add(ox),
                    dy.wrapping_neg().wrapping_add(oy),
                )
            } else {
                transformed(
                    &location(ctx, target)?,
                    dx.wrapping_neg()
                        .wrapping_mul(node(ctx, target)?.size)
                        .wrapping_add(ox),
                    dy.wrapping_neg()
                        .wrapping_mul(node(ctx, target)?.size)
                        .wrapping_add(oy),
                )
            };
        }
        Ok(result)
    }
    fn object(
        self,
        ctx: &mut impl Context,
        mover: Option<Node>,
        target: Option<Node>,
        size: [i32; 2],
        mut d: Direction,
        count: i32,
    ) -> Result<Option<Location>> {
        let mut closest = None;
        let mut best = 9999.9;
        let loc = location(ctx, target)?;
        let mut budget = 0usize;
        for _ in count..4 {
            let (dx, dy) = d.step();
            let length = if d.code() % 2 != 0 { size[1] } else { size[0] };
            for i in 0..length {
                budget += 1;
                if budget > 65_536 {
                    return Err(Error::Limit);
                }
                let (x, y) = if d.code() % 2 != 0 {
                    (if dx > 0 { dx.wrapping_mul(size[0]) } else { dx }, i)
                } else {
                    (i, if dy > 0 { dy.wrapping_mul(size[1]) } else { dy })
                };
                let l = transformed(&loc, x, y);
                if self.traversal(ctx, &l, d)? {
                    let dist = distance(&location(ctx, mover)?, &l);
                    if dist < best {
                        best = dist;
                        closest = Some(l);
                    }
                }
            }
            d = Direction::from_code((d.code() + 1) % 4).ok_or(Error::Index)?;
        }
        Ok(closest)
    }
}
