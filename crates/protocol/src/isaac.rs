//! ISAAC-32. RT4 consumes each 256-word block in reverse order.
use crate::{Error, Result};

#[derive(Clone)]
pub struct Isaac {
    memory: [u32; 256],
    results: [u32; 256],
    accumulator: u32,
    last: u32,
    counter: u32,
    remaining: usize,
}
fn mix(s: &mut [u32; 8]) {
    const SHIFTS: [u32; 8] = [11, 2, 8, 16, 10, 4, 8, 9];
    for i in 0..8 {
        s[i] ^= if i % 2 == 0 {
            s[(i + 1) % 8] << SHIFTS[i]
        } else {
            s[(i + 1) % 8] >> SHIFTS[i]
        };
        s[(i + 3) % 8] = s[(i + 3) % 8].wrapping_add(s[i]);
        s[(i + 1) % 8] = s[(i + 1) % 8].wrapping_add(s[(i + 2) % 8]);
    }
}
impl Isaac {
    pub fn new(seed: &[u32]) -> Result<Self> {
        if seed.len() > 256 {
            return Err(Error::new(0, "ISAAC seed exceeds 256 words"));
        }
        let mut result = Self {
            memory: [0; 256],
            results: [0; 256],
            accumulator: 0,
            last: 0,
            counter: 0,
            remaining: 256,
        };
        result.results[..seed.len()].copy_from_slice(seed);
        let mut state = [0x9e3779b9; 8];
        for _ in 0..4 {
            mix(&mut state);
        }
        for pass in 0..2 {
            for start in (0..256).step_by(8) {
                for (i, value) in state.iter_mut().enumerate() {
                    *value = value.wrapping_add(if pass == 0 {
                        result.results[start + i]
                    } else {
                        result.memory[start + i]
                    });
                }
                mix(&mut state);
                result.memory[start..start + 8].copy_from_slice(&state);
            }
        }
        result.generate();
        Ok(result)
    }
    fn generate(&mut self) {
        self.counter = self.counter.wrapping_add(1);
        self.last = self.last.wrapping_add(self.counter);
        for i in 0..256 {
            let x = self.memory[i];
            self.accumulator ^= match i & 3 {
                0 => self.accumulator << 13,
                1 => self.accumulator >> 6,
                2 => self.accumulator << 2,
                _ => self.accumulator >> 16,
            };
            self.accumulator = self.accumulator.wrapping_add(self.memory[(i + 128) & 255]);
            let y = self.memory[((x >> 2) & 255) as usize]
                .wrapping_add(self.accumulator)
                .wrapping_add(self.last);
            self.memory[i] = y;
            self.last = self.memory[((y >> 10) & 255) as usize].wrapping_add(x);
            self.results[i] = self.last;
        }
    }
    pub fn next_key(&mut self) -> u32 {
        if self.remaining == 0 {
            self.generate();
            self.remaining = 256;
        }
        self.remaining -= 1;
        self.results[self.remaining]
    }
}

/// Match GlobalConfig.USE_ISAAC, which is false in the pinned 2009scape profile.
/// Disabled connections avoid allocating cipher tables. Toggling does not consume keys.
pub struct OpcodeCipher {
    seed: Vec<u32>,
    generator: Option<Box<Isaac>>,
    enabled: bool,
}
impl OpcodeCipher {
    pub fn new(seed: &[u32], enabled: bool) -> Result<Self> {
        if seed.len() > 256 {
            return Err(Error::new(0, "ISAAC seed exceeds 256 words"));
        }
        Ok(Self {
            seed: seed.to_vec(),
            generator: None,
            enabled,
        })
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    pub fn next_key(&mut self) -> u32 {
        if !self.enabled {
            return 0;
        }
        self.generator
            .get_or_insert_with(|| Box::new(Isaac::new(&self.seed).expect("validated seed")))
            .next_key()
    }
    pub fn encode(&mut self, opcode: u8) -> u8 {
        opcode.wrapping_add(self.next_key() as u8)
    }
    pub fn decode(&mut self, opcode: u8) -> u8 {
        opcode.wrapping_sub(self.next_key() as u8)
    }
}
