//! Read-only 530 sector store. Handles are private; every disk read uses an explicit offset.
use crate::{CacheResult, Limits, disk::DiskStore};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io,
    path::Path,
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy, Debug)]
pub enum StorageMode {
    Resident,
    File,
}

enum Data {
    Resident(Vec<u8>),
    File {
        handle: File,
        length: u64,
    },
    /// The store a running client is still writing; see [`Store::live`].
    Live(Arc<Mutex<DiskStore>>),
}
impl Data {
    fn len(&self) -> u64 {
        match self {
            Self::Resident(data) => data.len() as u64,
            Self::File { length, .. } => *length,
            Self::Live(store) => store.lock().map_or(0, |store| store.data_length()),
        }
    }
    fn read(&self, offset: u64, destination: &mut [u8]) -> io::Result<()> {
        if offset
            .checked_add(destination.len() as u64)
            .is_none_or(|end| end > self.len())
        {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        match self {
            Self::Resident(data) => {
                destination
                    .copy_from_slice(&data[offset as usize..offset as usize + destination.len()]);
                Ok(())
            }
            Self::File { handle, .. } => read_file_at(handle, offset, destination),
            // Groups of a live store are read whole, through the store itself.
            Self::Live(_) => Err(io::ErrorKind::Unsupported.into()),
        }
    }
}

#[cfg(any(windows, unix))]
fn read_file_at(handle: &File, mut offset: u64, mut destination: &mut [u8]) -> io::Result<()> {
    while !destination.is_empty() {
        #[cfg(windows)]
        let result = std::os::windows::fs::FileExt::seek_read(handle, destination, offset);
        #[cfg(unix)]
        let result = std::os::unix::fs::FileExt::read_at(handle, destination, offset);
        match result {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(count) => {
                offset += count as u64;
                destination = &mut destination[count..];
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
#[cfg(not(any(windows, unix)))]
fn read_file_at(_: &File, _: u64, _: &mut [u8]) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "use the resident JS5 store on this platform",
    ))
}

/// Groups a live store was asked for before they had arrived, for the
/// client to ask the update server for (as `Js5.fetchGroup` does for a group
/// its store lacks). Clones share one set.
#[derive(Clone, Debug, Default)]
pub struct Misses(Arc<Mutex<BTreeSet<(u8, u32)>>>);
impl Misses {
    /// The groups recorded since the last call.
    pub fn take(&self) -> BTreeSet<(u8, u32)> {
        self.0
            .lock()
            .map(|mut set| std::mem::take(&mut *set))
            .unwrap_or_default()
    }
    fn record(&self, archive: u8, group: u32) {
        if let Ok(mut set) = self.0.lock() {
            set.insert((archive, group));
        }
    }
}

thread_local! {
    static MISSED: Cell<u64> = const { Cell::new(0) };
}
/// How many reads on the calling thread have met a group that a live store
/// did not have yet. A reader that can wait compares it before and after a
/// piece of work to learn that the work met data still on its way.
pub fn misses_on_this_thread() -> u64 {
    MISSED.with(Cell::get)
}

/// Clone cheaply and share across threads/clients. Index tables are immutable.
#[derive(Clone)]
pub struct Store {
    data: Arc<Data>,
    indices: Arc<BTreeMap<u8, Vec<u8>>>,
    max_container: usize,
    misses: Option<Misses>,
}
impl Store {
    pub fn open(directory: &Path, mode: StorageMode, limits: Limits) -> io::Result<Self> {
        let path = directory.join("main_file_cache.dat2");
        let data = match mode {
            StorageMode::Resident => Data::Resident(fs::read(path)?),
            StorageMode::File => {
                let handle = File::open(path)?;
                let length = handle.metadata()?.len();
                Data::File { handle, length }
            }
        };
        let mut indices = BTreeMap::new();
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if let Some(archive) = name
                .strip_prefix("main_file_cache.idx")
                .and_then(|number| number.parse::<u8>().ok())
            {
                indices.insert(archive, fs::read(path)?);
            }
        }
        Ok(Self {
            data: Arc::new(data),
            indices: Arc::new(indices),
            max_container: limits.max_container,
            misses: None,
        })
    }
    pub fn from_bytes(data: Vec<u8>, indices: BTreeMap<u8, Vec<u8>>, limits: Limits) -> Self {
        Self {
            data: Arc::new(Data::Resident(data)),
            indices: Arc::new(indices),
            max_container: limits.max_container,
            misses: None,
        }
    }
    /// A reading view of a store that is still being filled: every read sees
    /// what has been written so far, and a group that has not arrived reads
    /// as absent. For the browser client, whose cache lives in memory while
    /// the update connection fills it.
    pub fn live(store: Arc<Mutex<DiskStore>>, limits: Limits) -> Self {
        Self {
            data: Arc::new(Data::Live(store)),
            indices: Arc::new(BTreeMap::new()),
            max_container: limits.max_container,
            misses: None,
        }
    }
    /// [`Self::live`], recording in `misses` every group it is asked for
    /// before the group has arrived.
    pub fn live_recording(store: Arc<Mutex<DiskStore>>, limits: Limits, misses: Misses) -> Self {
        Self {
            misses: Some(misses),
            ..Self::live(store, limits)
        }
    }
    pub fn archives(&self) -> impl Iterator<Item = u8> + '_ {
        let live: Vec<u8> = match &*self.data {
            Data::Live(store) => store
                .lock()
                .map(|store| store.archives().collect())
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        self.indices.keys().copied().chain(live)
    }
    pub fn group_slots(&self, archive: u8) -> usize {
        if let Data::Live(store) = &*self.data {
            return store.lock().map_or(0, |store| store.group_slots(archive));
        }
        self.indices
            .get(&archive)
            .map_or(0, |index| index.len() / 6)
    }
    /// Invalid chains return None, like Cache.read; operating-system I/O errors are reported.
    pub fn read(&self, archive: u8, group: u32) -> CacheResult<Option<Vec<u8>>> {
        // This revision uses the eight-byte header, with a 16-bit group ID.
        if group > u32::from(u16::MAX) {
            return Ok(None);
        }
        if let Data::Live(store) = &*self.data {
            let found = store.lock().map_err(|_| crate::CacheError::Poisoned)?.read(
                archive,
                group,
                self.max_container,
            );
            if found.is_none() {
                MISSED.with(|count| count.set(count.get().wrapping_add(1)));
                if let Some(misses) = &self.misses {
                    misses.record(archive, group);
                }
            }
            return Ok(found);
        }
        let Some(index) = self.indices.get(&archive) else {
            return Ok(None);
        };
        let offset = group as usize * 6;
        let Some(entry) = index.get(offset..offset + 6) else {
            return Ok(None);
        };
        let length = u24(&entry[..3]) as usize;
        let mut sector = u24(&entry[3..]);
        let sectors = self.data.len() / 520;
        if length > self.max_container || sector == 0 || u64::from(sector) > sectors {
            return Ok(None);
        }
        let mut output = vec![0; length];
        let mut position = 0;
        let mut part = 0u32;
        let mut buffer = [0u8; 520];
        while position < length {
            if sector == 0 {
                return Ok(None);
            }
            let chunk = (length - position).min(512);
            match self
                .data
                .read(u64::from(sector) * 520, &mut buffer[..8 + chunk])
            {
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
                result => result?,
            }
            let header_group = u32::from(u16::from_be_bytes([buffer[0], buffer[1]]));
            let header_part = u32::from(u16::from_be_bytes([buffer[2], buffer[3]]));
            let next_sector = u24(&buffer[4..7]);
            if header_group != group
                || header_part != part
                || buffer[7] != archive
                || u64::from(next_sector) > sectors
            {
                return Ok(None);
            }
            output[position..position + chunk].copy_from_slice(&buffer[8..8 + chunk]);
            position += chunk;
            part += 1;
            sector = next_sector;
        }
        Ok(Some(output))
    }
}
fn u24(bytes: &[u8]) -> u32 {
    (u32::from(bytes[0]) << 16) | (u32::from(bytes[1]) << 8) | u32::from(bytes[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_live_store_records_the_groups_it_did_not_have_yet() {
        let mut disk = DiskStore::in_memory(Vec::new(), BTreeMap::new());
        disk.ensure_archive(7).unwrap();
        assert!(disk.write(7, 3, b"model three", 1 << 20));
        let disk = Arc::new(Mutex::new(disk));
        let misses = Misses::default();
        let store = Store::live_recording(Arc::clone(&disk), Limits::default(), misses.clone());
        let before = misses_on_this_thread();
        assert_eq!(
            store.read(7, 3).unwrap().as_deref(),
            Some(&b"model three"[..])
        );
        assert_eq!(
            misses_on_this_thread(),
            before,
            "a present group is no miss"
        );
        assert_eq!(store.read(7, 4).unwrap(), None);
        assert_eq!(store.read(7, 4).unwrap(), None);
        assert_eq!(misses_on_this_thread(), before + 2);
        assert_eq!(misses.take().into_iter().collect::<Vec<_>>(), [(7, 4)]);
        assert!(misses.take().is_empty(), "taken once");
        // Once written it reads as any other group.
        assert!(disk.lock().unwrap().write(7, 4, b"four", 1 << 20));
        assert_eq!(store.read(7, 4).unwrap().as_deref(), Some(&b"four"[..]));
        assert!(misses.take().is_empty());
    }
}
