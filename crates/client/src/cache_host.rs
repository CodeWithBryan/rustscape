//! The browser build's cache: the page's sockets and storage joined to
//! [`rs09_client::web_cache`], the loading bar while it arrives, and the
//! cache-backed parts of the client opened over it.
//!
//! The original applet fetches its cache from the game server it was loaded
//! from and keeps it on disk. A page names the update server on its start
//! form (`ws://host:port/`, the Rust server's WebSocket listener, which
//! speaks JS5 as its TCP listener does); without one the prepared packs are
//! used as before and nothing here runs.
//!
//! All decisions are in `web_cache.rs`, which is tested natively. This file
//! only binds the page's functions (`web/visual/game_transport.js`,
//! `cache_storage.js`) and draws.
use crate::cache_update_screen;
use macroquad::prelude::*;
use rs09_client::{
    cache_pack::{Source, cache_pack_from},
    cache_update::{Snapshot, login_crc_sums},
    music::{CacheSource, Music, Request},
    pack::Pack,
    web_cache::{self, Blobs, Stored, WebCache},
    world_pack::{Inputs, Options, world_pack_from},
};
use rs09_net::js5_stream::{MessagePort, PortState, StreamOpener};
use std::{
    cell::{Cell, RefCell},
    sync::{Arc, Mutex},
};

unsafe extern "C" {
    fn rs09_launch_value(index: i32, ptr: *mut u8, capacity: usize) -> i32;
    fn rs09_ws_open_cache(ptr: *const u8, len: usize) -> i32;
    fn rs09_ws_status(handle: i32) -> i32;
    fn rs09_ws_read(handle: i32, ptr: *mut u8, capacity: usize) -> i32;
    fn rs09_ws_send(handle: i32, ptr: *const u8, len: usize) -> i32;
    fn rs09_ws_close(handle: i32);
    fn rs09_store_open(ptr: *const u8, len: usize);
    fn rs09_store_status() -> i32;
    fn rs09_store_count() -> i32;
    fn rs09_store_key(index: i32, ptr: *mut u8, capacity: usize) -> i32;
    fn rs09_store_length(ptr: *const u8, len: usize) -> i32;
    fn rs09_store_read(key: *const u8, key_len: usize, ptr: *mut u8, len: usize) -> i32;
    fn rs09_store_release();
    fn rs09_store_put(key: *const u8, key_len: usize, ptr: *const u8, len: usize);
    fn rs09_store_commit() -> i32;
    fn rs09_store_busy() -> i32;
    fn rs09_store_failed() -> i32;
    fn rs09_store_clear();
    fn rs09_cache_state(ptr: *const u8, len: usize);
}

/// The database the cache is kept in, one per revision.
const DATABASE: &str = "rs09-cache-530";
/// The data file's bytes are given the original's size limit up front, so
/// that they never have to move while the cache arrives.
const DATA_RESERVE: usize = rs09_cache::disk::DATA_MAX_LENGTH as usize;
/// Work on the cache per drawn frame while the bar is up, in seconds.
const LOADING_BUDGET: f64 = 0.010;
/// The same once the client runs, per frame.
const RUNNING_BUDGET: f64 = 0.003;

thread_local! {
    static CACHE: RefCell<Option<Arc<Mutex<WebCache>>>> = const { RefCell::new(None) };
    /// Page time when the cache was started, and the next cycle due, in ms.
    static STARTED: Cell<f64> = const { Cell::new(0.0) };
    static NEXT_CYCLE: Cell<u64> = const { Cell::new(0) };
    static FRAMES: Cell<u32> = const { Cell::new(0) };
}

/// One value of the page's launch form (`window.rs09Launch`), read once.
pub fn launch_value(index: i32) -> String {
    let mut bytes = vec![0; 2048];
    let n = unsafe { rs09_launch_value(index, bytes.as_mut_ptr(), bytes.len()) };
    if n <= 0 {
        return String::new();
    }
    bytes.truncate(n as usize);
    String::from_utf8(bytes).unwrap_or_default()
}
fn now_ms() -> u64 {
    ((miniquad::date::now() - STARTED.with(Cell::get)) * 1000.0).max(0.0) as u64
}
fn cache() -> Option<Arc<Mutex<WebCache>>> {
    CACHE.with(|cache| cache.borrow().clone())
}
fn locked(cache: &Arc<Mutex<WebCache>>) -> std::sync::MutexGuard<'_, WebCache> {
    cache.lock().unwrap_or_else(|e| e.into_inner())
}
fn publish(phase: &str, status: serde_json::Value) {
    let mut state = status;
    state["phase"] = phase.into();
    let text = state.to_string();
    unsafe { rs09_cache_state(text.as_ptr(), text.len()) }
}

/// One connection of the page's, as the update client's message transport.
struct WsPort {
    handle: i32,
    closed: bool,
}
impl WsPort {
    fn open(url: &str) -> Option<Self> {
        let handle = unsafe { rs09_ws_open_cache(url.as_ptr(), url.len()) };
        (handle >= 0).then_some(Self {
            handle,
            closed: false,
        })
    }
}
impl MessagePort for WsPort {
    fn state(&mut self) -> PortState {
        if self.closed {
            return PortState::Closed;
        }
        match unsafe { rs09_ws_status(self.handle) } {
            0 => PortState::Connecting,
            1 => PortState::Open,
            2 => PortState::Closed,
            _ => PortState::Failed,
        }
    }
    fn receive(&mut self, destination: &mut [u8]) -> usize {
        if self.closed {
            return 0;
        }
        let n = unsafe { rs09_ws_read(self.handle, destination.as_mut_ptr(), destination.len()) };
        n.max(0) as usize
    }
    fn send(&mut self, bytes: &[u8]) -> bool {
        !self.closed && unsafe { rs09_ws_send(self.handle, bytes.as_ptr(), bytes.len()) } == 0
    }
    fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            unsafe { rs09_ws_close(self.handle) }
        }
    }
}
impl Drop for WsPort {
    fn drop(&mut self) {
        self.close();
    }
}

/// The page's IndexedDB store.
struct PageStorage;
impl Blobs for PageStorage {
    fn poll(&mut self) -> Stored {
        match unsafe { rs09_store_status() } {
            0 | 1 => Stored::Pending,
            2 => {
                let mut listing = Vec::new();
                for index in 0..unsafe { rs09_store_count() } {
                    let mut key = vec![0; 256];
                    let n = unsafe { rs09_store_key(index, key.as_mut_ptr(), key.len()) };
                    if n <= 0 {
                        continue;
                    }
                    key.truncate(n as usize);
                    let length = unsafe { rs09_store_length(key.as_ptr(), key.len()) };
                    if let (Ok(key), Ok(length)) = (String::from_utf8(key), usize::try_from(length))
                    {
                        listing.push((key, length));
                    }
                }
                Stored::Ready(listing)
            }
            _ => Stored::Unavailable,
        }
    }
    fn read(&mut self, key: &str, destination: &mut [u8]) -> bool {
        unsafe {
            rs09_store_read(
                key.as_ptr(),
                key.len(),
                destination.as_mut_ptr(),
                destination.len(),
            ) == 0
        }
    }
    fn release(&mut self) {
        unsafe { rs09_store_release() }
    }
    fn put(&mut self, key: &str, bytes: &[u8]) {
        unsafe { rs09_store_put(key.as_ptr(), key.len(), bytes.as_ptr(), bytes.len()) }
    }
    fn commit(&mut self) -> bool {
        unsafe { rs09_store_commit() == 0 }
    }
    fn busy(&self) -> bool {
        unsafe { rs09_store_busy() != 0 }
    }
    fn failed(&self) -> bool {
        unsafe { rs09_store_failed() != 0 }
    }
    fn clear(&mut self) {
        unsafe { rs09_store_clear() }
    }
}

fn bar(snapshot: &Snapshot, cache: Option<&WebCache>, note: Option<&str>) {
    let mut shown = snapshot.clone();
    if let Some(cache) = cache {
        // After the start-up sequence the bar counts what the original
        // downloads in the background; songs are not waited for.
        let progress = cache.progress();
        shown.groups = progress.groups;
        shown.groups_verified = progress.verified;
    }
    cache_update_screen::draw(&shown, note);
}

/// Obtains the cache if the page named an update server: restores what the
/// browser has stored, then runs the original start-up sequence against the
/// server behind the loading bar until everything the client reads without
/// waiting is in (see `web_cache`). On failure the message stays up until a
/// key or button is pressed and the client carries on with prepared packs.
pub async fn start() {
    let endpoint = launch_value(5);
    if endpoint.is_empty() {
        return;
    }
    let Some((prefix, port, path)) = web_cache::split_endpoint(&endpoint) else {
        fail("The update server is written ws://host:port/").await;
        return;
    };
    STARTED.with(|started| started.set(miniquad::date::now()));
    unsafe { rs09_store_open(DATABASE.as_ptr(), DATABASE.len()) }
    let mut storage = PageStorage;
    let waiting = Snapshot {
        percentage: 5,
        text: "Reading the cache stored in this browser".into(),
        ..Snapshot::default()
    };
    let listing = loop {
        match storage.poll() {
            Stored::Pending => {
                publish("storage", serde_json::json!({}));
                bar(&waiting, None, None);
                next_frame().await;
            }
            Stored::Unavailable => break None,
            Stored::Ready(listing) => break Some(listing),
        }
    };
    let empty = || {
        rs09_cache::disk::DiskStore::in_memory(Vec::with_capacity(DATA_RESERVE), Default::default())
    };
    let (store, blobs) = match listing {
        Some(listing) => {
            let restored = web_cache::restore(&mut storage, &listing, DATA_RESERVE);
            storage.release();
            if restored.is_none() && !listing.is_empty() {
                storage.clear();
            }
            (
                restored.unwrap_or_else(empty),
                Some(Box::new(storage) as Box<dyn Blobs>),
            )
        }
        None => (empty(), None),
    };
    let opener = StreamOpener::new(move |port| WsPort::open(&format!("{prefix}:{port}{path}")));
    let mut cache = WebCache::new(Box::new(opener), port, store, blobs);
    // The client opens its title once these are whole (and the start-up
    // sequence has run); maps, models and animations arrive behind it.
    cache
        .updater
        .set_start_archives(rs09_client::cache_update::start_archives());
    // `Math.random()` for the update connection's XOR key.
    let mut seed = (miniquad::date::now() * 1000.0) as u64 | 1;
    cache.updater.set_random(Box::new(move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    }));
    let mut next_cycle = 0u64;
    let mut frames = 0u32;
    loop {
        let frame_started = miniquad::date::now();
        let now = now_ms();
        let mut result = Ok(());
        // The original's cycles, 20 ms apart; a slow frame runs a few.
        let mut ran = 0;
        while result.is_ok() && now >= next_cycle && ran < 5 {
            result = cache.cycle(now);
            next_cycle += 20;
            ran += 1;
        }
        if next_cycle + 200 < now {
            next_cycle = now;
        }
        // Beyond the original: further passes while there is something to
        // read or verify, so the download is not held to 100 reads a cycle.
        while result.is_ok() && miniquad::date::now() - frame_started < LOADING_BUDGET {
            let before = (
                cache.updater.connection.queue.bytes_received,
                cache.progress().verified,
            );
            result = cache.io_pass(now);
            let after = (
                cache.updater.connection.queue.bytes_received,
                cache.progress().verified,
            );
            if before == after {
                break;
            }
        }
        cache.mirror(now, 2000, false);
        if frames % 10 == 0 || result.is_err() || cache.start_ready() {
            publish("loading", cache.status());
        }
        frames += 1;
        if let Err(error) = result {
            // What arrived is kept for the next start.
            cache.mirror(now, 0, true);
            fail(&format!("Error connecting to server: {error}")).await;
            return;
        }
        if cache.start_ready() {
            break;
        }
        bar(&cache.snapshot(), Some(&cache), None);
        next_frame().await;
    }
    cache.mirror(now_ms(), 0, true);
    let mut status = cache.status();
    status["start_ms"] = now_ms().into();
    publish("ready", status);
    NEXT_CYCLE.with(|next| next.set(now_ms()));
    CACHE.with(|slot| *slot.borrow_mut() = Some(Arc::new(Mutex::new(cache))));
}

async fn fail(message: &str) {
    macroquad::logging::error!("{}", message);
    publish("failed", serde_json::json!({"error": message}));
    let snapshot = Snapshot {
        percentage: 0,
        text: "The prepared packs are used instead - press a key".into(),
        ..Snapshot::default()
    };
    loop {
        bar(&snapshot, None, Some(message));
        next_frame().await;
        if get_last_key_pressed().is_some() || is_mouse_button_pressed(MouseButton::Left) {
            return;
        }
    }
}

/// Once per drawn frame while the client runs: the update connection's
/// cycles (songs are fetched through it when they are needed, and it is
/// closed and reopened by the original's rules) and the mirror to storage.
pub fn cycle() {
    let Some(cache) = cache() else {
        return;
    };
    let mut cache = locked(&cache);
    let now = now_ms();
    let mut next = NEXT_CYCLE.with(Cell::get);
    let mut ran = 0;
    while now >= next && ran < 5 {
        // Past the loading screen a failure is not fatal; it is retried by
        // the connection's own back-off and shows in the published state.
        let _ = cache.cycle(now);
        next += 20;
        ran += 1;
    }
    if next + 200 < now {
        next = now;
    }
    NEXT_CYCLE.with(|slot| slot.set(next));
    // Beyond the original, as while the bar was up: a few more passes while
    // there is something to read, so that the groups the title and the
    // scene wait for (and the rest behind them) are not held to one read
    // window per 20 ms. Bounded so that a frame keeps its time.
    let started = miniquad::date::now();
    while miniquad::date::now() - started < RUNNING_BUDGET {
        let before = cache.updater.connection.queue.bytes_received;
        if cache.io_pass(now).is_err() || cache.updater.connection.queue.bytes_received == before {
            break;
        }
    }
    cache.mirror(now, 5000, false);
    let frames = FRAMES.with(|frames| {
        frames.set(frames.get().wrapping_add(1));
        frames.get()
    });
    if frames % 30 == 0 {
        publish("running", cache.status());
    }
}

pub fn active() -> bool {
    cache().is_some()
}

/// The prepared pack at `path`, unless the cache came from the update
/// server: then the scene is built from it and no pack is fetched.
pub async fn prepared(path: &str) -> Result<Vec<u8>, String> {
    if active() {
        return Err("the cache from the update server is used".into());
    }
    load_file(path).await.map_err(|e| e.to_string())
}

async fn server_commit() -> String {
    load_file("upstream.lock.json")
        .await
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|lock| lock["server"]["commit"].as_str().map(str::to_owned))
        .unwrap_or_else(|| "unrecorded".into())
}

/// The starting scene built from the downloaded cache, as the desktop
/// client's `--cache` builds it. Region keys are read from
/// `assets/visual/xteas.json` if the page serves that file; a region without
/// a key keeps its terrain and has no locations until a server's rebuild
/// supplies the key and streams the scene.
pub async fn world_pack() -> Result<Pack, String> {
    let Some(cache) = cache() else {
        return Err("no update server was given".into());
    };
    let source = Source::from_store(locked(&cache).store());
    let mut inputs = Inputs::default();
    if let Ok(bytes) = load_file("assets/visual/xteas.json").await {
        inputs.keys = Inputs::parse_keys(&bytes).map_err(|e| format!("xteas.json: {e}"))?;
    }
    let options = Options {
        lenient: true,
        ..Options::default()
    };
    let commit = server_commit().await;
    // The starting scene's maps and models may still be arriving: built
    // again until it read nothing that was not there (each group it missed
    // is asked for at once), behind the loading box.
    let waiting = Snapshot {
        percentage: 100,
        text: "Loading - please wait.".into(),
        ..Snapshot::default()
    };
    let built = loop {
        let before = rs09_cache::store::misses_on_this_thread();
        let built = world_pack_from(&source, &inputs, &options, &commit);
        if rs09_cache::store::misses_on_this_thread() == before {
            break built?;
        }
        for _ in 0..10 {
            cycle();
            bar(&waiting, None, None);
            next_frame().await;
        }
    };
    for note in &built.notes {
        macroquad::logging::info!("Starting scene: {}", note);
    }
    Pack::decode(built.bytes)
}

/// Interfaces, definitions, models, animations and sounds read from the
/// downloaded cache on demand, as the desktop client's `--cache` reads them.
pub async fn ui_pack() -> Option<Pack> {
    let cache = cache()?;
    let source = Source::from_store(locked(&cache).store());
    match cache_pack_from(&source, &server_commit().await) {
        Ok(pack) => Some(pack),
        Err(error) => {
            macroquad::logging::error!("Interfaces from the cache: {}", error);
            None
        }
    }
}

/// The login block's CRC list, from the indexes the update server sent.
pub fn crc_sums() -> Option<[i32; 29]> {
    let cache = cache()?;
    let sums = locked(&cache).checksums()?;
    Some(login_crc_sums(&sums))
}

/// The music player over the downloaded cache and the page's audio device.
/// Songs and Vorbis samples are asked of the update server when a song
/// needs them, as the original fetches archives 6 and 14. The start form's
/// song (a name or a number) is requested at once, for listening offline.
pub fn music(effects: &mut crate::ui_audio::Effects, volume: i32) -> Option<Music> {
    let cache = cache()?;
    let source = match CacheSource::from_store(locked(&cache).store()) {
        Ok(source) => source,
        Err(error) => {
            macroquad::logging::error!("Music unavailable: {}", error);
            return None;
        }
    };
    let song = launch_value(6);
    let song = song
        .trim()
        .parse::<i32>()
        .ok()
        .or_else(|| source.song_named(&song.trim().to_ascii_lowercase()));
    let ready = Arc::clone(&cache);
    let source = source.on_demand(Box::new(move |archive, group| {
        locked(&ready).group_ready(archive, group)
    }));
    let mut music = Music::new(Box::new(source), volume);
    if let Err(error) = effects.music(music.device()) {
        macroquad::logging::error!("Music unavailable: {}", error);
        return None;
    }
    music.set_feed(Box::new(rs09_client::pcm::browser::feed));
    if let Some(song) = song {
        music.request(Request::Song(song));
    }
    Some(music)
}

/// Map streaming over the downloaded cache, when the starting scene came
/// from it. The server's rebuild messages supply the region keys.
pub fn world_map(
    scene_from_cache: bool,
    world: &mut crate::Workbench,
) -> Option<crate::world_cache::WorldMap> {
    if !scene_from_cache {
        return None;
    }
    let cache = cache()?;
    let opened = crate::world_cache::Cache::from_store(locked(&cache).store())
        .and_then(|cache| crate::world_cache::WorldMap::open_with(cache, "".as_ref(), world));
    match opened {
        Ok(map) => Some(map),
        Err(error) => {
            macroquad::logging::error!("Map streaming unavailable: {}", error);
            None
        }
    }
}
