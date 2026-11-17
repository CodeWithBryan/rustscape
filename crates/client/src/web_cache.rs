//! The cache of a client that has no files: held in memory, filled by the
//! update connection, and mirrored to whatever storage the host offers.
//!
//! The original applet downloads the cache from the game server over JS5 and
//! keeps it in `main_file_cache.dat2`/`.idx*` on disk; on the next start it
//! verifies those files and fetches only what is missing or damaged. The
//! browser build does the same with the same parts: [`Updater`] (the ported
//! start-up sequence and resource providers) writes through the ported
//! `Cache` into a [`DiskStore`] in memory, and [`Mirror`] copies the pieces
//! of those two kinds of file that changed into host storage (IndexedDB),
//! from where the next start restores them for the usual verification.
//!
//! What differs from the original, and why:
//! * The files live in memory, about 92 MB once everything has arrived; a
//!   page has no file to seek in, and the client reads the cache
//!   synchronously.
//! * [`WebCache::ready`] holds the first frame of the game back until every
//!   archive the original prefetches in the background has arrived. The
//!   original opens its login screen after the start-up sequence and lets
//!   scene building wait for whatever is still missing; this client's scene
//!   and interface code reads packs without a "not there yet" answer, so it
//!   is started only when nothing it reads can be missing. Songs (archive 6)
//!   and Vorbis samples (archive 14) are fetched when a song needs them, as
//!   in the original ([`WebCache::group_ready`]).
//! * Storage is written in pieces of [`MIRROR_CHUNK`] bytes every few
//!   seconds rather than sector by sector. A page closed mid-download leaves
//!   pieces and indexes out of step; the next start's verification treats
//!   whatever does not check out as missing, as it does for a damaged file.
//!
//! This module has no browser code in it: sockets come through
//! [`rs09_net::js5_stream`] and storage through [`Blobs`], so all of it runs
//! in native tests against the scripted update server.
use crate::cache_update::{ARCHIVE_COUNT, PREFETCH_ALL, Snapshot, UpdateError, Updater};
use rs09_cache::{
    Limits,
    disk::{DATA_MAX_LENGTH, DiskStore, INDEX_MAX_LENGTH, MIRROR_CHUNK},
    store::Store,
};
use rs09_net::{js5_client::SocketOpener, js5_provider::CacheQueue};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

/// Bumped when the stored layout changes; other values are discarded.
pub const STORAGE_FORMAT: u32 = 1;
const META: &str = "meta";
/// A stored data file this close to the original's size limit is thrown
/// away, as `FileOnDisk` deletes one that has reached it: failed downloads
/// leave dead sectors behind, and a full file accepts no more writes.
const DATA_DISCARD_MARGIN: u64 = 2 * MIRROR_CHUNK as u64;

fn data_key(chunk: u32) -> String {
    format!("dat2/{chunk:05}")
}
fn index_key(archive: u8) -> String {
    format!("idx{archive}")
}

/// Splits a WebSocket address into what goes before the port, the port, and
/// what follows it: the original's connect logic chooses the port, so the
/// address is put together again for each attempt. A missing port is the
/// scheme's default. `None` for anything that is not `ws://` or `wss://`.
pub fn split_endpoint(url: &str) -> Option<(String, u16, String)> {
    let (scheme, default_port) = if url.starts_with("ws://") {
        ("ws://", 80)
    } else if url.starts_with("wss://") {
        ("wss://", 443)
    } else {
        return None;
    };
    let rest = &url[scheme.len()..];
    let (authority, path) = match rest.find('/') {
        Some(slash) => rest.split_at(slash),
        None => (rest, "/"),
    };
    // An IPv6 host is bracketed, and its colons are not the port's.
    let host_end = authority.rfind(']').map_or(0, |bracket| bracket + 1);
    let (host, port) = match authority[host_end..].rfind(':') {
        Some(colon) => {
            let colon = host_end + colon;
            (&authority[..colon], authority[colon + 1..].parse().ok()?)
        }
        None => (authority, default_port),
    };
    if host.is_empty() {
        return None;
    }
    Some((format!("{scheme}{host}"), port, path.to_owned()))
}

/// What [`Blobs::poll`] found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stored {
    /// Still being read.
    Pending,
    /// The host has no usable storage; the cache is kept in memory only.
    Unavailable,
    /// Every stored key with its length.
    Ready(Vec<(String, usize)>),
}

/// Named byte strings that outlive the page: the host's stand-in for the
/// cache directory. Writes are staged with [`Blobs::put`] and made durable
/// together by [`Blobs::commit`].
pub trait Blobs: Send {
    fn poll(&mut self) -> Stored;
    /// Copies the first `destination.len()` bytes of a stored value; `false`
    /// when the key is absent or shorter.
    fn read(&mut self, key: &str, destination: &mut [u8]) -> bool;
    /// The values [`Blobs::poll`] made readable are no longer needed.
    fn release(&mut self);
    fn put(&mut self, key: &str, bytes: &[u8]);
    /// Starts writing everything put since the last commit, all or nothing.
    /// `false` when the host could not even start.
    fn commit(&mut self) -> bool;
    /// An earlier commit has not finished.
    fn busy(&self) -> bool;
    /// A commit failed (storage full, storage withdrawn).
    fn failed(&self) -> bool;
    /// Forgets everything stored.
    fn clear(&mut self);
}

/// Rebuilds the store from storage. `None` when nothing usable is stored: no
/// record of a store, another layout, or a data file at the size limit. The
/// result is not trusted; the update sequence verifies every group in it.
/// `reserve` is the capacity given to the data file's bytes up front.
pub fn restore(
    blobs: &mut dyn Blobs,
    listing: &[(String, usize)],
    reserve: usize,
) -> Option<DiskStore> {
    let length = |key: &str| listing.iter().find(|entry| entry.0 == key).map(|e| e.1);
    let mut meta = vec![0; length(META).filter(|&length| length <= 4096)?];
    if !blobs.read(META, &mut meta) {
        return None;
    }
    let meta: serde_json::Value = serde_json::from_slice(&meta).ok()?;
    if meta["format"].as_u64()? != u64::from(STORAGE_FORMAT)
        || meta["chunk"].as_u64()? != MIRROR_CHUNK as u64
    {
        return None;
    }
    let data_length = meta["data_length"].as_u64()?;
    if data_length >= DATA_MAX_LENGTH - DATA_DISCARD_MARGIN {
        return None;
    }
    let data_length = data_length as usize;
    let mut data = Vec::with_capacity(reserve.max(data_length));
    data.resize(data_length, 0);
    let mut indexes = BTreeMap::new();
    for (key, stored) in listing {
        if let Some(chunk) = key
            .strip_prefix("dat2/")
            .and_then(|chunk| chunk.parse::<usize>().ok())
        {
            // A piece that is absent or short stays zero: its groups fail
            // verification and are fetched again.
            let Some(start) = chunk
                .checked_mul(MIRROR_CHUNK)
                .filter(|&start| start < data_length)
            else {
                continue;
            };
            let end = data_length.min(start + (*stored).min(MIRROR_CHUNK));
            blobs.read(key, &mut data[start..end]);
        } else if let Some(archive) = key
            .strip_prefix("idx")
            .and_then(|archive| archive.parse::<u8>().ok())
        {
            if *stored as u64 >= INDEX_MAX_LENGTH {
                continue;
            }
            let mut bytes = vec![0; *stored];
            if blobs.read(key, &mut bytes) {
                indexes.insert(archive, bytes);
            }
        }
    }
    Some(DiskStore::in_memory(data, indexes))
}

/// Copies what the store's writes touched into [`Blobs`].
pub struct Mirror {
    blobs: Box<dyn Blobs>,
    /// Commits started, and the pieces and bytes they carried.
    pub commits: u64,
    pub pieces: u64,
    pub bytes: u64,
    pub failed: bool,
}
impl Mirror {
    pub fn new(blobs: Box<dyn Blobs>) -> Self {
        Self {
            blobs,
            commits: 0,
            pieces: 0,
            bytes: 0,
            failed: false,
        }
    }
    /// Stages and commits every piece written since the last call. Returns
    /// `false` while an earlier commit is still being written (nothing is
    /// taken from the store then) and after storage has failed.
    pub fn flush(&mut self, store: &mut DiskStore) -> bool {
        if self.failed || self.blobs.failed() {
            self.failed = true;
            return false;
        }
        if self.blobs.busy() {
            return false;
        }
        let (chunks, indexes) = store.take_dirty();
        if chunks.is_empty() && indexes.is_empty() {
            return true;
        }
        for chunk in chunks {
            if let Some(piece) = store.data_chunk(chunk) {
                self.blobs.put(&data_key(chunk), piece);
                self.pieces += 1;
                self.bytes += piece.len() as u64;
            }
        }
        for archive in indexes {
            if let Some(bytes) = store.index_bytes(archive) {
                self.blobs.put(&index_key(archive), bytes);
                self.pieces += 1;
                self.bytes += bytes.len() as u64;
            }
        }
        let meta = format!(
            "{{\"format\":{STORAGE_FORMAT},\"chunk\":{MIRROR_CHUNK},\"data_length\":{}}}",
            store.data_length()
        );
        self.blobs.put(META, meta.as_bytes());
        self.commits += 1;
        if !self.blobs.commit() {
            self.failed = true;
        }
        !self.failed
    }
    pub fn busy(&self) -> bool {
        self.blobs.busy()
    }
}

/// How far the background download is, over the archives the original
/// prefetches (all but 6 and 14).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub groups: usize,
    pub verified: usize,
}

pub struct WebCache {
    pub updater: Updater,
    store: Arc<Mutex<DiskStore>>,
    mirror: Option<Mirror>,
    /// Bytes of data file restored from storage at start.
    pub restored_bytes: u64,
    /// Groups asked for through [`Self::group_ready`] before they were in.
    pub on_demand: BTreeSet<(u8, u32)>,
    /// Those of them that have since arrived.
    pub on_demand_arrived: BTreeSet<(u8, u32)>,
    last_flush_ms: u64,
    error: Option<String>,
}
impl WebCache {
    /// `store` is the restored store or an empty one; `blobs` where it is
    /// mirrored, if anywhere. The update server is reached through `opener`
    /// on `port` (the original alternates between two ports; a page is given
    /// one address).
    pub fn new(
        opener: Box<dyn SocketOpener>,
        port: u16,
        mut store: DiskStore,
        blobs: Option<Box<dyn Blobs>>,
    ) -> Self {
        let restored_bytes = store.data_length();
        // `SignLink` opens the data file, idx0 to idx27 and idx255.
        for archive in (0..ARCHIVE_COUNT as u8).chain([255]) {
            let _ = store.ensure_archive(archive);
        }
        // What was restored is in storage already.
        store.take_dirty();
        let store = Arc::new(Mutex::new(store));
        let updater = Updater::new(
            opener,
            port,
            port,
            Some(Arc::clone(&store)),
            CacheQueue::manual(),
        );
        Self {
            updater,
            store,
            mirror: blobs.map(Mirror::new),
            restored_bytes,
            on_demand: BTreeSet::new(),
            on_demand_arrived: BTreeSet::new(),
            last_flush_ms: 0,
            error: None,
        }
    }
    /// One client cycle (20 ms): the original's update-server loop, resource
    /// providers and start-up sequence, then the queued store work, which
    /// has no thread of its own here.
    pub fn cycle(&mut self, now_ms: u64) -> Result<(), UpdateError> {
        let result = self.updater.cycle(now_ms);
        self.updater.flush_store();
        if let Err(error) = &result {
            self.error = Some(error.to_string());
        }
        result
    }
    /// Not in the original: another network and provider pass without
    /// counting a cycle, so a download is not held to 100 reads per 20 ms.
    pub fn io_pass(&mut self, now_ms: u64) -> Result<(), UpdateError> {
        let result = self.updater.io_pass(now_ms);
        self.updater.flush_store();
        if let Err(error) = &result {
            self.error = Some(error.to_string());
        }
        result
    }
    /// The start-up sequence has finished and every background archive has
    /// been verified or downloaded: everything but songs and Vorbis samples
    /// is in the store.
    pub fn ready(&self) -> bool {
        self.updater.settled()
    }
    pub fn snapshot(&self) -> Snapshot {
        self.updater.snapshot()
    }
    pub fn progress(&self) -> Progress {
        let mut progress = Progress::default();
        if let Some(master) = self.updater.master() {
            for provider in master.providers() {
                let archive = usize::from(provider.archive());
                if PREFETCH_ALL.get(archive).copied().unwrap_or(false) {
                    progress.groups += provider.index_size();
                    progress.verified += provider.verified_groups();
                }
            }
        }
        progress
    }
    /// A reading view of the store as it is now and as it fills.
    pub fn store(&self) -> Store {
        Store::live(Arc::clone(&self.store), Limits::default())
    }
    /// `Js5.isGroupReady`: whether a group can be read from the store. One
    /// that is not there is asked of the server at urgent priority; call
    /// again on a later cycle.
    pub fn group_ready(&mut self, archive: u8, group: u32) -> bool {
        let Ok(id) = i32::try_from(group) else {
            return false;
        };
        let ready = self
            .updater
            .group_ready(usize::from(archive), id)
            .unwrap_or(false);
        if ready {
            // A group that has just arrived is still on its way to the store.
            self.updater.flush_store();
            if self.on_demand.contains(&(archive, group)) {
                self.on_demand_arrived.insert((archive, group));
            }
        } else if self
            .updater
            .master()
            .and_then(|master| master.provider(archive))
            .is_some_and(|provider| provider.group_status(group) == -1)
        {
            self.on_demand.insert((archive, group));
        }
        ready
    }
    /// The 28 index CRCs the login block carries.
    pub fn checksums(&self) -> Option<[i32; ARCHIVE_COUNT]> {
        self.updater.checksums()
    }
    /// Mirrors what has been written since the last flush, at most every
    /// `interval_ms` unless `now` is forced. Returns whether a flush ran.
    pub fn mirror(&mut self, now_ms: u64, interval_ms: u64, force: bool) -> bool {
        let Some(mirror) = &mut self.mirror else {
            return false;
        };
        if !force && now_ms.saturating_sub(self.last_flush_ms) < interval_ms {
            return false;
        }
        let Ok(mut store) = self.store.lock() else {
            return false;
        };
        if mirror.flush(&mut store) {
            self.last_flush_ms = now_ms;
            true
        } else {
            false
        }
    }
    /// Whether everything written is in storage (or there is no storage).
    pub fn mirrored(&self) -> bool {
        match &self.mirror {
            None => true,
            Some(mirror) if mirror.failed => true,
            Some(mirror) => !mirror.busy(),
        }
    }
    pub fn data_length(&self) -> u64 {
        self.store.lock().map_or(0, |store| store.data_length())
    }
    pub fn status(&self) -> serde_json::Value {
        let snapshot = self.snapshot();
        let progress = self.progress();
        let queue = &self.updater.connection.queue;
        serde_json::json!({
            "state": snapshot.state,
            "text": snapshot.text,
            "percentage": snapshot.percentage,
            "loaded": snapshot.loaded,
            "ready": self.ready(),
            "connected": snapshot.connected,
            "errors": snapshot.errors,
            "response": snapshot.response,
            "error": self.error,
            "bytes_received": snapshot.bytes_received,
            "replies_received": snapshot.replies_received,
            "groups": progress.groups,
            "groups_verified": progress.verified,
            "listed_groups": snapshot.groups,
            "listed_groups_verified": snapshot.groups_verified,
            "urgent_requests": queue.urgent_request_count(),
            "prefetch_requests": queue.prefetch_request_count(),
            "data_bytes": self.data_length(),
            "restored_bytes": self.restored_bytes,
            "on_demand_groups": self.on_demand.len(),
            "on_demand_arrived": self.on_demand_arrived.len(),
            "storage": self.mirror.as_ref().map(|mirror| serde_json::json!({
                "commits": mirror.commits,
                "pieces": mirror.pieces,
                "bytes": mirror.bytes,
                "busy": mirror.busy(),
                "failed": mirror.failed,
            })),
        })
    }
}

/// [`Blobs`] in memory, for tests: clones share one storage, and a commit
/// can be held back to stand for a write that has not finished.
pub mod memory {
    use super::{Blobs, Stored};
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
    };

    #[derive(Default)]
    pub struct Shelf {
        pub stored: BTreeMap<String, Vec<u8>>,
        staged: Vec<(String, Vec<u8>)>,
        /// Committed but not yet written; [`MemoryBlobs::finish`] writes it.
        writing: Vec<(String, Vec<u8>)>,
        pub hold_commits: bool,
        pub unavailable: bool,
        pub failed: bool,
        pub commits: usize,
        pub released: bool,
    }
    #[derive(Clone, Default)]
    pub struct MemoryBlobs(pub Arc<Mutex<Shelf>>);
    impl MemoryBlobs {
        pub fn shelf(&self) -> std::sync::MutexGuard<'_, Shelf> {
            self.0.lock().expect("shelf")
        }
        /// Completes a held commit.
        pub fn finish(&self) {
            let mut shelf = self.shelf();
            let writing = std::mem::take(&mut shelf.writing);
            shelf.stored.extend(writing);
        }
        /// The page went away: staged and unfinished writes are lost.
        pub fn crash(&self) {
            let mut shelf = self.shelf();
            shelf.staged.clear();
            shelf.writing.clear();
        }
    }
    impl Blobs for MemoryBlobs {
        fn poll(&mut self) -> Stored {
            let shelf = self.shelf();
            if shelf.unavailable {
                return Stored::Unavailable;
            }
            Stored::Ready(
                shelf
                    .stored
                    .iter()
                    .map(|(key, value)| (key.clone(), value.len()))
                    .collect(),
            )
        }
        fn read(&mut self, key: &str, destination: &mut [u8]) -> bool {
            match self.shelf().stored.get(key) {
                Some(value) if value.len() >= destination.len() => {
                    destination.copy_from_slice(&value[..destination.len()]);
                    true
                }
                _ => false,
            }
        }
        fn release(&mut self) {
            self.shelf().released = true;
        }
        fn put(&mut self, key: &str, bytes: &[u8]) {
            self.shelf().staged.push((key.to_owned(), bytes.to_vec()));
        }
        fn commit(&mut self) -> bool {
            let mut shelf = self.shelf();
            if shelf.failed {
                return false;
            }
            shelf.commits += 1;
            let staged = std::mem::take(&mut shelf.staged);
            if shelf.hold_commits {
                shelf.writing.extend(staged);
            } else {
                shelf.stored.extend(staged);
            }
            true
        }
        fn busy(&self) -> bool {
            !self.shelf().writing.is_empty()
        }
        fn failed(&self) -> bool {
            self.shelf().failed
        }
        fn clear(&mut self) {
            let mut shelf = self.shelf();
            shelf.stored.clear();
            shelf.staged.clear();
            shelf.writing.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    //! No part of the original is transcribed here; the tests establish that
    //! the browser arrangement (message socket, memory store, mirrored
    //! storage) downloads, restores and repairs like the desktop one, against
    //! the scripted update server.
    use super::{memory::MemoryBlobs, *};
    use rs09_cache::index::Index;
    use rs09_net::{
        js5_client::scripted::{FakeServer, ScriptedSocket},
        js5_stream::{StreamOpener, scripted::PipePort},
    };

    fn payload(seed: u32, length: usize) -> Vec<u8> {
        (0..length).map(|i| (i as u32 * 31 + seed) as u8).collect()
    }
    /// 28 archives shaped where the start-up sequence looks, with enough
    /// bulk in archive 7 to run across several mirror pieces.
    fn server() -> FakeServer {
        let mut server = FakeServer::new(ARCHIVE_COUNT);
        for archive in 0..ARCHIVE_COUNT as u8 {
            let version = 100 + i32::from(archive);
            match archive {
                8 => {
                    let names = [
                        "p11_full",
                        "p12_full",
                        "b12_full",
                        "titlebg",
                        "logo",
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
                        "runes",
                    ];
                    let groups: Vec<_> = names
                        .iter()
                        .enumerate()
                        .map(|(id, name)| (id as u32, *name, payload(id as u32, 700), 7))
                        .collect();
                    server.add_named_archive(8, version, &groups);
                }
                13 => {
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
                14 => {
                    let groups: Vec<_> = (0..3).map(|id| (id, payload(id + 40, 5000), 2)).collect();
                    server.add_archive(14, version, &groups);
                }
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
                7 => {
                    let groups: Vec<_> = (0..40)
                        .map(|id| (id, payload(id, 60_000 + id as usize * 911), 1 + id as i32))
                        .collect();
                    server.add_archive(7, version, &groups);
                }
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
        cache: WebCache,
        server: FakeServer,
        pipes: Arc<Mutex<Vec<PipePort>>>,
        served: usize,
        now: u64,
    }
    impl Rig {
        fn new(server: FakeServer, store: DiskStore, blobs: Option<MemoryBlobs>) -> Self {
            let pipes = Arc::new(Mutex::new(Vec::<PipePort>::new()));
            let made = Arc::clone(&pipes);
            let opener = StreamOpener::new(move |_| {
                // Messages of 777 bytes: no reply ends on a boundary.
                let pipe = PipePort::new(ScriptedSocket::new(), 777);
                made.lock().unwrap().push(pipe.clone());
                Some(pipe)
            });
            let mut cache = WebCache::new(
                Box::new(opener),
                43594,
                store,
                blobs.map(|blobs| Box::new(blobs) as Box<dyn Blobs>),
            );
            cache.updater.set_random(Box::new(|| 0.5));
            Self {
                cache,
                server,
                pipes,
                served: 0,
                now: 0,
            }
        }
        fn serve(&mut self) {
            let pipes = self.pipes.lock().unwrap();
            if pipes.len() > self.served.max(1) {
                self.server.reconnected();
            }
            self.served = pipes.len();
            if let Some(pipe) = pipes.last() {
                self.server.serve(&pipe.wire);
            }
        }
        fn cycle(&mut self) {
            self.now += 20;
            self.cache.cycle(self.now).unwrap();
            self.serve();
            // Two extra passes a cycle, as the host makes while loading.
            for _ in 0..2 {
                self.cache.io_pass(self.now).unwrap();
                self.serve();
            }
        }
        fn run(&mut self) -> u64 {
            let mut cycles = 0;
            while !self.cache.ready() {
                self.cycle();
                self.cache.mirror(self.now, 500, false);
                cycles += 1;
                assert!(cycles < 50_000, "{}", self.cache.status());
            }
            self.cache.mirror(self.now, 0, true);
            cycles
        }
    }
    fn empty() -> DiskStore {
        DiskStore::in_memory(Vec::new(), BTreeMap::new())
    }
    fn reopen(blobs: &MemoryBlobs) -> Option<DiskStore> {
        let mut blobs = blobs.clone();
        let Stored::Ready(listing) = blobs.poll() else {
            return None;
        };
        let store = restore(&mut blobs, &listing, 0);
        blobs.release();
        store
    }
    fn requested(server: &FakeServer, archive: u8) -> usize {
        server
            .requested
            .iter()
            .filter(|request| request.0 == archive)
            .count()
    }

    #[test]
    fn an_empty_browser_fills_its_store_and_storage_holds_the_same_files() {
        let blobs = MemoryBlobs::default();
        let mut rig = Rig::new(server(), empty(), Some(blobs.clone()));
        assert!(!rig.cache.ready());
        rig.run();
        assert_eq!(rig.cache.restored_bytes, 0);
        // Everything the original prefetches is there; songs and Vorbis
        // samples were verified absent and left alone.
        let progress = rig.cache.progress();
        assert_eq!(progress.groups, progress.verified);
        assert_eq!(
            (requested(&rig.server, 6), requested(&rig.server, 14)),
            (0, 0)
        );
        assert_eq!(requested(&rig.server, 7), 40);
        let view = rig.cache.store();
        for archive in (0..28u8).filter(|archive| ![6, 14].contains(archive)) {
            let index = view.read(255, u32::from(archive)).unwrap().unwrap();
            assert_eq!(index, rig.server.index_container(archive));
            let index = Index::decode(&index, None, Limits::default()).unwrap();
            for group in &index.groups {
                let stored = view.read(archive, group.id).unwrap().unwrap();
                let (body, trailer) = stored.split_at(stored.len() - 2);
                assert_eq!(body, rig.server.group_container(archive, group.id as u16));
                assert_eq!(trailer, [(group.version >> 8) as u8, group.version as u8]);
            }
        }
        assert_eq!(view.read(6, 5).unwrap(), None);
        // The data file ran over several pieces and storage has all of it:
        // put together again it is byte for byte the store in memory.
        let (data, indexes) = rig.cache.store.lock().unwrap().memory_bytes().unwrap();
        assert!(data.len() > 2 * MIRROR_CHUNK);
        let restored = reopen(&blobs).expect("a store in storage");
        let (restored_data, restored_indexes) = restored.memory_bytes().unwrap();
        assert_eq!(restored_data, data);
        // An index file nothing was written to (archives 6 and 14) is not
        // stored; it is created empty again at the next start.
        let written: BTreeMap<u8, Vec<u8>> = indexes
            .into_iter()
            .filter(|(_, bytes)| !bytes.is_empty())
            .collect();
        assert_eq!(restored_indexes, written);
        assert!(blobs.shelf().released);
        assert!(blobs.shelf().commits > 1, "mirrored while downloading");
        let status = rig.cache.status();
        assert_eq!(status["ready"], true);
        assert_eq!(status["storage"]["failed"], false);
        assert_eq!(status["data_bytes"], data.len() as u64);
    }

    #[test]
    fn a_reload_verifies_the_stored_cache_and_downloads_nothing() {
        let blobs = MemoryBlobs::default();
        let mut first = Rig::new(server(), empty(), Some(blobs.clone()));
        first.run();
        let downloaded = first.cache.snapshot().bytes_received;
        let store = reopen(&blobs).expect("a store in storage");
        let mut second = Rig::new(server(), store, Some(blobs.clone()));
        assert!(second.cache.restored_bytes > 2 * MIRROR_CHUNK as u64);
        let commits = blobs.shelf().commits;
        second.run();
        // The master index is the only thing asked of the server.
        assert_eq!(second.server.requested, [(255, 255, true)]);
        let snapshot = second.cache.snapshot();
        assert_eq!(snapshot.replies_received, 1);
        assert!(snapshot.bytes_received < 400 && downloaded > 2_000_000);
        let progress = second.cache.progress();
        assert_eq!(progress.groups, progress.verified);
        // Nothing was written, so nothing was mirrored again.
        assert_eq!(blobs.shelf().commits, commits);
        assert_eq!(
            second.cache.checksums().unwrap(),
            first.cache.checksums().unwrap()
        );
    }

    #[test]
    fn a_page_closed_mid_download_resumes_and_repairs_from_what_was_stored() {
        let blobs = MemoryBlobs::default();
        let mut first = Rig::new(server(), empty(), Some(blobs.clone()));
        // Stop partway through archive 7, with the last mirror some time ago.
        let mut cycles = 0;
        while first.cache.progress().verified < 60 {
            first.cycle();
            first.cache.mirror(first.now, 300, false);
            cycles += 1;
            assert!(cycles < 50_000);
        }
        assert!(!first.cache.ready());
        blobs.crash();
        let stored = reopen(&blobs).expect("a partial store in storage");
        // Damage a piece as well: a group in it must be fetched again.
        let (mut data, indexes) = stored.memory_bytes().unwrap();
        let middle = data.len() / 2;
        data[middle] ^= 0xff;
        let mut second = Rig::new(
            server(),
            DiskStore::in_memory(data, indexes),
            Some(blobs.clone()),
        );
        second.run();
        // The indexes reached storage, so only the master index is asked
        // for again; of archive 7 only what had not been stored yet and the
        // group the damage hit.
        assert_eq!(requested(&second.server, 255), 1);
        let again = requested(&second.server, 7);
        assert!(again > 0 && again < 40, "{again}");
        let view = second.cache.store();
        for group in 0..40u32 {
            let stored = view.read(7, group).unwrap().unwrap();
            assert_eq!(
                stored[..stored.len() - 2],
                second.server.group_container(7, group as u16)
            );
        }
        // And storage now restores to a store that needs nothing.
        let store = reopen(&blobs).expect("a store in storage");
        let mut third = Rig::new(server(), store, Some(blobs.clone()));
        third.run();
        assert_eq!(third.server.requested, [(255, 255, true)]);
    }

    #[test]
    fn songs_are_fetched_when_asked_for_and_kept() {
        let blobs = MemoryBlobs::default();
        let mut rig = Rig::new(server(), empty(), Some(blobs.clone()));
        // Before the indexes are in, nothing is ready and nothing is asked.
        assert!(!rig.cache.group_ready(6, 5));
        rig.run();
        rig.server.reset_counters();
        assert_eq!(rig.cache.store().read(6, 5).unwrap(), None);
        let mut cycles = 0;
        while !rig.cache.group_ready(6, 5) {
            rig.cycle();
            cycles += 1;
            assert!(cycles < 1000);
        }
        // One urgent request, as `Js5.isGroupReady` makes.
        assert_eq!(rig.server.requested, [(6, 5, true)]);
        let stored = rig.cache.store().read(6, 5).unwrap().unwrap();
        assert_eq!(stored[..stored.len() - 2], rig.server.group_container(6, 5));
        assert_eq!(rig.cache.on_demand.len(), 1);
        assert_eq!(rig.cache.status()["on_demand_arrived"], 1);
        // Asking again costs nothing; a group the index does not list and
        // an archive the client does not have are never ready.
        assert!(rig.cache.group_ready(6, 5));
        assert!(!rig.cache.group_ready(6, 3));
        assert!(!rig.cache.group_ready(40, 0));
        rig.cycle();
        assert_eq!(rig.server.requested.len(), 1);
        // A group already in the store is ready at once, without the server.
        assert!(rig.cache.group_ready(7, 3));
        assert_eq!(rig.server.requested.len(), 1);
        // The song is mirrored and survives a reload.
        assert!(rig.cache.mirror(rig.now, 0, true));
        let store = reopen(&blobs).expect("a store in storage");
        let mut again = Rig::new(server(), store, Some(blobs));
        again.run();
        assert!(again.cache.group_ready(6, 5));
        assert_eq!(again.server.requested, [(255, 255, true)]);
    }

    #[test]
    fn storage_that_is_slow_absent_or_failing_never_stops_the_cache() {
        // A commit still being written holds the next flush back, and the
        // pieces written meanwhile go out with the one after.
        let blobs = MemoryBlobs::default();
        blobs.shelf().hold_commits = true;
        let mut rig = Rig::new(server(), empty(), Some(blobs.clone()));
        let mut held = 0;
        while !rig.cache.ready() {
            rig.cycle();
            if !rig.cache.mirror(rig.now, 100, false) && blobs.shelf().commits > 0 {
                held += 1;
            }
            if rig.now % 2000 == 0 {
                blobs.finish();
            }
        }
        assert!(held > 0);
        assert!(!rig.cache.mirrored() || blobs.shelf().commits > 0);
        blobs.finish();
        assert!(rig.cache.mirror(rig.now, 0, true));
        blobs.finish();
        assert!(rig.cache.mirrored());
        let (data, _) = rig.cache.store.lock().unwrap().memory_bytes().unwrap();
        assert_eq!(reopen(&blobs).unwrap().memory_bytes().unwrap().0, data);

        // No storage at all: the cache is complete in memory.
        let mut rig = Rig::new(server(), empty(), None);
        rig.run();
        assert!(!rig.cache.mirror(rig.now, 0, true));
        assert!(rig.cache.mirrored());
        assert_eq!(rig.cache.status()["storage"], serde_json::Value::Null);

        // Storage that fails is given up on; the download carries on.
        let blobs = MemoryBlobs::default();
        let mut rig = Rig::new(server(), empty(), Some(blobs.clone()));
        for _ in 0..40 {
            rig.cycle();
        }
        blobs.shelf().failed = true;
        rig.run();
        assert_eq!(rig.cache.status()["storage"]["failed"], true);
        assert!(rig.cache.mirrored());
    }

    #[test]
    fn an_address_is_split_around_its_port() {
        let split = |url| split_endpoint(url).map(|(a, port, b)| format!("{a}|{port}|{b}"));
        assert_eq!(
            split("ws://127.0.0.1:43597/").as_deref(),
            Some("ws://127.0.0.1|43597|/")
        );
        assert_eq!(
            split("wss://play.example.org/game?x=1").as_deref(),
            Some("wss://play.example.org|443|/game?x=1")
        );
        assert_eq!(split("ws://host").as_deref(), Some("ws://host|80|/"));
        assert_eq!(
            split("ws://[::1]:8080/a").as_deref(),
            Some("ws://[::1]|8080|/a")
        );
        assert_eq!(split("ws://[::1]").as_deref(), Some("ws://[::1]|80|/"));
        for bad in [
            "http://host:1/",
            "ws://:9/",
            "ws://host:port/",
            "ws://host:70000/",
            "",
        ] {
            assert_eq!(split(bad), None, "{bad}");
        }
    }

    #[test]
    fn storage_of_another_layout_or_at_the_size_limit_is_not_restored() {
        let mut blobs = MemoryBlobs::default();
        assert_eq!(blobs.poll(), Stored::Ready(vec![]));
        assert!(reopen(&blobs).is_none());
        let meta = |format: u32, chunk: usize, length: u64| {
            format!("{{\"format\":{format},\"chunk\":{chunk},\"data_length\":{length}}}")
                .into_bytes()
        };
        let store = |meta: Vec<u8>| {
            let mut shelf = blobs.shelf();
            shelf.stored.clear();
            shelf.stored.insert("meta".into(), meta);
            shelf.stored.insert("dat2/00000".into(), vec![7; 600]);
            shelf.stored.insert("dat2/00009".into(), vec![7; 600]);
            shelf.stored.insert("idx3".into(), vec![1; 12]);
            shelf.stored.insert("idx999".into(), vec![1; 12]);
            shelf.stored.insert("unrelated".into(), vec![1; 12]);
        };
        store(meta(STORAGE_FORMAT, MIRROR_CHUNK, 900));
        let restored = reopen(&blobs).expect("restorable");
        let (data, indexes) = restored.memory_bytes().unwrap();
        // A short piece leaves zeros; a piece past the end is ignored.
        assert_eq!(data.len(), 900);
        assert!(data[..600].iter().all(|&b| b == 7) && data[600..].iter().all(|&b| b == 0));
        assert_eq!(indexes.keys().copied().collect::<Vec<_>>(), [3]);
        store(meta(STORAGE_FORMAT + 1, MIRROR_CHUNK, 900));
        assert!(reopen(&blobs).is_none());
        store(meta(STORAGE_FORMAT, MIRROR_CHUNK / 2, 900));
        assert!(reopen(&blobs).is_none());
        store(meta(STORAGE_FORMAT, MIRROR_CHUNK, DATA_MAX_LENGTH - 1000));
        assert!(reopen(&blobs).is_none());
        store(b"not json".to_vec());
        assert!(reopen(&blobs).is_none());
        blobs.shelf().unavailable = true;
        assert_eq!(blobs.poll(), Stored::Unavailable);
    }
}
