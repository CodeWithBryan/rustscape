//! Read/write 530 sector store: a port of `rt4.Cache` over files that stay open.
//!
//! [`crate::store::Store`] reads an immutable snapshot; this is the store the
//! running client updates while it downloads. One data file is shared by every
//! archive, as in the original, so all access goes through one lock.
//!
//! Differences from the Java, all outside valid input:
//! * `FileOnDisk` deletes a file that has reached its size limit when it is
//!   opened and plants a marker byte past the limit when a write would exceed
//!   it. Here opening an oversize file is an error and an oversize write just
//!   fails; nothing is deleted.
//! * Java computes `sector * 520` in 32 bits. The data limit is kept below
//!   2 GiB, where the two agree.
#[cfg(any(windows, unix))]
use std::path::Path;
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::PathBuf,
    sync::{Arc, Mutex},
};

/// `SignLink` opens `main_file_cache.dat2` with this limit.
pub const DATA_MAX_LENGTH: u64 = 104_857_600;
/// `SignLink` opens every `main_file_cache.idx*` with this limit.
pub const INDEX_MAX_LENGTH: u64 = 1_048_576;
/// `client.createJs5`: longest group an archive's `Cache` reads or writes.
pub const ARCHIVE_MAX_GROUP: usize = 1_000_000;
/// `client.mainInit`: longest index the master `Cache` (archive 255) accepts.
pub const MASTER_MAX_GROUP: usize = 500_000;

const SECTOR: u64 = 520;
const PAYLOAD: usize = 512;
/// An in-memory store remembers which pieces of this size its writes touched,
/// so that whoever mirrors it elsewhere (the browser's storage) copies only
/// those. Not part of the original, which writes to the files themselves.
pub const MIRROR_CHUNK: usize = 1 << 20;

enum Backing {
    Memory(Vec<u8>),
    #[cfg(any(windows, unix))]
    File(std::fs::File),
}

/// `BufferedFile` as `Cache` uses it: a length that grows with writes, reads
/// that fail when they run past the end, and the `FileOnDisk` size limit.
struct Access {
    backing: Backing,
    length: u64,
    max_length: u64,
    /// [`MIRROR_CHUNK`] pieces written since [`DiskStore::take_dirty`]; kept
    /// for a memory backing only.
    dirty: BTreeSet<u32>,
}
impl Access {
    fn memory(bytes: Vec<u8>, max_length: u64) -> Self {
        Self {
            length: bytes.len() as u64,
            backing: Backing::Memory(bytes),
            max_length,
            dirty: BTreeSet::new(),
        }
    }
    #[cfg(any(windows, unix))]
    fn file(path: &Path, max_length: u64) -> io::Result<Self> {
        let handle = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        let length = handle.metadata()?.len();
        if length >= max_length {
            return Err(io::Error::other(format!(
                "{} is {length} bytes; the original client's limit is {max_length} and it would delete the file",
                path.display()
            )));
        }
        Ok(Self {
            backing: Backing::File(handle),
            length,
            max_length,
            dirty: BTreeSet::new(),
        })
    }
    /// `EOFException` is reported as `UnexpectedEof`.
    fn read(&mut self, offset: u64, destination: &mut [u8]) -> io::Result<()> {
        if offset
            .checked_add(destination.len() as u64)
            .is_none_or(|end| end > self.length)
        {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        match &mut self.backing {
            Backing::Memory(bytes) => {
                let start = offset as usize;
                destination.copy_from_slice(&bytes[start..start + destination.len()]);
                Ok(())
            }
            #[cfg(any(windows, unix))]
            Backing::File(handle) => read_at(handle, offset, destination),
        }
    }
    fn write(&mut self, offset: u64, source: &[u8]) -> io::Result<()> {
        let end = offset
            .checked_add(source.len() as u64)
            .filter(|&end| end <= self.max_length)
            .ok_or_else(|| io::Error::other("cache file size limit reached"))?;
        match &mut self.backing {
            Backing::Memory(bytes) => {
                // A seek past the end leaves zeros behind, as on disk.
                if bytes.len() < end as usize {
                    bytes.resize(end as usize, 0);
                }
                bytes[offset as usize..end as usize].copy_from_slice(source);
                if !source.is_empty() {
                    let chunk = MIRROR_CHUNK as u64;
                    self.dirty
                        .extend((offset / chunk) as u32..=((end - 1) / chunk) as u32);
                }
            }
            #[cfg(any(windows, unix))]
            Backing::File(handle) => write_at(handle, offset, source)?,
        }
        self.length = self.length.max(end);
        Ok(())
    }
}

#[cfg(any(windows, unix))]
fn read_at(handle: &std::fs::File, mut offset: u64, mut destination: &mut [u8]) -> io::Result<()> {
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
#[cfg(any(windows, unix))]
fn write_at(handle: &std::fs::File, mut offset: u64, mut source: &[u8]) -> io::Result<()> {
    while !source.is_empty() {
        #[cfg(windows)]
        let result = std::os::windows::fs::FileExt::seek_write(handle, source, offset);
        #[cfg(unix)]
        let result = std::os::unix::fs::FileExt::write_at(handle, source, offset);
        match result {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(count) => {
                offset += count as u64;
                source = &source[count..];
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn u24(bytes: &[u8]) -> u32 {
    (u32::from(bytes[0]) << 16) | (u32::from(bytes[1]) << 8) | u32::from(bytes[2])
}

/// The data file's bytes and each index file's bytes by archive.
pub type MemoryFiles = (Vec<u8>, BTreeMap<u8, Vec<u8>>);

/// The open cache files. Use [`Cache`] for the per-archive view the client holds.
pub struct DiskStore {
    data: Access,
    indexes: BTreeMap<u8, Access>,
    /// Where further index files are created; `None` for a store in memory.
    directory: Option<PathBuf>,
}
impl DiskStore {
    /// Opens `main_file_cache.dat2` and one `main_file_cache.idxN` per listed
    /// archive for reading and writing, creating whatever is absent, as
    /// `SignLink` does. The directory itself must exist.
    #[cfg(any(windows, unix))]
    pub fn open(directory: &Path, archives: impl IntoIterator<Item = u8>) -> io::Result<Self> {
        let data = Access::file(&directory.join("main_file_cache.dat2"), DATA_MAX_LENGTH)?;
        let mut indexes = BTreeMap::new();
        for archive in archives {
            indexes.insert(
                archive,
                Access::file(
                    &directory.join(format!("main_file_cache.idx{archive}")),
                    INDEX_MAX_LENGTH,
                )?,
            );
        }
        Ok(Self {
            data,
            indexes,
            directory: Some(directory.to_owned()),
        })
    }
    /// Opens (creating if absent) the index file of one more archive. The
    /// original client always opens exactly archives 0 to 27 and 255; this is
    /// for a server whose master index lists more.
    pub fn ensure_archive(&mut self, archive: u8) -> io::Result<()> {
        if self.indexes.contains_key(&archive) {
            return Ok(());
        }
        let access = match &self.directory {
            #[cfg(any(windows, unix))]
            Some(directory) => Access::file(
                &directory.join(format!("main_file_cache.idx{archive}")),
                INDEX_MAX_LENGTH,
            )?,
            #[cfg(not(any(windows, unix)))]
            Some(_) => return Err(io::ErrorKind::Unsupported.into()),
            None => Access::memory(Vec::new(), INDEX_MAX_LENGTH),
        };
        self.indexes.insert(archive, access);
        Ok(())
    }
    /// The same store held in memory, for tests and platforms without files.
    pub fn in_memory(data: Vec<u8>, indexes: BTreeMap<u8, Vec<u8>>) -> Self {
        Self {
            data: Access::memory(data, DATA_MAX_LENGTH),
            indexes: indexes
                .into_iter()
                .map(|(archive, bytes)| (archive, Access::memory(bytes, INDEX_MAX_LENGTH)))
                .collect(),
            directory: None,
        }
    }
    /// The bytes of an in-memory store: data file, then each index file.
    #[allow(irrefutable_let_patterns)] // no file backing in a browser build
    pub fn memory_bytes(&self) -> Option<MemoryFiles> {
        let Backing::Memory(data) = &self.data.backing else {
            return None;
        };
        let mut indexes = BTreeMap::new();
        for (&archive, access) in &self.indexes {
            let Backing::Memory(bytes) = &access.backing else {
                return None;
            };
            indexes.insert(archive, bytes.clone());
        }
        Some((data.clone(), indexes))
    }
    /// What an in-memory store's writes have touched since the last call:
    /// the [`MIRROR_CHUNK`] pieces of the data file, and the archives whose
    /// index file changed. Both ascending; empty for a store on disk.
    pub fn take_dirty(&mut self) -> (Vec<u32>, Vec<u8>) {
        let data = std::mem::take(&mut self.data.dirty).into_iter().collect();
        let mut indexes = Vec::new();
        for (&archive, access) in &mut self.indexes {
            if !std::mem::take(&mut access.dirty).is_empty() {
                indexes.push(archive);
            }
        }
        (data, indexes)
    }
    /// One [`MIRROR_CHUNK`] piece of an in-memory data file; the last is short.
    #[allow(clippy::infallible_destructuring_match)] // no file backing in a browser build
    pub fn data_chunk(&self, chunk: u32) -> Option<&[u8]> {
        let data = match &self.data.backing {
            Backing::Memory(data) => data,
            #[cfg(any(windows, unix))]
            Backing::File(_) => return None,
        };
        let start = (chunk as usize).checked_mul(MIRROR_CHUNK)?;
        let end = data.len().min(start.checked_add(MIRROR_CHUNK)?);
        data.get(start..end).filter(|piece| !piece.is_empty())
    }
    /// The bytes of one archive's index file in an in-memory store.
    pub fn index_bytes(&self, archive: u8) -> Option<&[u8]> {
        match &self.indexes.get(&archive)?.backing {
            Backing::Memory(bytes) => Some(bytes),
            #[cfg(any(windows, unix))]
            Backing::File(_) => None,
        }
    }
    pub fn archives(&self) -> impl Iterator<Item = u8> + '_ {
        self.indexes.keys().copied()
    }
    pub fn data_length(&self) -> u64 {
        self.data.length
    }
    /// Entries the archive's index file has room for, written or not.
    pub fn group_slots(&self, archive: u8) -> usize {
        self.indexes
            .get(&archive)
            .map_or(0, |index| (index.length / 6) as usize)
    }
    /// Asks the operating system to put everything written on stable storage.
    /// The original never does; a downloader calls this once at the end.
    pub fn sync(&self) -> io::Result<()> {
        #[cfg(any(windows, unix))]
        for access in std::iter::once(&self.data).chain(self.indexes.values()) {
            if let Backing::File(handle) = &access.backing {
                handle.sync_all()?;
            }
        }
        Ok(())
    }

    /// `Cache.read`: `None` for an absent group, a broken chain or an I/O error.
    pub fn read(&mut self, archive: u8, group: u32, max_length: usize) -> Option<Vec<u8>> {
        let index = self.indexes.get_mut(&archive)?;
        let entry_offset = u64::from(group) * 6;
        if index.length < entry_offset + 6 {
            return None;
        }
        let mut buffer = [0u8; 520];
        index.read(entry_offset, &mut buffer[..6]).ok()?;
        let length = u24(&buffer[..3]) as usize;
        let mut sector = u24(&buffer[3..6]);
        if length > max_length {
            return None;
        }
        if sector == 0 || u64::from(sector) > self.data.length / SECTOR {
            return None;
        }
        let mut output = vec![0; length];
        let mut position = 0;
        let mut part = 0u32;
        while position < length {
            if sector == 0 {
                return None;
            }
            let chunk = (length - position).min(PAYLOAD);
            self.data
                .read(u64::from(sector) * SECTOR, &mut buffer[..chunk + 8])
                .ok()?;
            let header_group = u32::from(u16::from_be_bytes([buffer[0], buffer[1]]));
            let header_part = u32::from(u16::from_be_bytes([buffer[2], buffer[3]]));
            let next_sector = u24(&buffer[4..7]);
            if group != header_group || part != header_part || archive != buffer[7] {
                return None;
            }
            if u64::from(next_sector) > self.data.length / SECTOR {
                return None;
            }
            output[position..position + chunk].copy_from_slice(&buffer[8..8 + chunk]);
            position += chunk;
            part += 1;
            sector = next_sector;
        }
        Some(output)
    }

    /// `Cache.write(group, len, src)`: first over the group's existing chain,
    /// then, if that chain is unusable, appended to the data file. `false`
    /// when nothing usable was stored. Java throws for a group longer than
    /// the limit instead of returning; neither writes anything.
    pub fn write(&mut self, archive: u8, group: u32, source: &[u8], max_length: usize) -> bool {
        if source.len() > max_length {
            return false;
        }
        self.write_chain(archive, group, source, true)
            .unwrap_or(false)
            || self
                .write_chain(archive, group, source, false)
                .unwrap_or(false)
    }

    /// The private `Cache.write(len, group, src, overwrite)`. An `Err` is the
    /// Java `IOException`, which the caller turns into `false`.
    fn write_chain(
        &mut self,
        archive: u8,
        group: u32,
        source: &[u8],
        mut overwrite: bool,
    ) -> io::Result<bool> {
        let Some(index) = self.indexes.get_mut(&archive) else {
            return Ok(false);
        };
        let length = source.len();
        let entry_offset = u64::from(group) * 6;
        let mut buffer = [0u8; 520];
        let mut sector;
        if overwrite {
            if index.length < entry_offset + 6 {
                return Ok(false);
            }
            index.read(entry_offset, &mut buffer[..6])?;
            sector = u24(&buffer[3..6]);
            if sector == 0 || self.data.length / SECTOR < u64::from(sector) {
                return Ok(false);
            }
        } else {
            sector = self.data.length.div_ceil(SECTOR) as u32;
            if sector == 0 {
                sector = 1;
            }
        }
        // The entry is rewritten before the chain is known to be usable.
        buffer[0] = (length >> 16) as u8;
        buffer[1] = (length >> 8) as u8;
        buffer[2] = length as u8;
        buffer[3] = (sector >> 16) as u8;
        buffer[4] = (sector >> 8) as u8;
        buffer[5] = sector as u8;
        index.write(entry_offset, &buffer[..6])?;
        let mut written = 0;
        let mut part = 0u32;
        while written < length {
            let mut next_sector = 0u32;
            if overwrite {
                match self.data.read(u64::from(sector) * SECTOR, &mut buffer[..8]) {
                    Ok(()) => {}
                    // As decompiled: an index entry that points at the end of
                    // the data file ends the write and still reports success.
                    // The group then fails to read and is fetched again.
                    Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                        return Ok(true);
                    }
                    Err(error) => return Err(error),
                }
                next_sector = u24(&buffer[4..7]);
                let header_group = u32::from(u16::from_be_bytes([buffer[0], buffer[1]]));
                let header_part = u32::from(u16::from_be_bytes([buffer[2], buffer[3]]));
                if header_group != group || part != header_part || archive != buffer[7] {
                    return Ok(false);
                }
                if u64::from(next_sector) > self.data.length / SECTOR {
                    return Ok(false);
                }
            }
            if next_sector == 0 {
                overwrite = false;
                next_sector = self.data.length.div_ceil(SECTOR) as u32;
                if next_sector == 0 {
                    next_sector += 1;
                }
                if next_sector == sector {
                    next_sector += 1;
                }
            }
            if length - written <= PAYLOAD {
                next_sector = 0;
            }
            let chunk = (length - written).min(PAYLOAD);
            buffer[0] = (group >> 8) as u8;
            buffer[1] = group as u8;
            buffer[2] = (part >> 8) as u8;
            buffer[3] = part as u8;
            buffer[4] = (next_sector >> 16) as u8;
            buffer[5] = (next_sector >> 8) as u8;
            buffer[6] = next_sector as u8;
            buffer[7] = archive;
            buffer[8..8 + chunk].copy_from_slice(&source[written..written + chunk]);
            part += 1;
            // Java writes the header and the payload separately; the final
            // sector is likewise left short rather than padded to 520 bytes.
            self.data
                .write(u64::from(sector) * SECTOR, &buffer[..8 + chunk])?;
            sector = next_sector;
            written += chunk;
        }
        Ok(true)
    }
}

/// `rt4.Cache`: one archive of a shared store, with its length limit.
#[derive(Clone)]
pub struct Cache {
    store: Arc<Mutex<DiskStore>>,
    archive: u8,
    max_length: usize,
}
impl Cache {
    pub fn new(store: Arc<Mutex<DiskStore>>, archive: u8, max_length: usize) -> Self {
        Self {
            store,
            archive,
            max_length,
        }
    }
    pub fn archive(&self) -> u8 {
        self.archive
    }
    pub fn store(&self) -> &Arc<Mutex<DiskStore>> {
        &self.store
    }
    /// Object identity in the Java: the same archive of the same open store.
    pub fn same(&self, other: &Cache) -> bool {
        self.archive == other.archive && Arc::ptr_eq(&self.store, &other.store)
    }
    pub fn read(&self, group: u32) -> Option<Vec<u8>> {
        self.store
            .lock()
            .ok()?
            .read(self.archive, group, self.max_length)
    }
    pub fn write(&self, group: u32, source: &[u8]) -> bool {
        self.store
            .lock()
            .is_ok_and(|mut store| store.write(self.archive, group, source, self.max_length))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Limits, store::Store};

    fn empty(archives: &[u8]) -> DiskStore {
        DiskStore::in_memory(
            Vec::new(),
            archives.iter().map(|&a| (a, Vec::new())).collect(),
        )
    }
    fn pattern(length: usize, seed: u8) -> Vec<u8> {
        (0..length)
            .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed))
            .collect()
    }

    #[test]
    fn written_groups_read_back_here_and_through_the_snapshot_reader() {
        let mut store = empty(&[3, 7]);
        let sizes = [1usize, 511, 512, 513, 1024, 1025, 2048, 65_000];
        for (i, &size) in sizes.iter().enumerate() {
            let group = [0u32, 1, 2, 255, 512, 4000, 9, 65_535][i];
            let archive = if i % 2 == 0 { 7 } else { 3 };
            assert!(store.write(archive, group, &pattern(size, i as u8), ARCHIVE_MAX_GROUP));
        }
        let (data, indexes) = store.memory_bytes().unwrap();
        let snapshot = Store::from_bytes(data, indexes, Limits::default());
        for (i, &size) in sizes.iter().enumerate() {
            let group = [0u32, 1, 2, 255, 512, 4000, 9, 65_535][i];
            let archive = if i % 2 == 0 { 7 } else { 3 };
            let expected = pattern(size, i as u8);
            assert_eq!(
                store.read(archive, group, ARCHIVE_MAX_GROUP).as_deref(),
                Some(&expected[..])
            );
            assert_eq!(
                snapshot.read(archive, group).unwrap().as_deref(),
                Some(&expected[..])
            );
        }
        // Unwritten slots below a written one hold sector zero.
        assert_eq!(store.read(7, 3, ARCHIVE_MAX_GROUP), None);
        assert_eq!(store.read(7, 70_000, ARCHIVE_MAX_GROUP), None);
        assert_eq!(store.read(9, 0, ARCHIVE_MAX_GROUP), None);
    }

    #[test]
    fn the_first_sector_is_one_and_the_last_is_left_short() {
        let mut store = empty(&[0]);
        assert!(store.write(0, 0, &pattern(513, 0), ARCHIVE_MAX_GROUP));
        let (data, indexes) = store.memory_bytes().unwrap();
        // Sector 0 is never used: 520 zero bytes, then 520 + 8 + 1.
        assert_eq!(data.len(), 520 + 520 + 9);
        assert!(data[..520].iter().all(|&b| b == 0));
        assert_eq!(&data[520..528], &[0, 0, 0, 0, 0, 0, 2, 0]);
        assert_eq!(&data[1040..1048], &[0, 0, 0, 1, 0, 0, 0, 0]);
        assert_eq!(indexes[&0], vec![0, 2, 1, 0, 0, 1]);
        // The next group starts on the following sector boundary.
        assert!(store.write(0, 1, &[9], ARCHIVE_MAX_GROUP));
        let (data, indexes) = store.memory_bytes().unwrap();
        assert_eq!(&indexes[&0][6..], &[0, 0, 1, 0, 0, 3]);
        assert_eq!(data.len(), 3 * 520 + 9);
        assert_eq!(&data[1560..], &[0, 1, 0, 0, 0, 0, 0, 0, 9]);
    }

    #[test]
    fn overwriting_reuses_the_chain_then_appends_what_does_not_fit() {
        let mut store = empty(&[5]);
        assert!(store.write(5, 0, &pattern(1000, 1), ARCHIVE_MAX_GROUP));
        assert!(store.write(5, 1, &pattern(100, 2), ARCHIVE_MAX_GROUP));
        let before = store.data_length();
        // Shorter and equal lengths stay inside the old sectors.
        assert!(store.write(5, 0, &pattern(600, 3), ARCHIVE_MAX_GROUP));
        assert_eq!(store.data_length(), before);
        assert_eq!(
            store.read(5, 0, ARCHIVE_MAX_GROUP).unwrap(),
            pattern(600, 3)
        );
        // Longer: both old sectors are reused and the rest goes to the end.
        assert!(store.write(5, 0, &pattern(1500, 4), ARCHIVE_MAX_GROUP));
        assert_eq!(
            store.read(5, 0, ARCHIVE_MAX_GROUP).unwrap(),
            pattern(1500, 4)
        );
        assert_eq!(
            store.read(5, 1, ARCHIVE_MAX_GROUP).unwrap(),
            pattern(100, 2)
        );
        let (data, _) = store.memory_bytes().unwrap();
        // Sector 1 -> 2 (old chain) -> 4 (appended after group 1 in sector 3).
        assert_eq!(&data[520 + 4..520 + 7], &[0, 0, 2]);
        assert_eq!(&data[1040 + 4..1040 + 7], &[0, 0, 4]);
    }

    #[test]
    fn a_chain_owned_by_another_group_is_abandoned_for_a_fresh_one() {
        let mut store = empty(&[5]);
        assert!(store.write(5, 0, &pattern(1000, 1), ARCHIVE_MAX_GROUP));
        let (mut data, indexes) = store.memory_bytes().unwrap();
        // The second sector of the chain now claims to belong to group 9.
        data[1040 + 1] = 9;
        let mut store = DiskStore::in_memory(data, indexes);
        assert_eq!(store.read(5, 0, ARCHIVE_MAX_GROUP), None);
        assert!(store.write(5, 0, &pattern(1000, 7), ARCHIVE_MAX_GROUP));
        assert_eq!(
            store.read(5, 0, ARCHIVE_MAX_GROUP).unwrap(),
            pattern(1000, 7)
        );
        let (_, indexes) = store.memory_bytes().unwrap();
        // The first attempt wrote sector 1 before it met the foreign header,
        // so the rewrite starts at the end of the file: sector 3.
        assert_eq!(indexes[&5], vec![0, 3, 232, 0, 0, 3]);
    }

    #[test]
    fn an_entry_pointing_at_the_end_of_the_file_reports_success_without_data() {
        // Transcription of the decompiled control flow, not a desirable rule.
        let mut index = vec![0u8; 6];
        index[5] = 1; // sector 1 of a data file that is exactly one sector long
        let mut store = DiskStore::in_memory(vec![0; 520], BTreeMap::from([(2, index)]));
        assert!(store.write(2, 0, &pattern(10, 0), ARCHIVE_MAX_GROUP));
        assert_eq!(store.data_length(), 520);
        assert_eq!(store.read(2, 0, ARCHIVE_MAX_GROUP), None);
    }

    #[test]
    fn limits_are_enforced_without_writing() {
        let mut store = empty(&[255]);
        assert!(!store.write(255, 0, &vec![0; MASTER_MAX_GROUP + 1], MASTER_MAX_GROUP));
        assert_eq!(store.data_length(), 0);
        // The index file holds 174762 entries; the next one is refused.
        assert!(!store.write(255, 174_763, &[1], MASTER_MAX_GROUP));
        assert!(store.write(255, 174_761, &[1], MASTER_MAX_GROUP));
        // No index file for the archive: nothing can be stored.
        assert!(!store.write(4, 0, &[1], ARCHIVE_MAX_GROUP));
        // A stored length above the reader's limit is not returned.
        assert_eq!(store.read(255, 174_761, 0), None);
    }

    #[test]
    fn an_in_memory_store_reports_the_pieces_its_writes_touched() {
        let mut store = empty(&[1, 2]);
        assert_eq!(store.take_dirty(), (vec![], vec![]));
        assert!(store.write(1, 0, &pattern(700, 1), ARCHIVE_MAX_GROUP));
        assert_eq!(store.take_dirty(), (vec![0], vec![1]));
        assert_eq!(store.take_dirty(), (vec![], vec![]));
        // A group that runs across the first megabyte touches two pieces.
        let long = pattern(MIRROR_CHUNK + 5000, 2);
        assert!(store.write(2, 3, &long, 2 * MIRROR_CHUNK));
        assert_eq!(store.take_dirty(), (vec![0, 1], vec![2]));
        // The pieces put back together are the data file.
        let (data, indexes) = store.memory_bytes().unwrap();
        let mut joined = Vec::new();
        let mut chunk = 0;
        while let Some(piece) = store.data_chunk(chunk) {
            joined.extend_from_slice(piece);
            chunk += 1;
        }
        assert_eq!(joined, data);
        assert_eq!(chunk, 2);
        assert_eq!(store.data_chunk(2), None);
        assert_eq!(store.index_bytes(2), Some(&indexes[&2][..]));
        assert_eq!(store.index_bytes(9), None);
        // Reading leaves nothing dirty; an overwrite marks only its piece.
        assert!(store.read(2, 3, 2 * MIRROR_CHUNK).is_some());
        assert!(store.write(1, 0, &pattern(10, 9), ARCHIVE_MAX_GROUP));
        assert_eq!(store.take_dirty(), (vec![0], vec![1]));
    }

    #[test]
    fn a_live_view_reads_groups_as_they_are_written() {
        let store = Arc::new(Mutex::new(empty(&[4])));
        let view = Store::live(Arc::clone(&store), Limits::default());
        assert_eq!(view.read(4, 2).unwrap(), None);
        assert!(
            store
                .lock()
                .unwrap()
                .write(4, 2, &pattern(900, 5), ARCHIVE_MAX_GROUP)
        );
        assert_eq!(view.read(4, 2).unwrap(), Some(pattern(900, 5)));
        assert_eq!(view.read(4, 70_000).unwrap(), None);
        assert_eq!(view.read(5, 0).unwrap(), None);
        assert_eq!(view.archives().collect::<Vec<_>>(), [4]);
        assert_eq!(view.group_slots(4), 3);
    }

    #[test]
    fn a_cache_handle_is_identified_by_store_and_archive() {
        let store = Arc::new(Mutex::new(empty(&[1, 2])));
        let one = Cache::new(Arc::clone(&store), 1, ARCHIVE_MAX_GROUP);
        let two = Cache::new(Arc::clone(&store), 2, ARCHIVE_MAX_GROUP);
        let other = Cache::new(Arc::new(Mutex::new(empty(&[1]))), 1, ARCHIVE_MAX_GROUP);
        assert!(one.same(&one.clone()));
        assert!(!one.same(&two));
        assert!(!one.same(&other));
        assert!(one.write(4, &[1, 2, 3]));
        assert_eq!(one.read(4), Some(vec![1, 2, 3]));
        assert_eq!(two.read(4), None);
    }
}
