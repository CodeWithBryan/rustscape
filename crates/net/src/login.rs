//! Shared native login transport. World packets and the retry/UI state machine remain separate.
use crate::{Chunk, Reader, Writer};
use bytes::{Bytes, BytesMut};
use rs09_protocol::{
    login::{self, LoginRequest, RsaKey},
    profile::Profile,
};
use std::{io, net::SocketAddr};

pub struct LoginPeer {
    reader: Reader,
    writer: Writer,
    pending: BytesMut,
    pub server_key: i64,
    submitted: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorldLogin {
    pub staff_level: u8,
    pub blackmarks: u8,
    pub underage: bool,
    pub parental_chat_consent: bool,
    pub parental_ad_consent: bool,
    pub quick_chat: bool,
    pub mouse_recording: bool,
    pub player_index: u16,
    pub player_member: bool,
    pub map_members: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoginReply {
    pub code: u8,
    pub world: Option<WorldLogin>,
    /// The client reads a following byte for codes 21 and 29.
    pub detail: Option<u8>,
}
impl LoginPeer {
    pub async fn connect(
        address: SocketAddr,
        websocket: bool,
        username: &[u8],
    ) -> io::Result<Self> {
        let (reader, writer) = crate::connect(address, websocket, 64 * 1024).await?;
        let mut peer = Self {
            reader,
            writer,
            pending: BytesMut::new(),
            server_key: 0,
            submitted: false,
        };
        peer.writer
            .write(Bytes::copy_from_slice(&login::handshake(username)))
            .await?;
        if peer.take(1).await?[0] != 0 {
            return Err(io::Error::other("login handshake rejected"));
        }
        peer.server_key = i64::from_be_bytes(peer.take(8).await?.as_ref().try_into().unwrap());
        Ok(peer)
    }
    async fn take(&mut self, count: usize) -> io::Result<Bytes> {
        while self.pending.len() < count {
            match self.reader.read().await? {
                Chunk::Binary(bytes) => {
                    if self.pending.len().saturating_add(bytes.len()) > 64 * 1024 {
                        return Err(io::Error::other("login peer ingress exceeds limit"));
                    }
                    self.pending.extend_from_slice(&bytes);
                }
                Chunk::Text => return Err(io::Error::other("binary login reply required")),
                Chunk::Closed => return Err(io::ErrorKind::UnexpectedEof.into()),
            }
        }
        Ok(self.pending.split_to(count).freeze())
    }
    pub async fn submit(
        &mut self,
        mut request: LoginRequest<'_>,
        profile: Profile,
        public_key: &RsaKey,
    ) -> io::Result<LoginReply> {
        if self.submitted {
            return Err(io::Error::other("login already submitted"));
        }
        self.submitted = true;
        request.isaac_seed[2] = (self.server_key >> 32) as i32;
        request.isaac_seed[3] = self.server_key as i32;
        let packet = request
            .encode(profile, public_key)
            .map_err(io::Error::other)?;
        self.writer.write(packet.into()).await?;
        let code = self.take(1).await?[0];
        let world = if code == 2 {
            let fields = self.take(11).await?;
            Some(WorldLogin {
                staff_level: fields[0],
                blackmarks: fields[1],
                underage: fields[2] == 1,
                parental_chat_consent: fields[3] == 1,
                parental_ad_consent: fields[4] == 1,
                quick_chat: fields[5] == 1,
                mouse_recording: fields[6] == 1,
                player_index: u16::from_be_bytes([fields[7], fields[8]]),
                player_member: fields[9] == 1,
                map_members: fields[10] == 1,
            })
        } else {
            None
        };
        let detail = if code == 21 || code == 29 {
            Some(self.take(1).await?[0])
        } else {
            None
        };
        Ok(LoginReply {
            code,
            world,
            detail,
        })
    }
    /// Retains any coalesced game bytes, including the first rebuild-packet header.
    pub fn into_transport(self) -> (Reader, Writer, Bytes) {
        (self.reader, self.writer, self.pending.freeze())
    }
}
