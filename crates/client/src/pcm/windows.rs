//! Windows PCM presentation: owned WinMM output buffers, no external mixer or
//! decoder. Only this client's submitted samples are captured, never loopback.
//!
//! The original opens one device line for sound effects and one for music.
//! Here both are rendered by this thread and summed after each has been
//! clipped the way its own line would clip it. The capture keeps recording
//! the sound-effect half only, so effect replays stay comparable.
use super::{Mixer, Voice};
use crate::audio::SoundEffect;
use std::{
    collections::BTreeMap,
    ffi::c_void,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
#[derive(Default)]
struct Flags(BTreeMap<u64, Arc<AtomicBool>>);
impl std::ops::Deref for Flags {
    type Target = BTreeMap<u64, Arc<AtomicBool>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for Flags {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Drop for Flags {
    fn drop(&mut self) {
        for flag in self.0.values() {
            flag.store(false, Ordering::Release);
        }
    }
}

#[repr(C, packed(2))]
struct Format {
    kind: u16,
    channels: u16,
    rate: u32,
    bytes_per_second: u32,
    block: u16,
    bits: u16,
    extra: u16,
}
#[repr(C)]
struct Header {
    data: *mut i8,
    bytes: u32,
    recorded: u32,
    user: usize,
    flags: u32,
    loops: u32,
    next: *mut Header,
    reserved: usize,
}
#[link(name = "winmm")]
unsafe extern "system" {
    fn waveOutOpen(
        handle: *mut *mut c_void,
        device: u32,
        format: *const Format,
        callback: usize,
        instance: usize,
        flags: u32,
    ) -> u32;
    fn waveOutPrepareHeader(handle: *mut c_void, header: *mut Header, size: u32) -> u32;
    fn waveOutWrite(handle: *mut c_void, header: *mut Header, size: u32) -> u32;
    fn waveOutReset(handle: *mut c_void) -> u32;
    fn waveOutUnprepareHeader(handle: *mut c_void, header: *mut Header, size: u32) -> u32;
    fn waveOutClose(handle: *mut c_void) -> u32;
}

pub struct Handle {
    pub id: u64,
    linked: Arc<AtomicBool>,
}
impl Handle {
    pub fn linked(&self) -> bool {
        self.linked.load(Ordering::Acquire)
    }
}
enum Command {
    Music(crate::music::Shared),
    Play(u64, Arc<SoundEffect>, i32, i32, Arc<AtomicBool>),
    Volume(u64, i32),
    Stop(u64),
    Close,
}
#[derive(Default, Clone)]
pub struct Capture {
    pub pcm: Vec<i16>,
    pub buffers: usize,
    pub nonzero: usize,
    pub error: Option<String>,
    pub commands: Vec<Recorded>,
}
#[derive(Clone)]
pub enum Recorded {
    Play {
        frame: usize,
        id: u64,
        pcm: Arc<SoundEffect>,
        loops: i32,
        volume: i32,
    },
    Volume {
        frame: usize,
        id: u64,
        volume: i32,
    },
    Stop {
        frame: usize,
        id: u64,
    },
}
impl Capture {
    pub fn command_json(&self) -> serde_json::Value {
        serde_json::json!({"frames":self.pcm.len()/2,"rate":22050,"channels":2,"commands":self.commands.iter().map(|c|match c{
            Recorded::Play{frame,id,pcm,loops,volume}=>serde_json::json!({"op":"play","frame":frame,"id":id,"samples":pcm.samples,"start":pcm.loop_start,"end":pcm.loop_end,"loops":loops,"volume":volume}),
            Recorded::Volume{frame,id,volume}=>serde_json::json!({"op":"volume","frame":frame,"id":id,"volume":volume}),
            Recorded::Stop{frame,id}=>serde_json::json!({"op":"stop","frame":frame,"id":id}),
        }).collect::<Vec<_>>()})
    }
}
pub struct Output {
    tx: mpsc::Sender<Command>,
    capture: Arc<Mutex<Capture>>,
    next: u64,
    thread: Option<thread::JoinHandle<()>>,
}
impl Output {
    pub fn new(record: bool) -> Result<Self, String> {
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let capture = Arc::new(Mutex::new(Capture::default()));
        let state = capture.clone();
        let thread = thread::spawn(move || {
            let result = run(rx, &ready_tx, &state, record);
            if let Err(error) = result {
                state.lock().unwrap().error = Some(error.clone());
                let _ = ready_tx.send(Err(error));
            }
        });
        match ready_rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|e| format!("Windows PCM output startup: {e}"))?
        {
            Ok(()) => Ok(Self {
                tx,
                capture,
                next: 0,
                thread: Some(thread),
            }),
            Err(e) => Err(e),
        }
    }
    pub fn play(&mut self, pcm: Arc<SoundEffect>, loops: i32, volume: i32) -> Option<Handle> {
        if self.capture.lock().unwrap().error.is_some()
            || Voice::new(pcm.clone(), loops, volume).is_none()
        {
            return None;
        }
        self.next = self.next.wrapping_add(1);
        let linked = Arc::new(AtomicBool::new(true));
        self.tx
            .send(Command::Play(self.next, pcm, loops, volume, linked.clone()))
            .ok()?;
        Some(Handle {
            id: self.next,
            linked,
        })
    }
    /// Mix the music sequencer into every block from now on.
    pub fn music(&self, music: crate::music::Shared) {
        let _ = self.tx.send(Command::Music(music));
    }
    pub fn volume(&self, id: u64, volume: i32) {
        let _ = self.tx.send(Command::Volume(id, volume));
    }
    pub fn stop(&self, id: u64) {
        let _ = self.tx.send(Command::Stop(id));
    }
    pub fn snapshot(&self) -> Capture {
        self.capture.lock().unwrap().clone()
    }
    pub fn error(&self) -> Option<String> {
        self.capture.lock().unwrap().error.clone()
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Close);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct Device {
    handle: *mut c_void,
    // WinMM retains each prepared header pointer while this vector grows.
    #[allow(clippy::vec_box)]
    headers: Vec<Box<Header>>,
    _samples: Vec<Vec<i16>>,
}
impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            waveOutReset(self.handle);
            for h in &mut self.headers {
                waveOutUnprepareHeader(self.handle, &mut **h, std::mem::size_of::<Header>() as u32);
            }
            waveOutClose(self.handle);
        }
    }
}
fn check(code: u32, operation: &str) -> Result<(), String> {
    if code == 0 {
        Ok(())
    } else {
        Err(format!("Windows PCM {operation} failed ({code})"))
    }
}
fn run(
    rx: mpsc::Receiver<Command>,
    ready: &mpsc::Sender<Result<(), String>>,
    state: &Arc<Mutex<Capture>>,
    record: bool,
) -> Result<(), String> {
    let format = Format {
        kind: 1,
        channels: 2,
        rate: 22050,
        bytes_per_second: 88200,
        block: 4,
        bits: 16,
        extra: 0,
    };
    let mut handle = std::ptr::null_mut();
    // SAFETY: format and output handle remain live for the synchronous call;
    // CALLBACK_NULL installs no callback into Rust memory.
    unsafe {
        check(waveOutOpen(&mut handle, u32::MAX, &format, 0, 0, 0), "open")?;
    }
    let mut device = Device {
        handle,
        headers: vec![],
        _samples: vec![],
    };
    let size = std::mem::size_of::<Header>() as u32;
    for _ in 0..4 {
        let mut samples = vec![0i16; 512];
        let mut header = Box::new(Header {
            data: samples.as_mut_ptr().cast(),
            bytes: 1024,
            recorded: 0,
            user: 0,
            flags: 0,
            loops: 0,
            next: std::ptr::null_mut(),
            reserved: 0,
        });
        // SAFETY: heap allocations stay stable until reset/unprepare in Drop.
        unsafe {
            check(waveOutPrepareHeader(handle, &mut *header, size), "prepare")?;
        }
        device._samples.push(samples);
        device.headers.push(header);
    }
    let _ = ready.send(Ok(()));
    let mut mixer = Mixer::default();
    let mut flags = Flags::default();
    let mut rendered = 0usize;
    let mut music = None::<crate::music::Shared>;
    let mut music_block = [0i32; 512];
    loop {
        loop {
            match rx.try_recv() {
                Ok(Command::Music(shared)) => music = Some(shared),
                Ok(Command::Play(id, pcm, loops, volume, flag)) => {
                    if record {
                        state.lock().unwrap().commands.push(Recorded::Play {
                            frame: rendered,
                            id,
                            pcm: pcm.clone(),
                            loops,
                            volume,
                        });
                    }
                    if let Some(voice) = Voice::new(pcm, loops, volume) {
                        mixer.voices.insert(id, voice);
                        flags.insert(id, flag);
                    } else {
                        flag.store(false, Ordering::Release);
                    }
                }
                Ok(Command::Volume(id, v)) => {
                    if record {
                        state.lock().unwrap().commands.push(Recorded::Volume {
                            frame: rendered,
                            id,
                            volume: v,
                        });
                    }
                    if let Some(voice) = mixer.voices.get_mut(&id) {
                        voice.volume = v;
                    }
                }
                Ok(Command::Stop(id)) => {
                    if record {
                        state.lock().unwrap().commands.push(Recorded::Stop {
                            frame: rendered,
                            id,
                        });
                    }
                    mixer.voices.remove(&id);
                    if let Some(flag) = flags.remove(&id) {
                        flag.store(false, Ordering::Release);
                    }
                }
                Ok(Command::Close) | Err(mpsc::TryRecvError::Disconnected) => {
                    for flag in flags.values() {
                        flag.store(false, Ordering::Release);
                    }
                    return Ok(());
                }
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
        for (i, h) in device.headers.iter_mut().enumerate() {
            // WinMM owns flags while queued; WHDR_INQUEUE and WHDR_DONE are the
            // documented handoff. Never overwrite a buffer still owned by it.
            let status = unsafe { std::ptr::read_volatile(&raw const h.flags) };
            if status & 0x10 != 0 && status & 1 == 0 {
                continue;
            }
            let pcm = mixer.read(256);
            device._samples[i].copy_from_slice(&pcm);
            if let Some(music) = &music {
                music
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .read(&mut music_block);
                super::add_music(&mut device._samples[i], &music_block);
            }
            unsafe {
                check(waveOutWrite(handle, &mut **h, size), "write")?;
            }
            {
                let mut capture = state.lock().unwrap();
                capture.buffers += 1;
                capture.nonzero += pcm.iter().filter(|&&v| v != 0).count();
                if record {
                    capture.pcm.extend_from_slice(&pcm);
                }
            }
            rendered += 256;
            mixer.voices.retain(|id, voice| {
                if !voice.linked {
                    if let Some(flag) = flags.remove(id) {
                        flag.store(false, Ordering::Release);
                    }
                    false
                } else {
                    true
                }
            });
        }
        thread::sleep(Duration::from_millis(2));
    }
}
