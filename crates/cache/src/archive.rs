//! Shared immutable groups. Slow I/O/decompression never runs under the cache map lock.
use crate::{
    CacheError, CacheResult, Limits,
    group::{self, GroupFiles},
    index::Index,
    store::Store,
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

type Key = (u32, [u32; 4]);
type Cell = Arc<OnceLock<CacheResult<Arc<GroupFiles>>>>;

pub struct Archive {
    store: Store,
    id: u8,
    pub index: Index,
    limits: Limits,
    failure_policy: group::FailurePolicy,
    groups: Mutex<BTreeMap<Key, Cell>>,
    cached_bytes: AtomicUsize,
}
impl Archive {
    pub fn open(
        store: Store,
        id: u8,
        expected_checksum: Option<u32>,
        limits: Limits,
    ) -> CacheResult<Self> {
        Self::open_with_policy(
            store,
            id,
            expected_checksum,
            limits,
            group::FailurePolicy::Strict,
        )
    }
    pub fn open_with_policy(
        store: Store,
        id: u8,
        expected_checksum: Option<u32>,
        limits: Limits,
        failure_policy: group::FailurePolicy,
    ) -> CacheResult<Self> {
        let data = store.read(255, u32::from(id))?.ok_or(CacheError::Missing {
            archive: 255,
            group: u32::from(id),
        })?;
        let index = Index::decode(&data, expected_checksum, limits)?;
        Ok(Self {
            store,
            id,
            index,
            limits,
            failure_policy,
            groups: Mutex::new(BTreeMap::new()),
            cached_bytes: AtomicUsize::new(0),
        })
    }
    pub fn group(&self, id: u32, key: [u32; 4]) -> CacheResult<Option<Arc<GroupFiles>>> {
        let Some(info) = self.index.group(id).filter(|info| info.capacity != 0) else {
            return Ok(None);
        };
        let cache_key = (id, key);
        let cell = {
            let mut groups = self.groups.lock().map_err(|_| CacheError::Poisoned)?;
            if let Some(cell) = groups.get(&cache_key) {
                Some(Arc::clone(cell))
            } else if groups.len() < self.limits.cached_groups {
                let cell = Arc::new(OnceLock::new());
                groups.insert(cache_key, Arc::clone(&cell));
                Some(cell)
            } else {
                None
            }
        };
        let load = || -> CacheResult<Arc<GroupFiles>> {
            let bytes = self.store.read(self.id, id)?.ok_or(CacheError::Missing {
                archive: self.id,
                group: id,
            })?;
            Ok(Arc::new(group::unpack_with_policy(
                &bytes,
                info,
                key,
                self.limits,
                self.failure_policy,
            )?))
        };
        let Some(cell) = cell else {
            return load().map(Some);
        };
        // Multiple readers of one group share exactly one in-flight decode.
        let result = cell.get_or_init(|| {
            let result = load();
            // `try_update`, its newer name, does not exist at the pinned 1.86.
            #[allow(deprecated)]
            let retain = if let Ok(files) = &result {
                self.cached_bytes
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                        used.checked_add(files.retained_bytes())
                            .filter(|&total| total <= self.limits.cached_bytes)
                    })
                    .is_ok()
            } else {
                false
            };
            if !retain {
                // The caller still gets valid data when the shared cache is full.
                if let Ok(mut groups) = self.groups.lock() {
                    groups.remove(&cache_key);
                }
            }
            result
        });
        result.clone().map(Some)
    }
    pub fn file(&self, group: u32, file: u32, key: [u32; 4]) -> CacheResult<Option<Arc<Vec<u8>>>> {
        if self
            .index
            .group(group)
            .and_then(|group| group.file(file))
            .is_none()
        {
            return Ok(None);
        }
        Ok(self.group(group, key)?.and_then(|group| group.get(file)))
    }
    pub fn cached_bytes(&self) -> usize {
        self.cached_bytes.load(Ordering::Relaxed)
    }
}
