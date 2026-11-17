//! RT4 synthesized sound effects. Music/MIDI/Vorbis and spatial mixing remain separate work.
use rs09_protocol::{Error, Result, buffer::Reader};

const RATE: usize = 22050;
#[derive(Clone)]
struct Envelope {
    wave: i32,
    min: i32,
    max: i32,
    times: Vec<i32>,
    levels: Vec<i32>,
    phase: usize,
    level: i32,
    slope: i32,
    next: i32,
    time: i32,
}
impl Default for Envelope {
    fn default() -> Self {
        Self {
            wave: 0,
            min: 0,
            max: 0,
            times: vec![0, 65535],
            levels: vec![0, 65535],
            phase: 0,
            level: 0,
            slope: 0,
            next: 0,
            time: 0,
        }
    }
}
impl Envelope {
    fn decode(r: &mut Reader<'_>) -> Result<Self> {
        let mut e = Self {
            wave: r.g1()?,
            min: r.g4()?,
            max: r.g4()?,
            ..Self::default()
        };
        e.stages(r)?;
        Ok(e)
    }
    fn stages(&mut self, r: &mut Reader<'_>) -> Result<()> {
        let n = r.g1()?;
        if n == 0 {
            return Err(Error::new(r.position(), "empty audio envelope"));
        }
        self.times.clear();
        self.levels.clear();
        for _ in 0..n {
            self.times.push(r.g2()?);
            self.levels.push(r.g2()?);
        }
        Ok(())
    }
    fn next(&mut self, duration: usize) -> i32 {
        if self.time >= self.next {
            self.level = self.levels[self.phase] << 15;
            self.phase = (self.phase + 1).min(self.times.len() - 1);
            self.next = (f64::from(self.times[self.phase]) / 65536.0 * duration as f64) as i32;
            if self.next > self.time {
                self.slope = ((self.levels[self.phase] << 15).wrapping_sub(self.level))
                    / (self.next - self.time);
            }
        }
        self.level = self.level.wrapping_add(self.slope);
        self.time += 1;
        self.level.wrapping_sub(self.slope) >> 15
    }
}
#[derive(Default)]
struct Filter {
    pairs: [usize; 2],
    gain: [[[i32; 4]; 2]; 2],
    octaves: [[[i32; 4]; 2]; 2],
    inverse: [i32; 2],
    coefficients: [[i32; 8]; 2],
    inverse_a0: i32,
    float_inverse: f32,
}
impl Filter {
    fn decode(r: &mut Reader<'_>, env: &mut Envelope) -> Result<Self> {
        let b = r.g1()? as usize;
        let mut f = Self {
            pairs: [b >> 4, b & 15],
            ..Self::default()
        };
        if f.pairs.iter().any(|&n| n > 4) {
            return Err(Error::new(r.position(), "too many filter pairs"));
        }
        if b == 0 {
            return Ok(f);
        }
        f.inverse = [r.g2()?, r.g2()?];
        let flags = r.g1()?;
        for c in 0..2 {
            for p in 0..f.pairs[c] {
                f.octaves[c][0][p] = r.g2()?;
                f.gain[c][0][p] = r.g2()?;
            }
        }
        for c in 0..2 {
            for p in 0..f.pairs[c] {
                if flags & (1 << (c * 4 + p)) != 0 {
                    f.octaves[c][1][p] = r.g2()?;
                    f.gain[c][1][p] = r.g2()?;
                } else {
                    f.octaves[c][1][p] = f.octaves[c][0][p];
                    f.gain[c][1][p] = f.gain[c][0][p];
                }
            }
        }
        if flags != 0 || f.inverse[0] != f.inverse[1] {
            env.stages(r)?;
        }
        Ok(f)
    }
    fn compute(&mut self, c: usize, t: f32) -> usize {
        if c == 0 {
            let amplitude = self.inverse[0] as f32 + (self.inverse[1] - self.inverse[0]) as f32 * t;
            let db = amplitude * 0.0030517578;
            self.float_inverse = 0.1f64.powf(f64::from(db / 20.0)) as f32;
            self.inverse_a0 = (self.float_inverse * 65536.0) as i32;
        }
        let n = self.pairs[c];
        if n == 0 {
            return 0;
        }
        let amp = |p: usize| {
            let v =
                self.gain[c][0][p] as f32 + t * (self.gain[c][1][p] - self.gain[c][0][p]) as f32;
            let db = v * 0.0015258789;
            1.0 - 10.0f64.powf(f64::from(-db / 20.0)) as f32
        };
        let phase = |p: usize| {
            let v = self.octaves[c][0][p] as f32
                + t * (self.octaves[c][1][p] - self.octaves[c][0][p]) as f32;
            let octave = v * 1.2207031e-4;
            let frequency = 2.0f64.powf(f64::from(octave)) as f32 * 32.703197;
            frequency * std::f32::consts::PI / 11025.0
        };
        let mut out = [0f32; 8];
        let a = amp(0);
        out[0] = -2.0 * a * f64::from(phase(0)).cos() as f32;
        out[1] = a * a;
        for i in 1..n {
            let a = amp(i);
            let coeff = -2.0 * a * f64::from(phase(i)).cos() as f32;
            let square = a * a;
            out[i * 2 + 1] = out[i * 2 - 1] * square;
            out[i * 2] = out[i * 2 - 1] * coeff + out[i * 2 - 2] * square;
            for j in (2..=i * 2 - 1).rev() {
                out[j] += out[j - 1] * coeff + out[j - 2] * square;
            }
            out[1] += out[0] * coeff + square;
            out[0] += coeff;
        }
        for (i, v) in out.iter_mut().enumerate().take(n * 2) {
            if c == 0 {
                *v *= self.float_inverse;
            }
            self.coefficients[c][i] = (*v * 65536.0) as i32;
        }
        n * 2
    }
}
struct Instrument {
    phase: Envelope,
    amp: Envelope,
    phase_mod: Option<(Envelope, Envelope)>,
    amp_mod: Option<(Envelope, Envelope)>,
    gate: Option<(Envelope, Envelope)>,
    harmonics: Vec<[i32; 3]>,
    reverb_delay: i32,
    reverb_volume: i32,
    length: usize,
    start: usize,
    filter: Filter,
    filter_env: Envelope,
}
fn optional_envelopes(r: &mut Reader<'_>) -> Result<Option<(Envelope, Envelope)>> {
    if r.g1()? == 0 {
        return Ok(None);
    }
    r.seek(r.position() - 1)?;
    Ok(Some((Envelope::decode(r)?, Envelope::decode(r)?)))
}
impl Instrument {
    fn decode(r: &mut Reader<'_>) -> Result<Self> {
        let phase = Envelope::decode(r)?;
        let amp = Envelope::decode(r)?;
        let phase_mod = optional_envelopes(r)?;
        let amp_mod = optional_envelopes(r)?;
        let gate = optional_envelopes(r)?;
        let mut harmonics = Vec::new();
        for i in 0..10 {
            let volume = r.gsmarts()?;
            if volume == 0 {
                break;
            }
            if i >= 5 {
                return Err(Error::new(r.position(), "more than five sound oscillators"));
            }
            harmonics.push([volume, r.gsmart()?, r.gsmarts()?]);
        }
        let reverb_delay = r.gsmarts()?;
        let reverb_volume = r.gsmarts()?;
        let length = r.g2()? as usize;
        let start = r.g2()? as usize;
        if length > 10000 {
            return Err(Error::new(
                r.position(),
                "sound instrument exceeds RT4 sample buffer",
            ));
        }
        let mut filter_env = Envelope::default();
        let filter = Filter::decode(r, &mut filter_env)?;
        Ok(Self {
            phase,
            amp,
            phase_mod,
            amp_mod,
            gate,
            harmonics,
            reverb_delay,
            reverb_volume,
            length,
            start,
            filter,
            filter_env,
        })
    }
    fn samples(&mut self, tables: &WaveTables) -> Vec<i32> {
        let n = self.length * RATE / 1000;
        let mut samples = vec![0i32; n];
        if self.length < 10 {
            return samples;
        }
        let duration = n as f64 / self.length as f64;
        let (mut phase_time, mut amp_time) = (0i32, 0i32);
        let params = |env: &Option<(Envelope, Envelope)>| {
            env.as_ref().map_or((0, 0), |(e, _)| {
                (
                    (f64::from(e.max - e.min) * 32.768 / duration) as i32,
                    (f64::from(e.min) * 32.768 / duration) as i32,
                )
            })
        };
        let (phase_range, phase_min) = params(&self.phase_mod);
        let (amp_range, amp_min) = params(&self.amp_mod);
        let mut oscillators = Vec::new();
        for &[volume, semitone, delay] in &self.harmonics {
            oscillators.push([
                0,
                (f64::from(delay) * duration) as i32,
                (volume << 14) / 100,
                (f64::from(self.phase.max - self.phase.min)
                    * 32.768
                    * 1.0057929410678534f64.powi(semitone)
                    / duration) as i32,
                (f64::from(self.phase.min) * 32.768 / duration) as i32,
            ]);
        }
        for i in 0..n {
            let mut phase = self.phase.next(n);
            let mut amplitude = self.amp.next(n);
            if let Some((e, a)) = &mut self.phase_mod {
                let level = e.next(n);
                let amp = a.next(n);
                phase = phase.wrapping_add(tables.wave(phase_time, amp, e.wave) >> 1);
                phase_time = phase_time
                    .wrapping_add((level.wrapping_mul(phase_range) >> 16).wrapping_add(phase_min));
            }
            if let Some((e, a)) = &mut self.amp_mod {
                let level = e.next(n);
                let amp = a.next(n);
                amplitude =
                    amplitude.wrapping_mul((tables.wave(amp_time, amp, e.wave) >> 1) + 32768) >> 15;
                amp_time = amp_time
                    .wrapping_add((level.wrapping_mul(amp_range) >> 16).wrapping_add(amp_min));
            }
            for osc in &mut oscillators {
                let dest = i + osc[1] as usize;
                if dest < n {
                    samples[dest] = samples[dest].wrapping_add(tables.wave(
                        osc[0],
                        amplitude.wrapping_mul(osc[2]) >> 15,
                        self.phase.wave,
                    ));
                    osc[0] = osc[0]
                        .wrapping_add((phase.wrapping_mul(osc[3]) >> 16).wrapping_add(osc[4]));
                }
            }
        }
        if let Some((closed, open)) = &mut self.gate {
            let (mut time, mut is_closed) = (0, true);
            for sample in &mut samples {
                let c = closed.next(n);
                let o = open.next(n);
                let threshold = closed.min.wrapping_add(
                    (closed.max - closed.min).wrapping_mul(if is_closed { c } else { o }) >> 8,
                );
                time += 256;
                if time >= threshold {
                    time = 0;
                    is_closed = !is_closed;
                }
                if is_closed {
                    *sample = 0;
                }
            }
        }
        if self.reverb_delay > 0 && self.reverb_volume > 0 {
            let delay = (f64::from(self.reverb_delay) * duration) as usize;
            for i in delay..n {
                samples[i] = samples[i]
                    .wrapping_add(samples[i - delay].wrapping_mul(self.reverb_volume) / 100);
            }
        }
        if self.filter.pairs != [0, 0] {
            let mut level = self.filter_env.next(n + 1);
            let mut a = self.filter.compute(0, level as f32 / 65536.0);
            let mut b = self.filter.compute(1, level as f32 / 65536.0);
            if n >= a + b {
                for i in 0..n {
                    // The original recomputes coefficients at 128-sample boundaries,
                    // after its initial short feedback warmup, and before the tail.
                    if i > 0 && i % 128 == 0 && i < n - a {
                        a = self.filter.compute(0, level as f32 / 65536.0);
                        b = self.filter.compute(1, level as f32 / 65536.0);
                    }
                    let mut value = if i + a < n {
                        ((i64::from(samples[i + a]) * i64::from(self.filter.inverse_a0)) >> 16)
                            as i32
                    } else {
                        0
                    };
                    for j in 0..a {
                        if i + a > j && i + a - j - 1 < n {
                            value = value.wrapping_add(
                                ((i64::from(samples[i + a - j - 1])
                                    * i64::from(self.filter.coefficients[0][j]))
                                    >> 16) as i32,
                            );
                        }
                    }
                    for j in 0..b.min(i) {
                        value = value.wrapping_sub(
                            ((i64::from(samples[i - j - 1])
                                * i64::from(self.filter.coefficients[1][j]))
                                >> 16) as i32,
                        );
                    }
                    samples[i] = value;
                    level = self.filter_env.next(n + 1);
                }
            }
        }
        for sample in &mut samples {
            *sample = (*sample).clamp(-32768, 32767);
        }
        samples
    }
}
struct WaveTables {
    sine: Vec<i32>,
    noise: Vec<i32>,
}
impl WaveTables {
    fn new() -> Self {
        let mut seed = 0x5deece66du64;
        let noise = (0..32768)
            .map(|_| {
                seed = (seed.wrapping_mul(0x5deece66d).wrapping_add(11)) & ((1u64 << 48) - 1);
                (((seed >> 16) as i32) & 2) - 1
            })
            .collect();
        let sine = (0..32768)
            .map(|i| ((f64::from(i) / 5215.1903).sin() * 16384.0) as i32)
            .collect();
        Self { sine, noise }
    }
    fn wave(&self, phase: i32, amp: i32, wave: i32) -> i32 {
        match wave {
            1 => {
                if phase & 32767 < 16384 {
                    amp
                } else {
                    -amp
                }
            }
            2 => self.sine[(phase & 32767) as usize].wrapping_mul(amp) >> 14,
            3 => ((phase & 32767).wrapping_mul(amp) >> 14) - amp,
            4 => self.noise[((phase / 2607) & 32767) as usize] * amp,
            _ => 0,
        }
    }
}

#[derive(Clone)]
pub struct SoundEffect {
    pub samples: Vec<i8>,
    pub loop_start: usize,
    pub loop_end: usize,
}
impl SoundEffect {
    pub fn decode(data: &[u8]) -> Result<Self> {
        Self::decode_inner(data, false).map(|(effect, _)| effect)
    }
    /// SoundPlayer loads SynthSound and calls getStart before creating its PCM.
    pub fn decode_for_queue(data: &[u8]) -> Result<(Self, i32)> {
        Self::decode_inner(data, true)
    }
    fn decode_inner(data: &[u8], trim: bool) -> Result<(Self, i32)> {
        let mut r = Reader::new(data);
        let mut instruments = Vec::new();
        for _ in 0..10 {
            if r.g1()? != 0 {
                r.seek(r.position() - 1)?;
                instruments.push(Instrument::decode(&mut r)?);
            }
        }
        let mut start = r.g2()? as usize;
        let mut end = r.g2()? as usize;
        let mut ticks = 0;
        if trim {
            let mut min = instruments
                .iter()
                .map(|i| i.start / 20)
                .min()
                .unwrap_or(9999999);
            if start < end {
                min = min.min(start / 20);
            }
            if min != 9999999 && min != 0 {
                ticks = min as i32;
                for instrument in &mut instruments {
                    instrument.start -= min * 20;
                }
                if start < end {
                    start -= min * 20;
                    end -= min * 20;
                }
            }
        }
        let loop_start = start * RATE / 1000;
        let loop_end = end * RATE / 1000;
        let length = instruments
            .iter()
            .map(|i| i.length + i.start)
            .max()
            .unwrap_or(0)
            * RATE
            / 1000;
        let mut samples = vec![0i8; length];
        let tables = WaveTables::new();
        for instrument in &mut instruments {
            let offset = instrument.start * RATE / 1000;
            for (i, sample) in instrument.samples(&tables).into_iter().enumerate() {
                samples[offset + i] =
                    (i32::from(samples[offset + i]) + (sample >> 8)).clamp(-128, 127) as i8;
            }
        }
        Ok((
            Self {
                samples,
                loop_start,
                loop_end,
            },
            ticks,
        ))
    }
    /// Bake a finite SynthSound loop into PCM for the platform audio backend.
    pub fn repeated(&self, loops: i32) -> Result<Self> {
        if !(1..=256).contains(&loops) {
            return Err(Error::new(0, "unsupported preview loop count"));
        }
        if loops == 1 || self.loop_start >= self.loop_end || self.samples.is_empty() {
            return Ok(self.clone());
        }
        if self.loop_end > self.samples.len() {
            return Err(Error::new(0, "sound loop exceeds PCM bounds"));
        }
        let mut samples = self.samples[..self.loop_start].to_vec();
        for _ in 0..loops {
            samples.extend_from_slice(&self.samples[self.loop_start..self.loop_end]);
        }
        samples.extend_from_slice(&self.samples[self.loop_end..]);
        Ok(Self {
            samples,
            loop_start: self.loop_start,
            loop_end: self.loop_end,
        })
    }
    pub fn wav(&self) -> Vec<u8> {
        let n = self.samples.len() as u32;
        let mut bytes = Vec::with_capacity(44 + n as usize);
        bytes.extend(b"RIFF");
        bytes.extend((36 + n).to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16u32.to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend((RATE as u32).to_le_bytes());
        bytes.extend((RATE as u32).to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(8u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend(n.to_le_bytes());
        bytes.extend(self.samples.iter().map(|&s| (i16::from(s) + 128) as u8));
        bytes
    }
}
