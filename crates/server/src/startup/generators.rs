//! One world's set of the original's random generators.
//!
//! The original draws from four process-wide sources: `RandomFunction.RANDOM`
//! (one shared `java.util.Random`), `Math.random()` (the JDK's own
//! `java.util.Random` behind it), the `java.util.Random` private to
//! `Collections.shuffle(List)`, and the world thread's `ThreadLocalRandom`. It
//! also constructs further `java.util.Random` objects without a seed (the
//! world-trace oracle counts them as `seeded`). Natively these belong to one
//! world: [`WorldGenerators`] owns them and is handed by `&mut` to every loader
//! and library that draws. Nothing here seeds a generator from the clock or the
//! operating system: the seeds are an explicit [`Seeds`] input with no default.
//!
//! Draw counts use the unit of the world-trace oracle (`Pin.Counting`): one
//! per call of `java.util.Random.next(bits)`, so `nextInt(bound)` can count
//! more than once (its rejection loop) and `nextDouble`/`nextLong` count two.
use crate::world::drop_tables::random::{BadRange, Source, ThreadLocalRandom};

/// `java.util.Random` (JDK 11) with the number of `next(bits)` calls made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CountingRandom {
    state: u64,
    draws: u64,
}
impl CountingRandom {
    const MULTIPLIER: u64 = 0x5_DEEC_E66D;
    const MASK: u64 = (1 << 48) - 1;

    /// `new Random(seed)`: the constructor scrambles the seed and draws nothing.
    pub const fn new(seed: i64) -> Self {
        Self {
            state: (seed as u64 ^ Self::MULTIPLIER) & Self::MASK,
            draws: 0,
        }
    }
    /// `next(bits)`.
    pub fn next(&mut self, bits: u32) -> i32 {
        self.draws += 1;
        self.state = self.state.wrapping_mul(Self::MULTIPLIER).wrapping_add(0xB) & Self::MASK;
        (self.state >> (48 - bits)) as i64 as i32
    }
    /// `nextInt()`.
    pub fn next_i32(&mut self) -> i32 {
        self.next(32)
    }
    /// `nextInt(bound)`; `None` where Java throws `IllegalArgumentException`.
    pub fn next_bound(&mut self, bound: i32) -> Option<i32> {
        if bound <= 0 {
            return None;
        }
        let mut r = self.next(31);
        let m = bound - 1;
        if bound & m == 0 {
            return Some(((i64::from(bound) * i64::from(r)) >> 31) as i32);
        }
        let mut u = r;
        loop {
            r = u % bound;
            if u.wrapping_sub(r).wrapping_add(m) >= 0 {
                return Some(r);
            }
            u = self.next(31);
        }
    }
    /// `nextBoolean()`.
    pub fn next_bool(&mut self) -> bool {
        self.next(1) != 0
    }
    /// `nextLong()`.
    pub fn next_i64(&mut self) -> i64 {
        (i64::from(self.next(32)) << 32).wrapping_add(i64::from(self.next(32)))
    }
    /// `nextDouble()`; also what `Math.random()` returns from its generator.
    pub fn next_double(&mut self) -> f64 {
        let high = i64::from(self.next(26));
        let low = i64::from(self.next(27));
        ((high << 27) + low) as f64 * (1.0 / (1u64 << 53) as f64)
    }
    /// Calls of `next(bits)` so far.
    pub const fn draws(&self) -> u64 {
        self.draws
    }
    /// The 48-bit state, as the oracle prints it (`%012x`).
    pub fn state_hex(&self) -> String {
        format!("{:012x}", self.state)
    }
}

/// The seeds of one world's generators. The original takes them from the
/// clock and the JDK's seed uniquifier; the world-trace oracle pins them (all
/// three `java.util.Random` sources to 0, the thread-local seed per scenario).
/// A native world receives them from its host: there is no default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seeds {
    pub shared: i64,
    pub math: i64,
    pub shuffle: i64,
    /// The world thread's `threadLocalRandomSeed` field.
    pub thread_local: i64,
}

/// Generator positions in the world-trace oracle's units.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Positions {
    pub shared: (u64, String),
    pub math: (u64, String),
    pub shuffle: (u64, String),
    /// The thread-local seed, `%016x`.
    pub thread_local: String,
    /// Unseeded `java.util.Random` constructions so far.
    pub seeded: u64,
}

/// The four generators one world draws from.
#[derive(Clone, Debug)]
pub struct WorldGenerators {
    pub shared: CountingRandom,
    pub math: CountingRandom,
    pub shuffle: CountingRandom,
    pub thread_local: ThreadLocalRandom,
    seeded: u64,
}
impl WorldGenerators {
    pub fn new(seeds: Seeds) -> Self {
        Self {
            shared: CountingRandom::new(seeds.shared),
            math: CountingRandom::new(seeds.math),
            shuffle: CountingRandom::new(seeds.shuffle),
            thread_local: ThreadLocalRandom::new(seeds.thread_local),
            seeded: 0,
        }
    }
    pub fn positions(&self) -> Positions {
        Positions {
            shared: (self.shared.draws(), self.shared.state_hex()),
            math: (self.math.draws(), self.math.state_hex()),
            shuffle: (self.shuffle.draws(), self.shuffle.state_hex()),
            thread_local: format!("{:016x}", self.thread_local.seed()),
            seeded: self.seeded,
        }
    }
    /// `new Random()` in the original: the n-th construction is a host input
    /// (the oracle seeds it with n). Returns that generator and counts it.
    pub fn construct_unseeded(&mut self, seed: impl FnOnce(u64) -> i64) -> CountingRandom {
        self.seeded += 1;
        CountingRandom::new(seed(self.seeded))
    }
    /// `Math.random()`.
    pub fn math_random(&mut self) -> f64 {
        self.math.next_double()
    }
    /// `RandomFunction.RANDOM.nextInt(bound)`; `None` for a nonpositive bound
    /// (Java throws).
    pub fn shared_next_int(&mut self, bound: i32) -> Option<i32> {
        self.shared.next_bound(bound)
    }
    /// `RandomFunction.RANDOM.nextDouble()`.
    pub fn shared_next_double(&mut self) -> f64 {
        self.shared.next_double()
    }
    /// `nextInt(bound)` on `Collections.shuffle`'s generator.
    pub fn shuffle_next_int(&mut self, bound: i32) -> Option<i32> {
        self.shuffle.next_bound(bound)
    }
    /// `ThreadLocalRandom.current().nextDouble(origin, bound)` on the world thread.
    pub fn thread_local_next_double(&mut self, origin: f64, bound: f64) -> Result<f64, BadRange> {
        self.thread_local.next_double(origin, bound)
    }
}

/// The drop-table, clue-reward and shared-table loaders and rolls draw
/// through this. Nonpositive bounds are never passed by those callers; if one
/// is, the draw that Java would refuse is reported as 0 without drawing, as
/// `drop_tables::random::JavaSource` does.
impl Source for WorldGenerators {
    fn next_int(&mut self, bound: i32) -> i32 {
        self.shared.next_bound(bound).unwrap_or(0)
    }
    fn next_boolean(&mut self) -> bool {
        self.shared.next_bool()
    }
    fn next_double(&mut self, bound: f64) -> Result<f64, BadRange> {
        self.thread_local.next_double(0.0, bound)
    }
    fn shuffle_int(&mut self, bound: i32) -> i32 {
        self.shuffle.next_bound(bound).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs09_game_core::random::JavaRandom;

    #[test]
    fn counting_random_matches_the_shared_java_random() {
        let mut ours = CountingRandom::new(-42);
        let mut theirs = JavaRandom::new(-42);
        for bound in [1, 2, 3, 7, 16, 100, 1 << 30, i32::MAX] {
            assert_eq!(ours.next_bound(bound), theirs.next_bound(bound));
            assert_eq!(ours.next_double().to_bits(), theirs.next_double().to_bits());
            assert_eq!(ours.next_bool(), theirs.next_bool());
            assert_eq!(ours.next_i64(), theirs.next_i64());
        }
    }

    #[test]
    fn a_fresh_generator_has_drawn_nothing() {
        let g = CountingRandom::new(0);
        assert_eq!(g.draws(), 0);
        // new Random(0): 0 ^ 0x5DEECE66D.
        assert_eq!(g.state_hex(), "0005deece66d");
    }
}
