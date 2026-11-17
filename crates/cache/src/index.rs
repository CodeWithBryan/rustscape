//! JS5 reference tables, protocols 5 and 6, preserving sparse IDs and name lookup.
use crate::{Limits, decompress};
use rs09_protocol::{Error, Result, buffer::Reader, checksum::crc32};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct FileInfo {
    pub id: u32,
    pub name_hash: Option<i32>,
}
#[derive(Clone, Debug)]
pub struct GroupInfo {
    pub id: u32,
    pub checksum: u32,
    pub version: i32,
    pub capacity: u32,
    pub name_hash: Option<i32>,
    pub files: Vec<FileInfo>,
    file_names: Option<NameLookup>,
}
impl GroupInfo {
    pub fn file(&self, id: u32) -> Option<&FileInfo> {
        self.files
            .binary_search_by_key(&id, |file| file.id)
            .ok()
            .map(|slot| &self.files[slot])
    }
    pub fn file_by_hash(&self, hash: i32) -> Option<u32> {
        self.file_names.as_ref()?.get(hash)
    }
}

/// IntHashTable keeps the first matching dense-array index, including -1 holes.
#[derive(Clone, Debug)]
struct NameLookup(HashMap<i32, u32>);
impl NameLookup {
    fn new(pairs: impl Iterator<Item = (u32, i32)>, capacity: u32) -> Self {
        let mut map = HashMap::new();
        let mut first_hole = 0;
        for (id, hash) in pairs {
            map.entry(hash)
                .and_modify(|value: &mut u32| *value = (*value).min(id))
                .or_insert(id);
            if first_hole == id {
                first_hole += 1;
            }
        }
        if first_hole < capacity {
            map.entry(-1)
                .and_modify(|value| *value = (*value).min(first_hole))
                .or_insert(first_hole);
        }
        Self(map)
    }
    fn get(&self, hash: i32) -> Option<u32> {
        self.0.get(&hash).copied()
    }
}

#[derive(Clone, Debug)]
pub struct Index {
    pub format: u8,
    pub version: i32,
    pub checksum: u32,
    pub capacity: u32,
    pub groups: Vec<GroupInfo>,
    group_names: Option<NameLookup>,
}
impl Index {
    pub fn decode(
        container: &[u8],
        expected_checksum: Option<u32>,
        limits: Limits,
    ) -> Result<Self> {
        if container.len() > limits.max_container {
            return Err(Error::new(0, "JS5 index container exceeds limit"));
        }
        let checksum = crc32(container);
        if expected_checksum.is_some_and(|expected| expected != checksum) {
            return Err(Error::new(0, "JS5 index CRC mismatch"));
        }
        let decoded = decompress(container, limits.max_uncompressed)?;
        let mut reader = Reader::new(&decoded);
        let format = reader.g1()? as u8;
        if format != 5 && format != 6 {
            return Err(Error::new(0, "unsupported JS5 index protocol"));
        }
        let version = if format == 6 { reader.g4()? } else { 0 };
        let names = reader.g1()? != 0;
        let count = reader.g2()? as usize;
        let mut groups = Vec::with_capacity(count);
        let mut last = 0u32;
        for i in 0..count {
            last = next_id(&mut reader, last, limits.max_group_id, i)?;
            groups.push(GroupInfo {
                id: last,
                checksum: 0,
                version: 0,
                capacity: 0,
                name_hash: None,
                files: Vec::new(),
                file_names: None,
            });
        }
        let capacity = groups.last().map_or(0, |group| group.id + 1);
        if names {
            for group in &mut groups {
                group.name_hash = Some(reader.g4()?);
            }
        }
        let group_names = names.then(|| {
            NameLookup::new(
                groups
                    .iter()
                    .map(|group| (group.id, group.name_hash.unwrap())),
                capacity,
            )
        });
        for group in &mut groups {
            group.checksum = reader.g4()? as u32;
        }
        for group in &mut groups {
            group.version = reader.g4()?;
        }
        let mut total_files = 0usize;
        for group in &mut groups {
            let count = reader.g2()? as usize;
            total_files = total_files
                .checked_add(count)
                .ok_or(Error::new(reader.position(), "JS5 file count overflow"))?;
            if total_files > limits.max_files {
                return Err(Error::new(
                    reader.position(),
                    "JS5 file count exceeds limit",
                ));
            }
            group.files = Vec::with_capacity(count);
            // IDs follow all group counts, not each individual group count.
            group.capacity = count as u32;
        }
        for group in &mut groups {
            let count = group.capacity as usize;
            let mut last = 0;
            for i in 0..count {
                last = next_id(&mut reader, last, limits.max_file_id, i)?;
                group.files.push(FileInfo {
                    id: last,
                    name_hash: None,
                });
            }
            group.capacity = group.files.last().map_or(0, |file| file.id + 1);
        }
        if names {
            for group in &mut groups {
                for file in &mut group.files {
                    file.name_hash = Some(reader.g4()?);
                }
                group.file_names = Some(NameLookup::new(
                    group
                        .files
                        .iter()
                        .map(|file| (file.id, file.name_hash.unwrap())),
                    group.capacity,
                ));
            }
        }
        Ok(Self {
            format,
            version,
            checksum,
            capacity,
            groups,
            group_names,
        })
    }
    pub fn group(&self, id: u32) -> Option<&GroupInfo> {
        self.groups
            .binary_search_by_key(&id, |group| group.id)
            .ok()
            .map(|slot| &self.groups[slot])
    }
    /// Raw name-table result. As in RT4, -1 can resolve to an unused sparse slot.
    pub fn group_by_hash(&self, hash: i32) -> Option<u32> {
        self.group_names.as_ref()?.get(hash)
    }
    pub fn group_by_ascii_name(&self, name: &str) -> Option<u32> {
        if !name.is_ascii() {
            return None;
        }
        let id = self.group_by_hash(name_hash(&name.to_ascii_lowercase().into_bytes()))?;
        self.group(id)
            .filter(|group| group.capacity != 0)
            .map(|group| group.id)
    }
}
fn next_id(reader: &mut Reader<'_>, last: u32, max: u32, ordinal: usize) -> Result<u32> {
    let delta = reader.g2()? as u32;
    let value = last
        .checked_add(delta)
        .ok_or(Error::new(reader.position(), "JS5 ID overflow"))?;
    if value > max || value == u32::MAX || (delta == 0 && ordinal != 0) {
        return Err(Error::new(reader.position(), "invalid or excessive JS5 ID"));
    }
    Ok(value)
}
/// JagString.getHash on already encoded bytes. Case folding belongs to the text layer.
pub fn name_hash(bytes: &[u8]) -> i32 {
    bytes.iter().fold(0i32, |hash, &byte| {
        hash.wrapping_mul(31).wrapping_add(i32::from(byte))
    })
}
