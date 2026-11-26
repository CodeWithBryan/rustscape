//! The RT4 (rev 530) resampling PCM voice, transcribed from the client's
//! `SoundPcmStream` (`client!b`), `PcmSound` (`client!kj`) and `PcmResampler`
//! (`client!vj`), with the pieces of `PcmStream`, `Sound` and `Node` they use.
//!
//! Same branches, evaluation order and 32-bit wrapping arithmetic as the
//! original: positions and rates are 24.8 fixed point, volumes are fixed
//! point, and mixing adds into whatever the output buffer already holds.
//! `AudioChannel.sampleRate` / `AudioChannel.stereo` are the explicit
//! [`Output`]; the stream's membership of its owner's list (`Node.isLinked()`,
//! `Node.unlink()`) is a flag the owner sets with
//! [`SoundPcmStream::set_linked`] and polls with [`SoundPcmStream::is_linked`].
//!
//! Where the original throws (`ArrayIndexOutOfBoundsException`,
//! `ArithmeticException`, `NegativeArraySizeException`) this module panics at
//! the same point, leaving the same partially updated state.

use std::sync::Arc;

/// `GlobalConfig.AUDIO_SAMPLE_RATE`, which `PcmResampler.gcd` reads.
pub const AUDIO_SAMPLE_RATE: i32 = 22050;

/// AudioChannel.sampleRate / AudioChannel.stereo, made explicit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Output {
    pub sample_rate: i32,
    pub stereo: bool,
}

/// PcmSound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcmSound {
    pub rate: i32,
    pub samples: Vec<i8>,
    pub start: i32,
    pub end: i32,
    pub ping_pong_loop: bool,
}

impl PcmSound {
    /// `PcmSound(int, byte[], int, int)`.
    pub fn new(rate: i32, samples: Vec<i8>, start: i32, end: i32) -> Self {
        Self::with_ping_pong(rate, samples, start, end, false)
    }

    /// `PcmSound(int, byte[], int, int, boolean)`.
    pub fn with_ping_pong(
        rate: i32,
        samples: Vec<i8>,
        start: i32,
        end: i32,
        ping_pong_loop: bool,
    ) -> Self {
        Self {
            rate,
            samples,
            start,
            end,
            ping_pong_loop,
        }
    }

    /// `PcmSound.resample`: in place, as the original (which returns `this`).
    pub fn resample(&mut self, resampler: &PcmResampler) {
        if let Some(samples) = resampler.resample_slice(&self.samples) {
            self.samples = samples;
        }
        self.rate = resampler.scale_rate(self.rate);
        if self.start == self.end {
            self.start = resampler.scale_position(self.start);
            self.end = self.start;
        } else {
            self.start = resampler.scale_position(self.start);
            self.end = resampler.scale_position(self.end);
            if self.start == self.end {
                self.start = self.start.wrapping_sub(1);
            }
        }
    }
}

/// PcmResampler: a 14-tap windowed-sinc polyphase resampler for signed bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcmResampler {
    output_rate: i32,
    /// `null` in the original when both rates are equal (pass-through).
    filter_coefficients: Option<Vec<[i32; 14]>>,
    input_rate: i32,
}

impl PcmResampler {
    /// `PcmResampler(int srcRate, int destRate)`.
    pub fn new(src_rate: i32, dest_rate: i32) -> Self {
        Self::with_audio_sample_rate(src_rate, dest_rate, AUDIO_SAMPLE_RATE)
    }

    /// The constructor with `GlobalConfig.AUDIO_SAMPLE_RATE` made explicit.
    pub fn with_audio_sample_rate(src_rate: i32, dest_rate: i32, audio_sample_rate: i32) -> Self {
        let mut resampler = Self {
            output_rate: 0,
            filter_coefficients: None,
            input_rate: 0,
        };
        if dest_rate != src_rate {
            let divisor = Self::gcd(dest_rate, src_rate, audio_sample_rate);
            let up_factor = dest_rate.wrapping_div(divisor);
            resampler.output_rate = up_factor;
            let down_factor = src_rate.wrapping_div(divisor);
            // `new int[downFactor][14]`: NegativeArraySizeException.
            assert!(down_factor >= 0, "negative resampler phase count");
            let mut filter_coefficients = vec![[0i32; 14]; down_factor as usize];
            resampler.input_rate = down_factor;
            for (phase, coeffs) in filter_coefficients.iter_mut().enumerate() {
                let center = phase as f64 / f64::from(down_factor) + 6.0;
                let gain = f64::from(up_factor) / f64::from(down_factor);
                let mut lo = (center + 1.0 - 7.0).floor() as i32;
                let mut hi = (center + 7.0).ceil() as i32;
                if lo < 0 {
                    lo = 0;
                }
                if hi > 14 {
                    hi = 14;
                }
                while lo < hi {
                    let x = (f64::from(lo) - center) * std::f64::consts::PI;
                    let mut sinc = gain;
                    if !(-1.0e-4..=1.0e-4).contains(&x) {
                        sinc = gain * (x.sin() / x);
                    }
                    sinc *= ((f64::from(lo) - center) * 0.2243994752564138).cos() * 0.46 + 0.54;
                    coeffs[lo as usize] = (sinc * 65536.0 + 0.5).floor() as i32;
                    lo += 1;
                }
            }
            resampler.filter_coefficients = Some(filter_coefficients);
        }
        resampler
    }

    /// `PcmResampler.gcd`. The first branch is the original's: a destination
    /// rate above the configured rate replaces the source rate outright.
    fn gcd(mut a: i32, mut b: i32, audio_sample_rate: i32) -> i32 {
        if a > audio_sample_rate {
            b = a;
            a = audio_sample_rate;
        }
        while a != 0 {
            let remainder = b.wrapping_rem(a);
            b = a;
            a = remainder;
        }
        b
    }

    /// `PcmResampler.resample`; the input is returned untouched when the
    /// rates were equal.
    pub fn resample(&self, samples: Vec<i8>) -> Vec<i8> {
        self.resample_slice(&samples).unwrap_or(samples)
    }

    fn resample_slice(&self, samples: &[i8]) -> Option<Vec<i8>> {
        let filter_coefficients = self.filter_coefficients.as_ref()?;
        let out_len = ((samples.len() as i64 * i64::from(self.output_rate)
            / i64::from(self.input_rate)) as i32)
            .wrapping_add(14);
        // `new int[outLen]`: NegativeArraySizeException.
        assert!(out_len >= 0, "negative resampled length");
        let mut accum = vec![0i32; out_len as usize];
        let mut out_pos: i32 = 0;
        let mut phase: i32 = 0;
        for &sample in samples {
            let coeffs = &filter_coefficients[phase as usize];
            let sample = i32::from(sample);
            for (j, &coeff) in coeffs.iter().enumerate() {
                let slot = &mut accum[out_pos.wrapping_add(j as i32) as usize];
                *slot = slot.wrapping_add(coeff.wrapping_mul(sample));
            }
            phase = phase.wrapping_add(self.output_rate);
            let j = phase.wrapping_div(self.input_rate);
            out_pos = out_pos.wrapping_add(j);
            phase = phase.wrapping_sub(j.wrapping_mul(self.input_rate));
        }
        let mut out = vec![0i8; out_len as usize];
        for (slot, &sum) in out.iter_mut().zip(&accum) {
            let clamped = sum.wrapping_add(32768) >> 16;
            *slot = if clamped < -128 {
                -128
            } else if clamped <= 127 {
                clamped as i8
            } else {
                127
            };
        }
        Some(out)
    }

    /// `PcmResampler.scaleRate`.
    pub fn scale_rate(&self, mut rate: i32) -> i32 {
        if self.filter_coefficients.is_some() {
            rate =
                (i64::from(self.output_rate) * i64::from(rate) / i64::from(self.input_rate)) as i32;
        }
        rate
    }

    /// `PcmResampler.scalePosition`.
    pub fn scale_position(&self, mut position: i32) -> i32 {
        if self.filter_coefficients.is_some() {
            position = ((i64::from(self.output_rate) * i64::from(position)
                / i64::from(self.input_rate)) as i32)
                .wrapping_add(6);
        }
        position
    }
}

/// `SoundPcmStream.calculateLeftVolume`.
pub fn calculate_left_volume(volume: i32, pan: i32) -> i32 {
    if pan < 0 {
        volume
    } else {
        (f64::from(volume) * (f64::from(16384i32.wrapping_sub(pan)) * 1.220703125E-4).sqrt() + 0.5)
            as i32
    }
}

/// `SoundPcmStream.calculateRightVolume`.
pub fn calculate_right_volume(volume: i32, pan: i32) -> i32 {
    if pan < 0 {
        volume.wrapping_neg()
    } else {
        (f64::from(volume) * (f64::from(pan) * 1.220703125E-4).sqrt() + 0.5) as i32
    }
}

/// SoundPcmStream: one voice playing a [`PcmSound`] at a 24.8 fixed-point
/// rate, forwards or backwards, with loops, ping-pong and volume/pan ramps.
#[derive(Clone, Debug)]
pub struct SoundPcmStream {
    sound: Arc<PcmSound>,
    output: Output,
    /// `Node.isLinked()` of the stream in its owner's list.
    linked: bool,
    volume_delta: i32,
    left_volume_delta: i32,
    current_volume: i32,
    loop_count: i32,
    fade_remaining: i32,
    right_volume: i32,
    right_volume_delta: i32,
    left_volume: i32,
    start: i32,
    end: i32,
    ping_pong_loop: bool,
    /// Signed: negative while playing backwards.
    sample_rate: i32,
    /// `Integer.MIN_VALUE` while fading out to release.
    volume: i32,
    pan: i32,
    sample_position: i32,
}

/// `output[index] += value`, with Java's wrapping add and bounds failure.
#[inline(always)]
fn add(output: &mut [i32], index: i32, value: i32) {
    let slot = &mut output[index as usize];
    *slot = slot.wrapping_add(value);
}

#[inline(always)]
fn sample_at(samples: &[i8], index: i32) -> i32 {
    i32::from(samples[index as usize])
}

/// `(from << 8) + (to - from) * (samplePos & 0xFF)`.
#[inline(always)]
fn interpolate(from: i32, to: i32, sample_pos: i32) -> i32 {
    (from << 8).wrapping_add(to.wrapping_sub(from).wrapping_mul(sample_pos & 0xFF))
}

/// `if (rate == 0 || (limit = base + numerator / rate) > maxOutPos) limit = maxOutPos;`
#[inline(always)]
fn resampled_limit(base: i32, numerator: i32, rate: i32, max_out_pos: i32) -> i32 {
    if rate == 0 {
        return max_out_pos;
    }
    let limit = base.wrapping_add(numerator.wrapping_div(rate));
    if limit > max_out_pos {
        max_out_pos
    } else {
        limit
    }
}

fn java_int(frames: usize) -> i32 {
    i32::try_from(frames).expect("frame count exceeds the Java int range")
}

// The sixteen `mix*` loops. The whole-sample variants keep the original's
// four-frame unrolled loop followed by the remainder loop (`limit -= 3` /
// `limit += 3`, or 6 for interleaved stereo); the resampled variants keep the
// interior loop followed by the boundary loop that interpolates towards
// `last_sample`.

#[allow(clippy::too_many_arguments)]
fn mix_forward_stereo(
    samples: &[i8],
    output: &mut [i32],
    mut sample_pos: i32,
    mut out_pos: i32,
    left_vol: i32,
    right_vol: i32,
    max_out_pos: i32,
    sample_end: i32,
    stream: &mut SoundPcmStream,
) -> i32 {
    sample_pos >>= 8;
    let end_sample_idx = sample_end >> 8;
    let scaled_left_vol = left_vol << 2;
    let scaled_right_vol = right_vol << 2;
    let mut limit = out_pos
        .wrapping_add(end_sample_idx)
        .wrapping_sub(sample_pos);
    if limit > max_out_pos {
        limit = max_out_pos;
    }
    out_pos <<= 1;
    limit <<= 1;
    limit = limit.wrapping_sub(6);
    while out_pos < limit {
        for _ in 0..4 {
            let sample = sample_at(samples, sample_pos);
            sample_pos = sample_pos.wrapping_add(1);
            add(output, out_pos, sample.wrapping_mul(scaled_left_vol));
            out_pos = out_pos.wrapping_add(1);
            add(output, out_pos, sample.wrapping_mul(scaled_right_vol));
            out_pos = out_pos.wrapping_add(1);
        }
    }
    limit = limit.wrapping_add(6);
    while out_pos < limit {
        let sample = sample_at(samples, sample_pos);
        sample_pos = sample_pos.wrapping_add(1);
        add(output, out_pos, sample.wrapping_mul(scaled_left_vol));
        out_pos = out_pos.wrapping_add(1);
        add(output, out_pos, sample.wrapping_mul(scaled_right_vol));
        out_pos = out_pos.wrapping_add(1);
    }
    stream.sample_position = sample_pos << 8;
    out_pos >> 1
}

#[allow(clippy::too_many_arguments)]
fn mix_forward_stereo_resampled_fading(
    samples: &[i8],
    output: &mut [i32],
    mut sample_pos: i32,
    mut out_pos: i32,
    mut left_vol: i32,
    mut right_vol: i32,
    left_vol_delta: i32,
    right_vol_delta: i32,
    max_out_pos: i32,
    sample_end: i32,
    stream: &mut SoundPcmStream,
    rate: i32,
    last_sample: i32,
) -> i32 {
    stream.current_volume = stream
        .current_volume
        .wrapping_sub(stream.volume_delta.wrapping_mul(out_pos));
    let mut limit = resampled_limit(
        out_pos,
        sample_end
            .wrapping_add(rate)
            .wrapping_sub(sample_pos)
            .wrapping_sub(257),
        rate,
        max_out_pos,
    );
    out_pos <<= 1;
    limit <<= 1;
    while out_pos < limit {
        let sample_idx = sample_pos >> 8;
        let sample = sample_at(samples, sample_idx);
        let interpolated = interpolate(
            sample,
            sample_at(samples, sample_idx.wrapping_add(1)),
            sample_pos,
        );
        add(output, out_pos, interpolated.wrapping_mul(left_vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        left_vol = left_vol.wrapping_add(left_vol_delta);
        add(output, out_pos, interpolated.wrapping_mul(right_vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        right_vol = right_vol.wrapping_add(right_vol_delta);
        sample_pos = sample_pos.wrapping_add(rate);
    }
    limit = resampled_limit(
        out_pos >> 1,
        sample_end
            .wrapping_add(rate)
            .wrapping_sub(sample_pos)
            .wrapping_sub(1),
        rate,
        max_out_pos,
    );
    limit <<= 1;
    while out_pos < limit {
        let sample = sample_at(samples, sample_pos >> 8);
        let interpolated = interpolate(sample, last_sample, sample_pos);
        add(output, out_pos, interpolated.wrapping_mul(left_vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        left_vol = left_vol.wrapping_add(left_vol_delta);
        add(output, out_pos, interpolated.wrapping_mul(right_vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        right_vol = right_vol.wrapping_add(right_vol_delta);
        sample_pos = sample_pos.wrapping_add(rate);
    }
    let frames = out_pos >> 1;
    stream.current_volume = stream
        .current_volume
        .wrapping_add(stream.volume_delta.wrapping_mul(frames));
    stream.left_volume = left_vol;
    stream.right_volume = right_vol;
    stream.sample_position = sample_pos;
    frames
}

#[allow(clippy::too_many_arguments)]
fn mix_forward_mono_resampled_fading(
    samples: &[i8],
    output: &mut [i32],
    mut sample_pos: i32,
    mut out_pos: i32,
    mut vol: i32,
    vol_delta: i32,
    max_out_pos: i32,
    sample_end: i32,
    stream: &mut SoundPcmStream,
    rate: i32,
    last_sample: i32,
) -> i32 {
    stream.left_volume = stream
        .left_volume
        .wrapping_sub(stream.left_volume_delta.wrapping_mul(out_pos));
    stream.right_volume = stream
        .right_volume
        .wrapping_sub(stream.right_volume_delta.wrapping_mul(out_pos));
    let mut limit = resampled_limit(
        out_pos,
        sample_end
            .wrapping_add(rate)
            .wrapping_sub(sample_pos)
            .wrapping_sub(257),
        rate,
        max_out_pos,
    );
    while out_pos < limit {
        let sample_idx = sample_pos >> 8;
        let sample = sample_at(samples, sample_idx);
        let interpolated = interpolate(
            sample,
            sample_at(samples, sample_idx.wrapping_add(1)),
            sample_pos,
        );
        add(output, out_pos, interpolated.wrapping_mul(vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        vol = vol.wrapping_add(vol_delta);
        sample_pos = sample_pos.wrapping_add(rate);
    }
    limit = resampled_limit(
        out_pos,
        sample_end
            .wrapping_add(rate)
            .wrapping_sub(sample_pos)
            .wrapping_sub(1),
        rate,
        max_out_pos,
    );
    while out_pos < limit {
        let sample = sample_at(samples, sample_pos >> 8);
        let interpolated = interpolate(sample, last_sample, sample_pos);
        add(output, out_pos, interpolated.wrapping_mul(vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        vol = vol.wrapping_add(vol_delta);
        sample_pos = sample_pos.wrapping_add(rate);
    }
    stream.left_volume = stream
        .left_volume
        .wrapping_add(stream.left_volume_delta.wrapping_mul(out_pos));
    stream.right_volume = stream
        .right_volume
        .wrapping_add(stream.right_volume_delta.wrapping_mul(out_pos));
    stream.current_volume = vol;
    stream.sample_position = sample_pos;
    out_pos
}

#[allow(clippy::too_many_arguments)]
fn mix_backward_mono_resampled(
    samples: &[i8],
    output: &mut [i32],
    mut sample_pos: i32,
    mut out_pos: i32,
    vol: i32,
    max_out_pos: i32,
    sample_end: i32,
    stream: &mut SoundPcmStream,
    rate: i32,
    last_sample: i32,
) -> i32 {
    let mut limit = resampled_limit(
        out_pos,
        sample_end
            .wrapping_add(rate)
            .wrapping_add(256)
            .wrapping_sub(sample_pos),
        rate,
        max_out_pos,
    );
    while out_pos < limit {
        let sample_idx = sample_pos >> 8;
        let sample = sample_at(samples, sample_idx.wrapping_sub(1));
        let interpolated = interpolate(sample, sample_at(samples, sample_idx), sample_pos);
        add(output, out_pos, interpolated.wrapping_mul(vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        sample_pos = sample_pos.wrapping_add(rate);
    }
    limit = resampled_limit(
        out_pos,
        sample_end.wrapping_add(rate).wrapping_sub(sample_pos),
        rate,
        max_out_pos,
    );
    while out_pos < limit {
        let interpolated =
            interpolate(last_sample, sample_at(samples, sample_pos >> 8), sample_pos);
        add(output, out_pos, interpolated.wrapping_mul(vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        sample_pos = sample_pos.wrapping_add(rate);
    }
    stream.sample_position = sample_pos;
    out_pos
}

#[allow(clippy::too_many_arguments)]
fn mix_forward_mono(
    samples: &[i8],
    output: &mut [i32],
    mut sample_pos: i32,
    mut out_pos: i32,
    vol: i32,
    max_out_pos: i32,
    sample_end: i32,
    stream: &mut SoundPcmStream,
) -> i32 {
    sample_pos >>= 8;
    let end_sample_idx = sample_end >> 8;
    let scaled_vol = vol << 2;
    let mut limit = out_pos
        .wrapping_add(end_sample_idx)
        .wrapping_sub(sample_pos);
    if limit > max_out_pos {
        limit = max_out_pos;
    }
    limit = limit.wrapping_sub(3);
    while out_pos < limit {
        for _ in 0..4 {
            add(
                output,
                out_pos,
                sample_at(samples, sample_pos).wrapping_mul(scaled_vol),
            );
            out_pos = out_pos.wrapping_add(1);
            sample_pos = sample_pos.wrapping_add(1);
        }
    }
    limit = limit.wrapping_add(3);
    while out_pos < limit {
        add(
            output,
            out_pos,
            sample_at(samples, sample_pos).wrapping_mul(scaled_vol),
        );
        out_pos = out_pos.wrapping_add(1);
        sample_pos = sample_pos.wrapping_add(1);
    }
    stream.sample_position = sample_pos << 8;
    out_pos
}

#[allow(clippy::too_many_arguments)]
fn mix_forward_stereo_fading(
    samples: &[i8],
    output: &mut [i32],
    mut sample_pos: i32,
    mut out_pos: i32,
    mut left_vol: i32,
    mut right_vol: i32,
    left_vol_delta: i32,
    right_vol_delta: i32,
    max_out_pos: i32,
    sample_end: i32,
    stream: &mut SoundPcmStream,
) -> i32 {
    sample_pos >>= 8;
    let end_sample_idx = sample_end >> 8;
    left_vol <<= 2;
    right_vol <<= 2;
    let scaled_left_vol_delta = left_vol_delta << 2;
    let scaled_right_vol_delta = right_vol_delta << 2;
    let mut limit = out_pos
        .wrapping_add(end_sample_idx)
        .wrapping_sub(sample_pos);
    if limit > max_out_pos {
        limit = max_out_pos;
    }
    stream.current_volume = stream.current_volume.wrapping_add(
        stream
            .volume_delta
            .wrapping_mul(limit.wrapping_sub(out_pos)),
    );
    out_pos <<= 1;
    limit <<= 1;
    limit = limit.wrapping_sub(6);
    while out_pos < limit {
        for _ in 0..4 {
            let sample = sample_at(samples, sample_pos);
            sample_pos = sample_pos.wrapping_add(1);
            add(output, out_pos, sample.wrapping_mul(left_vol));
            out_pos = out_pos.wrapping_add(1);
            left_vol = left_vol.wrapping_add(scaled_left_vol_delta);
            add(output, out_pos, sample.wrapping_mul(right_vol));
            out_pos = out_pos.wrapping_add(1);
            right_vol = right_vol.wrapping_add(scaled_right_vol_delta);
        }
    }
    limit = limit.wrapping_add(6);
    while out_pos < limit {
        let sample = sample_at(samples, sample_pos);
        sample_pos = sample_pos.wrapping_add(1);
        add(output, out_pos, sample.wrapping_mul(left_vol));
        out_pos = out_pos.wrapping_add(1);
        left_vol = left_vol.wrapping_add(scaled_left_vol_delta);
        add(output, out_pos, sample.wrapping_mul(right_vol));
        out_pos = out_pos.wrapping_add(1);
        right_vol = right_vol.wrapping_add(scaled_right_vol_delta);
    }
    stream.left_volume = left_vol >> 2;
    stream.right_volume = right_vol >> 2;
    stream.sample_position = sample_pos << 8;
    out_pos >> 1
}

#[allow(clippy::too_many_arguments)]
fn mix_backward_mono_fading(
    samples: &[i8],
    output: &mut [i32],
    mut sample_pos: i32,
    mut out_pos: i32,
    mut vol: i32,
    vol_delta: i32,
    max_out_pos: i32,
    sample_end: i32,
    stream: &mut SoundPcmStream,
) -> i32 {
    sample_pos >>= 8;
    let end_sample_idx = sample_end >> 8;
    vol <<= 2;
    let scaled_vol_delta = vol_delta << 2;
    let mut limit = out_pos
        .wrapping_add(sample_pos)
        .wrapping_add(1)
        .wrapping_sub(end_sample_idx);
    if limit > max_out_pos {
        limit = max_out_pos;
    }
    stream.left_volume = stream.left_volume.wrapping_add(
        stream
            .left_volume_delta
            .wrapping_mul(limit.wrapping_sub(out_pos)),
    );
    stream.right_volume = stream.right_volume.wrapping_add(
        stream
            .right_volume_delta
            .wrapping_mul(limit.wrapping_sub(out_pos)),
    );
    limit = limit.wrapping_sub(3);
    while out_pos < limit {
        for _ in 0..4 {
            add(
                output,
                out_pos,
                sample_at(samples, sample_pos).wrapping_mul(vol),
            );
            out_pos = out_pos.wrapping_add(1);
            sample_pos = sample_pos.wrapping_sub(1);
            vol = vol.wrapping_add(scaled_vol_delta);
        }
    }
    limit = limit.wrapping_add(3);
    while out_pos < limit {
        add(
            output,
            out_pos,
            sample_at(samples, sample_pos).wrapping_mul(vol),
        );
        out_pos = out_pos.wrapping_add(1);
        sample_pos = sample_pos.wrapping_sub(1);
        vol = vol.wrapping_add(scaled_vol_delta);
    }
    stream.current_volume = vol >> 2;
    stream.sample_position = sample_pos << 8;
    out_pos
}

#[allow(clippy::too_many_arguments)]
fn mix_forward_mono_fading(
    samples: &[i8],
    output: &mut [i32],
    mut sample_pos: i32,
    mut out_pos: i32,
    mut vol: i32,
    vol_delta: i32,
    max_out_pos: i32,
    sample_end: i32,
    stream: &mut SoundPcmStream,
) -> i32 {
    sample_pos >>= 8;
    let end_sample_idx = sample_end >> 8;
    vol <<= 2;
    let scaled_vol_delta = vol_delta << 2;
    let mut limit = out_pos
        .wrapping_add(end_sample_idx)
        .wrapping_sub(sample_pos);
    if limit > max_out_pos {
        limit = max_out_pos;
    }
    stream.left_volume = stream.left_volume.wrapping_add(
        stream
            .left_volume_delta
            .wrapping_mul(limit.wrapping_sub(out_pos)),
    );
    stream.right_volume = stream.right_volume.wrapping_add(
        stream
            .right_volume_delta
            .wrapping_mul(limit.wrapping_sub(out_pos)),
    );
    limit = limit.wrapping_sub(3);
    while out_pos < limit {
        for _ in 0..4 {
            add(
                output,
                out_pos,
                sample_at(samples, sample_pos).wrapping_mul(vol),
            );
            out_pos = out_pos.wrapping_add(1);
            sample_pos = sample_pos.wrapping_add(1);
            vol = vol.wrapping_add(scaled_vol_delta);
        }
    }
    limit = limit.wrapping_add(3);
    while out_pos < limit {
        add(
            output,
            out_pos,
            sample_at(samples, sample_pos).wrapping_mul(vol),
        );
        out_pos = out_pos.wrapping_add(1);
        sample_pos = sample_pos.wrapping_add(1);
        vol = vol.wrapping_add(scaled_vol_delta);
    }
    stream.current_volume = vol >> 2;
    stream.sample_position = sample_pos << 8;
    out_pos
}

#[allow(clippy::too_many_arguments)]
fn mix_forward_stereo_resampled(
    samples: &[i8],
    output: &mut [i32],
    mut sample_pos: i32,
    mut out_pos: i32,
    left_vol: i32,
    right_vol: i32,
    max_out_pos: i32,
    sample_end: i32,
    stream: &mut SoundPcmStream,
    rate: i32,
    last_sample: i32,
) -> i32 {
    let mut limit = resampled_limit(
        out_pos,
        sample_end
            .wrapping_add(rate)
            .wrapping_sub(sample_pos)
            .wrapping_sub(257),
        rate,
        max_out_pos,
    );
    out_pos <<= 1;
    limit <<= 1;
    while out_pos < limit {
        let sample_idx = sample_pos >> 8;
        let sample = sample_at(samples, sample_idx);
        let interpolated = interpolate(
            sample,
            sample_at(samples, sample_idx.wrapping_add(1)),
            sample_pos,
        );
        add(output, out_pos, interpolated.wrapping_mul(left_vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        add(output, out_pos, interpolated.wrapping_mul(right_vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        sample_pos = sample_pos.wrapping_add(rate);
    }
    limit = resampled_limit(
        out_pos >> 1,
        sample_end
            .wrapping_add(rate)
            .wrapping_sub(sample_pos)
            .wrapping_sub(1),
        rate,
        max_out_pos,
    );
    limit <<= 1;
    while out_pos < limit {
        let sample = sample_at(samples, sample_pos >> 8);
        let interpolated = interpolate(sample, last_sample, sample_pos);
        add(output, out_pos, interpolated.wrapping_mul(left_vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        add(output, out_pos, interpolated.wrapping_mul(right_vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        sample_pos = sample_pos.wrapping_add(rate);
    }
    stream.sample_position = sample_pos;
    out_pos >> 1
}

#[allow(clippy::too_many_arguments)]
fn mix_forward_mono_resampled(
    samples: &[i8],
    output: &mut [i32],
    mut sample_pos: i32,
    mut out_pos: i32,
    vol: i32,
    max_out_pos: i32,
    sample_end: i32,
    stream: &mut SoundPcmStream,
    rate: i32,
    last_sample: i32,
) -> i32 {
    let mut limit = resampled_limit(
        out_pos,
        sample_end
            .wrapping_add(rate)
            .wrapping_sub(sample_pos)
            .wrapping_sub(257),
        rate,
        max_out_pos,
    );
    while out_pos < limit {
        let sample_idx = sample_pos >> 8;
        let sample = sample_at(samples, sample_idx);
        let interpolated = interpolate(
            sample,
            sample_at(samples, sample_idx.wrapping_add(1)),
            sample_pos,
        );
        add(output, out_pos, interpolated.wrapping_mul(vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        sample_pos = sample_pos.wrapping_add(rate);
    }
    limit = resampled_limit(
        out_pos,
        sample_end
            .wrapping_add(rate)
            .wrapping_sub(sample_pos)
            .wrapping_sub(1),
        rate,
        max_out_pos,
    );
    while out_pos < limit {
        let sample = sample_at(samples, sample_pos >> 8);
        let interpolated = interpolate(sample, last_sample, sample_pos);
        add(output, out_pos, interpolated.wrapping_mul(vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        sample_pos = sample_pos.wrapping_add(rate);
    }
    stream.sample_position = sample_pos;
    out_pos
}

#[allow(clippy::too_many_arguments)]
fn mix_backward_stereo_resampled(
    samples: &[i8],
    output: &mut [i32],
    mut sample_pos: i32,
    mut out_pos: i32,
    left_vol: i32,
    right_vol: i32,
    max_out_pos: i32,
    sample_end: i32,
    stream: &mut SoundPcmStream,
    rate: i32,
    last_sample: i32,
) -> i32 {
    let mut limit = resampled_limit(
        out_pos,
        sample_end
            .wrapping_add(rate)
            .wrapping_add(256)
            .wrapping_sub(sample_pos),
        rate,
        max_out_pos,
    );
    out_pos <<= 1;
    limit <<= 1;
    while out_pos < limit {
        let sample_idx = sample_pos >> 8;
        let sample = sample_at(samples, sample_idx.wrapping_sub(1));
        let interpolated = interpolate(sample, sample_at(samples, sample_idx), sample_pos);
        add(output, out_pos, interpolated.wrapping_mul(left_vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        add(output, out_pos, interpolated.wrapping_mul(right_vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        sample_pos = sample_pos.wrapping_add(rate);
    }
    limit = resampled_limit(
        out_pos >> 1,
        sample_end.wrapping_add(rate).wrapping_sub(sample_pos),
        rate,
        max_out_pos,
    );
    limit <<= 1;
    while out_pos < limit {
        let interpolated =
            interpolate(last_sample, sample_at(samples, sample_pos >> 8), sample_pos);
        add(output, out_pos, interpolated.wrapping_mul(left_vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        add(output, out_pos, interpolated.wrapping_mul(right_vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        sample_pos = sample_pos.wrapping_add(rate);
    }
    stream.sample_position = sample_pos;
    out_pos >> 1
}

#[allow(clippy::too_many_arguments)]
fn mix_backward_mono(
    samples: &[i8],
    output: &mut [i32],
    mut sample_pos: i32,
    mut out_pos: i32,
    vol: i32,
    max_out_pos: i32,
    sample_end: i32,
    stream: &mut SoundPcmStream,
) -> i32 {
    sample_pos >>= 8;
    let end_sample_idx = sample_end >> 8;
    let scaled_vol = vol << 2;
    let mut limit = out_pos
        .wrapping_add(sample_pos)
        .wrapping_add(1)
        .wrapping_sub(end_sample_idx);
    if limit > max_out_pos {
        limit = max_out_pos;
    }
    limit = limit.wrapping_sub(3);
    while out_pos < limit {
        for _ in 0..4 {
            add(
                output,
                out_pos,
                sample_at(samples, sample_pos).wrapping_mul(scaled_vol),
            );
            out_pos = out_pos.wrapping_add(1);
            sample_pos = sample_pos.wrapping_sub(1);
        }
    }
    limit = limit.wrapping_add(3);
    while out_pos < limit {
        add(
            output,
            out_pos,
            sample_at(samples, sample_pos).wrapping_mul(scaled_vol),
        );
        out_pos = out_pos.wrapping_add(1);
        sample_pos = sample_pos.wrapping_sub(1);
    }
    stream.sample_position = sample_pos << 8;
    out_pos
}

#[allow(clippy::too_many_arguments)]
fn mix_backward_mono_resampled_fading(
    samples: &[i8],
    output: &mut [i32],
    mut sample_pos: i32,
    mut out_pos: i32,
    mut vol: i32,
    vol_delta: i32,
    max_out_pos: i32,
    sample_end: i32,
    stream: &mut SoundPcmStream,
    rate: i32,
    last_sample: i32,
) -> i32 {
    stream.left_volume = stream
        .left_volume
        .wrapping_sub(stream.left_volume_delta.wrapping_mul(out_pos));
    stream.right_volume = stream
        .right_volume
        .wrapping_sub(stream.right_volume_delta.wrapping_mul(out_pos));
    let mut limit = resampled_limit(
        out_pos,
        sample_end
            .wrapping_add(rate)
            .wrapping_add(256)
            .wrapping_sub(sample_pos),
        rate,
        max_out_pos,
    );
    while out_pos < limit {
        let sample_idx = sample_pos >> 8;
        let sample = sample_at(samples, sample_idx.wrapping_sub(1));
        let interpolated = interpolate(sample, sample_at(samples, sample_idx), sample_pos);
        add(output, out_pos, interpolated.wrapping_mul(vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        vol = vol.wrapping_add(vol_delta);
        sample_pos = sample_pos.wrapping_add(rate);
    }
    limit = resampled_limit(
        out_pos,
        sample_end.wrapping_add(rate).wrapping_sub(sample_pos),
        rate,
        max_out_pos,
    );
    while out_pos < limit {
        let interpolated =
            interpolate(last_sample, sample_at(samples, sample_pos >> 8), sample_pos);
        add(output, out_pos, interpolated.wrapping_mul(vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        vol = vol.wrapping_add(vol_delta);
        sample_pos = sample_pos.wrapping_add(rate);
    }
    stream.left_volume = stream
        .left_volume
        .wrapping_add(stream.left_volume_delta.wrapping_mul(out_pos));
    stream.right_volume = stream
        .right_volume
        .wrapping_add(stream.right_volume_delta.wrapping_mul(out_pos));
    stream.current_volume = vol;
    stream.sample_position = sample_pos;
    out_pos
}

#[allow(clippy::too_many_arguments)]
fn mix_backward_stereo(
    samples: &[i8],
    output: &mut [i32],
    mut sample_pos: i32,
    mut out_pos: i32,
    left_vol: i32,
    right_vol: i32,
    max_out_pos: i32,
    sample_end: i32,
    stream: &mut SoundPcmStream,
) -> i32 {
    sample_pos >>= 8;
    let end_sample_idx = sample_end >> 8;
    let scaled_left_vol = left_vol << 2;
    let scaled_right_vol = right_vol << 2;
    let mut limit = out_pos
        .wrapping_add(sample_pos)
        .wrapping_add(1)
        .wrapping_sub(end_sample_idx);
    if limit > max_out_pos {
        limit = max_out_pos;
    }
    out_pos <<= 1;
    limit <<= 1;
    limit = limit.wrapping_sub(6);
    while out_pos < limit {
        for _ in 0..4 {
            let sample = sample_at(samples, sample_pos);
            sample_pos = sample_pos.wrapping_sub(1);
            add(output, out_pos, sample.wrapping_mul(scaled_left_vol));
            out_pos = out_pos.wrapping_add(1);
            add(output, out_pos, sample.wrapping_mul(scaled_right_vol));
            out_pos = out_pos.wrapping_add(1);
        }
    }
    limit = limit.wrapping_add(6);
    while out_pos < limit {
        let sample = sample_at(samples, sample_pos);
        sample_pos = sample_pos.wrapping_sub(1);
        add(output, out_pos, sample.wrapping_mul(scaled_left_vol));
        out_pos = out_pos.wrapping_add(1);
        add(output, out_pos, sample.wrapping_mul(scaled_right_vol));
        out_pos = out_pos.wrapping_add(1);
    }
    stream.sample_position = sample_pos << 8;
    out_pos >> 1
}

#[allow(clippy::too_many_arguments)]
fn mix_backward_stereo_resampled_fading(
    samples: &[i8],
    output: &mut [i32],
    mut sample_pos: i32,
    mut out_pos: i32,
    mut left_vol: i32,
    mut right_vol: i32,
    left_vol_delta: i32,
    right_vol_delta: i32,
    max_out_pos: i32,
    sample_end: i32,
    stream: &mut SoundPcmStream,
    rate: i32,
    last_sample: i32,
) -> i32 {
    stream.current_volume = stream
        .current_volume
        .wrapping_sub(stream.volume_delta.wrapping_mul(out_pos));
    let mut limit = resampled_limit(
        out_pos,
        sample_end
            .wrapping_add(rate)
            .wrapping_add(256)
            .wrapping_sub(sample_pos),
        rate,
        max_out_pos,
    );
    out_pos <<= 1;
    limit <<= 1;
    while out_pos < limit {
        let sample_idx = sample_pos >> 8;
        let sample = sample_at(samples, sample_idx.wrapping_sub(1));
        let interpolated = interpolate(sample, sample_at(samples, sample_idx), sample_pos);
        add(output, out_pos, interpolated.wrapping_mul(left_vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        left_vol = left_vol.wrapping_add(left_vol_delta);
        add(output, out_pos, interpolated.wrapping_mul(right_vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        right_vol = right_vol.wrapping_add(right_vol_delta);
        sample_pos = sample_pos.wrapping_add(rate);
    }
    limit = resampled_limit(
        out_pos >> 1,
        sample_end.wrapping_add(rate).wrapping_sub(sample_pos),
        rate,
        max_out_pos,
    );
    limit <<= 1;
    while out_pos < limit {
        let interpolated =
            interpolate(last_sample, sample_at(samples, sample_pos >> 8), sample_pos);
        add(output, out_pos, interpolated.wrapping_mul(left_vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        left_vol = left_vol.wrapping_add(left_vol_delta);
        add(output, out_pos, interpolated.wrapping_mul(right_vol) >> 6);
        out_pos = out_pos.wrapping_add(1);
        right_vol = right_vol.wrapping_add(right_vol_delta);
        sample_pos = sample_pos.wrapping_add(rate);
    }
    let frames = out_pos >> 1;
    stream.current_volume = stream
        .current_volume
        .wrapping_add(stream.volume_delta.wrapping_mul(frames));
    stream.left_volume = left_vol;
    stream.right_volume = right_vol;
    stream.sample_position = sample_pos;
    frames
}

#[allow(clippy::too_many_arguments)]
fn mix_backward_stereo_fading(
    samples: &[i8],
    output: &mut [i32],
    mut sample_pos: i32,
    mut out_pos: i32,
    mut left_vol: i32,
    mut right_vol: i32,
    left_vol_delta: i32,
    right_vol_delta: i32,
    max_out_pos: i32,
    sample_end: i32,
    stream: &mut SoundPcmStream,
) -> i32 {
    sample_pos >>= 8;
    let end_sample_idx = sample_end >> 8;
    left_vol <<= 2;
    right_vol <<= 2;
    let scaled_left_vol_delta = left_vol_delta << 2;
    let scaled_right_vol_delta = right_vol_delta << 2;
    let mut limit = out_pos
        .wrapping_add(sample_pos)
        .wrapping_add(1)
        .wrapping_sub(end_sample_idx);
    if limit > max_out_pos {
        limit = max_out_pos;
    }
    stream.current_volume = stream.current_volume.wrapping_add(
        stream
            .volume_delta
            .wrapping_mul(limit.wrapping_sub(out_pos)),
    );
    out_pos <<= 1;
    limit <<= 1;
    limit = limit.wrapping_sub(6);
    while out_pos < limit {
        for _ in 0..4 {
            let sample = sample_at(samples, sample_pos);
            sample_pos = sample_pos.wrapping_sub(1);
            add(output, out_pos, sample.wrapping_mul(left_vol));
            out_pos = out_pos.wrapping_add(1);
            left_vol = left_vol.wrapping_add(scaled_left_vol_delta);
            add(output, out_pos, sample.wrapping_mul(right_vol));
            out_pos = out_pos.wrapping_add(1);
            right_vol = right_vol.wrapping_add(scaled_right_vol_delta);
        }
    }
    limit = limit.wrapping_add(6);
    while out_pos < limit {
        let sample = sample_at(samples, sample_pos);
        sample_pos = sample_pos.wrapping_sub(1);
        add(output, out_pos, sample.wrapping_mul(left_vol));
        out_pos = out_pos.wrapping_add(1);
        left_vol = left_vol.wrapping_add(scaled_left_vol_delta);
        add(output, out_pos, sample.wrapping_mul(right_vol));
        out_pos = out_pos.wrapping_add(1);
        right_vol = right_vol.wrapping_add(scaled_right_vol_delta);
    }
    stream.left_volume = left_vol >> 2;
    stream.right_volume = right_vol >> 2;
    stream.sample_position = sample_pos << 8;
    out_pos >> 1
}

impl SoundPcmStream {
    /// Both constructors; a sound the original cannot hold (no samples, or
    /// more than a Java array can index) yields no stream.
    fn new(
        sound: Arc<PcmSound>,
        sample_rate: i32,
        volume: i32,
        pan: i32,
        output: Output,
    ) -> Option<Self> {
        if sound.samples.is_empty() || sound.samples.len() > i32::MAX as usize {
            return None;
        }
        let mut stream = Self {
            start: sound.start,
            end: sound.end,
            ping_pong_loop: sound.ping_pong_loop,
            sound,
            output,
            linked: false,
            volume_delta: 0,
            left_volume_delta: 0,
            current_volume: 0,
            loop_count: 0,
            fade_remaining: 0,
            right_volume: 0,
            right_volume_delta: 0,
            left_volume: 0,
            sample_rate,
            volume,
            pan,
            sample_position: 0,
        };
        stream.recalculate_channel_volumes();
        Some(stream)
    }

    /// `create(PcmSound, int)`: the sound's own rate against the output rate,
    /// centre pan, `volume` in 0..=255-style units (stored `<< 6`).
    pub fn create(sound: Arc<PcmSound>, volume: i32, output: Output) -> Option<Self> {
        if sound.samples.is_empty() {
            return None;
        }
        let sample_rate =
            (i64::from(sound.rate) * 256 * 100 / (i64::from(output.sample_rate) * 100)) as i32;
        Self::new(sound, sample_rate, volume << 6, 8192, output)
    }

    /// `create(PcmSound, int, int, int)`: explicit 24.8 rate, raw volume, pan.
    pub fn create_with_rate(
        sound: Arc<PcmSound>,
        sample_rate: i32,
        volume: i32,
        pan: i32,
        output: Output,
    ) -> Option<Self> {
        Self::new(sound, sample_rate, volume, pan, output)
    }

    /// The sound this voice plays (`PcmStream.sound`).
    pub fn sound(&self) -> &Arc<PcmSound> {
        &self.sound
    }

    pub fn output(&self) -> Output {
        self.output
    }

    /// Java `Node.isLinked()`. A new stream is not linked; its owner links it
    /// with [`Self::set_linked`] when it adds the stream to a mixer list, and
    /// the stream unlinks itself when it finishes or has faded out to release.
    pub fn is_linked(&self) -> bool {
        self.linked
    }

    /// The owner adding the stream to (or taking it out of) its list.
    pub fn set_linked(&mut self, linked: bool) {
        self.linked = linked;
    }

    /// Java `Node.unlink()`: a no-op when the stream is not linked.
    pub fn unlink(&mut self) {
        self.linked = false;
    }

    /// 24.8 fixed-point position (`samplePosition`).
    pub fn sample_position(&self) -> i32 {
        self.sample_position
    }

    /// Remaining loops; negative loops forever (`loopCount`).
    pub fn loop_count(&self) -> i32 {
        self.loop_count
    }

    /// Signed 24.8 rate; negative while playing backwards (`sampleRate`).
    pub fn sample_rate(&self) -> i32 {
        self.sample_rate
    }

    /// Frames left in the current ramp (`fadeRemaining`).
    pub fn fade_remaining(&self) -> i32 {
        self.fade_remaining
    }

    /// The stored target volume, `i32::MIN` while fading out to release.
    pub fn target_volume(&self) -> i32 {
        self.volume
    }

    /// The stored pan, without `get_pan`'s clamp of negatives to -1.
    pub fn target_pan(&self) -> i32 {
        self.pan
    }

    pub fn current_volume(&self) -> i32 {
        self.current_volume
    }

    pub fn left_volume(&self) -> i32 {
        self.left_volume
    }

    pub fn right_volume(&self) -> i32 {
        self.right_volume
    }

    pub fn volume_delta(&self) -> i32 {
        self.volume_delta
    }

    pub fn left_volume_delta(&self) -> i32 {
        self.left_volume_delta
    }

    pub fn right_volume_delta(&self) -> i32 {
        self.right_volume_delta
    }

    fn sample_length(&self) -> i32 {
        self.sound.samples.len() as i32
    }

    /// `read(int[], int, int)`. Interleaved when stereo, exactly as the Java
    /// `int[]`: `buffer` holds `offset + length` frames, twice as many ints
    /// when stereo. Mixes (adds) into the buffer.
    pub fn read(&mut self, buffer: &mut [i32], offset: usize, length: usize) {
        let offset = java_int(offset);
        let length = java_int(length);
        if self.volume == 0 && self.fade_remaining == 0 {
            self.skip(length);
            return;
        }
        let sound = Arc::clone(&self.sound);
        let samples = sound.samples.as_slice();
        let loop_start = self.start << 8;
        let loop_end = self.end << 8;
        let sample_length = self.sample_length() << 8;
        let loop_length = loop_end.wrapping_sub(loop_start);
        if loop_length <= 0 {
            self.loop_count = 0;
        }
        let mut pos = offset;
        let end_pos = length.wrapping_add(offset);
        if self.sample_position < 0 {
            if self.sample_rate <= 0 {
                self.cancel_transition();
                self.unlink();
                return;
            }
            self.sample_position = 0;
        }
        if self.sample_position >= sample_length {
            if self.sample_rate >= 0 {
                self.cancel_transition();
                self.unlink();
                return;
            }
            self.sample_position = sample_length.wrapping_sub(1);
        }
        if self.loop_count >= 0 {
            if self.loop_count > 0 {
                if self.ping_pong_loop {
                    'loop_done: {
                        if self.sample_rate < 0 {
                            let boundary = sample_at(samples, self.start);
                            pos = self.read_backward(
                                samples, buffer, offset, loop_start, end_pos, boundary,
                            );
                            if self.sample_position >= loop_start {
                                return;
                            }
                            self.reflect(loop_start);
                            self.loop_count = self.loop_count.wrapping_sub(1);
                            if self.loop_count == 0 {
                                break 'loop_done;
                            }
                        }
                        loop {
                            let boundary = sample_at(samples, self.end.wrapping_sub(1));
                            pos = self
                                .read_forward(samples, buffer, pos, loop_end, end_pos, boundary);
                            if self.sample_position < loop_end {
                                return;
                            }
                            self.reflect(loop_end);
                            self.loop_count = self.loop_count.wrapping_sub(1);
                            if self.loop_count == 0 {
                                break;
                            }
                            let boundary = sample_at(samples, self.start);
                            pos = self
                                .read_backward(samples, buffer, pos, loop_start, end_pos, boundary);
                            if self.sample_position >= loop_start {
                                return;
                            }
                            self.reflect(loop_start);
                            self.loop_count = self.loop_count.wrapping_sub(1);
                            if self.loop_count == 0 {
                                break;
                            }
                        }
                    }
                } else if self.sample_rate < 0 {
                    loop {
                        let boundary = sample_at(samples, self.end.wrapping_sub(1));
                        pos =
                            self.read_backward(samples, buffer, pos, loop_start, end_pos, boundary);
                        if self.sample_position >= loop_start {
                            return;
                        }
                        let loop_skips = loop_end
                            .wrapping_sub(self.sample_position)
                            .wrapping_sub(1)
                            .wrapping_div(loop_length);
                        if loop_skips >= self.loop_count {
                            self.sample_position = self
                                .sample_position
                                .wrapping_add(loop_length.wrapping_mul(self.loop_count));
                            self.loop_count = 0;
                            break;
                        }
                        self.sample_position = self
                            .sample_position
                            .wrapping_add(loop_length.wrapping_mul(loop_skips));
                        self.loop_count = self.loop_count.wrapping_sub(loop_skips);
                    }
                } else {
                    loop {
                        let boundary = sample_at(samples, self.start);
                        pos = self.read_forward(samples, buffer, pos, loop_end, end_pos, boundary);
                        if self.sample_position < loop_end {
                            return;
                        }
                        let loop_skips = self
                            .sample_position
                            .wrapping_sub(loop_start)
                            .wrapping_div(loop_length);
                        if loop_skips >= self.loop_count {
                            self.sample_position = self
                                .sample_position
                                .wrapping_sub(loop_length.wrapping_mul(self.loop_count));
                            self.loop_count = 0;
                            break;
                        }
                        self.sample_position = self
                            .sample_position
                            .wrapping_sub(loop_length.wrapping_mul(loop_skips));
                        self.loop_count = self.loop_count.wrapping_sub(loop_skips);
                    }
                }
            }
            if self.sample_rate < 0 {
                self.read_backward(samples, buffer, pos, 0, end_pos, 0);
                if self.sample_position < 0 {
                    self.sample_position = -1;
                    self.cancel_transition();
                    self.unlink();
                }
            } else {
                self.read_forward(samples, buffer, pos, sample_length, end_pos, 0);
                if self.sample_position >= sample_length {
                    self.sample_position = sample_length;
                    self.cancel_transition();
                    self.unlink();
                }
            }
        } else if self.ping_pong_loop {
            if self.sample_rate < 0 {
                let boundary = sample_at(samples, self.start);
                pos = self.read_backward(samples, buffer, offset, loop_start, end_pos, boundary);
                if self.sample_position >= loop_start {
                    return;
                }
                self.reflect(loop_start);
            }
            loop {
                let boundary = sample_at(samples, self.end.wrapping_sub(1));
                pos = self.read_forward(samples, buffer, pos, loop_end, end_pos, boundary);
                if self.sample_position < loop_end {
                    return;
                }
                self.reflect(loop_end);
                let boundary = sample_at(samples, self.start);
                pos = self.read_backward(samples, buffer, pos, loop_start, end_pos, boundary);
                if self.sample_position >= loop_start {
                    return;
                }
                self.reflect(loop_start);
            }
        } else if self.sample_rate < 0 {
            loop {
                let boundary = sample_at(samples, self.end.wrapping_sub(1));
                pos = self.read_backward(samples, buffer, pos, loop_start, end_pos, boundary);
                if self.sample_position >= loop_start {
                    return;
                }
                self.sample_position = loop_end
                    .wrapping_sub(
                        loop_end
                            .wrapping_sub(1)
                            .wrapping_sub(self.sample_position)
                            .wrapping_rem(loop_length),
                    )
                    .wrapping_sub(1);
            }
        } else {
            loop {
                let boundary = sample_at(samples, self.start);
                pos = self.read_forward(samples, buffer, pos, loop_end, end_pos, boundary);
                if self.sample_position < loop_end {
                    return;
                }
                self.sample_position = loop_start.wrapping_add(
                    self.sample_position
                        .wrapping_sub(loop_start)
                        .wrapping_rem(loop_length),
                );
            }
        }
    }

    /// The ping-pong turn-around: mirror the position about `edge` and flip
    /// the direction.
    fn reflect(&mut self, edge: i32) {
        self.sample_position = edge
            .wrapping_add(edge)
            .wrapping_sub(self.sample_position)
            .wrapping_sub(1);
        self.sample_rate = self.sample_rate.wrapping_neg();
    }

    /// `applyVolumeTransition`: at the end of a ramp, either start a unit-step
    /// ramp towards the exact target, settle on it, or (fade-out) release.
    fn apply_volume_transition(&mut self) -> bool {
        let mut target_volume = self.volume;
        let target_left_vol;
        let target_right_vol;
        if target_volume == i32::MIN {
            target_right_vol = 0;
            target_left_vol = 0;
            target_volume = 0;
        } else {
            target_left_vol = calculate_left_volume(target_volume, self.pan);
            target_right_vol = calculate_right_volume(target_volume, self.pan);
        }
        if self.current_volume != target_volume
            || self.left_volume != target_left_vol
            || self.right_volume != target_right_vol
        {
            if self.current_volume < target_volume {
                self.volume_delta = 1;
                self.fade_remaining = target_volume.wrapping_sub(self.current_volume);
            } else if self.current_volume > target_volume {
                self.volume_delta = -1;
                self.fade_remaining = self.current_volume.wrapping_sub(target_volume);
            } else {
                self.volume_delta = 0;
            }
            if self.left_volume < target_left_vol {
                self.left_volume_delta = 1;
                let distance = target_left_vol.wrapping_sub(self.left_volume);
                if self.fade_remaining == 0 || self.fade_remaining > distance {
                    self.fade_remaining = distance;
                }
            } else if self.left_volume > target_left_vol {
                self.left_volume_delta = -1;
                let distance = self.left_volume.wrapping_sub(target_left_vol);
                if self.fade_remaining == 0 || self.fade_remaining > distance {
                    self.fade_remaining = distance;
                }
            } else {
                self.left_volume_delta = 0;
            }
            if self.right_volume < target_right_vol {
                self.right_volume_delta = 1;
                let distance = target_right_vol.wrapping_sub(self.right_volume);
                if self.fade_remaining == 0 || self.fade_remaining > distance {
                    self.fade_remaining = distance;
                }
            } else if self.right_volume > target_right_vol {
                self.right_volume_delta = -1;
                let distance = self.right_volume.wrapping_sub(target_right_vol);
                if self.fade_remaining == 0 || self.fade_remaining > distance {
                    self.fade_remaining = distance;
                }
            } else {
                self.right_volume_delta = 0;
            }
            false
        } else if self.volume == i32::MIN {
            self.volume = 0;
            self.current_volume = 0;
            self.left_volume = 0;
            self.right_volume = 0;
            self.unlink();
            true
        } else {
            self.recalculate_channel_volumes();
            false
        }
    }

    /// `fadeOutAndRelease`: ramp to silence over at most `duration` frames,
    /// then unlink; immediately when `duration` is 0 or already silent.
    pub fn fade_out_and_release(&mut self, mut duration: i32) {
        if duration == 0 {
            self.reset_volume();
            self.unlink();
        } else if self.left_volume == 0 && self.right_volume == 0 {
            self.fade_remaining = 0;
            self.volume = 0;
            self.current_volume = 0;
            self.unlink();
        } else {
            let mut max_duration = self.current_volume.wrapping_neg();
            if self.current_volume > max_duration {
                max_duration = self.current_volume;
            }
            if self.left_volume.wrapping_neg() > max_duration {
                max_duration = self.left_volume.wrapping_neg();
            }
            if self.left_volume > max_duration {
                max_duration = self.left_volume;
            }
            if self.right_volume.wrapping_neg() > max_duration {
                max_duration = self.right_volume.wrapping_neg();
            }
            if self.right_volume > max_duration {
                max_duration = self.right_volume;
            }
            if duration > max_duration {
                duration = max_duration;
            }
            self.fade_remaining = duration;
            self.volume = i32::MIN;
            self.volume_delta = self.current_volume.wrapping_neg().wrapping_div(duration);
            self.left_volume_delta = self.left_volume.wrapping_neg().wrapping_div(duration);
            self.right_volume_delta = self.right_volume.wrapping_neg().wrapping_div(duration);
        }
    }

    fn read_forward(
        &mut self,
        samples: &[i8],
        output: &mut [i32],
        mut out_pos: i32,
        sample_end: i32,
        max_out_pos: i32,
        boundary_sample: i32,
    ) -> i32 {
        loop {
            if self.fade_remaining > 0 {
                let mut fade_end = out_pos.wrapping_add(self.fade_remaining);
                if fade_end > max_out_pos {
                    fade_end = max_out_pos;
                }
                self.fade_remaining = self.fade_remaining.wrapping_add(out_pos);
                if self.sample_rate == 256 && (self.sample_position & 0xFF) == 0 {
                    if self.output.stereo {
                        out_pos = mix_forward_stereo_fading(
                            samples,
                            output,
                            self.sample_position,
                            out_pos,
                            self.left_volume,
                            self.right_volume,
                            self.left_volume_delta,
                            self.right_volume_delta,
                            fade_end,
                            sample_end,
                            self,
                        );
                    } else {
                        out_pos = mix_forward_mono_fading(
                            samples,
                            output,
                            self.sample_position,
                            out_pos,
                            self.current_volume,
                            self.volume_delta,
                            fade_end,
                            sample_end,
                            self,
                        );
                    }
                } else if self.output.stereo {
                    out_pos = mix_forward_stereo_resampled_fading(
                        samples,
                        output,
                        self.sample_position,
                        out_pos,
                        self.left_volume,
                        self.right_volume,
                        self.left_volume_delta,
                        self.right_volume_delta,
                        fade_end,
                        sample_end,
                        self,
                        self.sample_rate,
                        boundary_sample,
                    );
                } else {
                    out_pos = mix_forward_mono_resampled_fading(
                        samples,
                        output,
                        self.sample_position,
                        out_pos,
                        self.current_volume,
                        self.volume_delta,
                        fade_end,
                        sample_end,
                        self,
                        self.sample_rate,
                        boundary_sample,
                    );
                }
                self.fade_remaining = self.fade_remaining.wrapping_sub(out_pos);
                if self.fade_remaining != 0 {
                    return out_pos;
                }
                if !self.apply_volume_transition() {
                    continue;
                }
                return max_out_pos;
            }
            if self.sample_rate == 256 && (self.sample_position & 0xFF) == 0 {
                if self.output.stereo {
                    return mix_forward_stereo(
                        samples,
                        output,
                        self.sample_position,
                        out_pos,
                        self.left_volume,
                        self.right_volume,
                        max_out_pos,
                        sample_end,
                        self,
                    );
                }
                return mix_forward_mono(
                    samples,
                    output,
                    self.sample_position,
                    out_pos,
                    self.current_volume,
                    max_out_pos,
                    sample_end,
                    self,
                );
            }
            if self.output.stereo {
                return mix_forward_stereo_resampled(
                    samples,
                    output,
                    self.sample_position,
                    out_pos,
                    self.left_volume,
                    self.right_volume,
                    max_out_pos,
                    sample_end,
                    self,
                    self.sample_rate,
                    boundary_sample,
                );
            }
            return mix_forward_mono_resampled(
                samples,
                output,
                self.sample_position,
                out_pos,
                self.current_volume,
                max_out_pos,
                sample_end,
                self,
                self.sample_rate,
                boundary_sample,
            );
        }
    }

    /// `setVolume`: immediate, in `create`'s units (stored `<< 6`).
    pub fn set_volume(&mut self, volume: i32) {
        self.set_volume_and_pan(volume << 6, self.get_pan());
    }

    /// `PcmStream.getEffectiveVolume`: the mixer's 0..=255 audibility
    /// estimate used to rank voices.
    pub fn get_effective_volume(&self) -> i32 {
        let mut effective_vol = self.current_volume.wrapping_mul(3) >> 6;
        effective_vol = (effective_vol ^ (effective_vol >> 31))
            .wrapping_add(((effective_vol as u32) >> 31) as i32);
        if self.loop_count == 0 {
            effective_vol = effective_vol.wrapping_sub(
                effective_vol
                    .wrapping_mul(self.sample_position)
                    .wrapping_div(self.sample_length() << 8),
            );
        } else if self.loop_count >= 0 {
            effective_vol = effective_vol.wrapping_sub(
                effective_vol
                    .wrapping_mul(self.start)
                    .wrapping_div(self.sample_length()),
            );
        }
        if effective_vol > 255 {
            255
        } else {
            effective_vol
        }
    }

    /// `PcmStream.getActiveChannelCount`: 0 when silent and not ramping.
    pub fn get_active_channel_count(&self) -> i32 {
        if self.volume == 0 && self.fade_remaining == 0 {
            0
        } else {
            1
        }
    }

    /// `getVolume`: the raw target volume, 0 while fading out to release.
    pub fn get_volume(&self) -> i32 {
        if self.volume == i32::MIN {
            0
        } else {
            self.volume
        }
    }

    /// `skip(int)`: advance `length` frames without producing output.
    pub fn skip(&mut self, mut length: i32) {
        if self.fade_remaining > 0 {
            if length >= self.fade_remaining {
                if self.volume == i32::MIN {
                    self.volume = 0;
                    self.current_volume = 0;
                    self.left_volume = 0;
                    self.right_volume = 0;
                    self.unlink();
                    length = self.fade_remaining;
                }
                self.fade_remaining = 0;
                self.recalculate_channel_volumes();
            } else {
                self.current_volume = self
                    .current_volume
                    .wrapping_add(self.volume_delta.wrapping_mul(length));
                self.left_volume = self
                    .left_volume
                    .wrapping_add(self.left_volume_delta.wrapping_mul(length));
                self.right_volume = self
                    .right_volume
                    .wrapping_add(self.right_volume_delta.wrapping_mul(length));
                self.fade_remaining = self.fade_remaining.wrapping_sub(length);
            }
        }
        let loop_start = self.start << 8;
        let loop_end = self.end << 8;
        let sample_length = self.sample_length() << 8;
        let loop_length = loop_end.wrapping_sub(loop_start);
        if loop_length <= 0 {
            self.loop_count = 0;
        }
        if self.sample_position < 0 {
            if self.sample_rate <= 0 {
                self.cancel_transition();
                self.unlink();
                return;
            }
            self.sample_position = 0;
        }
        if self.sample_position >= sample_length {
            if self.sample_rate >= 0 {
                self.cancel_transition();
                self.unlink();
                return;
            }
            self.sample_position = sample_length.wrapping_sub(1);
        }
        self.sample_position = self
            .sample_position
            .wrapping_add(self.sample_rate.wrapping_mul(length));
        if self.loop_count >= 0 {
            if self.loop_count > 0 {
                if self.ping_pong_loop {
                    'loop_done: {
                        if self.sample_rate < 0 {
                            if self.sample_position >= loop_start {
                                return;
                            }
                            self.reflect(loop_start);
                            self.loop_count = self.loop_count.wrapping_sub(1);
                            if self.loop_count == 0 {
                                break 'loop_done;
                            }
                        }
                        loop {
                            if self.sample_position < loop_end {
                                return;
                            }
                            self.reflect(loop_end);
                            self.loop_count = self.loop_count.wrapping_sub(1);
                            if self.loop_count == 0 {
                                break;
                            }
                            if self.sample_position >= loop_start {
                                return;
                            }
                            self.reflect(loop_start);
                            self.loop_count = self.loop_count.wrapping_sub(1);
                            if self.loop_count == 0 {
                                break;
                            }
                        }
                    }
                } else {
                    'loop_done: {
                        if self.sample_rate < 0 {
                            if self.sample_position >= loop_start {
                                return;
                            }
                            let loop_skips = loop_end
                                .wrapping_sub(self.sample_position)
                                .wrapping_sub(1)
                                .wrapping_div(loop_length);
                            if loop_skips >= self.loop_count {
                                self.sample_position = self
                                    .sample_position
                                    .wrapping_add(loop_length.wrapping_mul(self.loop_count));
                                self.loop_count = 0;
                                break 'loop_done;
                            }
                            self.sample_position = self
                                .sample_position
                                .wrapping_add(loop_length.wrapping_mul(loop_skips));
                            self.loop_count = self.loop_count.wrapping_sub(loop_skips);
                        } else if self.sample_position >= loop_end {
                            let loop_skips = self
                                .sample_position
                                .wrapping_sub(loop_start)
                                .wrapping_div(loop_length);
                            if loop_skips >= self.loop_count {
                                self.sample_position = self
                                    .sample_position
                                    .wrapping_sub(loop_length.wrapping_mul(self.loop_count));
                                self.loop_count = 0;
                                break 'loop_done;
                            }
                            self.sample_position = self
                                .sample_position
                                .wrapping_sub(loop_length.wrapping_mul(loop_skips));
                            self.loop_count = self.loop_count.wrapping_sub(loop_skips);
                        } else {
                            return;
                        }
                        return;
                    }
                }
            }
            if self.sample_rate < 0 {
                if self.sample_position < 0 {
                    self.sample_position = -1;
                    self.cancel_transition();
                    self.unlink();
                }
            } else if self.sample_position >= sample_length {
                self.sample_position = sample_length;
                self.cancel_transition();
                self.unlink();
            }
        } else if self.ping_pong_loop {
            if self.sample_rate < 0 {
                if self.sample_position >= loop_start {
                    return;
                }
                self.reflect(loop_start);
            }
            while self.sample_position >= loop_end {
                self.reflect(loop_end);
                if self.sample_position >= loop_start {
                    return;
                }
                self.reflect(loop_start);
            }
        } else if self.sample_rate < 0 {
            if self.sample_position >= loop_start {
                return;
            }
            self.sample_position = loop_end
                .wrapping_sub(
                    loop_end
                        .wrapping_sub(1)
                        .wrapping_sub(self.sample_position)
                        .wrapping_rem(loop_length),
                )
                .wrapping_sub(1);
        } else if self.sample_position >= loop_end {
            self.sample_position = loop_start.wrapping_add(
                self.sample_position
                    .wrapping_sub(loop_start)
                    .wrapping_rem(loop_length),
            );
        }
    }

    /// `setLoops`: additional passes through the loop; negative is forever.
    pub fn set_loops(&mut self, loops: i32) {
        self.loop_count = loops;
    }

    fn reset_volume(&mut self) {
        self.set_volume_and_pan(0, self.get_pan());
    }

    /// `fadeToVolume`: ramp to the raw `volume` over `duration` frames at the
    /// current pan. (The original names these arguments `volume` and `pan`,
    /// but passes them on as duration and target volume.)
    pub fn fade_to_volume(&mut self, duration: i32, volume: i32) {
        self.fade_to_volume_and_pan(duration, volume, self.get_pan());
    }

    /// `setSamplePosition`: clamped to `-1 ..= samples.len() << 8`.
    pub fn set_sample_position(&mut self, mut position: i32) {
        let max_position = self.sample_length() << 8;
        if position < -1 {
            position = -1;
        }
        if position > max_position {
            position = max_position;
        }
        self.sample_position = position;
    }

    fn read_backward(
        &mut self,
        samples: &[i8],
        output: &mut [i32],
        mut out_pos: i32,
        sample_end: i32,
        max_out_pos: i32,
        boundary_sample: i32,
    ) -> i32 {
        loop {
            if self.fade_remaining > 0 {
                let mut fade_end = out_pos.wrapping_add(self.fade_remaining);
                if fade_end > max_out_pos {
                    fade_end = max_out_pos;
                }
                self.fade_remaining = self.fade_remaining.wrapping_add(out_pos);
                if self.sample_rate == -256 && (self.sample_position & 0xFF) == 0 {
                    if self.output.stereo {
                        out_pos = mix_backward_stereo_fading(
                            samples,
                            output,
                            self.sample_position,
                            out_pos,
                            self.left_volume,
                            self.right_volume,
                            self.left_volume_delta,
                            self.right_volume_delta,
                            fade_end,
                            sample_end,
                            self,
                        );
                    } else {
                        out_pos = mix_backward_mono_fading(
                            samples,
                            output,
                            self.sample_position,
                            out_pos,
                            self.current_volume,
                            self.volume_delta,
                            fade_end,
                            sample_end,
                            self,
                        );
                    }
                } else if self.output.stereo {
                    out_pos = mix_backward_stereo_resampled_fading(
                        samples,
                        output,
                        self.sample_position,
                        out_pos,
                        self.left_volume,
                        self.right_volume,
                        self.left_volume_delta,
                        self.right_volume_delta,
                        fade_end,
                        sample_end,
                        self,
                        self.sample_rate,
                        boundary_sample,
                    );
                } else {
                    out_pos = mix_backward_mono_resampled_fading(
                        samples,
                        output,
                        self.sample_position,
                        out_pos,
                        self.current_volume,
                        self.volume_delta,
                        fade_end,
                        sample_end,
                        self,
                        self.sample_rate,
                        boundary_sample,
                    );
                }
                self.fade_remaining = self.fade_remaining.wrapping_sub(out_pos);
                if self.fade_remaining != 0 {
                    return out_pos;
                }
                if !self.apply_volume_transition() {
                    continue;
                }
                return max_out_pos;
            }
            if self.sample_rate == -256 && (self.sample_position & 0xFF) == 0 {
                if self.output.stereo {
                    return mix_backward_stereo(
                        samples,
                        output,
                        self.sample_position,
                        out_pos,
                        self.left_volume,
                        self.right_volume,
                        max_out_pos,
                        sample_end,
                        self,
                    );
                }
                return mix_backward_mono(
                    samples,
                    output,
                    self.sample_position,
                    out_pos,
                    self.current_volume,
                    max_out_pos,
                    sample_end,
                    self,
                );
            }
            if self.output.stereo {
                return mix_backward_stereo_resampled(
                    samples,
                    output,
                    self.sample_position,
                    out_pos,
                    self.left_volume,
                    self.right_volume,
                    max_out_pos,
                    sample_end,
                    self,
                    self.sample_rate,
                    boundary_sample,
                );
            }
            return mix_backward_mono_resampled(
                samples,
                output,
                self.sample_position,
                out_pos,
                self.current_volume,
                max_out_pos,
                sample_end,
                self,
                self.sample_rate,
                boundary_sample,
            );
        }
    }

    /// `getAbsoluteRate`: the 24.8 rate without the direction.
    pub fn get_absolute_rate(&self) -> i32 {
        if self.sample_rate < 0 {
            self.sample_rate.wrapping_neg()
        } else {
            self.sample_rate
        }
    }

    fn cancel_transition(&mut self) {
        if self.fade_remaining == 0 {
            return;
        }
        if self.volume == i32::MIN {
            self.volume = 0;
        }
        self.fade_remaining = 0;
        self.recalculate_channel_volumes();
    }

    fn set_volume_and_pan(&mut self, volume: i32, pan: i32) {
        self.volume = volume;
        self.pan = pan;
        self.fade_remaining = 0;
        self.recalculate_channel_volumes();
    }

    /// `reverseDirection`: despite the name, always leaves the stream playing
    /// backwards (`-|rate|`).
    pub fn reverse_direction(&mut self) {
        self.sample_rate = (self.sample_rate ^ (self.sample_rate >> 31))
            .wrapping_add(((self.sample_rate as u32) >> 31) as i32);
        self.sample_rate = self.sample_rate.wrapping_neg();
    }

    /// `setRate`: a new 24.8 rate, keeping the current direction.
    pub fn set_rate(&mut self, rate: i32) {
        if self.sample_rate < 0 {
            self.sample_rate = rate.wrapping_neg();
        } else {
            self.sample_rate = rate;
        }
    }

    /// `isOutOfBounds`.
    pub fn is_out_of_bounds(&self) -> bool {
        self.sample_position < 0 || self.sample_position >= self.sample_length() << 8
    }

    /// `isTransitioning`: a volume/pan ramp is in progress.
    pub fn is_transitioning(&self) -> bool {
        self.fade_remaining != 0
    }

    fn recalculate_channel_volumes(&mut self) {
        self.current_volume = self.volume;
        self.left_volume = calculate_left_volume(self.volume, self.pan);
        self.right_volume = calculate_right_volume(self.volume, self.pan);
    }

    /// `fadeToVolumeAndPan`: ramp to a raw volume and pan over at most
    /// `duration` frames; immediate when `duration` is 0.
    pub fn fade_to_volume_and_pan(
        &mut self,
        mut duration: i32,
        target_volume: i32,
        target_pan: i32,
    ) {
        if duration == 0 {
            self.set_volume_and_pan(target_volume, target_pan);
            return;
        }
        let target_left = calculate_left_volume(target_volume, target_pan);
        let target_right = calculate_right_volume(target_volume, target_pan);
        if self.left_volume == target_left && self.right_volume == target_right {
            self.fade_remaining = 0;
            return;
        }
        let mut max_delta = target_volume.wrapping_sub(self.current_volume);
        if self.current_volume.wrapping_sub(target_volume) > max_delta {
            max_delta = self.current_volume.wrapping_sub(target_volume);
        }
        if target_left.wrapping_sub(self.left_volume) > max_delta {
            max_delta = target_left.wrapping_sub(self.left_volume);
        }
        if self.left_volume.wrapping_sub(target_left) > max_delta {
            max_delta = self.left_volume.wrapping_sub(target_left);
        }
        if target_right.wrapping_sub(self.right_volume) > max_delta {
            max_delta = target_right.wrapping_sub(self.right_volume);
        }
        if self.right_volume.wrapping_sub(target_right) > max_delta {
            max_delta = self.right_volume.wrapping_sub(target_right);
        }
        if duration > max_delta {
            duration = max_delta;
        }
        self.fade_remaining = duration;
        self.volume = target_volume;
        self.pan = target_pan;
        self.volume_delta = target_volume
            .wrapping_sub(self.current_volume)
            .wrapping_div(duration);
        self.left_volume_delta = target_left
            .wrapping_sub(self.left_volume)
            .wrapping_div(duration);
        self.right_volume_delta = target_right
            .wrapping_sub(self.right_volume)
            .wrapping_div(duration);
    }

    /// `getPan`: 0 (left) ..= 16384 (right); any negative pan reads as -1.
    pub fn get_pan(&self) -> i32 {
        if self.pan < 0 { -1 } else { self.pan }
    }
}

#[cfg(test)]
mod tests {
    //! Expected values were recorded from the unchanged Java classes.
    use super::*;

    const MONO: Output = Output {
        sample_rate: 22050,
        stereo: false,
    };
    const STEREO: Output = Output {
        sample_rate: 22050,
        stereo: true,
    };
    const EIGHT: [i8; 8] = [12, -90, 127, -128, 33, 77, -5, 64];

    fn linked(stream: Option<SoundPcmStream>) -> SoundPcmStream {
        let mut stream = stream.expect("a playable sound");
        assert!(!stream.is_linked());
        stream.set_linked(true);
        stream
    }

    #[test]
    fn whole_sample_mix_runs_to_the_end_and_unlinks() {
        let sound = Arc::new(PcmSound::new(22050, vec![10, -20, 30, -40, 50], 0, 0));
        let mut stream = linked(SoundPcmStream::create(Arc::clone(&sound), 64, MONO));
        assert_eq!(stream.sample_rate(), 256);
        assert_eq!((stream.get_volume(), stream.get_pan()), (4096, 8192));
        let mut buffer = [0; 8];
        stream.read(&mut buffer, 1, 3);
        assert_eq!(buffer, [0, 163840, -327680, 491520, 0, 0, 0, 0]);
        assert_eq!(stream.sample_position(), 3 << 8);
        assert_eq!(stream.get_effective_volume(), 77);
        assert!(stream.is_linked());
        stream.read(&mut buffer, 4, 4);
        assert_eq!(buffer, [0, 163840, -327680, 491520, -655360, 819200, 0, 0]);
        assert_eq!(stream.sample_position(), 5 << 8);
        assert!(!stream.is_linked() && stream.is_out_of_bounds());

        let mut stream = linked(SoundPcmStream::create(sound, 64, STEREO));
        let mut buffer = [0; 12];
        stream.read(&mut buffer, 0, 6);
        assert_eq!(
            buffer,
            [
                163840, 163840, -327680, -327680, 491520, 491520, -655360, -655360, 819200, 819200,
                0, 0
            ]
        );
        assert!(!stream.is_linked());
    }

    #[test]
    fn create_scales_the_rate_and_rejects_empty_sounds() {
        let output = Output {
            sample_rate: 44100,
            stereo: true,
        };
        let sound = Arc::new(PcmSound::new(22050, EIGHT.to_vec(), 0, 0));
        let stream = SoundPcmStream::create(Arc::clone(&sound), 255, output).unwrap();
        assert_eq!(stream.sample_rate(), 128);
        assert_eq!(stream.get_volume(), 16320);
        assert_eq!(stream.get_effective_volume(), 255);
        assert_eq!(stream.get_active_channel_count(), 1);
        assert_eq!(stream.output(), output);
        assert!(Arc::ptr_eq(stream.sound(), &sound));
        let empty = Arc::new(PcmSound::new(22050, Vec::new(), 0, 0));
        assert!(SoundPcmStream::create(Arc::clone(&empty), 255, output).is_none());
        assert!(SoundPcmStream::create_with_rate(empty, 256, 1, 8192, output).is_none());
    }

    #[test]
    fn endless_loop_interpolates_across_the_loop_seam() {
        let sound = Arc::new(PcmSound::new(22050, EIGHT.to_vec(), 2, 6));
        let mut stream = linked(SoundPcmStream::create_with_rate(
            sound, 181, 5000, 3000, STEREO,
        ));
        assert_eq!((stream.left_volume(), stream.right_volume()), (6391, 3026));
        stream.set_loops(-1);
        let mut buffer = [0; 32];
        stream.read(&mut buffer, 0, 16);
        assert_eq!(
            buffer,
            [
                306768, 145248, -1536836, -727659, -3795, -1797, 2457239, 1163449, -2151770,
                -1018817, -1069594, -506430, 1116028, 528415, 1911308, 904963, 2807246, 1329170,
                878462, 415933, -2982800, -1412291, -72798, -34469, 1388444, 657398, 2213083,
                1047847, 3116810, 1475742, -700314, -331584
            ]
        );
        assert_eq!((stream.sample_position(), stream.loop_count()), (848, -1));
        assert_eq!(stream.get_effective_volume(), 234);
        assert!(stream.is_linked());
    }

    #[test]
    fn ping_pong_with_a_fade_wraps_into_a_full_buffer() {
        let sound = Arc::new(PcmSound::with_ping_pong(22050, EIGHT.to_vec(), 1, 5, true));
        let mut stream = linked(SoundPcmStream::create_with_rate(
            sound, 300, 6000, 8192, MONO,
        ));
        stream.set_loops(3);
        stream.fade_to_volume_and_pan(10, 9000, 12000);
        assert!(stream.is_transitioning());
        let mut buffer = [i32::MAX; 24];
        stream.read(&mut buffer, 0, 24);
        // Three turn-arounds, then backwards off the start of the sound.
        assert_eq!(
            buffer,
            [
                -2147195649,
                2146155528,
                -2146444974,
                2146242078,
                -2146533249,
                2144303998,
                -2143759271,
                2144567647,
                2147193847,
                2147050278,
                2147041522,
                -2146295649,
                2143485256,
                -2144773103,
                2145749740,
                -2147303087,
                i32::MAX,
                i32::MAX,
                i32::MAX,
                i32::MAX,
                i32::MAX,
                i32::MAX,
                i32::MAX,
                i32::MAX
            ]
        );
        assert_eq!(stream.sample_position(), -1);
        assert_eq!((stream.loop_count(), stream.sample_rate()), (0, -300));
        assert_eq!(stream.get_absolute_rate(), 300);
        assert_eq!((stream.get_volume(), stream.get_pan()), (9000, 12000));
        assert_eq!(
            (
                stream.current_volume(),
                stream.left_volume(),
                stream.right_volume()
            ),
            (9000, 6584, 10893)
        );
        assert!(!stream.is_transitioning() && !stream.is_linked());
        // A finished stream adds nothing more.
        let mut buffer = [0; 24];
        stream.read(&mut buffer, 0, 24);
        assert_eq!(buffer, [0; 24]);
    }

    #[test]
    fn backward_fade_out_releases_the_stream() {
        let sound = Arc::new(PcmSound::new(22050, EIGHT.to_vec(), 0, 8));
        let mut stream = linked(SoundPcmStream::create_with_rate(
            sound, 256, 4096, 8192, STEREO,
        ));
        stream.set_sample_position(7 << 8);
        stream.reverse_direction();
        stream.reverse_direction();
        assert_eq!(stream.sample_rate(), -256);
        stream.fade_out_and_release(4);
        assert_eq!(stream.get_volume(), 0);
        assert_eq!(stream.target_volume(), i32::MIN);
        assert_eq!(stream.fade_remaining(), 4);
        assert!(stream.is_linked());
        let mut buffer = [0; 16];
        stream.read(&mut buffer, 0, 8);
        assert_eq!(
            buffer,
            [
                1048576, 1048576, -61440, -61440, 630784, 630784, 135168, 135168, 0, 0, 0, 0, 0, 0,
                0, 0
            ]
        );
        assert_eq!(stream.sample_position(), 3 << 8);
        assert_eq!(stream.get_active_channel_count(), 0);
        assert!(!stream.is_linked());
        // `unlink` on a stream that is not linked changes nothing.
        stream.unlink();
        assert!(!stream.is_linked());
    }

    #[test]
    fn skip_lands_where_read_does() {
        for (rate, loops, ping_pong) in [
            (256, 0, false),
            (256, 2, false),
            (256, -1, true),
            (181, -1, false),
            (300, 3, true),
            (77, 0, false),
        ] {
            for frames in [0usize, 1, 7, 23, 200] {
                let sound = Arc::new(PcmSound::with_ping_pong(
                    22050,
                    EIGHT.to_vec(),
                    2,
                    6,
                    ping_pong,
                ));
                let mut reader = linked(SoundPcmStream::create_with_rate(
                    sound, rate, 4096, 8192, STEREO,
                ));
                reader.set_loops(loops);
                let mut skipper = reader.clone();
                reader.read(&mut vec![0; frames * 2], 0, frames);
                skipper.skip(frames as i32);
                let state = |s: &SoundPcmStream| {
                    (
                        s.sample_position(),
                        s.sample_rate(),
                        s.loop_count(),
                        s.is_linked(),
                    )
                };
                assert_eq!(
                    state(&reader),
                    state(&skipper),
                    "rate {rate} loops {loops} ping-pong {ping_pong} frames {frames}"
                );
            }
        }
    }

    #[test]
    fn resampler_matches_the_original_filter() {
        let up = PcmResampler::new(22050, 44100);
        // Doubling the rate zero-stuffs with a gain of two (and clips).
        let doubled = vec![
            0, 0, 0, 0, 0, 0, 0, 0, 127, 0, 127, 0, -128, 0, -2, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(up.resample(vec![0, 64, 127, -128, -1]), doubled);
        assert_eq!((up.scale_rate(22050), up.scale_position(10)), (44100, 26));
        let mut sound = PcmSound::new(22050, vec![0, 64, 127, -128, -1], 1, 4);
        sound.resample(&up);
        assert_eq!(sound, PcmSound::new(44100, doubled, 8, 14));

        let down = PcmResampler::new(22050, 11025);
        let ramp: Vec<i8> = (0..12).map(|i| (i * 20 - 100) as i8).collect();
        assert_eq!(
            down.resample(ramp.clone()),
            [
                -1, 1, -1, 3, -3, 7, -73, -66, -19, 21, 56, 109, 33, -10, 4, -3, 2, -1, 2, 0
            ]
        );
        // Loop points that collapse onto one sample keep a one-sample loop.
        let mut sound = PcmSound::new(22050, ramp.clone(), 3, 4);
        sound.resample(&down);
        assert_eq!((sound.rate, sound.start, sound.end), (11025, 7, 8));

        let same = PcmResampler::new(22050, 22050);
        assert_eq!(same.resample(ramp.clone()), ramp);
        assert_eq!((same.scale_rate(123), same.scale_position(45)), (123, 45));
    }

    #[test]
    fn channel_volumes_follow_the_square_root_pan_law() {
        assert_eq!(calculate_left_volume(16384, 8192), 16384);
        assert_eq!(calculate_right_volume(16384, 8192), 16384);
        assert_eq!(calculate_left_volume(1000, 0), 1414);
        assert_eq!(calculate_right_volume(1000, 0), 0);
        assert_eq!(calculate_left_volume(1000, 4096), 1225);
        assert_eq!(calculate_right_volume(1000, 4096), 707);
        // A negative pan: full left, phase-inverted right.
        assert_eq!(calculate_left_volume(1000, -1), 1000);
        assert_eq!(calculate_right_volume(1000, -1), -1000);
        // Past full right the left gain is the square root of a negative.
        assert_eq!(calculate_left_volume(1000, 20000), 0);
        assert_eq!(calculate_right_volume(1000, 20000), 1563);
    }
}
