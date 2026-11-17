//! Revision 530 JS5 container decoding. Asset parsing lives above this layer.
#![forbid(unsafe_code)]

use rs09_protocol::{Error, Result, buffer::Reader};
use std::io::{Cursor, Read};
pub mod archive;
pub mod disk;
pub mod group;
pub mod index;
pub mod store;

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_container: usize,
    pub max_uncompressed: usize,
    pub max_group_id: u32,
    pub max_file_id: u32,
    pub max_files: usize,
    pub cached_groups: usize,
    pub cached_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_container: 16 * 1024 * 1024,
            max_uncompressed: 64 * 1024 * 1024,
            max_group_id: 1_000_000,
            max_file_id: 1_000_000,
            max_files: 1_000_000,
            cached_groups: 1024,
            cached_bytes: 256 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug)]
pub enum CacheError {
    Format(Error),
    Io(std::sync::Arc<std::io::Error>),
    Missing { archive: u8, group: u32 },
    Poisoned,
}
impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Format(error) => error.fmt(f),
            Self::Io(error) => error.fmt(f),
            Self::Missing { archive, group } => {
                write!(f, "missing JS5 archive {archive}, group {group}")
            }
            Self::Poisoned => write!(f, "JS5 cache lock poisoned"),
        }
    }
}
impl std::error::Error for CacheError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Format(error) => Some(error),
            Self::Io(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}
impl From<Error> for CacheError {
    fn from(value: Error) -> Self {
        Self::Format(value)
    }
}
impl From<std::io::Error> for CacheError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(std::sync::Arc::new(value))
    }
}
pub type CacheResult<T> = std::result::Result<T, CacheError>;

/// A caller-controlled cap prevents a malformed header from exhausting memory.
pub fn decompress(container: &[u8], max_output: usize) -> Result<Vec<u8>> {
    let mut reader = Reader::new(container);
    let compression = reader.g1()?;
    let compressed_length = length(reader.g4()?, container.len())?;
    if compression == 0 {
        if compressed_length > max_output {
            return Err(Error::new(1, "JS5 output exceeds limit"));
        }
        return Ok(reader.take(compressed_length)?.to_vec());
    }
    let output_length = length(reader.g4()?, max_output)?;
    let source = reader.take(compressed_length)?;
    let limit = u64::try_from(output_length)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut output = Vec::with_capacity(output_length.min(64 * 1024));
    let decoded = match compression {
        1 => {
            // The cache omits the BZip2 header and uses 100 KiB blocks.
            let input = Cursor::new(b"BZh1").chain(Cursor::new(source));
            bzip2::read::BzDecoder::new(input)
                .take(limit)
                .read_to_end(&mut output)
        }
        2 => {
            // RT4 uses a fixed ten-byte GZip header and raw DEFLATE, ignoring
            // trailer CRC/ISIZE. Edited containers in the pinned cache rely on it.
            if source.len() < 18 || source[..2] != [31, 139] {
                return Err(Error::new(9, "invalid RT4 GZip header"));
            }
            flate2::read::DeflateDecoder::new(&source[10..source.len() - 8])
                .take(limit)
                .read_to_end(&mut output)
        }
        _ => return Err(Error::new(0, "unsupported JS5 compression")),
    };
    decoded.map_err(|_| Error::new(9, "invalid compressed JS5 data"))?;
    if output.len() != output_length {
        return Err(Error::new(5, "JS5 uncompressed length mismatch"));
    }
    Ok(output)
}

fn length(value: i32, limit: usize) -> Result<usize> {
    let value = usize::try_from(value).map_err(|_| Error::new(1, "negative JS5 length"))?;
    if value > limit {
        return Err(Error::new(1, "JS5 length exceeds limit"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_untrusted_lengths_before_allocating() {
        assert!(decompress(&[0, 127, 255, 255, 255], 1024).is_err());
        assert!(decompress(&[2, 0, 0, 0, 0, 127, 255, 255, 255], 1024).is_err());
        assert!(decompress(&[0, 255, 255, 255, 255], 1024).is_err());
        assert!(decompress(&[0, 0, 0, 0, 1], 1024).is_err());
    }
}
