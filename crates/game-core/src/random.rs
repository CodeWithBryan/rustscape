//! java.util.Random's 48-bit generator and RT4 RandomUtils sampling.
#[derive(Clone, Debug)]
pub struct JavaRandom {
    state: u64,
}
impl JavaRandom {
    pub const fn new(seed: i64) -> Self {
        Self {
            state: (seed as u64 ^ 0x5deece66d) & ((1 << 48) - 1),
        }
    }
    fn bits(&mut self, n: u32) -> u32 {
        self.state = self.state.wrapping_mul(0x5deece66d).wrapping_add(11) & ((1 << 48) - 1);
        (self.state >> (48 - n)) as u32
    }
    pub fn next_i32(&mut self) -> i32 {
        self.bits(32) as i32
    }
    pub fn next_bool(&mut self) -> bool {
        self.bits(1) != 0
    }
    pub fn next_i64(&mut self) -> i64 {
        ((i64::from(self.next_i32())) << 32).wrapping_add(i64::from(self.next_i32()))
    }
    pub fn next_double(&mut self) -> f64 {
        let high = u64::from(self.bits(26));
        let low = u64::from(self.bits(27));
        ((high << 27) + low) as f64 / ((1u64 << 53) as f64)
    }
    pub fn next_bound(&mut self, bound: i32) -> Option<i32> {
        if bound <= 0 {
            return None;
        }
        if bound & (bound - 1) == 0 {
            return Some(((i64::from(bound) * i64::from(self.bits(31))) >> 31) as i32);
        }
        loop {
            let bits = self.bits(31) as i32;
            let value = bits % bound;
            if bits.wrapping_sub(value).wrapping_add(bound - 1) >= 0 {
                return Some(value);
            }
        }
    }
    pub fn rt4_bound(&mut self, bound: i32) -> Option<i32> {
        if bound <= 0 {
            return None;
        }
        if crate::int_utils::is_power_of_two(bound) {
            return Some((((self.next_i32() as u32 as u64) * bound as u64) >> 32) as i32);
        }
        let threshold = i32::MIN.wrapping_sub(((1u64 << 32) % bound as u64) as i32);
        loop {
            let sample = self.next_i32();
            if sample < threshold {
                return shuffle(sample, bound);
            }
        }
    }
}
pub fn shuffle(value: i32, range: i32) -> Option<i32> {
    if range <= 0 {
        return None;
    }
    let correction = (range - 1) & (value >> 31);
    Some(correction + value.wrapping_add(((value as u32) >> 31) as i32) % range)
}
