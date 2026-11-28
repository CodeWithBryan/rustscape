//! Reusable async native game transport. Sending a command reports its bytes;
//! gameplay outcomes must be read from subsequent authoritative world updates.
use rs09_net::{
    Chunk, Reader, Writer,
    login::{LoginPeer, WorldLogin},
};
use rs09_protocol::{
    game::{self, Direction, Frame},
    login::{LoginRequest, RsaKey},
    profile::RS09_REVISION_530,
};
use std::{io, net::SocketAddr};
mod managed;
pub use managed::{CloseReason, ManagedSession, Sender, Stats, Status};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneryAction {
    pub option: u8,
    pub id: i32,
    pub tile: [i32; 2],
}
/// The world refused the login with this reply code (the error inside the
/// `io::Error` of [`Session::connect`]; 5 is "already logged in").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoginRejected {
    pub code: u8,
}
impl std::fmt::Display for LoginRejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "world login rejected: code {}", self.code)
    }
}
impl std::error::Error for LoginRejected {}
pub struct Session {
    metadata: WorldLogin,
    reader: Reader,
    writer: Writer,
    decoder: game::Decoder,
}
impl Session {
    /// Run transport liveness independently of observation consumption or AI work.
    /// Dropping the returned owner closes the connection, including cloned senders.
    pub fn into_managed(self) -> ManagedSession {
        ManagedSession::start(self)
    }
    pub fn metadata(&self) -> &WorldLogin {
        &self.metadata
    }
    pub async fn connect(
        address: SocketAddr,
        websocket: bool,
        username: &str,
        password: &str,
    ) -> io::Result<Self> {
        let mut peer = LoginPeer::connect(address, websocket, username.as_bytes()).await?;
        let reply = peer
            .submit(
                LoginRequest {
                    username: username.as_bytes(),
                    password: password.as_bytes(),
                    ..LoginRequest::default()
                },
                RS09_REVISION_530,
                &RsaKey::reference_public(),
            )
            .await?;
        if reply.code != 2 {
            return Err(io::Error::other(LoginRejected { code: reply.code }));
        }
        let metadata = reply
            .world
            .ok_or_else(|| io::Error::other("successful login lacks world metadata"))?;
        let (reader, writer, pending) = peer.into_transport();
        let mut decoder = game::Decoder::new(Direction::ServerToClient, 1024 * 1024, 65535);
        decoder.push(&pending).map_err(io::Error::other)?;
        Ok(Self {
            metadata,
            reader,
            writer,
            decoder,
        })
    }
    /// Coalesced post-login bytes are consumed before another socket read.
    /// The caller chooses read deadlines and may retain its own world reducer.
    pub async fn next_frame(&mut self) -> io::Result<Frame> {
        loop {
            if let Some(frame) = self.decoder.next_frame().map_err(io::Error::other)? {
                return Ok(frame);
            }
            match self.reader.read().await? {
                Chunk::Binary(bytes) => self.decoder.push(&bytes).map_err(io::Error::other)?,
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "world connection closed",
                    ));
                }
            }
        }
    }
    /// Validate complete framing before touching the transport.
    pub async fn send_frame(&mut self, frame: &Frame) -> io::Result<Vec<u8>> {
        let mut wire = vec![];
        frame
            .write(Direction::ClientToServer, None, &mut wire)
            .map_err(io::Error::other)?;
        self.writer.write(wire.clone().into()).await?;
        Ok(wire)
    }
    pub async fn walk(&mut self, tile: [i32; 2], running: bool) -> io::Result<Vec<u8>> {
        self.send_frame(&game::actions::worldspace_walk(tile[0], tile[1], running))
            .await
    }
    pub async fn scenery(&mut self, action: SceneryAction) -> io::Result<Vec<u8>> {
        self.send_frame(&action.frame()?).await
    }
    /// An agent action (NPC options, use-with, dialogue) as its original packet.
    pub async fn act(&mut self, action: &crate::agent::Action) -> io::Result<Vec<u8>> {
        self.send_frame(&action.frame()?).await
    }
    pub async fn close(&mut self) -> io::Result<()> {
        self.writer.close(1000, "observations complete").await
    }
}

impl SceneryAction {
    /// Original menu options0..4. Integer fields use the original low16bits.
    pub fn frame(self) -> io::Result<Frame> {
        let action = [42, 50, 49, 46, 1001]
            .get(usize::from(self.option))
            .copied()
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "scenery option must be0..4")
            })?;
        let wire = game::actions::location_packet(action, self.id, self.tile)
            .expect("supported original scenery option");
        Ok(Frame {
            opcode: wire[0],
            payload: wire[1..].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs09_protocol::game::updates;
    use tokio::{
        net::{TcpListener, TcpStream},
        time::{Duration, timeout},
    };
    async fn fixture(pending: &[u8]) -> (Session, Reader, Writer) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (client, server) = tokio::join!(
            TcpStream::connect(listener.local_addr().unwrap()),
            listener.accept()
        );
        let (reader, writer) = rs09_net::accept(client.unwrap(), false, 65536)
            .await
            .unwrap();
        let (peer_reader, peer_writer) = rs09_net::accept(server.unwrap().0, false, 65536)
            .await
            .unwrap();
        let metadata = WorldLogin {
            staff_level: 0,
            blackmarks: 0,
            underage: false,
            parental_chat_consent: false,
            parental_ad_consent: false,
            quick_chat: false,
            mouse_recording: false,
            player_index: 7,
            player_member: false,
            map_members: false,
        };
        let mut decoder = game::Decoder::new(Direction::ServerToClient, 65536, 65535);
        decoder.push(pending).unwrap();
        (
            Session {
                metadata,
                reader,
                writer,
                decoder,
            },
            peer_reader,
            peer_writer,
        )
    }
    #[tokio::test]
    async fn pending_frames_precede_socket_reads_and_partial_frames_survive() {
        let first = updates::synth_sound(62, 1, 0);
        let second = updates::run_energy(42.0);
        let mut pending = vec![];
        first
            .write(Direction::ServerToClient, None, &mut pending)
            .unwrap();
        pending.push(second.opcode);
        let (mut session, _r, mut w) = fixture(&pending).await;
        assert_eq!(
            timeout(Duration::from_millis(100), session.next_frame())
                .await
                .unwrap()
                .unwrap(),
            first
        );
        w.write(second.payload.clone().into()).await.unwrap();
        assert_eq!(session.next_frame().await.unwrap(), second);
        drop(w);
        drop(_r);
        assert_eq!(
            session.next_frame().await.unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }
    #[tokio::test]
    async fn invalid_requests_never_publish_a_partial_frame() {
        let (mut session, mut r, _w) = fixture(&[]).await;
        assert!(
            session
                .send_frame(&Frame {
                    opcode: 254,
                    payload: vec![]
                })
                .await
                .is_err()
        );
        assert!(
            session
                .scenery(SceneryAction {
                    option: 5,
                    id: 12348,
                    tile: [3207, 3217]
                })
                .await
                .is_err()
        );
        let wire = session
            .scenery(SceneryAction {
                option: 0,
                id: 12348,
                tile: [3207, 3217],
            })
            .await
            .unwrap();
        let mut actual = vec![];
        while actual.len() < wire.len() {
            let Chunk::Binary(bytes) = timeout(Duration::from_secs(1), r.read())
                .await
                .unwrap()
                .unwrap()
            else {
                panic!("closed")
            };
            actual.extend_from_slice(&bytes);
        }
        assert_eq!(actual, wire);
    }
}
