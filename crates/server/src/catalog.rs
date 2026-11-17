//! Shared packed responses. Cache decoding and simulation are outside this service.
use bytes::Bytes;
use rs09_cache::{CacheError, CacheResult, Limits, index::Index, store::Store};
use rs09_protocol::{
    Error,
    js5::{self, Request},
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

type Key = (u8, u16, bool);
type Cell = Arc<OnceLock<CacheResult<Option<Bytes>>>>;
pub struct Catalog {
    store: Store,
    master: Bytes,
    available: Vec<bool>,
    max_payload: usize,
    max_entries: usize,
    max_bytes: usize,
    frames: Mutex<BTreeMap<Key, Cell>>,
    retained: AtomicUsize,
    loads: AtomicUsize,
}
impl Catalog {
    pub fn new(
        store: Store,
        limits: Limits,
        max_entries: usize,
        max_bytes: usize,
    ) -> CacheResult<Self> {
        let count = store.group_slots(255);
        if count == 0 || count > 256 {
            return Err(Error::new(0, "invalid master index slot count").into());
        }
        let mut entries = Vec::with_capacity(count);
        for id in 0..count {
            let entry = if store.group_slots(id as u8) == 0 {
                None
            } else if let Some(data) = store.read(255, id as u32)? {
                let index = Index::decode(&data, None, limits)?;
                Some((index.checksum, index.version))
            } else {
                None
            };
            entries.push(entry);
        }
        let payload = js5::master_payload(&entries);
        let available = entries.iter().map(Option::is_some).collect();
        let master = js5::encode_master(&payload, limits.max_container)?;
        Ok(Self {
            store,
            master: master.into(),
            available,
            max_payload: 1_000_000,
            max_entries,
            max_bytes,
            frames: Mutex::new(BTreeMap::new()),
            retained: AtomicUsize::new(0),
            loads: AtomicUsize::new(0),
        })
    }
    pub fn reply(&self, request: Request, key: u8) -> CacheResult<Option<Bytes>> {
        if request.archive == 255 && request.group == 255 {
            return Ok(Some(self.master.clone()));
        }
        if request.archive != 255
            && !self
                .available
                .get(usize::from(request.archive))
                .copied()
                .unwrap_or(false)
        {
            return Err(Error::new(0, "cache archive is unavailable").into());
        }
        let tag = (request.archive, request.group, request.urgent);
        let cell = {
            let mut frames = self.frames.lock().map_err(|_| CacheError::Poisoned)?;
            if let Some(cell) = frames.get(&tag) {
                Some(Arc::clone(cell))
            } else if frames.len() < self.max_entries {
                let cell = Arc::new(OnceLock::new());
                frames.insert(tag, Arc::clone(&cell));
                Some(cell)
            } else {
                None
            }
        };
        let load = || -> CacheResult<Option<Bytes>> {
            self.loads.fetch_add(1, Ordering::Relaxed);
            let Some(packed) = self.store.read(request.archive, u32::from(request.group))? else {
                return Ok(None);
            };
            let limit = if request.archive == 255 {
                500_000
            } else {
                1_000_000
            };
            if packed.len() > limit {
                return Ok(None);
            }
            Ok(Some(
                js5::encode_archive(request, 0, &packed, self.max_payload)?.into(),
            ))
        };
        let result = if let Some(cell) = cell {
            cell.get_or_init(|| {
                let result = load();
                let retained = if let Ok(Some(bytes)) = &result {
                    self.retained
                        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                            used.checked_add(bytes.len())
                                .filter(|&total| total <= self.max_bytes)
                        })
                        .is_ok()
                } else {
                    false
                };
                if !retained {
                    if let Ok(mut frames) = self.frames.lock() {
                        frames.remove(&tag);
                    }
                }
                result
            })
            .clone()
        } else {
            load()
        };
        let Some(bytes) = result? else {
            return Ok(None);
        };
        if key == 0 {
            return Ok(Some(bytes));
        }
        let encoded: Vec<_> = bytes.iter().map(|&byte| byte ^ key).collect();
        Ok(Some(encoded.into()))
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained.load(Ordering::Relaxed)
    }
    /// Completed unencrypted frames need neither a blocking job nor a fresh byte buffer.
    pub fn cached_reply(&self, request: Request, key: u8) -> Option<Bytes> {
        if request.archive == 255 && request.group == 255 {
            return Some(self.master.clone());
        }
        if key != 0 {
            return None;
        }
        let frames = self.frames.lock().ok()?;
        frames
            .get(&(request.archive, request.group, request.urgent))?
            .get()?
            .as_ref()
            .ok()?
            .as_ref()
            .cloned()
    }
    pub fn loads(&self) -> usize {
        self.loads.load(Ordering::Relaxed)
    }
}
