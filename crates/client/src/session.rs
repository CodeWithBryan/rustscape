//! Incremental RT4 login and game byte stream. Transport and rendering are external.
//!
//! Uses the shared, JVM-verified login writer and packet framer. World-list hopping,
//! and automatic port retries are not implemented here; `crate::lifecycle`
//! drives the retry and the reconnect login (type 18, reply 15) around it.
//! `Event::Reconnected` is reply 15 to a reconnect login: the session carries
//! on without the player details and without a first rebuild packet.
use rs09_protocol::{
    Error, Result,
    game::{Decoder, Direction, Frame},
    isaac::OpcodeCipher,
    login::{self, LoginRequest, RsaKey},
    profile::{Profile, RS09_REVISION_530},
};
use std::collections::VecDeque;

const LIMIT: usize = 1024 * 1024;

/// Intentionally has no Debug implementation: credentials must not enter diagnostics.
pub struct Config {
    pub username: Vec<u8>,
    pub password: Vec<u8>,
    pub random_seed: [i32; 2],
    pub crc_sums: [i32; 29],
    pub profile: Profile,
    pub width: i32,
    pub height: i32,
    /// `client.gameState == 40` when the login block is written: the block is
    /// sent as type 18 (reconnect) instead of 16, and reply 15 resumes the game.
    pub reconnect: bool,
    /// `Protocol.verifyId` as the login block carries it.
    pub verify_id: i32,
}
impl Config {
    pub fn new(username: Vec<u8>, password: Vec<u8>, random_seed: [i32; 2]) -> Self {
        Self {
            username,
            password,
            random_seed,
            crc_sums: [0; 29],
            profile: RS09_REVISION_530,
            width: 765,
            height: 503,
            reconnect: false,
            verify_id: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    ConfirmationRequired,
    Rejected { code: u8, detail: Option<u8> },
    World(WorldLogin),
    Reconnected,
    Frame(Frame),
    Timeout,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Handshake,
    Key,
    Reply,
    Confirmation,
    Detail(u8),
    Header,
    FirstFrame,
    Game,
    Closed,
}

pub struct Session {
    config: Config,
    stage: Stage,
    pending: Vec<u8>,
    outgoing: VecDeque<Vec<u8>>,
    decoder: Option<Decoder>,
    cipher: Option<OpcodeCipher>,
    world: Option<WorldLogin>,
    login_cycles: u32,
    read_cycles: u32,
    write_cycles: u32,
}
impl Session {
    pub fn new(config: Config) -> Result<Self> {
        if config.username.is_empty() || config.password.is_empty() {
            return Err(Error::new(0, "username and password required"));
        }
        let mut outgoing = VecDeque::new();
        outgoing.push_back(login::handshake(&config.username).to_vec());
        Ok(Self {
            config,
            stage: Stage::Handshake,
            pending: vec![],
            outgoing,
            decoder: None,
            cipher: None,
            world: None,
            login_cycles: 0,
            read_cycles: 0,
            write_cycles: 0,
        })
    }
    /// A session already in its game stage, for a connection whose login
    /// exchange was carried out by [`crate::login_flow::LoginFlow`]:
    /// `outbound` is the client-to-server opcode cipher past whatever the
    /// login wrote through it, `decoder` the server-to-client framer past
    /// whatever the login read. Both cycle counters start at zero, as
    /// `client.resetGameSessionState` and `LoginManager.reconnect` leave
    /// them.
    pub fn established(profile: Profile, outbound: OpcodeCipher, decoder: Decoder) -> Self {
        let mut config = Config::new(Vec::new(), Vec::new(), [0; 2]);
        config.profile = profile;
        Self {
            config,
            stage: Stage::Game,
            pending: vec![],
            outgoing: VecDeque::new(),
            decoder: Some(decoder),
            cipher: Some(outbound),
            world: None,
            login_cycles: 0,
            read_cycles: 0,
            write_cycles: 0,
        }
    }
    pub fn in_game(&self) -> bool {
        self.stage == Stage::Game
    }
    pub fn closed(&self) -> bool {
        self.stage == Stage::Closed
    }
    pub fn take_output(&mut self) -> Option<Vec<u8>> {
        self.outgoing.pop_front()
    }
    pub fn close(&mut self) {
        self.stage = Stage::Closed;
        self.pending.clear();
        self.outgoing.clear();
        self.config.password.fill(0);
        self.config.password.clear();
    }
    pub fn confirm(&mut self) -> Result<()> {
        if self.stage != Stage::Confirmation {
            return Err(Error::new(0, "login confirmation not requested"));
        }
        let opcode = self.cipher.as_mut().expect("login cipher").encode(17);
        self.outgoing.push_back(vec![opcode]);
        self.stage = Stage::Reply;
        Ok(())
    }
    pub fn send(&mut self, frame: &Frame) -> Result<()> {
        if !self.in_game() {
            return Err(Error::new(0, "world session not ready"));
        }
        let mut bytes = vec![];
        frame.write(Direction::ClientToServer, self.cipher.as_mut(), &mut bytes)?;
        let queued: usize = self.outgoing.iter().map(Vec::len).sum();
        if queued.saturating_add(bytes.len()) > LIMIT {
            self.close();
            return Err(Error::new(0, "game egress exceeds limit"));
        }
        self.outgoing.push_back(bytes);
        self.write_cycles = 0;
        Ok(())
    }
    /// One original 20 ms client cycle. The caller owns wall-clock scheduling.
    /// A timeout closes this attempt; original automatic alternate-port retry is separate.
    pub fn tick(&mut self) -> Result<Option<Event>> {
        if self.in_game() {
            self.read_cycles += 1;
            if self.read_cycles > 750 {
                self.close();
                return Ok(Some(Event::Timeout));
            }
            self.write_cycles += 1;
            if self.write_cycles > 50 {
                self.send(&Frame {
                    opcode: 93,
                    payload: vec![],
                })?;
            }
        } else if !matches!(self.stage, Stage::Confirmation | Stage::Closed) {
            self.login_cycles += 1;
            if self.login_cycles > 2000 {
                self.close();
                return Ok(Some(Event::Timeout));
            }
        }
        Ok(None)
    }
    /// One original 20 ms cycle that has packets to write: the end of
    /// `Protocol.loop` (Protocol.java:2573-2577, 2920, 2976-2988).
    ///
    /// `written` is everything the cycle wrote so far, in order. The
    /// keep-alive (opcode 93) follows it when more than 50 cycles have
    /// passed since the last write, whatever this cycle wrote itself; then
    /// `verify_id` (opcode 177), the last thing the original adds before
    /// its one socket write. The caller sends [`Self::take_output`] as that
    /// write.
    ///
    /// As in [`Self::tick`], more than 750 cycles without a server packet
    /// close the session before anything is written. Outside the game this
    /// is [`Self::tick`] and the frames are dropped.
    pub fn cycle(&mut self, written: &[Frame], verify_id: Option<&Frame>) -> Result<Option<Event>> {
        if !self.in_game() {
            return self.tick();
        }
        self.read_cycles += 1;
        if self.read_cycles > 750 {
            self.close();
            return Ok(Some(Event::Timeout));
        }
        self.write_cycles += 1;
        let keep_alive = self.write_cycles > 50;
        for frame in written {
            self.send(frame)?;
        }
        if keep_alive {
            self.send(&Frame {
                opcode: 93,
                payload: vec![],
            })?;
        }
        if let Some(frame) = verify_id {
            self.send(frame)?;
        }
        Ok(None)
    }
    /// `ClientProt.ping` (ClientProt.java:199-219), the only write of game
    /// state 25: `client.mainLoop` runs `LoginManager.rebuildMap` there and
    /// not `Protocol.loop`, so `LoginManager.ticksSinceLastPacket` does not
    /// grow and the read time-out cannot strike.
    ///
    /// The call counts as a cycle since the last write. Unless `force` is
    /// set it returns while fewer than 50 have passed. Otherwise the
    /// keep-alive (opcode 93) is added behind whatever is `waiting` in the
    /// outgoing buffer and all of it is written. Returns whether it wrote;
    /// the caller then empties its buffer and sends [`Self::take_output`]
    /// as one write.
    ///
    /// `rebuildMap` begins every call with `ping(false)`
    /// (LoginManager.java:886) and follows each of its blocking stages with
    /// `ping(true)`: four in a rebuild, seven with an underwater map.
    pub fn ping(&mut self, force: bool, waiting: &[Frame]) -> Result<bool> {
        if !self.in_game() {
            return Ok(false);
        }
        self.write_cycles += 1;
        if self.write_cycles < 50 && !force {
            return Ok(false);
        }
        for frame in waiting {
            self.send(frame)?;
        }
        self.send(&Frame {
            opcode: 93,
            payload: vec![],
        })?;
        Ok(true)
    }
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Event>> {
        let result = self.feed_inner(bytes);
        if result.is_err() {
            self.close();
        }
        result
    }
    fn feed_inner(&mut self, bytes: &[u8]) -> Result<Vec<Event>> {
        if self.closed() {
            return Err(Error::new(0, "session closed"));
        }
        if self.pending.len().saturating_add(bytes.len()) > LIMIT {
            return Err(Error::new(0, "session ingress exceeds limit"));
        }
        self.pending.extend_from_slice(bytes);
        let mut events = vec![];
        loop {
            match self.stage {
                Stage::Handshake => {
                    if self.pending.is_empty() {
                        break;
                    }
                    let code = self.pending.remove(0);
                    if code != 0 {
                        events.push(Event::Rejected { code, detail: None });
                        self.close();
                        break;
                    }
                    self.stage = Stage::Key;
                }
                Stage::Key => {
                    if self.pending.len() < 8 {
                        break;
                    }
                    let key = i64::from_be_bytes(self.pending[..8].try_into().unwrap());
                    self.pending.drain(..8);
                    let seed = [
                        self.config.random_seed[0],
                        self.config.random_seed[1],
                        (key >> 32) as i32,
                        key as i32,
                    ];
                    let request = LoginRequest {
                        username: &self.config.username,
                        password: &self.config.password,
                        isaac_seed: seed,
                        crc_sums: self.config.crc_sums,
                        window_mode: 2,
                        screen_width: self.config.width,
                        screen_height: self.config.height,
                        reconnect: self.config.reconnect,
                        packet_count: self.config.verify_id,
                        ..LoginRequest::default()
                    };
                    self.outgoing.push_back(
                        request.encode(self.config.profile, &RsaKey::reference_public())?,
                    );
                    let seed = seed.map(|n| n as u32);
                    self.cipher = Some(OpcodeCipher::new(&seed, self.config.profile.use_isaac)?);
                    self.decoder = Some(
                        Decoder::new(Direction::ServerToClient, LIMIT, 65535).with_cipher(
                            OpcodeCipher::new(
                                &seed.map(|n| n.wrapping_add(50)),
                                self.config.profile.use_isaac,
                            )?,
                        ),
                    );
                    self.config.password.fill(0);
                    self.config.password.clear();
                    self.stage = Stage::Reply;
                }
                Stage::Reply => {
                    if self.pending.is_empty() {
                        break;
                    }
                    let code = self.pending.remove(0);
                    match code {
                        1 => {
                            self.stage = Stage::Confirmation;
                            events.push(Event::ConfirmationRequired);
                        }
                        2 => self.stage = Stage::Header,
                        15 if self.config.reconnect => {
                            self.stage = Stage::Game;
                            events.push(Event::Reconnected);
                        }
                        21 | 29 => self.stage = Stage::Detail(code),
                        _ => {
                            events.push(Event::Rejected { code, detail: None });
                            self.close();
                            break;
                        }
                    }
                }
                Stage::Detail(code) => {
                    if self.pending.is_empty() {
                        break;
                    }
                    let detail = self.pending[0];
                    events.push(Event::Rejected {
                        code,
                        detail: Some(detail),
                    });
                    self.close();
                    break;
                }
                Stage::Header => {
                    // LoginManager step 8 requires eleven fields AND the first three-byte header.
                    if self.pending.len() < 14 {
                        break;
                    }
                    let f = &self.pending;
                    self.world = Some(WorldLogin {
                        staff_level: f[0],
                        blackmarks: f[1],
                        underage: f[2] == 1,
                        parental_chat_consent: f[3] == 1,
                        parental_ad_consent: f[4] == 1,
                        quick_chat: f[5] == 1,
                        mouse_recording: f[6] == 1,
                        player_index: u16::from_be_bytes([f[7], f[8]]),
                        player_member: f[9] == 1,
                        map_members: f[10] == 1,
                    });
                    self.pending.drain(..11);
                    self.stage = Stage::FirstFrame;
                }
                Stage::FirstFrame | Stage::Game => {
                    let decoder = self.decoder.as_mut().expect("game decoder");
                    decoder.push(&self.pending)?;
                    self.pending.clear();
                    while let Some(frame) = decoder.next_frame()? {
                        if self.stage == Stage::FirstFrame {
                            if frame.opcode != 162 {
                                return Err(Error::new(0, "initial normal rebuild required"));
                            }
                            events.push(Event::World(self.world.take().expect("world header")));
                            self.stage = Stage::Game;
                        }
                        self.read_cycles = 0;
                        events.push(Event::Frame(frame));
                    }
                    break;
                }
                Stage::Confirmation | Stage::Closed => break,
            }
        }
        Ok(events)
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.config.password.fill(0);
    }
}
