//! The voice limiter in `AudioChannel.read`. Every sixteenth of a second the
//! original marks all streams of a channel inactive and reactivates them in
//! order of loudness until 32 voice channels are in use. A sound that several
//! voices share is admitted more reluctantly with each voice already playing
//! it. What an inactive stream then does is its owner's business: a mixer
//! skips it, while a note keeps reading its own voice.

/// `AudioChannel.maxActiveChannels`.
pub const MAX_ACTIVE_CHANNELS: i32 = 32;

/// One voice below the channel's stream tree, in tree order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Candidate {
    /// Identity of the `Sound` it plays; voices of one sound share a counter.
    pub sound: usize,
    /// `getEffectiveVolume()`.
    pub volume: i32,
    /// `getActiveChannelCount()`.
    pub channels: i32,
}

#[derive(Clone, Copy)]
enum Member {
    /// A stream without a sound above the voices; the number counts the
    /// parents still below it.
    Parent(u32),
    Voice(usize),
}

/// Which voices are active after the pass. `parents` is the number of silent
/// streams between the channel and the voices, the channel's own stream
/// included: 1 for a mixer of voices, 2 for the music sequencer and its note
/// stream. Each has the default effective volume of 255.
pub fn admit(voices: &[Candidate], parents: u32) -> Vec<bool> {
    let mut active = vec![false; voices.len()];
    if parents == 0 {
        return active;
    }
    let mut positions = std::collections::HashMap::<usize, i32>::new();
    let mut buckets: [Vec<(Member, i32)>; 8] = Default::default();
    buckets[255 >> 5].push((Member::Parent(parents - 1), 255));
    let mut channels = 0;
    let mut bucket_mask = 255u32;
    let mut priority = 7i32;
    'fill: while bucket_mask != 0 {
        let (mut bucket, mut threshold) = if priority < 0 {
            ((priority & 3) as usize, -(priority >> 2))
        } else {
            (priority as usize, 0)
        };
        let mut mask = (bucket_mask >> bucket) & 0x1111_1111;
        while mask != 0 {
            if mask & 1 != 0 {
                bucket_mask &= !(1 << bucket);
                let mut at = 0;
                while at < buckets[bucket].len() {
                    let (member, own) = buckets[bucket][at];
                    if let Member::Voice(index) = member {
                        let voice = voices[index];
                        let position = positions.entry(voice.sound).or_default();
                        if *position > threshold {
                            // Heard too often already at this threshold; it
                            // waits for a later pass over this bucket.
                            bucket_mask |= 1 << bucket;
                            at += 1;
                            continue;
                        }
                        active[index] = true;
                        channels += voice.channels;
                        *position += voice.channels;
                    }
                    if channels >= MAX_ACTIVE_CHANNELS {
                        break 'fill;
                    }
                    match member {
                        Member::Parent(0) => {
                            for (index, voice) in voices.iter().enumerate() {
                                let priority = own.wrapping_mul(voice.volume) >> 8;
                                buckets[((priority >> 5) & 7) as usize]
                                    .push((Member::Voice(index), priority));
                            }
                        }
                        Member::Parent(below) => {
                            let priority = (own * 255) >> 8;
                            buckets[(priority >> 5) as usize]
                                .push((Member::Parent(below - 1), priority));
                        }
                        Member::Voice(_) => {}
                    }
                    buckets[bucket].remove(at);
                }
            }
            bucket += 4;
            threshold += 1;
            mask >>= 4;
        }
        priority -= 1;
    }
    active
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voice(sound: usize, volume: i32) -> Candidate {
        Candidate {
            sound,
            volume,
            channels: 1,
        }
    }

    #[test]
    fn everything_plays_while_fewer_than_32_channels_are_wanted() {
        let voices: Vec<_> = (0..31).map(|i| voice(i, 10 + i as i32)).collect();
        assert!(admit(&voices, 1).iter().all(|&active| active));
        assert!(admit(&voices, 2).iter().all(|&active| active));
        assert!(admit(&[], 1).is_empty());
    }

    #[test]
    fn the_loudest_voices_win_and_the_pass_stops_at_the_limit() {
        // 40 distinct sounds; the quiet ones come first in tree order.
        let mut voices: Vec<_> = (0..20).map(|i| voice(i, 40)).collect();
        voices.extend((20..40).map(|i| voice(i, 250)));
        let active = admit(&voices, 1);
        // The 20 loud voices are taken from the top bucket, then quiet ones
        // in tree order until the 32nd channel. The voice that reaches the
        // limit is itself active.
        assert!(active[20..].iter().all(|&a| a));
        assert_eq!(active[..20].iter().filter(|&&a| a).count(), 12);
        assert!(active[..12].iter().all(|&a| a));
    }

    #[test]
    fn repeats_of_one_sound_yield_to_other_sounds_of_the_same_loudness() {
        // 31 voices of sound 7, then two other sounds, all equally loud. In
        // the first pass over the bucket only the first voice of sound 7 is
        // admitted (its counter is then above the threshold 0), then the two
        // others; later passes admit one more repeat each.
        let mut voices: Vec<_> = (0..31).map(|_| voice(7, 200)).collect();
        voices.push(voice(8, 200));
        voices.push(voice(9, 200));
        let active = admit(&voices, 1);
        assert!(active[31] && active[32]);
        // 33 voices want 33 channels; the last repeat of sound 7 is left out.
        assert_eq!(active.iter().filter(|&&a| a).count(), 32);
        assert!(!active[30]);
    }

    #[test]
    fn a_silent_voice_uses_no_channel() {
        let mut voices: Vec<_> = (0..45).map(|i| voice(i, 200)).collect();
        for quiet in voices.iter_mut().take(10) {
            quiet.channels = 0;
        }
        // The ten silent voices are active without counting, so the limit is
        // reached only with the 32nd audible one.
        let active = admit(&voices, 1);
        assert!(active[..42].iter().all(|&a| a));
        assert!(active[42..].iter().all(|&a| !a));
    }
}
