//! The RandomFunction methods the gathering sources use, and the JDK
//! thread-local generator behind `RandomFunction.randomDouble`.
//!
//! Each method draws from the same generator as the original, the same number
//! of times: `RANDOM` (shared java.util.Random), `Math.random()`,
//! `ThreadLocalRandom.current()` or `Collections.shuffle`'s private generator.
//! The generators themselves belong to the owning world and are requested
//! through [`Service`]; nothing here seeds or substitutes one.
use super::service::{Calls, Context, Result, Service};

/// `RandomFunction.random(int maxValue)`: no draw unless the bound is positive.
pub fn random(ctx: &mut (impl Context + ?Sized), max_value: i32) -> Result<i32> {
    if max_value <= 0 {
        return Ok(0);
    }
    ctx.int(Service::RngSharedNextInt, &[max_value.into()])
}
/// `RandomFunction.random(int a, int b)`, with Java's wrapping `Math.abs(b - a)`.
pub fn random_range(ctx: &mut (impl Context + ?Sized), a: i32, b: i32) -> Result<i32> {
    let n = b.wrapping_sub(a).wrapping_abs();
    let offset = if n == 0 { 0 } else { random(ctx, n)? };
    Ok(a.min(b).wrapping_add(offset))
}
/// `RandomFunction.roll(int chance)`.
pub fn roll(ctx: &mut (impl Context + ?Sized), chance: i32) -> Result<bool> {
    if chance <= 1 {
        return Ok(true);
    }
    Ok(random(ctx, chance)? == 1)
}
/// `RandomFunction.randomize(int value)`.
pub fn randomize(ctx: &mut (impl Context + ?Sized), value: i32) -> Result<i32> {
    if value < 1 {
        return Ok(0);
    }
    ctx.int(Service::RngSharedNextInt, &[value.into()])
}
/// `RandomFunction.getRandom(int maxValue)`: `(int) (Math.random() * (maxValue + 1))`.
pub fn get_random(ctx: &mut (impl Context + ?Sized), max_value: i32) -> Result<i32> {
    let draw = ctx.double(Service::RngMathNextDouble, &[])?;
    Ok((draw * f64::from(max_value.wrapping_add(1))) as i32)
}
/// `RandomFunction.randomDouble(double maxValue)`: `ThreadLocalRandom.current().nextDouble(0.0, maxValue)`.
pub fn random_double(ctx: &mut (impl Context + ?Sized), max_value: f64) -> Result<f64> {
    ctx.double(
        Service::RngThreadLocalNextDouble,
        &[0.0.into(), max_value.into()],
    )
}
/// `Math.min(double, double)`: NaN wins, and -0.0 is below 0.0.
fn java_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_negative() { a } else { b }
    } else if a <= b {
        a
    } else {
        b
    }
}
/// `Math.max(double, double)`: NaN wins, and 0.0 is above -0.0.
fn java_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_negative() { b } else { a }
    } else if a >= b {
        a
    } else {
        b
    }
}
/// `RandomFunction.random(double a, double b)`: `min + (max - min) * RANDOM.nextDouble()`.
pub fn random_between(ctx: &mut (impl Context + ?Sized), a: f64, b: f64) -> Result<f64> {
    let (min, max) = (java_min(a, b), java_max(a, b));
    Ok(min + (max - min) * ctx.double(Service::RngSharedNextDouble, &[])?)
}
/// `RandomFunction.getSkillSuccessChance(low, high, level)`.
pub fn skill_success_chance(low: f64, high: f64, level: i32) -> f64 {
    let a = (low * (f64::from(99i32.wrapping_sub(level)) / 98.0)).floor();
    let b = (high * (f64::from(level.wrapping_sub(1)) / 98.0)).floor();
    let value = (a + b + 1.0) as i32;
    (f64::from(value) / 256.0).clamp(0.0, 1.0) * 100.0
}
/// `Collections.shuffle(list)` for a random-access list: one draw per position, from the end.
pub fn shuffle<T>(ctx: &mut (impl Context + ?Sized), list: &mut [T]) -> Result<()> {
    for i in (2..=list.len()).rev() {
        let j = ctx.int(Service::RngShuffleNextInt, &[(i as i32).into()])?;
        let j = usize::try_from(j).map_err(|_| super::service::Error::Reply)?;
        if j >= i {
            return Err(super::service::Error::Reply);
        }
        list.swap(i - 1, j);
    }
    Ok(())
}
/// `RandomFunction.rollWeightedChanceTable(List<WeightedChanceItem>)` followed
/// by `WeightedChanceItem.getItem()`. Rows are (id, minimum, maximum, weight).
/// The table is shuffled in place after the weight draw, the comparison is
/// `rand <= weight`, and an exhausted table returns `None` (the original null).
pub fn roll_weighted_chance_table(
    ctx: &mut (impl Context + ?Sized),
    table: &mut [(i32, i32, i32, i32)],
) -> Result<Option<(i32, i32)>> {
    let sum = table.iter().fold(0i32, |a, r| a.wrapping_add(r.3));
    let mut rand = random(ctx, sum)?;
    shuffle(ctx, table)?;
    for &(id, minimum, maximum, weight) in table.iter() {
        if rand <= weight {
            return Ok(Some((id, random_range(ctx, minimum, maximum)?)));
        }
        rand = rand.wrapping_sub(weight);
    }
    Ok(None)
}

/// JDK 11 `java.util.concurrent.ThreadLocalRandom` for one thread: the seed
/// advances by a fixed gamma and each output is a mix of the new seed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThreadLocalRandom {
    pub seed: i64,
}
impl ThreadLocalRandom {
    const GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;
    fn next_seed(&mut self) -> u64 {
        self.seed = (self.seed as u64).wrapping_add(Self::GAMMA) as i64;
        self.seed as u64
    }
    fn mix64(mut z: u64) -> u64 {
        z = (z ^ (z >> 33)).wrapping_mul(0xff51_afd7_ed55_8ccd);
        z = (z ^ (z >> 33)).wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        z ^ (z >> 33)
    }
    fn mix32(mut z: u64) -> i32 {
        z = (z ^ (z >> 33)).wrapping_mul(0xff51_afd7_ed55_8ccd);
        ((z ^ (z >> 33)).wrapping_mul(0xc4ce_b9fe_1a85_ec53) >> 32) as i32
    }
    /// `nextDouble(origin, bound)` for `origin < bound`.
    ///
    /// The pinned JDK 11 computes `Math.nextAfter(bound, origin)` when rounding
    /// reaches the bound and then discards it, so the bound itself can be
    /// returned. A success roll of exactly 100.0 therefore fails even at a
    /// 100% chance; that is reference behaviour and is kept.
    pub fn next_double(&mut self, origin: f64, bound: f64) -> f64 {
        let seed = self.next_seed();
        let r = (Self::mix64(seed) >> 11) as f64 * (1.0 / (1u64 << 53) as f64);
        r * (bound - origin) + origin
    }
    /// `nextInt(bound)` for a positive bound.
    pub fn next_int(&mut self, bound: i32) -> i32 {
        let seed = self.next_seed();
        let mut r = Self::mix32(seed);
        let m = bound.wrapping_sub(1);
        if bound & m == 0 {
            r &= m;
        } else {
            let mut u = ((r as u32) >> 1) as i32;
            loop {
                r = u % bound;
                if u.wrapping_add(m).wrapping_sub(r) >= 0 {
                    break;
                }
                let seed = self.next_seed();
                u = ((Self::mix32(seed) as u32) >> 1) as i32;
            }
        }
        r
    }
}
