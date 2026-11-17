//! Serial PCM presentation state. The browser supplies its device clock; all
//! voices, loops, gain, clipping and capture samples remain in Rust.
use super::{Mixer, Voice};
use crate::audio::SoundEffect;
use std::{cell::Cell, collections::BTreeMap, rc::Rc, sync::Arc};

pub struct Handle {
    pub id: u64,
    linked: Rc<Cell<bool>>,
}
impl Handle {
    pub fn linked(&self) -> bool {
        self.linked.get()
    }
}
enum Recorded {
    Play(u64, u64, Arc<SoundEffect>, i32, i32),
    Volume(u64, u64, i32),
    Stop(u64, u64),
}
pub struct Presenter {
    mixer: Mixer,
    flags: BTreeMap<u64, Rc<Cell<bool>>>,
    next: u64,
    rendered: u64,
    buffers: usize,
    nonzero: usize,
    record: bool,
    pcm: Vec<i16>,
    commands: Vec<Recorded>,
    recorded_source_bytes: usize,
    capture_complete: bool,
}
impl Presenter {
    pub fn new(record: bool) -> Self {
        Self {
            mixer: Mixer::default(),
            flags: BTreeMap::new(),
            next: 0,
            rendered: 0,
            buffers: 0,
            nonzero: 0,
            record,
            pcm: vec![],
            commands: vec![],
            recorded_source_bytes: 0,
            capture_complete: true,
        }
    }
    fn recording(&mut self, source_bytes: usize) -> bool {
        if !self.record || !self.capture_complete {
            return false;
        }
        if self.rendered >= 22050 * 30
            || self.commands.len() >= 8192
            || self.recorded_source_bytes.saturating_add(source_bytes) > 4 * 1024 * 1024
        {
            self.capture_complete = false;
            return false;
        }
        self.recorded_source_bytes += source_bytes;
        true
    }
    pub fn play(&mut self, pcm: Arc<SoundEffect>, loops: i32, volume: i32) -> Option<Handle> {
        let voice = Voice::new(pcm.clone(), loops, volume)?;
        self.next = self.next.wrapping_add(1);
        let id = self.next;
        Some(self.install(id, pcm, voice, loops, volume))
    }
    /// Cross-thread hosts allocate IDs and retain them after completion
    /// so late volume/stop commands keep their original identity.
    pub fn play_identified(
        &mut self,
        id: u64,
        pcm: Arc<SoundEffect>,
        loops: i32,
        volume: i32,
    ) -> Option<Handle> {
        if self.flags.contains_key(&id) {
            return None;
        }
        let voice = Voice::new(pcm.clone(), loops, volume)?;
        self.next = self.next.max(id);
        Some(self.install(id, pcm, voice, loops, volume))
    }
    fn install(
        &mut self,
        id: u64,
        pcm: Arc<SoundEffect>,
        voice: Voice,
        loops: i32,
        volume: i32,
    ) -> Handle {
        if self.recording(pcm.samples.len()) {
            self.commands
                .push(Recorded::Play(self.rendered, id, pcm, loops, volume));
        }
        let linked = Rc::new(Cell::new(true));
        self.flags.insert(id, linked.clone());
        self.mixer.voices.insert(id, voice);
        Handle { id, linked }
    }
    pub fn rendered_frames(&self) -> u64 {
        self.rendered
    }
    pub fn active_voices(&self) -> usize {
        self.mixer.voices.len()
    }
    pub fn volume(&mut self, id: u64, volume: i32) {
        if self.recording(0) {
            self.commands
                .push(Recorded::Volume(self.rendered, id, volume));
        }
        if let Some(voice) = self.mixer.voices.get_mut(&id) {
            voice.volume = volume;
        }
    }
    pub fn stop(&mut self, id: u64) {
        if self.recording(0) {
            self.commands.push(Recorded::Stop(self.rendered, id));
        }
        self.mixer.voices.remove(&id);
        if let Some(flag) = self.flags.remove(&id) {
            flag.set(false);
        }
    }
    pub fn read_block(&mut self) -> Vec<i16> {
        let pcm = self.mixer.read(256);
        self.buffers += 1;
        self.nonzero += pcm.iter().filter(|&&s| s != 0).count();
        if self.record && self.capture_complete {
            if self.rendered + 256 <= 22050 * 30 {
                self.pcm.extend_from_slice(&pcm);
            } else {
                self.capture_complete = false;
            }
        }
        self.rendered += 256;
        self.mixer.voices.retain(|id, voice| {
            if voice.linked {
                true
            } else {
                if let Some(flag) = self.flags.remove(id) {
                    flag.set(false);
                }
                false
            }
        });
        pcm
    }
    pub fn status(&self) -> serde_json::Value {
        serde_json::json!({"rendered_frames":self.rendered,"buffers_submitted":self.buffers,
            "nonzero_samples":self.nonzero,"active_voices":self.mixer.voices.len(),
            "recording":self.record,"capture_complete":self.capture_complete,
            "recorded_frames":self.pcm.len()/2})
    }
    pub fn capture(&self) -> serde_json::Value {
        let commands = self.commands.iter().map(|c|match c {
            Recorded::Play(frame,id,pcm,loops,volume) => serde_json::json!({"op":"play","frame":frame,"id":id,"samples":pcm.samples,"start":pcm.loop_start,"end":pcm.loop_end,"loops":loops,"volume":volume}),
            Recorded::Volume(frame,id,volume) => serde_json::json!({"op":"volume","frame":frame,"id":id,"volume":volume}),
            Recorded::Stop(frame,id) => serde_json::json!({"op":"stop","frame":frame,"id":id}),
        }).collect::<Vec<_>>();
        serde_json::json!({"complete":self.record && self.capture_complete,
            "commands":{"frames":self.pcm.len()/2,"rate":22050,"channels":2,"commands":commands},
            "pcm":self.pcm})
    }
}
impl Drop for Presenter {
    fn drop(&mut self) {
        for flag in self.flags.values() {
            flag.set(false);
        }
    }
}
