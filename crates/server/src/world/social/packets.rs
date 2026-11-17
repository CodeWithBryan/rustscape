//! `PacketRepository.send`, `PacketWriteQueue` and the outgoing social
//! packets: `ContactPackets`, `CommunicationMessage`, `UpdateClanChat` and the
//! two `MSPacketRepository` packets this slice sends.
//!
//! The three game packets are queued when they are sent and built when the
//! queue is flushed at the end of the tick, as the original's
//! `PacketWriteQueue` does: every field they read (contact ranks, the ignore
//! list, the player's clan, the clan's members and ranks, `pm_index`) is read
//! at the flush, not when the packet was sent.
use super::names::{cp1252_bytes, encrypt_player_chat, string_to_long};
use super::{ClanId, Error, Host, Level, Result, Social, Value, rank_value};
use crate::player::communication::ClanRank;
use rs09_game_core::random::JavaRandom;
use std::hash::Hash;

/// `PacketHeader`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Header {
    Normal,
    Byte,
    Short,
}
impl Header {
    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Byte => "BYTE",
            Self::Short => "SHORT",
        }
    }
}

/// A finished `IoBuffer` as a session receives it: the opcode after
/// `cypherOpcode` (an int that may leave the byte range), the header kind and
/// the payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Written {
    pub opcode: i32,
    pub header: Header,
    pub payload: Vec<u8>,
}

/// `ContactContext.getType()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContactKind {
    UpdateState = 0,
    UpdateFriend = 1,
    IgnoreList = 2,
}

/// `MessageContext.SEND_MESSAGE`, `RECIEVE_MESSAGE` and `CLAN_MESSAGE`.
pub const SEND_MESSAGE: i32 = 71;
pub const RECIEVE_MESSAGE: i32 = 0;
pub const CLAN_MESSAGE: i32 = 54;

/// A packet and its context, as `PacketWriteQueue` keeps them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Queued<P> {
    /// `ContactPackets` with a `ContactContext`.
    Contact {
        player: P,
        kind: ContactKind,
        name: Option<String>,
        world_id: i32,
    },
    /// `CommunicationMessage` with a `MessageContext`.
    Message {
        player: P,
        opcode: i32,
        other: String,
        chat_icon: i32,
        message: String,
    },
    /// `UpdateClanChat` with a `ClanContext`.
    Clan {
        player: P,
        clan: ClanId,
        leave: bool,
    },
}
impl<P> Queued<P> {
    pub fn player(&self) -> &P {
        match self {
            Self::Contact { player, .. }
            | Self::Message { player, .. }
            | Self::Clan { player, .. } => player,
        }
    }
    /// The simple name of the packet class.
    pub fn class_name(&self) -> &'static str {
        match self {
            Self::Contact { .. } => "ContactPackets",
            Self::Message { .. } => "CommunicationMessage",
            Self::Clan { .. } => "UpdateClanChat",
        }
    }
}

/// The part of `IoBuffer` the social packets use, over its 2048-byte buffer.
struct Buffer {
    opcode: i32,
    header: Header,
    data: Vec<u8>,
}
impl Buffer {
    const CAPACITY: usize = 2048;
    fn new(opcode: i32, header: Header) -> Self {
        Self {
            opcode,
            header,
            data: Vec::new(),
        }
    }
    fn bytes(&mut self, bytes: &[u8]) -> Result<()> {
        if self.data.len() + bytes.len() > Self::CAPACITY {
            return Err(Error::BufferOverflow);
        }
        self.data.extend_from_slice(bytes);
        Ok(())
    }
    /// `put(int)`.
    fn put(&mut self, value: i32) -> Result<()> {
        self.bytes(&[value as u8])
    }
    /// `putShort`.
    fn put_short(&mut self, value: i32) -> Result<()> {
        self.bytes(&(value as i16).to_be_bytes())
    }
    /// `putTri`.
    fn put_tri(&mut self, value: i32) -> Result<()> {
        self.bytes(&[(value >> 16) as u8, (value >> 8) as u8, value as u8])
    }
    /// `putLong`.
    fn put_long(&mut self, value: i64) -> Result<()> {
        self.bytes(&value.to_be_bytes())
    }
    /// `putString`: `getBytes()` in the default charset (UTF-8 here), then 0.
    fn put_string(&mut self, text: &str) -> Result<()> {
        self.bytes(text.as_bytes())?;
        self.put(0)
    }
    /// `putBytes(datas, offset, len)`: the bytes from `offset` up to the index `len`.
    fn put_bytes(&mut self, data: &[u8; 256], offset: i32, end: i32) -> Result<()> {
        let mut i = offset;
        while i < end {
            let byte = usize::try_from(i).ok().and_then(|i| data.get(i)).ok_or(
                Error::ArrayIndexOutOfBounds {
                    index: i64::from(i),
                    length: 256,
                },
            )?;
            self.put(i32::from(*byte))?;
            i += 1;
        }
        Ok(())
    }
    fn finish(self) -> Written {
        Written {
            opcode: self.opcode,
            header: self.header,
            payload: self.data,
        }
    }
}

impl<P: Clone + Eq + Hash> Social<P> {
    /// `PacketRepository.send(clazz, context)`: nothing for a bot or a
    /// player without a session, nothing queued for an artificial player;
    /// otherwise `PacketWriteQueue.handle` queues the three social packets.
    pub fn send_packet<H: Host<P>>(&mut self, host: &mut H, packet: Queued<P>) {
        let player = packet.player().clone();
        if host.is_ai(&player) {
            return;
        }
        if !host.has_session(&player) {
            return;
        }
        if !host.is_artificial(&player) {
            self.writes.push_back(packet);
        }
    }

    /// `PacketWriteQueue.flush()`: the queued packets are built and written in
    /// order. A packet that fails is logged with every failure of this flush
    /// so far (the original's one StringWriter) and the next one is written.
    /// A failing log ends the flush; the packets after it stay in the second
    /// list and come back with the flush after the next one.
    pub fn flush<H: Host<P>>(&mut self, host: &mut H) -> Result<()> {
        if self.writes.is_empty() {
            return Ok(());
        }
        std::mem::swap(&mut self.writes, &mut self.flushing);
        let mut trace = String::new();
        while let Some(packet) = self.flushing.pop_front() {
            if let Err(error) = self.write_packet(host, &packet) {
                trace.push_str(&error.java_string());
                trace.push('\n');
                let message = format!(
                    "Error flushing packet class core.net.packet.out.{}: {trace}",
                    packet.class_name()
                );
                host.log("Companion", Level::Err, &message)?;
            }
        }
        Ok(())
    }

    fn write_packet<H: Host<P>>(&mut self, host: &mut H, packet: &Queued<P>) -> Result<()> {
        let buffer = match packet {
            Queued::Contact {
                player,
                kind,
                name,
                world_id,
            } => Some(contact_packet(
                host,
                player,
                *kind,
                name.as_deref(),
                *world_id,
            )?),
            Queued::Message {
                player,
                opcode,
                other,
                chat_icon,
                message,
            } => {
                let Some(buffer) =
                    self.message_packet(host, player, *opcode, other, *chat_icon, message)?
                else {
                    return Ok(());
                };
                if host.is_artificial(player) {
                    if !host.is_ai(player) {
                        return Err(Error::ClassCast);
                    }
                    host.ai_incoming_chat(player, *opcode, other, message)?;
                }
                Some(buffer)
            }
            Queued::Clan { clan, leave, .. } => Some(self.clan_packet(host, *clan, *leave)?),
        };
        if let Some(mut buffer) = buffer {
            let player = packet.player();
            let value = host.cipher(player)?;
            buffer.opcode = buffer.opcode.wrapping_add(i32::from(value as i8));
            host.write(player, buffer.finish())?;
        }
        Ok(())
    }

    /// `CommunicationMessage.send`, up to the cipher. `None` where a clan
    /// message finds no clan and returns before writing.
    fn message_packet<H: Host<P>>(
        &mut self,
        host: &mut H,
        player: &P,
        opcode: i32,
        other: &str,
        chat_icon: i32,
        message: &str,
    ) -> Result<Option<Buffer>> {
        let mut buffer = Buffer::new(opcode, Header::Byte);
        let encoded = cp1252_bytes(message);
        let length = encoded.len() as i32;
        let mut bytes = [0u8; 256];
        match opcode {
            SEND_MESSAGE => {
                let written = encrypt_player_chat(&mut bytes, 0, 0, length, &encoded);
                buffer.put_long(string_to_long(other))?;
                buffer.put(length)?;
                buffer.put_bytes(&bytes, 0, written)?;
            }
            RECIEVE_MESSAGE => {
                bytes[0] = length as u8;
                let written = 1 + encrypt_player_chat(&mut bytes, 0, 1, length, &encoded);
                host.set_attribute(player, "replyTo", Value::Text(other.to_owned()))?;
                buffer.put_long(string_to_long(other))?;
                let random = JavaRandom::new(host.random_seed())
                    .next_bound(0xFFFF)
                    .expect("positive bound");
                buffer.put_short(random)?;
                let index = message_index(host, player)?;
                buffer.put_tri(index)?;
                buffer.put(chat_icon)?;
                buffer.put_bytes(&bytes, 0, written)?;
            }
            CLAN_MESSAGE => {
                let Some(clan) = self.clan_of(player) else {
                    return Ok(None);
                };
                bytes[0] = length as u8;
                let written = 1 + encrypt_player_chat(&mut bytes, 0, 1, length, &encoded);
                buffer.put_long(string_to_long(other))?;
                buffer.put(0)?;
                let name = self.clans[clan.0].name.clone();
                buffer.put_long(string_to_long(&name))?;
                let random = JavaRandom::new(host.random_seed())
                    .next_bound(0xFFFF)
                    .expect("positive bound");
                buffer.put_short(random)?;
                let index = message_index(host, player)?;
                buffer.put_tri(index)?;
                buffer.put(chat_icon)?;
                buffer.put_bytes(&bytes, 0, written)?;
            }
            _ => {}
        }
        Ok(Some(buffer))
    }

    /// `UpdateClanChat.send`, up to the cipher: the owner, name, kick
    /// requirement and at most 100 members with their worlds and ranks.
    fn clan_packet<H: Host<P>>(
        &mut self,
        host: &mut H,
        clan: ClanId,
        leave: bool,
    ) -> Result<Buffer> {
        let mut buffer = Buffer::new(55, Header::Short);
        if !leave {
            let owner = self.clans[clan.0].owner.clone();
            let name = self.clans[clan.0].name.clone();
            buffer.put_long(string_to_long(&owner))?;
            buffer.put_long(string_to_long(&name))?;
            buffer.put(rank_value(self.clans[clan.0].kick_requirement)?)?;
            let size = self.clans[clan.0].players.len().min(100);
            buffer.put(size as i32)?;
            for i in 0..size {
                let (name, world) = {
                    let entry = &self.clans[clan.0].players[i];
                    (entry.name.clone(), entry.world_id)
                };
                buffer.put_long(string_to_long(&name))?;
                buffer.put_short(world)?;
                let rank = self.rank_of_entry(host, clan, i)?;
                buffer.put(rank.value())?;
                buffer.put_string(&format!("World {world}"))?;
            }
        } else {
            buffer.put_long(0)?;
        }
        Ok(buffer)
    }

    /// `MSPacketRepository.sendContactUpdate`: the update for the management
    /// server; a world without its session fails with a NullPointerException.
    pub(crate) fn send_contact_update<H: Host<P>>(
        &mut self,
        host: &mut H,
        username: &str,
        contact: &str,
        remove: bool,
        block: bool,
        rank: Option<ClanRank>,
    ) -> Result<()> {
        let mut buffer = Buffer::new(if block { 5 } else { 4 }, Header::Byte);
        buffer.put_string(username)?;
        buffer.put_string(contact)?;
        match rank {
            Some(rank) => {
                buffer.put(2)?;
                buffer.put(rank.ordinal())?;
            }
            None => buffer.put(i32::from(remove))?,
        }
        if !host.ms_session() {
            return Err(Error::NullPointer);
        }
        host.ms_write(buffer.finish())
    }

    /// `MSPacketRepository.sendChatSetting`: written to the management
    /// server when it has a session, otherwise a message to the player.
    pub(crate) fn send_chat_setting<H: Host<P>>(
        &mut self,
        host: &mut H,
        player: &P,
        public: i32,
        private: i32,
        trade: i32,
    ) -> Result<()> {
        let mut buffer = Buffer::new(13, Header::Byte);
        buffer.put_string(&host.name(player))?;
        buffer.put(public)?;
        buffer.put(private)?;
        buffer.put(trade)?;
        if host.ms_session() {
            host.ms_write(buffer.finish())
        } else {
            host.player_message(
                player,
                "Privacy settings unavailable at the moment. Please try again later.",
            )
        }
    }
}

/// `ContactPackets.send`, up to the cipher.
fn contact_packet<P, H: Host<P>>(
    host: &mut H,
    player: &P,
    kind: ContactKind,
    name: Option<&str>,
    world_id: i32,
) -> Result<Buffer> {
    Ok(match kind {
        ContactKind::UpdateState => {
            let mut buffer = Buffer::new(197, Header::Normal);
            buffer.put(2)?;
            buffer
        }
        ContactKind::IgnoreList => {
            let mut buffer = Buffer::new(126, Header::Short);
            let blocked =
                host.with_details(player, |details| details.communication.blocked.clone());
            for entry in blocked {
                let entry = entry.ok_or(Error::NullPointer)?;
                if entry.is_empty() {
                    continue;
                }
                buffer.put_long(string_to_long(&entry))?;
            }
            buffer
        }
        ContactKind::UpdateFriend => {
            let mut buffer = Buffer::new(62, Header::Byte);
            let name = name.ok_or(Error::NullPointer)?;
            buffer.put_long(string_to_long(name))?;
            buffer.put_short(world_id)?;
            let rank = host.with_details(player, |details| {
                details.communication.contacts.get(name).map(|c| c.rank)
            });
            match rank {
                Some(rank) => buffer.put(rank_value(rank)?)?,
                None => buffer.put(0)?,
            }
            if world_id > 0 {
                buffer.put_string(&format!("World {world_id}"))?;
            }
            buffer
        }
    })
}

/// `CommunicationMessage.getMessageIndex`: `pm_index` plus one, stored back.
fn message_index<P, H: Host<P>>(host: &mut H, player: &P) -> Result<i32> {
    let current = match host.get_attribute(player, "pm_index")? {
        None => 0,
        Some(Value::Int(value)) => value,
        Some(_) => return Err(Error::ClassCast),
    };
    let count = current.wrapping_add(1);
    host.set_attribute(player, "pm_index", Value::Int(count))?;
    Ok(count)
}
