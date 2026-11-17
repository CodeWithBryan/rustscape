//! RT4 collision flags and line checks. Object interaction reach tests are pending.
//! The flags are contiguous, in the original x-major order.
#[derive(Clone, Debug)]
pub struct CollisionMap {
    width: usize,
    height: usize,
    flags: Vec<i32>,
}

impl CollisionMap {
    pub fn new(width: usize, height: usize) -> Option<Self> {
        if width > i32::MAX as usize || height > i32::MAX as usize {
            return None;
        }
        let count = width.checked_mul(height)?;
        let mut flags = Vec::new();
        flags.try_reserve_exact(count).ok()?;
        flags.resize(count, 0);
        let mut result = Self {
            width,
            height,
            flags,
        };
        result.clear();
        Some(result)
    }
    pub fn flags(&self) -> &[i32] {
        &self.flags
    }
    fn index(&self, x: i32, y: i32) -> Option<usize> {
        let (x, y) = (usize::try_from(x).ok()?, usize::try_from(y).ok()?);
        if x < self.width && y < self.height {
            Some(x * self.height + y)
        } else {
            None
        }
    }
    pub fn get(&self, x: i32, y: i32) -> Option<i32> {
        self.index(x, y).map(|index| self.flags[index])
    }
    pub fn set(&mut self, x: i32, y: i32, flags: i32) -> Option<()> {
        let index = self.index(x, y)?;
        self.flags[index] = flags;
        Some(())
    }
    pub fn clear(&mut self) {
        for x in 0..self.width {
            for y in 0..self.height {
                self.flags[x * self.height + y] = if x == 0
                    || y == 0
                    || x >= self.width.saturating_sub(5)
                    || y >= self.height.saturating_sub(5)
                {
                    0xffffff
                } else {
                    0x1000000
                };
            }
        }
    }
    pub fn set_mask(&mut self, x: i32, y: i32, mask: i32, present: bool) -> Option<()> {
        let index = self.index(x, y)?;
        if present {
            self.flags[index] |= mask;
        } else {
            self.flags[index] &= !mask;
        }
        Some(())
    }
    /// All wall rotations and shapes 0..3, with the matching projectile flags.
    pub fn wall(
        &mut self,
        at: (i32, i32),
        angle: u8,
        shape: u8,
        projectiles: bool,
        present: bool,
    ) -> Option<()> {
        if angle > 3 || shape > 3 {
            return None;
        }
        const CARDINAL: [(i32, i32, i32, i32); 4] = [
            (-1, 0, 128, 8),
            (0, 1, 2, 32),
            (1, 0, 8, 128),
            (0, -1, 32, 2),
        ];
        const DIAGONAL: [(i32, i32, i32, i32); 4] = [
            (-1, 1, 1, 16),
            (1, 1, 4, 64),
            (1, -1, 16, 1),
            (-1, -1, 64, 4),
        ];
        let mut changes = [(0usize, 0i32); 4];
        let mut count = 0;
        let sides = if shape == 2 { 2 } else { 1 };
        for side in 0..sides {
            let direction = (usize::from(angle) + side) & 3;
            let (dx, dy, own, other) = if shape == 1 || shape == 3 {
                DIAGONAL[direction]
            } else {
                CARDINAL[direction]
            };
            for (x, y, mask) in [
                (at.0, at.1, own),
                (at.0.checked_add(dx)?, at.1.checked_add(dy)?, other),
            ] {
                let mask = if projectiles {
                    mask | (mask << 9)
                } else {
                    mask
                };
                changes[count] = (self.index(x, y)?, mask);
                count += 1;
            }
        }
        for &(index, mask) in &changes[..count] {
            if present {
                self.flags[index] |= mask;
            } else {
                self.flags[index] &= !mask;
            }
        }
        Some(())
    }
    /// Rectangle sizes are supplied after applying the object's rotation.
    pub fn scenery(&mut self, at: (i32, i32), size: (i32, i32), projectiles: bool, present: bool) {
        let mask = if projectiles { 0x20100 } else { 0x100 };
        let right = (i64::from(at.0) + i64::from(size.0)).min(self.width as i64);
        let top = (i64::from(at.1) + i64::from(size.1)).min(self.height as i64);
        for x in i64::from(at.0).max(0)..right {
            for y in i64::from(at.1).max(0)..top {
                self.set_mask(x as i32, y as i32, mask, present)
                    .expect("clipped rectangle");
            }
        }
    }
    pub fn is_path_clear(&self, source: (i32, i32), destination: (i32, i32)) -> bool {
        if source == destination {
            return true;
        }
        if self.index(source.0, source.1).is_none()
            || self.index(destination.0, destination.1).is_none()
        {
            return false;
        }
        let (mut sx, mut sy) = source;
        let (dx, dy) = destination;
        let abs_dx = (dx - sx).abs();
        let abs_dy = (dy - sy).abs();
        let mut accumulator = 32768i32;
        if abs_dx <= abs_dy {
            let step = abs_dx.wrapping_mul(65536) / abs_dy;
            while sy != dy {
                let (move_y, mask) = if dy > sy {
                    (1, 0x12c0102)
                } else {
                    (-1, 0x12c0120)
                };
                if self.get(sx, sy).is_none_or(|flags| flags & mask != 0) {
                    return false;
                }
                sy += move_y;
                accumulator = accumulator.wrapping_add(step);
                if accumulator >= 65536 {
                    accumulator -= 65536;
                    if sx != dx {
                        let (move_x, mask) = if sx < dx {
                            (1, 0x12c0108)
                        } else {
                            (-1, 0x12c0180)
                        };
                        if self.get(sx, sy).is_none_or(|flags| flags & mask != 0) {
                            return false;
                        }
                        sx += move_x;
                    }
                }
            }
        } else {
            let step = abs_dy.wrapping_mul(65536) / abs_dx;
            while sx != dx {
                let (move_x, mask) = if sx < dx {
                    (1, 0x12c0108)
                } else {
                    (-1, 0x12c0180)
                };
                if self.get(sx, sy).is_none_or(|flags| flags & mask != 0) {
                    return false;
                }
                sx += move_x;
                accumulator = accumulator.wrapping_add(step);
                if accumulator >= 65536 {
                    accumulator -= 65536;
                    if sy != dy {
                        let (move_y, mask) = if sy < dy {
                            (1, 0x12c0102)
                        } else {
                            (-1, 0x12c0120)
                        };
                        if self.get(sx, sy).is_none_or(|flags| flags & mask != 0) {
                            return false;
                        }
                        sy += move_y;
                    }
                }
            }
        }
        self.get(dx, dy).is_some_and(|flags| flags & 0x1240100 == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_wall_does_not_partially_mutate_a_map() {
        let mut map = CollisionMap::new(8, 8).unwrap();
        let original = map.flags().to_vec();
        assert!(map.wall((0, 0), 0, 0, true, true).is_none());
        assert_eq!(map.flags(), original);
        assert!(CollisionMap::new(usize::MAX, 1).is_none());
        assert!(!map.is_path_clear((-1, 0), (1, 1)));
    }
}
