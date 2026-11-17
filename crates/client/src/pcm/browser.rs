//! Commands to the dedicated Rust AudioWorklet engine. No renderer callback
//! mixes audio or controls its clock. Device scheduling remains a host service.
use super::Voice;
use crate::audio::SoundEffect;
use std::sync::Arc;
unsafe extern "C" {
    fn rs09_pcm_open() -> i32;
    fn rs09_pcm_play(
        low: u32,
        high: u32,
        ptr: *const i8,
        len: usize,
        start: usize,
        end: usize,
        loops: i32,
        volume: i32,
    ) -> i32;
    fn rs09_pcm_volume(low: u32, high: u32, volume: i32) -> i32;
    fn rs09_pcm_stop(low: u32, high: u32) -> i32;
    fn rs09_pcm_linked(low: u32, high: u32) -> i32;
    fn rs09_pcm_status(ptr: *mut u8, capacity: usize) -> i32;
    fn rs09_pcm_close();
    fn rs09_pcm_music(ptr: *const i32, blocks: usize) -> i32;
    fn rs09_pcm_music_sent() -> f64;
    fn rs09_pcm_music_played() -> f64;
}
/// [`crate::music::Music::set_feed`] for this device: reads the blocks the
/// audio thread is short of and hands them over. The audio thread adds one
/// to each effect block it plays, as the Windows device thread does.
pub fn feed(device: &mut crate::music::Device) {
    let (sent, played) = unsafe { (rs09_pcm_music_sent(), rs09_pcm_music_played()) };
    if sent < 0.0 || played < 0.0 {
        // The device has failed; nothing can be played.
        return;
    }
    let blocks = super::feed::blocks_due(sent as u64, played as u64, super::feed::LEAD_BLOCKS);
    if blocks == 0 {
        return;
    }
    let samples = super::feed::read(device, blocks);
    unsafe {
        rs09_pcm_music(samples.as_ptr(), blocks);
    }
}
pub struct Handle {
    pub id: u64,
}
impl Handle {
    pub fn linked(&self) -> bool {
        unsafe { rs09_pcm_linked(self.id as u32, (self.id >> 32) as u32) != 0 }
    }
}
pub struct Output {
    next: u64,
    error: Option<String>,
}
impl Output {
    pub fn new(_record: bool) -> Result<Self, String> {
        if unsafe { rs09_pcm_open() } < 0 {
            return Err("AudioWorklet initialization failed".into());
        }
        Ok(Self {
            next: 0,
            error: None,
        })
    }
    pub fn play(&mut self, pcm: Arc<SoundEffect>, loops: i32, volume: i32) -> Option<Handle> {
        if self.error().is_some() || Voice::new(pcm.clone(), loops, volume).is_none() {
            return None;
        }
        self.next = self.next.wrapping_add(1);
        let id = self.next;
        if unsafe {
            rs09_pcm_play(
                id as u32,
                (id >> 32) as u32,
                pcm.samples.as_ptr(),
                pcm.samples.len(),
                pcm.loop_start,
                pcm.loop_end,
                loops,
                volume,
            )
        } < 0
        {
            self.error = Some("AudioWorklet PCM transfer rejected".into());
            return None;
        }
        Some(Handle { id })
    }
    /// The audio thread has its own memory and cannot read the sequencer;
    /// the player hands it blocks instead (see [`feed`]).
    pub fn music(&self, _music: crate::music::Shared) {}
    pub fn volume(&self, id: u64, volume: i32) {
        unsafe {
            rs09_pcm_volume(id as u32, (id >> 32) as u32, volume);
        }
    }
    pub fn stop(&self, id: u64) {
        unsafe {
            rs09_pcm_stop(id as u32, (id >> 32) as u32);
        }
    }
    fn device(&self) -> serde_json::Value {
        let mut bytes = vec![0; 4096];
        let n = unsafe { rs09_pcm_status(bytes.as_mut_ptr(), bytes.len()) };
        if n > 0 && n as usize <= bytes.len() {
            serde_json::from_slice(&bytes[..n as usize]).unwrap_or_default()
        } else {
            serde_json::json!({"error":"AudioWorklet status unavailable"})
        }
    }
    pub fn error(&self) -> Option<String> {
        self.error
            .clone()
            .or_else(|| self.device()["error"].as_str().map(str::to_owned))
    }
    pub fn status(&self) -> serde_json::Value {
        serde_json::json!({"backend":"AudioWorklet22050Hz/stereo/PCM16","device":self.device(),"error":self.error})
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        unsafe {
            rs09_pcm_close();
        }
    }
}
