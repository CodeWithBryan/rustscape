//! The three random generators the original drop path draws from, and the
//! `RandomFunction` calls it makes on them.
//!
//! * `RandomFunction.RANDOM`, a shared `java.util.Random`: table insertion
//!   positions, amounts, clue choice, the herb-slot coin flip and the weighted
//!   chance tables' pick.
//! * `ThreadLocalRandom.current()`: the one double of every weighted roll.
//! * The `java.util.Random` private to `Collections.shuffle(List)`: the rare
//!   drop table's per-roll order and the weighted chance tables' in-place
//!   shuffle.
use rs09_game_core::random::JavaRandom;
use std::cmp::Ordering;

/// `ThreadLocalRandom.nextDouble(origin, bound)` refused its range
/// (`IllegalArgumentException`): the bound is not above the origin, or is NaN.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BadRange;

/// JDK 11 `java.util.concurrent.ThreadLocalRandom` for one thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreadLocalRandom {
    seed: u64,
}
impl ThreadLocalRandom {
    const GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;
    const DOUBLE_UNIT: f64 = 1.0 / (1u64 << 53) as f64;

    /// A generator whose per-thread seed field holds `seed`.
    pub const fn new(seed: i64) -> Self {
        Self { seed: seed as u64 }
    }
    pub const fn seed(&self) -> i64 {
        self.seed as i64
    }
    /// Number of `next_long` calls that lead from `earlier` to this state.
    pub const fn draws_since(&self, earlier: &Self) -> u64 {
        // GAMMA is odd, so it has an inverse modulo 2^64.
        self.seed
            .wrapping_sub(earlier.seed)
            .wrapping_mul(0xf1de_83e1_9937_733d)
    }
    pub fn next_long(&mut self) -> i64 {
        self.seed = self.seed.wrapping_add(Self::GAMMA);
        let mut z = self.seed;
        z = (z ^ (z >> 33)).wrapping_mul(0xff51_afd7_ed55_8ccd);
        z = (z ^ (z >> 33)).wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        (z ^ (z >> 33)) as i64
    }
    /// `nextDouble(origin, bound)`.
    ///
    /// The result can equal `bound`. The pinned JDK (Temurin 11.0.32.1)
    /// computes its correction for a product that rounds up onto the bound and
    /// then discards it (`if (r >= bound) Math.nextAfter(bound, origin);`), so
    /// there is none here either. The one-argument `nextDouble(bound)` of the
    /// same class does correct; the drop path never calls it.
    pub fn next_double(&mut self, origin: f64, bound: f64) -> Result<f64, BadRange> {
        if origin.partial_cmp(&bound) != Some(Ordering::Less) {
            return Err(BadRange);
        }
        let r = ((self.next_long() as u64) >> 11) as f64 * Self::DOUBLE_UNIT;
        Ok(r * (bound - origin) + origin)
    }
}

/// The draws the drop path makes, by generator.
pub trait Source {
    /// `RandomFunction.RANDOM.nextInt(bound)`. Callers pass a positive bound.
    fn next_int(&mut self, bound: i32) -> i32;
    /// `RandomFunction.RANDOM.nextBoolean()`.
    fn next_boolean(&mut self) -> bool;
    /// `ThreadLocalRandom.current().nextDouble(0.0, bound)`.
    fn next_double(&mut self, bound: f64) -> Result<f64, BadRange>;
    /// `nextInt(bound)` on the generator of `Collections.shuffle(List)`.
    /// Callers pass a positive bound.
    fn shuffle_int(&mut self, bound: i32) -> i32;
}

/// The original's three generators, each with the JDK 11 algorithm.
#[derive(Clone, Debug)]
pub struct JavaSource {
    pub random: JavaRandom,
    pub thread_local: ThreadLocalRandom,
    pub shuffle: JavaRandom,
}
impl JavaSource {
    pub const fn new(random: i64, shuffle: i64, thread_local: i64) -> Self {
        Self {
            random: JavaRandom::new(random),
            thread_local: ThreadLocalRandom::new(thread_local),
            shuffle: JavaRandom::new(shuffle),
        }
    }
}
impl Source for JavaSource {
    fn next_int(&mut self, bound: i32) -> i32 {
        self.random.next_bound(bound).unwrap_or(0)
    }
    fn next_boolean(&mut self) -> bool {
        self.random.next_bool()
    }
    fn next_double(&mut self, bound: f64) -> Result<f64, BadRange> {
        self.thread_local.next_double(0.0, bound)
    }
    fn shuffle_int(&mut self, bound: i32) -> i32 {
        self.shuffle.next_bound(bound).unwrap_or(0)
    }
}

/// `RandomFunction.random(int maxValue)`: no draw unless the bound is positive.
pub fn random(source: &mut dyn Source, max_value: i32) -> i32 {
    if max_value <= 0 {
        0
    } else {
        source.next_int(max_value)
    }
}

/// `RandomFunction.random(int a, int b)`. The range is half open, equal ends
/// draw nothing, and the 32-bit difference wraps as in Java.
pub fn random_range(source: &mut dyn Source, a: i32, b: i32) -> i32 {
    let n = b.wrapping_sub(a).wrapping_abs();
    a.min(b)
        .wrapping_add(if n == 0 { 0 } else { random(source, n) })
}

/// `RandomFunction.randomDouble(double maxValue)`.
pub fn random_double(source: &mut dyn Source, max_value: f64) -> Result<f64, BadRange> {
    source.next_double(max_value)
}

/// `Collections.shuffle(list)` with its private generator.
pub fn shuffle<T>(source: &mut dyn Source, list: &mut [T]) {
    for i in (2..=list.len()).rev() {
        let other = source.shuffle_int(i as i32) as usize;
        list.swap(i - 1, other);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_since_inverts_the_increment() {
        let start = ThreadLocalRandom::new(-7);
        let mut moved = start.clone();
        for expected in 0..100u64 {
            assert_eq!(moved.draws_since(&start), expected);
            moved.next_long();
        }
    }
}
