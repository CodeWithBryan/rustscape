//! Server direction codes and clipping probes, including the original edge cases.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Direction {
    North = 0,
    East = 1,
    South = 2,
    West = 3,
    NorthEast = 4,
    SouthEast = 5,
    SouthWest = 6,
    NorthWest = 7,
}

impl Direction {
    pub const ALL: [Self; 8] = [
        Self::NorthWest,
        Self::North,
        Self::NorthEast,
        Self::West,
        Self::East,
        Self::SouthWest,
        Self::South,
        Self::SouthEast,
    ];

    pub const fn code(self) -> i32 {
        self as i32
    }
    pub const fn from_code(value: i32) -> Option<Self> {
        Some(match value {
            0 => Self::North,
            1 => Self::East,
            2 => Self::South,
            3 => Self::West,
            4 => Self::NorthEast,
            5 => Self::SouthEast,
            6 => Self::SouthWest,
            7 => Self::NorthWest,
            _ => return None,
        })
    }
    pub fn from_delta(x: i32, y: i32) -> Self {
        match (x.signum(), y.signum()) {
            (-1, -1) => Self::SouthWest,
            (-1, 0) => Self::West,
            (-1, 1) => Self::NorthWest,
            (1, -1) => Self::SouthEast,
            (1, 0) => Self::East,
            (1, 1) => Self::NorthEast,
            (0, -1) => Self::South,
            _ => Self::North,
        }
    }
    pub fn logical_delta(x: i32, y: i32) -> Self {
        if x.wrapping_abs() > y.wrapping_abs() {
            if x > 0 { Self::East } else { Self::West }
        } else if y < 0 {
            Self::South
        } else {
            Self::North
        }
    }
    pub fn walk_flag(mut flags: i32, rotation: u8) -> Option<Self> {
        if rotation > 3 {
            return None;
        }
        if rotation != 0 {
            flags = ((flags << rotation) & 15).wrapping_add(flags >> (4 - rotation));
        }
        if flags <= 0 {
            return None;
        }
        for (mask, direction) in [
            (8, Self::West),
            (2, Self::East),
            (4, Self::South),
            (1, Self::North),
        ] {
            if flags & mask == 0 {
                return Some(direction);
            }
        }
        None
    }
    /// Preserve the upstream cardinal lookup even when called on a diagonal.
    pub fn opposite(self) -> Self {
        Self::from_code((self.code() + 2) & 3).unwrap()
    }
    pub const fn step(self) -> (i32, i32) {
        match self {
            Self::NorthWest => (-1, 1),
            Self::North => (0, 1),
            Self::NorthEast => (1, 1),
            Self::West => (-1, 0),
            Self::East => (1, 0),
            Self::SouthWest => (-1, -1),
            Self::South => (0, -1),
            Self::SouthEast => (1, -1),
        }
    }
    pub const fn traversal(self) -> &'static [(i32, i32, i32)] {
        match self {
            Self::NorthWest => &[(-1, 0, 0x12c0108), (0, 1, 0x12c0120), (-1, 1, 0x12c0138)],
            Self::North => &[(0, 1, 0x12c0120)],
            Self::NorthEast => &[(1, 0, 0x12c0180), (0, 1, 0x12c0120), (1, 1, 0x12c01e0)],
            Self::West => &[(-1, 0, 0x12c0108)],
            Self::East => &[(1, 0, 0x12c0180)],
            Self::SouthWest => &[(-1, 0, 0x12c0108), (0, -1, 0x12c0102), (-1, -1, 0x12c010e)],
            Self::South => &[(0, -1, 0x12c0102)],
            Self::SouthEast => &[(1, 0, 0x12c0180), (0, -1, 0x12c0102), (1, -1, 0x12c0183)],
        }
    }
    pub fn can_move_from(
        self,
        z: i32,
        x: i32,
        y: i32,
        mut clipping: impl FnMut(i32, i32, i32) -> i32,
    ) -> bool {
        let mut allowed = true;
        // Java calls the supplier for every probe, including after a blocked one.
        for &(dx, dy, mask) in self.traversal() {
            if clipping(z, x.wrapping_add(dx), y.wrapping_add(dy)) & mask != 0 {
                allowed = false;
            }
        }
        allowed
    }
}
