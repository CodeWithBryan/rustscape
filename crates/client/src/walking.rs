//! Single-tile ground-click routing and default-player movement from pinned RT4.
//! actor_motion joins live action/force/facing rules; other BAS types remain separate.
use crate::{animation::Sequence, terrain::Region};
use rs09_game_core::collision::CollisionMap;
use rs09_protocol::{
    Error, Result,
    buffer::{Reader, Writer},
};
use std::collections::VecDeque;

const DIRECTIONS: [(i32, i32, i32); 8] = [
    (-1, 0, 2),
    (1, 0, 8),
    (0, -1, 1),
    (0, 1, 4),
    (-1, -1, 3),
    (1, -1, 9),
    (-1, 1, 6),
    (1, 1, 12),
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClippingDefinition {
    pub size: [i32; 2],
    pub walk: i32,
    pub projectiles: bool,
    pub break_route: bool,
}
impl ClippingDefinition {
    /// Reads LocType's stream at runtime; only collision fields are interpreted.
    pub fn decode(data: &[u8]) -> Result<Self> {
        fn string(r: &mut Reader<'_>) -> Result<()> {
            while r.g1()? != 0 {}
            Ok(())
        }
        let mut r = Reader::new(data);
        let mut d = Self {
            size: [1; 2],
            walk: 2,
            projectiles: true,
            break_route: false,
        };
        loop {
            match r.g1()? {
                0 => {
                    if d.break_route {
                        d.walk = 0;
                        d.projectiles = false;
                    }
                    return Ok(d);
                }
                1 => {
                    let n = r.g1()? as usize;
                    r.take(n * 3)?;
                }
                5 => {
                    let n = r.g1()? as usize;
                    r.take(n * 2)?;
                }
                2 | 30..=34 => string(&mut r)?,
                14 => d.size[0] = r.g1()?,
                15 => d.size[1] = r.g1()?,
                17 => {
                    d.walk = 0;
                    d.projectiles = false;
                }
                18 => d.projectiles = false,
                27 => d.walk = 1,
                74 => d.break_route = true,
                40 | 41 => {
                    let n = r.g1()? as usize;
                    r.take(n * 4)?;
                }
                42 => {
                    let n = r.g1()? as usize;
                    r.take(n)?;
                }
                op @ 77 | op @ 92 => {
                    r.take(if op == 92 { 6 } else { 4 })?;
                    let n = r.g1()? as usize;
                    r.take((n + 1) * 2)?;
                }
                78 | 99 | 100 => {
                    r.take(3)?;
                }
                79 => {
                    r.take(5)?;
                    let n = r.g1()? as usize;
                    r.take(n * 2)?;
                }
                249 => {
                    let n = r.g1()?;
                    for _ in 0..n {
                        let text = r.g1()? != 0;
                        r.take(3)?;
                        if text {
                            string(&mut r)?;
                        } else {
                            r.take(4)?;
                        }
                    }
                }
                19 | 28 | 29 | 39 | 69 | 75 | 81 | 101 => {
                    r.g1()?;
                }
                24 | 60 | 65..=67 | 70..=72 | 93 | 102 => {
                    r.g2()?;
                }
                21..=23 | 62 | 64 | 73 | 82 | 88..=91 | 94..=98 => {}
                _ => {
                    return Err(Error::new(
                        r.position() - 1,
                        "unsupported location opcode in clipping decoder",
                    ));
                }
            }
        }
    }
}
pub fn can_step(map: &CollisionMap, x: i32, y: i32, dx: i32, dy: i32) -> bool {
    let clear = |x, y, mask| map.get(x, y).is_some_and(|f| f & mask == 0);
    match (dx, dy) {
        (-1, 0) => clear(x - 1, y, 0x12c0108),
        (1, 0) => clear(x + 1, y, 0x12c0180),
        (0, -1) => clear(x, y - 1, 0x12c0102),
        (0, 1) => clear(x, y + 1, 0x12c0120),
        (-1, -1) => {
            clear(x - 1, y - 1, 0x12c010e)
                && clear(x - 1, y, 0x12c0108)
                && clear(x, y - 1, 0x12c0102)
        }
        (1, -1) => {
            clear(x + 1, y - 1, 0x12c0183)
                && clear(x + 1, y, 0x12c0180)
                && clear(x, y - 1, 0x12c0102)
        }
        (-1, 1) => {
            clear(x - 1, y + 1, 0x12c0138)
                && clear(x - 1, y, 0x12c0108)
                && clear(x, y + 1, 0x12c0120)
        }
        (1, 1) => {
            clear(x + 1, y + 1, 0x12c01e0)
                && clear(x + 1, y, 0x12c0180)
                && clear(x, y + 1, 0x12c0120)
        }
        _ => false,
    }
}
/// Original findPath2/findPath1 frontier tests. The two routines share their
/// endpoint masks; findPath1 adds the interior edge scans for size>2.
pub fn can_step_sized(map: &CollisionMap, p: [i32; 2], d: [i32; 2], size: i32) -> bool {
    let [x, y] = p;
    let [dx, dy] = d;
    if !(1..=104).contains(&size) || !(0..104).contains(&x) || !(0..104).contains(&y) {
        return false;
    }
    if size == 1 {
        return can_step(map, x, y, dx, dy);
    }
    let n = size;
    let clear = |x, y, mask| map.get(x, y).is_some_and(|f| f & mask == 0);
    let middle = |mut test: &mut dyn FnMut(i32) -> bool| (1..n - 1).all(&mut test);
    match (dx, dy) {
        (-1, 0) => {
            x > 0
                && clear(x - 1, y, 0x12c010e)
                && clear(x - 1, y + n - 1, 0x12c0138)
                && middle(&mut |i| clear(x - 1, y + i, 0x12c013e))
        }
        (1, 0) => {
            x < 102
                && clear(x + n, y, 0x12c0183)
                && clear(x + n, y + n - 1, 0x12c01e0)
                && middle(&mut |i| clear(x + n, y + i, 0x12c01e3))
        }
        (0, -1) => {
            y > 0
                && clear(x, y - 1, 0x12c010e)
                && clear(x + n - 1, y - 1, 0x12c0183)
                && middle(&mut |i| clear(x + i, y - 1, 0x12c018f))
        }
        (0, 1) => {
            y < 102
                && clear(x, y + n, 0x12c0138)
                && clear(x + n - 1, y + n, 0x12c01e0)
                && middle(&mut |i| clear(x + i, y + n, 0x12c01f8))
        }
        (-1, -1) => {
            x > 0
                && y > 0
                && clear(x - 1, y + n - 2, 0x12c0138)
                && clear(x - 1, y - 1, 0x12c010e)
                && clear(x + n - 2, y - 1, 0x12c0183)
                && middle(&mut |i| {
                    clear(x - 1, y + i - 1, 0x12c013e) && clear(x + i - 1, y - 1, 0x12c018f)
                })
        }
        (1, -1) => {
            x < 102
                && y > 0
                && clear(x + 1, y - 1, 0x12c010e)
                && clear(x + n, y - 1, 0x12c0183)
                && clear(x + n, y + n - 2, 0x12c01e0)
                && middle(&mut |i| {
                    clear(x + n, y + i - 1, 0x12c01e3) && clear(x + i + 1, y - 1, 0x12c018f)
                })
        }
        (-1, 1) => {
            x > 0
                && y < 102
                && clear(x - 1, y + 1, 0x12c010e)
                && clear(x - 1, y + n, 0x12c0138)
                && clear(x, y + n, 0x12c01e0)
                && middle(&mut |i| {
                    clear(x - 1, y + i + 1, 0x12c013e) && clear(x + i - 1, y + n, 0x12c01f8)
                })
        }
        (1, 1) => {
            x < 102
                && y < 102
                && clear(x + 1, y + n, 0x12c0138)
                && clear(x + n, y + n, 0x12c01e0)
                && clear(x + n, y + 1, 0x12c0183)
                && middle(&mut |i| {
                    clear(x + i + 1, y + n, 0x12c01f8) && clear(x + n, y + i + 1, 0x12c01e3)
                })
        }
        _ => false,
    }
}
#[derive(Clone, Debug)]
pub struct Route {
    /// The original compressed queue, destination first; sendMovePacket reverses it.
    pub turns: Vec<[i32; 2]>,
    pub steps: Vec<[i32; 2]>,
    pub approximate: bool,
}
/// Original LoginManager map flag. X==0 hides it; clear153 retains Y.
#[derive(Clone, Copy, Debug, Default)]
pub struct DestinationFlag {
    pub tile: [i32; 2],
}
impl DestinationFlag {
    pub fn route(&mut self, route: &Route) {
        self.tile = route.turns[0];
    }
    pub fn apply(&mut self, frame: &rs09_protocol::game::Frame) -> Result<bool> {
        if frame.opcode != 153 {
            return Ok(false);
        }
        if !frame.payload.is_empty() {
            return Err(Error::new(0, "CLEAR_MINIMAP_FLAG has a payload"));
        }
        self.tile[0] = 0;
        Ok(true)
    }
}
impl Route {
    /// The server receives at most 25 compressed turns from a ground click.
    pub fn sent_steps(&self) -> &[[i32; 2]] {
        if self.turns.len() <= 25 {
            return &self.steps;
        }
        let terminal = self.turns[self.turns.len() - 25];
        let count = self
            .steps
            .iter()
            .position(|&step| step == terminal)
            .map_or(0, |i| i + 1);
        &self.steps[..count]
    }
    /// Ground-click packet with the pinned profile's opcode ISAAC disabled.
    pub fn packet(&self, origin: [i32; 2], run: bool) -> Vec<u8> {
        self.move_packet(origin, run, false)
    }
    pub fn move_packet(&self, origin: [i32; 2], run: bool, interaction: bool) -> Vec<u8> {
        let n = self.turns.len().min(25);
        let start = self.turns[self.turns.len() - 1];
        let mut w = Writer::new();
        w.p1(if interaction { 77 } else { 215 });
        w.p1((n * 2 + 3) as i32);
        w.p1add(i32::from(run));
        w.p2(origin[0] + start[0]);
        w.p2add(origin[1] + start[1]);
        for point in self.turns.iter().rev().skip(1).take(n - 1) {
            w.p1add(point[0] - start[0]);
            w.p1sub(point[1] - start[1]);
        }
        w.into_vec()
    }
}
pub fn find_path(
    map: &CollisionMap,
    source: [i32; 2],
    destination: [i32; 2],
    approximate: bool,
) -> Option<Route> {
    find_path_sized(map, source, destination, approximate, 1)
}
pub fn find_path_sized(
    map: &CollisionMap,
    source: [i32; 2],
    destination: [i32; 2],
    approximate: bool,
    size: i32,
) -> Option<Route> {
    find_route(map, source, destination, approximate, None, size)
}
pub fn find_path_to(
    map: &CollisionMap,
    source: [i32; 2],
    target: &crate::interaction::Approach,
    approximate: bool,
) -> Option<Route> {
    find_path_to_sized(map, source, target, approximate, 1)
}
pub fn find_path_to_sized(
    map: &CollisionMap,
    source: [i32; 2],
    target: &crate::interaction::Approach,
    approximate: bool,
    size: i32,
) -> Option<Route> {
    find_route(map, source, target.tile, approximate, Some(target), size)
}
fn find_route(
    map: &CollisionMap,
    source: [i32; 2],
    destination: [i32; 2],
    approximate: bool,
    target: Option<&crate::interaction::Approach>,
    size: i32,
) -> Option<Route> {
    if !(1..=104).contains(&size)
        || source.iter().any(|&n| n > 104 - size)
        || source
            .into_iter()
            .chain(destination)
            .any(|n| !(0..104).contains(&n))
    {
        return None;
    }
    let index = |x: i32, y: i32| (x * 104 + y) as usize;
    let mut parents = vec![0; 104 * 104];
    let mut costs = vec![99999999; 104 * 104];
    let mut queue = [[0; 2]; 4096];
    queue[0] = source;
    parents[index(source[0], source[1])] = 99;
    costs[index(source[0], source[1])] = 0;
    let (mut reader, mut writer) = (0, 1);
    let (mut x, mut y) = (source[0], source[1]);
    let mut reached = false;
    while writer != reader {
        [x, y] = queue[reader];
        reader = (reader + 1) & 4095;
        if [x, y] == destination || target.is_some_and(|t| t.reached_sized(map, [x, y], size)) {
            reached = true;
            break;
        }
        let cost = costs[index(x, y)] + 1;
        for (dx, dy, parent) in DIRECTIONS {
            let (nx, ny) = (x + dx, y + dy);
            if !(0..104).contains(&nx)
                || !(0..104).contains(&ny)
                || parents[index(nx, ny)] != 0
                || !can_step_sized(map, [x, y], [dx, dy], size)
            {
                continue;
            }
            queue[writer] = [nx, ny];
            writer = (writer + 1) & 4095;
            parents[index(nx, ny)] = parent;
            costs[index(nx, ny)] = cost;
        }
    }
    if !reached {
        if !approximate {
            return None;
        }
        let (mut best, mut best_cost) = (1000, 100);
        for sx in destination[0] - 10..=destination[0] + 10 {
            for sy in destination[1] - 10..=destination[1] + 10 {
                if !(0..104).contains(&sx) || !(0..104).contains(&sy) || costs[index(sx, sy)] >= 100
                {
                    continue;
                }
                // Ground clicks pass zero location width/length, including this +1 quirk.
                let size = target.map_or([0; 2], |t| t.size);
                let dx = if sx < destination[0] {
                    destination[0] - sx
                } else {
                    (sx + 1 - destination[0] - size[0]).max(0)
                };
                let dy = if sy < destination[1] {
                    destination[1] - sy
                } else {
                    (sy + 1 - destination[1] - size[1]).max(0)
                };
                let distance = dx * dx + dy * dy;
                if distance < best || distance == best && costs[index(sx, sy)] < best_cost {
                    best = distance;
                    best_cost = costs[index(sx, sy)];
                    x = sx;
                    y = sy;
                }
            }
        }
        if best == 1000 || [x, y] == source {
            return None;
        }
    }
    let mut turns = vec![[x, y]];
    let mut steps = Vec::new();
    let mut previous = parents[index(x, y)];
    while [x, y] != source {
        let p = parents[index(x, y)];
        if p != previous {
            previous = p;
            turns.push([x, y]);
        }
        steps.push([x, y]);
        if p & 2 != 0 {
            x += 1;
        } else if p & 8 != 0 {
            x -= 1;
        }
        if p & 1 != 0 {
            y += 1;
        } else if p & 4 != 0 {
            y -= 1;
        }
    }
    steps.reverse();
    Some(Route {
        turns,
        steps,
        approximate: !reached,
    })
}

#[derive(Clone, Copy, Debug)]
pub struct Placement {
    pub position: [i32; 2],
    pub plane: usize,
    pub shape: u8,
    pub rotation: u8,
    pub size: [i32; 2],
    pub walk: i32,
    pub projectiles: bool,
}
pub struct Navigation {
    pub origin: [i32; 2],
    pub maps: [CollisionMap; 4],
}
impl Navigation {
    pub fn new(regions: &[Region], placements: &[Placement], origin: [i32; 2]) -> Self {
        let mut maps = std::array::from_fn(|_| CollisionMap::new(104, 104).unwrap());
        let mut bridge = [false; 104 * 104];
        let mut seen = std::collections::BTreeSet::new();
        for region in regions.iter().filter(|r| seen.insert((r.x, r.z))) {
            let [rx, ry] = [region.x * 64 - origin[0], region.z * 64 - origin[1]];
            for x in rx.max(0)..(rx + 64).min(104) {
                for y in ry.max(0)..(ry + 64).min(104) {
                    let [lx, ly] = [(x - rx) as usize, (y - ry) as usize];
                    let lowered = region.tile(1, lx, ly).flags & 2 != 0;
                    bridge[(x * 104 + y) as usize] = lowered;
                    if (1..103).contains(&x) && (1..103).contains(&y) {
                        for map in &mut maps {
                            map.set_mask(x, y, 0x1000000, false);
                        }
                    }
                    for plane in usize::from(lowered)..4 {
                        if region.tile(plane, lx, ly).flags & 1 != 0 {
                            maps[plane - usize::from(lowered)].set_mask(x, y, 0x200000, true);
                        }
                    }
                }
            }
        }
        for loc in placements {
            let [x, y] = [loc.position[0] - origin[0], loc.position[1] - origin[1]];
            if x <= 0 || y <= 0 || x >= 103 || y >= 103 {
                continue;
            }
            let level = loc.plane as i32 - i32::from(bridge[(x * 104 + y) as usize]);
            if level < 0 {
                continue;
            }
            let map = &mut maps[level as usize];
            if loc.shape == 22 {
                if loc.walk == 1 {
                    map.set_mask(x, y, 0x40000, true);
                }
            } else if loc.walk != 0 {
                if loc.shape <= 3 {
                    map.wall((x, y), loc.rotation, loc.shape, loc.projectiles, true);
                } else if loc.shape >= 9 {
                    let mut size = loc.size;
                    if loc.rotation & 1 != 0 {
                        size.swap(0, 1);
                    }
                    map.scenery((x, y), (size[0], size[1]), loc.projectiles, true);
                }
            }
        }
        Self { origin, maps }
    }
}

/// BAS movement set. Opcodes 29–37 (acceleration, accelerated yaw, lean) are
/// kept in `turning`; no record of the pinned cache uses them.
#[derive(Clone, Debug)]
pub struct PlayerBas {
    pub idle: i32,
    pub walk: i32,
    pub run: i32,
    pub stand: [i32; 2],
    pub turns: [i32; 3],
    pub slow: i32,
    pub slow_turns: [i32; 3],
    pub run_turns: [i32; 3],
    pub terrain: [i32; 2],
    pub body_transforms: Option<crate::player_body::Transforms>,
    pub turning: crate::actor_turning::Bas,
}
impl Default for PlayerBas {
    fn default() -> Self {
        Self {
            idle: -1,
            walk: -1,
            run: -1,
            stand: [-1; 2],
            turns: [-1; 3],
            slow: -1,
            slow_turns: [-1; 3],
            run_turns: [-1; 3],
            terrain: [0; 2],
            body_transforms: None,
            turning: Default::default(),
        }
    }
}
impl PlayerBas {
    pub fn sequences(&self) -> [i32; 15] {
        [
            self.idle,
            self.walk,
            self.run,
            self.stand[0],
            self.stand[1],
            self.turns[0],
            self.turns[1],
            self.turns[2],
            self.slow,
            self.slow_turns[0],
            self.slow_turns[1],
            self.slow_turns[2],
            self.run_turns[0],
            self.run_turns[1],
            self.run_turns[2],
        ]
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let mut b = Self::default();
        loop {
            match r.g1()? {
                0 => return Ok(b),
                1 => {
                    b.idle = r.g2()?;
                    b.walk = r.g2()?;
                    if b.idle == 65535 {
                        b.idle = -1;
                    }
                    if b.walk == 65535 {
                        b.walk = -1;
                    }
                }
                2 => b.slow = r.g2()?,
                op @ 3..=5 => b.slow_turns[(op - 3) as usize] = r.g2()?,
                6 => b.run = r.g2()?,
                op @ 7..=9 => b.run_turns[(op - 7) as usize] = r.g2()?,
                26 => b.terrain = [r.g1()? * 4, r.g1()? * 4],
                op @ 29..=37 => b.turning.decode(op, &mut r)?,
                27 => {
                    let slot = r.g1()? as usize;
                    let transform = b
                        .body_transforms
                        .get_or_insert([None; 12])
                        .get_mut(slot)
                        .ok_or(Error::new(
                            r.position() - 1,
                            "BAS body slot outside original array",
                        ))?;
                    let mut values = [0; 6];
                    for value in &mut values {
                        *value = r.g2b()?;
                    }
                    *transform = Some(values);
                }
                38 => b.stand[0] = r.g2()?,
                39 => b.stand[1] = r.g2()?,
                op @ 40..=42 => b.turns[(op - 40) as usize] = r.g2()?,
                43..=45 => {
                    r.g2()?;
                }
                // BasType.decode reads nothing for an opcode it does not know.
                _ => {}
            }
        }
    }

    fn speed_sequence(&self, walk: i32, speed: i32, multiplier: u8) -> i32 {
        let (base, turns) = if speed >= 8 && self.run != -1 {
            (self.run, self.run_turns)
        } else if self.slow != -1 && multiplier == 0 {
            (self.slow, self.slow_turns)
        } else {
            return walk;
        };
        // RT4 compares the selected ID in full/CCW/CW order. Aliased IDs must
        // retain that priority even when the actor was walking straight ahead.
        for (walk_turn, speed_turn) in self.turns.into_iter().zip(turns) {
            if walk_turn == walk && speed_turn != -1 {
                return speed_turn;
            }
        }
        base
    }
}
#[derive(Clone, Debug)]
pub struct Movement {
    /// Latest validated input command, retained for the future live transport.
    pub pending_move_packet: Vec<u8>,
    pub fine: [i32; 2],
    pub angle: i32,
    pub target_angle: i32,
    pub sequence: i32,
    pub standing_turns: i32,
    pub frame: usize,
    pub next_frame: usize,
    pub progress: i32,
    pub queue: VecDeque<([i32; 2], u8)>,
    /// Queue head survives consumption, forced movement and bounds resets.
    pub head: [i32; 2],
    pub head_speed: u8,
    pub catchup: i32,
    pub face_entity: i32,
    /// Original doubled world-tile coordinates (zero means unset).
    pub face_tile: [i32; 2],
    pub force_start_tile: [i32; 2],
    pub force_destination: [i32; 2],
    pub force_direction: i32,
    pub turn_speed: i32,
    pub rotates_on_move: bool,
    /// Accelerated speed, 16-bit yaw and lean state (BAS opcodes 29–37).
    pub turning: crate::actor_turning::State,
}
impl Movement {
    pub fn new(tile: [i32; 2]) -> Self {
        Self {
            pending_move_packet: Vec::new(),
            fine: tile.map(|n| n * 128 + 64),
            angle: 0,
            target_angle: 0,
            sequence: -1,
            standing_turns: 0,
            frame: 0,
            next_frame: 0,
            progress: 0,
            queue: VecDeque::new(),
            head: tile,
            head_speed: 0,
            catchup: 0,
            face_entity: -1,
            face_tile: [0; 2],
            force_start_tile: [0; 2],
            force_destination: [0; 2],
            force_direction: 0,
            turn_speed: 32,
            rotates_on_move: true,
            turning: Default::default(),
        }
    }
    pub fn enqueue(&mut self, tile: [i32; 2], speed: u8) {
        self.head = tile;
        self.head_speed = speed;
        if self.queue.len() == 9 {
            self.queue.pop_front();
        }
        self.queue.push_back((tile, speed));
    }
    pub fn advance(&mut self, b: &PlayerBas) {
        self.advance_sized(b, 1);
    }
    /// Default-rotation NPC movement uses its footprint when centering on tiles.
    pub fn advance_sized(&mut self, b: &PlayerBas, size: i32) {
        self.advance_path(b, size, false, &mut 0);
        self.advance_facing(b, None);
    }
    pub(crate) fn advance_path(
        &mut self,
        b: &PlayerBas,
        size: i32,
        blocked: bool,
        movement_steps: &mut i32,
    ) {
        self.sequence = b.idle;
        if self.queue.is_empty() {
            self.catchup = 0;
            return;
        }
        if blocked {
            self.catchup = self.catchup.wrapping_add(1);
            return;
        }
        if let Some(&(tile, speed_kind)) = self.queue.front() {
            let dest = tile.map(|n| n * 128 + size * 64);
            let [x, y] = self.fine;
            if (dest[0] - x).abs() > 256 || (dest[1] - y).abs() > 256 {
                self.fine = dest;
            } else {
                self.target_angle = match (dest[0].cmp(&x), dest[1].cmp(&y)) {
                    (std::cmp::Ordering::Less, std::cmp::Ordering::Greater) => 768,
                    (std::cmp::Ordering::Less, std::cmp::Ordering::Less) => 256,
                    (std::cmp::Ordering::Less, _) => 512,
                    (std::cmp::Ordering::Greater, std::cmp::Ordering::Greater) => 1280,
                    (std::cmp::Ordering::Greater, std::cmp::Ordering::Less) => 1792,
                    (std::cmp::Ordering::Greater, _) => 1536,
                    (_, std::cmp::Ordering::Greater) => 1024,
                    (_, std::cmp::Ordering::Less) => 0,
                    _ => self.target_angle,
                };
                let mut difference = (self.target_angle - self.angle) & 2047;
                if difference > 1024 {
                    difference -= 2048;
                }
                let mut anim = if (-256..=256).contains(&difference) {
                    b.walk
                } else if (256..768).contains(&difference) {
                    b.turns[2]
                } else if (-768..=-256).contains(&difference) {
                    b.turns[1]
                } else {
                    b.turns[0]
                };
                if anim == -1 {
                    anim = b.walk;
                }
                self.sequence = anim;
                let mut speed = if self.rotates_on_move
                    && self.angle != self.target_angle
                    && self.face_entity == -1
                    && self.turn_speed != 0
                {
                    2
                } else {
                    4
                };
                if self.queue.len() > if self.rotates_on_move { 2 } else { 1 } {
                    speed = 6;
                }
                if self.queue.len() > if self.rotates_on_move { 3 } else { 2 } {
                    speed = 8;
                }
                if self.catchup > 0 && self.queue.len() > 1 {
                    speed = 8;
                    self.catchup -= 1;
                }
                if speed_kind == 2 {
                    speed *= 2;
                } else if speed_kind == 0 {
                    speed /= 2;
                }
                self.sequence = b.speed_sequence(self.sequence, speed, speed_kind);
                if b.turning.movement_acceleration != -1 {
                    speed = self.turning.accelerate(
                        b.turning.movement_acceleration,
                        speed,
                        self.queue.len(),
                        [(dest[0] - x).abs(), (dest[1] - y).abs()],
                    );
                }
                for (axis, target) in dest.into_iter().enumerate() {
                    self.fine[axis] += (target - self.fine[axis]).clamp(-speed, speed);
                }
                if self.fine == dest {
                    self.queue.pop_front();
                    if *movement_steps > 0 {
                        *movement_steps -= 1;
                    }
                }
            }
        }
    }
    pub(crate) fn advance_facing(&mut self, b: &PlayerBas, target: Option<[i32; 2]>) {
        if self.turn_speed == 0 {
            return;
        }
        let mut face = |target: [i32; 2]| {
            let dx = self.fine[0].wrapping_sub(target[0]);
            let dy = self.fine[1].wrapping_sub(target[1]);
            if dx != 0 || dy != 0 {
                self.target_angle = ((f64::from(dx).atan2(f64::from(dy)) * 325.949) as i32) & 2047;
            }
        };
        if self.face_entity != -1 {
            if let Some(target) = target {
                face(target);
            }
        }
        if self.face_tile != [0; 2] && (self.queue.is_empty() || self.catchup > 0) {
            face(self.face_tile.map(|n| n.wrapping_mul(64)));
            self.face_tile = [0; 2];
        }
        let difference = (self.target_angle - self.angle) & 2047;
        if difference == 0 {
            self.standing_turns = 0;
            self.turning.yaw_velocity = 0;
        } else if b.turning.yaw_acceleration != 0 {
            self.turn_accelerated(b);
        } else {
            self.standing_turns += 1;
            if difference > 1024 {
                self.angle -= self.turn_speed;
            } else {
                self.angle += self.turn_speed;
            }
            let turning = difference >= self.turn_speed && difference <= 2048 - self.turn_speed;
            if !turning {
                self.angle = self.target_angle;
            }
            if self.sequence == b.idle && (self.standing_turns > 25 || turning) {
                let turn = b.stand[usize::from(difference <= 1024)];
                self.sequence = if turn == -1 { b.walk } else { turn };
            }
            self.angle &= 2047;
        }
    }
    pub fn animate(&mut self, seq: &Sequence) -> Vec<usize> {
        let mut sounds = Vec::new();
        self.progress += 1;
        if self.frame < seq.frames.len() && self.progress > i32::from(seq.delays[self.frame]) {
            self.progress = 1;
            self.frame += 1;
            self.next_frame += 1;
            sounds.push(self.frame);
        }
        if self.frame >= seq.frames.len() {
            self.frame = 0;
            self.progress = 0;
            sounds.push(0);
        }
        self.next_frame = (self.frame + 1) % seq.frames.len();
        sounds
    }
}
