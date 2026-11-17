//! Original forward, integer-rate synthesized PCM stream and signed24-bit mix,
//! with `AudioChannel`'s voice limiter over the mixer. Pitch, fades and
//! ping-pong loops are in `sound_stream`; music uses those.
use crate::{
    audio::SoundEffect,
    limiter::{self, Candidate},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

pub mod presentation;

pub struct Voice {
    pub pcm: Arc<SoundEffect>,
    pub position: usize,
    pub loops: i32,
    pub volume: i32,
    pub linked: bool,
}
impl Voice {
    /// SynthSound creates22050Hz, non-ping-pong PCM; this is SoundPcmStream's
    /// rate256/center-pan path. A loop count denotes additional repeats.
    pub fn new(pcm: Arc<SoundEffect>, loops: i32, volume: i32) -> Option<Self> {
        if pcm.samples.is_empty() {
            return None;
        }
        if loops != 0 && pcm.loop_end > pcm.loop_start && pcm.loop_end > pcm.samples.len() {
            return None;
        }
        Some(Self {
            pcm,
            position: 0,
            loops,
            volume,
            linked: true,
        })
    }
    pub fn read(&mut self, out: &mut [i32], offset: usize, frames: usize, stereo: bool) {
        if !self.linked {
            return;
        }
        let channels = if stereo { 2 } else { 1 };
        assert!((offset + frames) * channels <= out.len());
        let (start, end) = (self.pcm.loop_start, self.pcm.loop_end);
        if end <= start {
            self.loops = 0;
        }
        for frame in 0..frames {
            if self.position >= self.pcm.samples.len() {
                self.linked = false;
                break;
            }
            let sample = i32::from(self.pcm.samples[self.position])
                .wrapping_mul(self.volume.wrapping_shl(8));
            for ch in 0..channels {
                let i = (offset + frame) * channels + ch;
                out[i] = out[i].wrapping_add(sample);
            }
            self.position += 1;
            if self.loops != 0 && self.position >= end && end > start {
                self.position = start + (self.position - start) % (end - start);
                if self.loops > 0 {
                    self.loops -= 1;
                }
            }
            if self.position >= self.pcm.samples.len() {
                self.linked = false;
                break;
            }
        }
    }
    /// `SoundPcmStream.skip` for this path: the position, loops and end of
    /// the sound advance exactly as in `read`, without output.
    pub fn skip(&mut self, frames: usize) {
        self.read(&mut vec![0; frames], 0, frames, false);
    }
    /// `getEffectiveVolume` and `getActiveChannelCount` of the centre-pan
    /// stream this voice stands for. `sound` identifies its `PcmSound`.
    pub fn candidate(&self, sound: usize) -> Candidate {
        let mut volume = self.volume.wrapping_shl(6).wrapping_mul(3) >> 6;
        volume = (volume ^ (volume >> 31)).wrapping_add(((volume as u32) >> 31) as i32);
        let length = self.pcm.samples.len() as i32;
        if self.loops == 0 {
            volume -= volume.wrapping_mul((self.position as i32).wrapping_shl(8))
                / length.wrapping_shl(8);
        } else if self.loops >= 0 {
            volume -= volume.wrapping_mul(self.pcm.loop_start as i32) / length;
        }
        Candidate {
            sound,
            volume: volume.min(255),
            channels: i32::from(self.volume != 0),
        }
    }
}

/// The sound-effect channel: `MixerPcmStream` under an `AudioChannel`.
#[derive(Default)]
pub struct Mixer {
    pub voices: BTreeMap<u64, Voice>,
    /// `AudioChannel.priorityUpdateCountdown`.
    countdown: i32,
    /// Voices the last limiter pass left inactive. A voice added since is
    /// active, as a new stream is in the original.
    inactive: BTreeSet<u64>,
}
impl Mixer {
    /// `AudioChannel.read` for one block (the original's is 256 frames): the
    /// limiter pass when it is due, then every active voice mixed and every
    /// inactive one skipped.
    pub fn read(&mut self, frames: usize) -> Vec<i16> {
        self.countdown -= frames as i32;
        if self.countdown <= 0 {
            self.countdown += 22050 >> 4;
            self.limit();
        }
        self.countdown = self.countdown.max(0);
        let mut out = vec![0i32; frames * 2];
        for (id, voice) in &mut self.voices {
            if self.inactive.contains(id) {
                voice.skip(frames);
            } else {
                voice.read(&mut out, 0, frames, true);
            }
        }
        out.into_iter().map(clip).collect()
    }
    /// The original adds each new stream at the head of the mixer's list, so
    /// tree order is newest first. Every play makes its own `PcmSound`, so no
    /// two voices share a sound.
    fn limit(&mut self) {
        let ids: Vec<u64> = self.voices.keys().rev().copied().collect();
        let voices: Vec<Candidate> = ids
            .iter()
            .enumerate()
            .map(|(index, id)| self.voices[id].candidate(index))
            .collect();
        self.inactive = ids
            .into_iter()
            .zip(limiter::admit(&voices, 1))
            .filter_map(|(id, active)| (!active).then_some(id))
            .collect();
    }
}
/// JavaAudioChannel.write's signed24-bit saturation and little-endian16 output.
pub fn clip(sample: i32) -> i16 {
    (sample.clamp(-8388608, 8388607) >> 8) as i16
}

/// The original opens one device line for sound effects and one for music;
/// each line clips its own 24-bit mix and the device adds the two. Here one
/// output plays both: `effects` is the effect line's block, already clipped,
/// and `music` the music line's 24-bit mix for the same frames.
pub fn add_music(effects: &mut [i16], music: &[i32]) {
    for (out, &sample) in effects.iter_mut().zip(music) {
        *out = out.saturating_add(clip(sample));
    }
}

pub fn wav(samples: &[i16]) -> Vec<u8> {
    let bytes = u32::try_from(samples.len() * 2).expect("PCM WAV exceeds4GiB");
    let mut out = Vec::with_capacity(44 + bytes as usize);
    out.extend(b"RIFF");
    out.extend((bytes + 36).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(2u16.to_le_bytes());
    out.extend(22050u32.to_le_bytes());
    out.extend(88200u32.to_le_bytes());
    out.extend(4u16.to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend(bytes.to_le_bytes());
    for sample in samples {
        out.extend(sample.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sound_stream::{Output, PcmSound, SoundPcmStream};

    fn effect(length: usize, start: usize, end: usize) -> Arc<SoundEffect> {
        Arc::new(SoundEffect {
            samples: (0..length).map(|i| (i * 7 % 200) as i8).collect(),
            loop_start: start,
            loop_end: end,
        })
    }

    /// The limiter's inputs for a voice equal those of the full stream port
    /// (itself compared with the original) at every position.
    #[test]
    fn a_voice_reports_the_stream_ports_effective_volume() {
        let output = Output {
            sample_rate: 22050,
            stereo: true,
        };
        let mut seed = 0x2545_f491u32;
        let mut next = |bound: u32| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed % bound
        };
        let mut compared = 0;
        for _ in 0..400 {
            let length = 30 + next(900) as usize;
            let start = next(length as u32 - 1) as usize;
            let end = start + 1 + next((length - start - 1) as u32) as usize;
            let loops = [0, 0, -1, 1, 4][next(5) as usize];
            let volume = [0, 1, 64, 127, 200, 255][next(6) as usize];
            let pcm = effect(length, start, end);
            let mut voice = Voice::new(pcm.clone(), loops, volume).unwrap();
            let sound = Arc::new(PcmSound::new(
                22050,
                pcm.samples.clone(),
                start as i32,
                end as i32,
            ));
            let mut stream = SoundPcmStream::create(sound, volume, output).unwrap();
            stream.set_loops(loops);
            stream.set_linked(true);
            for _ in 0..6 {
                let got = voice.candidate(9);
                assert_eq!(
                    [got.volume, got.channels],
                    [
                        stream.get_effective_volume(),
                        stream.get_active_channel_count()
                    ],
                    "length {length} loop {start}..{end} loops {loops} volume {volume}"
                );
                compared += 1;
                if !voice.linked || !stream.is_linked() {
                    break;
                }
                let frames = 1 + next(200) as usize;
                voice.skip(frames);
                stream.skip(frames as i32);
                assert_eq!(voice.linked, stream.is_linked());
            }
        }
        assert!(compared > 1500, "{compared}");
    }

    #[test]
    fn music_is_clipped_on_its_own_line_before_it_joins_the_effects() {
        let mut block = [100i16, -100, 32_000, -32_000, 0, 0];
        add_music(
            &mut block,
            &[
                256 * 50,
                -256 * 50,
                256 * 1000,
                -256 * 1000,
                i32::MAX,
                i32::MIN,
            ],
        );
        // The 24-bit music mix saturates first, then the two lines add and
        // saturate again, as two device lines would.
        assert_eq!(block, [150, -150, 32_767, -32_768, 32_767, -32_768]);
        // A shorter music block leaves the rest of the effects untouched.
        let mut block = [5i16; 4];
        add_music(&mut block, &[256, 256]);
        assert_eq!(block, [6, 6, 5, 5]);
    }

    #[test]
    fn the_mixer_skips_voices_the_limiter_turns_away() {
        let mut mixer = Mixer::default();
        // 40 one-second sounds: ids 1..=8 loud, the rest quiet. (Effective
        // volume is three times the volume, capped at 255, so 20 is quiet
        // and anything from 85 up is as loud as it gets.) The newest voices
        // (highest id) come first in the original's stream order.
        for id in 1..=40u64 {
            let volume = if id <= 8 { 255 } else { 20 };
            mixer
                .voices
                .insert(id, Voice::new(effect(22050, 0, 0), 0, volume).unwrap());
        }
        let block = mixer.read(256);
        assert_eq!(block.len(), 512);
        // 32 channels: the eight loud voices and the 24 newest quiet ones.
        let expected: BTreeSet<u64> = (9..=16).collect();
        assert_eq!(mixer.inactive, expected);
        // Skipped voices still advance, so they end when they would have.
        assert!(mixer.voices.values().all(|voice| voice.position == 256));
        // A voice started between passes plays at once.
        mixer
            .voices
            .insert(41, Voice::new(effect(22050, 0, 0), 0, 50).unwrap());
        mixer.read(256);
        assert!(!mixer.inactive.contains(&41));
        // Fewer than 32 voices are never limited.
        let mut small = Mixer::default();
        for id in 1..=20u64 {
            small
                .voices
                .insert(id, Voice::new(effect(500, 0, 0), 0, 200).unwrap());
        }
        small.read(256);
        assert!(small.inactive.is_empty());
    }
}
