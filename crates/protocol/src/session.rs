//! Incremental ingress framing. TCP/WebSocket chunk boundaries do not define packets.
use crate::{Error, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    Handshake,
    Cache,
    Login,
    Closed,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Incoming {
    CacheHandshake { revision: u32 },
    LoginHandshake { name_hash: u8 },
    WorldList { stamp: i32 },
    CacheRequest(crate::js5::Request),
    CacheKey { key: u8, valid_padding: bool },
    CacheControl { opcode: u8, value: u32 },
    LoginPacket(Vec<u8>),
    Disconnect,
}
pub struct Decoder {
    buffer: Vec<u8>,
    position: usize,
    phase: Phase,
    max_buffer: usize,
    max_login: usize,
}
impl Decoder {
    pub fn new(max_buffer: usize, max_login: usize) -> Self {
        Self {
            buffer: Vec::new(),
            position: 0,
            phase: Phase::Handshake,
            max_buffer,
            max_login,
        }
    }
    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn buffered(&self) -> usize {
        self.buffer.len() - self.position
    }
    /// Transfers bytes coalesced after the login request to the game framer.
    pub fn into_buffered(self) -> Vec<u8> {
        let mut buffer = self.buffer;
        buffer.drain(..self.position);
        buffer
    }
    pub fn push(&mut self, bytes: &[u8]) -> Result<()> {
        if self
            .buffered()
            .checked_add(bytes.len())
            .is_none_or(|length| length > self.max_buffer)
        {
            return Err(Error::new(
                self.position,
                "connection ingress exceeds limit",
            ));
        }
        if self.position != 0 {
            self.buffer.drain(..self.position);
            self.position = 0;
        }
        self.buffer.extend_from_slice(bytes);
        Ok(())
    }
    pub fn next_packet(&mut self) -> Result<Option<Incoming>> {
        let input = &self.buffer[self.position..];
        if input.is_empty() || self.phase == Phase::Closed {
            return Ok(None);
        }
        let opcode = input[0];
        let length = match self.phase {
            Phase::Handshake => match opcode {
                14 => 2,
                15 | 255 => 5,
                _ => 1,
            },
            Phase::Cache => {
                if opcode == 5 || opcode == 9 {
                    5
                } else {
                    4
                }
            }
            Phase::Login => {
                if input.len() < 3 {
                    return Ok(None);
                }
                let length = usize::from(u16::from_be_bytes([input[1], input[2]]));
                if length > self.max_login {
                    return Err(Error::new(self.position, "login packet exceeds limit"));
                }
                length + 3
            }
            Phase::Closed => return Ok(None),
        };
        if input.len() < length {
            return Ok(None);
        }
        let packet = &input[..length];
        let command = match self.phase {
            Phase::Handshake => match opcode {
                14 => {
                    self.phase = Phase::Login;
                    Incoming::LoginHandshake {
                        name_hash: packet[1],
                    }
                }
                15 => {
                    let revision = u32::from_be_bytes(packet[1..5].try_into().unwrap());
                    self.phase = if revision == 530 {
                        Phase::Cache
                    } else {
                        Phase::Closed
                    };
                    Incoming::CacheHandshake { revision }
                }
                255 => Incoming::WorldList {
                    stamp: i32::from_be_bytes(packet[1..5].try_into().unwrap()),
                },
                _ => {
                    self.phase = Phase::Closed;
                    Incoming::Disconnect
                }
            },
            Phase::Cache => match opcode {
                0 | 1 => Incoming::CacheRequest(crate::js5::Request {
                    archive: packet[1],
                    group: u16::from_be_bytes([packet[2], packet[3]]),
                    urgent: opcode == 1,
                }),
                4 => {
                    let valid_padding = packet[2..4] == [0, 0];
                    if !valid_padding {
                        self.phase = Phase::Closed;
                    }
                    Incoming::CacheKey {
                        key: packet[1],
                        valid_padding,
                    }
                }
                7 => {
                    self.phase = Phase::Closed;
                    Incoming::Disconnect
                }
                _ => Incoming::CacheControl {
                    opcode,
                    value: packet[1..]
                        .iter()
                        .fold(0, |value, &byte| (value << 8) | u32::from(byte)),
                },
            },
            Phase::Login => Incoming::LoginPacket(packet.to_vec()),
            Phase::Closed => unreachable!(),
        };
        self.position += length;
        if self.position == self.buffer.len() {
            self.buffer.clear();
            self.position = 0;
        }
        Ok(Some(command))
    }
}

pub fn login_handshake_reply(server_key: i64) -> [u8; 9] {
    let mut output = [0; 9];
    output[1..].copy_from_slice(&server_key.to_be_bytes());
    output
}
