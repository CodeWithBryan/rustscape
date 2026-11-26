//! Revision-530 game frames and typed client actions. No game rules run here.
//!
//! The pinned server reads plain inbound opcodes. The pinned profile disables
//! ISAAC; a caller may explicitly supply a cipher for client-side experiments,
//! but this does not establish server compatibility with ISAAC enabled.
use crate::{Error, Result, isaac::OpcodeCipher};
pub mod actions;
pub mod chat;
pub mod events;
pub mod ground;
pub mod menu;
pub mod npcs;
pub mod outgoing;
pub mod players;
pub mod rebuild;
mod sizes;
pub mod updates;
pub use actions::ClientAction;
pub use sizes::{CLIENT_PACKET_SIZES, SERVER_PACKET_SIZES};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    ClientToServer,
    ServerToClient,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Size {
    Fixed(usize),
    Byte,
    Short,
}
impl Direction {
    pub fn size(self, opcode: u8) -> Option<Size> {
        let value = match self {
            Self::ClientToServer => CLIENT_PACKET_SIZES[usize::from(opcode)],
            Self::ServerToClient => SERVER_PACKET_SIZES[usize::from(opcode)],
        };
        match value {
            -1 => Some(Size::Byte),
            -2 => Some(Size::Short),
            0.. => Some(Size::Fixed(value as usize)),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    pub opcode: u8,
    pub payload: Vec<u8>,
}
impl Frame {
    /// Validates before consuming a cipher word or modifying the output.
    pub fn write(
        &self,
        direction: Direction,
        cipher: Option<&mut OpcodeCipher>,
        output: &mut Vec<u8>,
    ) -> Result<()> {
        let size = direction
            .size(self.opcode)
            .ok_or(Error::new(0, "unknown game opcode size"))?;
        let valid = match size {
            Size::Fixed(n) => self.payload.len() == n,
            Size::Byte => self.payload.len() <= 255,
            Size::Short => self.payload.len() <= 65535,
        };
        if !valid {
            return Err(Error::new(0, "game payload length does not fit header"));
        }
        output.push(cipher.map_or(self.opcode, |cipher| cipher.encode(self.opcode)));
        match size {
            Size::Fixed(_) => {}
            Size::Byte => output.push(self.payload.len() as u8),
            Size::Short => output.extend_from_slice(&(self.payload.len() as u16).to_be_bytes()),
        }
        output.extend_from_slice(&self.payload);
        Ok(())
    }
}

/// Bounded stream framing, independent of TCP/WebSocket read boundaries.
/// Opcode cipher state advances once per opcode, including fragmented headers.
pub struct Decoder {
    direction: Direction,
    buffer: Vec<u8>,
    position: usize,
    max_buffer: usize,
    max_payload: usize,
    pending: Option<(u8, Size)>,
    cipher: Option<OpcodeCipher>,
    failed: bool,
}
impl Decoder {
    pub fn new(direction: Direction, max_buffer: usize, max_payload: usize) -> Self {
        Self {
            direction,
            buffer: Vec::new(),
            position: 0,
            max_buffer,
            max_payload,
            pending: None,
            cipher: None,
            failed: false,
        }
    }
    pub fn with_cipher(mut self, cipher: OpcodeCipher) -> Self {
        self.cipher = Some(cipher);
        self
    }
    pub fn buffered(&self) -> usize {
        self.buffer.len() - self.position
    }
    pub fn push(&mut self, bytes: &[u8]) -> Result<()> {
        if self.failed {
            return Err(Error::new(self.position, "game decoder failed"));
        }
        if self
            .buffered()
            .checked_add(bytes.len())
            .is_none_or(|n| n > self.max_buffer)
        {
            return Err(Error::new(self.position, "game ingress exceeds limit"));
        }
        if self.position > 0 {
            self.buffer.drain(..self.position);
            self.position = 0;
        }
        self.buffer.extend_from_slice(bytes);
        Ok(())
    }
    pub fn next_frame(&mut self) -> Result<Option<Frame>> {
        if self.failed {
            return Err(Error::new(self.position, "game decoder failed"));
        }
        if self.pending.is_none() {
            let Some(&wire) = self.buffer.get(self.position) else {
                return Ok(None);
            };
            let opcode = self.cipher.as_mut().map_or(wire, |c| c.decode(wire));
            self.position += 1;
            let Some(size) = self.direction.size(opcode) else {
                self.failed = true;
                return Err(Error::new(self.position - 1, "unknown game opcode size"));
            };
            self.pending = Some((opcode, size));
        }
        let (opcode, mut size) = self.pending.unwrap();
        let prefix = match size {
            Size::Byte => 1,
            Size::Short => 2,
            Size::Fixed(_) => 0,
        };
        if self.buffered() < prefix {
            return Ok(None);
        }
        if prefix > 0 {
            let n = if prefix == 1 {
                usize::from(self.buffer[self.position])
            } else {
                usize::from(u16::from_be_bytes([
                    self.buffer[self.position],
                    self.buffer[self.position + 1],
                ]))
            };
            self.position += prefix;
            size = Size::Fixed(n);
            self.pending = Some((opcode, size));
        }
        let Size::Fixed(n) = size else { unreachable!() };
        if n > self.max_payload {
            self.failed = true;
            return Err(Error::new(self.position, "game payload exceeds limit"));
        }
        if self.buffered() < n {
            return Ok(None);
        }
        let payload = self.buffer[self.position..self.position + n].to_vec();
        self.position += n;
        self.pending = None;
        Ok(Some(Frame { opcode, payload }))
    }
}
