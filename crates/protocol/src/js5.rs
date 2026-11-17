//! Revision 530 cache wire framing, including the pinned server's formatter quirks.
use crate::{Error, Result, buffer::Writer};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Request {
    pub archive: u8,
    pub group: u16,
    pub urgent: bool,
}
impl Request {
    pub fn encode(self) -> [u8; 4] {
        let group = self.group.to_be_bytes();
        [u8::from(self.urgent), self.archive, group[0], group[1]]
    }
}

pub fn handshake(revision: u32) -> [u8; 5] {
    let revision = revision.to_be_bytes();
    [15, revision[0], revision[1], revision[2], revision[3]]
}

/// Mirrors core.cache.Cache.getArchiveData on a single packed container.
pub fn encode_archive(
    request: Request,
    key: u8,
    packed: &[u8],
    max_payload: usize,
) -> Result<Vec<u8>> {
    let header = packed
        .get(..5)
        .ok_or(Error::new(0, "truncated JS5 container header"))?;
    let length = i32::from_be_bytes(header[1..].try_into().unwrap());
    let length = usize::try_from(length).map_err(|_| Error::new(1, "negative JS5 wire length"))?;
    if length > max_payload {
        return Err(Error::new(1, "JS5 wire payload exceeds limit"));
    }
    let compression = header[0];
    let mut transmitted = length
        .checked_add(if compression == 0 { 0 } else { 4 })
        .ok_or(Error::new(1, "JS5 wire length overflow"))?;
    // Preserve the server's workaround for compressed containers lacking a version.
    if request.archive != 255 && compression != 0 && packed.len().checked_sub(length) == Some(9) {
        transmitted += 2;
    }
    let capacity = transmitted
        .checked_add(transmitted.div_ceil(511))
        .and_then(|n| n.checked_add(8))
        .ok_or(Error::new(1, "JS5 reply size overflow"))?;
    let mut output = Vec::with_capacity(capacity);
    output.push(request.archive);
    output.extend(request.group.to_be_bytes());
    output.push(compression | if request.urgent { 0 } else { 128 });
    output.extend((length as u32).to_be_bytes());
    for i in 0..transmitted {
        if output.len() % 512 == 0 {
            output.push(255);
        }
        output.push(packed.get(i + 5).copied().unwrap_or(0));
    }
    if key != 0 {
        for byte in &mut output {
            *byte ^= key;
        }
    }
    Ok(output)
}

/// Missing slot 24 has a special checksum in Cache.generateReferenceData.
pub fn master_payload(entries: &[Option<(u32, i32)>]) -> Vec<u8> {
    let mut output = Writer::new();
    for (archive, entry) in entries.iter().enumerate() {
        let (checksum, version) = entry.unwrap_or((if archive == 24 { 609698396 } else { 0 }, 0));
        output.p4(checksum as i32);
        output.p4(version);
    }
    output.into_vec()
}

/// JS5WriteEvent.getReferenceData ignores priority/XOR and starts its marker
/// counter at 10 even though the actual header has eight bytes.
pub fn encode_master(payload: &[u8], max_payload: usize) -> Result<Vec<u8>> {
    if payload.len() > max_payload || payload.len() > i32::MAX as usize {
        return Err(Error::new(0, "JS5 master payload exceeds limit"));
    }
    let mut output = vec![255, 0, 255, 0];
    output.extend((payload.len() as u32).to_be_bytes());
    let mut offset = 10;
    for &byte in payload {
        if offset == 512 {
            output.push(255);
            offset = 1;
        }
        output.push(byte);
        offset += 1;
    }
    Ok(output)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Response {
    pub request: Request,
    pub container: Vec<u8>,
}

/// Incremental client parser for the standard RT4 block stream. Trailer bytes
/// reserved by Js5NetRequest remain the caller's responsibility.
pub struct ResponseDecoder {
    header: [u8; 8],
    header_length: usize,
    current: Option<Response>,
    expected: usize,
    block_position: usize,
    key: u8,
    max_payload: usize,
}
impl ResponseDecoder {
    pub const fn new(key: u8, max_payload: usize) -> Self {
        Self {
            header: [0; 8],
            header_length: 0,
            current: None,
            expected: 0,
            block_position: 0,
            key,
            max_payload,
        }
    }
    pub fn set_key(&mut self, key: u8) -> Result<()> {
        if self.current.is_some() || self.header_length != 0 {
            return Err(Error::new(0, "cannot rekey a partial JS5 reply"));
        }
        self.key = key;
        Ok(())
    }
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Response>> {
        let mut responses = Vec::new();
        let mut position = 0;
        while position < bytes.len() {
            if self.current.is_none() {
                let count = (8 - self.header_length).min(bytes.len() - position);
                for i in 0..count {
                    self.header[self.header_length + i] = bytes[position + i] ^ self.key;
                }
                self.header_length += count;
                position += count;
                if self.header_length != 8 {
                    continue;
                }
                let length = i32::from_be_bytes(self.header[4..8].try_into().unwrap());
                let length = usize::try_from(length)
                    .map_err(|_| Error::new(4, "negative JS5 reply length"))?;
                if length > self.max_payload {
                    return Err(Error::new(4, "JS5 reply exceeds limit"));
                }
                let compression = self.header[3] & 127;
                self.expected = length
                    .checked_add(if compression == 0 { 5 } else { 9 })
                    .ok_or(Error::new(4, "JS5 reply length overflow"))?;
                let mut container = Vec::with_capacity(self.expected);
                container.push(compression);
                container.extend((length as u32).to_be_bytes());
                self.current = Some(Response {
                    request: Request {
                        archive: self.header[0],
                        group: u16::from_be_bytes([self.header[1], self.header[2]]),
                        urgent: self.header[3] & 128 == 0,
                    },
                    container,
                });
                self.header_length = 0;
                self.block_position = 8;
            } else if self.block_position == 0 {
                let byte = bytes[position] ^ self.key;
                position += 1;
                if byte != 255 {
                    return Err(Error::new(0, "invalid JS5 block marker"));
                }
                self.block_position = 1;
            } else {
                // Reached only once the header branch above has set it.
                #[allow(clippy::unnecessary_unwrap)]
                let output = &mut self.current.as_mut().unwrap().container;
                let count = (512 - self.block_position)
                    .min(self.expected - output.len())
                    .min(bytes.len() - position);
                let input = &bytes[position..position + count];
                if self.key == 0 {
                    output.extend_from_slice(input);
                } else {
                    output.extend(input.iter().map(|&byte| byte ^ self.key));
                }
                position += count;
                self.block_position += count;
            }
            if self
                .current
                .as_ref()
                .is_some_and(|response| response.container.len() == self.expected)
            {
                responses.push(self.current.take().unwrap());
                self.block_position = 0;
            } else if self.block_position == 512 {
                self.block_position = 0;
            }
        }
        Ok(responses)
    }
    pub fn is_idle(&self) -> bool {
        self.current.is_none() && self.header_length == 0
    }
    /// How many bytes `Js5NetQueue.loop` asks its socket for at this point:
    /// the rest of the eight-byte header, one block marker, or the rest of the
    /// current 512-byte block. Feeding exactly this much per read reproduces
    /// the original read sizes.
    pub fn wanted(&self) -> usize {
        match &self.current {
            None => 8 - self.header_length,
            Some(_) if self.block_position == 0 => 1,
            Some(response) => {
                (512 - self.block_position).min(self.expected - response.container.len())
            }
        }
    }
    /// The reply being received: its request, the container bytes held so far
    /// (the five-byte container header counts, as in `Js5NetRequest`) and the
    /// container's full length.
    pub fn partial(&self) -> Option<(Request, usize, usize)> {
        self.current
            .as_ref()
            .map(|response| (response.request, response.container.len(), self.expected))
    }
}
