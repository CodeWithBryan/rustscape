//! Shared UI/world sound queue with timestamped logical-cycle inputs.
#[cfg(not(any(target_os = "windows", target_arch = "wasm32")))]
use macroquad::audio::{PlaySoundParams, Sound, load_sound_from_bytes, play_sound};
use rs09_client::{animation::Sequence, audio::SoundEffect, pack::Pack, sound_queue::Queue};
use std::collections::{BTreeMap, BTreeSet};
#[cfg(any(target_os = "windows", target_arch = "wasm32"))]
use std::sync::Arc;

#[cfg(any(target_os = "windows", target_arch = "wasm32"))]
struct Prepared {
    pcm: Arc<SoundEffect>,
}
#[cfg(not(any(target_os = "windows", target_arch = "wasm32")))]
type Prepared = Sound;
enum Request {
    Effect([i32; 3]),
    Sequence {
        sounds: Vec<u32>,
        fine: [i32; 2],
        is_self: bool,
    },
    /// SOUND_AREA: scene-local source and listener tiles, then id, radius,
    /// loops and delay.
    Area([i32; 2], [i32; 2], [i32; 4]),
}
#[derive(Default)]
pub struct Effects {
    sounds: BTreeMap<[i32; 2], Prepared>,
    starts: BTreeMap<i32, i32>,
    failed: BTreeSet<[i32; 2]>,
    pending: Vec<(i32, Request)>,
    queue: Queue,
    cycle: Option<i32>,
    pub submitted: usize,
    pub sequence_triggers: usize,
    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
    output: Option<rs09_client::pcm::Output>,
    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
    record_pcm: bool,
    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
    output_failed: bool,
    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
    output_errors: Vec<String>,
}
impl Effects {
    pub fn status(&self) -> serde_json::Value {
        #[cfg(target_arch = "wasm32")]
        {
            self.output.as_ref().map_or_else(
                || serde_json::json!({"backend":null,"output_failed":self.output_failed}),
                |o| o.status(),
            )
        }
        #[cfg(target_os = "windows")]
        {
            serde_json::json!({"backend":self.output.as_ref().map(|_|"WinMM22050Hz/stereo/PCM16"),"output_failed":self.output_failed,"error":self.output.as_ref().and_then(|o|o.error())})
        }
        #[cfg(not(any(target_os = "windows", target_arch = "wasm32")))]
        {
            serde_json::json!({"backend":"macroquad finite sounds"})
        }
    }
    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
    pub fn record_pcm(&mut self, record: bool) {
        self.record_pcm = record;
    }
    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
    pub fn start_pcm(
        &mut self,
        pcm: Arc<SoundEffect>,
        loops: i32,
        volume: i32,
    ) -> Option<rs09_client::pcm::Handle> {
        if self.output_failed {
            return None;
        }
        if self.output.is_none() {
            match rs09_client::pcm::Output::new(self.record_pcm) {
                Ok(output) => self.output = Some(output),
                Err(error) => {
                    self.output_failed = true;
                    self.output_errors.push(error);
                    return None;
                }
            }
        }
        self.output.as_mut()?.play(pcm, loops, volume)
    }
    /// Open the device if needed and have it play the music sequencer.
    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
    pub fn music(&mut self, music: rs09_client::music::Shared) -> Result<(), String> {
        if self.output.is_none() && !self.output_failed {
            match rs09_client::pcm::Output::new(self.record_pcm) {
                Ok(output) => self.output = Some(output),
                Err(error) => {
                    self.output_failed = true;
                    self.output_errors.push(error);
                }
            }
        }
        match &self.output {
            Some(output) => {
                output.music(music);
                Ok(())
            }
            None => Err(self
                .output_errors
                .last()
                .cloned()
                .unwrap_or_else(|| "PCM output unavailable".into())),
        }
    }
    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
    pub fn pcm_volume(&self, id: u64, volume: i32) {
        if let Some(output) = &self.output {
            output.volume(id, volume);
        }
    }
    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
    pub fn pcm_stop(&self, id: u64) {
        if let Some(output) = &self.output {
            output.stop(id);
        }
    }
    #[cfg(target_os = "windows")]
    pub fn capture_pcm(&self, path: &str) -> Result<serde_json::Value, String> {
        if let Some(output) = &self.output {
            let capture = output.snapshot();
            std::fs::write(path, rs09_client::pcm::wav(&capture.pcm)).map_err(|e| e.to_string())?;
            let commands = std::path::Path::new(path).with_file_name("audio-commands.json");
            std::fs::write(commands, capture.command_json().to_string())
                .map_err(|e| e.to_string())?;
            Ok(
                serde_json::json!({"backend":"WinMM22050Hz/stereo/PCM16","buffers_submitted":capture.buffers,"nonzero_samples":capture.nonzero,"recorded_frames":capture.pcm.len()/2,"error":capture.error}),
            )
        } else {
            Ok(
                serde_json::json!({"backend":null,"buffers_submitted":0,"recorded_frames":0,"output_failed":self.output_failed}),
            )
        }
    }
    /// `SoundPlayer.size = 0` (client.resetGameSessionState): forget every
    /// sound that is queued but not yet playing.
    pub fn reset_queue(&mut self) {
        self.pending.clear();
        self.queue = Queue::default();
    }
    pub fn enqueue(&mut self, requests: &mut Vec<[i32; 3]>, cycle: i32) {
        for request in requests.drain(..) {
            self.pending.push((cycle, Request::Effect(request)));
        }
    }
    /// `SoundPlayer.size = 0` (`LoginManager.loadRegion`): every queued effect
    /// is forgotten, including requests this binding has not admitted yet.
    /// Sounds already handed to the output keep playing.
    pub fn clear_queue(&mut self) {
        self.pending.clear();
        self.queue.entries.clear();
    }
    pub fn area(&mut self, tile: [i32; 2], listener: [i32; 2], fields: [i32; 4], cycle: i32) {
        self.pending
            .push((cycle, Request::Area(tile, listener, fields)));
    }
    pub fn sequence(
        &mut self,
        seq: &Sequence,
        frame: i32,
        fine: [i32; 2],
        is_self: bool,
        cycle: i32,
    ) {
        if let Some(sounds) = usize::try_from(frame)
            .ok()
            .and_then(|i| seq.sounds.get(i))
            .filter(|s| !s.is_empty())
        {
            self.pending.push((
                cycle,
                Request::Sequence {
                    sounds: sounds.clone(),
                    fine,
                    is_self,
                },
            ));
            self.sequence_triggers += 1;
        }
    }
    async fn prepare(&mut self, pack: &Pack) -> Vec<String> {
        let mut errors = vec![];
        let keys: BTreeSet<_> = self.queue.entries.iter().map(|e| [e.id, e.loops]).collect();
        for key in keys {
            if self.sounds.contains_key(&key) || self.failed.contains(&key) {
                continue;
            }
            let before = rs09_cache::store::misses_on_this_thread();
            let effect = pack
                .get(&format!("sound/{}", key[0]))
                .and_then(|b| SoundEffect::decode_for_queue(b).map_err(|e| e.to_string()));
            if effect.is_err() && rs09_cache::store::misses_on_this_thread() != before {
                // Still arriving from the update server: asked for, and
                // tried again while the sound is queued.
                continue;
            }
            match effect {
                Ok((effect, start)) => {
                    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
                    let prepared: Result<Prepared, String> = Ok(Prepared {
                        pcm: Arc::new(effect),
                    });
                    #[cfg(not(any(target_os = "windows", target_arch = "wasm32")))]
                    let prepared: Result<Prepared, String> = match effect.repeated(key[1]) {
                        Ok(effect) => load_sound_from_bytes(&effect.wav())
                            .await
                            .map_err(|e| e.to_string()),
                        Err(e) => Err(e.to_string()),
                    };
                    match prepared {
                        Ok(sound) => {
                            self.sounds.insert(key, sound);
                            self.starts.insert(key[0], start);
                        }
                        Err(error) => {
                            self.failed.insert(key);
                            errors.push(format!("Sound {}: {error}", key[0]));
                        }
                    }
                }
                Err(error) => {
                    self.failed.insert(key);
                    errors.push(format!("Sound {}: {error}", key[0]));
                }
            }
        }
        errors
    }
    /// Original queue cycles follow timestamped UI and actor requests. Native
    /// playback and WebAudio use identical synthesized finite-loop PCM.
    pub async fn advance(
        &mut self,
        pack: &Pack,
        until: i32,
        listener: [i32; 2],
        volumes: [i32; 2],
    ) -> Vec<String> {
        let mut errors = vec![];
        let previous = self
            .cycle
            .unwrap_or_else(|| self.pending.first().map_or(until, |p| p.0).wrapping_sub(1));
        let elapsed = until.wrapping_sub(previous).clamp(0, 1000);

        for offset in 1..=elapsed {
            let cycle = previous.wrapping_add(offset);
            let mut later = vec![];
            for (at, request) in self.pending.drain(..) {
                if at.wrapping_sub(cycle) > 0 {
                    later.push((at, request));
                    continue;
                }
                match request {
                    Request::Effect([id, loops, delay]) => {
                        self.queue.play(loops, id, delay, volumes[0])
                    }
                    Request::Sequence {
                        sounds,
                        fine,
                        is_self,
                    } => self.queue.sequence(
                        &Sequence {
                            sounds: vec![sounds],
                            ..Default::default()
                        },
                        0,
                        fine,
                        is_self,
                        volumes,
                        || macroquad::rand::gen_range(0.0, 1.0),
                    ),
                    Request::Area(tile, listener, fields) => {
                        self.queue.area(tile, listener, fields, volumes[1])
                    }
                }
            }
            self.pending = later;
            errors.extend(self.prepare(pack).await);
            for event in self
                .queue
                .tick(listener, volumes, |id| self.starts.get(&id).copied())
            {
                #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
                if let Some(pcm) = self
                    .sounds
                    .get(&[event.id, event.loops])
                    .map(|s| s.pcm.clone())
                    && self.start_pcm(pcm, event.loops - 1, event.volume).is_some()
                {
                    self.submitted += 1;
                }
                #[cfg(not(any(target_os = "windows", target_arch = "wasm32")))]
                if let Some(sound) = self.sounds.get(&[event.id, event.loops]) {
                    play_sound(
                        sound,
                        PlaySoundParams {
                            looped: false,
                            volume: event.volume as f32 / 256.0,
                        },
                    );
                    self.submitted += 1;
                }
            }
            self.cycle = Some(cycle);
        }
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        {
            if let Some(error) = self
                .output
                .as_ref()
                .and_then(|o| o.error())
                .filter(|_| !self.output_failed)
            {
                self.output_failed = true;
                self.output_errors.push(error);
            }
            errors.append(&mut self.output_errors);
        }
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scene_rebuild_forgets_admitted_and_waiting_effects() {
        let mut effects = Effects::default();
        effects.enqueue(&mut vec![[5, 1, 0]], 10);
        effects.area([1, 1], [1, 1], [6, 2, 1, 0], 10);
        effects.queue.area([1, 1], [1, 1], [7, 2, 1, 0], 127);
        assert_eq!((effects.pending.len(), effects.queue.entries.len()), (2, 1));
        effects.clear_queue();
        assert!(effects.pending.is_empty() && effects.queue.entries.is_empty());
        // No output device was opened and none is touched.
        assert_eq!(effects.submitted, 0);
    }
}
