//! The cache directory an update is still filling while the client runs.
//!
//! The original opens its login screen once `client.mainLoad` has what it
//! needs and keeps downloading the rest in the background (`createJs5(...,
//! prefetchAll, ...)`). The desktop client does the same since the start
//! gate (`cache_update::start_archives`): from then on every reader that
//! would open the cache directory by its path reads the update's own store
//! instead, through [`store_for`], so it sees each group as it arrives and
//! has a group it meets before then asked for (`Store::live_recording`).
use rs09_cache::store::Store;
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

static LIVE: Mutex<Option<(PathBuf, Store)>> = Mutex::new(None);

fn key(directory: &Path) -> PathBuf {
    std::fs::canonicalize(directory).unwrap_or_else(|_| directory.to_path_buf())
}

/// From now on `directory` is read through `store`.
pub fn install(directory: &Path, store: Store) {
    if let Ok(mut live) = LIVE.lock() {
        *live = Some((key(directory), store));
    }
}

/// The live store of `directory`, if an update is filling it.
pub fn store_for(directory: &Path) -> Option<Store> {
    let live = LIVE.lock().ok()?;
    let (path, store) = live.as_ref()?;
    (*path == key(directory)).then(|| store.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs09_cache::{Limits, disk::DiskStore};
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
    };

    #[test]
    fn only_the_directory_being_filled_is_read_live() {
        let disk = Arc::new(Mutex::new(DiskStore::in_memory(
            Vec::new(),
            BTreeMap::new(),
        )));
        let here = std::env::temp_dir();
        install(&here, Store::live(disk, Limits::default()));
        assert!(store_for(&here).is_some());
        assert!(store_for(&here.join(".")).is_some(), "the same directory");
        assert!(store_for(Path::new("no-such-cache-directory")).is_none());
        *LIVE.lock().unwrap() = None;
        assert!(store_for(&here).is_none());
    }
}
