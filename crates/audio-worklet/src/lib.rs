//! Small Rust audio-thread host. The browser supplies prepared signed8-bit
//! synth PCM and commands; all voices, loops, integer gain and clipping use the
//! same core as the native client. No graphics, networking or game simulation.
use rs09_audio::{
    audio::SoundEffect,
    pcm::{
        add_music,
        presentation::{Handle, Presenter},
    },
};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};

#[derive(Default)]
pub struct Engine {
    input: Vec<u8>,
    output: Vec<i16>,
    presenter: Option<Presenter>,
    handles: BTreeMap<u64, Handle>,
    completed: Vec<u32>,
    music_input: Vec<i32>,
    /// Music blocks handed over and not yet played: the music line's 24-bit
    /// mix, 256 stereo frames each, read by the client between its cycles.
    music: VecDeque<Vec<i32>>,
    music_started: bool,
    music_played: u64,
    music_gaps: u64,
    music_nonzero: u64,
}
impl Engine {
    /// Platform-transfer bound; valid prepared cache sounds are far smaller.
    pub const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
    /// Stereo samples in a block.
    pub const BLOCK_SAMPLES: usize = 512;
    /// Music blocks held at most (about three quarters of a second); the
    /// client keeps far fewer ahead.
    pub const MAX_MUSIC_BLOCKS: usize = 64;
    /// Room for `blocks` music blocks to be copied into before
    /// [`Engine::music_push`].
    pub fn music_input(&mut self, blocks: usize) -> Option<&mut [i32]> {
        if blocks == 0 || blocks > Self::MAX_MUSIC_BLOCKS {
            return None;
        }
        self.music_input.resize(blocks * Self::BLOCK_SAMPLES, 0);
        Some(&mut self.music_input)
    }
    /// Queues the first `blocks` blocks of the input area; returns how many
    /// were taken (fewer when the queue is full).
    pub fn music_push(&mut self, blocks: usize) -> usize {
        let offered = blocks.min(self.music_input.len() / Self::BLOCK_SAMPLES);
        let taken = offered.min(Self::MAX_MUSIC_BLOCKS.saturating_sub(self.music.len()));
        for block in self
            .music_input
            .chunks_exact(Self::BLOCK_SAMPLES)
            .take(taken)
        {
            self.music.push_back(block.to_vec());
        }
        self.music_started |= taken != 0;
        taken
    }
    pub fn music_queued(&self) -> usize {
        self.music.len()
    }
    /// Music blocks played since the start.
    pub fn music_played(&self) -> u64 {
        self.music_played
    }
    /// Blocks played without music because none had been handed over.
    pub fn music_gaps(&self) -> u64 {
        self.music_gaps
    }
    /// Music samples played that were not zero after clipping.
    pub fn music_nonzero(&self) -> u64 {
        self.music_nonzero
    }
    pub fn input(&mut self, len: usize) -> Option<&mut [u8]> {
        if len == 0 || len > Self::MAX_SOURCE_BYTES {
            return None;
        }
        self.input.resize(len, 0);
        Some(&mut self.input)
    }
    pub fn play(
        &mut self,
        id: u64,
        len: usize,
        start: usize,
        end: usize,
        loops: i32,
        volume: i32,
    ) -> bool {
        if len == 0 || len > self.input.len() || len > Self::MAX_SOURCE_BYTES {
            return false;
        }
        let pcm = Arc::new(SoundEffect {
            samples: self.input[..len].iter().map(|&s| s as i8).collect(),
            loop_start: start,
            loop_end: end,
        });
        let p = self.presenter.get_or_insert_with(|| Presenter::new(false));
        if let Some(h) = p.play_identified(id, pcm, loops, volume) {
            self.handles.insert(id, h);
            true
        } else {
            false
        }
    }
    pub fn volume(&mut self, id: u64, volume: i32) {
        if let Some(p) = &mut self.presenter {
            p.volume(id, volume);
        }
    }
    pub fn stop(&mut self, id: u64) {
        if let Some(p) = &mut self.presenter {
            p.stop(id);
        }
        self.handles.remove(&id);
    }
    pub fn started(&self) -> bool {
        self.presenter.is_some() || self.music_started
    }
    pub fn frames(&self) -> u64 {
        self.presenter
            .as_ref()
            .map_or(0, Presenter::rendered_frames)
    }
    pub fn active(&self) -> usize {
        self.presenter.as_ref().map_or(0, Presenter::active_voices)
    }
    pub fn render(&mut self) -> &[i16] {
        self.completed.clear();
        if let Some(p) = &mut self.presenter {
            self.output = p.read_block();
            self.handles.retain(|&id, h| {
                if h.linked() {
                    true
                } else {
                    self.completed.extend([id as u32, (id >> 32) as u32]);
                    false
                }
            });
        } else {
            self.output.clear();
            self.output.resize(Self::BLOCK_SAMPLES, 0);
        }
        // The music line joins after the effect line has been clipped.
        if let Some(block) = self.music.pop_front() {
            self.music_nonzero += block
                .iter()
                .filter(|&&sample| rs09_audio::pcm::clip(sample) != 0)
                .count() as u64;
            add_music(&mut self.output, &block);
            self.music_played += 1;
        } else if self.music_started {
            self.music_gaps += 1;
        }
        &self.output
    }
    pub fn completed(&self) -> &[u32] {
        &self.completed
    }
}

#[cfg(target_arch = "wasm32")]
mod abi {
    use super::Engine;
    use std::cell::RefCell;
    thread_local! { static ENGINE:RefCell<Engine>=RefCell::new(Engine::default()); }
    fn id(low: u32, high: u32) -> u64 {
        u64::from(low) | (u64::from(high) << 32)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn rs09_audio_input(len: u32) -> u32 {
        ENGINE.with(|e| {
            e.borrow_mut()
                .input(len as usize)
                .map_or(0, |b| b.as_mut_ptr() as u32)
        })
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn rs09_audio_play(
        low: u32,
        high: u32,
        len: u32,
        start: u32,
        end: u32,
        loops: i32,
        volume: i32,
    ) -> i32 {
        ENGINE.with(|e| {
            i32::from(e.borrow_mut().play(
                id(low, high),
                len as usize,
                start as usize,
                end as usize,
                loops,
                volume,
            ))
        })
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn rs09_audio_volume(low: u32, high: u32, volume: i32) {
        ENGINE.with(|e| e.borrow_mut().volume(id(low, high), volume));
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn rs09_audio_stop(low: u32, high: u32) {
        ENGINE.with(|e| e.borrow_mut().stop(id(low, high)));
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn rs09_audio_started() -> i32 {
        ENGINE.with(|e| i32::from(e.borrow().started()))
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn rs09_audio_read() -> u32 {
        ENGINE.with(|e| e.borrow_mut().render().as_ptr() as u32)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn rs09_audio_frames() -> f64 {
        ENGINE.with(|e| e.borrow().frames() as f64)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn rs09_audio_active() -> u32 {
        ENGINE.with(|e| e.borrow().active() as u32)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn rs09_audio_completed() -> u32 {
        ENGINE.with(|e| e.borrow().completed().as_ptr() as u32)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn rs09_audio_completed_count() -> u32 {
        ENGINE.with(|e| e.borrow().completed().len() as u32 / 2)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn rs09_audio_music_input(blocks: u32) -> u32 {
        ENGINE.with(|e| {
            e.borrow_mut()
                .music_input(blocks as usize)
                .map_or(0, |b| b.as_mut_ptr() as u32)
        })
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn rs09_audio_music_push(blocks: u32) -> u32 {
        ENGINE.with(|e| e.borrow_mut().music_push(blocks as usize) as u32)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn rs09_audio_music_queued() -> u32 {
        ENGINE.with(|e| e.borrow().music_queued() as u32)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn rs09_audio_music_played() -> f64 {
        ENGINE.with(|e| e.borrow().music_played() as f64)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn rs09_audio_music_gaps() -> f64 {
        ENGINE.with(|e| e.borrow().music_gaps() as f64)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn rs09_audio_music_nonzero() -> f64 {
        ENGINE.with(|e| e.borrow().music_nonzero() as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::Engine;
    #[test]
    fn audio_thread_handles_keep_ids_and_complete_without_render_callbacks() {
        let mut e = Engine::default();
        assert!(!e.started());
        assert_eq!(e.frames(), 0);
        assert!(e.render().iter().all(|&s| s == 0));
        assert_eq!(e.frames(), 0);
        e.input(2).unwrap().copy_from_slice(&[1, 255]);
        let id = (1u64 << 40) | 19;
        assert!(e.play(id, 2, 0, 0, 0, 127));
        assert!(!e.play(id, 2, 0, 0, 0, 127));
        e.render();
        assert_eq!(e.frames(), 256);
        assert_eq!(e.completed(), &[19, 256]);
        assert_eq!(e.active(), 0);
        // Late commands remain harmless after completion, including IDs with
        // nonzero high words; game/render callbacks are not needed to expire.
        e.volume(id, 23);
        e.stop(id);
        e.render();
        assert_eq!(e.frames(), 512);
        assert!(e.completed().is_empty());
    }
    #[test]
    fn music_blocks_play_in_order_under_the_effects_and_gaps_are_silent() {
        let mut e = Engine::default();
        assert!(e.music_input(0).is_none());
        assert!(e.music_input(Engine::MAX_MUSIC_BLOCKS + 1).is_none());
        // Two blocks: the first at 24-bit value 256 * 7, the second past the
        // 24-bit range, which the music line clips on its own.
        let input = e.music_input(2).unwrap();
        input[..512].fill(256 * 7);
        input[512..].fill(i32::MAX);
        assert_eq!(e.music_push(2), 2);
        assert!(e.started(), "music alone starts the output");
        assert_eq!(e.music_queued(), 2);
        assert!(e.render().iter().all(|&s| s == 7));
        assert!(e.render().iter().all(|&s| s == 32_767));
        assert_eq!((e.music_played(), e.music_gaps()), (2, 0));
        assert_eq!(e.music_nonzero(), 1024);
        // Nothing handed over in time: silence, counted, and no stale block.
        assert!(e.render().iter().all(|&s| s == 0));
        assert_eq!((e.music_played(), e.music_gaps()), (2, 1));
        // An effect and music together: each clipped, then added.
        e.input(1).unwrap()[0] = 100;
        assert!(e.play(1, 1, 0, 1, -1, 127));
        let effect = e.render()[0];
        assert!(effect > 0);
        e.music_input(1).unwrap().fill(256 * 5);
        assert_eq!(e.music_push(1), 1);
        assert!(e.render().iter().all(|&s| s == effect + 5));
        // The queue is bounded; what does not fit is refused, not dropped
        // from the front.
        e.music_input(Engine::MAX_MUSIC_BLOCKS).unwrap().fill(256);
        assert_eq!(
            e.music_push(Engine::MAX_MUSIC_BLOCKS),
            Engine::MAX_MUSIC_BLOCKS
        );
        assert_eq!(e.music_push(3), 0);
        assert_eq!(e.music_queued(), Engine::MAX_MUSIC_BLOCKS);
    }
    #[test]
    fn audio_thread_transfer_bounds_and_stop_leave_silence() {
        let mut e = Engine::default();
        assert!(e.input(0).is_none());
        assert!(e.input(Engine::MAX_SOURCE_BYTES + 1).is_none());
        assert!(!e.play(1, 1, 0, 1, -1, 127));
        e.input(1).unwrap()[0] = 17;
        assert!(e.play(0, 1, 0, 0, 0, 127));
        e.render();
        assert_eq!(e.completed(), &[0, 0]);
        assert!(!e.play(1, 2, 0, 1, -1, 127));
        assert!(e.play(1, 1, 0, 1, -1, 127));
        assert!(e.render().iter().any(|&s| s != 0));
        assert_eq!(e.active(), 1);
        e.volume(1, 0);
        assert!(e.render().iter().all(|&s| s == 0));
        e.stop(1);
        assert_eq!(e.active(), 0);
        assert!(e.render().iter().all(|&s| s == 0));
        assert!(e.completed().is_empty());
    }
}
