//! The RT4 client's Vorbis sample decoder, transcribed from `VorbisSound`, `VorbisCodebook`,
//! `VorbisFloor`, `VorbisResidue` and `VorbisMapping` (original classes `jc`, `ji`, `ie`, `vb`
//! and `uk`).
//!
//! This is the original's reduced, non-standard decoder, not a general Vorbis implementation:
//! one channel, floor type 1 only, no end-of-packet tolerance, residue ranges that are not
//! clipped to the block, and a headerless container (JS5 archive 14: group 0 file 0 is the
//! shared setup, every other sound is its own group). Arithmetic keeps the Java widths and
//! evaluation order so that the 8-bit output matches the JVM bit for bit.
//!
//! Deliberate differences, all on input the original cannot decode either:
//! - wherever the original would throw (reading past a packet, an index outside an array, a
//!   division by zero, a negative array size) decoding returns an error instead, which
//!   [`VorbisSound::try_to_pcm`] reports and [`VorbisSound::to_pcm`] turns into a sound that
//!   is silent from the malformed packet on;
//! - [`Setup::decode`] rejects block sizes below 8 and a short block larger than the long
//!   block, a lookup-1 codebook with zero dimensions, and codebooks whose value table would
//!   exceed [`MAX_CODEBOOK_VALUES`]; the original accepts such a setup and fails, exhausts
//!   memory or hangs later;
//! - [`VorbisSound::decode`] rejects a negative sample count, which the original only trips
//!   over when it allocates the samples;
//! - a residue whose classification book has zero dimensions is an error; the original never
//!   returns from it.
//!
//! The original keeps the setup, the transform vector and the floor scratch arrays in
//! statics. Here the setup is the immutable [`Setup`] and the working arrays belong to each
//! sound's decode state. No value of those arrays survives from one packet to a later read
//! in the original, so the results are the same.

use std::f64::consts::{FRAC_PI_2, PI};

/// Largest `entries * dimensions` accepted for one codebook's value table.
pub const MAX_CODEBOOK_VALUES: usize = 1 << 24;

type Fault = &'static str;

const OVERRUN: Fault = "read past the end of the data";
const OUT_OF_RANGE: Fault = "index outside the original's array";
const DIVIDE_BY_ZERO: Fault = "division by zero";
const NEGATIVE_SIZE: Fault = "negative array size";

fn at<T>(slice: &[T], index: i32) -> Result<&T, Fault> {
    usize::try_from(index)
        .ok()
        .and_then(|index| slice.get(index))
        .ok_or(OUT_OF_RANGE)
}

fn at_mut<T>(slice: &mut [T], index: i32) -> Result<&mut T, Fault> {
    usize::try_from(index)
        .ok()
        .and_then(|index| slice.get_mut(index))
        .ok_or(OUT_OF_RANGE)
}

/// `IntUtils.bitCount`: the number of bits needed to hold `value`.
fn bit_count(mut value: i32) -> i32 {
    let mut count = 0;
    if !(0..65536).contains(&value) {
        count += 16;
        value = ((value as u32) >> 16) as i32;
    }
    if value >= 256 {
        count += 8;
        value >>= 8;
    }
    if value >= 16 {
        count += 4;
        value >>= 4;
    }
    if value >= 4 {
        value >>= 2;
        count += 2;
    }
    if value >= 1 {
        value >>= 1;
        count += 1;
    }
    value + count
}

/// `IntUtils.bitReverse`.
fn bit_reverse(mut bits: i32, mut value: i32) -> i32 {
    let mut result = 0;
    while bits > 0 {
        result = result << 1 | value & 1;
        value = ((value as u32) >> 1) as i32;
        bits -= 1;
    }
    result
}

/// `IntUtils.pow`, with Java's wrapping `int` products.
fn int_pow(mut exponent: i32, mut base: i32) -> i32 {
    let mut result = 1i32;
    while exponent > 1 {
        if exponent & 1 != 0 {
            result = result.wrapping_mul(base);
        }
        base = base.wrapping_mul(base);
        exponent >>= 1;
    }
    if exponent == 1 {
        result.wrapping_mul(base)
    } else {
        result
    }
}

/// `VorbisSound.float32Unpack`.
pub fn float32_unpack(x: i32) -> f32 {
    let mut mantissa = x & 0x1F_FFFF;
    let sign = x & i32::MIN;
    let exponent = (x >> 21) & 0x3FF;
    if sign != 0 {
        mantissa = -mantissa;
    }
    // `Math.pow(2.0, exponent - 788)` is an exact power of two between 2^-788 and 2^235.
    let scale = f64::from_bits(((exponent - 788 + 1023) as u64) << 52);
    (f64::from(mantissa) * scale) as f32
}

/// The static bit cursor of `VorbisSound` (`setBytes`, `readBit`, `readBits`): least
/// significant bit first.
struct BitReader<'a> {
    bytes: &'a [u8],
    position: usize,
    bit_position: u32,
}

impl<'a> BitReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            position: 0,
            bit_position: 0,
        }
    }

    fn byte(&self) -> Result<u32, Fault> {
        self.bytes
            .get(self.position)
            .map(|&byte| u32::from(byte))
            .ok_or(OVERRUN)
    }

    fn read_bit(&mut self) -> Result<i32, Fault> {
        let bit = (self.byte()? >> self.bit_position) & 1;
        self.bit_position += 1;
        self.position += (self.bit_position >> 3) as usize;
        self.bit_position &= 7;
        Ok(bit as i32)
    }

    /// As in the original, a read that ends exactly on a byte boundary does not touch the
    /// following byte, so it succeeds at the very end of the data.
    fn read_bits(&mut self, mut len: i32) -> Result<i32, Fault> {
        let mut result = 0u32;
        let mut shift = 0u32;
        while len >= 8 - self.bit_position as i32 {
            let available = 8 - self.bit_position;
            let mask = (1u32 << available) - 1;
            let bits = (self.byte()? >> self.bit_position) & mask;
            result = result.wrapping_add(bits.wrapping_shl(shift));
            self.bit_position = 0;
            self.position += 1;
            shift += available;
            len -= available as i32;
        }
        if len > 0 {
            let mask = (1u32 << len) - 1;
            let bits = (self.byte()? >> self.bit_position) & mask;
            result = result.wrapping_add(bits.wrapping_shl(shift));
            self.bit_position += len as u32;
        }
        Ok(result as i32)
    }
}

/// `VorbisCodebook`.
#[derive(Clone, Debug)]
struct Codebook {
    dimensions: i32,
    /// A node holds the index of its one-branch, its zero-branch is the next node, and a
    /// negative node is the complemented entry.
    entry_tree: Vec<i32>,
    /// `valueVector`, flattened to `entries * dimensions`; absent for lookup type 0.
    value_vector: Option<Vec<f32>>,
}

impl Codebook {
    fn decode(reader: &mut BitReader<'_>) -> Result<Self, Fault> {
        reader.read_bits(24)?;
        let dimensions = reader.read_bits(16)?;
        let entries = reader.read_bits(24)?;
        let mut lengths = vec![0i32; entries as usize];
        let ordered = reader.read_bit()? != 0;
        if ordered {
            let mut current_entry = 0;
            let mut current_length = reader.read_bits(5)? + 1;
            while current_entry < entries {
                let count = reader.read_bits(bit_count(entries - current_entry))?;
                for _ in 0..count {
                    *at_mut(&mut lengths, current_entry)? = current_length;
                    current_entry += 1;
                }
                current_length += 1;
            }
        } else {
            let sparse = reader.read_bit()? != 0;
            for length in &mut lengths {
                *length = if sparse && reader.read_bit()? == 0 {
                    0
                } else {
                    reader.read_bits(5)? + 1
                };
            }
        }
        let entry_tree = Self::create_entry_tree(&lengths)?;
        let lookup_type = reader.read_bits(4)?;
        let mut value_vector = None;
        if lookup_type > 0 {
            let minimum_value = float32_unpack(reader.read_bits(32)?);
            let delta_value = float32_unpack(reader.read_bits(32)?);
            let value_bits = reader.read_bits(4)? + 1;
            let sequence_p = reader.read_bit()? != 0;
            let lookup_values = if lookup_type == 1 {
                if dimensions == 0 {
                    return Err("lookup-1 codebook with zero dimensions");
                }
                lookup1_values(entries, dimensions)
            } else {
                entries.wrapping_mul(dimensions)
            };
            if lookup_values < 0 {
                return Err(NEGATIVE_SIZE);
            }
            let mut multiplicands = Vec::new();
            for _ in 0..lookup_values {
                multiplicands.push(reader.read_bits(value_bits)?);
            }
            let total = (entries as usize)
                .checked_mul(dimensions as usize)
                .filter(|&total| total <= MAX_CODEBOOK_VALUES)
                .ok_or("codebook value table exceeds limit")?;
            let mut values = Vec::with_capacity(total);
            // Lookup types above 2 take the type 2 path, as in the original.
            for entry in 0..entries {
                let mut last = 0.0f32;
                let mut index_divisor = if lookup_type == 1 {
                    1
                } else {
                    entry.wrapping_mul(dimensions)
                };
                for _ in 0..dimensions {
                    let multiplicand = if lookup_type == 1 {
                        if index_divisor == 0 || lookup_values == 0 {
                            return Err(DIVIDE_BY_ZERO);
                        }
                        let lookup_offset = entry
                            .wrapping_div(index_divisor)
                            .wrapping_rem(lookup_values);
                        *at(&multiplicands, lookup_offset)?
                    } else {
                        *at(&multiplicands, index_divisor)?
                    };
                    let value = multiplicand as f32 * delta_value + minimum_value + last;
                    values.push(value);
                    if sequence_p {
                        last = value;
                    }
                    index_divisor = if lookup_type == 1 {
                        index_divisor.wrapping_mul(lookup_values)
                    } else {
                        index_divisor.wrapping_add(1)
                    };
                }
            }
            value_vector = Some(values);
        }
        Ok(Self {
            dimensions,
            entry_tree,
            value_vector,
        })
    }

    fn create_entry_tree(lengths: &[i32]) -> Result<Vec<i32>, Fault> {
        let mut codewords = vec![0i32; lengths.len()];
        let mut current_codes = [0i32; 33];
        for (i, &length) in lengths.iter().enumerate() {
            if length == 0 {
                continue;
            }
            if !(1..=32).contains(&length) {
                return Err(OUT_OF_RANGE);
            }
            let length = length as usize;
            let high_bit = 1i32 << (32 - length);
            let code = current_codes[length];
            codewords[i] = code;
            let next_code;
            if code & high_bit == 0 {
                next_code = code | high_bit;
                let mut depth = length - 1;
                while depth >= 1 {
                    let previous = current_codes[depth];
                    if previous != code {
                        break;
                    }
                    let bit = 1i32 << (32 - depth);
                    if previous & bit != 0 {
                        current_codes[depth] = current_codes[depth - 1];
                        break;
                    }
                    current_codes[depth] = previous | bit;
                    depth -= 1;
                }
            } else {
                next_code = current_codes[length - 1];
            }
            current_codes[length] = next_code;
            for slot in &mut current_codes[length + 1..] {
                if *slot == code {
                    *slot = next_code;
                }
            }
        }
        let mut entry_tree = vec![0i32; 8];
        let mut tree_size = 0i32;
        for (i, &length) in lengths.iter().enumerate() {
            if length == 0 {
                continue;
            }
            let codeword = codewords[i];
            let mut code = 0i32;
            for bit in 0..length {
                let mask = (0x8000_0000u32 >> bit) as i32;
                if codeword & mask == 0 {
                    code += 1;
                } else {
                    let node = at_mut(&mut entry_tree, code)?;
                    if *node == 0 {
                        *node = tree_size;
                    }
                    code = *node;
                }
                if code >= entry_tree.len() as i32 {
                    // Doubled once only; a jump beyond that is an error on the next access.
                    entry_tree.resize(entry_tree.len() * 2, 0);
                }
            }
            *at_mut(&mut entry_tree, code)? = !(i as i32);
            if code >= tree_size {
                tree_size = code + 1;
            }
        }
        Ok(entry_tree)
    }

    fn decode_scalar(&self, reader: &mut BitReader<'_>) -> Result<i32, Fault> {
        let mut node = 0usize;
        loop {
            let value = *self.entry_tree.get(node).ok_or(OUT_OF_RANGE)?;
            if value < 0 {
                return Ok(!value);
            }
            node = if reader.read_bit()? == 0 {
                node + 1
            } else {
                value as usize
            };
        }
    }

    fn decode_vq(&self, reader: &mut BitReader<'_>) -> Result<&[f32], Fault> {
        let entry = self.decode_scalar(reader)? as usize;
        let values = self
            .value_vector
            .as_ref()
            .ok_or("vector decode from a codebook without values")?;
        let dimensions = self.dimensions as usize;
        values
            .get(entry * dimensions..(entry + 1) * dimensions)
            .ok_or(OUT_OF_RANGE)
    }
}

/// `VorbisCodebook.lookup1Values`.
fn lookup1_values(entries: i32, dimensions: i32) -> i32 {
    let root = f64::from(entries).powf(1.0 / f64::from(dimensions));
    let mut value = (root as i32).wrapping_add(1);
    while int_pow(dimensions, value) > entries {
        value = value.wrapping_sub(1);
    }
    value
}

const RANGES: [i32; 4] = [256, 128, 86, 64];

#[rustfmt::skip]
const INVERSE_DB_TABLE: [f32; 256] = [
    1.0649863e-7, 1.1341951e-7, 1.2079015e-7, 1.2863978e-7, 1.369995e-7, 1.459025e-7,
    1.5538409e-7, 1.6548181e-7, 1.7623574e-7, 1.8768856e-7, 1.998856e-7, 2.128753e-7,
    2.2670913e-7, 2.4144197e-7, 2.5713223e-7, 2.7384212e-7, 2.9163792e-7, 3.1059022e-7,
    3.307741e-7, 3.5226967e-7, 3.7516213e-7, 3.995423e-7, 4.255068e-7, 4.5315863e-7,
    4.8260745e-7, 5.1397e-7, 5.4737063e-7, 5.829419e-7, 6.208247e-7, 6.611694e-7,
    7.041359e-7, 7.4989464e-7, 7.98627e-7, 8.505263e-7, 9.057983e-7, 9.646621e-7,
    1.0273513e-6, 1.0941144e-6, 1.1652161e-6, 1.2409384e-6, 1.3215816e-6, 1.4074654e-6,
    1.4989305e-6, 1.5963394e-6, 1.7000785e-6, 1.8105592e-6, 1.9282195e-6, 2.053526e-6,
    2.1869757e-6, 2.3290977e-6, 2.4804558e-6, 2.6416496e-6, 2.813319e-6, 2.9961443e-6,
    3.1908505e-6, 3.39821e-6, 3.619045e-6, 3.8542307e-6, 4.1047006e-6, 4.371447e-6,
    4.6555283e-6, 4.958071e-6, 5.280274e-6, 5.623416e-6, 5.988857e-6, 6.3780467e-6,
    6.7925284e-6, 7.2339453e-6, 7.704048e-6, 8.2047e-6, 8.737888e-6, 9.305725e-6,
    9.910464e-6, 1.0554501e-5, 1.1240392e-5, 1.1970856e-5, 1.2748789e-5, 1.3577278e-5,
    1.4459606e-5, 1.5399271e-5, 1.6400005e-5, 1.7465769e-5, 1.8600793e-5, 1.9809577e-5,
    2.1096914e-5, 2.2467912e-5, 2.3928002e-5, 2.5482977e-5, 2.7139005e-5, 2.890265e-5,
    3.078091e-5, 3.2781227e-5, 3.4911533e-5, 3.718028e-5, 3.9596467e-5, 4.2169668e-5,
    4.491009e-5, 4.7828602e-5, 5.0936775e-5, 5.424693e-5, 5.7772202e-5, 6.152657e-5,
    6.552491e-5, 6.9783084e-5, 7.4317984e-5, 7.914758e-5, 8.429104e-5, 8.976875e-5,
    9.560242e-5, 1.0181521e-4, 1.0843174e-4, 1.1547824e-4, 1.2298267e-4, 1.3097477e-4,
    1.3948625e-4, 1.4855085e-4, 1.5820454e-4, 1.6848555e-4, 1.7943469e-4, 1.9109536e-4,
    2.0351382e-4, 2.167393e-4, 2.3082423e-4, 2.4582449e-4, 2.6179955e-4, 2.7881275e-4,
    2.9693157e-4, 3.1622787e-4, 3.3677815e-4, 3.5866388e-4, 3.8197188e-4, 4.0679457e-4,
    4.3323037e-4, 4.613841e-4, 4.913675e-4, 5.2329927e-4, 5.573062e-4, 5.935231e-4,
    6.320936e-4, 6.731706e-4, 7.16917e-4, 7.635063e-4, 8.1312325e-4, 8.6596457e-4,
    9.2223985e-4, 9.821722e-4, 0.0010459992, 0.0011139743, 0.0011863665, 0.0012634633,
    0.0013455702, 0.0014330129, 0.0015261382, 0.0016253153, 0.0017309374, 0.0018434235,
    0.0019632196, 0.0020908006, 0.0022266726, 0.0023713743, 0.0025254795, 0.0026895993,
    0.0028643848, 0.0030505287, 0.003248769, 0.0034598925, 0.0036847359, 0.0039241905,
    0.0041792067, 0.004450795, 0.004740033, 0.005048067, 0.0053761187, 0.005725489,
    0.0060975635, 0.0064938175, 0.0069158226, 0.0073652514, 0.007843887, 0.008353627,
    0.008896492, 0.009474637, 0.010090352, 0.01074608, 0.011444421, 0.012188144,
    0.012980198, 0.013823725, 0.014722068, 0.015678791, 0.016697686, 0.017782796,
    0.018938422, 0.020169148, 0.021479854, 0.022875736, 0.02436233, 0.025945531,
    0.027631618, 0.029427277, 0.031339627, 0.03337625, 0.035545226, 0.037855156,
    0.0403152, 0.042935107, 0.045725275, 0.048696756, 0.05186135, 0.05523159,
    0.05882085, 0.062643364, 0.06671428, 0.07104975, 0.075666964, 0.08058423,
    0.08582105, 0.09139818, 0.097337745, 0.1036633, 0.11039993, 0.11757434,
    0.12521498, 0.13335215, 0.14201812, 0.15124726, 0.16107617, 0.1715438,
    0.18269168, 0.19456401, 0.20720787, 0.22067343, 0.23501402, 0.25028655,
    0.26655158, 0.28387362, 0.3023213, 0.32196787, 0.34289113, 0.36517414,
    0.3889052, 0.41417846, 0.44109413, 0.4697589, 0.50028646, 0.53279793,
    0.5674221, 0.6042964, 0.64356697, 0.6853896, 0.72993004, 0.777365,
    0.8278826, 0.88168305, 0.9389798, 1.0,
];

fn inverse_db(y: i32) -> Result<f32, Fault> {
    at(&INVERSE_DB_TABLE, y).copied()
}

/// `VorbisFloor.currentXList`, `y` and `step2Flag`: one packet's floor curve.
#[derive(Clone, Debug)]
struct FloorScratch {
    x: Vec<i32>,
    y: Vec<i32>,
    step2: Vec<bool>,
}

impl FloorScratch {
    fn new(points: usize) -> Self {
        Self {
            x: vec![0; points],
            y: vec![0; points],
            step2: vec![false; points],
        }
    }

    /// `VorbisFloor.sort`: the original's quicksort of the three parallel arrays by x.
    fn sort(&mut self, low: i32, high: i32) {
        if low >= high {
            return;
        }
        let mut pivot_index = low as usize;
        let pivot_x = self.x[pivot_index];
        let pivot_y = self.y[pivot_index];
        let pivot_flag = self.step2[pivot_index];
        for i in low as usize + 1..=high as usize {
            let x = self.x[i];
            if x < pivot_x {
                self.x[pivot_index] = x;
                self.y[pivot_index] = self.y[i];
                self.step2[pivot_index] = self.step2[i];
                pivot_index += 1;
                self.x[i] = self.x[pivot_index];
                self.y[i] = self.y[pivot_index];
                self.step2[i] = self.step2[pivot_index];
            }
        }
        self.x[pivot_index] = pivot_x;
        self.y[pivot_index] = pivot_y;
        self.step2[pivot_index] = pivot_flag;
        self.sort(low, pivot_index as i32 - 1);
        self.sort(pivot_index as i32 + 1, high);
    }
}

/// `VorbisFloor`: floor type 1.
#[derive(Clone, Debug)]
struct Floor {
    class_list: Vec<i32>,
    class_dimensions: Vec<i32>,
    subclasses: Vec<i32>,
    class_master_books: Vec<i32>,
    subclass_books: Vec<Vec<i32>>,
    multiplier: i32,
    x_list: Vec<i32>,
}

impl Floor {
    fn decode(reader: &mut BitReader<'_>) -> Result<Self, Fault> {
        if reader.read_bits(16)? != 1 {
            return Err("floor type is not 1");
        }
        let partition_count = reader.read_bits(5)?;
        let mut max_class = 0;
        let mut class_list = Vec::new();
        for _ in 0..partition_count {
            let class = reader.read_bits(4)?;
            class_list.push(class);
            if class >= max_class {
                max_class = class + 1;
            }
        }
        let mut class_dimensions = Vec::new();
        let mut subclasses = Vec::new();
        let mut class_master_books = Vec::new();
        let mut subclass_books = Vec::new();
        for _ in 0..max_class {
            class_dimensions.push(reader.read_bits(3)? + 1);
            let subclass_bits = reader.read_bits(2)?;
            subclasses.push(subclass_bits);
            class_master_books.push(if subclass_bits != 0 {
                reader.read_bits(8)?
            } else {
                0
            });
            let mut books = Vec::new();
            for _ in 0..1 << subclass_bits {
                books.push(reader.read_bits(8)? - 1);
            }
            subclass_books.push(books);
        }
        let multiplier = reader.read_bits(2)? + 1;
        let range_bits = reader.read_bits(4)?;
        let mut x_list = vec![0, 1 << range_bits];
        for &class in &class_list {
            for _ in 0..class_dimensions[class as usize] {
                x_list.push(reader.read_bits(range_bits)?);
            }
        }
        Ok(Self {
            class_list,
            class_dimensions,
            subclasses,
            class_master_books,
            subclass_books,
            multiplier,
            x_list,
        })
    }

    /// `decodePacket`: false when the packet marks the floor unused.
    fn decode_packet(
        &self,
        reader: &mut BitReader<'_>,
        codebooks: &[Codebook],
        scratch: &mut FloorScratch,
    ) -> Result<bool, Fault> {
        if reader.read_bit()? == 0 {
            return Ok(false);
        }
        scratch.x[..self.x_list.len()].copy_from_slice(&self.x_list);
        let range = RANGES[(self.multiplier - 1) as usize];
        let bits_per_y = bit_count(range - 1);
        scratch.y[0] = reader.read_bits(bits_per_y)?;
        scratch.y[1] = reader.read_bits(bits_per_y)?;
        let mut offset = 2;
        for &class in &self.class_list {
            let class = class as usize;
            let subclass_bits = self.subclasses[class];
            let subclass_mask = (1 << subclass_bits) - 1;
            let mut codeword = 0;
            if subclass_bits > 0 {
                codeword = at(codebooks, self.class_master_books[class])?.decode_scalar(reader)?;
            }
            for _ in 0..self.class_dimensions[class] {
                let book = self.subclass_books[class][(codeword & subclass_mask) as usize];
                codeword = ((codeword as u32) >> subclass_bits) as i32;
                scratch.y[offset] = if book >= 0 {
                    at(codebooks, book)?.decode_scalar(reader)?
                } else {
                    0
                };
                offset += 1;
            }
        }
        Ok(true)
    }

    /// `lowNeighbour`.
    fn low_neighbour(x_list: &[i32], index: usize) -> i32 {
        let target = x_list[index];
        let mut result = -1;
        let mut best = i32::MIN;
        for (i, &x) in x_list[..index].iter().enumerate() {
            if x < target && x > best {
                result = i as i32;
                best = x;
            }
        }
        result
    }

    /// `highNeighbour`.
    fn high_neighbour(x_list: &[i32], index: usize) -> i32 {
        let target = x_list[index];
        let mut result = -1;
        let mut best = i32::MAX;
        for (i, &x) in x_list[..index].iter().enumerate() {
            if x > target && x < best {
                result = i as i32;
                best = x;
            }
        }
        result
    }

    /// `renderPoint`.
    fn render_point(x0: i32, y0: i32, x1: i32, y1: i32, x: i32) -> Result<i32, Fault> {
        let dy = y1.wrapping_sub(y0);
        let dx = x1.wrapping_sub(x0);
        let abs_dy = if dy < 0 { dy.wrapping_neg() } else { dy };
        let scaled = abs_dy.wrapping_mul(x.wrapping_sub(x0));
        if dx == 0 {
            return Err(DIVIDE_BY_ZERO);
        }
        let interpolated = scaled.wrapping_div(dx);
        Ok(if dy < 0 {
            y0.wrapping_sub(interpolated)
        } else {
            y0.wrapping_add(interpolated)
        })
    }

    /// `renderLine`: multiplies `output[x0..min(x1, limit)]` by the line's decibel values.
    fn render_line(
        x0: i32,
        y0: i32,
        mut x1: i32,
        y1: i32,
        output: &mut [f32],
        limit: i32,
    ) -> Result<(), Fault> {
        let dy = y1.wrapping_sub(y0);
        let dx = x1.wrapping_sub(x0);
        let abs_dy = if dy < 0 { dy.wrapping_neg() } else { dy };
        if dx == 0 {
            return Err(DIVIDE_BY_ZERO);
        }
        let slope = dy.wrapping_div(dx);
        let mut current_y = y0;
        let mut error = 0i32;
        let slope_adjusted = if dy < 0 {
            slope.wrapping_sub(1)
        } else {
            slope.wrapping_add(1)
        };
        let abs_slope = if slope < 0 {
            slope.wrapping_neg()
        } else {
            slope
        };
        let error_increment = abs_dy.wrapping_sub(abs_slope.wrapping_mul(dx));
        *at_mut(output, x0)? *= inverse_db(y0)?;
        if x1 > limit {
            x1 = limit;
        }
        for x in x0.wrapping_add(1)..x1 {
            error = error.wrapping_add(error_increment);
            if error >= dx {
                error = error.wrapping_sub(dx);
                current_y = current_y.wrapping_add(slope_adjusted);
            } else {
                current_y = current_y.wrapping_add(slope);
            }
            *at_mut(output, x)? *= inverse_db(current_y)?;
        }
        Ok(())
    }

    /// `synthesize`: applies the decoded curve to the first `block_size` residue values.
    fn synthesize(
        &self,
        output: &mut [f32],
        block_size: i32,
        scratch: &mut FloorScratch,
    ) -> Result<(), Fault> {
        let x_count = self.x_list.len();
        let range = RANGES[(self.multiplier - 1) as usize];
        scratch.step2[0] = true;
        scratch.step2[1] = true;
        for i in 2..x_count {
            let low = Self::low_neighbour(&scratch.x, i);
            let high = Self::high_neighbour(&scratch.x, i);
            if low < 0 || high < 0 {
                return Err(OUT_OF_RANGE);
            }
            let (low, high) = (low as usize, high as usize);
            let predicted = Self::render_point(
                scratch.x[low],
                scratch.y[low],
                scratch.x[high],
                scratch.y[high],
                scratch.x[i],
            )?;
            let actual = scratch.y[i];
            let high_room = range.wrapping_sub(predicted);
            let low_room = (if high_room < predicted {
                high_room
            } else {
                predicted
            })
            .wrapping_shl(1);
            if actual == 0 {
                scratch.step2[i] = false;
                scratch.y[i] = predicted;
            } else {
                scratch.step2[low] = true;
                scratch.step2[high] = true;
                scratch.step2[i] = true;
                scratch.y[i] = if actual >= low_room {
                    if high_room > predicted {
                        // The original's `actual + predicted - predicted`.
                        actual.wrapping_add(predicted).wrapping_sub(predicted)
                    } else {
                        predicted
                            .wrapping_sub(actual)
                            .wrapping_add(high_room)
                            .wrapping_sub(1)
                    }
                } else if actual & 1 == 0 {
                    predicted.wrapping_add(actual / 2)
                } else {
                    predicted.wrapping_sub(actual.wrapping_add(1) / 2)
                };
            }
        }
        scratch.sort(0, x_count as i32 - 1);
        let mut x0 = 0;
        let mut y0 = scratch.y[0].wrapping_mul(self.multiplier);
        for i in 1..x_count {
            if scratch.step2[i] {
                let x1 = scratch.x[i];
                let y1 = scratch.y[i].wrapping_mul(self.multiplier);
                Self::render_line(x0, y0, x1, y1, output, block_size)?;
                if x1 >= block_size {
                    return Ok(());
                }
                x0 = x1;
                y0 = y1;
            }
        }
        let last = inverse_db(y0)?;
        for x in x0..block_size {
            *at_mut(output, x)? *= last;
        }
        Ok(())
    }
}

/// `VorbisResidue`.
#[derive(Clone, Debug)]
struct Residue {
    kind: i32,
    begin: i32,
    end: i32,
    partition_size: i32,
    classifications: i32,
    class_book: i32,
    books: Vec<i32>,
}

impl Residue {
    fn decode(reader: &mut BitReader<'_>) -> Result<Self, Fault> {
        let kind = reader.read_bits(16)?;
        let begin = reader.read_bits(24)?;
        let end = reader.read_bits(24)?;
        let partition_size = reader.read_bits(24)? + 1;
        let classifications = reader.read_bits(6)? + 1;
        let class_book = reader.read_bits(8)?;
        let mut cascades = Vec::new();
        for _ in 0..classifications {
            let mut high_bits = 0;
            let low_bits = reader.read_bits(3)?;
            if reader.read_bit()? != 0 {
                high_bits = reader.read_bits(5)?;
            }
            cascades.push(high_bits << 3 | low_bits);
        }
        let mut books = Vec::new();
        for i in 0..classifications as usize * 8 {
            books.push(if cascades[i >> 3] & 1 << (i & 7) == 0 {
                -1
            } else {
                reader.read_bits(8)?
            });
        }
        Ok(Self {
            kind,
            begin,
            end,
            partition_size,
            classifications,
            class_book,
            books,
        })
    }

    /// `synthesize`: clears `output[..size]` and adds the residue vectors. The partitions run
    /// to the residue's own end whatever the block size, as in the original.
    fn synthesize(
        &self,
        output: &mut [f32],
        size: usize,
        do_not_decode: bool,
        reader: &mut BitReader<'_>,
        codebooks: &[Codebook],
        classification_data: &mut Vec<i32>,
    ) -> Result<(), Fault> {
        output[..size].fill(0.0);
        if do_not_decode {
            return Ok(());
        }
        let class_book = at(codebooks, self.class_book)?;
        let class_dimensions = class_book.dimensions;
        let residue_size = self.end - self.begin;
        let partition_count = residue_size / self.partition_size;
        if partition_count < 0 {
            return Err(NEGATIVE_SIZE);
        }
        if partition_count > 0 && class_dimensions == 0 {
            return Err("residue classification book has zero dimensions");
        }
        classification_data.clear();
        classification_data.resize(partition_count as usize, 0);
        for pass in 0..8 {
            let mut partition = 0;
            while partition < partition_count {
                if pass == 0 {
                    let mut codeword = class_book.decode_scalar(reader)?;
                    for j in (0..class_dimensions).rev() {
                        if partition + j < partition_count {
                            classification_data[(partition + j) as usize] =
                                codeword % self.classifications;
                        }
                        codeword /= self.classifications;
                    }
                }
                for _ in 0..class_dimensions {
                    let class = classification_data[partition as usize];
                    let book = self.books[(class * 8 + pass) as usize];
                    if book >= 0 {
                        let offset = self.begin + partition * self.partition_size;
                        let codebook = at(codebooks, book)?;
                        // The original divides by zero for type 0 and otherwise reads vectors
                        // until the packet runs out.
                        if codebook.dimensions == 0 {
                            return Err("residue book has zero dimensions");
                        }
                        if self.kind == 0 {
                            let steps = self.partition_size / codebook.dimensions;
                            for step in 0..steps {
                                let vector = codebook.decode_vq(reader)?;
                                for (dimension, &value) in vector.iter().enumerate() {
                                    let index = offset + step + dimension as i32 * steps;
                                    *at_mut(output, index)? += value;
                                }
                            }
                        } else {
                            let mut k = 0;
                            while k < self.partition_size {
                                for &value in codebook.decode_vq(reader)? {
                                    *at_mut(output, offset + k)? += value;
                                    k += 1;
                                }
                            }
                        }
                    }
                    partition += 1;
                    if partition >= partition_count {
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}

/// `VorbisMapping`. The decoder is mono: `mux` is the one channel's submap.
#[derive(Clone, Debug)]
struct Mapping {
    submaps: i32,
    mux: i32,
    submap_floor: Vec<i32>,
    submap_residue: Vec<i32>,
}

impl Mapping {
    fn decode(reader: &mut BitReader<'_>) -> Result<Self, Fault> {
        reader.read_bits(16)?;
        let submaps = if reader.read_bit()? == 0 {
            1
        } else {
            reader.read_bits(4)? + 1
        };
        if reader.read_bit()? != 0 {
            reader.read_bits(8)?;
        }
        reader.read_bits(2)?;
        let mut mux = 0;
        if submaps > 1 {
            mux = reader.read_bits(4)?;
        }
        let mut submap_floor = Vec::new();
        let mut submap_residue = Vec::new();
        for _ in 0..submaps {
            reader.read_bits(8)?;
            submap_floor.push(reader.read_bits(8)?);
            submap_residue.push(reader.read_bits(8)?);
        }
        Ok(Self {
            submaps,
            mux,
            submap_floor,
            submap_residue,
        })
    }
}

/// The per-block-size statics of `VorbisSound` (`trigA0`, `trigB0`, `trigC0`, `bitReverse0`
/// and their long-block twins), plus the two window slopes. The original evaluates the window
/// sines for every sample of every packet; they depend only on the slope length, so they are
/// computed once here with the same expressions.
#[derive(Clone, Debug)]
struct Tables {
    trig_a: Vec<f32>,
    trig_b: Vec<f32>,
    trig_c: Vec<f32>,
    bit_reverse: Vec<usize>,
    /// Rising slope over `block_size / 2` samples.
    window_left: Vec<f32>,
    /// Falling slope over `block_size / 2` samples.
    window_right: Vec<f32>,
}

impl Tables {
    fn new(block_size: i32) -> Self {
        let half = (block_size >> 1) as usize;
        let quarter = (block_size >> 2) as usize;
        let eighth = (block_size >> 3) as usize;
        let size = f64::from(block_size);
        let mut trig_a = vec![0.0f32; half];
        for i in 0..quarter {
            trig_a[i * 2] = ((i * 4) as f64 * PI / size).cos() as f32;
            trig_a[i * 2 + 1] = -(((i * 4) as f64 * PI / size).sin() as f32);
        }
        let double_size = f64::from(block_size * 2);
        let mut trig_b = vec![0.0f32; half];
        for i in 0..quarter {
            trig_b[i * 2] = ((i * 2 + 1) as f64 * PI / double_size).cos() as f32;
            trig_b[i * 2 + 1] = ((i * 2 + 1) as f64 * PI / double_size).sin() as f32;
        }
        let mut trig_c = vec![0.0f32; quarter];
        for i in 0..eighth {
            trig_c[i * 2] = ((i * 4 + 2) as f64 * PI / size).cos() as f32;
            trig_c[i * 2 + 1] = -(((i * 4 + 2) as f64 * PI / size).sin() as f32);
        }
        let log = bit_count(eighth as i32 - 1);
        let bit_reverse = (0..eighth as i32)
            .map(|i| bit_reverse(log, i) as usize)
            .collect();
        let slope = |phase: f64| {
            (0..half)
                .map(|i| {
                    let b = ((i as f64 + 0.5) / half as f64 * 0.5 * PI + phase).sin() as f32;
                    (f64::from(b) * FRAC_PI_2 * f64::from(b)).sin() as f32
                })
                .collect::<Vec<f32>>()
        };
        Self {
            trig_a,
            trig_b,
            trig_c,
            bit_reverse,
            // The original's left slope has no phase term at all; adding 0.0 is exact.
            window_left: slope(0.0),
            window_right: slope(FRAC_PI_2),
        }
    }
}

/// The inverse transform of `VorbisSound.decodePacket`: `vec[..n / 2]` holds the spectrum on
/// entry and `vec[..n]` the unwindowed block on return.
fn inverse_transform(vec: &mut [f32], tables: &Tables) {
    let n = vec.len();
    let n2 = n >> 1;
    let n4 = n >> 2;
    let n8 = n >> 3;
    let trig_a = &tables.trig_a[..];
    let trig_b = &tables.trig_b[..];
    let trig_c = &tables.trig_c[..];
    for value in &mut vec[..n2] {
        *value *= 0.5;
    }
    for k in n2..n {
        vec[k] = -vec[n - k - 1];
    }
    for k in 0..n4 {
        let a = vec[k * 4] - vec[n - k * 4 - 1];
        let b = vec[k * 4 + 2] - vec[n - k * 4 - 3];
        let c = trig_a[k * 2];
        let d = trig_a[k * 2 + 1];
        vec[n - k * 4 - 1] = a * c - b * d;
        vec[n - k * 4 - 3] = a * d + b * c;
    }
    for k in 0..n8 {
        let a = vec[n2 + k * 4 + 3];
        let b = vec[n2 + k * 4 + 1];
        let c = vec[k * 4 + 3];
        let d = vec[k * 4 + 1];
        vec[n2 + k * 4 + 3] = a + c;
        vec[n2 + k * 4 + 1] = b + d;
        let e = trig_a[n2 - k * 4 - 4];
        let f = trig_a[n2 - k * 4 - 3];
        vec[k * 4 + 3] = (a - c) * e - (b - d) * f;
        vec[k * 4 + 1] = (b - d) * e + (a - c) * f;
    }
    let log_n = bit_count(n as i32 - 1) as usize;
    for stage in 0..log_n.saturating_sub(3) {
        let stride = n >> (stage + 2);
        let half_stride = 8 << stage;
        for block in 0..2 << stage {
            let off1 = n - stride * 2 * block;
            let off2 = n - stride * (block * 2 + 1);
            for j in 0..n >> (stage + 4) {
                let idx = j * 4;
                let p = vec[off1 - idx - 1];
                let q = vec[off1 - idx - 3];
                let r = vec[off2 - idx - 1];
                let t = vec[off2 - idx - 3];
                vec[off1 - idx - 1] = p + r;
                vec[off1 - idx - 3] = q + t;
                let tw1 = trig_a[j * half_stride];
                let tw2 = trig_a[j * half_stride + 1];
                vec[off2 - idx - 1] = (p - r) * tw1 - (q - t) * tw2;
                vec[off2 - idx - 3] = (q - t) * tw1 + (p - r) * tw2;
            }
        }
    }
    for (i, &j) in tables.bit_reverse[..n8.saturating_sub(1)]
        .iter()
        .enumerate()
        .skip(1)
    {
        if i < j {
            vec.swap(i * 8 + 1, j * 8 + 1);
            vec.swap(i * 8 + 3, j * 8 + 3);
            vec.swap(i * 8 + 5, j * 8 + 5);
            vec.swap(i * 8 + 7, j * 8 + 7);
        }
    }
    for i in 0..n2 {
        vec[i] = vec[i * 2 + 1];
    }
    for i in 0..n8 {
        vec[n - i * 2 - 1] = vec[i * 4];
        vec[n - i * 2 - 2] = vec[i * 4 + 1];
        vec[n - n4 - i * 2 - 1] = vec[i * 4 + 2];
        vec[n - n4 - i * 2 - 2] = vec[i * 4 + 3];
    }
    for i in 0..n8 {
        let b = trig_c[i * 2];
        let c = trig_c[i * 2 + 1];
        let d = vec[n2 + i * 2];
        let e = vec[n2 + i * 2 + 1];
        let f = vec[n - i * 2 - 2];
        let g = vec[n - i * 2 - 1];
        let cross1 = c * (d - f) + b * (e + g);
        vec[n2 + i * 2] = (d + f + cross1) * 0.5;
        vec[n - i * 2 - 2] = (d + f - cross1) * 0.5;
        let cross2 = c * (e + g) - b * (d - f);
        vec[n2 + i * 2 + 1] = (e + cross2 - g) * 0.5;
        vec[n - i * 2 - 1] = (g + cross2 - e) * 0.5;
    }
    for i in 0..n4 {
        let even = vec[i * 2 + n2];
        let odd = vec[i * 2 + n2 + 1];
        vec[i] = even * trig_b[i * 2] + odd * trig_b[i * 2 + 1];
        vec[n2 - i - 1] = even * trig_b[i * 2 + 1] - odd * trig_b[i * 2];
    }
    for i in 0..n4 {
        vec[n + i - n4] = -vec[i];
    }
    for i in 0..n4 {
        vec[i] = vec[n4 + i];
    }
    for i in 0..n4 {
        vec[n4 + i] = -vec[n4 - i - 1];
    }
    for i in 0..n4 {
        vec[n2 + i] = vec[n - i - 1];
    }
}

/// The shared setup the original keeps in `VorbisSound` statics (codebooks, floors, residues,
/// mappings, modes, block sizes and the transform tables), decoded from archive 14 group 0
/// file 0 by `decodeCodebooks`.
#[derive(Clone, Debug)]
pub struct Setup {
    block_size_0: i32,
    block_size_1: i32,
    /// Short-block and long-block tables.
    tables: [Tables; 2],
    codebooks: Vec<Codebook>,
    floors: Vec<Floor>,
    residues: Vec<Residue>,
    mappings: Vec<Mapping>,
    mode_block_flags: Vec<bool>,
    mode_mappings: Vec<i32>,
    /// Length of the longest floor x list, the size of the original's static floor arrays.
    floor_points: usize,
}

impl Setup {
    pub fn decode(header: &[u8]) -> Result<Self, String> {
        Self::decode_header(header).map_err(|fault| format!("vorbis setup: {fault}"))
    }

    /// `blockSize0`: samples spanned by a short block.
    pub fn block_size_0(&self) -> i32 {
        self.block_size_0
    }

    /// `blockSize1`: samples spanned by a long block.
    pub fn block_size_1(&self) -> i32 {
        self.block_size_1
    }

    fn decode_header(header: &[u8]) -> Result<Self, Fault> {
        let reader = &mut BitReader::new(header);
        let block_size_0 = 1 << reader.read_bits(4)?;
        let block_size_1 = 1 << reader.read_bits(4)?;
        if block_size_0 < 8 || block_size_0 > block_size_1 {
            return Err("unsupported block sizes");
        }
        let tables = [Tables::new(block_size_0), Tables::new(block_size_1)];
        let mut codebooks = Vec::new();
        for _ in 0..reader.read_bits(8)? + 1 {
            codebooks.push(Codebook::decode(reader)?);
        }
        for _ in 0..reader.read_bits(6)? + 1 {
            reader.read_bits(16)?;
        }
        let mut floors = Vec::new();
        for _ in 0..reader.read_bits(6)? + 1 {
            floors.push(Floor::decode(reader)?);
        }
        let mut residues = Vec::new();
        for _ in 0..reader.read_bits(6)? + 1 {
            residues.push(Residue::decode(reader)?);
        }
        let mut mappings = Vec::new();
        for _ in 0..reader.read_bits(6)? + 1 {
            mappings.push(Mapping::decode(reader)?);
        }
        let mut mode_block_flags = Vec::new();
        let mut mode_mappings = Vec::new();
        for _ in 0..reader.read_bits(6)? + 1 {
            mode_block_flags.push(reader.read_bit()? != 0);
            reader.read_bits(16)?;
            reader.read_bits(16)?;
            mode_mappings.push(reader.read_bits(8)?);
        }
        let floor_points = floors
            .iter()
            .map(|floor| floor.x_list.len())
            .max()
            .unwrap_or(0);
        Ok(Self {
            block_size_0,
            block_size_1,
            tables,
            codebooks,
            floors,
            residues,
            mappings,
            mode_block_flags,
            mode_mappings,
            floor_points,
        })
    }
}

/// `PcmSound`'s constructor arguments as `VorbisSound.toPcmSound` passes them: signed 8-bit
/// mono samples at `rate`, with the loop points copied from the sound file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pcm {
    pub rate: i32,
    pub samples: Vec<i8>,
    pub start: i32,
    pub end: i32,
    pub ping_pong: bool,
}

/// The state `VorbisSound` keeps between calls of `toPcmSound` (`samples`, `sampleIndex`,
/// `packetIndex`, `prevVector`, `prevN`, `prevQuarter`, `prevNoResidue`) together with the
/// working arrays that are static in the original.
#[derive(Clone, Debug)]
struct Progress {
    samples: Vec<i8>,
    sample_index: i32,
    packet_index: usize,
    prev_n: usize,
    prev_quarter: usize,
    prev_no_residue: bool,
    prev_vector: Vec<f32>,
    vector: Vec<f32>,
    /// The packet's finished samples; the original allocates this array per packet.
    output: Vec<f32>,
    floor: FloorScratch,
    classification_data: Vec<i32>,
}

impl Progress {
    fn new(setup: &Setup, samples_len: i32) -> Result<Self, Fault> {
        let samples_len = usize::try_from(samples_len).map_err(|_| NEGATIVE_SIZE)?;
        let mut samples = Vec::new();
        samples
            .try_reserve_exact(samples_len)
            .map_err(|_| "sample buffer allocation failed")?;
        samples.resize(samples_len, 0);
        let block = setup.block_size_1 as usize;
        Ok(Self {
            samples,
            sample_index: 0,
            packet_index: 0,
            prev_n: 0,
            prev_quarter: 0,
            prev_no_residue: false,
            prev_vector: vec![0.0; block],
            vector: vec![0.0; block],
            output: Vec::new(),
            floor: FloorScratch::new(setup.floor_points),
            classification_data: Vec::new(),
        })
    }

    /// The packet loop of `toPcmSound`: `Ok(false)` when the budget ran out first.
    fn run(
        &mut self,
        setup: &Setup,
        packets: &[Vec<u8>],
        mut budget: Option<&mut i32>,
    ) -> Result<bool, Fault> {
        if self.vector.len() != setup.block_size_1 as usize
            || self.floor.x.len() != setup.floor_points
        {
            return Err("decode resumed with a different setup");
        }
        let samples_len = self.samples.len() as i32;
        while self.packet_index < packets.len() {
            if budget.as_deref().is_some_and(|budget| *budget <= 0) {
                return Ok(false);
            }
            if self.decode_packet(setup, &packets[self.packet_index])? {
                let mut write_position = self.sample_index;
                let mut sample_count = self.output.len() as i32;
                if sample_count > samples_len - write_position {
                    sample_count = samples_len - write_position;
                }
                for i in 0..sample_count {
                    self.samples[write_position as usize] = to_sample(self.output[i as usize]);
                    write_position += 1;
                }
                if let Some(budget) = budget.as_deref_mut() {
                    *budget = budget.wrapping_sub(write_position - self.sample_index);
                }
                self.sample_index = write_position;
            }
            self.packet_index += 1;
        }
        Ok(true)
    }

    /// `decodePacket`: decodes one block into `vector` and overlaps it with the previous one.
    /// False for the first packet, which only primes the overlap (the original's `null`).
    fn decode_packet(&mut self, setup: &Setup, packet: &[u8]) -> Result<bool, Fault> {
        let reader = &mut BitReader::new(packet);
        reader.read_bit()?;
        let mode_number = reader.read_bits(bit_count(setup.mode_mappings.len() as i32 - 1))?;
        let long = *at(&setup.mode_block_flags, mode_number)?;
        let block_size = if long {
            setup.block_size_1
        } else {
            setup.block_size_0
        };
        let n = block_size as usize;
        let mut previous_window_flag = false;
        let mut next_window_flag = false;
        if long {
            previous_window_flag = reader.read_bit()? != 0;
            next_window_flag = reader.read_bit()? != 0;
        }
        let short_quarter = (setup.block_size_0 >> 2) as usize;
        let window_center = n >> 1;
        let (left_start, left_end, left_tables) = if long && !previous_window_flag {
            ((n >> 2) - short_quarter, (n >> 2) + short_quarter, 0)
        } else {
            (0, window_center, usize::from(long))
        };
        let (right_start, right_end, right_tables) = if long && !next_window_flag {
            (
                n - (n >> 2) - short_quarter,
                n + short_quarter - (n >> 2),
                0,
            )
        } else {
            (window_center, n, usize::from(long))
        };
        let mapping = at(&setup.mappings, *at(&setup.mode_mappings, mode_number)?)?;
        let floor = at(&setup.floors, *at(&mapping.submap_floor, mapping.mux)?)?;
        let no_residue = !floor.decode_packet(reader, &setup.codebooks, &mut self.floor)?;
        for &residue in &mapping.submap_residue[..mapping.submaps as usize] {
            at(&setup.residues, residue)?.synthesize(
                &mut self.vector,
                n >> 1,
                no_residue,
                reader,
                &setup.codebooks,
                &mut self.classification_data,
            )?;
        }
        if no_residue {
            self.vector[n >> 1..n].fill(0.0);
        } else {
            floor.synthesize(&mut self.vector, (n >> 1) as i32, &mut self.floor)?;
            inverse_transform(&mut self.vector[..n], &setup.tables[usize::from(long)]);
            let left = &setup.tables[left_tables].window_left;
            for (value, window) in self.vector[left_start..left_end].iter_mut().zip(left) {
                *value *= *window;
            }
            let right = &setup.tables[right_tables].window_right;
            for (value, window) in self.vector[right_start..right_end].iter_mut().zip(right) {
                *value *= *window;
            }
        }
        let has_output = self.prev_n > 0;
        if has_output {
            let len = (self.prev_n + n) >> 2;
            self.output.clear();
            self.output.resize(len, 0.0);
            if !self.prev_no_residue {
                // A long block that announced a long successor but is followed by a short one
                // overruns the output in the original.
                let tail = self
                    .output
                    .get_mut(..self.prev_quarter)
                    .ok_or(OUT_OF_RANGE)?;
                let half = self.prev_n >> 1;
                for (value, previous) in tail.iter_mut().zip(&self.prev_vector[half..]) {
                    *value += *previous;
                }
            }
            if !no_residue {
                let first = (len + left_start).checked_sub(n >> 1).ok_or(OUT_OF_RANGE)?;
                let head = &self.vector[left_start..n >> 1];
                for (value, current) in self.output[first..].iter_mut().zip(head) {
                    *value += *current;
                }
            }
        }
        std::mem::swap(&mut self.prev_vector, &mut self.vector);
        self.prev_n = n;
        self.prev_quarter = right_end - (n >> 1);
        self.prev_no_residue = no_residue;
        Ok(has_output)
    }
}

/// The sample conversion of `toPcmSound`: scale to unsigned 8 bits, clamp, re-centre.
fn to_sample(value: f32) -> i8 {
    let mut sample = (value * 128.0 + 128.0) as i32;
    if sample & !0xFF != 0 {
        sample = !sample >> 31;
    }
    (sample - 128) as i8
}

/// One sound file of archive 14 (the `VorbisSound` instance fields).
#[derive(Clone, Debug)]
pub struct VorbisSound {
    /// `rate`: sample rate in hertz.
    pub rate: i32,
    /// `samplesLen`: length of the decoded sound in samples.
    pub samples_len: i32,
    /// `start`: loop start in samples.
    pub start: i32,
    /// `end`: loop end in samples; the file stores it complemented for a ping-pong loop.
    pub end: i32,
    /// `pingPongLoop`.
    pub ping_pong_loop: bool,
    /// `packets`: the raw audio packets.
    pub packets: Vec<Vec<u8>>,
    /// A decode that a budget interrupted; `None` when no decode is under way (the original's
    /// `samples == null`).
    progress: Option<Box<Progress>>,
}

impl VorbisSound {
    /// The constructor's `decode(byte[])`: the sound file's header and packet list.
    pub fn decode(data: &[u8]) -> Result<Self, String> {
        Self::decode_file(data).map_err(|fault| format!("vorbis sound: {fault}"))
    }

    fn decode_file(data: &[u8]) -> Result<Self, Fault> {
        let mut offset = 0usize;
        let mut g4 = |data: &[u8]| -> Result<i32, Fault> {
            let bytes = data.get(offset..offset + 4).ok_or(OVERRUN)?;
            offset += 4;
            Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        };
        let rate = g4(data)?;
        let samples_len = g4(data)?;
        let start = g4(data)?;
        let mut end = g4(data)?;
        let mut ping_pong_loop = false;
        if end < 0 {
            end = !end;
            ping_pong_loop = true;
        }
        let packet_count = g4(data)?;
        if packet_count < 0 || samples_len < 0 {
            return Err(NEGATIVE_SIZE);
        }
        let mut packets = Vec::new();
        for _ in 0..packet_count {
            let mut len = 0usize;
            loop {
                let n = usize::from(*data.get(offset).ok_or(OVERRUN)?);
                offset += 1;
                len += n;
                if n < 255 {
                    break;
                }
            }
            let packet = data.get(offset..offset + len).ok_or(OVERRUN)?;
            offset += len;
            packets.push(packet.to_vec());
        }
        Ok(Self {
            rate,
            samples_len,
            start,
            end,
            ping_pong_loop,
            packets,
            progress: None,
        })
    }

    /// True while a budgeted decode is part-way through (the original's `samples != null`).
    pub fn is_decoding(&self) -> bool {
        self.progress.is_some()
    }

    /// `toPcmSound(int[])`. `budget` is the original's one-element array (`SoundBank`'s
    /// `samplingRates`), `None` its `null`: the number of samples the caller still allows to
    /// be decoded.
    ///
    /// Returns `None`, as the original returns `null`, when the budget is zero or negative on
    /// entry or before any packet; the samples each packet yields are subtracted from the
    /// budget, which may therefore end below zero. The partial decode stays in `self` and the
    /// next call continues with the next packet. Once every packet is decoded the result is
    /// returned and the state is dropped, so a further call decodes from the start again.
    /// The result does not depend on how the calls were budgeted.
    ///
    /// To reproduce `SoundBank.getVorbisSound`: it addresses a sound by one id (file `id` of
    /// group 0 when the archive's capacity is 1, otherwise group `id` file 0, which must have
    /// capacity 1); looks the finished `PcmSound` up in its cache first; returns `null`
    /// without touching anything when a budget is present and not positive; otherwise fetches
    /// or reuses the cached `VorbisSound` (`VorbisSound.create` needs the setup, group 0 file
    /// 0, and returns `null` until both files are available), calls this, and on a result
    /// drops the `VorbisSound` and caches the `PcmSound`. `SoundBank` does not subtract
    /// anything from the budget itself for Vorbis sounds.
    ///
    /// A malformed packet makes the original throw. This returns the sound with everything
    /// decoded before that packet and silence after it; [`Self::try_to_pcm`] reports the
    /// error instead.
    pub fn to_pcm(&mut self, setup: &Setup, budget: Option<&mut i32>) -> Option<Pcm> {
        match self.try_to_pcm(setup, budget) {
            Ok(pcm) => pcm,
            Err(_) => {
                let samples = self
                    .progress
                    .take()
                    .map_or_else(Vec::new, |progress| progress.samples);
                Some(self.pcm(samples))
            }
        }
    }

    /// [`Self::to_pcm`], except that input the original would throw on is an error. The
    /// interrupted state is kept, as the original's is, and a retry fails the same way.
    pub fn try_to_pcm(
        &mut self,
        setup: &Setup,
        budget: Option<&mut i32>,
    ) -> Result<Option<Pcm>, String> {
        if budget.as_deref().is_some_and(|budget| *budget <= 0) {
            return Ok(None);
        }
        let mut progress = match self.progress.take() {
            Some(progress) => progress,
            None => Box::new(
                Progress::new(setup, self.samples_len)
                    .map_err(|fault| format!("vorbis sound: {fault}"))?,
            ),
        };
        match progress.run(setup, &self.packets, budget) {
            Ok(true) => Ok(Some(self.pcm(progress.samples))),
            Ok(false) => {
                self.progress = Some(progress);
                Ok(None)
            }
            Err(fault) => {
                let packet = progress.packet_index;
                self.progress = Some(progress);
                Err(format!("vorbis packet {packet}: {fault}"))
            }
        }
    }

    fn pcm(&self, samples: Vec<i8>) -> Pcm {
        Pcm {
            rate: self.rate,
            samples,
            start: self.start,
            end: self.end,
            ping_pong: self.ping_pong_loop,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes bits in the order `BitReader` reads them.
    #[derive(Default)]
    struct BitWriter {
        bytes: Vec<u8>,
        bit: u32,
    }

    impl BitWriter {
        fn put(&mut self, value: u32, len: u32) -> &mut Self {
            for i in 0..len {
                if self.bit == 0 {
                    self.bytes.push(0);
                }
                *self.bytes.last_mut().unwrap() |= ((value >> i & 1) as u8) << self.bit;
                self.bit = (self.bit + 1) & 7;
            }
            self
        }
    }

    fn fnv(hash: &mut u64, word: u32) {
        for byte in word.to_le_bytes() {
            *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

    #[test]
    fn bit_reader_matches_the_original_cursor() {
        let data = [0b1010_1101, 0xFF, 0x40, 0x80, 0x12, 0x34, 0x56, 0x78, 0x9A];
        let mut reader = BitReader::new(&data);
        assert_eq!(reader.read_bit(), Ok(1));
        assert_eq!(reader.read_bit(), Ok(0));
        assert_eq!(reader.read_bits(3), Ok(0b011));
        assert_eq!(reader.read_bits(0), Ok(0));
        // Three bits left in the first byte, then the next eight, then one more.
        assert_eq!(reader.read_bits(12), Ok(0b0111_1111_1101));
        assert_eq!(reader.read_bits(7), Ok(0x20));
        assert_eq!(reader.read_bits(16), Ok(0x1280));
        // A read that ends on the last bit of the data succeeds.
        assert_eq!(reader.read_bits(32), Ok(0x9A78_5634u32 as i32));
        assert_eq!(reader.read_bits(0), Ok(0));
        assert_eq!(reader.read_bit(), Err(OVERRUN));
        assert_eq!(reader.read_bits(1), Err(OVERRUN));

        // A value whose top bit is set comes back as a negative int.
        let mut reader = BitReader::new(&[0xFE, 0xFF, 0xFF, 0xFF, 0x01]);
        assert_eq!(reader.read_bit(), Ok(0));
        assert_eq!(reader.read_bits(32), Ok(-1));
        assert_eq!(reader.read_bits(8), Err(OVERRUN));
    }

    #[test]
    fn integer_helpers() {
        let counts = [0, 1, 2, 2, 3, 3, 3, 3, 4];
        for (value, count) in counts.into_iter().enumerate() {
            assert_eq!(bit_count(value as i32), count);
        }
        assert_eq!(bit_count(65535), 16);
        assert_eq!(bit_count(65536), 17);
        assert_eq!(bit_count(-1), 32);
        assert_eq!(bit_reverse(3, 0b001), 0b100);
        assert_eq!(bit_reverse(5, 0b10110), 0b01101);
        assert_eq!(int_pow(4, 3), 81);
        assert_eq!(int_pow(0, 7), 1);
        assert_eq!(int_pow(1, 7), 7);
        assert_eq!(int_pow(32, 2), 0);
    }

    /// Values printed by the JVM for `VorbisSound.float32Unpack`.
    #[test]
    fn float32_unpack_matches_the_jvm() {
        let cases: [(u32, u32); 12] = [
            (0x6010_0000, 0x3F80_0000),
            (0xE010_0000, 0xBF80_0000),
            (0x6030_0000, 0x4000_0000),
            (0x5E90_0000, 0x3980_0000),
            (0x0000_0001, 0x0000_0000),
            (0x7FFF_FFFF, 0x7F80_0000),
            (0xFFFF_FFFF, 0xFF80_0000),
            (0x62A1_2345, 0x4811_A280),
            (0x5D0F_FFFF, 0x337F_FFF0),
            (0xE395_5555, 0xCDAA_AAA8),
            (0x4AB0_0001, 0x0000_0000),
            (0x0000_0000, 0x0000_0000),
        ];
        for (input, bits) in cases {
            assert_eq!(
                float32_unpack(input as i32).to_bits(),
                bits,
                "{input:#010X}"
            );
        }
    }

    /// Values printed by the JVM for `VorbisCodebook.lookup1Values`.
    #[test]
    fn lookup1_values_matches_the_jvm() {
        let cases = [
            (81, 4, 3),
            (625, 4, 5),
            (8, 3, 2),
            (10, 2, 3),
            (1, 1, 1),
            (0, 3, 0),
            (16_777_215, 1, 16_777_215),
            (4096, 12, 2),
            (6561, 8, 3),
            (100, 2, 10),
        ];
        for (entries, dimensions, expected) in cases {
            assert_eq!(lookup1_values(entries, dimensions), expected);
        }
    }

    /// FNV-1a of the JVM's `VorbisFloor.INVERSE_DB_TABLE` bits.
    #[test]
    fn inverse_db_table_matches_the_jvm() {
        let mut hash = FNV_OFFSET;
        for value in INVERSE_DB_TABLE {
            fnv(&mut hash, value.to_bits());
        }
        assert_eq!(hash, 0x60B2_AA15_3494_C6F4);
    }

    /// FNV-1a of the bits of `trigA`, `trigB`, `trigC`, `bitReverse` and both window slopes as
    /// the JVM's `Math.sin` and `Math.cos` produce them, for every supported block size.
    #[test]
    fn transform_tables_match_the_jvm() {
        let expected: [u64; 13] = [
            0x7625_1712_F69A_80B2,
            0xC655_1052_F2B3_4D7F,
            0x0124_027B_B5C5_9B87,
            0x820C_DBB9_9BC9_ADD5,
            0x164F_8AFA_259D_0B0B,
            0x68AA_701E_5935_C6F6,
            0xBCB7_4787_07C7_9452,
            0x09A8_9EE5_381C_E269,
            0x3507_2DBC_DA35_7351,
            0x093E_40DA_301F_BBE2,
            0x4EDC_7966_4CC8_4CB8,
            0x0175_9BC2_9498_FC2A,
            0x85ED_176C_0752_A919,
        ];
        for (index, expected) in expected.into_iter().enumerate() {
            let size = 8 << index;
            let tables = Tables::new(size);
            let mut hash = FNV_OFFSET;
            for table in [&tables.trig_a, &tables.trig_b, &tables.trig_c] {
                for value in table {
                    fnv(&mut hash, value.to_bits());
                }
            }
            for &value in &tables.bit_reverse {
                fnv(&mut hash, value as u32);
            }
            for table in [&tables.window_left, &tables.window_right] {
                for value in table {
                    fnv(&mut hash, value.to_bits());
                }
            }
            assert_eq!(hash, expected, "block size {size}");
        }
    }

    #[test]
    fn sample_conversion_clamps() {
        assert_eq!(to_sample(0.0), 0);
        assert_eq!(to_sample(-0.0), 0);
        assert_eq!(to_sample(0.5), 64);
        assert_eq!(to_sample(-0.5), -64);
        assert_eq!(to_sample(-1.0), -128);
        assert_eq!(to_sample(0.999), 127);
        assert_eq!(to_sample(1.0), 127);
        assert_eq!(to_sample(-1.01), -128);
        assert_eq!(to_sample(1e30), 127);
        assert_eq!(to_sample(-1e30), -128);
        assert_eq!(to_sample(f32::INFINITY), 127);
        // `(int) NaN` is 0 in Java, the most negative sample.
        assert_eq!(to_sample(f32::NAN), -128);
    }

    /// Four entries with lengths 1, 2, 3, 3 (codewords 0, 10, 110, 111) and a lookup-1 value
    /// table over two dimensions.
    fn write_codebook(writer: &mut BitWriter) {
        writer.put(0x564342, 24).put(2, 16).put(4, 24);
        writer.put(0, 1).put(0, 1);
        for length in [1, 2, 3, 3] {
            writer.put(length - 1, 5);
        }
        // Lookup 1, minimum -1.0, delta 0.5, two-bit multiplicands (stored as 1), no sequence
        // flag, then the multiplicands 1 and 3.
        writer.put(1, 4).put(0xE010_0000, 32).put(0x5FF0_0000, 32);
        writer.put(1, 4).put(0, 1);
        writer.put(1, 2).put(3, 2);
    }

    #[test]
    fn synthetic_codebook() {
        let mut writer = BitWriter::default();
        write_codebook(&mut writer);
        // Codewords for entries 2, 0, 3, 1, most significant bit first.
        for bit in [1, 1, 0, 0, 1, 1, 1, 1, 0] {
            writer.put(bit, 1);
        }
        let mut reader = BitReader::new(&writer.bytes);
        let codebook = Codebook::decode(&mut reader).unwrap();
        assert_eq!(codebook.dimensions, 2);
        assert_eq!(float32_unpack(0x5FF0_0000u32 as i32), 0.5);
        // lookup1Values(4, 2) is 2; entry e uses multiplicands e % 2 and e / 2 % 2.
        assert_eq!(
            codebook.value_vector.as_deref().unwrap(),
            [-0.5, -0.5, 0.5, -0.5, -0.5, 0.5, 0.5, 0.5]
        );
        assert_eq!(codebook.decode_scalar(&mut reader), Ok(2));
        assert_eq!(codebook.decode_scalar(&mut reader), Ok(0));
        assert_eq!(codebook.decode_vq(&mut reader).unwrap(), [0.5, 0.5]);
        assert_eq!(codebook.decode_scalar(&mut reader), Ok(1));
    }

    #[test]
    fn ordered_codebook_lengths() {
        // Five entries: one of length 1, none of length 2, four of length 3.
        let mut writer = BitWriter::default();
        writer.put(0x564342, 24).put(1, 16).put(5, 24).put(1, 1);
        writer.put(0, 5).put(1, 3).put(0, 3).put(4, 3).put(0, 4);
        for bit in [1, 0, 0, 0, 1, 1, 1] {
            writer.put(bit, 1);
        }
        let mut reader = BitReader::new(&writer.bytes);
        let codebook = Codebook::decode(&mut reader).unwrap();
        assert!(codebook.value_vector.is_none());
        assert_eq!(codebook.decode_scalar(&mut reader), Ok(1));
        assert_eq!(codebook.decode_scalar(&mut reader), Ok(0));
        assert_eq!(codebook.decode_scalar(&mut reader), Ok(4));
        assert!(codebook.decode_vq(&mut reader).is_err());
    }

    /// A setup with blocks of `1 << short` and `1 << long` samples, the codebook above, a floor
    /// of two end points, a residue of four-value partitions over a short block's spectrum,
    /// and a short and a long mode.
    fn synthetic_setup(short: u32, long: u32, residue_kind: u32) -> Vec<u8> {
        let mut writer = BitWriter::default();
        writer.put(short, 4).put(long, 4);
        // Every count is stored less one: one codebook, one ignored time-domain entry.
        writer.put(0, 8);
        write_codebook(&mut writer);
        writer.put(0, 6).put(0, 16);
        // Floor: type 1, no partitions, multiplier 1, x range up to half a long block.
        writer
            .put(0, 6)
            .put(1, 16)
            .put(0, 5)
            .put(0, 2)
            .put(long - 1, 4);
        // Residue: begin 0, end at half a short block, partition size 4, one class, class
        // book 0, and a cascade with only the first pass, using book 0.
        writer.put(0, 6).put(residue_kind, 16).put(0, 24);
        writer.put(1 << (short - 1), 24);
        writer.put(3, 24).put(0, 6).put(0, 8);
        writer.put(1, 3).put(0, 1).put(0, 8);
        // Mapping: one submap, no coupling, floor 0, residue 0.
        writer.put(0, 6).put(0, 16).put(0, 1).put(0, 1).put(0, 2);
        writer.put(0, 8).put(0, 8).put(0, 8);
        // Modes: short, long.
        writer.put(1, 6);
        writer.put(0, 1).put(0, 16).put(0, 16).put(0, 8);
        writer.put(1, 1).put(0, 16).put(0, 16).put(0, 8);
        writer.bytes
    }

    fn sound_file(samples_len: i32, end: i32, packets: &[Vec<u8>]) -> Vec<u8> {
        let mut data = Vec::new();
        for word in [11025, samples_len, 3, end, packets.len() as i32] {
            data.extend_from_slice(&word.to_be_bytes());
        }
        for packet in packets {
            let mut len = packet.len();
            while len >= 255 {
                data.push(255);
                len -= 255;
            }
            data.push(len as u8);
            data.extend_from_slice(packet);
        }
        data
    }

    /// A deterministic sound for `synthetic_setup`: 48 packets of random block size, window
    /// flags, floor end points and residue vectors, one in eight with the floor unused.
    fn synthetic_sound(short: u32, long: u32, seed: u64) -> Vec<u8> {
        let mut state = seed;
        let mut random = |bound: u32| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 33) as u32 % bound
        };
        let longs: Vec<bool> = (0..48).map(|_| random(2) == 1).collect();
        let partitions = (1 << short) / 2 / 4;
        let mut packets = Vec::new();
        let mut samples = 0;
        for (index, &is_long) in longs.iter().enumerate() {
            let mut writer = BitWriter::default();
            writer.put(0, 1).put(u32::from(is_long), 1);
            if is_long {
                // The original overruns its output when a long window is announced towards a
                // short neighbour; the flags are free otherwise.
                let previous = index > 0 && longs[index - 1] && random(4) != 0;
                let next = index + 1 < longs.len() && longs[index + 1] && random(4) != 0;
                writer.put(u32::from(previous), 1).put(u32::from(next), 1);
            }
            if random(8) == 0 {
                writer.put(0, 1);
            } else {
                writer.put(1, 1);
                writer.put(192 + random(48), 8).put(192 + random(48), 8);
                for partition in 0..partitions {
                    // One classification codeword per two partitions, two vectors for each.
                    for _ in 0..if partition % 2 == 0 { 3 } else { 2 } {
                        let entry = random(4);
                        writer.put(u32::MAX, entry.min(3));
                        if entry < 3 {
                            writer.put(0, 1);
                        }
                    }
                }
            }
            if index > 0 {
                let size = |long_block: bool| 1 << if long_block { long } else { short };
                samples += (size(longs[index - 1]) + size(is_long)) / 4;
            }
            packets.push(writer.bytes);
        }
        sound_file(samples, 0, &packets)
    }

    /// Streams that switch between short and long blocks, which no sound in the 530 cache
    /// does (its setup makes both block sizes 1024). The expected values are FNV-1a hashes of
    /// the samples the original classes decode from the same bytes; set
    /// `VORBIS_SYNTHETIC_DUMP` to a directory to write those bytes out for the JVM.
    #[test]
    fn synthetic_streams_match_the_jvm() {
        let cases: [(u32, u32, u32, u64); 5] = [
            (3, 4, 2, 0xCADC_5460_3F41_1CCF),
            (5, 8, 0, 0xE940_B8F6_0413_49EC),
            (7, 11, 1, 0xB0ED_67AD_ACA6_449F),
            (9, 13, 2, 0xA5CC_CCFC_480D_B9DB),
            (6, 6, 0, 0x5562_431F_6F05_DC34),
        ];
        for (index, (short, long, residue_kind, expected)) in cases.into_iter().enumerate() {
            let setup_bytes = synthetic_setup(short, long, residue_kind);
            let sound_bytes = synthetic_sound(short, long, index as u64 + 1);
            if let Some(directory) = std::env::var_os("VORBIS_SYNTHETIC_DUMP") {
                let directory = std::path::PathBuf::from(directory);
                let write = |name: &str, bytes: &[u8]| {
                    std::fs::write(
                        directory.join(format!("synthetic_{index}_{name}.bin")),
                        bytes,
                    )
                    .unwrap();
                };
                write("setup", &setup_bytes);
                write("sound", &sound_bytes);
            }
            let setup = Setup::decode(&setup_bytes).unwrap();
            let mut sound = VorbisSound::decode(&sound_bytes).unwrap();
            let pcm = sound.try_to_pcm(&setup, None).unwrap().unwrap();
            let mut hash = FNV_OFFSET;
            for &sample in &pcm.samples {
                hash = (hash ^ u64::from(sample as u8)).wrapping_mul(0x0000_0100_0000_01b3);
            }
            assert_eq!(hash, expected, "case {index}: {hash:#018X}");
        }
    }

    /// A packet whose floor is flagged unused: a silent block.
    fn silent_packet(long: bool) -> Vec<u8> {
        let mut writer = BitWriter::default();
        writer.put(0, 1).put(u32::from(long), 1);
        if long {
            writer.put(1, 2);
        }
        writer.put(0, 1);
        writer.bytes
    }

    #[test]
    fn sound_file_header_and_packets() {
        let packets = [vec![1u8; 3], vec![2u8; 255], vec![3u8; 600], Vec::new()];
        let sound = VorbisSound::decode(&sound_file(1000, !40, &packets)).unwrap();
        assert_eq!((sound.rate, sound.samples_len), (11025, 1000));
        assert_eq!(
            (sound.start, sound.end, sound.ping_pong_loop),
            (3, 40, true)
        );
        assert_eq!(sound.packets, packets);
        let sound = VorbisSound::decode(&sound_file(1000, 40, &packets)).unwrap();
        assert_eq!((sound.end, sound.ping_pong_loop), (40, false));

        let data = sound_file(1000, 40, &packets);
        assert!(VorbisSound::decode(&data[..data.len() - 1]).is_err());
        assert!(VorbisSound::decode(&data[..19]).is_err());
        assert!(VorbisSound::decode(&sound_file(-1, 40, &packets)).is_err());
    }

    #[test]
    fn budget_interrupts_and_resumes() {
        let setup = Setup::decode(&synthetic_setup(3, 4, 2)).unwrap();
        assert_eq!((setup.block_size_0(), setup.block_size_1()), (8, 16));
        // Short, short, long, long, short: the blocks after the first yield 4, 6, 8 and 6.
        let packets: Vec<Vec<u8>> = [false, false, true, true, false]
            .into_iter()
            .map(silent_packet)
            .collect();
        let mut sound = VorbisSound::decode(&sound_file(30, 20, &packets)).unwrap();
        let whole = sound.to_pcm(&setup, None).unwrap();
        assert_eq!(whole.samples, vec![0i8; 30]);
        assert_eq!((whole.rate, whole.start, whole.end), (11025, 3, 20));
        assert!(!whole.ping_pong && !sound.is_decoding());

        let mut budget = 0;
        assert_eq!(sound.to_pcm(&setup, Some(&mut budget)), None);
        assert!(!sound.is_decoding());
        // The first packet yields nothing, so a budget of 5 lasts through the third.
        budget = 5;
        assert_eq!(sound.to_pcm(&setup, Some(&mut budget)), None);
        assert_eq!(budget, -5);
        assert!(sound.is_decoding());
        assert_eq!(sound.to_pcm(&setup, Some(&mut budget)), None);
        budget = 8;
        assert_eq!(sound.to_pcm(&setup, Some(&mut budget)), None);
        assert_eq!(budget, 0);
        budget = 1;
        assert_eq!(sound.to_pcm(&setup, Some(&mut budget)), Some(whole.clone()));
        assert_eq!(budget, -5);
        assert!(!sound.is_decoding());

        // Only as many samples as the file declares are kept.
        sound.samples_len = 7;
        assert_eq!(sound.to_pcm(&setup, None).unwrap().samples.len(), 7);
    }

    #[test]
    fn residue_packet_decodes_and_malformed_packets_fail() {
        let setup = Setup::decode(&synthetic_setup(3, 4, 2)).unwrap();
        // A short block: used floor with both end points at 255 (gain 1.0), classification
        // codeword for class 0, then two vectors: entries 3 and 0.
        let mut writer = BitWriter::default();
        writer.put(0, 1).put(0, 1).put(1, 1).put(255, 8).put(255, 8);
        for bit in [0, 1, 1, 1, 0] {
            writer.put(bit, 1);
        }
        let packet = writer.bytes;
        let packets = vec![packet.clone(), packet.clone(), packet.clone()];
        let mut sound = VorbisSound::decode(&sound_file(8, 0, &packets)).unwrap();
        let pcm = sound.try_to_pcm(&setup, None).unwrap().unwrap();
        assert_eq!(pcm.samples.len(), 8);
        assert_eq!(pcm.samples[..4], pcm.samples[4..]);
        assert!(pcm.samples.iter().any(|&sample| sample != 0));

        // The same sound with its last packet cut short.
        let mut cut = packets.clone();
        cut[2].truncate(2);
        let mut sound = VorbisSound::decode(&sound_file(8, 0, &cut)).unwrap();
        let error = sound.try_to_pcm(&setup, None).unwrap_err();
        assert!(error.contains("packet 2"), "{error}");
        assert!(sound.is_decoding());
        assert!(sound.try_to_pcm(&setup, None).is_err());
        let partial = sound.to_pcm(&setup, None).unwrap();
        assert_eq!(partial.samples[..4], pcm.samples[..4]);
        assert_eq!(partial.samples[4..], [0; 4]);
        assert!(!sound.is_decoding());

        // A long block that promises a long successor, followed by a short block.
        let mut long = BitWriter::default();
        long.put(0, 1)
            .put(1, 1)
            .put(3, 2)
            .put(1, 1)
            .put(255, 8)
            .put(255, 8);
        for bit in [0, 1, 1, 1, 0] {
            long.put(bit, 1);
        }
        let mut sound = VorbisSound::decode(&sound_file(64, 0, &[long.bytes, packet])).unwrap();
        assert!(sound.try_to_pcm(&setup, None).is_err());

        assert!(Setup::decode(&synthetic_setup(3, 4, 2)[..40]).is_err());
        assert!(Setup::decode(&[0x21]).is_err());
        assert!(Setup::decode(&[0x34]).is_err());
    }
}
