//! Obtaining, verifying and repairing the cache from the game server: the JS5
//! half of `client.mainLoad`, stepped once per client cycle.
//!
//! [`Updater::cycle`] is one pass of the original main loop as far as the
//! cache is concerned: update-server I/O, the per-archive verification and
//! background download, then the staged start-up sequence with the original
//! loading-bar percentages and texts ([`STAGES`]). The stages that load
//! fonts, sprites, sound and so on in the original only *wait for their
//! data* here; decoding stays with the code that reads the store afterwards.
//!
//! Three things go beyond the original and are marked where they occur:
//! [`Updater::set_complete`] (also download the two archives the original
//! fetches on demand, and any archive past 27 the server lists),
//! [`Updater::io_pass`] (extra network passes between cycles) and
//! [`run`]'s stall detection.
use rs09_cache::disk::{ARCHIVE_MAX_GROUP, Cache, DiskStore, MASTER_MAX_GROUP};
#[cfg(not(target_arch = "wasm32"))]
use rs09_cache::{
    Limits,
    index::Index,
    store::{StorageMode, Store},
};
#[cfg(not(target_arch = "wasm32"))]
use rs09_net::js5_tcp::TcpOpener;
use rs09_net::{
    js5_client::{FatalError, Js5Connection, Js5NetQueue, SocketOpener},
    js5_provider::{CacheQueue, Js5Archive, MasterIndex, ProviderError, ResourceProvider},
};
#[cfg(not(target_arch = "wasm32"))]
use rs09_protocol::checksum::crc32;
use std::{
    io,
    sync::{Arc, Mutex},
    time::Duration,
};
#[cfg(not(target_arch = "wasm32"))]
use std::{
    path::{Path, PathBuf},
    thread::{self, JoinHandle},
    time::Instant,
};

/// The original client knows archives 0 to 27.
pub const ARCHIVE_COUNT: usize = 28;
/// `client.JS5_ARCHIVE_WEIGHTS`: each archive index's share of "Checking for
/// updates". They add up to 100.
pub const ARCHIVE_WEIGHTS: [i32; ARCHIVE_COUNT] = [
    4, 4, 1, 2, 6, 4, 2, 49, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
];
/// `createJs5`'s `prefetchAll` argument: every archive is downloaded in the
/// background except 6 (music) and 14, which are fetched when needed.
pub const PREFETCH_ALL: [bool; ARCHIVE_COUNT] = [
    true, true, true, true, true, true, false, true, true, true, true, true, true, true, false,
    true, true, true, true, true, true, true, true, true, true, true, true, true,
];
/// `LocalizedText.GAME0_LOADING`, drawn above the bar.
pub const PRIMARY_TEXT: &str = "RuneScape is loading - please wait...";
/// The client cycle: `mainLoop` runs 50 times a second.
pub const CYCLE: Duration = Duration::from_millis(20);

/// One `mainLoadState` of `client.mainLoad`.
pub struct Stage {
    pub state: i32,
    /// `mainLoadPercentage` while in this state (the bar is 3 pixels per unit).
    pub waiting_percentage: i32,
    /// Text while waiting; a progress number and `%` follow where
    /// `progress` is set.
    pub waiting: Option<&'static str>,
    pub progress: bool,
    /// `mainLoadPercentage` and text once the state is passed.
    pub done_percentage: i32,
    pub done: Option<&'static str>,
    /// What has to be in hand to pass.
    pub requires: &'static str,
}
const fn stage(
    state: i32,
    waiting_percentage: i32,
    waiting: Option<&'static str>,
    progress: bool,
    done_percentage: i32,
    done: Option<&'static str>,
    requires: &'static str,
) -> Stage {
    Stage {
        state,
        waiting_percentage,
        waiting,
        progress,
        done_percentage,
        done,
        requires,
    }
}
/// The start-up sequence in order. Texts are the English `LocalizedText`
/// strings with the `JagString` escapes resolved.
pub const STAGES: [Stage; 19] = [
    stage(
        0,
        5,
        Some("Allocating memory"),
        false,
        5,
        Some("Allocated memory"),
        "nothing",
    ),
    stage(
        10,
        10,
        None,
        false,
        10,
        Some("Created gameworld"),
        "nothing",
    ),
    stage(
        30,
        12,
        Some("Connecting to update server"),
        false,
        15,
        Some("Connected to update server"),
        "the master index: archive 255, group 255",
    ),
    stage(
        40,
        20,
        Some("Checking for updates - "),
        true,
        20,
        Some("Loaded update list"),
        "the index of every archive 0-27, weighted by ARCHIVE_WEIGHTS",
    ),
    stage(
        45,
        30,
        None,
        false,
        30,
        Some("Prepared sound engine"),
        "nothing",
    ),
    stage(
        50,
        35,
        Some("Loading fonts - "),
        true,
        35,
        Some("Loaded fonts"),
        "archive 8 groups p11_full, p12_full, b12_full and the same IDs in archive 13",
    ),
    stage(
        60,
        40,
        Some("Loading title screen - "),
        true,
        40,
        Some("Loaded title screen"),
        "archive 8 groups titlebg and logo",
    ),
    stage(
        65,
        45,
        None,
        false,
        45,
        Some("Opened title screen"),
        "nothing",
    ),
    stage(
        70,
        50,
        Some("Loading config - "),
        true,
        50,
        Some("Loaded config"),
        "every group of archives 2, 16, 17, 18, 19, 20, 21, 22, 24, 25 and 27",
    ),
    stage(
        80,
        60,
        Some("Loading sprites - "),
        true,
        60,
        Some("Loaded sprites"),
        "fifteen named archive 8 groups, mapfunction to hint_mapedge",
    ),
    stage(
        90,
        70,
        Some("Loading textures - "),
        true,
        70,
        Some("Loaded textures"),
        "every group of archive 26",
    ),
    stage(100, 70, None, false, 70, None, "archive 8 group runes"),
    stage(
        110,
        75,
        None,
        false,
        75,
        Some("Loaded input handler"),
        "nothing",
    ),
    stage(
        120,
        80,
        Some("Loading wordpack - 0%"),
        false,
        80,
        Some("Loaded wordpack"),
        "archive 10 group huffman",
    ),
    stage(
        130,
        85,
        Some("Loading interfaces - "),
        true,
        95,
        Some("Loaded interfaces"),
        "every group of archives 3, 12 and 13, then archive 23 group details",
    ),
    stage(
        135,
        95,
        Some("Loading world list data"),
        false,
        96,
        Some("Loaded world list data"),
        "the world list, fetched on its own connection outside this module",
    ),
    stage(
        140,
        97,
        None,
        false,
        97,
        Some("Starting 3d Library"),
        "nothing",
    ),
    stage(
        150,
        100,
        None,
        false,
        100,
        Some("Started 3d Library"),
        "nothing",
    ),
    stage(
        160,
        100,
        None,
        false,
        100,
        None,
        "nothing: the login screen opens",
    ),
];
pub fn stage_for(state: i32) -> Option<&'static Stage> {
    STAGES.iter().find(|stage| stage.state == state)
}

const FONTS: [&str; 3] = ["p11_full", "p12_full", "b12_full"];
const TITLE: [&str; 2] = ["titlebg", "logo"];
const SPRITES: [&str; 15] = [
    "mapfunction",
    "hitmarks",
    "hitbar_default",
    "headicons_pk",
    "headicons_prayer",
    "hint_headicons",
    "hint_mapmarkers",
    "mapflag",
    "cross",
    "mapdots",
    "scrollbar",
    "name_icons",
    "floorshadows",
    "compass",
    "hint_mapedge",
];
const CONFIG_ARCHIVES: [usize; 11] = [2, 16, 17, 18, 19, 20, 21, 22, 24, 25, 27];

/// The result of `WorldList.fetch`, supplied by whoever fetches the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldList {
    /// Not fetched at all: stage 135 is passed without the list.
    NotFetched,
    Pending,
    Loaded,
    /// The world-list server answered 7 or 9: `worldlistfull`.
    Full,
    /// Any other failure: `worldlistio_<code>`.
    Failed(i32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateError {
    /// `client.error(...)`; the client stops.
    Fatal(FatalError),
    /// A `RuntimeException` out of a resource provider.
    Provider(ProviderError),
    /// The master index has no entry for an archive the client needs.
    MasterIndex(u8),
    /// `worldlistfull` or `worldlistio_<code>`.
    WorldList(String),
    /// Not from the original: nothing arrived for the configured time while
    /// requests were outstanding (see [`run`]).
    Stalled(String),
    Io(String),
}
impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fatal(error) => f.write_str(error.message()),
            Self::Provider(error) => error.fmt(f),
            Self::MasterIndex(archive) => {
                write!(
                    f,
                    "the server's master index does not list archive {archive}"
                )
            }
            Self::WorldList(error) => f.write_str(error),
            Self::Stalled(detail) => write!(f, "update stalled: {detail}"),
            Self::Io(error) => f.write_str(error),
        }
    }
}
impl std::error::Error for UpdateError {}
impl From<ProviderError> for UpdateError {
    fn from(error: ProviderError) -> Self {
        Self::Provider(error)
    }
}
impl From<io::Error> for UpdateError {
    fn from(error: io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

/// What a progress display needs, cheap to copy out each frame.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub state: i32,
    /// 0 to 100; the original bar is `percentage * 3` pixels of 300.
    pub percentage: i32,
    pub text: String,
    pub game_state: i32,
    pub connected: bool,
    pub errors: i32,
    pub response: i32,
    /// Groups listed by the indexes obtained so far, and how many of them
    /// are verified in the store.
    pub groups: usize,
    pub groups_verified: usize,
    pub bytes_received: u64,
    pub replies_received: u64,
    /// The start-up sequence has reached the login screen.
    pub loaded: bool,
    /// Every archive has settled: nothing left to verify or download.
    pub settled: bool,
}

/// The update half of the client: connection, providers and load sequence.
pub struct Updater {
    pub connection: Js5Connection,
    cache_queue: CacheQueue,
    store: Option<Arc<Mutex<DiskStore>>>,
    master: Option<MasterIndex>,
    archives: Vec<Js5Archive>,
    complete: bool,
    state: i32,
    percentage: i32,
    text: String,
    font_ids: [i32; 3],
    title_ids: [i32; 2],
    sprite_ids: [i32; 15],
    runes_id: i32,
    /// `MusicPlayer.titleSong`: archive 6 group "scape main", or -1.
    pub title_song: i32,
    /// `LoginManager.loginScreenId`: archive 3 group "loginscreen", or -1.
    pub login_screen: i32,
    pub world_list: WorldList,
    loaded: bool,
}
impl Updater {
    /// `store` is the opened cache (archives 0 to 27 and 255); without one
    /// nothing is verified or kept, as when the original cannot open its
    /// cache files. Ports: the original alternates between two after a
    /// failure; pass the same port twice for a single-port server.
    pub fn new(
        opener: Box<dyn SocketOpener>,
        default_port: u16,
        alternate_port: u16,
        store: Option<Arc<Mutex<DiskStore>>>,
        cache_queue: CacheQueue,
    ) -> Self {
        Self {
            connection: Js5Connection::new(opener, default_port, alternate_port),
            cache_queue,
            store,
            master: None,
            archives: Vec::new(),
            complete: false,
            state: 0,
            percentage: 10,
            text: String::new(),
            font_ids: [-1; 3],
            title_ids: [-1; 2],
            sprite_ids: [-1; 15],
            runes_id: -1,
            title_song: -1,
            login_screen: -1,
            world_list: WorldList::NotFetched,
            loaded: false,
        }
    }
    /// Not in the original: also download archives 6 and 14 in full, and
    /// every archive past 27 that the server's master index lists. Call
    /// before the first cycle.
    pub fn set_complete(&mut self, complete: bool) {
        self.complete = complete;
    }
    /// The stand-in for `Math.random()` behind the update connection's XOR
    /// key. A host without a clock (the browser build) seeds its own.
    pub fn set_random(&mut self, random: Box<dyn FnMut() -> f64 + Send>) {
        self.connection.queue.set_random(random);
    }
    /// Carries out the store reads and writes queued so far, for a host
    /// whose [`CacheQueue`] has no worker thread ([`CacheQueue::manual`]).
    pub fn flush_store(&self) {
        self.cache_queue.flush();
    }
    /// `Js5.isGroupReady` on one of the client's archives: `true` once the
    /// group is verified in the store; until then it is read from the store
    /// or asked of the server at urgent priority, as the original does for
    /// anything it needs that has not arrived. `false` before the archive's
    /// index is in hand and for a group the index does not list.
    pub fn group_ready(&mut self, archive: usize, group: i32) -> Result<bool, UpdateError> {
        if self.archives.len() <= archive {
            return Ok(false);
        }
        Ok(self.with(archive, |js5, provider, net, queue| {
            js5.is_group_ready(group, provider, net, queue)
        })??)
    }
    pub fn main_load_state(&self) -> i32 {
        self.state
    }
    pub fn master(&self) -> Option<&MasterIndex> {
        self.master.as_ref()
    }
    /// The start-up sequence has finished and the login screen is open.
    pub fn loaded(&self) -> bool {
        self.loaded
    }
    /// Loaded, and every archive has finished verifying and downloading.
    pub fn settled(&self) -> bool {
        self.loaded
            && self.connection.queue.urgent_request_count() == 0
            && self.connection.queue.prefetch_request_count() == 0
            && self.master.as_ref().is_some_and(|master| {
                master
                    .providers()
                    .all(|provider| provider.index().is_some() && !provider.busy())
            })
    }
    /// The 28 index CRCs the login block carries (`Js5.getChecksum` of each
    /// archive), once every index is in hand.
    pub fn checksums(&self) -> Option<[i32; ARCHIVE_COUNT]> {
        let master = self.master.as_ref()?;
        let mut sums = [0; ARCHIVE_COUNT];
        for (archive, sum) in sums.iter_mut().enumerate() {
            *sum = master.provider(archive as u8)?.index()?.index.checksum as i32;
        }
        Some(sums)
    }
    pub fn snapshot(&self) -> Snapshot {
        let (mut groups, mut groups_verified) = (0, 0);
        if let Some(master) = &self.master {
            for provider in master.providers() {
                groups += provider.index_size();
                groups_verified += provider.verified_groups();
            }
        }
        Snapshot {
            state: self.state,
            percentage: self.percentage,
            text: self.text.clone(),
            game_state: self.connection.game_state,
            connected: self.connection.queue.connected(),
            errors: self.connection.queue.errors,
            response: self.connection.queue.response,
            groups,
            groups_verified,
            bytes_received: self.connection.queue.bytes_received,
            replies_received: self.connection.queue.replies_received,
            loaded: self.loaded,
            settled: self.settled(),
        }
    }

    /// One client cycle, in the original's order: `js5NetworkLoop`,
    /// `processResourceProviders`, then `mainLoad` while loading (game
    /// states 0 and 5). Call every [`CYCLE`]; the reconnect back-off counts
    /// these calls.
    pub fn cycle(&mut self, now_ms: u64) -> Result<(), UpdateError> {
        self.connection.network_loop(now_ms);
        if let Some(error) = self.connection.fatal {
            return Err(UpdateError::Fatal(error));
        }
        self.process_providers(now_ms)?;
        if matches!(self.connection.game_state, 0 | 5) {
            self.main_load()?;
        }
        Ok(())
    }
    /// Not in the original: reads and answers the update connection again
    /// without counting a cycle, so a download is not held to 100 socket
    /// reads per 20 ms. Never connects; that stays with [`Self::cycle`].
    pub fn io_pass(&mut self, now_ms: u64) -> Result<(), UpdateError> {
        if self.connection.game_state == 1000 || !self.connection.queue.connected() {
            return Ok(());
        }
        self.connection.queue.tick(now_ms);
        self.process_providers(now_ms)
    }
    fn process_providers(&mut self, now_ms: u64) -> Result<(), UpdateError> {
        if let Some(master) = &mut self.master {
            master.process_resource_providers(
                &mut self.connection.queue,
                &self.cache_queue,
                now_ms,
            )?;
        }
        Ok(())
    }
    /// Waits for queued store writes, flushes them to disk and closes the
    /// connection (opcode 7). The original abandons queued writes on exit.
    pub fn finish(&mut self) -> io::Result<()> {
        self.cache_queue.flush();
        self.connection.queue.drop_connection();
        self.connection.queue.quit();
        if let Some(store) = &self.store {
            store
                .lock()
                .map_err(|_| io::Error::other("cache store lock poisoned"))?
                .sync()?;
        }
        Ok(())
    }

    fn create_js5(&mut self, archive: u8, prefetch_all: bool) -> Result<(), UpdateError> {
        let (cache, master_cache) = match &self.store {
            Some(store) => {
                store
                    .lock()
                    .map_err(|_| io::Error::other("cache store lock poisoned"))?
                    .ensure_archive(archive)?;
                (
                    Some(Cache::new(Arc::clone(store), archive, ARCHIVE_MAX_GROUP)),
                    Some(Cache::new(Arc::clone(store), 255, MASTER_MAX_GROUP)),
                )
            }
            None => (None, None),
        };
        let provider = self
            .master
            .as_mut()
            .and_then(|master| {
                master.resource_provider(archive, cache, master_cache, &self.cache_queue)
            })
            .ok_or(UpdateError::MasterIndex(archive))?;
        if prefetch_all {
            provider.prefetch_all();
        }
        Ok(())
    }
    fn with<R>(
        &mut self,
        archive: usize,
        call: impl FnOnce(&mut Js5Archive, &mut ResourceProvider, &mut Js5NetQueue, &CacheQueue) -> R,
    ) -> Result<R, UpdateError> {
        let provider = self
            .master
            .as_mut()
            .and_then(|master| master.provider_mut(archive as u8))
            .ok_or(UpdateError::MasterIndex(archive as u8))?;
        let js5 = self
            .archives
            .get_mut(archive)
            .ok_or(UpdateError::MasterIndex(archive as u8))?;
        Ok(call(
            js5,
            provider,
            &mut self.connection.queue,
            &self.cache_queue,
        ))
    }
    fn group_id(&mut self, archive: usize, name: &str) -> Result<i32, UpdateError> {
        self.with(archive, |js5, provider, net, queue| {
            if js5.is_index_ready(provider, net, queue) {
                js5.group_id(name)
            } else {
                -1
            }
        })
    }
    fn file_ready(&mut self, archive: usize, id: i32) -> Result<bool, UpdateError> {
        Ok(self.with(archive, |js5, provider, net, queue| {
            js5.is_file_ready(id, provider, net, queue)
        })??)
    }
    fn fetch_all(&mut self, archive: usize) -> Result<bool, UpdateError> {
        Ok(self.with(archive, |js5, provider, net, queue| {
            js5.fetch_all(provider, net, queue)
        })??)
    }
    fn percentage_complete(&mut self, archive: usize) -> Result<i32, UpdateError> {
        self.with(archive, |js5, provider, net, queue| {
            js5.percentage_complete(provider, net, queue)
        })
    }
    fn waiting(&mut self, state: i32, progress: Option<i32>) {
        let stage = stage_for(state).expect("listed stage");
        self.percentage = stage.waiting_percentage;
        if let Some(text) = stage.waiting {
            self.text = match progress {
                Some(progress) if stage.progress => format!("{text}{progress}%"),
                _ => text.to_owned(),
            };
        }
    }
    fn done(&mut self, state: i32, next: i32) {
        let stage = stage_for(state).expect("listed stage");
        self.percentage = stage.done_percentage;
        if let Some(text) = stage.done {
            self.text = text.to_owned();
        }
        self.state = next;
    }

    /// `client.mainLoad`, one state per call as in the original.
    fn main_load(&mut self) -> Result<(), UpdateError> {
        match self.state {
            // The memory test compares zero against 16 MB and never waits.
            0 => self.done(0, 10),
            10 => self.done(10, 30),
            30 => {
                if self.master.is_none() {
                    self.master = Some(MasterIndex::new(&mut self.connection.queue));
                }
                let ready = self
                    .master
                    .as_mut()
                    .is_some_and(|master| master.is_ready(&mut self.connection.queue));
                if !ready {
                    self.waiting(30, None);
                    return Ok(());
                }
                for (archive, prefetch) in PREFETCH_ALL.into_iter().enumerate() {
                    self.create_js5(archive as u8, prefetch || self.complete)?;
                }
                self.archives = (0..ARCHIVE_COUNT).map(|_| Js5Archive::new()).collect();
                if self.complete {
                    let listed = self.master.as_ref().map_or(0, MasterIndex::archive_count);
                    for archive in ARCHIVE_COUNT..listed.min(255) {
                        let entry = self.master.as_ref().and_then(|m| m.entry(archive as u8));
                        // An archive the server lacks is listed as all zero.
                        if entry.is_some_and(|entry| entry != (0, 0)) {
                            self.create_js5(archive as u8, true)?;
                        }
                    }
                }
                self.done(30, 40);
            }
            40 => {
                let mut percentage = 0;
                for (archive, weight) in ARCHIVE_WEIGHTS.iter().enumerate() {
                    let provider = self
                        .master
                        .as_mut()
                        .and_then(|master| master.provider_mut(archive as u8))
                        .ok_or(UpdateError::MasterIndex(archive as u8))?;
                    percentage += provider
                        .index_percentage_complete(&mut self.connection.queue, &self.cache_queue)
                        * weight
                        / 100;
                }
                if percentage != 100 {
                    // At zero the previous text stays.
                    let shown = (percentage != 0).then_some(percentage);
                    let text = std::mem::take(&mut self.text);
                    self.waiting(40, shown);
                    if shown.is_none() {
                        self.text = text;
                    }
                    return Ok(());
                }
                // Sprites.init, TitleScreen.init, Flames.init: names to IDs.
                for (slot, name) in FONTS.iter().enumerate() {
                    self.font_ids[slot] = self.group_id(8, name)?;
                }
                for (slot, name) in SPRITES.iter().enumerate() {
                    self.sprite_ids[slot] = self.group_id(8, name)?;
                }
                for (slot, name) in TITLE.iter().enumerate() {
                    self.title_ids[slot] = self.group_id(8, name)?;
                }
                self.runes_id = self.group_id(8, "runes")?;
                self.done(40, 45);
            }
            45 => {
                self.title_song = self.group_id(6, "scape main")?;
                self.done(45, 50);
            }
            50 => {
                let mut ready = 0;
                for archive in [8, 13] {
                    for id in self.font_ids {
                        ready += i32::from(self.file_ready(archive, id)?);
                    }
                }
                if ready >= 6 {
                    self.done(50, 60);
                } else {
                    self.waiting(50, Some(ready * 100 / 6));
                }
            }
            60 => {
                let mut ready = 0;
                for id in self.title_ids {
                    ready += i32::from(self.file_ready(8, id)?);
                }
                if ready >= 2 {
                    self.done(60, 65);
                } else {
                    self.waiting(60, Some(ready * 100 / 2));
                }
            }
            65 => {
                // Fonts.load, then the title screen opens behind the bar.
                self.connection.set_game_state(5);
                self.done(65, 70);
            }
            70 => {
                let mut percentage = 0;
                for archive in CONFIG_ARCHIVES {
                    self.fetch_all(archive)?;
                    percentage += self.percentage_complete(archive)?;
                }
                if percentage >= 1100 {
                    self.done(70, 80);
                } else {
                    self.waiting(70, Some(percentage / 11));
                }
            }
            80 => {
                let mut ready = 0;
                for id in self.sprite_ids {
                    ready += i32::from(self.file_ready(8, id)?);
                }
                if ready >= 15 {
                    self.done(80, 90);
                } else {
                    self.waiting(80, Some(ready * 100 / 15));
                }
            }
            90 => {
                if self.fetch_all(26)? {
                    self.done(90, 100);
                } else {
                    let percentage = self.percentage_complete(26)?;
                    self.waiting(90, Some(percentage));
                }
            }
            100 => {
                let runes = self.runes_id;
                if self.file_ready(8, runes)? {
                    self.state = 110;
                }
            }
            110 => self.done(110, 120),
            120 => {
                let ready = self.with(10, |js5, provider, net, queue| {
                    js5.is_named_file_ready("", "huffman", provider, net, queue)
                })??;
                if ready {
                    self.done(120, 130);
                } else {
                    self.waiting(120, None);
                }
            }
            130 => {
                if !self.fetch_all(3)? {
                    let shown = self.percentage_complete(3)? * 3 / 4;
                    self.waiting(130, Some(shown));
                } else if !self.fetch_all(12)? {
                    let shown = self.percentage_complete(12)? / 10 + 75;
                    self.waiting(130, Some(shown));
                } else if !self.fetch_all(13)? {
                    let shown = self.percentage_complete(13)? / 20 + 85;
                    self.waiting(130, Some(shown));
                } else {
                    let (ready, progress) = self.with(23, |js5, provider, net, queue| {
                        if !js5.is_index_ready(provider, net, queue) {
                            return Ok((false, 0));
                        }
                        // The name table is consulted without checking that
                        // the group exists; an unknown name is never ready.
                        let details = js5.group_id("details");
                        let ready = js5.is_group_ready(details, provider, net, queue)?;
                        Ok::<_, ProviderError>((
                            ready,
                            js5.group_percentage_complete(details, provider),
                        ))
                    })??;
                    if ready {
                        self.done(130, 135);
                    } else {
                        self.waiting(130, Some(progress / 10 + 90));
                    }
                }
            }
            135 => match self.world_list {
                WorldList::Pending => self.waiting(135, None),
                WorldList::Full => {
                    self.connection.game_state = 1000;
                    return Err(UpdateError::WorldList("worldlistfull".into()));
                }
                WorldList::Failed(code) => {
                    self.connection.game_state = 1000;
                    return Err(UpdateError::WorldList(format!("worldlistio_{code}")));
                }
                WorldList::Loaded | WorldList::NotFetched => self.done(135, 140),
            },
            140 => {
                self.login_screen = self.group_id(3, "loginscreen")?;
                self.done(140, 150);
            }
            150 => self.done(150, 160),
            160 => {
                // InterfaceList.resetToLoginScreen: game state 10.
                self.connection.set_game_state(10);
                self.loaded = true;
            }
            _ => {}
        }
        Ok(())
    }
}

/// The login block's CRC list: the 28 index CRCs, then the zero the 2009scape
/// client sends for the archive it does not have (`LOGIN_FAKE_IDX28`).
pub fn login_crc_sums(checksums: &[i32; ARCHIVE_COUNT]) -> [i32; 29] {
    let mut sums = [0; 29];
    sums[..ARCHIVE_COUNT].copy_from_slice(checksums);
    sums
}

/// The 28 index CRCs of a cache directory, as `Js5.getChecksum` would report
/// them after loading each index from it. `None` when an index is absent.
#[cfg(not(target_arch = "wasm32"))]
pub fn stored_checksums(directory: &Path) -> io::Result<Option<[i32; ARCHIVE_COUNT]>> {
    let store = Store::open(directory, StorageMode::File, Limits::default())?;
    let mut sums = [0; ARCHIVE_COUNT];
    for (archive, sum) in sums.iter_mut().enumerate() {
        match store.read(255, archive as u32).map_err(io::Error::other)? {
            Some(index) => *sum = crc32(&index) as i32,
            None => return Ok(None),
        }
    }
    Ok(Some(sums))
}

/// Whether a directory holds the files a cache needs to be opened at all.
#[cfg(not(target_arch = "wasm32"))]
pub fn directory_usable(directory: &Path) -> bool {
    ["main_file_cache.dat2", "main_file_cache.idx255"]
        .iter()
        .all(|name| directory.join(name).is_file())
}

/// One archive of [`verify_directory`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArchiveCheck {
    pub archive: u8,
    pub index_crc: u32,
    pub index_version: i32,
    /// Groups the index lists (with at least one file).
    pub groups: usize,
    /// Groups stored with the index's CRC and version trailer.
    pub verified: usize,
    pub stored_bytes: u64,
    pub missing: Vec<u32>,
    pub crc_mismatch: Vec<u32>,
    pub version_mismatch: Vec<u32>,
}
impl ArchiveCheck {
    pub fn complete(&self) -> bool {
        self.verified == self.groups
    }
}
/// Reads a cache directory back through the read-only store reader and
/// applies the client's trust rule to every group of every archive the
/// stored master file (`idx255`) has an index for. This is an independent
/// re-check after a download, not part of the original.
#[cfg(not(target_arch = "wasm32"))]
pub fn verify_directory(directory: &Path) -> io::Result<Vec<ArchiveCheck>> {
    let limits = Limits::default();
    let store = Store::open(directory, StorageMode::Resident, limits)?;
    let mut archives = Vec::new();
    for archive in 0..store.group_slots(255).min(255) as u8 {
        let Some(stored) = store
            .read(255, u32::from(archive))
            .map_err(io::Error::other)?
        else {
            continue;
        };
        let index = Index::decode(&stored, None, limits).map_err(io::Error::other)?;
        let mut check = ArchiveCheck {
            archive,
            index_crc: index.checksum,
            index_version: index.version,
            ..ArchiveCheck::default()
        };
        for group in index.groups.iter().filter(|group| !group.files.is_empty()) {
            check.groups += 1;
            let Some(stored) = store.read(archive, group.id).map_err(io::Error::other)? else {
                check.missing.push(group.id);
                continue;
            };
            check.stored_bytes += stored.len() as u64;
            if stored.len() <= 2 || crc32(&stored[..stored.len() - 2]) != group.checksum {
                check.crc_mismatch.push(group.id);
                continue;
            }
            let trailer = u16::from_be_bytes([stored[stored.len() - 2], stored[stored.len() - 1]]);
            if i32::from(trailer) != group.version & 0xffff {
                check.version_mismatch.push(group.id);
                continue;
            }
            check.verified += 1;
        }
        archives.push(check);
    }
    Ok(archives)
}

/// How [`run`] drives the updater.
#[derive(Clone, Debug)]
pub struct Options {
    /// See [`Updater::set_complete`]. Without it archives 6 and 14 are only
    /// verified, as in the original, and a fresh directory ends up without
    /// their groups.
    pub complete: bool,
    /// Run [`Updater::io_pass`] between cycles instead of sleeping.
    pub fast: bool,
    /// The original alternates between two ports after a failure.
    pub alternate_port: Option<u16>,
    /// Give up when nothing has been received or verified for this long
    /// although the update is not finished. The original waits forever.
    pub stall: Duration,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            complete: true,
            fast: true,
            alternate_port: None,
            stall: Duration::from_secs(120),
        }
    }
}

/// What [`run`] did.
#[derive(Clone, Debug)]
pub struct Report {
    pub cycles: u64,
    pub elapsed: Duration,
    /// Time until the start-up sequence reached the login screen.
    pub loaded_after: Duration,
    pub bytes_received: u64,
    pub replies_received: u64,
    pub checksums: [i32; ARCHIVE_COUNT],
    /// Groups listed by all indexes, and how many the updater verified.
    pub groups: usize,
    pub groups_verified: usize,
    /// Size of the data file when the update finished.
    pub data_length: u64,
}

/// Brings the cache in `directory` up to date from the update server at
/// `host:port`, creating the directory and files as needed, and returns once
/// every archive has settled and the writes are on disk. `progress` is called
/// after every cycle.
///
/// The cycle rate, the 20-requests-per-priority window, verification order
/// and failure handling are the original's; `options` lists what is not.
#[cfg(not(target_arch = "wasm32"))]
pub fn run(
    host: &str,
    port: u16,
    directory: &Path,
    options: &Options,
    progress: &mut dyn FnMut(&Snapshot),
) -> Result<Report, UpdateError> {
    std::fs::create_dir_all(directory)?;
    let archives = (0..ARCHIVE_COUNT as u8).chain([255]);
    let store = Arc::new(Mutex::new(DiskStore::open(directory, archives)?));
    let mut updater = Updater::new(
        Box::new(TcpOpener::new(host)),
        port,
        options.alternate_port.unwrap_or(port),
        Some(Arc::clone(&store)),
        CacheQueue::threaded()?,
    );
    updater.set_complete(options.complete);
    let started = Instant::now();
    let cycle = CYCLE.as_millis() as u64;
    let mut next_cycle = 0u64;
    let mut cycles = 0u64;
    let mut loaded_after = None;
    let mut last_change = (0u64, (0u64, 0usize, 0i32));
    let outcome = loop {
        let now = started.elapsed().as_millis() as u64;
        if now >= next_cycle {
            if let Err(error) = updater.cycle(now) {
                break Err(error);
            }
            cycles += 1;
            next_cycle = (next_cycle + cycle).max(now.saturating_sub(cycle));
            let snapshot = updater.snapshot();
            progress(&snapshot);
            if snapshot.loaded && loaded_after.is_none() {
                loaded_after = Some(started.elapsed());
            }
            if snapshot.settled {
                break Ok(());
            }
            let mark = (
                snapshot.bytes_received,
                snapshot.groups_verified,
                snapshot.state,
            );
            if mark != last_change.1 {
                last_change = (now, mark);
            } else if now - last_change.0 > options.stall.as_millis() as u64 {
                let waiting: Vec<String> = updater
                    .connection
                    .queue
                    .in_flight()
                    .iter()
                    .map(|request| format!("{}/{}", request.archive, request.group))
                    .collect();
                break Err(UpdateError::Stalled(format!(
                    "nothing received for {:?} in state {} ({}); unanswered requests: [{}]",
                    options.stall,
                    snapshot.state,
                    snapshot.text,
                    waiting.join(", ")
                )));
            }
        } else if options.fast {
            let before = updater.connection.queue.bytes_received;
            if let Err(error) = updater.io_pass(now) {
                break Err(error);
            }
            if updater.connection.queue.bytes_received == before {
                thread::sleep(
                    Duration::from_millis(1).min(Duration::from_millis(next_cycle - now)),
                );
            }
        } else {
            thread::sleep(Duration::from_millis(next_cycle - now));
        }
    };
    // Whatever was verified so far is kept, also after a failure.
    let finished = updater.finish();
    outcome?;
    finished?;
    let snapshot = updater.snapshot();
    let checksums = updater
        .checksums()
        .ok_or_else(|| UpdateError::Io("an archive index was never obtained".into()))?;
    let data_length = store
        .lock()
        .map_err(|_| UpdateError::Io("cache store lock poisoned".into()))?
        .data_length();
    Ok(Report {
        cycles,
        elapsed: started.elapsed(),
        loaded_after: loaded_after.unwrap_or_else(|| started.elapsed()),
        bytes_received: snapshot.bytes_received,
        replies_received: snapshot.replies_received,
        checksums,
        groups: snapshot.groups,
        groups_verified: snapshot.groups_verified,
        data_length,
    })
}

/// [`run`] on its own thread, for a caller that draws while it works.
#[cfg(not(target_arch = "wasm32"))]
pub struct Background {
    snapshot: Arc<Mutex<Snapshot>>,
    thread: Option<JoinHandle<Result<Report, UpdateError>>>,
}
#[cfg(not(target_arch = "wasm32"))]
impl Background {
    pub fn start(
        host: String,
        port: u16,
        directory: PathBuf,
        options: Options,
    ) -> io::Result<Self> {
        let snapshot = Arc::new(Mutex::new(Snapshot {
            percentage: 10,
            ..Snapshot::default()
        }));
        let shared = Arc::clone(&snapshot);
        let thread = thread::Builder::new()
            .name("cache-update".into())
            .spawn(move || {
                run(&host, port, &directory, &options, &mut |latest| {
                    if let Ok(mut shared) = shared.lock() {
                        shared.clone_from(latest);
                    }
                })
            })?;
        Ok(Self {
            snapshot,
            thread: Some(thread),
        })
    }
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot
            .lock()
            .map(|snapshot| snapshot.clone())
            .unwrap_or_default()
    }
    pub fn finished(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }
    /// Waits for the update to end and returns its result.
    pub fn join(mut self) -> Result<Report, UpdateError> {
        match self.thread.take().map(JoinHandle::join) {
            Some(Ok(result)) => result,
            _ => Err(UpdateError::Io(
                "the update thread stopped unexpectedly".into(),
            )),
        }
    }
}

/// The command-line switches of the desktop client's start-up hook.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg(not(target_arch = "wasm32"))]
pub struct Request {
    pub host: String,
    pub port: u16,
    pub directory: PathBuf,
}
/// Decides whether the desktop client updates its cache before loading:
/// when a server is named (`--update-server HOST:PORT`, else the live
/// `--connect` endpoint) and either `--update-cache` is given or the cache
/// directory (`--cache DIR`, default `assets/cache`) has no cache in it.
#[cfg(not(target_arch = "wasm32"))]
pub fn requested(args: &[String]) -> Result<Option<Request>, String> {
    let option = |key: &str| {
        args.iter()
            .position(|argument| argument == key)
            .and_then(|position| args.get(position + 1))
            .cloned()
    };
    let forced = args.iter().any(|argument| argument == "--update-cache");
    let directory = PathBuf::from(option("--cache").unwrap_or_else(|| "assets/cache".into()));
    let Some(server) = option("--update-server").or_else(|| option("--connect")) else {
        return if forced {
            Err("--update-cache needs --update-server HOST:PORT (or --connect)".into())
        } else {
            Ok(None)
        };
    };
    if !forced && directory_usable(&directory) {
        return Ok(None);
    }
    let (host, port) = server
        .rsplit_once(':')
        .ok_or("the update server is written HOST:PORT")?;
    let port = port
        .parse()
        .map_err(|_| "the update server port is not a number")?;
    Ok(Some(Request {
        host: host.trim_matches(['[', ']']).to_owned(),
        port,
        directory,
    }))
}

#[cfg(test)]
mod tests {
    //! Transcription tests of the start-up sequence against the scripted
    //! server of `rs09_net::js5_client::scripted`; no socket is opened.
    use super::*;
    use rs09_net::js5_client::scripted::{FakeServer, Next, ScriptedOpener, ScriptedSocket};
    use std::collections::BTreeMap;

    fn payload(seed: u32, length: usize) -> Vec<u8> {
        (0..length).map(|i| (i as u32 * 31 + seed) as u8).collect()
    }
    /// 28 archives shaped like the real ones where the sequence looks.
    fn server() -> FakeServer {
        server_listing(ARCHIVE_COUNT)
    }
    /// The same archives behind a master index with `count` entries.
    fn server_listing(count: usize) -> FakeServer {
        let mut server = FakeServer::new(count);
        for archive in 0..ARCHIVE_COUNT as u8 {
            let version = 100 + i32::from(archive);
            match archive {
                8 => {
                    let names: Vec<&str> = FONTS
                        .iter()
                        .chain(&TITLE)
                        .chain(&SPRITES)
                        .copied()
                        .chain(["runes", "unused"])
                        .collect();
                    let groups: Vec<_> = names
                        .iter()
                        .enumerate()
                        .map(|(id, name)| (id as u32, *name, payload(id as u32, 700), 7))
                        .collect();
                    server.add_named_archive(8, version, &groups);
                }
                13 => {
                    // Font metrics share the font sprites' group IDs (0-2).
                    let groups: Vec<_> = (0..4).map(|id| (id, payload(id, 90), 3)).collect();
                    server.add_archive(13, version, &groups);
                }
                6 => server.add_named_archive(
                    6,
                    version,
                    &[
                        (0, "other", payload(1, 50), 1),
                        (5, "scape main", payload(2, 3000), 1),
                    ],
                ),
                10 => server.add_named_archive(10, version, &[(0, "huffman", payload(3, 256), 1)]),
                23 => server.add_named_archive(
                    23,
                    version,
                    &[
                        (0, "main", payload(4, 10), 1),
                        (1, "details", payload(5, 4000), 9),
                    ],
                ),
                3 => server.add_named_archive(
                    3,
                    version,
                    &[
                        (0, "x", payload(6, 20), 1),
                        (2, "loginscreen", payload(7, 60), 1),
                    ],
                ),
                _ => {
                    let groups: Vec<_> = (0..3 + u32::from(archive))
                        .map(|id| (id * 2, payload(id, 40 + id as usize * 9), 1 + id as i32))
                        .collect();
                    server.add_archive(archive, version, &groups);
                }
            }
        }
        server
    }
    struct Rig {
        updater: Updater,
        opener: ScriptedOpener,
        socket: ScriptedSocket,
        server: FakeServer,
        store: Arc<Mutex<DiskStore>>,
        now: u64,
        connected_before: bool,
        /// Every change of (state, percentage, text), in order.
        seen: Vec<(i32, i32, String)>,
    }
    fn fresh_socket(opener: &ScriptedOpener) -> ScriptedSocket {
        let socket = ScriptedSocket::new();
        socket.wire().handshake.push_back(0);
        opener
            .0
            .lock()
            .unwrap()
            .next
            .push_back(Next::Socket(socket.clone()));
        socket
    }
    impl Rig {
        fn new(server: FakeServer, store: DiskStore) -> Self {
            let opener = ScriptedOpener::default();
            let socket = fresh_socket(&opener);
            let store = Arc::new(Mutex::new(store));
            let updater = Updater::new(
                Box::new(opener.clone()),
                43594,
                43594,
                Some(Arc::clone(&store)),
                CacheQueue::manual(),
            );
            Self {
                updater,
                opener,
                socket,
                server,
                store,
                now: 0,
                connected_before: false,
                seen: Vec::new(),
            }
        }
        fn cycle(&mut self) -> Result<(), UpdateError> {
            self.now += 20;
            let connected = self.updater.connection.queue.connected();
            self.updater.cycle(self.now)?;
            if !connected && self.updater.connection.queue.connected() {
                if self.connected_before {
                    self.server.reconnected();
                }
                self.connected_before = true;
            }
            if !self.updater.connection.queue.connected()
                && self.opener.0.lock().unwrap().next.is_empty()
            {
                self.socket = fresh_socket(&self.opener);
            }
            self.server.serve(&self.socket);
            self.updater.cache_queue.flush();
            let snapshot = self.updater.snapshot();
            let entry = (snapshot.state, snapshot.percentage, snapshot.text);
            if self.seen.last() != Some(&entry) {
                self.seen.push(entry);
            }
            Ok(())
        }
        fn run(&mut self) {
            for _ in 0..20_000 {
                self.cycle().unwrap();
                if self.updater.settled() {
                    return;
                }
            }
            panic!("never settled: {:?}", self.updater.snapshot());
        }
    }
    fn empty_store() -> DiskStore {
        DiskStore::in_memory(Vec::new(), BTreeMap::from([(255, Vec::new())]))
    }

    #[test]
    fn the_stage_table_is_the_original_sequence() {
        assert_eq!(ARCHIVE_WEIGHTS.iter().sum::<i32>(), 100);
        let states: Vec<i32> = STAGES.iter().map(|stage| stage.state).collect();
        assert_eq!(
            states,
            [
                0, 10, 30, 40, 45, 50, 60, 65, 70, 80, 90, 100, 110, 120, 130, 135, 140, 150, 160
            ]
        );
        // The bar never moves backwards.
        let mut last = 0;
        for stage in &STAGES {
            assert!(stage.waiting_percentage >= last, "state {}", stage.state);
            assert!(stage.done_percentage >= stage.waiting_percentage);
            last = stage.done_percentage;
        }
        assert_eq!(PREFETCH_ALL.iter().filter(|&&all| !all).count(), 2);
        assert!(!PREFETCH_ALL[6] && !PREFETCH_ALL[14]);
        assert_eq!(login_crc_sums(&[7; 28])[27..], [7, 0]);
    }

    #[test]
    fn an_empty_directory_is_loaded_in_the_original_order() {
        let mut rig = Rig::new(server(), empty_store());
        rig.run();
        let texts: Vec<&str> = rig.seen.iter().map(|entry| entry.2.as_str()).collect();
        // Every "done" text appears, in table order.
        let mut position = 0;
        for stage in &STAGES {
            if let Some(done) = stage.done {
                let found = texts[position..]
                    .iter()
                    .position(|text| *text == done)
                    .unwrap_or_else(|| panic!("{done} missing or out of order: {texts:?}"));
                position += found;
            }
        }
        assert_eq!(
            rig.seen.first(),
            Some(&(10, 5, "Allocated memory".to_owned()))
        );
        assert!(
            rig.seen
                .contains(&(30, 12, "Connecting to update server".to_owned()))
        );
        assert!(
            rig.seen
                .contains(&(40, 15, "Connected to update server".to_owned()))
        );
        assert!(
            texts
                .iter()
                .any(|text| text.starts_with("Checking for updates - "))
        );
        assert!(
            texts
                .iter()
                .any(|text| text.starts_with("Loading config - "))
        );
        assert_eq!(
            rig.seen.last(),
            Some(&(160, 100, "Started 3d Library".to_owned()))
        );
        // The bar only ever grows.
        assert!(rig.seen.windows(2).all(|pair| pair[0].1 <= pair[1].1));
        // Connected while loading (2), told once that the title screen
        // opened (3) and never told anything else.
        assert_eq!(rig.server.controls, [6, 2, 3]);
        assert_eq!(rig.updater.connection.game_state, 10);
        assert_eq!(rig.server.connections, 1);
        assert_eq!((rig.updater.title_song, rig.updater.login_screen), (5, 2));
        // The first request is the master index, then the 28 indexes.
        assert_eq!(rig.server.requested[0], (255, 255, true));
        let indexes: Vec<_> = (0..28)
            .map(|archive| (255u8, archive as u16, true))
            .collect();
        assert_eq!(rig.server.requested[1..29], indexes);
        // What the load sequence needs is asked for at urgent priority, in
        // sequence order: fonts from 8 then 13, then the title screen.
        let urgent: Vec<_> = rig
            .server
            .requested
            .iter()
            .filter(|request| request.2 && request.0 != 255)
            .map(|request| (request.0, request.1))
            .collect();
        assert_eq!(
            urgent[..8],
            [
                (8, 0),
                (8, 1),
                (8, 2),
                (13, 0),
                (13, 1),
                (13, 2),
                (8, 3),
                (8, 4)
            ]
        );
        // Archives 6 and 14 are verified but not downloaded.
        let fetched = |archive: u8| {
            rig.server
                .requested
                .iter()
                .filter(|request| request.0 == archive)
                .count()
        };
        assert_eq!((fetched(6), fetched(14)), (0, 0));
        // Background only: once each. Archive 26 is also wanted urgently by
        // the sequence, which may ask again for a group already on its way.
        assert_eq!(fetched(7), 10);
        assert!(fetched(26) >= 29);
        // The login block gets the CRC of each index as the server sent it.
        let sums = rig.updater.checksums().unwrap();
        for archive in 0..28u8 {
            assert_eq!(
                sums[usize::from(archive)] as u32,
                rig.server.master_entry(archive).0
            );
        }
        // Everything downloaded is in the store with its version trailer.
        let store = rig.store.lock().unwrap();
        let (data, indexes) = store.memory_bytes().unwrap();
        let snapshot = Store::from_bytes(data, indexes, Limits::default());
        for archive in (0..28u8).filter(|archive| ![6, 14].contains(archive)) {
            let index = snapshot.read(255, u32::from(archive)).unwrap().unwrap();
            assert_eq!(index, rig.server.index_container(archive));
            let index = Index::decode(&index, None, Limits::default()).unwrap();
            for group in &index.groups {
                let stored = snapshot.read(archive, group.id).unwrap().unwrap();
                let (body, trailer) = stored.split_at(stored.len() - 2);
                assert_eq!(body, rig.server.group_container(archive, group.id as u16));
                assert_eq!(trailer, [(group.version >> 8) as u8, group.version as u8]);
            }
        }
        assert_eq!(snapshot.read(6, 5).unwrap(), None);
    }

    #[test]
    fn complete_mode_also_fills_the_on_demand_archives_and_extras() {
        let mut server = server_listing(30);
        server.add_archive(29, 9, &[(0, payload(1, 500), 4), (3, payload(2, 9), 5)]);
        let mut rig = Rig::new(server, empty_store());
        rig.updater.set_complete(true);
        rig.run();
        let fetched = |archive: u8| {
            rig.server
                .requested
                .iter()
                .filter(|request| request.0 == archive)
                .count()
        };
        assert_eq!((fetched(6), fetched(14), fetched(29)), (2, 17, 2));
        // Archive 28 is listed as absent (all zero) and is left alone.
        assert!(!rig.server.requested.contains(&(255, 28, true)));
        let store = rig.store.lock().unwrap();
        assert_eq!(store.archives().count(), 30);
        let (data, indexes) = store.memory_bytes().unwrap();
        let snapshot = Store::from_bytes(data, indexes, Limits::default());
        assert!(snapshot.read(29, 3).unwrap().is_some());
        assert!(snapshot.read(6, 5).unwrap().is_some());
        assert!(indexes_empty(&snapshot, 28));
    }
    fn indexes_empty(store: &Store, archive: u8) -> bool {
        store.group_slots(archive) == 0 && store.read(255, u32::from(archive)).unwrap().is_none()
    }

    #[test]
    fn a_second_start_trusts_the_store_and_repairs_what_was_damaged() {
        let mut rig = Rig::new(server(), empty_store());
        rig.updater.set_complete(true);
        rig.run();
        let (mut data, indexes) = rig.store.lock().unwrap().memory_bytes().unwrap();
        // Damage the first sector of three groups the load sequence itself
        // never asks for: payload bytes, a chain link, and the whole sector.
        let victims = [(0u8, 2u16), (7, 4), (9, 0)];
        for (case, (archive, group)) in victims.into_iter().enumerate() {
            let entry = &indexes[&archive][usize::from(group) * 6..][..6];
            let sector =
                usize::from(entry[3]) << 16 | usize::from(entry[4]) << 8 | usize::from(entry[5]);
            let range = match case {
                0 => sector * 520 + 14..sector * 520 + 15,
                1 => sector * 520 + 2..sector * 520 + 4,
                _ => sector * 520..sector * 520 + 48,
            };
            for byte in &mut data[range] {
                *byte ^= 0xa5;
            }
        }
        let mut second = Rig::new(server(), DiskStore::in_memory(data, indexes));
        second.updater.set_complete(true);
        second.run();
        // Only the master index and the three damaged groups came over the
        // wire; every index and every other group was trusted as stored.
        assert_eq!(second.server.requested[0], (255, 255, true));
        let mut fetched = second.server.requested[1..].to_vec();
        fetched.sort();
        assert_eq!(
            fetched,
            victims.map(|(archive, group)| (archive, group, false))
        );
        let store = second.store.lock().unwrap();
        let (data, indexes) = store.memory_bytes().unwrap();
        let repaired = Store::from_bytes(data, indexes, Limits::default());
        for archive in 0..28u8 {
            let index = repaired.read(255, u32::from(archive)).unwrap().unwrap();
            let index = Index::decode(&index, None, Limits::default()).unwrap();
            for group in &index.groups {
                let stored = repaired.read(archive, group.id).unwrap().unwrap();
                assert_eq!(crc32(&stored[..stored.len() - 2]), group.checksum);
            }
        }
    }

    #[test]
    fn an_unreachable_server_stops_the_client_with_js5connect() {
        let opener = ScriptedOpener::default();
        let mut updater = Updater::new(
            Box::new(opener.clone()),
            43594,
            43595,
            None,
            CacheQueue::manual(),
        );
        let mut result = Ok(());
        let mut cycles = 0;
        while result.is_ok() && cycles < 5000 {
            cycles += 1;
            result = updater.cycle(cycles * 20);
        }
        assert_eq!(result, Err(UpdateError::Fatal(FatalError::Connect)));
        // The master index is asked for in cycle 3; attempts follow in
        // cycles 4, 5, 256 and 757, and cycle 758 gives up.
        assert_eq!(cycles, 758);
        assert_eq!(opener.0.lock().unwrap().ports, [43594, 43595, 43594, 43595]);
        assert_eq!(updater.snapshot().text, "Connecting to update server");
        assert_eq!(updater.snapshot().percentage, 12);
    }

    #[test]
    fn a_missing_archive_and_a_failed_world_list_are_reported() {
        let mut rig = Rig::new(FakeServer::new(20), empty_store());
        let mut result = Ok(());
        for _ in 0..50 {
            result = rig.cycle();
            if result.is_err() {
                break;
            }
        }
        assert_eq!(result, Err(UpdateError::MasterIndex(20)));
        let mut rig = Rig::new(server(), empty_store());
        rig.updater.world_list = WorldList::Pending;
        for _ in 0..3000 {
            rig.cycle().unwrap();
        }
        assert_eq!(rig.updater.main_load_state(), 135);
        assert_eq!(rig.updater.snapshot().text, "Loading world list data");
        assert!(!rig.updater.loaded());
        rig.updater.world_list = WorldList::Failed(1001);
        assert_eq!(
            rig.cycle(),
            Err(UpdateError::WorldList("worldlistio_1001".into()))
        );
    }

    #[test]
    fn the_hook_runs_only_when_a_server_is_named_and_the_cache_is_absent_or_forced() {
        let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let missing = "runs/no-such-cache-directory";
        assert_eq!(requested(&args(&["client"])), Ok(None));
        assert_eq!(requested(&args(&["client", "--cache", missing])), Ok(None));
        assert!(requested(&args(&["client", "--update-cache"])).is_err());
        assert_eq!(
            requested(&args(&[
                "client",
                "--cache",
                missing,
                "--connect",
                "127.0.0.1:43594"
            ])),
            Ok(Some(Request {
                host: "127.0.0.1".into(),
                port: 43594,
                directory: missing.into(),
            }))
        );
        assert_eq!(
            requested(&args(&[
                "client",
                "--update-cache",
                "--update-server",
                "[::1]:40000",
                "--connect",
                "1.2.3.4:5",
            ]))
            .unwrap()
            .map(|request| (request.host, request.port)),
            Some(("::1".into(), 40000))
        );
        assert!(requested(&args(&["client", "--update-cache", "--connect", "nohost"])).is_err());
    }
}
