//! Original SmartPathfinder search order, heap ties, fallback and backtracking.
use super::{Context, Error, Reach, Result};
use crate::world::doors::Location;
use rs09_game_core::direction::Direction;
#[derive(Clone)]
pub struct Request {
    pub start: Option<Location>,
    pub end: Option<Location>,
    pub mover_size: i32,
    pub size: [i32; 2],
    pub rotation: i32,
    pub kind: i32,
    pub walking_flag: i32,
    pub near: bool,
    pub bfs: bool,
    pub max_distance: i32,
    pub has_supplier: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Path {
    pub successful: bool,
    pub move_near: bool,
    pub points: Vec<[i32; 2]>,
}
pub struct State<'a> {
    pub current: [i32; 2],
    pub destination: [i32; 2],
    pub write_position: usize,
    pub found: bool,
    pub queue_x: &'a [i32],
    pub queue_y: &'a [i32],
    pub via: &'a [i32],
    pub cost: &'a [i32],
}
pub struct Smart {
    queue_x: Vec<i32>,
    queue_y: Vec<i32>,
    via: Vec<i32>,
    cost: Vec<i32>,
    write: usize,
    current: [i32; 2],
    destination: [i32; 2],
    found: bool,
}
impl Default for Smart {
    fn default() -> Self {
        Self {
            queue_x: vec![0],
            queue_y: vec![0],
            via: vec![0; 104 * 104],
            cost: vec![0; 104 * 104],
            write: 0,
            current: [0; 2],
            destination: [0; 2],
            found: false,
        }
    }
}
fn index(x: i32, y: i32) -> Result<usize> {
    if (0..104).contains(&x) && (0..104).contains(&y) {
        Ok(x as usize * 104 + y as usize)
    } else {
        Err(Error::Index)
    }
}
fn scene(v: i32) -> i32 {
    v.wrapping_sub((v >> 3).wrapping_sub(6).wrapping_shl(3))
}
fn flag(d: Direction) -> i32 {
    match d {
        Direction::South => 1,
        Direction::West => 2,
        Direction::North => 4,
        Direction::East => 8,
        Direction::SouthWest => 3,
        Direction::NorthWest => 6,
        Direction::SouthEast => 9,
        Direction::NorthEast => 12,
    }
}
const DIRECTIONS: [Direction; 8] = [
    Direction::South,
    Direction::West,
    Direction::North,
    Direction::East,
    Direction::SouthWest,
    Direction::NorthWest,
    Direction::SouthEast,
    Direction::NorthEast,
];
struct ProbeList {
    values: [(i32, i32, i32); 3],
    len: usize,
}
impl ProbeList {
    fn new<const N: usize>(values: [(i32, i32, i32); N]) -> Self {
        let mut output = Self {
            values: [(0, 0, 0); 3],
            len: N,
        };
        output.values[..N].copy_from_slice(&values);
        output
    }
}
impl IntoIterator for ProbeList {
    type Item = (i32, i32, i32);
    type IntoIter = std::iter::Take<std::array::IntoIter<Self::Item, 3>>;
    fn into_iter(self) -> Self::IntoIter {
        self.values.into_iter().take(self.len)
    }
}
/// Java PriorityQueue's equal-priority sift rules are observable in chosen paths.
struct Heap {
    values: Vec<u32>,
    end: Location,
}
impl Heap {
    fn score(&self, p: u32) -> i32 {
        let [x, y, _] = self.end.coordinates();
        ((p >> 16) & 255) as i32
            + (scene(x).wrapping_sub(((p >> 8) & 255) as i32))
                .wrapping_abs()
                .max((scene(y).wrapping_sub((p & 255) as i32)).wrapping_abs())
    }
    fn push(&mut self, p: u32) {
        let mut k = self.values.len();
        self.values.push(p);
        while k > 0 {
            let parent = (k - 1) >> 1;
            let e = self.values[parent];
            if self.score(p) >= self.score(e) {
                break;
            }
            self.values[k] = e;
            k = parent;
        }
        self.values[k] = p;
    }
    fn pop(&mut self) -> Option<u32> {
        let out = *self.values.first()?;
        let last = self.values.pop().unwrap();
        let size = self.values.len();
        if size > 0 {
            let mut k = 0;
            while k < size / 2 {
                let mut child = 2 * k + 1;
                let right = child + 1;
                if right < size && self.score(self.values[child]) > self.score(self.values[right]) {
                    child = right;
                }
                if self.score(last) <= self.score(self.values[child]) {
                    break;
                }
                self.values[k] = self.values[child];
                k = child;
            }
            self.values[k] = last;
        }
        Some(out)
    }
}
impl Smart {
    pub fn state(&self) -> State<'_> {
        State {
            current: self.current,
            destination: self.destination,
            write_position: self.write,
            found: self.found,
            queue_x: &self.queue_x,
            queue_y: &self.queue_y,
            via: &self.via,
            cost: &self.cost,
        }
    }
    pub fn reset(&mut self) {
        self.queue_x.resize(4096, 0);
        self.queue_y.resize(4096, 0);
        self.queue_x.fill(0);
        self.queue_y.fill(0);
        self.via.fill(0);
        self.cost.fill(0);
        self.write = 0;
    }
    pub fn check(&mut self, x: i32, y: i32, direction: i32, cost: i32, penalty: i32) -> Result<()> {
        let i = index(x, y)?;
        let cost = cost.wrapping_add(penalty);
        if self.cost[i] > cost {
            *self.queue_x.get_mut(self.write).ok_or(Error::Index)? = x;
            *self.queue_y.get_mut(self.write).ok_or(Error::Index)? = y;
            self.via[i] = direction;
            self.cost[i] = cost;
            self.write = (self.write + 1) & 4095;
        }
        Ok(())
    }
    fn reached(
        &mut self,
        ctx: &mut impl Context,
        request: &Request,
        end: &Location,
        base: [i32; 3],
        size: i32,
    ) -> Result<bool> {
        if self.current == self.destination {
            return Ok(true);
        }
        let [x, y] = self.current;
        let [dx, dy, _] = end.coordinates();
        let reach = Reach {
            from: [base[0].wrapping_add(x), base[1].wrapping_add(y), base[2]],
            mover: [size, size],
            to: [dx, dy],
            size: request.size,
            kind: request.kind.wrapping_sub(1),
            rotation: request.rotation,
            walking_flag: request.walking_flag,
        };
        if request.kind != 0 {
            if (request.kind < 5 || request.kind == 10) && reach.door(ctx)? {
                return Ok(true);
            }
            // Original declaration names rotation before type, while this caller
            // passes type-1 then rotation. Preserve that argument ordering.
            if request.kind < 10
                && (Reach {
                    kind: request.rotation,
                    rotation: request.kind.wrapping_sub(1),
                    ..reach
                })
                .decoration(ctx)?
            {
                return Ok(true);
            }
        }
        if request.size[0] != 0 && request.size[1] != 0 && reach.rectangle(ctx)? {
            return Ok(true);
        }
        Ok(false)
    }
    fn astar(
        &mut self,
        ctx: &mut impl Context,
        r: &Request,
        end: &Location,
        base: [i32; 3],
    ) -> Result<()> {
        let mut heap = Heap {
            values: Vec::with_capacity(4096),
            end: end.clone(),
        };
        heap.push((self.current[0] as u32) << 8 | self.current[1] as u32);
        while !self.found {
            let Some(p) = heap.pop() else {
                break;
            };
            let current_cost = ((p >> 16) & 255) as i32;
            self.current = [((p >> 8) & 255) as i32, (p & 255) as i32];
            if self.reached(ctx, r, end, base, 1)? {
                self.found = true;
                break;
            }
            let abs = [
                base[0].wrapping_add(self.current[0]),
                base[1].wrapping_add(self.current[1]),
            ];
            let new_cost = current_cost + 1;
            for d in DIRECTIONS {
                let (dx, dy) = d.step();
                let x = self.current[0] + dx;
                let y = self.current[1] + dy;
                let Ok(i) = index(x, y) else {
                    continue;
                };
                if self.via[i] != 0 {
                    continue;
                }
                let mut allowed = true;
                // Direction.canMoveFrom evaluates every traversal mask even
                // after one blocks; supplier failures still propagate.
                for &(qx, qy, mask) in d.traversal() {
                    if ctx.clipping(base[2], abs[0].wrapping_add(qx), abs[1].wrapping_add(qy))?
                        & mask
                        != 0
                    {
                        allowed = false;
                    }
                }
                if allowed {
                    let penalty = dx.abs() + dy.abs() - 1;
                    let direction = flag(d);
                    self.check(x, y, direction, new_cost, penalty)?;
                    if self.via[i] == direction {
                        heap.push(((new_cost + penalty) as u32) << 16 | (x as u32) << 8 | y as u32);
                    }
                }
            }
        }
        Ok(())
    }
    fn single_step(
        &mut self,
        ctx: &mut impl Context,
        base: [i32; 3],
        d: Direction,
        cost: i32,
    ) -> Result<()> {
        let (dx, dy) = d.step();
        let [x, y] = self.current;
        let Ok(i) = index(x + dx, y + dy) else {
            return Ok(());
        };
        if self.via[i] != 0 {
            return Ok(());
        }
        let mask = match d {
            Direction::South => 0x12c0102,
            Direction::West => 0x12c0108,
            Direction::North => 0x12c0120,
            Direction::East => 0x12c0180,
            Direction::SouthWest => 0x12c010e,
            Direction::NorthWest => 0x12c0138,
            Direction::SouthEast => 0x12c0183,
            Direction::NorthEast => 0x12c01e0,
        };
        let ax = base[0].wrapping_add(x);
        let ay = base[1].wrapping_add(y);
        if ctx.clipping(base[2], ax.wrapping_add(dx), ay.wrapping_add(dy))? & mask != 0 {
            return Ok(());
        }
        if dx != 0 && dy != 0 {
            if ctx.clipping(base[2], ax.wrapping_add(dx), ay)?
                & (if dx < 0 { 0x12c0108 } else { 0x12c0180 })
                != 0
            {
                return Ok(());
            }
            if ctx.clipping(base[2], ax, ay.wrapping_add(dy))?
                & (if dy < 0 { 0x12c0102 } else { 0x12c0120 })
                != 0
            {
                return Ok(());
            }
        }
        self.check(x + dx, y + dy, flag(d), cost, 0)
    }
    fn sized_step(
        &mut self,
        ctx: &mut impl Context,
        base: [i32; 3],
        d: Direction,
        cost: i32,
        size: i32,
    ) -> Result<()> {
        let (dx, dy) = d.step();
        let [x, y] = self.current;
        if (dx < 0 && x <= 0) || (dy < 0 && y <= 0) || (dx > 0 && x >= 102) || (dy > 0 && y >= 102)
        {
            return Ok(());
        }
        let i = index(x + dx, y + dy)?;
        if self.via[i] != 0 {
            return Ok(());
        }
        let s = size;
        let m = s.wrapping_sub(1);
        let p = s.wrapping_sub(2);
        let probes: ProbeList = if size == 2 {
            match d {
                Direction::South => ProbeList::new([(0, -1, 0x12c010e), (1, -1, 0x12c0183)]),
                Direction::West => ProbeList::new([(-1, 0, 0x12c010e), (-1, 1, 0x12c0138)]),
                Direction::North => ProbeList::new([(0, 2, 0x12c0138), (1, 2, 0x12c01e0)]),
                Direction::East => ProbeList::new([(2, 0, 0x12c0183), (2, 1, 0x12c01e0)]),
                Direction::SouthWest => {
                    ProbeList::new([(-1, -1, 0x12c010e), (-1, 0, 0x12c0138), (0, -1, 0x12c0183)])
                }
                Direction::NorthWest => {
                    ProbeList::new([(-1, 1, 0x12c010e), (-1, 2, 0x12c0138), (0, 2, 0x12c01e0)])
                }
                Direction::SouthEast => {
                    ProbeList::new([(1, -1, 0x12c010e), (2, 0, 0x12c01e0), (2, -1, 0x12c0183)])
                }
                Direction::NorthEast => {
                    ProbeList::new([(1, 2, 0x12c0138), (2, 2, 0x12c01e0), (2, 1, 0x12c0183)])
                }
            }
        } else {
            match d {
                Direction::South => ProbeList::new([(0, -1, 0x12c010e), (m, -1, 0x12c0183)]),
                Direction::West => ProbeList::new([(-1, 0, 0x12c010e), (-1, m, 0x12c0138)]),
                Direction::North => ProbeList::new([(0, s, 0x12c0138), (m, s, 0x12c01e0)]),
                Direction::East => ProbeList::new([(s, 0, 0x12c0183), (s, m, 0x12c01e0)]),
                Direction::SouthWest => {
                    ProbeList::new([(-1, p, 0x12c0138), (-1, -1, 0x12c010e), (p, -1, 0x12c0183)])
                }
                Direction::NorthWest => {
                    ProbeList::new([(-1, 1, 0x12c010e), (-1, s, 0x12c0138), (0, s, 0x12c01e0)])
                }
                Direction::SouthEast => {
                    ProbeList::new([(1, -1, 0x12c010e), (s, -1, 0x12c0183), (s, p, 0x12c01e0)])
                }
                Direction::NorthEast => {
                    ProbeList::new([(1, s, 0x12c0138), (s, s, 0x12c01e0), (s, 1, 0x12c0183)])
                }
            }
        };
        let ax = base[0].wrapping_add(x);
        let ay = base[1].wrapping_add(y);
        for (qx, qy, mask) in probes {
            if ctx.clipping(base[2], ax.wrapping_add(qx), ay.wrapping_add(qy))? & mask != 0 {
                return Ok(());
            }
        }
        if size > 2 {
            if size > 65_536 {
                return Err(Error::Limit);
            }
            for j in 1..size - 1 {
                let tests: ProbeList = match d {
                    Direction::South => ProbeList::new([(j, -1, 0x12c018f)]),
                    Direction::West => ProbeList::new([(-1, j, 0x12c013e)]),
                    Direction::North => ProbeList::new([(j, s, 0x12c01f8)]),
                    Direction::East => ProbeList::new([(s, j, 0x12c01e3)]),
                    Direction::SouthWest => {
                        ProbeList::new([(-1, j - 1, 0x12c013e), (j - 1, -1, 0x12c018f)])
                    }
                    Direction::NorthWest => {
                        ProbeList::new([(-1, j + 1, 0x12c013e), (j - 1, s, 0x12c01f8)])
                    }
                    Direction::SouthEast => {
                        ProbeList::new([(s, j - 1, 0x12c01e3), (j + 1, -1, 0x12c018f)])
                    }
                    Direction::NorthEast => {
                        ProbeList::new([(j + 1, s, 0x12c01f8), (s, j + 1, 0x12c01e3)])
                    }
                };
                for (qx, qy, mask) in tests {
                    if ctx.clipping(base[2], ax.wrapping_add(qx), ay.wrapping_add(qy))? & mask != 0
                    {
                        return Ok(());
                    }
                }
            }
        }
        self.check(x + dx, y + dy, flag(d), cost, 0)
    }
    fn bfs(
        &mut self,
        ctx: &mut impl Context,
        r: &Request,
        end: &Location,
        base: [i32; 3],
    ) -> Result<()> {
        let mut read = 0;
        while self.write != read {
            self.current = [self.queue_x[read], self.queue_y[read]];
            read = (read + 1) & 4095;
            if self.current == self.destination {
                self.found = true;
                break;
            }
            let iteration = |this: &mut Self, ctx: &mut _| -> Result<bool> {
                if this.reached(ctx, r, end, base, r.mover_size.max(1))? {
                    return Ok(true);
                }
                let cost = this.cost[index(this.current[0], this.current[1])?].wrapping_add(1);
                for d in DIRECTIONS {
                    if r.mover_size < 2 {
                        this.single_step(ctx, base, d, cost)?;
                    } else {
                        this.sized_step(ctx, base, d, cost, r.mover_size)?;
                    }
                }
                Ok(false)
            };
            match iteration(self, ctx) {
                Ok(true) => {
                    self.found = true;
                    break;
                }
                Ok(false) => {}
                Err(_) if r.mover_size < 2 => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
    pub fn find(&mut self, ctx: &mut impl Context, r: &Request) -> Result<Path> {
        self.reset();
        let start = r.start.as_ref().ok_or(Error::Null)?;
        let dest = r.end.as_ref().ok_or(Error::Null)?;
        let [sx, sy, sz] = start.coordinates();
        let [dx, dy, _] = dest.coordinates();
        let vx = dx.wrapping_sub(sx) as f64;
        let vy = dy.wrapping_sub(sy) as f64;
        let magnitude = (vx.powf(2.0) + vy.powf(2.0)).sqrt();
        let mut end = dest.clone();
        if magnitude.floor() > r.max_distance as f64 {
            if magnitude.floor() >= 50.0 {
                return Ok(Path {
                    move_near: true,
                    ..Path::default()
                });
            }
            let scale = r.max_distance.wrapping_sub(1) as f64;
            end = Location::new(
                sx.wrapping_add((vx / magnitude * scale).floor() as i32),
                sy.wrapping_add((vy / magnitude * scale).floor() as i32),
                0,
            );
        }
        self.found = false;
        self.via.fill(0);
        self.cost.fill(99_999_999);
        let base = Location::new(
            (sx >> 3).wrapping_sub(6).wrapping_shl(3),
            (sy >> 3).wrapping_sub(6).wrapping_shl(3),
            sz,
        )
        .coordinates();
        self.current = [scene(sx), scene(sy)];
        let [ex, ey, _] = end.coordinates();
        self.destination = [
            ex.wrapping_sub((sx >> 3).wrapping_sub(6).wrapping_shl(3)),
            ey.wrapping_sub((sy >> 3).wrapping_sub(6).wrapping_shl(3)),
        ];
        self.check(self.current[0], self.current[1], 99, 0, 0)?;
        if r.has_supplier {
            let _ = if r.mover_size < 2 && !r.bfs {
                self.astar(ctx, r, &end, base)
            } else {
                self.bfs(ctx, r, &end, base)
            };
        }
        let mut path = Path::default();
        if !self.found && r.near {
            let mut full_cost = 1000;
            let mut this_cost = 100;
            for x in self.destination[0].wrapping_sub(10)..=self.destination[0].wrapping_add(10) {
                for y in self.destination[1].wrapping_sub(10)..=self.destination[1].wrapping_add(10)
                {
                    let Ok(i) = index(x, y) else {
                        continue;
                    };
                    if self.cost[i] >= 100 {
                        continue;
                    }
                    let mut ax = 0i32;
                    let mut ay = 0i32;
                    let corner_x = self.destination[0].wrapping_add(r.size[0]).wrapping_sub(1);
                    let corner_y = self.destination[1].wrapping_add(r.size[1]).wrapping_sub(1);
                    if x < self.destination[0] {
                        ax = self.destination[0].wrapping_sub(x);
                    } else if x > corner_x {
                        ax = x.wrapping_sub(corner_x);
                    }
                    if y < self.destination[1] {
                        ay = self.destination[1].wrapping_sub(y);
                    } else if y > corner_y {
                        ay = y.wrapping_sub(corner_y);
                    }
                    let total = ax.wrapping_mul(ax).wrapping_add(ay.wrapping_mul(ay));
                    if total < full_cost || (total == full_cost && self.cost[i] < this_cost) {
                        full_cost = total;
                        this_cost = self.cost[i];
                        self.current = [x, y];
                    }
                }
            }
            if full_cost == 1000 {
                return Ok(path);
            }
            path.move_near = true;
        }
        let mut read = 0usize;
        self.queue_x[read] = self.current[0];
        self.queue_y[read] = self.current[1];
        read += 1;
        let mut direction = self.via[index(self.current[0], self.current[1])?];
        let mut attempts = 0;
        loop {
            let [x, y, _] = start.coordinates();
            if self.current == [scene(x), scene(y)] {
                break;
            }
            attempts += 1;
            if attempts > self.queue_x.len() {
                return Ok(path);
            }
            *self.queue_x.get_mut(read).ok_or(Error::Index)? = self.current[0];
            *self.queue_y.get_mut(read).ok_or(Error::Index)? = self.current[1];
            read += 1;
            if direction & 2 != 0 {
                self.current[0] += 1;
            } else if direction & 8 != 0 {
                self.current[0] -= 1;
            }
            if direction & 1 != 0 {
                self.current[1] += 1;
            } else if direction & 4 != 0 {
                self.current[1] -= 1;
            }
            direction = self.via[index(self.current[0], self.current[1])?];
        }
        for i in (0..read).rev() {
            path.points.push([
                base[0].wrapping_add(self.queue_x[i]),
                base[1].wrapping_add(self.queue_y[i]),
            ]);
        }
        path.successful = true;
        if end.coordinates() != dest.coordinates() {
            path.move_near = true;
        }
        Ok(path)
    }
}
