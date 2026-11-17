//! The RT4 title-screen connection logic: `LoginManager` (login, world hop,
//! reconnect), `CreateManager` (account creation) and the parts of `client`
//! that decide the game state around them. No rendering and no socket: the
//! connection sits behind [`Dialer`] and [`Socket`], and one call to
//! [`LoginFlow::cycle`] is one 20 ms client cycle.
//!
//! The three original state machines share `Protocol.socket`,
//! `Protocol.socketRequest` and `Protocol.outboundBuffer`; here that is
//! [`Net`]. The world list ([`crate::world_list::WorldList`]) uses it too.
//!
//! Differences from the original:
//! - The original reads the first reply byte of each request with a blocking
//!   `InputStream.read()` under a 30 second socket timeout, freezing the
//!   client meanwhile. Here that read returns to the caller until the byte
//!   arrives. While it waits the step's loop counter stands still, as it does
//!   in the blocked original, and 1500 waiting cycles (30 seconds of 20 ms
//!   cycles) count as the socket timeout.
//! - Where the original aborts a script command with an unchecked exception
//!   (a name with no base-37 form, a request that overruns its fixed
//!   128/129-byte buffer) the command returns a [`ScriptError`] and the
//!   state is left as the original leaves it at the throw. A login block the
//!   shared encoder refuses (a zero byte inside the password or settings,
//!   which an original string cannot hold) ends the attempt with reply -4.
//! - Browser calls (`zap`/`unzap`, the banner refresh) are not made, and
//!   `WorldList.hopWorld`'s browser branch, which the pinned client cannot
//!   reach, is absent.
use crate::world_list::WorldList;
use rs09_protocol::{
    buffer::Writer,
    game::{Decoder, Direction, Frame},
    isaac::OpcodeCipher,
    login::{self, LoginRequest, RsaKey},
    profile::{Profile, RS09_REVISION_530},
};
use std::io;

/// `GlobalConfig.DEFAULT_HOSTNAME`.
pub const DEFAULT_HOSTNAME: &str = "test.2009scape.org";
/// `GlobalConfig.DEFAULT_PORT`.
pub const DEFAULT_PORT: i32 = 43594;
/// `GlobalConfig.ALTERNATE_PORT`.
pub const ALTERNATE_PORT: i32 = 43593;
/// `client.LOGINSCREEN`: the title interface's group name in archive 3.
pub const TITLE_INTERFACE: &str = "loginscreen";
/// `client.TITLE_SONG`: the title song's group name in archive 6.
pub const TITLE_SONG: &str = "scape main";
/// `TitleScreen.TITLEBG` and `TitleScreen.LOGO`: sprite groups in archive 8,
/// drawn behind the loading bar in game state 5.
pub const TITLE_BACKGROUND: &str = "titlebg";
pub const TITLE_LOGO: &str = "logo";
/// Cycles a blocking read waits before it fails: the original's 30 second
/// `setSoTimeout` in 20 ms cycles.
pub const READ_TIMEOUT_CYCLES: u32 = 1500;

/// `BufferedSocket` as the login code uses it.
pub trait Socket {
    /// `available()`: bytes that can be read now. An ended stream reports 0,
    /// as `InputStream.available` does; an error is the `IOException`.
    fn available(&mut self) -> io::Result<usize>;
    /// `read(0, len, buffer)`: exactly `out.len()` bytes, an error if the
    /// stream cannot supply them.
    fn read(&mut self, out: &mut [u8]) -> io::Result<()>;
    /// The peer has closed the stream and nothing is left to read: the case
    /// where `InputStream.read()` returns -1.
    fn at_end(&mut self) -> bool;
    /// `write(bytes, len)`.
    fn write(&mut self, bytes: &[u8]) -> io::Result<()>;
    /// `close()`.
    fn close(&mut self);
}

/// `PrivilegedRequest.status` of a socket request: 0, 2 or 1 with its result.
pub enum Opened<S> {
    Pending,
    Failed,
    Ready(S),
}

/// `SignLink.openSocket`.
pub trait Dialer {
    type Socket: Socket;
    type Request;
    fn open(&mut self, host: &str, port: i32) -> Self::Request;
    fn status(&mut self, request: &mut Self::Request) -> Opened<Self::Socket>;
}

/// `Protocol.socketRequest`, `Protocol.socket`, `Protocol.previousSocket`
/// and the bytes of `Protocol.outboundBuffer` up to its offset.
pub struct Net<D: Dialer> {
    pub dialer: D,
    pub request: Option<D::Request>,
    pub socket: Option<D::Socket>,
    pub previous_socket: Option<D::Socket>,
    pub outbound: Vec<u8>,
    /// Cycles the current blocking read has waited.
    pub waited: u32,
}
impl<D: Dialer> Net<D> {
    pub fn new(dialer: D) -> Self {
        Self {
            dialer,
            request: None,
            socket: None,
            previous_socket: None,
            outbound: Vec::new(),
            waited: 0,
        }
    }
    /// `Protocol.socketRequest = signLink.openSocket(host, port)`.
    pub fn open(&mut self, host: &str, port: i32) {
        self.request = Some(self.dialer.open(host, port));
    }
    /// The request's status; a connected socket becomes `Protocol.socket`
    /// and the request is cleared.
    pub fn poll(&mut self) -> Opened<()> {
        let Some(request) = self.request.as_mut() else {
            return Opened::Failed;
        };
        match self.dialer.status(request) {
            Opened::Pending => Opened::Pending,
            Opened::Failed => Opened::Failed,
            Opened::Ready(socket) => {
                self.socket = Some(socket);
                self.request = None;
                self.waited = 0;
                Opened::Ready(())
            }
        }
    }
    /// `if (socket != null) { socket.close(); socket = null; }`.
    pub fn close(&mut self) {
        if let Some(mut socket) = self.socket.take() {
            socket.close();
        }
        self.waited = 0;
    }
    fn connected(&mut self) -> io::Result<&mut D::Socket> {
        self.socket
            .as_mut()
            .ok_or_else(|| io::ErrorKind::NotConnected.into())
    }
    pub fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.connected()?.write(bytes)
    }
    pub fn available(&mut self) -> io::Result<usize> {
        self.connected()?.available()
    }
    pub fn read(&mut self, count: usize) -> io::Result<Vec<u8>> {
        let mut bytes = vec![0; count];
        self.connected()?.read(&mut bytes)?;
        Ok(bytes)
    }
    /// The original's blocking `socket.read()`: None while the byte has not
    /// arrived, -1 at the end of the stream, an error after
    /// [`READ_TIMEOUT_CYCLES`] calls without it.
    pub fn read_blocking(&mut self) -> io::Result<Option<i32>> {
        let socket = self.connected()?;
        if socket.available()? >= 1 {
            let mut byte = [0];
            socket.read(&mut byte)?;
            self.waited = 0;
            return Ok(Some(i32::from(byte[0])));
        }
        if socket.at_end() {
            self.waited = 0;
            return Ok(Some(-1));
        }
        self.waited += 1;
        if self.waited > READ_TIMEOUT_CYCLES {
            self.waited = 0;
            return Err(io::ErrorKind::TimedOut.into());
        }
        Ok(None)
    }
}

/// `GlobalJsonConfig`: the optional `config.json` beside the client.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonConfig {
    pub ip_management: String,
    pub ip_address: String,
    pub world: i32,
    pub server_port: i32,
    pub wl_port: i32,
}

/// The connection targets of `client` and `Player.worldId`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Endpoints {
    pub hostname: String,
    pub port: i32,
    pub default_port: i32,
    pub alternate_port: i32,
    pub world_list_hostname: String,
    pub world_list_port: i32,
    pub world_list_default_port: i32,
    pub world_list_alternate_port: i32,
    /// `client.worldListId`.
    pub world_list_id: i32,
    /// `client.modeWhere`: 0 live, 1 and 2 test layouts. The standalone
    /// client always runs with 2.
    pub mode_where: i32,
    /// `Player.worldId`; -1 makes a login ask the world-list server for a
    /// world first.
    pub world_id: i32,
    pub json: Option<JsonConfig>,
}
impl Endpoints {
    /// `client.mainInit`: the ports and host for a `modeWhere` and world.
    /// `select_default_world` is `GlobalConfig.SELECT_DEFAULT_WORLD` (true
    /// in the pinned client), which starts on `world_list_id` instead of
    /// asking for a world.
    pub fn new(
        default_hostname: &str,
        mode_where: i32,
        world_list_id: i32,
        json: Option<JsonConfig>,
        select_default_world: bool,
    ) -> Self {
        let mut hostname = default_hostname.to_owned();
        let (mut alternate, mut default) = if mode_where == 0 {
            (ALTERNATE_PORT + 1, DEFAULT_PORT + 1)
        } else {
            (ALTERNATE_PORT + world_list_id, DEFAULT_PORT + world_list_id)
        };
        if let Some(json) = &json {
            hostname = json.ip_address.clone();
            alternate = json.server_port + world_list_id;
            default = json.server_port;
        }
        Self {
            hostname: hostname.clone(),
            port: default,
            default_port: default,
            alternate_port: alternate,
            world_list_hostname: hostname,
            world_list_port: default,
            world_list_default_port: default,
            world_list_alternate_port: alternate,
            world_list_id,
            mode_where,
            world_id: if select_default_world {
                world_list_id
            } else {
                -1
            },
            json,
        }
    }
    /// The standalone client's `main`: world 1 (or the JSON file's world)
    /// and `modeWhere` 2.
    pub fn standalone(default_hostname: &str, json: Option<JsonConfig>) -> Self {
        let world = json.as_ref().map_or(1, |json| json.world);
        Self::new(default_hostname, 2, world, json, true)
    }
    fn toggle_port(&mut self) {
        self.port = if self.port == self.default_port {
            self.alternate_port
        } else {
            self.default_port
        };
    }
    fn toggle_world_list_port(&mut self) {
        self.world_list_port = if self.world_list_port == self.world_list_default_port {
            self.world_list_alternate_port
        } else {
            self.world_list_default_port
        };
    }
}

/// What the login block reads from the rest of the client at the moment it
/// is built.
#[derive(Clone, Debug)]
pub struct Environment<'a> {
    /// `client.advertSuppressed`.
    pub advert_suppressed: bool,
    /// `DisplayMode.getWindowMode()`.
    pub window_mode: i32,
    /// `GameShell.canvasWidth` and `canvasHeight`.
    pub canvas_width: i32,
    pub canvas_height: i32,
    /// `Preferences.antiAliasingMode`.
    pub anti_aliasing: i32,
    /// `client.writeUid`: see [`uid_bytes`].
    pub uid: [u8; 24],
    /// `client.settings`.
    pub settings: &'a [u8],
    /// `client.affiliate`.
    pub affiliate: i32,
    /// `Preferences.toInt()`: see [`preferences_to_int`].
    pub preferences: i32,
    /// `Protocol.verifyId`.
    pub verify_id: i32,
    /// `js5Archive0..27.getChecksum()`.
    pub checksums: [i32; 28],
}
impl Default for Environment<'_> {
    /// The standalone client's values before any preference is loaded.
    fn default() -> Self {
        Self {
            advert_suppressed: false,
            window_mode: 0,
            canvas_width: 765,
            canvas_height: 503,
            anti_aliasing: 0,
            uid: [0; 24],
            settings: b"",
            affiliate: 0,
            preferences: 0,
            verify_id: 0,
            checksums: [0; 28],
        }
    }
}

/// `client.writeUid`: zeros when the client has no UID file at all (None);
/// otherwise the file's first 24 bytes, or 24 times 0xFF when the file is
/// shorter than that or all zero.
pub fn uid_bytes(file: Option<&[u8]>) -> [u8; 24] {
    let Some(file) = file else {
        return [0; 24];
    };
    match file.get(..24) {
        Some(bytes) if bytes.iter().any(|&b| b != 0) => bytes.try_into().expect("24 bytes"),
        _ => [0xFF; 24],
    }
}

/// `Preferences.toInt()`.
pub fn preferences_to_int(p: &crate::ui::vm::Preferences) -> i32 {
    let bit = |on: bool, shift: u32| i32::from(on) << shift;
    bit(p.stereo, 19)
        .wrapping_add(bit(p.fog, 16))
        .wrapping_add(bit(p.high_water_detail, 15))
        .wrapping_add(bit(p.high_detail_lighting, 13))
        .wrapping_add(bit(p.character_shadows, 10))
        .wrapping_add(bit(p.many_ground_textures, 9))
        .wrapping_add(bit(p.many_idle_animations, 7))
        .wrapping_add(bit(p.high_detail_textures, 6))
        .wrapping_add(bit(p.show_ground_decorations, 5))
        .wrapping_add(bit(p.all_levels_visible, 3))
        .wrapping_add(p.brightness & 7)
        .wrapping_add(bit(p.remove_roofs_selectively, 4))
        .wrapping_add(bit(p.flickering_effects, 8))
        .wrapping_add((p.scenery_shadows & 3) << 11)
        .wrapping_add(bit(p.sound_volume != 0, 20))
        .wrapping_add(bit(p.music_volume != 0, 21))
        .wrapping_add(bit(p.ambient_volume != 0, 22))
        .wrapping_add(p.particles.wrapping_shl(23))
}

/// `Base37.decode37`: None for the values the original rejects.
pub fn decode37(value: i64) -> Option<Vec<u8>> {
    (value > 0 && value < 6_582_952_005_840_035_281 && value % 37 != 0)
        .then(|| login::decode_name(value).into_bytes())
}

/// `JagString.toTitleCase` of base-37 text.
fn title_case(name: &[u8]) -> Vec<u8> {
    let text: String = name.iter().map(|&b| char::from(b)).collect();
    crate::ui::social::title_case(&text)
        .chars()
        .map(|c| c as u32 as u8)
        .collect()
}

/// A script command the original would abort with an unchecked exception.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScriptError {
    /// `Base37.decode37` returned null: the name has no letters or digits.
    InvalidName,
    /// The plain block does not fit the original's fixed buffer.
    BlockTooLong,
    /// Command 5610 with no suggestion list, or with a suggestion the server
    /// sent as an invalid base-37 value.
    NoSuggestions,
}
impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidName => "name has no base-37 form",
            Self::BlockTooLong => "request does not fit the original buffer",
            Self::NoSuggestions => "no suggested names to read",
        })
    }
}
impl std::error::Error for ScriptError {}

/// The statics of `LoginManager` that belong to logging in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Login {
    /// 0 idle, 1 connect, 2 handshake, 3 server key and login block, 4 reply,
    /// 5 waiting for the script to continue, 6 send the continue opcode,
    /// 7 world-switch delay byte, 8 player details, 9 first packet,
    /// 10 disallow detail byte.
    pub step: i32,
    pub loops: i32,
    pub errors: i32,
    /// -2 nothing asked, -3 in progress, -4 connection error, -5 timed out,
    /// otherwise the server's reply code.
    pub reply: i32,
    pub hop_step: i32,
    pub hop_loops: i32,
    pub hop_errors: i32,
    /// Reply 21: `(seconds byte + 3) * 60`. The client never counts it down.
    pub hop_time: i32,
    /// Reply 29's detail byte; -1 when none.
    pub disallow_result: i32,
    pub ping_failed: bool,
    pub server_key: i64,
    pub login_type: i32,
    pub staff_mod_level: i32,
    pub blackmarks: i32,
    pub player_underage: bool,
    pub parental_chat_consent: bool,
    pub parental_advert_consent: bool,
    pub map_quick_chat: bool,
    /// `MouseRecorder.enabled`.
    pub mouse_recorder: bool,
    /// `PlayerList.selfId`.
    pub self_id: i32,
    pub player_member: bool,
    pub map_members: bool,
    /// `Protocol.opcode` and `Protocol.length` of the first packet.
    pub opcode: i32,
    pub length: usize,
    /// The ISAAC seed of the connection in use: two client words, then the
    /// server key.
    pub isaac_seed: [i32; 4],
}
impl Default for Login {
    fn default() -> Self {
        Self {
            step: 0,
            loops: 0,
            errors: 0,
            reply: -2,
            hop_step: 0,
            hop_loops: 0,
            hop_errors: 0,
            hop_time: 0,
            disallow_result: -1,
            ping_failed: false,
            server_key: 0,
            login_type: -1,
            staff_mod_level: 0,
            blackmarks: 0,
            player_underage: false,
            parental_chat_consent: false,
            parental_advert_consent: false,
            map_quick_chat: false,
            mouse_recorder: false,
            self_id: 0,
            player_member: false,
            map_members: false,
            opcode: -1,
            length: 0,
            isaac_seed: [0; 4],
        }
    }
}

/// The statics of `CreateManager`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Create {
    /// 0 idle, 1 connect, 2 request and reply, 3 suggestion count,
    /// 4 suggestions.
    pub step: i32,
    pub loops: i32,
    pub errors: i32,
    /// -2 nothing asked, -3 in progress, -4 connection error, -5 timed out,
    /// otherwise the server's reply code (21 carries suggested names).
    pub reply: i32,
    /// `suggestedNames`: None is the original's null array; an inner None is
    /// a value `Base37.decode37` rejected.
    pub suggested_names: Option<Vec<Option<Vec<u8>>>>,
}
impl Default for Create {
    fn default() -> Self {
        Self {
            step: 0,
            loops: 0,
            errors: 0,
            reply: -2,
            suggested_names: None,
        }
    }
}

/// What `client.setGameState` does besides storing the state. The flow has
/// already cleared the login state and closed the previous socket where the
/// original does; the rest is for the caller to carry out.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateChange {
    pub from: i32,
    pub to: i32,
    /// Entering 40: `LoginManager.clear()` ran, so a login attempt starts on
    /// the next cycle.
    pub login_cleared: bool,
    /// Leaving for anything but 40 closed `Protocol.previousSocket`.
    pub previous_socket_closed: bool,
    /// Entering 25 or 28: zero the missing-file counters, set both peaks to
    /// 1, loading-screen state 0, and `WorldMap.clear(true)`.
    pub loading_reset: bool,
    /// Entering 5 loads and draws the title background and logo; every other
    /// state releases them.
    pub title_background: bool,
    /// Some(true): entered the title group (5, 10, 28) from outside it, so
    /// start the title song and tell the cache server "logged out".
    /// Some(false): left it, so fade the music out and tell the cache server
    /// "logged in". None: stayed on the same side.
    pub title_music: Option<bool>,
}

/// A completed login: `LoginManager.loop` step 9. The game state is already
/// 25 and the connection is [`Net::socket`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entered {
    pub world: crate::session::WorldLogin,
    /// The eleven detail bytes as they arrived.
    pub details: [u8; 11],
    /// The first packet's opcode after the inbound cipher. The original does
    /// not look at it: it treats the payload as a normal map rebuild.
    pub opcode: u8,
    /// The first packet's three header bytes as they arrived.
    pub header: [u8; 3],
    /// The first packet's payload: `Protocol.readRebuildPacket(false)` input.
    pub rebuild: Vec<u8>,
    pub server_key: i64,
    pub isaac_seed: [i32; 4],
    /// Opcodes already written through the outbound cipher (one per
    /// continue-login command).
    pub outbound_opcodes: u32,
    pub change: Option<StateChange>,
}
impl Entered {
    /// The outbound opcode cipher, advanced past what login already sent.
    pub fn outbound_cipher(&self, profile: Profile) -> OpcodeCipher {
        let mut cipher = cipher(self.isaac_seed, 0, profile);
        for _ in 0..self.outbound_opcodes {
            cipher.next_key();
        }
        cipher
    }
    /// A server-to-client frame decoder whose cipher is past the first
    /// packet's opcode, for the bytes that follow that packet.
    pub fn decoder(&self, profile: Profile, max_buffer: usize, max_payload: usize) -> Decoder {
        let mut cipher = cipher(self.isaac_seed, 50, profile);
        cipher.next_key();
        Decoder::new(Direction::ServerToClient, max_buffer, max_payload).with_cipher(cipher)
    }
    /// The first packet as a frame.
    pub fn frame(&self) -> Frame {
        Frame {
            opcode: self.opcode,
            payload: self.rebuild.clone(),
        }
    }
    /// The existing in-game [`crate::session::Session`] brought to the point
    /// this login reached, by replaying the reply bytes into it. It holds the
    /// same cipher states as this connection and has already produced the
    /// world details and the first frame, which are returned with it.
    ///
    /// `Session` has no constructor for an established connection, hence the
    /// replay; it also insists that the first packet is opcode 162, which
    /// the original does not check.
    pub fn session(
        &self,
        username: &[u8],
        profile: Profile,
        width: i32,
        height: i32,
    ) -> rs09_protocol::Result<(crate::session::Session, Vec<crate::session::Event>)> {
        use crate::session::{Config, Session};
        let mut config = Config::new(
            username.to_vec(),
            b"-".to_vec(),
            [self.isaac_seed[0], self.isaac_seed[1]],
        );
        config.profile = profile;
        config.width = width;
        config.height = height;
        let mut session = Session::new(config)?;
        let mut bytes = vec![0];
        bytes.extend_from_slice(&self.server_key.to_be_bytes());
        session.feed(&bytes)?;
        for _ in 0..self.outbound_opcodes {
            session.feed(&[1])?;
            session.confirm()?;
        }
        while session.take_output().is_some() {}
        let mut bytes = vec![2];
        bytes.extend_from_slice(&self.details);
        bytes.extend_from_slice(&self.header);
        bytes.extend_from_slice(&self.rebuild);
        let events = session.feed(&bytes)?;
        Ok((session, events))
    }
}

/// Game state 40 ended with reply 15: the new connection ([`Net::socket`])
/// carries on the session. The game state is already 30.
///
/// `LoginManager.reconnect` for the caller: empty the outbound buffer; forget
/// the current and the three previous packet opcodes and the packet length;
/// zero the map flag, the mini-menu size, the minimap state, the reboot timer
/// and the ticks since the last packet; close the menu; set every player's
/// and NPC's face-entity to -1; clear the inventories (`Inv.clear`); set the
/// camera type to 1; mark all interface rectangles dirty; then send the
/// window details packet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reconnected {
    pub isaac_seed: [i32; 4],
    pub change: Option<StateChange>,
}
impl Reconnected {
    /// Fresh ciphers for the new connection: nothing has passed through
    /// either yet.
    pub fn ciphers(&self, profile: Profile) -> (OpcodeCipher, OpcodeCipher) {
        (
            cipher(self.isaac_seed, 0, profile),
            cipher(self.isaac_seed, 50, profile),
        )
    }
}

/// `new Buffer(capacity)`: a write past the end is the original's
/// `ArrayIndexOutOfBoundsException`.
struct Fixed {
    bytes: Vec<u8>,
    capacity: usize,
}
impl Fixed {
    fn new(capacity: usize) -> Self {
        Self {
            bytes: Vec::new(),
            capacity,
        }
    }
    fn put(&mut self, bytes: &[u8]) -> Result<(), ScriptError> {
        if self.bytes.len() + bytes.len() > self.capacity {
            return Err(ScriptError::BlockTooLong);
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    /// `Buffer.rsaenc`: the length byte and the encrypted block, which the
    /// original writes back into the same buffer.
    fn rsaenc(self, key: &RsaKey) -> Result<Vec<u8>, ScriptError> {
        let frame = key
            .frame(&self.bytes)
            .map_err(|_| ScriptError::BlockTooLong)?;
        if frame.len() > self.capacity {
            return Err(ScriptError::BlockTooLong);
        }
        Ok(frame)
    }
}

fn cipher(seed: [i32; 4], offset: i32, profile: Profile) -> OpcodeCipher {
    let seed = seed.map(|word| word.wrapping_add(offset) as u32);
    OpcodeCipher::new(&seed, profile.use_isaac).expect("four seed words")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Event {
    /// The login block went out with `Preferences.toInt()` in it: set
    /// `Preferences.sentToServer`.
    PreferencesSent,
    Entered(Box<Entered>),
    Reconnected(Reconnected),
    /// Game state 40 ended with any other reply: `LoginManager.processLogout`
    /// ran as far as the flow can take it (see [`LoginFlow::process_logout`]).
    LoggedOut {
        reply: i32,
        change: Option<StateChange>,
    },
}

/// `Protocol.handleConnectionLost`'s two outcomes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lost {
    /// The logout-on-disconnect timer was running: logged out to the title
    /// screen.
    LoggedOut(Option<StateChange>),
    /// Game state 40: logging in again with the remembered credentials.
    Reconnecting(Option<StateChange>),
}

/// `client.mainLoad` state 135.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldListStartup {
    /// Keep showing "Loading world list data" at 95%.
    Loading,
    /// "Loaded world list data", 96%; go on to load state 140.
    Loaded,
    /// The server answered 7 or 9: error "worldlistfull", game state 1000.
    Full,
    /// Error "worldlistio_N", game state 1000.
    Failed(i32),
}

/// The peaks behind the loading percentage of game states 25 and 28
/// (`client.peakMapFilesMissing` and `peakLocModelsMissing`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoadingProgress {
    pub peak_map_files: i32,
    pub peak_loc_models: i32,
}
impl Default for LoadingProgress {
    fn default() -> Self {
        Self {
            peak_map_files: 1,
            peak_loc_models: 1,
        }
    }
}
impl LoadingProgress {
    /// The percentage `client.mainRedraw` shows: None in loading-screen
    /// state 0 (plain "Loading - please wait."), 0 to 50 while map files are
    /// missing (state 1), 50 to 100 while scenery models are (state 2).
    pub fn percent(
        &mut self,
        loading_screen_state: i32,
        map_files_missing: i32,
        loc_models_missing: i32,
    ) -> Option<i32> {
        match loading_screen_state {
            1 => {
                self.peak_map_files = self.peak_map_files.max(map_files_missing);
                Some((self.peak_map_files - map_files_missing) * 50 / self.peak_map_files)
            }
            2 => {
                self.peak_loc_models = self.peak_loc_models.max(loc_models_missing);
                Some((self.peak_loc_models - loc_models_missing) * 50 / self.peak_loc_models + 50)
            }
            _ => None,
        }
    }
}

/// The English text `client.mainRedraw` draws in a box at the top left
/// (`Fonts.drawTextOnScreen`) in game states 25, 28 and 40; `<br>` is the
/// original's line break. None in every other state.
pub fn status_text(game_state: i32, percent: Option<i32>) -> Option<String> {
    match game_state {
        25 | 28 => Some(match percent {
            Some(percent) => format!("Loading - please wait.<br>({percent}%)"),
            None => "Loading - please wait.".to_owned(),
        }),
        40 => Some("Connection lost.<br>Please wait - attempting to reestablish.".to_owned()),
        _ => None,
    }
}

/// `LoginManager`, `CreateManager`, the world list and the game state.
pub struct LoginFlow<D: Dialer> {
    pub net: Net<D>,
    pub endpoints: Endpoints,
    pub worlds: WorldList,
    pub login: Login,
    pub create: Create,
    /// `client.gameState`: 0 loading, 5 loading behind the title background,
    /// 10 title screen, 25 building the map, 28 building the title screen's
    /// map, 30 in game, 40 connection lost, 1000 fatal error.
    pub game_state: i32,
    /// `GlRenderer.enabled`: the title screen has a 3D scene behind it, so
    /// returning to it passes through game state 28.
    pub title_scene: bool,
    /// `Player.usernameInput`, `Player.password` and `Player.name37`. The
    /// original keeps the password for the whole session; game state 40
    /// logs in again with it.
    username: Vec<u8>,
    password: Vec<u8>,
    pub name37: i64,
    /// `Math.random()`.
    random: Box<dyn FnMut() -> f64>,
    profile: Profile,
    key: RsaKey,
    out_cipher: Option<OpcodeCipher>,
    in_cipher: Option<OpcodeCipher>,
    outbound_opcodes: u32,
    details: [u8; 11],
    header: [u8; 3],
    login_awaiting: bool,
    hop_awaiting: bool,
    create_awaiting: bool,
}

impl<D: Dialer> LoginFlow<D> {
    /// The pinned profile and RSA key. `random` is `Math.random()`: uniform
    /// in `[0, 1)`.
    pub fn new(dialer: D, endpoints: Endpoints, random: Box<dyn FnMut() -> f64>) -> Self {
        Self::with_profile(
            dialer,
            endpoints,
            random,
            RS09_REVISION_530,
            RsaKey::reference_public(),
        )
    }
    pub fn with_profile(
        dialer: D,
        endpoints: Endpoints,
        random: Box<dyn FnMut() -> f64>,
        profile: Profile,
        key: RsaKey,
    ) -> Self {
        Self {
            net: Net::new(dialer),
            endpoints,
            worlds: WorldList::default(),
            login: Login::default(),
            create: Create::default(),
            game_state: 0,
            title_scene: false,
            username: Vec::new(),
            password: Vec::new(),
            name37: 0,
            random,
            profile,
            key,
            out_cipher: None,
            in_cipher: None,
            outbound_opcodes: 0,
            details: [0; 11],
            header: [0; 3],
            login_awaiting: false,
            hop_awaiting: false,
            create_awaiting: false,
        }
    }
    pub fn profile(&self) -> Profile {
        self.profile
    }
    /// `Player.usernameInput`.
    pub fn username(&self) -> &[u8] {
        &self.username
    }
    /// Overwrite and drop the remembered password. The original never does:
    /// game state 40 logs in again with it, so after this a reconnect sends
    /// an empty password and the server refuses it.
    pub fn forget_password(&mut self) {
        self.password.fill(0);
        self.password.clear();
    }

    /// `(int) (Math.random() * scale)`.
    fn random_int(&mut self, scale: f64) -> i32 {
        ((self.random)() * scale) as i32
    }

    // ----- client -----

    /// `client.setGameState`. None when the state does not change.
    pub fn set_game_state(&mut self, state: i32) -> Option<StateChange> {
        if self.game_state == state {
            return None;
        }
        let from = self.game_state;
        let login_cleared = state == 40;
        if login_cleared {
            self.clear();
        }
        let entering = matches!(state, 5 | 10 | 28);
        let previous = if state != 40 {
            self.net.previous_socket.take()
        } else {
            None
        };
        let previous_socket_closed = previous.is_some();
        if let Some(mut socket) = previous {
            socket.close();
        }
        let was = matches!(from, 5 | 10 | 28);
        self.game_state = state;
        Some(StateChange {
            from,
            to: state,
            login_cleared,
            previous_socket_closed,
            loading_reset: state == 25 || state == 28,
            title_background: state == 5,
            title_music: (was != entering).then_some(entering),
        })
    }

    /// `client.mainLoad` state 135: the first world-list fetch, called once
    /// per cycle until it stops returning [`WorldListStartup::Loading`].
    pub fn startup_world_list(&mut self, now_ms: i64) -> WorldListStartup {
        let code = self
            .worlds
            .fetch(&mut self.net, &mut self.endpoints, now_ms);
        if code == -1 {
            WorldListStartup::Loading
        } else if code == 7 || code == 9 {
            WorldListStartup::Full
        } else if self.worlds.loaded {
            WorldListStartup::Loaded
        } else {
            WorldListStartup::Failed(code)
        }
    }

    /// The end of `InterfaceList.resetToLoginScreen`: game state 10, or 28
    /// first when the title screen has a scene to build (the caller moves on
    /// to 10 with [`Self::set_game_state`] when that scene is ready).
    pub fn reset_to_login_screen(&mut self) -> Option<StateChange> {
        self.set_game_state(if self.title_scene { 28 } else { 10 })
    }

    /// `LoginManager.processLogout` as far as this module reaches: the
    /// connection is closed and the game state returns to the title screen.
    ///
    /// The rest of the original, in order, for the caller: unload the caches
    /// (`client.unload`), clear the scene and the four collision maps, clear
    /// the world map, fade the music out and forget the jingle flag and the
    /// song (`music.reset()`), clear area sounds, leave any dynamic map
    /// region, zero the camera origin and scene centre, clear the hint
    /// markers, players, NPCs and ground items, reset camera effects,
    /// `Protocol.verifyId = 0`, reset the varps, then open the title
    /// interface as the top level and run its load scripts
    /// (`InterfaceList.resetToLoginScreen(true)`).
    pub fn process_logout(&mut self) -> Option<StateChange> {
        self.net.close();
        self.reset_to_login_screen()
    }

    /// `Protocol.handleConnectionLost`, called in game state 30 when a read
    /// or write fails, the ping fails, or 750 cycles pass without a packet.
    /// `socket` is the game connection if the caller still has it; it is
    /// kept until the reconnect ends, as the original keeps
    /// `Protocol.previousSocket`.
    pub fn connection_lost(
        &mut self,
        logout_on_disconnect_timer: i32,
        socket: Option<D::Socket>,
    ) -> Lost {
        if let Some(socket) = socket {
            self.net.socket = Some(socket);
        }
        if logout_on_disconnect_timer > 0 {
            Lost::LoggedOut(self.process_logout())
        } else {
            self.net.previous_socket = self.net.socket.take();
            Lost::Reconnecting(self.set_game_state(40))
        }
    }

    /// `LoginManager.reconnect`'s state change; see [`Reconnected`].
    fn reconnect(&mut self) -> Reconnected {
        self.net.outbound.clear();
        Reconnected {
            isaac_seed: self.login.isaac_seed,
            change: self.set_game_state(30),
        }
    }

    /// The network part of one client cycle (`client.mainLoop`). In game
    /// state 10 call it after the title interface's scripts have run
    /// (`mainUpdate`); in state 40 it is the whole cycle. It does nothing in
    /// any other state.
    pub fn cycle(&mut self, env: &Environment<'_>) -> Vec<Event> {
        let mut events = Vec::new();
        if self.game_state == 10 {
            self.create_loop();
            self.hop_loop();
            self.login_loop(env, &mut events);
        } else if self.game_state == 40 {
            self.login_loop(env, &mut events);
            if self.login.reply != -3 {
                if self.login.reply == 15 {
                    events.push(Event::Reconnected(self.reconnect()));
                } else if self.login.reply != 2 {
                    let reply = self.login.reply;
                    let change = self.process_logout();
                    events.push(Event::LoggedOut { reply, change });
                }
            }
        }
        events
    }

    // ----- LoginManager -----

    /// `LoginManager.clear`: start a login attempt with the remembered
    /// credentials on the next cycle.
    pub fn clear(&mut self) {
        self.login.ping_failed = false;
        self.login.hop_time = 0;
        self.login.reply = -3;
        self.login.loops = 0;
        self.login.step = 1;
        self.login.errors = 0;
        self.login.disallow_result = -1;
        self.login_awaiting = false;
    }

    /// `LoginManager.startLogin`.
    ///
    /// With no world chosen (`Player.worldId == -1`) the original builds a
    /// world-hop request in a 128-byte buffer and encrypts it in place. The
    /// pinned 1024-bit key's output is 128 bytes plus a length byte, so that
    /// almost always overruns the buffer and the script command dies with an
    /// exception after the hop state was already armed. That is kept: the
    /// error is returned, `hop_step` is 1, and the hop then sends whatever
    /// the shared outbound buffer last held. (The pinned client never gets
    /// here: it always starts with a world chosen.)
    pub fn start_login(
        &mut self,
        username: &[u8],
        password: &[u8],
        auth_type: i32,
    ) -> Result<(), ScriptError> {
        self.password.fill(0);
        self.password = password.to_vec();
        self.login.login_type = auth_type;
        self.username = username.to_vec();
        if self.username.is_empty() || self.password.is_empty() {
            self.login.reply = 3;
        } else if self.endpoints.world_id == -1 {
            self.login.hop_loops = 0;
            self.login.hop_errors = 0;
            self.login.reply = -3;
            self.login.hop_step = 1;
            self.hop_awaiting = false;
            let mut block = Fixed::new(128);
            block.put(&[10])?;
            let noise = self.random_int(99999.0);
            block.put(&(noise as u16).to_be_bytes())?;
            block.put(&530u16.to_be_bytes())?;
            block.put(&login::encode_name(&self.username).to_be_bytes())?;
            let noise = self.random_int(9.9999999E7);
            block.put(&noise.to_be_bytes())?;
            block.put(&self.password)?;
            block.put(&[0])?;
            let noise = self.random_int(9.9999999E7);
            block.put(&noise.to_be_bytes())?;
            let encrypted = block.rsaenc(&self.key)?;
            self.net.outbound = vec![210, encrypted.len() as u8];
            self.net.outbound.extend_from_slice(&encrypted);
        } else {
            self.clear();
        }
        Ok(())
    }

    /// `LoginManager.continueDelayedLogin`.
    pub fn continue_delayed_login(&mut self) {
        if self.login.step == 5 {
            self.login.step = 6;
        }
    }

    /// `LoginManager.loopAuto`: the world hop of a login without a world.
    fn hop_loop(&mut self) {
        if self.login.hop_step == 0 {
            return;
        }
        if self.hop_steps().is_err() {
            self.net.close();
            self.hop_awaiting = false;
            if self.login.hop_errors < 1 {
                self.endpoints.toggle_world_list_port();
                self.login.hop_step = 1;
                self.login.hop_loops = 0;
                self.login.hop_errors += 1;
            } else {
                self.login.reply = -4;
                self.login.hop_step = 0;
            }
        }
    }

    fn hop_steps(&mut self) -> io::Result<()> {
        if !self.hop_awaiting {
            self.login.hop_loops += 1;
            if self.login.hop_loops > 1500 {
                self.net.close();
                if self.login.hop_errors >= 1 {
                    self.login.reply = -5;
                    self.login.hop_step = 0;
                    return Ok(());
                }
                self.login.hop_loops = 0;
                self.login.hop_errors += 1;
                self.login.hop_step = 1;
                self.endpoints.toggle_world_list_port();
            }
        }
        if self.login.hop_step == 1 {
            self.net.open(
                &self.endpoints.world_list_hostname,
                self.endpoints.world_list_port,
            );
            self.login.hop_step = 2;
        }
        if self.login.hop_step == 2 {
            if !self.hop_awaiting {
                match self.net.poll() {
                    Opened::Failed => return Err(io::ErrorKind::ConnectionRefused.into()),
                    Opened::Pending => return Ok(()),
                    Opened::Ready(()) => {}
                }
                let bytes = self.net.outbound.clone();
                self.net.write(&bytes)?;
                self.hop_awaiting = true;
            }
            let Some(response) = self.net.read_blocking()? else {
                return Ok(());
            };
            self.hop_awaiting = false;
            if response != 101 {
                self.login.reply = response;
                self.login.hop_step = 0;
                self.net.close();
                return Ok(());
            }
            self.login.hop_step = 3;
        }
        if self.login.hop_step == 3 {
            if self.net.available()? < 2 {
                return Ok(());
            }
            let bytes = self.net.read(2)?;
            let world = (i32::from(bytes[0]) << 8) | i32::from(bytes[1]);
            self.hop_world(world);
            if self.endpoints.world_id == -1 {
                self.login.hop_step = 0;
                self.login.reply = 6;
                self.net.close();
                return Ok(());
            }
            self.login.hop_step = 0;
            self.net.close();
            self.clear();
        }
        Ok(())
    }

    /// `WorldList.hopWorld`: point the client at a world of the list. False
    /// when the list has no such world.
    ///
    /// Only the branch the pinned client can take is here: its
    /// `SignLink.clientMode` is the constant 1, so the other branch (send the
    /// browser to the world's page) never runs.
    pub fn hop_world(&mut self, world_id: i32) -> bool {
        let Some(world) = self.worlds.world(world_id) else {
            return false;
        };
        let hostname: String = world.hostname.iter().map(|&b| char::from(b)).collect();
        let id = world.id;
        self.endpoints.hostname = hostname;
        self.endpoints.world_id = id;
        if self.endpoints.mode_where != 0 {
            self.endpoints.default_port = id + 43594;
            self.endpoints.port = self.endpoints.default_port;
            self.endpoints.alternate_port = id + 43594;
        }
        true
    }

    /// `LoginManager.loop`.
    fn login_loop(&mut self, env: &Environment<'_>, events: &mut Vec<Event>) {
        if self.login.step == 0 || self.login.step == 5 {
            return;
        }
        if self.login_steps(env, events).is_err() {
            self.net.close();
            self.login_awaiting = false;
            if self.login.errors >= 1 {
                self.login.step = 0;
                self.login.reply = -4;
            } else {
                self.login.step = 1;
                self.login.loops = 0;
                self.login.errors += 1;
                self.endpoints.toggle_port();
            }
        }
    }

    fn login_steps(&mut self, env: &Environment<'_>, events: &mut Vec<Event>) -> io::Result<()> {
        if !self.login_awaiting {
            self.login.loops += 1;
            if self.login.loops > 2000 {
                self.net.close();
                if self.login.errors >= 1 {
                    self.login.reply = -5;
                    self.login.step = 0;
                    return Ok(());
                }
                self.login.loops = 0;
                self.endpoints.toggle_port();
                self.login.step = 1;
                self.login.errors += 1;
            }
        }
        if self.login.step == 1 {
            if let Some(json) = &self.endpoints.json {
                self.endpoints.hostname = json.ip_management.clone();
                self.endpoints.port = json.server_port + self.endpoints.world_list_id;
            }
            self.net.open(&self.endpoints.hostname, self.endpoints.port);
            self.login.step = 2;
        }
        if self.login.step == 2 {
            if !self.login_awaiting {
                match self.net.poll() {
                    Opened::Failed => return Err(io::ErrorKind::ConnectionRefused.into()),
                    Opened::Pending => return Ok(()),
                    Opened::Ready(()) => {}
                }
                self.name37 = login::encode_name(&self.username);
                self.net.outbound = login::handshake(&self.username).to_vec();
                let bytes = self.net.outbound.clone();
                self.net.write(&bytes)?;
                self.login_awaiting = true;
            }
            let Some(response) = self.net.read_blocking()? else {
                return Ok(());
            };
            self.login_awaiting = false;
            if response != 0 {
                self.login.reply = response;
                self.login.step = 0;
                self.net.close();
                return Ok(());
            }
            self.login.step = 3;
        }
        if self.login.step == 3 {
            if self.net.available()? < 8 {
                return Ok(());
            }
            let bytes = self.net.read(8)?;
            self.login.server_key = i64::from_be_bytes(bytes.try_into().expect("eight bytes"));
            let mut key = [0i32; 4];
            key[2] = (self.login.server_key >> 32) as i32;
            key[3] = self.login.server_key as i32;
            key[1] = self.random_int(9.9999999E7);
            key[0] = self.random_int(9.9999999E7);
            let mut crc_sums = [0; login::CACHE_INDEX_COUNT];
            crc_sums[..28].copy_from_slice(&env.checksums);
            let block = LoginRequest {
                reconnect: self.game_state == 40,
                login_type: self.login.login_type,
                advertisements_suppressed: env.advert_suppressed,
                window_mode: env.window_mode,
                screen_width: env.canvas_width,
                screen_height: env.canvas_height,
                display_mode: env.anti_aliasing,
                uid: env.uid,
                settings: env.settings,
                affiliate: env.affiliate,
                settings_hash: env.preferences,
                packet_count: env.verify_id,
                crc_sums,
                isaac_seed: key,
                username: &self.username,
                password: &self.password,
            }
            .encode(self.profile, &self.key);
            let Ok(block) = block else {
                self.login.reply = -4;
                self.login.step = 0;
                self.net.close();
                return Ok(());
            };
            events.push(Event::PreferencesSent);
            // Protocol.outboundBuffer holds the encrypted part of the block.
            let fake_index = if self.profile.login_fake_index_28 {
                4
            } else {
                0
            };
            let secret = (163 + fake_index + env.settings.len()).min(block.len());
            self.net.outbound = block[secret..].to_vec();
            self.net.write(&block)?;
            self.login.isaac_seed = key;
            self.out_cipher = Some(cipher(key, 0, self.profile));
            self.in_cipher = Some(cipher(key, 50, self.profile));
            self.outbound_opcodes = 0;
            self.login.step = 4;
        }
        if self.login.step == 4 {
            if self.net.available()? < 1 {
                return Ok(());
            }
            let reply = i32::from(self.net.read(1)?[0]);
            if reply == 21 {
                self.login.step = 7;
            } else if reply == 29 {
                self.login.step = 10;
            } else if reply == 1 {
                self.login.step = 5;
                self.login.reply = reply;
                return Ok(());
            } else if reply == 2 {
                self.login.step = 8;
            } else if reply == 15 {
                self.login.step = 0;
                self.login.reply = reply;
                return Ok(());
            } else if reply == 23 && self.login.errors < 1 {
                self.login.step = 1;
                self.login.errors += 1;
                self.login.loops = 0;
                self.net.close();
                return Ok(());
            } else {
                self.login.reply = reply;
                self.login.step = 0;
                self.net.close();
                return Ok(());
            }
        }
        if self.login.step == 6 {
            let opcode = self
                .out_cipher
                .as_mut()
                .map_or(17, |cipher| cipher.encode(17));
            self.outbound_opcodes += 1;
            self.net.outbound = vec![opcode];
            self.net.write(&[opcode])?;
            self.login.step = 4;
            return Ok(());
        }
        if self.login.step == 7 {
            if self.net.available()? >= 1 {
                self.login.hop_time = (i32::from(self.net.read(1)?[0]) + 3) * 60;
                self.login.step = 0;
                self.login.reply = 21;
                self.net.close();
            }
            return Ok(());
        }
        if self.login.step == 10 {
            if self.net.available()? >= 1 {
                self.login.disallow_result = i32::from(self.net.read(1)?[0]);
                self.login.step = 0;
                self.login.reply = 29;
                self.net.close();
            }
            return Ok(());
        }
        if self.login.step == 8 {
            if self.net.available()? < 14 {
                return Ok(());
            }
            let bytes = self.net.read(14)?;
            self.details.copy_from_slice(&bytes[..11]);
            self.header.copy_from_slice(&bytes[11..]);
            self.login.staff_mod_level = i32::from(bytes[0]);
            self.login.blackmarks = i32::from(bytes[1]);
            self.login.player_underage = bytes[2] == 1;
            self.login.parental_chat_consent = bytes[3] == 1;
            self.login.parental_advert_consent = bytes[4] == 1;
            self.login.map_quick_chat = bytes[5] == 1;
            self.login.mouse_recorder = bytes[6] == 1;
            self.login.self_id = (i32::from(bytes[7]) << 8) | i32::from(bytes[8]);
            self.login.player_member = bytes[9] == 1;
            self.login.map_members = bytes[10] == 1;
            self.login.opcode = i32::from(
                self.in_cipher
                    .as_mut()
                    .map_or(bytes[11], |cipher| cipher.decode(bytes[11])),
            );
            self.login.length = (usize::from(bytes[12]) << 8) | usize::from(bytes[13]);
            self.login.step = 9;
        }
        if self.login.step == 9 {
            if self.net.available()? < self.login.length {
                return Ok(());
            }
            let rebuild = self.net.read(self.login.length)?;
            self.login.reply = 2;
            self.login.step = 0;
            let world = crate::session::WorldLogin {
                staff_level: self.details[0],
                blackmarks: self.details[1],
                underage: self.login.player_underage,
                parental_chat_consent: self.login.parental_chat_consent,
                parental_ad_consent: self.login.parental_advert_consent,
                quick_chat: self.login.map_quick_chat,
                mouse_recording: self.login.mouse_recorder,
                player_index: self.login.self_id as u16,
                player_member: self.login.player_member,
                map_members: self.login.map_members,
            };
            let opcode = self.login.opcode as u8;
            self.login.opcode = -1;
            // client.resetGameSessionState empties the outbound buffer.
            self.net.outbound.clear();
            // readRebuildPacket ends in loadRegion, which enters state 25.
            let change = self.set_game_state(25);
            events.push(Event::Entered(Box::new(Entered {
                world,
                details: self.details,
                opcode,
                header: self.header,
                rebuild,
                server_key: self.login.server_key,
                isaac_seed: self.login.isaac_seed,
                outbound_opcodes: self.outbound_opcodes,
                change,
            })));
        }
        Ok(())
    }

    // ----- CreateManager -----

    /// `CreateManager.loop`.
    fn create_loop(&mut self) {
        if self.create.step == 0 {
            return;
        }
        if self.create_steps().is_err() {
            self.net.close();
            self.create_awaiting = false;
            if self.create.errors < 1 {
                self.create.errors += 1;
                self.endpoints.toggle_port();
                self.create.loops = 0;
                self.create.step = 1;
            } else {
                self.create.reply = -4;
                self.create.step = 0;
            }
        }
    }

    fn create_steps(&mut self) -> io::Result<()> {
        if !self.create_awaiting {
            self.create.loops += 1;
            if self.create.loops > 2000 {
                self.net.close();
                if self.create.errors >= 1 {
                    self.create.reply = -5;
                    self.create.step = 0;
                    return Ok(());
                }
                self.create.step = 1;
                self.create.loops = 0;
                self.create.errors += 1;
                self.endpoints.toggle_port();
            }
        }
        if self.create.step == 1 {
            self.net.open(&self.endpoints.hostname, self.endpoints.port);
            self.create.step = 2;
        }
        if self.create.step == 2 {
            if !self.create_awaiting {
                match self.net.poll() {
                    Opened::Failed => return Err(io::ErrorKind::ConnectionRefused.into()),
                    Opened::Pending => return Ok(()),
                    Opened::Ready(()) => {}
                }
                let bytes = self.net.outbound.clone();
                self.net.write(&bytes)?;
                self.create_awaiting = true;
            }
            let Some(response) = self.net.read_blocking()? else {
                return Ok(());
            };
            self.create_awaiting = false;
            if response != 21 {
                self.create.reply = response;
                self.create.step = 0;
                self.net.close();
                return Ok(());
            }
            self.create.step = 3;
        }
        if self.create.step == 3 {
            if self.net.available()? < 1 {
                return Ok(());
            }
            let count = usize::from(self.net.read(1)?[0]);
            self.create.suggested_names = Some(vec![None; count]);
            self.create.step = 4;
        }
        if self.create.step == 4 {
            let count = self.create.suggested_names.as_ref().map_or(0, Vec::len);
            if self.net.available()? < count * 8 {
                return Ok(());
            }
            let bytes = self.net.read(count * 8)?;
            let names = (0..count)
                .map(|i| {
                    let name = &bytes[i * 8..i * 8 + 8];
                    decode37(i64::from_be_bytes(name.try_into().expect("eight bytes")))
                })
                .collect();
            self.create.suggested_names = Some(names);
            self.create.reply = 21;
            self.create.step = 0;
            self.net.close();
        }
        Ok(())
    }

    fn create_started(&mut self) {
        self.create.step = 1;
        self.create.loops = 0;
        self.create.errors = 0;
        self.create.reply = -3;
        self.create_awaiting = false;
    }

    /// The name as the pinned profile sends it: text when
    /// `LOGIN_USE_STRINGS`, the base-37 value otherwise.
    fn name_bytes(&self, name: i64) -> Result<Vec<u8>, ScriptError> {
        if self.profile.login_use_strings {
            let mut text = decode37(name).ok_or(ScriptError::InvalidName)?;
            text.push(0);
            Ok(text)
        } else {
            Ok(name.to_be_bytes().to_vec())
        }
    }

    /// `CreateManager.checkName`: ask whether a name (as `encode37`) is free.
    pub fn check_name(&mut self, name: i64) -> Result<(), ScriptError> {
        // The original has reset the buffer and written the opcode before a
        // null name fails.
        self.net.outbound = vec![186];
        let name = self.name_bytes(name)?;
        self.net.outbound.extend_from_slice(&name);
        self.create_started();
        Ok(())
    }

    /// `CreateManager.checkInfo`: validate a date of birth and country.
    pub fn check_info(&mut self, year: i32, country: i32, day: i32, month: i32) {
        let mut out = Writer::new();
        out.p1(147);
        out.p1(day);
        out.p1(month);
        out.p2(year);
        out.p2(country);
        self.net.outbound = out.into_vec();
        self.create_started();
    }

    /// `CreateManager.createAccount`. `affiliate` is `client.affiliate`.
    #[allow(clippy::too_many_arguments)]
    pub fn create_account(
        &mut self,
        day: i32,
        country: i32,
        month: i32,
        password: &[u8],
        name: i64,
        year: i32,
        affiliate: i32,
    ) -> Result<(), ScriptError> {
        let mut block = Fixed::new(if self.profile.login_use_strings {
            129
        } else {
            128
        });
        block.put(&[10])?;
        let noise = self.random_int(99999.0);
        block.put(&(noise as u16).to_be_bytes())?;
        block.put(&530u16.to_be_bytes())?;
        block.put(&self.name_bytes(name)?)?;
        let noise = self.random_int(9.9999999E7);
        block.put(&noise.to_be_bytes())?;
        block.put(password)?;
        block.put(&[0])?;
        let noise = self.random_int(9.9999999E7);
        block.put(&noise.to_be_bytes())?;
        block.put(&(affiliate as u16).to_be_bytes())?;
        block.put(&[day as u8, month as u8])?;
        let noise = self.random_int(9.9999999E7);
        block.put(&noise.to_be_bytes())?;
        block.put(&(year as u16).to_be_bytes())?;
        block.put(&(country as u16).to_be_bytes())?;
        let noise = self.random_int(9.9999999E7);
        block.put(&noise.to_be_bytes())?;
        let encrypted = block.rsaenc(&self.key)?;
        self.net.outbound = vec![36, encrypted.len() as u8];
        self.net.outbound.extend_from_slice(&encrypted);
        self.create_started();
        Ok(())
    }

    // ----- ScriptRunner 5600..5611, 6500, 6503 -----

    /// The guard on the login and account commands: at the title screen with
    /// no request of any kind in progress.
    pub fn idle(&self) -> bool {
        self.game_state == 10
            && self.login.hop_step == 0
            && self.login.step == 0
            && self.create.step == 0
            && self.worlds.step == 0
    }

    /// Command 5600. The script pushed the username, then the password, and
    /// one integer (the login type).
    pub fn script_login(
        &mut self,
        username: &[u8],
        password: &[u8],
        login_type: i32,
    ) -> Result<(), ScriptError> {
        if self.idle() {
            self.start_login(username, password, login_type)?;
        }
        Ok(())
    }
    /// Command 5601.
    pub fn script_continue_login(&mut self) {
        self.continue_delayed_login();
    }
    /// Command 5602.
    pub fn script_reset_login_reply(&mut self) {
        if self.login.step == 0 {
            self.login.reply = -2;
        }
    }
    /// Command 5603: the four integers in the order the script pushed them
    /// (day, month, year, country).
    pub fn script_check_info(&mut self, ints: [i32; 4]) {
        if self.idle() {
            self.check_info(ints[2], ints[3], ints[0], ints[1]);
        }
    }
    /// Command 5604.
    pub fn script_check_name(&mut self, name: &[u8]) -> Result<(), ScriptError> {
        if self.idle() {
            self.check_name(login::encode_name(name))?;
        }
        Ok(())
    }
    /// Command 5605: the four integers in the order the script pushed them
    /// (day, month, year, country), then the name and the password.
    pub fn script_create_account(
        &mut self,
        ints: [i32; 4],
        name: &[u8],
        password: &[u8],
        affiliate: i32,
    ) -> Result<(), ScriptError> {
        if self.idle() {
            self.create_account(
                ints[0],
                ints[3],
                ints[1],
                password,
                login::encode_name(name),
                ints[2],
                affiliate,
            )?;
        }
        Ok(())
    }
    /// Command 5606.
    pub fn script_reset_create_reply(&mut self) {
        if self.create.step == 0 {
            self.create.reply = -2;
        }
    }
    /// Command 5610: five strings, the suggested names in title case and
    /// then empty ones. Reading them discards the list.
    pub fn script_suggested_names(&mut self) -> Result<[Vec<u8>; 5], ScriptError> {
        let names = self
            .create
            .suggested_names
            .as_ref()
            .ok_or(ScriptError::NoSuggestions)?;
        let mut out: [Vec<u8>; 5] = Default::default();
        for (slot, name) in out.iter_mut().zip(names) {
            *slot = title_case(name.as_ref().ok_or(ScriptError::NoSuggestions)?);
        }
        self.create.suggested_names = None;
        Ok(out)
    }
    /// Command 6500: 0 while the world list is being fetched, otherwise 1.
    pub fn script_fetch_world_list(&mut self, now_ms: i64) -> i32 {
        if self.game_state == 10
            && self.login.hop_step == 0
            && self.login.step == 0
            && self.create.step == 0
        {
            let code = self
                .worlds
                .fetch(&mut self.net, &mut self.endpoints, now_ms);
            i32::from(code != -1)
        } else {
            1
        }
    }
    /// Command 6503: 1 when the client now points at that world.
    pub fn script_hop_world(&mut self, world_id: i32) -> i32 {
        if self.game_state == 10
            && self.login.hop_step == 0
            && self.login.step == 0
            && self.create.step == 0
        {
            i32::from(self.hop_world(world_id))
        } else {
            0
        }
    }
}
/// Script commands 5600 to 5611, 6500 to 6503, 6506 and 6507 as one
/// object-safe interface, so the script VM can reach the flow without
/// knowing its socket type. Each method is the `ScriptRunner` branch of that
/// number; the two extra getters are what commands 3318 and 5015 read.
/// (6504 and 6505 only touch `Preferences.lastWorldId`.)
pub trait Commands {
    /// The script pushed the username, then the password, and one integer.
    fn op5600(
        &mut self,
        username: &[u8],
        password: &[u8],
        login_type: i32,
    ) -> Result<(), ScriptError>;
    fn op5601(&mut self);
    fn op5602(&mut self);
    /// The four integers in push order.
    fn op5603(&mut self, ints: [i32; 4]);
    fn op5604(&mut self, name: &[u8]) -> Result<(), ScriptError>;
    /// The four integers in push order; the script pushed the name, then the
    /// password. `affiliate` is `client.affiliate`.
    fn op5605(
        &mut self,
        ints: [i32; 4],
        name: &[u8],
        password: &[u8],
        affiliate: i32,
    ) -> Result<(), ScriptError>;
    fn op5606(&mut self);
    /// `LoginManager.reply`.
    fn op5607(&self) -> i32;
    /// `LoginManager.hopTime`.
    fn op5608(&self) -> i32;
    /// `CreateManager.reply`.
    fn op5609(&self) -> i32;
    /// Five strings to push in order.
    fn op5610(&mut self) -> Result<[Vec<u8>; 5], ScriptError>;
    /// `LoginManager.disallowResult`.
    fn op5611(&self) -> i32;
    /// `now_ms` is the monotonic clock; pushes the returned integer.
    fn op6500(&mut self, now_ms: i64) -> i32;
    /// None pushes -1, 0, "", 0, "", 0; an entry pushes its ID, flags,
    /// activity, country flag, country name and player count in that order
    /// (integers and strings on their own stacks).
    fn op6501(&mut self) -> Option<crate::world_list::Entry>;
    fn op6502(&mut self) -> Option<crate::world_list::Entry>;
    fn op6503(&mut self, world_id: i32) -> i32;
    /// As 6501 without the ID: None pushes -1, "", 0, "", 0.
    fn op6506(&self, world_id: i32) -> Option<crate::world_list::Entry>;
    /// The four integers in push order. An error aborts the script.
    fn op6507(&mut self, ints: [i32; 4]) -> Result<(), crate::world_list::EmptySlot>;
    /// `Player.worldId`, which command 3318 pushes.
    fn world_id(&self) -> i32;
    /// `Player.usernameInput`, which command 5015 pushes while the local
    /// player has no name yet.
    fn username_input(&self) -> &[u8];
}
impl<D: Dialer> Commands for LoginFlow<D> {
    fn op5600(
        &mut self,
        username: &[u8],
        password: &[u8],
        login_type: i32,
    ) -> Result<(), ScriptError> {
        self.script_login(username, password, login_type)
    }
    fn op5601(&mut self) {
        self.script_continue_login();
    }
    fn op5602(&mut self) {
        self.script_reset_login_reply();
    }
    fn op5603(&mut self, ints: [i32; 4]) {
        self.script_check_info(ints);
    }
    fn op5604(&mut self, name: &[u8]) -> Result<(), ScriptError> {
        self.script_check_name(name)
    }
    fn op5605(
        &mut self,
        ints: [i32; 4],
        name: &[u8],
        password: &[u8],
        affiliate: i32,
    ) -> Result<(), ScriptError> {
        self.script_create_account(ints, name, password, affiliate)
    }
    fn op5606(&mut self) {
        self.script_reset_create_reply();
    }
    fn op5607(&self) -> i32 {
        self.login.reply
    }
    fn op5608(&self) -> i32 {
        self.login.hop_time
    }
    fn op5609(&self) -> i32 {
        self.create.reply
    }
    fn op5610(&mut self) -> Result<[Vec<u8>; 5], ScriptError> {
        self.script_suggested_names()
    }
    fn op5611(&self) -> i32 {
        self.login.disallow_result
    }
    fn op6500(&mut self, now_ms: i64) -> i32 {
        self.script_fetch_world_list(now_ms)
    }
    fn op6501(&mut self) -> Option<crate::world_list::Entry> {
        self.worlds.first_world()
    }
    fn op6502(&mut self) -> Option<crate::world_list::Entry> {
        self.worlds.next_world()
    }
    fn op6503(&mut self, world_id: i32) -> i32 {
        self.script_hop_world(world_id)
    }
    fn op6506(&self, world_id: i32) -> Option<crate::world_list::Entry> {
        self.worlds.get(world_id)
    }
    fn op6507(&mut self, ints: [i32; 4]) -> Result<(), crate::world_list::EmptySlot> {
        self.worlds
            .sort(ints[0], ints[1] == 1, ints[2], ints[3] == 1)
    }
    fn world_id(&self) -> i32 {
        self.endpoints.world_id
    }
    fn username_input(&self) -> &[u8] {
        &self.username
    }
}
impl<D: Dialer> Drop for LoginFlow<D> {
    fn drop(&mut self) {
        self.password.fill(0);
    }
}

/// [`Dialer`] over the client's [`crate::transport::Transport`]: TCP on
/// native builds, a WebSocket in the browser.
pub struct TransportDialer {
    /// Turns a host and port into what `Transport::connect` takes: an
    /// `IP:port` on native builds (name resolution is the caller's), a
    /// WebSocket address in the browser.
    pub endpoint: Box<EndpointFn>,
}
/// See [`TransportDialer::endpoint`].
pub type EndpointFn = dyn FnMut(&str, i32) -> String;
impl Dialer for TransportDialer {
    type Socket = TransportSocket;
    type Request = Option<crate::transport::Transport>;
    fn open(&mut self, host: &str, port: i32) -> Self::Request {
        crate::transport::Transport::connect(&(self.endpoint)(host, port)).ok()
    }
    fn status(&mut self, request: &mut Self::Request) -> Opened<TransportSocket> {
        let Some(transport) = request.as_mut() else {
            return Opened::Failed;
        };
        match transport.poll() {
            None => Opened::Pending,
            Some(crate::transport::Event::Connected) => Opened::Ready(TransportSocket {
                transport: request.take().expect("checked above"),
                buffer: Vec::new(),
                ended: false,
                failed: false,
            }),
            Some(_) => {
                *request = None;
                Opened::Failed
            }
        }
    }
}

/// [`Socket`] over a connected [`crate::transport::Transport`].
pub struct TransportSocket {
    transport: crate::transport::Transport,
    buffer: Vec<u8>,
    ended: bool,
    failed: bool,
}
impl TransportSocket {
    fn pump(&mut self) {
        while !self.ended && !self.failed {
            match self.transport.poll() {
                Some(crate::transport::Event::Data(bytes)) => self.buffer.extend_from_slice(&bytes),
                Some(crate::transport::Event::Closed) => self.ended = true,
                Some(crate::transport::Event::Error(_)) => self.failed = true,
                Some(crate::transport::Event::Connected) => {}
                None => break,
            }
        }
    }
    /// The transport and the bytes already received from it, for the
    /// in-game session to carry on with.
    pub fn into_parts(mut self) -> (crate::transport::Transport, Vec<u8>) {
        self.pump();
        (self.transport, self.buffer)
    }
}
impl Socket for TransportSocket {
    fn available(&mut self) -> io::Result<usize> {
        self.pump();
        if self.failed && self.buffer.is_empty() {
            return Err(io::ErrorKind::ConnectionReset.into());
        }
        Ok(self.buffer.len())
    }
    fn read(&mut self, out: &mut [u8]) -> io::Result<()> {
        self.pump();
        if self.buffer.len() < out.len() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        out.copy_from_slice(&self.buffer[..out.len()]);
        self.buffer.drain(..out.len());
        Ok(())
    }
    fn at_end(&mut self) -> bool {
        self.pump();
        self.ended && self.buffer.is_empty()
    }
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.transport
            .send(bytes.to_vec())
            .map_err(io::Error::other)
    }
    fn close(&mut self) {
        self.transport.close();
    }
}
