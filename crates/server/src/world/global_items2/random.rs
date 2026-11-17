//! The three generators these classes draw from, and the draws they make:
//!
//! * `RandomFunction.RANDOM` (a shared `java.util.Random`): `random(int)`,
//!   `random(a, b)`, table insertion positions and item amounts.
//! * The `java.util.Random` private to `Collections.shuffle(List)`:
//!   `getChanceItem`'s shuffle.
//! * The calling thread's `ThreadLocalRandom`: `randomDouble` (the weighted
//!   table roll) and Kotlin's `Random.Default` (`random()` of an array),
//!   which on JDK 8+ is `ThreadLocalRandom.current().nextInt(bound)`.
use super::{Error, Result};
use crate::world::drop_tables::random::{BadRange, Source};

/// The draws the handlers make, by generator.
pub trait Random {
    /// `RandomFunction.RANDOM.nextInt(bound)` (callers pass a positive bound).
    fn next_int(&mut self, bound: i32) -> i32;
    /// `RandomFunction.RANDOM.nextBoolean()`.
    fn next_boolean(&mut self) -> bool;
    /// `nextInt(bound)` of the generator of `Collections.shuffle(List)`.
    fn shuffle_int(&mut self, bound: i32) -> i32;
    /// `ThreadLocalRandom.current().nextInt(bound)` (callers pass a positive
    /// bound).
    fn thread_int(&mut self, bound: i32) -> i32;
    /// `ThreadLocalRandom.current().nextDouble(origin, bound)`.
    fn thread_double(&mut self, origin: f64, bound: f64) -> std::result::Result<f64, BadRange>;
}

/// `java.util.Random`'s 48-bit generator with its state readable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JavaRandom {
    /// The scrambled 48-bit seed (`Random.seed`).
    pub state: i64,
}
impl JavaRandom {
    const MULTIPLIER: i64 = 0x5_DEEC_E66D;
    const MASK: i64 = (1 << 48) - 1;
    /// `new Random(seed)` / `setSeed(seed)`.
    pub const fn new(seed: i64) -> Self {
        Self {
            state: (seed ^ Self::MULTIPLIER) & Self::MASK,
        }
    }
    pub fn next(&mut self, bits: u32) -> i32 {
        self.state = (self.state.wrapping_mul(Self::MULTIPLIER).wrapping_add(0xB)) & Self::MASK;
        (self.state >> (48 - bits)) as i32
    }
    /// `nextInt(bound)` for a positive bound.
    pub fn next_int(&mut self, bound: i32) -> i32 {
        let mut r = self.next(31);
        let m = bound - 1;
        if bound & m == 0 {
            return ((i64::from(bound) * i64::from(r)) >> 31) as i32;
        }
        let mut u = r;
        loop {
            r = u % bound;
            if u.wrapping_sub(r).wrapping_add(m) >= 0 {
                return r;
            }
            u = self.next(31);
        }
    }
    pub fn next_boolean(&mut self) -> bool {
        self.next(1) != 0
    }
}

/// JDK 11 `ThreadLocalRandom` for one thread, with its seed readable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreadRandom {
    /// `Thread.threadLocalRandomSeed`.
    pub seed: i64,
}
impl ThreadRandom {
    const GAMMA: i64 = 0x9e37_79b9_7f4a_7c15_u64 as i64;
    const DOUBLE_UNIT: f64 = 1.0 / (1u64 << 53) as f64;
    pub const fn new(seed: i64) -> Self {
        Self { seed }
    }
    fn next_seed(&mut self) -> i64 {
        self.seed = self.seed.wrapping_add(Self::GAMMA);
        self.seed
    }
    fn mix64(z: i64) -> i64 {
        let z = z as u64;
        let z = (z ^ (z >> 33)).wrapping_mul(0xff51_afd7_ed55_8ccd);
        let z = (z ^ (z >> 33)).wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        (z ^ (z >> 33)) as i64
    }
    fn mix32(z: i64) -> i32 {
        let z = z as u64;
        let z = (z ^ (z >> 33)).wrapping_mul(0xff51_afd7_ed55_8ccd);
        ((z ^ (z >> 33)).wrapping_mul(0xc4ce_b9fe_1a85_ec53) >> 32) as i32
    }
    /// `nextInt(bound)` for a positive bound.
    pub fn next_int(&mut self, bound: i32) -> i32 {
        let mut r = Self::mix32(self.next_seed());
        let m = bound - 1;
        if bound & m == 0 {
            return r & m;
        }
        let mut u = ((r as u32) >> 1) as i32;
        loop {
            r = u % bound;
            if u.wrapping_add(m).wrapping_sub(r) >= 0 {
                return r;
            }
            u = ((Self::mix32(self.next_seed()) as u32) >> 1) as i32;
        }
    }
    /// `nextDouble(origin, bound)` (JDK 11 keeps a product that rounds onto
    /// the bound).
    pub fn next_double(&mut self, origin: f64, bound: f64) -> std::result::Result<f64, BadRange> {
        if origin.partial_cmp(&bound) != Some(std::cmp::Ordering::Less) {
            return Err(BadRange);
        }
        let r = ((Self::mix64(self.next_seed()) as u64) >> 11) as f64 * Self::DOUBLE_UNIT;
        Ok(r * (bound - origin) + origin)
    }
}

/// The original's three generators with the JDK 11 algorithms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JavaGenerators {
    pub random: JavaRandom,
    pub shuffle: JavaRandom,
    pub thread: ThreadRandom,
}
impl JavaGenerators {
    /// `RANDOM.setSeed(random)`, a shuffle generator `new Random(shuffle)`
    /// and the thread seed `thread`.
    pub const fn new(random: i64, shuffle: i64, thread: i64) -> Self {
        Self {
            random: JavaRandom::new(random),
            shuffle: JavaRandom::new(shuffle),
            thread: ThreadRandom::new(thread),
        }
    }
    /// The raw state of each generator.
    pub fn states(&self) -> [i64; 3] {
        [self.random.state, self.shuffle.state, self.thread.seed]
    }
}
impl Random for JavaGenerators {
    fn next_int(&mut self, bound: i32) -> i32 {
        self.random.next_int(bound)
    }
    fn next_boolean(&mut self) -> bool {
        self.random.next_boolean()
    }
    fn shuffle_int(&mut self, bound: i32) -> i32 {
        self.shuffle.next_int(bound)
    }
    fn thread_int(&mut self, bound: i32) -> i32 {
        self.thread.next_int(bound)
    }
    fn thread_double(&mut self, origin: f64, bound: f64) -> std::result::Result<f64, BadRange> {
        self.thread.next_double(origin, bound)
    }
}

/// The drop tables' [`Source`] over these generators (the casket's table).
pub struct AsSource<'a>(pub &'a mut dyn Random);
impl Source for AsSource<'_> {
    fn next_int(&mut self, bound: i32) -> i32 {
        self.0.next_int(bound)
    }
    fn next_boolean(&mut self) -> bool {
        self.0.next_boolean()
    }
    fn next_double(&mut self, bound: f64) -> std::result::Result<f64, BadRange> {
        self.0.thread_double(0.0, bound)
    }
    fn shuffle_int(&mut self, bound: i32) -> i32 {
        self.0.shuffle_int(bound)
    }
}

/// `RandomFunction.random(int maxValue)`: no draw unless the bound is positive.
pub fn random(r: &mut dyn Random, max_value: i32) -> i32 {
    if max_value <= 0 {
        0
    } else {
        r.next_int(max_value)
    }
}

/// `RandomFunction.random(int a, int b)`: half open, equal ends draw nothing.
pub fn random_range(r: &mut dyn Random, a: i32, b: i32) -> i32 {
    let n = b.wrapping_sub(a).wrapping_abs();
    a.min(b).wrapping_add(if n == 0 { 0 } else { random(r, n) })
}

/// Kotlin's `array.random()`: `Random.Default.nextInt(size)` (the thread's
/// generator); an empty array is `NoSuchElementException`.
pub fn kotlin_random<T: Copy>(r: &mut dyn Random, values: &[T]) -> Result<T> {
    if values.is_empty() {
        return Err(Error::NoSuchElement);
    }
    let index = r.thread_int(values.len() as i32) as usize;
    Ok(values[index])
}

/// A `ChanceItem(id, amount, chanceRate)` of a table: its id, its minimum
/// and maximum amounts and its rate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChanceItem {
    pub id: i32,
    pub minimum: i32,
    pub maximum: i32,
    pub rate: f64,
}
impl ChanceItem {
    /// `ChanceItem(id, minimumAmount, chanceRate)`: maximum = minimum.
    pub const fn new(id: i32, amount: i32, rate: i32) -> Self {
        Self {
            id,
            minimum: amount,
            maximum: amount,
            rate: rate as f64,
        }
    }
    /// `ChanceItem(id)`: amount 1, rate 1.0.
    pub const fn single(id: i32) -> Self {
        Self::new(id, 1, 1)
    }
    /// `getRandomItem()`'s amount: the minimum when both ends agree,
    /// otherwise `RandomFunction.random(min, max)`.
    pub fn random_amount(&self, r: &mut dyn Random) -> i32 {
        if self.minimum == self.maximum {
            self.minimum
        } else {
            random_range(r, self.minimum, self.maximum)
        }
    }
}

/// `RandomFunction.getChanceItem(items)`: one `random((int) total)` draw, a
/// `Collections.shuffle` of a copy, then the first whose running rate passes
/// the draw. The index into `items` of the one chosen.
pub fn get_chance_item(r: &mut dyn Random, items: &[ChanceItem]) -> Option<usize> {
    let mut total = 0.0;
    for item in items {
        total += item.rate;
    }
    let draw = random(r, total as i32);
    let mut choices: Vec<usize> = (0..items.len()).collect();
    for i in (2..=choices.len()).rev() {
        let other = r.shuffle_int(i as i32) as usize;
        choices.swap(i - 1, other);
    }
    let mut sub_total = 0.0;
    for index in choices {
        sub_total += items[index].rate;
        if f64::from(draw) < sub_total {
            return Some(index);
        }
    }
    None
}
