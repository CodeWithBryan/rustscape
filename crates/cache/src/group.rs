//! JS5 stripe extraction and XTEA decryption, independent of storage and rendering.
use crate::{Limits, decompress, index::GroupInfo};
use rs09_protocol::{Error, Result, buffer::Reader, xtea};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailurePolicy {
    Strict,
    Rt4Client,
}
#[derive(Debug)]
pub struct GroupFiles {
    files: Vec<(u32, Arc<Vec<u8>>)>,
    fallback: Option<Error>,
}
impl GroupFiles {
    pub fn compatibility_fallback(&self) -> Option<&Error> {
        self.fallback.as_ref()
    }
    pub fn get(&self, id: u32) -> Option<Arc<Vec<u8>>> {
        self.files
            .binary_search_by_key(&id, |(file, _)| *file)
            .ok()
            .map(|slot| Arc::clone(&self.files[slot].1))
    }
    pub fn iter(&self) -> impl Iterator<Item = (u32, &[u8])> {
        self.files.iter().map(|(id, bytes)| (*id, bytes.as_slice()))
    }
    pub fn byte_len(&self) -> usize {
        self.files.iter().map(|(_, data)| data.len()).sum()
    }
    /// Requested payload/control storage; excludes allocator-specific bookkeeping.
    pub fn retained_bytes(&self) -> usize {
        let controls = self
            .files
            .capacity()
            .saturating_mul(std::mem::size_of::<(u32, Arc<Vec<u8>>)>())
            .saturating_add(std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>());
        self.files.iter().fold(controls, |size, (_, bytes)| {
            size.saturating_add(bytes.capacity())
                .saturating_add(std::mem::size_of::<Vec<u8>>() + 2 * std::mem::size_of::<usize>())
        })
    }
}

pub fn unpack(
    container: &[u8],
    group: &GroupInfo,
    key: [u32; 4],
    limits: Limits,
) -> Result<GroupFiles> {
    unpack_with_policy(container, group, key, limits, FailurePolicy::Strict)
}
pub fn unpack_with_policy(
    container: &[u8],
    group: &GroupInfo,
    key: [u32; 4],
    limits: Limits,
    policy: FailurePolicy,
) -> Result<GroupFiles> {
    if container.len() > limits.max_container {
        return Err(Error::new(0, "JS5 group exceeds container limit"));
    }
    let mut decrypted;
    let source = if key == [0; 4] {
        container
    } else {
        decrypted = container.to_vec();
        let length = decrypted.len();
        xtea::decrypt_js5(&mut decrypted, length, key)?;
        &decrypted
    };
    let (data, fallback) = match decompress(source, limits.max_uncompressed) {
        Ok(data) => (data, None),
        Err(error) if policy == FailurePolicy::Rt4Client => (vec![0], Some(error)),
        Err(error) => return Err(error),
    };
    let count = group.files.len();
    if count > limits.max_files {
        return Err(Error::new(0, "JS5 group exceeds file count limit"));
    }
    if count == 0 {
        return Err(Error::new(0, "JS5 group contains no files"));
    }
    if count == 1 {
        return Ok(GroupFiles {
            files: vec![(group.files[0].id, Arc::new(data))],
            fallback,
        });
    }
    let stripes = usize::from(
        *data
            .last()
            .ok_or(Error::new(0, "missing JS5 stripe count"))?,
    );
    let table_length = count
        .checked_mul(stripes)
        .and_then(|n| n.checked_mul(4))
        .ok_or(Error::new(0, "JS5 stripe table overflow"))?;
    let table_start = data
        .len()
        .checked_sub(table_length + 1)
        .ok_or(Error::new(0, "truncated JS5 stripe table"))?;
    let mut table = Reader::new(&data[table_start..data.len() - 1]);
    let mut sizes = vec![0usize; count];
    for _ in 0..stripes {
        let mut chunk = 0i64;
        for size in &mut sizes {
            chunk += i64::from(table.g4()?);
            let chunk = usize::try_from(chunk)
                .map_err(|_| Error::new(table.position(), "negative JS5 stripe size"))?;
            *size = size
                .checked_add(chunk)
                .ok_or(Error::new(table.position(), "JS5 file size overflow"))?;
            if *size > table_start {
                return Err(Error::new(table.position(), "JS5 file exceeds group data"));
            }
        }
    }
    let total = sizes
        .iter()
        .try_fold(0usize, |total, &n| total.checked_add(n))
        .ok_or(Error::new(0, "JS5 group size overflow"))?;
    if total != table_start {
        return Err(Error::new(table_start, "JS5 stripe payload size mismatch"));
    }
    let mut extracted: Vec<_> = sizes.iter().map(|&size| Vec::with_capacity(size)).collect();
    table.seek(0)?;
    let mut position = 0usize;
    for _ in 0..stripes {
        let mut length = 0i64;
        for output in &mut extracted {
            length += i64::from(table.g4()?);
            let length = usize::try_from(length)
                .map_err(|_| Error::new(table.position(), "negative JS5 stripe size"))?;
            let end = position
                .checked_add(length)
                .ok_or(Error::new(position, "JS5 stripe offset overflow"))?;
            output.extend_from_slice(
                data.get(position..end)
                    .ok_or(Error::new(position, "truncated JS5 stripe payload"))?,
            );
            position = end;
        }
    }
    Ok(GroupFiles {
        files: group
            .files
            .iter()
            .zip(extracted)
            .map(|(file, bytes)| (file.id, Arc::new(bytes)))
            .collect(),
        fallback,
    })
}
