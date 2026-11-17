//! Music for an audio device that cannot read the sequencer itself.
//!
//! On Windows the device thread reads [`crate::music::Device`] whenever a
//! buffer comes free. A browser's audio thread runs in its own module with
//! its own memory, so the client's thread reads the blocks instead, once per
//! client cycle, and hands them over; the audio thread queues them and adds
//! one to each effect block it plays (`rs09_audio::pcm::add_music`, the same
//! call the Windows thread makes). This is the bookkeeping for that: how many
//! blocks to read so the queue stays a fixed time ahead of playback.
use crate::music::Device;

/// Stereo samples in one block: 256 frames, the original's device block.
pub const BLOCK_SAMPLES: usize = 512;
/// Blocks kept ahead of the audio thread: 16 are 186 ms at 22050 Hz. The
/// client's thread reads them between frames, so the lead has to cover the
/// longest frame; a longer one is heard as a gap. Volume changes and fades
/// reach the ear this much later than on a device that reads for itself.
pub const LEAD_BLOCKS: u32 = 16;

/// Blocks to read now, given how many have been handed over and how many of
/// those the device has played. Never more than the lead: after a long
/// pause (a hidden tab) the queue is refilled, not the missed time replayed.
pub fn blocks_due(sent: u64, played: u64, lead: u32) -> usize {
    let ahead = sent.saturating_sub(played);
    u64::from(lead).saturating_sub(ahead) as usize
}

/// Reads `blocks` blocks of the sequencer's 24-bit mix, one after another.
pub fn read(device: &mut Device, blocks: usize) -> Vec<i32> {
    let mut samples = vec![0i32; blocks * BLOCK_SAMPLES];
    let mut block = [0i32; BLOCK_SAMPLES];
    for out in samples.chunks_exact_mut(BLOCK_SAMPLES) {
        device.read(&mut block);
        out.copy_from_slice(&block);
    }
    samples
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs09_audio::{
        music::{Channel, MidiPcmStream},
        sound_stream::Output,
    };

    #[test]
    fn the_queue_is_kept_one_lead_ahead_of_playback() {
        // Nothing sent yet: fill the lead.
        assert_eq!(blocks_due(0, 0, 16), 16);
        // A 20 ms cycle later the device has played one or two blocks.
        assert_eq!(blocks_due(16, 2, 16), 2);
        assert_eq!(blocks_due(18, 2, 16), 0);
        // After a pause of any length only the lead is read again.
        assert_eq!(blocks_due(18, 18, 16), 16);
        assert_eq!(blocks_due(18, 5000, 16), 16);
        // The device's count can run behind what was sent, never the reverse
        // in a way that asks for a negative number.
        assert_eq!(blocks_due(40, 2, 16), 0);
        assert_eq!(blocks_due(u64::MAX, 0, 16), 0);
    }

    #[test]
    fn blocks_are_read_in_order_from_the_one_device() {
        let mut stream = MidiPcmStream::new(Output {
            sample_rate: 22050,
            stereo: true,
        });
        stream.init();
        let mut device = Device {
            stream,
            channel: Channel::default(),
        };
        // A silent sequencer gives silence, a whole number of blocks of it.
        let samples = read(&mut device, 3);
        assert_eq!(samples.len(), 3 * BLOCK_SAMPLES);
        assert!(samples.iter().all(|&sample| sample == 0));
        assert!(read(&mut device, 0).is_empty());
    }
}
