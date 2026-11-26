//! What the client does with the update connection: `rt4.Js5MasterIndex`,
//! `rt4.Js5CachedResourceProvider`, `rt4.Js5CacheQueue` and the readiness half
//! of `rt4.Js5`.
//!
//! The rules, in the order the original applies them:
//! * The master index (archive 255, group 255) is taken from the server as
//!   sent. Nothing checks it; it supplies each archive index's CRC and version.
//! * An archive's index is read from the local store first and kept if its CRC
//!   and its version both equal the master's. Otherwise it is requested
//!   (urgent) and accepted on CRC alone, then written to the store.
//! * A stored group is trusted when the CRC of everything but its last two
//!   bytes equals the index's CRC for the group and those two bytes equal the
//!   low 16 bits of the index's version. Anything else marks the group for
//!   download.
//! * A downloaded group is accepted when the CRC of what the server sent
//!   equals the index's. The version is then written into the two spare bytes
//!   and the group stored. A mismatch drops the connection, picks a new XOR
//!   key and counts an error; four in a row stop the client.
use crate::js5_client::{Js5NetQueue, NetRequest};
use rs09_cache::{Limits, disk::Cache, index::Index};
use rs09_protocol::checksum::crc32;
use std::{
    collections::{BTreeMap, VecDeque},
    io,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

/// `Js5CachedResourceProvider.loop` stops queueing disk reads at this many.
pub const CACHE_QUEUE_LIMIT: usize = 250;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Work {
    /// Read on the caller's thread; never queued.
    Immediate,
    Write,
    Read,
}
struct CacheInner {
    cache: Cache,
    group: u32,
    work: Work,
    urgent: bool,
    incomplete: AtomicBool,
    data: Mutex<Option<Arc<Vec<u8>>>>,
}
/// `rt4.Js5CacheRequest`.
#[derive(Clone)]
pub struct CacheRequest(Arc<CacheInner>);
impl CacheRequest {
    fn new(
        cache: &Cache,
        group: u32,
        work: Work,
        urgent: bool,
        data: Option<Arc<Vec<u8>>>,
    ) -> Self {
        Self(Arc::new(CacheInner {
            cache: cache.clone(),
            group,
            work,
            urgent,
            incomplete: AtomicBool::new(work != Work::Immediate),
            data: Mutex::new(data),
        }))
    }
    pub fn urgent(&self) -> bool {
        self.0.urgent
    }
    pub fn incomplete(&self) -> bool {
        self.0.incomplete.load(Ordering::Acquire)
    }
    /// `None` while incomplete (the Java throws) and when the store had no
    /// readable group.
    pub fn data(&self) -> Option<Arc<Vec<u8>>> {
        if self.incomplete() {
            return None;
        }
        self.0.data.lock().expect("cache request").clone()
    }
    fn execute(&self) {
        match self.0.work {
            Work::Write => {
                let data = self.0.data.lock().expect("cache request").clone();
                if let Some(data) = data {
                    self.0.cache.write(self.0.group, &data);
                }
            }
            Work::Read => {
                let data = self.0.cache.read(self.0.group).map(Arc::new);
                *self.0.data.lock().expect("cache request") = data;
            }
            Work::Immediate => {}
        }
        self.0.incomplete.store(false, Ordering::Release);
    }
}

#[derive(Default)]
struct QueueState {
    list: VecDeque<CacheRequest>,
    stop: bool,
    busy: bool,
}
#[derive(Default)]
struct Shared {
    state: Mutex<QueueState>,
    wake: Condvar,
    idle: Condvar,
}
/// `rt4.Js5CacheQueue`: store reads and writes in request order, off the
/// client's thread.
pub struct CacheQueue {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}
impl CacheQueue {
    /// The original arrangement: one worker thread.
    pub fn threaded() -> io::Result<Self> {
        let shared = Arc::new(Shared::default());
        let worker = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("js5-cache".into())
            .spawn(move || {
                loop {
                    let request = {
                        let mut state = worker.state.lock().expect("cache queue");
                        loop {
                            if state.stop {
                                return;
                            }
                            if let Some(request) = state.list.pop_front() {
                                state.busy = true;
                                break request;
                            }
                            state = worker.wake.wait(state).expect("cache queue");
                        }
                    };
                    request.execute();
                    worker.state.lock().expect("cache queue").busy = false;
                    worker.idle.notify_all();
                }
            })?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }
    /// No worker: requests wait until [`CacheQueue::run_one`] or
    /// [`CacheQueue::flush`]. For tests and single-threaded callers.
    pub fn manual() -> Self {
        Self {
            shared: Arc::new(Shared::default()),
            thread: None,
        }
    }
    /// Requests queued and not yet started.
    pub fn size(&self) -> usize {
        self.shared.state.lock().expect("cache queue").list.len()
    }
    fn enqueue(&self, request: CacheRequest) -> CacheRequest {
        self.shared
            .state
            .lock()
            .expect("cache queue")
            .list
            .push_back(request.clone());
        self.shared.wake.notify_all();
        request
    }
    pub fn write(&self, cache: &Cache, data: Arc<Vec<u8>>, group: u32) -> CacheRequest {
        self.enqueue(CacheRequest::new(
            cache,
            group,
            Work::Write,
            false,
            Some(data),
        ))
    }
    pub fn read(&self, group: u32, cache: &Cache) -> CacheRequest {
        self.enqueue(CacheRequest::new(cache, group, Work::Read, false, None))
    }
    /// A write still waiting in the queue answers first; otherwise the store
    /// is read on the calling thread.
    pub fn read_synchronous(&self, cache: &Cache, group: u32) -> CacheRequest {
        {
            let state = self.shared.state.lock().expect("cache queue");
            for queued in &state.list {
                if queued.0.group == group
                    && queued.0.work == Work::Write
                    && queued.0.cache.same(cache)
                {
                    let data = queued.0.data.lock().expect("cache request").clone();
                    return CacheRequest::new(cache, group, Work::Immediate, false, data);
                }
            }
        }
        let data = cache.read(group).map(Arc::new);
        CacheRequest::new(cache, group, Work::Immediate, true, data)
    }
    /// Runs the oldest queued request on this thread. Only for
    /// [`CacheQueue::manual`] queues; `false` when nothing was waiting.
    pub fn run_one(&self) -> bool {
        let request = self
            .shared
            .state
            .lock()
            .expect("cache queue")
            .list
            .pop_front();
        request.is_some_and(|request| {
            request.execute();
            true
        })
    }
    /// Returns once everything queued so far has been carried out. The
    /// original has no such call: it abandons queued writes when it quits.
    pub fn flush(&self) {
        if self.thread.is_none() {
            while self.run_one() {}
            return;
        }
        let mut state = self.shared.state.lock().expect("cache queue");
        while !state.list.is_empty() || state.busy {
            state = self.shared.idle.wait(state).expect("cache queue");
        }
    }
    /// `Js5CacheQueue.quit`: stops the worker after its current request.
    pub fn quit(&mut self) {
        self.shared.state.lock().expect("cache queue").stop = true;
        self.shared.wake.notify_all();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
impl Drop for CacheQueue {
    fn drop(&mut self) {
        self.quit();
    }
}

/// `rt4.Js5Index` as the provider and `Js5` read it: dense per-group arrays.
pub struct IndexTable {
    pub index: Index,
    /// Files per group; zero where the index has no such group.
    pub group_sizes: Vec<u32>,
    /// One past the highest file ID per group; zero where there is no group.
    pub group_capacities: Vec<u32>,
    pub group_checksums: Vec<u32>,
    pub group_versions: Vec<i32>,
}
impl IndexTable {
    fn decode(data: &[u8], expected_checksum: u32, limits: Limits) -> Option<Self> {
        let index = Index::decode(data, Some(expected_checksum), limits).ok()?;
        let capacity = index.capacity as usize;
        let mut table = Self {
            group_sizes: vec![0; capacity],
            group_capacities: vec![0; capacity],
            group_checksums: vec![0; capacity],
            group_versions: vec![0; capacity],
            index,
        };
        for group in &table.index.groups {
            let id = group.id as usize;
            table.group_sizes[id] = group.files.len() as u32;
            table.group_capacities[id] = group.capacity;
            table.group_checksums[id] = group.checksum;
            table.group_versions[id] = group.version;
        }
        Some(table)
    }
    /// `Js5Index.capacity`: one past the highest group ID.
    pub fn capacity(&self) -> usize {
        self.group_sizes.len()
    }
    /// `Js5Index.size`: how many groups the index lists.
    pub fn size(&self) -> usize {
        self.index.groups.len()
    }
}

#[derive(Clone)]
enum AnyRequest {
    Net(NetRequest),
    Cache(CacheRequest),
}
impl AnyRequest {
    fn urgent(&self) -> bool {
        match self {
            Self::Net(request) => request.urgent(),
            Self::Cache(request) => request.urgent(),
        }
    }
    fn incomplete(&self) -> bool {
        match self {
            Self::Net(request) => request.incomplete(),
            Self::Cache(request) => request.incomplete(),
        }
    }
    fn data(&self) -> Option<Arc<Vec<u8>>> {
        match self {
            Self::Net(request) => request.data(),
            Self::Cache(request) => request.data(),
        }
    }
    fn percentage_complete(&self) -> i32 {
        match self {
            Self::Net(request) => request.percentage_complete(),
            Self::Cache(request) => {
                if request.incomplete() {
                    0
                } else {
                    100
                }
            }
        }
    }
}
struct Tracked {
    request: AnyRequest,
    orphan: bool,
}

/// A `RuntimeException` the original lets escape into its main loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderError {
    /// A background mode was asked of a provider that has no local store, or
    /// a download was asked for a group not yet found wanting.
    Inconsistent,
    /// A finished background request sat unclaimed through two sweeps.
    OrphanedPrefetch,
}
impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Inconsistent => "update provider state is inconsistent",
            Self::OrphanedPrefetch => "a finished background request was never collected",
        })
    }
}
impl std::error::Error for ProviderError {}

/// `rt4.Js5CachedResourceProvider`: one archive's index and groups, from the
/// local store where it can be trusted and from the server otherwise.
pub struct ResourceProvider {
    archive: u8,
    cache: Option<Cache>,
    master_cache: Option<Cache>,
    expected_checksum: u32,
    expected_version: i32,
    discard_orphans: bool,
    limits: Limits,
    index: Option<Arc<IndexTable>>,
    index_request: Option<AnyRequest>,
    /// 0 not looked at, 1 verified in the store, -1 must be downloaded.
    group_status: Vec<i8>,
    verified_groups: usize,
    requests: BTreeMap<u32, Tracked>,
    prefetch_queue: Vec<u32>,
    group_queue: Option<VecDeque<u32>>,
    verify_all: bool,
    prefetch_all: bool,
    group: usize,
    orphan_check_time: u64,
}
impl ResourceProvider {
    /// `cache` holds the archive's groups, `master_cache` the indexes
    /// (archive 255). Either may be absent, as when the original cannot open
    /// its cache files; nothing is verified or stored then.
    pub fn new(
        archive: u8,
        cache: Option<Cache>,
        master_cache: Option<Cache>,
        cache_queue: &CacheQueue,
        expected_checksum: u32,
        expected_version: i32,
        discard_orphans: bool,
    ) -> Self {
        let index_request = master_cache.as_ref().map(|master| {
            AnyRequest::Cache(cache_queue.read_synchronous(master, u32::from(archive)))
        });
        Self {
            archive,
            verify_all: cache.is_some(),
            group_queue: cache.is_some().then(VecDeque::new),
            cache,
            master_cache,
            expected_checksum,
            expected_version,
            discard_orphans,
            limits: Limits::default(),
            index: None,
            index_request,
            group_status: Vec::new(),
            verified_groups: 0,
            requests: BTreeMap::new(),
            prefetch_queue: Vec::new(),
            prefetch_all: false,
            group: 0,
            orphan_check_time: 0,
        }
    }
    pub fn archive(&self) -> u8 {
        self.archive
    }
    pub fn expected_checksum(&self) -> u32 {
        self.expected_checksum
    }
    pub fn expected_version(&self) -> i32 {
        self.expected_version
    }
    /// The index, if it has been obtained; does not ask for it.
    pub fn index(&self) -> Option<&Arc<IndexTable>> {
        self.index.as_ref()
    }
    /// 1 verified in the store, -1 to be downloaded, 0 not examined yet.
    pub fn group_status(&self, group: u32) -> i8 {
        self.group_status.get(group as usize).copied().unwrap_or(0)
    }
    /// `true` while the start-up verification or the background download of
    /// this archive is still running.
    pub fn busy(&self) -> bool {
        self.group_queue.is_some()
    }
    pub fn verifying(&self) -> bool {
        self.group_queue.is_some() && self.verify_all
    }
    pub fn verified_groups(&self) -> usize {
        self.verified_groups
    }
    /// As in the original: while verifying, the ID of the oldest group still
    /// waiting on the disk queue; afterwards the number of groups.
    pub fn total_verified_groups(&self) -> usize {
        match &self.index {
            None => 0,
            Some(_) if self.verify_all => self
                .group_queue
                .as_ref()
                .and_then(|queue| queue.front())
                .map_or(0, |&group| group as usize),
            Some(index) => index.size(),
        }
    }
    pub fn index_size(&self) -> usize {
        self.index.as_ref().map_or(0, |index| index.size())
    }

    fn request_index(&self, net: &mut Js5NetQueue) -> Option<AnyRequest> {
        if net.is_urgent_request_queue_full() {
            return None;
        }
        net.read(255, 0, u16::from(self.archive), true)
            .ok()
            .map(AnyRequest::Net)
    }
    /// `fetchIndex`: `None` until the index is in hand.
    pub fn fetch_index(
        &mut self,
        net: &mut Js5NetQueue,
        cache_queue: &CacheQueue,
    ) -> Option<Arc<IndexTable>> {
        if let Some(index) = &self.index {
            return Some(Arc::clone(index));
        }
        if self.index_request.is_none() {
            self.index_request = Some(self.request_index(net)?);
        }
        let request = self.index_request.clone()?;
        if request.incomplete() {
            return None;
        }
        let data = request.data();
        let decoded = data
            .as_deref()
            .and_then(|data| IndexTable::decode(data, self.expected_checksum, self.limits));
        let index = match &request {
            // From the store: the version has to be the master's as well.
            AnyRequest::Cache(_) => {
                decoded.filter(|index| index.index.version == self.expected_version)
            }
            AnyRequest::Net(_) => {
                if decoded.is_none() {
                    net.rekey();
                }
                decoded
            }
        };
        let Some(index) = index else {
            self.index_request = self.request_index(net);
            return None;
        };
        if let (AnyRequest::Net(_), Some(master), Some(data)) = (&request, &self.master_cache, data)
        {
            cache_queue.write(master, data, u32::from(self.archive));
        }
        if self.cache.is_some() {
            self.group_status = vec![0; index.capacity()];
            self.verified_groups = 0;
        }
        self.index_request = None;
        let index = Arc::new(index);
        self.index = Some(Arc::clone(&index));
        Some(index)
    }
    /// 100 with the index in hand, otherwise how much of it has arrived.
    pub fn index_percentage_complete(
        &mut self,
        net: &mut Js5NetQueue,
        cache_queue: &CacheQueue,
    ) -> i32 {
        if self.fetch_index(net, cache_queue).is_some() {
            return 100;
        }
        self.index_request
            .as_ref()
            .map_or(0, AnyRequest::percentage_complete)
    }

    /// `prefetchAll`: after verification, download every group the store
    /// lacks, in the background (prefetch priority).
    pub fn prefetch_all(&mut self) {
        if self.cache.is_some() {
            self.prefetch_all = true;
            if self.group_queue.is_none() {
                self.group_queue = Some(VecDeque::new());
            }
        }
    }
    /// `prefetchGroup`: ask for one group in the background. As in the
    /// original this is only acted on while the archive is still verifying
    /// or downloading.
    pub fn prefetch_group(&mut self, group: u32) {
        if self.cache.is_some() && !self.prefetch_queue.contains(&group) {
            self.prefetch_queue.push(group);
        }
    }
    /// `getPercentageComplete(group)`: progress of the request for a group.
    pub fn percentage_complete(&self, group: u32) -> i32 {
        self.requests
            .get(&group)
            .map_or(0, |tracked| tracked.request.percentage_complete())
    }

    /// `processPrefetchQueue`.
    pub fn process_prefetch_queue(
        &mut self,
        net: &mut Js5NetQueue,
        cache_queue: &CacheQueue,
    ) -> Result<(), ProviderError> {
        if self.group_queue.is_none() {
            return Ok(());
        }
        let Some(index) = self.fetch_index(net, cache_queue) else {
            return Ok(());
        };
        let queued = std::mem::take(&mut self.prefetch_queue);
        let mut kept = Vec::with_capacity(queued.len());
        let mut result = Ok(());
        for group in queued {
            if result.is_err() {
                kept.push(group);
                continue;
            }
            let slot = group as usize;
            if slot >= index.capacity() || index.group_sizes[slot] == 0 {
                continue;
            }
            if self.group_status[slot] == 0 {
                result = self.fetch_group_inner(1, group, net, cache_queue).map(drop);
            }
            if result.is_ok() && self.group_status[slot] == -1 {
                result = self.fetch_group_inner(2, group, net, cache_queue).map(drop);
            }
            if self.group_status[slot] != 1 {
                kept.push(group);
            }
        }
        self.prefetch_queue = kept;
        result
    }

    /// `loop`: one cycle of verification or background download, then the
    /// once-a-second sweep for finished requests nobody collected.
    pub fn tick(
        &mut self,
        net: &mut Js5NetQueue,
        cache_queue: &CacheQueue,
        now_ms: u64,
    ) -> Result<(), ProviderError> {
        if self.group_queue.is_some() {
            let Some(index) = self.fetch_index(net, cache_queue) else {
                return Ok(());
            };
            if self.verify_all || self.prefetch_all {
                let verifying = self.verify_all;
                let mode = if verifying { 1 } else { 2 };
                // Verifying waits on status 0; downloading on anything but 1.
                let unsettled = |status: i8| if verifying { status == 0 } else { status != 1 };
                let mut complete = true;
                let queued = self.group_queue.take().unwrap_or_default();
                let mut kept = VecDeque::with_capacity(queued.len());
                let mut failure = None;
                for group in queued {
                    let slot = group as usize;
                    if failure.is_none() && unsettled(self.group_status[slot]) {
                        failure = self.fetch_group_inner(mode, group, net, cache_queue).err();
                    }
                    if unsettled(self.group_status[slot]) {
                        complete = false;
                        kept.push_back(group);
                    }
                }
                while failure.is_none() && index.group_sizes.len() > self.group {
                    let slot = self.group;
                    if index.group_sizes[slot] == 0 {
                        self.group += 1;
                        continue;
                    }
                    let full = if verifying {
                        cache_queue.size() >= CACHE_QUEUE_LIMIT
                    } else {
                        net.is_prefetch_request_queue_full()
                    };
                    if full {
                        complete = false;
                        break;
                    }
                    if unsettled(self.group_status[slot]) {
                        failure = self
                            .fetch_group_inner(mode, slot as u32, net, cache_queue)
                            .err();
                    }
                    if unsettled(self.group_status[slot]) {
                        complete = false;
                        kept.push_back(slot as u32);
                    }
                    self.group += 1;
                }
                self.group_queue = Some(kept);
                if let Some(error) = failure {
                    return Err(error);
                }
                if complete {
                    if verifying {
                        self.verify_all = false;
                    } else {
                        self.prefetch_all = false;
                    }
                    self.group = 0;
                }
            } else {
                self.group_queue = None;
            }
        }
        if !self.discard_orphans || self.orphan_check_time > now_ms {
            return Ok(());
        }
        let mut dropped = Vec::new();
        for (&group, tracked) in &mut self.requests {
            if tracked.request.incomplete() {
                continue;
            }
            if !tracked.orphan {
                tracked.orphan = true;
            } else if tracked.request.urgent() {
                dropped.push(group);
            } else {
                return Err(ProviderError::OrphanedPrefetch);
            }
        }
        for group in dropped {
            self.requests.remove(&group);
        }
        self.orphan_check_time = now_ms + 1000;
        Ok(())
    }

    /// `fetchGroup`: the stored form of a group (container plus version
    /// trailer) once it is verified, asking the store or, urgently, the
    /// server as needed. `None` until then.
    pub fn fetch_group(
        &mut self,
        group: u32,
        net: &mut Js5NetQueue,
        cache_queue: &CacheQueue,
    ) -> Result<Option<Arc<Vec<u8>>>, ProviderError> {
        let known = self
            .index
            .as_ref()
            .is_some_and(|index| (group as usize) < index.capacity());
        if !known {
            return Ok(None);
        }
        let Some(request) = self.fetch_group_inner(0, group, net, cache_queue)? else {
            return Ok(None);
        };
        self.requests.remove(&group);
        Ok(request.data())
    }

    fn request_group(&self, group: u32, urgent: bool, net: &mut Js5NetQueue) -> Option<AnyRequest> {
        // The request carries the group in 16 bits; the Java would wrap a
        // larger ID into the archive byte and ask for something else.
        let group = u16::try_from(group).ok()?;
        net.read(self.archive, 2, group, urgent)
            .ok()
            .map(AnyRequest::Net)
    }
    fn track(&mut self, group: u32, request: AnyRequest) {
        self.requests.insert(
            group,
            Tracked {
                request,
                orphan: false,
            },
        );
    }

    /// `fetchGroupInner`. Mode 0: wanted now. Mode 1: verify the stored copy
    /// in the background. Mode 2: download in the background.
    fn fetch_group_inner(
        &mut self,
        mode: u8,
        group: u32,
        net: &mut Js5NetQueue,
        cache_queue: &CacheQueue,
    ) -> Result<Option<AnyRequest>, ProviderError> {
        let slot = group as usize;
        let Some(index) = self.index.clone() else {
            return Err(ProviderError::Inconsistent);
        };
        let mut request = self
            .requests
            .get(&group)
            .map(|tracked| tracked.request.clone());
        if mode == 0
            && request
                .as_ref()
                .is_some_and(|r| !r.urgent() && r.incomplete())
        {
            // A background request is abandoned for an urgent one.
            self.requests.remove(&group);
            request = None;
        }
        let request = match request {
            Some(request) => request,
            None => {
                let created = match (mode, &self.cache) {
                    (0, Some(cache)) if self.group_status[slot] != -1 => {
                        AnyRequest::Cache(cache_queue.read_synchronous(cache, group))
                    }
                    (0, _) => {
                        if net.is_urgent_request_queue_full() {
                            return Ok(None);
                        }
                        match self.request_group(group, true, net) {
                            Some(request) => request,
                            None => return Ok(None),
                        }
                    }
                    (1, Some(cache)) => AnyRequest::Cache(cache_queue.read(group, cache)),
                    (2, Some(_)) => {
                        if self.group_status[slot] != -1 {
                            return Err(ProviderError::Inconsistent);
                        }
                        if net.is_prefetch_request_queue_full() {
                            return Ok(None);
                        }
                        match self.request_group(group, false, net) {
                            Some(request) => request,
                            None => return Ok(None),
                        }
                    }
                    _ => return Err(ProviderError::Inconsistent),
                };
                self.track(group, created.clone());
                created
            }
        };
        if request.incomplete() {
            return Ok(None);
        }
        let version = index.group_versions[slot];
        // Scoped so that stamping the trailer below edits the bytes in place.
        let (crc_matches, trailer_matches) = {
            let data = request.data();
            let data = data.as_deref().filter(|data| data.len() > 2);
            (
                data.is_some_and(|data| {
                    crc32(&data[..data.len() - 2]) == index.group_checksums[slot]
                }),
                data.is_some_and(|data| {
                    let stored = [data[data.len() - 2], data[data.len() - 1]];
                    i32::from(u16::from_be_bytes(stored)) == version & 0xffff
                }),
            )
        };
        match &request {
            AnyRequest::Net(net_request) => {
                if !crc_matches {
                    net.rekey();
                    self.requests.remove(&group);
                    if request.urgent()
                        && !net.is_urgent_request_queue_full()
                        && let Some(retry) = self.request_group(group, true, net)
                    {
                        self.track(group, retry);
                    }
                    return Ok(None);
                }
                net.errors = 0;
                net.response = 0;
                let stamped = net_request.stamp_trailer(version);
                if let (Some(cache), Some(data)) = (&self.cache, stamped) {
                    cache_queue.write(cache, data, group);
                    if self.group_status[slot] != 1 {
                        self.verified_groups += 1;
                        self.group_status[slot] = 1;
                    }
                }
            }
            AnyRequest::Cache(_) => {
                if !(crc_matches && trailer_matches) {
                    self.group_status[slot] = -1;
                    self.requests.remove(&group);
                    if request.urgent()
                        && !net.is_urgent_request_queue_full()
                        && let Some(retry) = self.request_group(group, true, net)
                    {
                        self.track(group, retry);
                    }
                    return Ok(None);
                }
                if self.group_status[slot] != 1 {
                    self.verified_groups += 1;
                    self.group_status[slot] = 1;
                }
            }
        }
        if !request.urgent() {
            self.requests.remove(&group);
        }
        Ok(Some(request))
    }
}

/// `rt4.Js5MasterIndex`.
pub struct MasterIndex {
    request: Option<NetRequest>,
    buffer: Option<Arc<Vec<u8>>>,
    providers: Vec<Option<ResourceProvider>>,
}
impl MasterIndex {
    /// Asks for archive 255, group 255 at urgent priority.
    pub fn new(net: &mut Js5NetQueue) -> Self {
        Self {
            request: net.read(255, 0, 255, true).ok(),
            buffer: None,
            providers: Vec::new(),
        }
    }
    /// `isReady`. The reply is used as received: an uncompressed container
    /// whose payload is eight bytes (CRC, version) per archive.
    pub fn is_ready(&mut self, net: &mut Js5NetQueue) -> bool {
        if self.buffer.is_some() {
            return true;
        }
        if self.request.is_none() {
            self.request = net.read(255, 0, 255, true).ok();
        }
        let Some(data) = self.request.as_ref().and_then(NetRequest::data) else {
            return false;
        };
        let count = data.len().saturating_sub(5) / 8;
        self.providers = (0..count).map(|_| None).collect();
        self.buffer = Some(data);
        true
    }
    /// How many archives the server's master index describes.
    pub fn archive_count(&self) -> usize {
        self.providers.len()
    }
    /// The CRC and version the master index gives for an archive's index.
    pub fn entry(&self, archive: u8) -> Option<(u32, i32)> {
        let buffer = self.buffer.as_ref()?;
        let offset = usize::from(archive) * 8 + 5;
        let bytes = buffer.get(offset..offset + 8)?;
        Some((
            u32::from_be_bytes(bytes[..4].try_into().ok()?),
            i32::from_be_bytes(bytes[4..].try_into().ok()?),
        ))
    }
    /// `getResourceProvider`: creates the archive's provider on first use.
    /// `None` before the master index is ready or for an archive it does not
    /// list (the Java throws in both cases).
    pub fn resource_provider(
        &mut self,
        archive: u8,
        cache: Option<Cache>,
        master_cache: Option<Cache>,
        cache_queue: &CacheQueue,
    ) -> Option<&mut ResourceProvider> {
        let (checksum, version) = self.entry(archive)?;
        let slot = self.providers.get_mut(usize::from(archive))?;
        Some(slot.get_or_insert_with(|| {
            ResourceProvider::new(
                archive,
                cache,
                master_cache,
                cache_queue,
                checksum,
                version,
                true,
            )
        }))
    }
    pub fn provider(&self, archive: u8) -> Option<&ResourceProvider> {
        self.providers.get(usize::from(archive))?.as_ref()
    }
    pub fn provider_mut(&mut self, archive: u8) -> Option<&mut ResourceProvider> {
        self.providers.get_mut(usize::from(archive))?.as_mut()
    }
    pub fn providers(&self) -> impl Iterator<Item = &ResourceProvider> {
        self.providers.iter().flatten()
    }
    /// `processResourceProviders`: every provider's prefetch queue, then
    /// every provider's loop, in archive order.
    pub fn process_resource_providers(
        &mut self,
        net: &mut Js5NetQueue,
        cache_queue: &CacheQueue,
        now_ms: u64,
    ) -> Result<(), ProviderError> {
        for provider in self.providers.iter_mut().flatten() {
            provider.process_prefetch_queue(net, cache_queue)?;
        }
        for provider in self.providers.iter_mut().flatten() {
            provider.tick(net, cache_queue, now_ms)?;
        }
        Ok(())
    }
}

/// The part of `rt4.Js5` that decides whether data is ready: which groups of
/// an archive have been fetched. The packed bytes themselves are not kept;
/// the Rust client reads them from the store afterwards.
#[derive(Default)]
pub struct Js5Archive {
    index: Option<Arc<IndexTable>>,
    packed: Vec<bool>,
}
impl Js5Archive {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn is_index_ready(
        &mut self,
        provider: &mut ResourceProvider,
        net: &mut Js5NetQueue,
        cache_queue: &CacheQueue,
    ) -> bool {
        if self.index.is_none() {
            let Some(index) = provider.fetch_index(net, cache_queue) else {
                return false;
            };
            self.packed = vec![false; index.capacity()];
            self.index = Some(index);
        }
        true
    }
    pub fn index(&self) -> Option<&Arc<IndexTable>> {
        self.index.as_ref()
    }
    /// `getChecksum`: the CRC of the archive's index, sent at login.
    pub fn checksum(&self) -> Option<u32> {
        self.index.as_ref().map(|index| index.index.checksum)
    }
    fn valid(&self, group: i32) -> Option<usize> {
        let index = self.index.as_ref()?;
        let slot = usize::try_from(group).ok()?;
        (index.group_capacities.get(slot).copied()? != 0).then_some(slot)
    }
    fn fetch(
        &mut self,
        slot: usize,
        provider: &mut ResourceProvider,
        net: &mut Js5NetQueue,
        cache_queue: &CacheQueue,
    ) -> Result<bool, ProviderError> {
        if !self.packed[slot] {
            self.packed[slot] = provider
                .fetch_group(slot as u32, net, cache_queue)?
                .is_some();
        }
        Ok(self.packed[slot])
    }
    /// `getGroupId`: -1 without the index, without names or without a group
    /// of that name. Names are lower-cased ASCII, as the original's are.
    pub fn group_id(&self, name: &str) -> i32 {
        let Some(index) = &self.index else {
            return -1;
        };
        index
            .index
            .group_by_ascii_name(name)
            .map_or(-1, |group| group as i32)
    }
    /// `fetchAll`: asks for every group not yet held; `true` once all are.
    pub fn fetch_all(
        &mut self,
        provider: &mut ResourceProvider,
        net: &mut Js5NetQueue,
        cache_queue: &CacheQueue,
    ) -> Result<bool, ProviderError> {
        if !self.is_index_ready(provider, net, cache_queue) {
            return Ok(false);
        }
        let index = Arc::clone(self.index.as_ref().expect("index ready"));
        let mut success = true;
        for group in &index.index.groups {
            let slot = group.id as usize;
            if !self.packed[slot] && !self.fetch(slot, provider, net, cache_queue)? {
                success = false;
            }
        }
        Ok(success)
    }
    /// `getPercentageComplete(int)`.
    pub fn group_percentage_complete(&self, group: i32, provider: &ResourceProvider) -> i32 {
        match self.valid(group) {
            Some(slot) if self.packed[slot] => 100,
            Some(slot) => provider.percentage_complete(slot as u32),
            None => 0,
        }
    }
    /// `getPercentageComplete()`: the mean over the archive's groups.
    pub fn percentage_complete(
        &mut self,
        provider: &mut ResourceProvider,
        net: &mut Js5NetQueue,
        cache_queue: &CacheQueue,
    ) -> i32 {
        if !self.is_index_ready(provider, net, cache_queue) {
            return 0;
        }
        let index = Arc::clone(self.index.as_ref().expect("index ready"));
        let (mut total, mut complete) = (0i32, 0i32);
        for (slot, &size) in index.group_sizes.iter().enumerate() {
            if size > 0 {
                total = total.wrapping_add(100);
                complete =
                    complete.wrapping_add(self.group_percentage_complete(slot as i32, provider));
            }
        }
        if total == 0 {
            100
        } else {
            complete.wrapping_mul(100) / total
        }
    }
    /// `isGroupReady(int)`.
    pub fn is_group_ready(
        &mut self,
        group: i32,
        provider: &mut ResourceProvider,
        net: &mut Js5NetQueue,
        cache_queue: &CacheQueue,
    ) -> Result<bool, ProviderError> {
        if !self.is_index_ready(provider, net, cache_queue) {
            return Ok(false);
        }
        match self.valid(group) {
            Some(slot) => self.fetch(slot, provider, net, cache_queue),
            None => Ok(false),
        }
    }
    /// `isFileReady(int)`: the file of a single-group archive, or the single
    /// file of a group. The Java throws when the ID names a group with
    /// several files in an archive with several groups; that is `false` here.
    pub fn is_file_ready(
        &mut self,
        id: i32,
        provider: &mut ResourceProvider,
        net: &mut Js5NetQueue,
        cache_queue: &CacheQueue,
    ) -> Result<bool, ProviderError> {
        if !self.is_index_ready(provider, net, cache_queue) {
            return Ok(false);
        }
        let index = Arc::clone(self.index.as_ref().expect("index ready"));
        if index.capacity() == 1 {
            return self.is_file_ready_in(0, id, provider, net, cache_queue);
        }
        match self.valid(id) {
            Some(slot) if index.group_capacities[slot] == 1 => {
                self.is_file_ready_in(id, 0, provider, net, cache_queue)
            }
            _ => Ok(false),
        }
    }
    /// `isFileReady(file, group)`: the group is fetched whole.
    pub fn is_file_ready_in(
        &mut self,
        group: i32,
        file: i32,
        provider: &mut ResourceProvider,
        net: &mut Js5NetQueue,
        cache_queue: &CacheQueue,
    ) -> Result<bool, ProviderError> {
        if !self.is_index_ready(provider, net, cache_queue) {
            return Ok(false);
        }
        let in_range = self.index.as_ref().is_some_and(|index| {
            usize::try_from(group)
                .ok()
                .and_then(|slot| index.group_capacities.get(slot))
                .is_some_and(|&capacity| file >= 0 && (file as u32) < capacity)
        });
        if !in_range {
            return Ok(false);
        }
        self.fetch(group as usize, provider, net, cache_queue)
    }
    /// `isFileReady(JagString file, JagString group)`.
    pub fn is_named_file_ready(
        &mut self,
        file: &str,
        group: &str,
        provider: &mut ResourceProvider,
        net: &mut Js5NetQueue,
        cache_queue: &CacheQueue,
    ) -> Result<bool, ProviderError> {
        if !self.is_index_ready(provider, net, cache_queue) {
            return Ok(false);
        }
        let index = Arc::clone(self.index.as_ref().expect("index ready"));
        let group = self.group_id(group);
        let Some(slot) = self.valid(group) else {
            return Ok(false);
        };
        let hash = rs09_cache::index::name_hash(file.to_ascii_lowercase().as_bytes());
        let file = index
            .index
            .group(slot as u32)
            .and_then(|info| info.file_by_hash(hash))
            .map_or(-1, |file| file as i32);
        self.is_file_ready_in(group, file, provider, net, cache_queue)
    }
}

#[cfg(test)]
mod tests {
    //! Transcription tests of the provider rules against a scripted server
    //! that answers with the Rust server's (JVM-checked) reply formatter.
    use super::*;
    use crate::js5_client::scripted::{FakeServer, ScriptedSocket};
    use rs09_cache::disk::{ARCHIVE_MAX_GROUP, DiskStore, MASTER_MAX_GROUP};

    struct Rig {
        net: Js5NetQueue,
        socket: ScriptedSocket,
        server: FakeServer,
        queue: CacheQueue,
        store: Arc<Mutex<DiskStore>>,
        now: u64,
    }
    impl Rig {
        fn new(server: FakeServer) -> Self {
            let store = DiskStore::in_memory(
                Vec::new(),
                [3u8, 255].into_iter().map(|a| (a, Vec::new())).collect(),
            );
            let mut rig = Self {
                net: Js5NetQueue::with_random(Box::new(|| 0.25)),
                socket: ScriptedSocket::new(),
                server,
                queue: CacheQueue::manual(),
                store: Arc::new(Mutex::new(store)),
                now: 0,
            };
            rig.net.start(true, Box::new(rig.socket.clone()), 0);
            rig
        }
        fn cache(&self, archive: u8) -> Cache {
            let limit = if archive == 255 {
                MASTER_MAX_GROUP
            } else {
                ARCHIVE_MAX_GROUP
            };
            Cache::new(Arc::clone(&self.store), archive, limit)
        }
        fn provider(&self, archive: u8) -> ResourceProvider {
            let (checksum, version) = self.server.master_entry(archive);
            ResourceProvider::new(
                archive,
                Some(self.cache(archive)),
                Some(self.cache(255)),
                &self.queue,
                checksum,
                version,
                true,
            )
        }
        /// One client cycle: network, providers, then all queued disk work.
        fn cycle(&mut self, provider: &mut ResourceProvider) {
            self.now += 20;
            if !self.net.tick(self.now) {
                // What js5Connect does once its delay has passed.
                self.socket = ScriptedSocket::new();
                self.server.reconnected();
                self.net
                    .start(true, Box::new(self.socket.clone()), self.now);
                self.net.tick(self.now);
            }
            self.server.serve(&self.socket);
            provider
                .process_prefetch_queue(&mut self.net, &self.queue)
                .unwrap();
            provider.tick(&mut self.net, &self.queue, self.now).unwrap();
            self.queue.flush();
        }
        fn run(&mut self, provider: &mut ResourceProvider) -> usize {
            let mut cycles = 0;
            while provider.busy() {
                self.cycle(provider);
                cycles += 1;
                assert!(cycles < 10_000, "provider never settled");
            }
            cycles
        }
        fn stored(&self, archive: u8, group: u32) -> Option<Vec<u8>> {
            self.cache(archive).read(group)
        }
    }
    fn groups() -> Vec<(u32, Vec<u8>, i32)> {
        // (id, payload, version); IDs are sparse on purpose.
        (0..30u32)
            .map(|i| {
                let id = i * 2 + (i / 7);
                let payload: Vec<u8> = (0..(i * 97 + 1)).map(|n| (n * 13 + i) as u8).collect();
                (id, payload, 0x1_0000 + i as i32 * 3)
            })
            .collect()
    }
    fn server() -> FakeServer {
        let mut server = FakeServer::new(4);
        server.add_archive(3, 77, &groups());
        server
    }

    #[test]
    fn an_empty_store_is_filled_and_every_group_gets_its_version_trailer() {
        let mut rig = Rig::new(server());
        let mut provider = rig.provider(3);
        provider.prefetch_all();
        assert!(provider.verifying());
        rig.run(&mut provider);
        assert_eq!(provider.verified_groups(), 30);
        assert_eq!(provider.total_verified_groups(), 30);
        assert_eq!(provider.index_size(), 30);
        // The index went to archive 255 exactly as received: no trailer.
        assert_eq!(rig.stored(255, 3).unwrap(), rig.server.index_container(3));
        for (id, _, version) in groups() {
            let stored = rig.stored(3, id).unwrap();
            let mut expected = rig.server.group_container(3, id as u16);
            expected.extend([(version >> 8) as u8, version as u8]);
            assert_eq!(stored, expected);
            assert_eq!(provider.group_status(id), 1);
        }
        // 1 index + 30 groups, each asked for once; groups at prefetch priority.
        assert_eq!(rig.server.urgent_requests, 1);
        assert_eq!(rig.server.prefetch_requests, 30);
        assert!(rig.server.peak_outstanding <= 20);
        assert_eq!((rig.net.errors, rig.net.response), (0, 0));
    }

    #[test]
    fn a_good_store_is_trusted_and_only_damaged_groups_are_fetched_again() {
        let mut rig = Rig::new(server());
        let mut provider = rig.provider(3);
        provider.prefetch_all();
        rig.run(&mut provider);
        let all = groups();
        let (stale_version, wrong_crc, missing) = (all[2].0, all[9].0, all[20].0);
        {
            let mut store = rig.store.lock().unwrap();
            // The trailer is right for the CRC but not the index's version.
            let mut bytes = store.read(3, stale_version, ARCHIVE_MAX_GROUP).unwrap();
            let end = bytes.len();
            bytes[end - 1] ^= 1;
            assert!(store.write(3, stale_version, &bytes, ARCHIVE_MAX_GROUP));
            // One payload byte differs.
            let mut bytes = store.read(3, wrong_crc, ARCHIVE_MAX_GROUP).unwrap();
            bytes[6] ^= 0x40;
            assert!(store.write(3, wrong_crc, &bytes, ARCHIVE_MAX_GROUP));
            // Too short to hold anything but a trailer.
            assert!(store.write(3, missing, &[0, 0], ARCHIVE_MAX_GROUP));
        }
        // A second start: the index comes from the store, not the server.
        rig.server.reset_counters();
        let mut provider = rig.provider(3);
        provider.prefetch_all();
        let before = rig.server.urgent_requests;
        rig.run(&mut provider);
        assert_eq!(rig.server.urgent_requests, before);
        let mut fetched = rig.server.requested.clone();
        fetched.sort();
        let mut expected: Vec<_> = [stale_version, wrong_crc, missing]
            .into_iter()
            .map(|group| (3u8, group as u16, false))
            .collect();
        expected.sort();
        assert_eq!(fetched, expected);
        assert_eq!(provider.verified_groups(), 30);
        for (id, _, version) in groups() {
            let stored = rig.stored(3, id).unwrap();
            assert_eq!(
                stored[stored.len() - 2..],
                [(version >> 8) as u8, version as u8]
            );
            assert_eq!(
                stored[..stored.len() - 2],
                rig.server.group_container(3, id as u16)
            );
        }
    }

    #[test]
    fn without_prefetch_all_the_store_is_verified_and_nothing_is_downloaded() {
        let mut rig = Rig::new(server());
        let mut provider = rig.provider(3);
        rig.run(&mut provider);
        assert_eq!(provider.verified_groups(), 0);
        assert_eq!(rig.server.prefetch_requests, 0);
        let (id, _, version) = groups()[4].clone();
        assert_eq!(provider.group_status(id), -1);
        // Asked for now: an urgent request, verified, stored and handed over.
        let mut data = None;
        for _ in 0..50 {
            data = provider.fetch_group(id, &mut rig.net, &rig.queue).unwrap();
            if data.is_some() {
                break;
            }
            rig.cycle(&mut provider);
        }
        let data = data.expect("urgent group");
        assert_eq!(
            data[data.len() - 2..],
            [(version >> 8) as u8, version as u8]
        );
        assert_eq!(rig.server.requested.last(), Some(&(3, id as u16, true)));
        rig.queue.flush();
        assert_eq!(rig.stored(3, id).unwrap(), *data);
        assert_eq!(provider.group_status(id), 1);
        // The next request for it is answered by the store.
        let count = rig.server.requested.len();
        let again = provider.fetch_group(id, &mut rig.net, &rig.queue).unwrap();
        assert_eq!(again.as_deref(), Some(&*data));
        assert_eq!(rig.server.requested.len(), count);
        // Background requests for single groups are ignored once settled.
        provider.prefetch_group(groups()[5].0);
        rig.cycle(&mut provider);
        assert_eq!(rig.server.prefetch_requests, 0);
    }

    #[test]
    fn prefetch_group_jumps_the_queue_of_an_archive_that_is_downloading() {
        let mut rig = Rig::new(server());
        let mut provider = rig.provider(3);
        provider.prefetch_all();
        let wanted = groups()[8].0;
        provider.prefetch_group(wanted);
        provider.prefetch_group(wanted);
        provider.prefetch_group(99_999); // not in the index: dropped
        rig.run(&mut provider);
        assert_eq!(
            rig.server.requested[..2],
            [(255, 3, true), (3, wanted as u16, false)]
        );
        assert_eq!(rig.server.prefetch_requests, 30);
        assert_eq!(provider.group_status(wanted), 1);
    }

    /// The original only looks at single-group requests while the archive is
    /// verifying or downloading. One that finishes after the archive has
    /// settled is never stored, and the sweep then trips its own assertion.
    #[test]
    fn prefetch_group_on_a_settled_archive_is_never_taken_up() {
        let mut rig = Rig::new(server());
        let mut provider = rig.provider(3);
        let wanted = groups()[8].0;
        provider.prefetch_group(wanted);
        for _ in 0..30 {
            rig.cycle(&mut provider);
        }
        assert!(!provider.busy());
        assert_eq!(
            rig.server.requested,
            [(255, 3, true), (3, wanted as u16, false)]
        );
        assert_eq!(provider.group_status(wanted), -1);
        assert_eq!(rig.stored(3, wanted), None);
        rig.now += 1000;
        assert_eq!(provider.tick(&mut rig.net, &rig.queue, rig.now), Ok(()));
        rig.now += 1000;
        assert_eq!(
            provider.tick(&mut rig.net, &rig.queue, rig.now),
            Err(ProviderError::OrphanedPrefetch)
        );
    }

    #[test]
    fn a_corrupt_download_rekeys_counts_an_error_and_is_asked_for_again() {
        let mut rig = Rig::new(server());
        // The last group: nothing good follows it in the same cycle, so the
        // error it causes is still visible when the cycle ends.
        let victim = groups()[29].0 as u16;
        rig.server.corrupt_once.insert((3, victim));
        let mut provider = rig.provider(3);
        provider.prefetch_all();
        let mut worst = (0, 0, 0u8);
        while provider.busy() {
            rig.cycle(&mut provider);
            if rig.net.errors > worst.0 {
                worst = (rig.net.errors, rig.net.response, rig.net.encryption_key());
            }
        }
        // (byte) (0.25 * 255.0 + 1.0) = 64; the server was told the key.
        assert_eq!(worst, (1, -1, 64));
        assert_eq!(rig.server.key, 64);
        assert_eq!(rig.server.connections, 2);
        // The first good group after the failure clears the counter.
        assert_eq!((rig.net.errors, rig.net.response), (0, 0));
        assert_eq!(provider.verified_groups(), 30);
        let asked = rig
            .server
            .requested
            .iter()
            .filter(|request| **request == (3, victim, false))
            .count();
        assert!(asked >= 2);
        for (id, ..) in groups() {
            assert!(rig.stored(3, id).is_some());
        }
    }

    #[test]
    fn a_stored_index_is_kept_only_with_the_masters_crc_and_version() {
        let mut rig = Rig::new(server());
        let mut provider = rig.provider(3);
        rig.run(&mut provider);
        assert_eq!(rig.server.requested, [(255, 3, true)]);
        // Same index bytes, but the master now names another version.
        rig.server.reset_counters();
        let (checksum, version) = rig.server.master_entry(3);
        let mut provider = ResourceProvider::new(
            3,
            Some(rig.cache(3)),
            Some(rig.cache(255)),
            &rig.queue,
            checksum,
            version + 1,
            true,
        );
        rig.run(&mut provider);
        assert_eq!(rig.server.requested, [(255, 3, true)]);
        // A different CRC in the master: the stored index is refused, and so
        // is the server's copy, which costs an error each time it arrives.
        rig.server.reset_counters();
        let mut provider = ResourceProvider::new(
            3,
            Some(rig.cache(3)),
            Some(rig.cache(255)),
            &rig.queue,
            checksum ^ 1,
            version,
            true,
        );
        for _ in 0..40 {
            rig.cycle(&mut provider);
        }
        assert!(provider.index().is_none());
        assert!(rig.net.errors >= 2);
        assert_eq!(rig.net.response, -1);
    }

    #[test]
    fn verification_keeps_at_most_250_disk_reads_queued() {
        let mut server = FakeServer::new(4);
        let many: Vec<_> = (0..600u32).map(|id| (id, vec![id as u8; 3], 1)).collect();
        server.add_archive(3, 5, &many);
        let mut rig = Rig::new(server);
        let mut provider = rig.provider(3);
        // Obtain the index, then stop running disk work.
        for _ in 0..5 {
            rig.cycle(&mut provider);
        }
        assert!(provider.index().is_some());
        let mut provider = rig.provider(3);
        provider.tick(&mut rig.net, &rig.queue, 0).unwrap();
        assert_eq!(rig.queue.size(), 250);
        assert_eq!(provider.total_verified_groups(), 0);
        provider.tick(&mut rig.net, &rig.queue, 0).unwrap();
        assert_eq!(rig.queue.size(), 250);
        for _ in 0..100 {
            assert!(rig.queue.run_one());
        }
        provider.tick(&mut rig.net, &rig.queue, 0).unwrap();
        assert_eq!(rig.queue.size(), 250);
        // The oldest group still unread is what the original reports here.
        assert_eq!(provider.total_verified_groups(), 100);
        assert!(provider.verifying());
    }

    #[test]
    fn a_finished_urgent_request_nobody_collects_is_dropped_after_two_sweeps() {
        let mut rig = Rig::new(server());
        let mut provider = rig.provider(3);
        rig.run(&mut provider);
        let id = groups()[1].0;
        assert!(
            provider
                .fetch_group(id, &mut rig.net, &rig.queue)
                .unwrap()
                .is_none()
        );
        for _ in 0..10 {
            rig.cycle(&mut provider);
        }
        // Completed and still tracked: its progress is visible.
        assert_eq!(provider.percentage_complete(id), 100);
        rig.now += 1000;
        provider.tick(&mut rig.net, &rig.queue, rig.now).unwrap(); // marked
        assert_eq!(provider.percentage_complete(id), 100);
        rig.now += 1000;
        provider.tick(&mut rig.net, &rig.queue, rig.now).unwrap(); // dropped
        assert_eq!(provider.percentage_complete(id), 0);
    }

    #[test]
    fn the_master_index_gives_each_archive_its_crc_and_version() {
        let mut rig = Rig::new(server());
        let mut master = MasterIndex::new(&mut rig.net);
        assert!(!master.is_ready(&mut rig.net));
        assert_eq!(master.entry(3), None);
        for _ in 0..5 {
            rig.now += 20;
            rig.net.tick(rig.now);
            rig.server.serve(&rig.socket);
        }
        assert!(master.is_ready(&mut rig.net));
        assert_eq!(master.archive_count(), 4);
        assert_eq!(master.entry(3), Some(rig.server.master_entry(3)));
        assert_eq!(master.entry(0), Some((0, 0)));
        assert_eq!(master.entry(4), None);
        assert_eq!(rig.server.requested, [(255, 255, true)]);
        let cache = rig.cache(3);
        let master_cache = rig.cache(255);
        assert!(
            master
                .resource_provider(
                    4,
                    Some(cache.clone()),
                    Some(master_cache.clone()),
                    &rig.queue
                )
                .is_none()
        );
        master
            .resource_provider(3, Some(cache), Some(master_cache), &rig.queue)
            .unwrap()
            .prefetch_all();
        let mut archive = Js5Archive::new();
        for _ in 0..200 {
            rig.now += 20;
            rig.net.tick(rig.now);
            rig.server.serve(&rig.socket);
            master
                .process_resource_providers(&mut rig.net, &rig.queue, rig.now)
                .unwrap();
            rig.queue.flush();
        }
        let provider = master.provider_mut(3).unwrap();
        assert!(!provider.busy());
        // The Js5 view: everything is fetched from the store, group by group.
        assert!(
            archive
                .fetch_all(provider, &mut rig.net, &rig.queue)
                .unwrap()
        );
        assert_eq!(
            archive.percentage_complete(provider, &mut rig.net, &rig.queue),
            100
        );
        assert_eq!(archive.checksum(), Some(rig.server.master_entry(3).0));
        assert_eq!(archive.group_id("anything"), -1);
        let first = groups()[0].0 as i32;
        assert!(
            archive
                .is_group_ready(first, provider, &mut rig.net, &rig.queue)
                .unwrap()
        );
        assert!(
            archive
                .is_file_ready(first, provider, &mut rig.net, &rig.queue)
                .unwrap()
        );
        assert!(
            !archive
                .is_group_ready(1, provider, &mut rig.net, &rig.queue)
                .unwrap()
        );
        assert!(
            !archive
                .is_file_ready(-1, provider, &mut rig.net, &rig.queue)
                .unwrap()
        );
        assert_eq!(archive.group_percentage_complete(1, provider), 0);
    }
}
