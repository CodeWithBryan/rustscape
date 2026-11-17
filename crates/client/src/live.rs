//! Live connection orchestration. No authoritative game rules run in the client.
use crate::{
    session::{Config, Event, Session},
    transport::{self, Transport},
};
use rs09_protocol::game::Frame;
use std::collections::BTreeMap;
pub struct Connection {
    transport: Transport,
    session: Session,
    connected: bool,
    pub status: String,
    pub frames: BTreeMap<u8, usize>,
    pub world: Option<crate::session::WorldLogin>,
    /// The login was refused: the reply code and its detail byte.
    pub rejected: Option<(u8, Option<u8>)>,
    /// Reply 15 arrived: a reconnect login resumed the session.
    pub reconnected: bool,
    /// Set by [`Self::poll`] when the player details arrive, for the caller to
    /// take before it handles that poll's frames.
    pub entered: Option<crate::session::WorldLogin>,
    /// Frames a caller read while it waited for the login to finish.
    unread: Vec<Frame>,
}
impl Connection {
    pub fn connect(endpoint: &str, config: Config) -> Result<Self, String> {
        Ok(Self {
            transport: Transport::connect(endpoint)?,
            session: Session::new(config).map_err(|e| e.to_string())?,
            connected: false,
            status: "Connecting".into(),
            frames: BTreeMap::new(),
            world: None,
            rejected: None,
            reconnected: false,
            entered: None,
            unread: vec![],
        })
    }
    /// A game connection whose login exchange happened elsewhere: an open
    /// transport, a session already in its game stage, the player details
    /// (None after a reply 15 reconnect) and the frames read so far, which
    /// the first [`Self::poll`] returns.
    pub fn established(
        transport: Transport,
        session: Session,
        world: Option<crate::session::WorldLogin>,
        frames: Vec<Frame>,
    ) -> Self {
        Self {
            transport,
            session,
            connected: true,
            status: "Connected to world".into(),
            frames: BTreeMap::new(),
            world,
            rejected: None,
            reconnected: false,
            entered: None,
            unread: frames,
        }
    }
    pub fn in_game(&self) -> bool {
        self.session.in_game()
    }
    /// The connection has ended, whoever ended it.
    pub fn closed(&self) -> bool {
        self.session.closed()
    }
    /// Hand frames back; the next [`Self::poll`] returns them first.
    pub fn unread(&mut self, frames: Vec<Frame>) {
        self.unread.extend(frames);
    }
    pub fn stop(&mut self, reason: impl Into<String>) {
        self.status = reason.into();
        self.session.close();
        self.transport.close();
    }
    fn flush(&mut self) -> Result<(), String> {
        if self.connected {
            while let Some(bytes) = self.session.take_output() {
                self.transport.send(bytes)?;
            }
        }
        Ok(())
    }
    pub fn send(&mut self, frame: &Frame) -> Result<(), String> {
        self.session.send(frame).map_err(|e| e.to_string())?;
        self.flush()
    }
    pub fn tick(&mut self) -> Result<(), String> {
        if self.connected && self.session.tick().map_err(|e| e.to_string())?.is_some() {
            self.stop("Connection timed out");
        }
        self.flush()
    }
    /// One 20 ms cycle with the frames it wrote: see
    /// [`Session::cycle`]. Everything the cycle produced goes out as one
    /// transport write, as the original ends `Protocol.loop` with one
    /// socket write.
    pub fn cycle(&mut self, written: &[Frame], verify_id: Option<&Frame>) -> Result<(), String> {
        if !self.connected {
            return Ok(());
        }
        let timed_out = self
            .session
            .cycle(written, verify_id)
            .map_err(|e| e.to_string())?
            .is_some();
        if timed_out {
            self.stop("Connection timed out");
            return Ok(());
        }
        self.write()
    }
    /// Everything the session has ready, as one transport write.
    fn write(&mut self) -> Result<(), String> {
        let mut write = Vec::new();
        while let Some(bytes) = self.session.take_output() {
            write.extend(bytes);
        }
        if !write.is_empty() {
            self.transport.send(write)?;
        }
        Ok(())
    }
    pub fn poll(&mut self) -> Result<Vec<Frame>, String> {
        let mut frames = std::mem::take(&mut self.unread);
        for _ in 0..128 {
            let Some(event) = self.transport.poll() else {
                break;
            };
            match event {
                transport::Event::Connected => {
                    self.connected = true;
                    self.status = "Logging in".into();
                }
                transport::Event::Closed => {
                    self.stop("Disconnected");
                    break;
                }
                transport::Event::Error(error) => {
                    self.stop(error);
                    break;
                }
                transport::Event::Data(bytes) => {
                    for event in self.session.feed(&bytes).map_err(|e| e.to_string())? {
                        match event {
                            Event::Reconnected => {
                                self.reconnected = true;
                                self.status = "Connected to world".into();
                            }
                            Event::World(world) => {
                                self.entered = Some(world.clone());
                                self.world = Some(world);
                                self.status = "Connected to world".into();
                            }
                            Event::Frame(frame) => {
                                *self.frames.entry(frame.opcode).or_default() += 1;
                                frames.push(frame);
                            }
                            Event::ConfirmationRequired => {
                                self.status = "Login confirmation required".into()
                            }
                            Event::Rejected { code, detail } => {
                                self.rejected = Some((code, detail));
                                self.stop(format!(
                                    "Login reply {code}{}",
                                    detail.map_or(String::new(), |n| format!(" ({n})"))
                                ));
                            }
                            Event::Timeout => self.stop("Connection timed out"),
                        }
                    }
                }
            }
        }
        self.flush()?;
        Ok(frames)
    }
}

/// `ClientProt.ping` with the frames waiting to be written: see
/// [`Session::ping`]. What it writes is one transport write. A write that
/// fails is `LoginManager.pingFailed`: the connection is lost.
impl crate::client_events::Ping for Connection {
    fn ping(&mut self, force: bool, waiting: &[Frame]) -> bool {
        if !self.connected {
            return false;
        }
        let wrote = self
            .session
            .ping(force, waiting)
            .map_err(|e| e.to_string())
            .and_then(|wrote| self.write().map(|()| wrote));
        match wrote {
            Ok(wrote) => wrote,
            Err(error) => {
                self.stop(error);
                false
            }
        }
    }
}
