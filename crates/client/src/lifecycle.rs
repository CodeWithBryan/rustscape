//! Session lifecycle around login: `client.setGameState`, the state-40 branch
//! of `client.mainLoop`, `Protocol.handleConnectionLost`, the logout and
//! error policy of `Protocol.readPacket`, `client.resetGameSessionState`,
//! `LoginManager.processLogout` and `LoginManager.reconnect`.
//!
//! No rendering, sockets or clocks here. [`Lifecycle`] is the state machine;
//! it reaches the login code only through [`LoginSeam`] and tells its caller
//! what the original does next as a list of [`Effect`]s. [`reset_session`],
//! [`process_logout`] and [`reconnect`] are the state changes those effects
//! name, over the client state this library owns ([`Session`]); state that
//! only the application has (GPU caches, the scene) is the caller's.
//!
//! [`LoginLoop`] is `LoginManager.loop`'s attempt, retry and time-out rule
//! over an abstract connection ([`Dial`]); [`LiveDial`] puts the existing
//! [`crate::live::Connection`] behind it.
//!
//! Differences from the original:
//! - Game states 0, 5 and 28 (start-up loading and the building of the title
//!   screen's 3D scene) have no counterpart here: [`GameState::Title`] is
//!   state 10, and the application keeps 28 within it
//!   ([`crate::title_scene`]).
//! - The original reads no packets while it rebuilds the scene (state 25):
//!   `client.mainLoop` runs `LoginManager.rebuildMap` there, not
//!   `Protocol.loop`. Scene loading here does not block, so the caller
//!   decides when the rebuild is finished ([`Lifecycle::rebuild_finished`]);
//!   it holds server packets back until then
//!   ([`crate::client_events::ClientEvents::reading_cycles`]).
//! - `TracingException.report` also posts the report to `clienterror.ws` in
//!   the applet build. Only the text is produced here.
use crate::{
    camera,
    live::Connection,
    npc_updates, overheads,
    session::{Config, WorldLogin},
    ui::{font, state::State, vm::Vm},
    walking::{DestinationFlag, Movement},
};
use rs09_protocol::game::{
    Frame,
    events::ServerEvent,
    ground,
    outgoing::{map_rebuild_complete, map_rebuild_marker},
    players,
};
use std::collections::BTreeMap;

/// `client.gameState` for the states this client reaches.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameState {
    /// 10: logged out, at the title screen.
    Title,
    /// 25: logged in, rebuilding the scene ("Loading - please wait.").
    Loading,
    /// 30: in game.
    InGame,
    /// 40: the connection dropped; the scene is kept while a reconnect login
    /// runs.
    ConnectionLost,
}
impl GameState {
    /// The original's number.
    pub const fn code(self) -> i32 {
        match self {
            Self::Title => 10,
            Self::Loading => 25,
            Self::InGame => 30,
            Self::ConnectionLost => 40,
        }
    }
}

/// `LoginManager.reply` while an attempt runs.
pub const REPLY_IN_PROGRESS: i32 = -3;
/// `LoginManager.reply` before anything was asked.
pub const REPLY_NONE: i32 = -2;
/// `LoginManager.reply`: the connection failed twice.
pub const REPLY_IO_ERROR: i32 = -4;
/// `LoginManager.reply`: the attempt timed out twice.
pub const REPLY_TIMEOUT: i32 = -5;

/// One call of `LoginManager.loop`, as `client.mainLoop` reads its result
/// (`LoginManager.reply`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoginPoll {
    /// Reply -3: still in progress.
    Pending,
    /// Reply 2. The eleven player details have been read and the connection
    /// now carries game packets, the first of which is the scene rebuild.
    /// The original has run `client.resetGameSessionState` by now; here the
    /// lifecycle asks for it with [`Effect::ResetSession`].
    Entered(WorldLogin),
    /// Reply 15: a reconnect login (type 18) resumed the session.
    Reconnected,
    /// Any other reply, including -4 (connection failed) and -5 (timed out).
    Failed(i32),
}

/// The seam to the login code (`LoginManager`). The lifecycle owns the game
/// state; the login code owns the connection and the credentials.
pub trait LoginSeam {
    /// `LoginManager.clear()`: begin a login attempt with the remembered
    /// credentials on the next [`Self::cycle`]. `reconnect` is
    /// `client.gameState == 40` at the time the login block is written: the
    /// block then goes out as type 18 instead of 16.
    fn start(&mut self, reconnect: bool);
    /// `LoginManager.loop()`: one 20 ms cycle of the attempt. After a result
    /// other than [`LoginPoll::Pending`] it keeps returning nothing new:
    /// the caller acts on the first one.
    fn cycle(&mut self) -> LoginPoll;
}

/// Why the client is at the title screen.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LogoutReason {
    /// The server sent LOGOUT (opcode 86).
    Server,
    /// The connection dropped while the logout timer was running.
    LostWhileLoggingOut,
    /// The reconnect login of game state 40 ended with this reply.
    ReconnectFailed(i32),
    /// A packet the original has no handler for ("T1").
    UnknownOpcode(u8),
    /// A packet handler failed ("T2").
    HandlerError,
}

/// What the original does next, in order. The caller carries these out.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    /// `client.setGameState` changed the state.
    State { from: GameState, to: GameState },
    /// `client.resetGameSessionState()` for a login that just succeeded,
    /// with the facts of `LoginManager.loop` step 8: see [`reset_session`]
    /// and [`Vm::apply_login`]. The connection that produced them is now the
    /// game connection.
    ResetSession(WorldLogin),
    /// `ClientProt.sendWindowDetails()`: queue WINDOW_STATUS (opcode 243).
    WindowStatus,
    /// `LoginManager.processLogout()`: close the game connection, then see
    /// [`process_logout`].
    ProcessLogout(LogoutReason),
    /// `Protocol.handleConnectionLost`'s `previousSocket = socket; socket =
    /// null`: the dropped connection is set aside, and closed when the game
    /// state next changes to anything but 40.
    SetAsideConnection,
    /// `LoginManager.reconnect()` after reply 15: see [`reconnect`]. The
    /// login code's new connection is now the game connection.
    Reconnect,
    /// Write this packet to the game connection.
    Send(Frame),
    /// `TracingException.report(text, ..)`, printed by the original as
    /// `"Error: " + text`.
    Report(String),
}

/// The game state and the rules that change it.
pub struct Lifecycle {
    state: GameState,
    /// A login attempt is running in the title state (`LoginManager.step`
    /// is not 0 in game state 10).
    logging_in: bool,
    /// `LoginManager.reply`.
    pub reply: i32,
    /// `SceneGraph.centralZoneX` and `centralZoneY`; None is the -1 a login
    /// stores so that the first rebuild is never skipped.
    central_zone: Option<[i32; 2]>,
    /// `Protocol.opcode2`, `opcode3` and `opcode4`: the last three packets.
    opcodes: [i32; 3],
    /// `PlayerList.selfId` of the last login.
    pub self_id: i32,
    /// `MouseRecorder.enabled` of the last login.
    pub mouse_recorder: bool,
    /// Why the last return to the title screen happened.
    pub logout_reason: Option<LogoutReason>,
    effects: Vec<Effect>,
}
impl Default for Lifecycle {
    fn default() -> Self {
        Self::new()
    }
}
impl Lifecycle {
    /// At the title screen with no login asked for.
    pub fn new() -> Self {
        Self {
            state: GameState::Title,
            logging_in: false,
            reply: REPLY_NONE,
            central_zone: Some([0, 0]),
            opcodes: [-1; 3],
            self_id: 0,
            mouse_recorder: false,
            logout_reason: None,
            effects: Vec::new(),
        }
    }
    pub fn state(&self) -> GameState {
        self.state
    }
    /// A title-screen login attempt is in progress.
    pub fn logging_in(&self) -> bool {
        self.logging_in
    }
    /// The effects queued since the last call, oldest first.
    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }
    /// An effect is queued that ends the current game connection.
    pub fn stopping(&self) -> bool {
        self.effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::ProcessLogout(_) | Effect::SetAsideConnection
            )
        })
    }

    /// `client.setGameState`: nothing happens when the state is unchanged.
    fn set_state(&mut self, to: GameState) {
        if self.state != to {
            self.effects.push(Effect::State {
                from: self.state,
                to,
            });
            self.state = to;
        }
    }

    /// `LoginManager.processLogout()` (LoginManager.java:649-695). It ends in
    /// `InterfaceList.resetToLoginScreen(true)`, which sets game state 10
    /// (28 first when the title screen has a 3D scene).
    fn process_logout(&mut self, reason: LogoutReason) {
        self.logout_reason = Some(reason.clone());
        self.effects.push(Effect::ProcessLogout(reason));
        self.central_zone = Some([0, 0]);
        self.logging_in = false;
        self.set_state(GameState::Title);
    }

    /// The title screen asks for a login (`LoginManager.startLogin` with a
    /// world selected, which is `LoginManager.clear()`).
    pub fn start_login(&mut self, login: &mut dyn LoginSeam) {
        if self.state != GameState::Title {
            return;
        }
        self.reply = REPLY_IN_PROGRESS;
        self.logging_in = true;
        login.start(false);
    }

    /// The network part of one 20 ms `client.mainLoop` cycle in game states
    /// 10 and 40 (client.java:1729-1745). It does nothing in other states:
    /// there the caller reads game packets.
    pub fn cycle(&mut self, login: &mut dyn LoginSeam) {
        match self.state {
            GameState::Title if self.logging_in => match login.cycle() {
                LoginPoll::Pending => {}
                LoginPoll::Entered(world) => self.entered(world),
                // Reply 15 outside game state 40 is only a reply code: the
                // original stores it and the title screen shows its message.
                LoginPoll::Reconnected => self.failed_at_title(15),
                LoginPoll::Failed(reply) => self.failed_at_title(reply),
            },
            GameState::ConnectionLost => match login.cycle() {
                // `if (LoginManager.reply != -3)`
                LoginPoll::Pending => {}
                // `if (LoginManager.reply == 15) LoginManager.reconnect();`
                LoginPoll::Reconnected => {
                    self.reply = 15;
                    self.opcodes = [-1; 3];
                    self.effects.push(Effect::Reconnect);
                    self.set_state(GameState::InGame);
                    self.effects.push(Effect::WindowStatus);
                }
                // Reply 2: step 9 of the login has already reset the session
                // and read the rebuild packet; the state follows from that.
                LoginPoll::Entered(world) => self.entered(world),
                // `else if (LoginManager.reply != 2) processLogout();`
                LoginPoll::Failed(reply) => {
                    self.reply = reply;
                    self.process_logout(LogoutReason::ReconnectFailed(reply));
                }
            },
            _ => {}
        }
    }

    fn failed_at_title(&mut self, reply: i32) {
        self.reply = reply;
        self.logging_in = false;
    }

    /// `LoginManager.loop` step 9 (LoginManager.java:536-544): `reply = 2`,
    /// `client.resetGameSessionState()`, `SceneGraph.centralZoneX = -1`. The
    /// rebuild packet that follows is the caller's next
    /// [`Self::rebuild_packet`].
    fn entered(&mut self, world: WorldLogin) {
        self.reply = 2;
        self.logging_in = false;
        self.self_id = i32::from(world.player_index);
        self.mouse_recorder = world.mouse_recording;
        self.opcodes = [-1; 3];
        self.central_zone = None;
        self.logout_reason = None;
        self.effects.push(Effect::ResetSession(world));
        // The last statement of resetGameSessionState.
        self.effects.push(Effect::WindowStatus);
    }

    /// `Protocol.readPacketInternal` took a whole packet off the connection
    /// (Protocol.java:950-952): the three remembered opcodes shift.
    pub fn packet_read(&mut self, opcode: u8) {
        self.opcodes = [i32::from(opcode), self.opcodes[0], self.opcodes[1]];
    }

    /// A rebuild packet (REBUILD_NORMAL 162 or REBUILD_REGION 214) reached
    /// `LoginManager.loadRegion` (LoginManager.java:698-711): unless the zone
    /// is the one already loaded, the game state becomes 25. Returns false
    /// when the original skips the rebuild.
    ///
    /// With the hardware renderer every level is visible, so the plane is
    /// not part of the comparison; this client always renders that way.
    pub fn rebuild_packet(&mut self, zone_x: i32, zone_y: i32) -> bool {
        if self.central_zone == Some([zone_x, zone_y]) {
            return false;
        }
        self.central_zone = Some([zone_x, zone_y]);
        self.set_state(GameState::Loading);
        true
    }

    /// `client.setGameState(25)` with no rebuild packet behind it:
    /// `WorldMap.reset()` (WorldMap.java:977-981), which runs when a
    /// top-level interface is opened with reset type 2 as the world map
    /// closes (Protocol.java:1284), and the `::rebuild` command
    /// (Cheat.java:168-169). `rebuildMap` then builds the scene again from the map
    /// files it holds and ends as after a packet; the central zone is not
    /// touched. Returns whether the state changed.
    pub fn rebuild_requested(&mut self) -> bool {
        if self.state != GameState::InGame {
            return false;
        }
        self.set_state(GameState::Loading);
        true
    }

    /// The end of `LoginManager.rebuildMap` (LoginManager.java:1106-1130) in
    /// game state 25: the rebuild marker (a client in its own window sends
    /// it), game state 30, then MAP_REBUILD_COMPLETE.
    pub fn rebuild_finished(&mut self) {
        if self.state != GameState::Loading {
            return;
        }
        self.effects.push(Effect::Send(map_rebuild_marker()));
        self.set_state(GameState::InGame);
        self.effects.push(Effect::Send(map_rebuild_complete()));
    }

    /// `Protocol.handleConnectionLost()` (Protocol.java:3314-3322): a read or
    /// write failed, the ping failed, or 750 cycles passed without a packet.
    /// `logout_timer` is `Protocol.logoutOnDisconnectTimer`: 250 cycles
    /// after the logout button or an idle logout was sent, counting down.
    ///
    /// The original only calls this in game state 30: a write that fails in
    /// state 25 sets `LoginManager.pingFailed`, which `Protocol.loop` acts
    /// on in its first cycle back in state 30. It is accepted in state 25
    /// here, where a connection found closed is reported at once.
    pub fn connection_lost(&mut self, logout_timer: i32, login: &mut dyn LoginSeam) {
        if !matches!(self.state, GameState::InGame | GameState::Loading) {
            return;
        }
        if logout_timer > 0 {
            self.process_logout(LogoutReason::LostWhileLoggingOut);
        } else {
            self.effects.push(Effect::SetAsideConnection);
            // setGameState(40) runs LoginManager.clear().
            self.reply = REPLY_IN_PROGRESS;
            login.start(true);
            self.set_state(GameState::ConnectionLost);
        }
    }

    /// The server's LOGOUT packet (opcode 86, Protocol.java:2143-2146):
    /// `LoginManager.processLogout()`; no further packet is read.
    pub fn logout_packet(&mut self) {
        self.process_logout(LogoutReason::Server);
    }

    /// The end of `Protocol.readPacketInternal`'s chain (Protocol.java:
    /// 2370-2373): no handler for this opcode. Reports "T1" and logs out.
    /// `length` is the packet's payload length.
    pub fn unknown_opcode(&mut self, length: usize) {
        self.effects.push(Effect::Report(format!(
            "T1 - {},{},{} - {length}",
            self.opcodes[0], self.opcodes[1], self.opcodes[2]
        )));
        let opcode = self.opcodes[0] as u8;
        self.process_logout(LogoutReason::UnknownOpcode(opcode));
    }

    /// `Protocol.readPacket`'s catch of any exception but an `IOException`
    /// (Protocol.java:2385-2393): reports "T2" with the local player's
    /// absolute tile and the first 50 payload bytes as signed decimals, then
    /// logs out. An `IOException` is [`Self::connection_lost`] instead.
    pub fn handler_error(&mut self, payload: &[u8], self_tile: [i32; 2]) {
        let mut text = format!(
            "T2 - {},{},{} - {},{},{} - ",
            self.opcodes[0],
            self.opcodes[1],
            self.opcodes[2],
            payload.len(),
            self_tile[0],
            self_tile[1]
        );
        for &byte in payload.iter().take(50) {
            text.push_str(&format!("{},", byte as i8));
        }
        self.effects.push(Effect::Report(text));
        self.process_logout(LogoutReason::HandlerError);
    }
}

/// Carries out what the world map asked of the scene
/// ([`crate::ui::world_map_host::Host::take_scene_requests`]). Returns
/// whether `GameShell.resetTimer()` is due, which is the caller's clock.
///
/// - `reset_timer`: `GameShell.resetTimer()` after each slow step of
///   `WorldMap.load`, so that the time the step took is not caught up.
/// - `rebuild_scene`: `WorldMap.reset()` (WorldMap.java:977-981), run when
///   the top-level interface is opened with reset type 2 as the map closes
///   (Protocol.java:1284): `client.setGameState(25)`. The original then
///   shows "Loading - please wait.", builds the scene again from the map
///   files it holds, and ends `rebuildMap` as always: the rebuild marker
///   (opcode 20), game state 30, MAP_REBUILD_COMPLETE (opcode 110) and
///   `WorldMap.restorePreviousMap()`. Here the lifecycle enters state 25
///   and `events` is told to end the rebuild with those two packets.
/// - `clear_scene`: `client.clearScene()` from `WorldMap.load`
///   (WorldMap.java:126) is not carried out. It frees the scene graph and
///   the collision maps while the map covers the screen, where neither can
///   be seen or clicked (the map's interface has no game view and no
///   minimap), and the rebuild above restores both. The scene is kept
///   instead, so that rebuild has nothing to load and ends in the next
///   cycle. (`SceneGraph.buildScene` would also step the floor's colour
///   jitter once; this client steps it only when a scene is installed.)
pub fn world_map_requests(
    requests: crate::ui::world_map_host::SceneRequests,
    life: &mut Lifecycle,
    events: &mut crate::client_events::ClientEvents,
) -> bool {
    if requests.rebuild_scene > 0 && life.rebuild_requested() {
        events.rebuild_requested();
    }
    requests.reset_timer > 0
}

/// The 97 server opcodes `Protocol.readPacketInternal` has a branch for,
/// ascending (the `ServerProt` constants it compares `opcode` with). Any
/// other opcode ends in the "T1" report and a logout.
pub const ORIGINAL_SERVER_OPCODES: [u8; 97] = [
    0, 2, 4, 9, 13, 14, 16, 17, 20, 21, 22, 24, 26, 27, 32, 33, 36, 37, 38, 42, 44, 48, 50, 54, 55,
    56, 60, 62, 65, 66, 69, 70, 71, 73, 81, 84, 85, 86, 89, 97, 102, 104, 105, 111, 112, 114, 115,
    116, 119, 121, 123, 125, 126, 128, 130, 131, 132, 135, 141, 142, 144, 145, 149, 153, 154, 155,
    159, 160, 162, 164, 165, 169, 171, 172, 176, 179, 187, 191, 192, 195, 196, 197, 202, 207, 208,
    209, 214, 217, 220, 225, 226, 230, 232, 234, 235, 240, 247,
];
/// The original has a handler for this server opcode.
pub fn original_handles(opcode: u8) -> bool {
    ORIGINAL_SERVER_OPCODES.binary_search(&opcode).is_ok()
}

/// The English text `client.mainRedraw` passes to `Fonts.drawTextOnScreen`
/// (client.java:871-891): "Loading - please wait." in game state 25, with a
/// percentage on a second line while map files (0 to 50) or scenery models
/// (50 to 100) are still missing, and the two-line connection-lost message
/// in state 40. `<br>` is the original's line break. None in other states.
pub fn status_text(state: GameState, percent: Option<i32>) -> Option<String> {
    match state {
        GameState::Loading => Some(match percent {
            Some(percent) => format!("Loading - please wait.<br>({percent}%)"),
            None => "Loading - please wait.".to_owned(),
        }),
        GameState::ConnectionLost => {
            Some("Connection lost.<br>Please wait - attempting to reestablish.".to_owned())
        }
        _ => None,
    }
}

/// `Fonts.drawTextOnScreen` (Fonts.java:61-71) laid out for `p12_full`: a
/// black rectangle with a white one-pixel outline at the top left of the
/// canvas, and the text centred in it in white without a shadow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusBox {
    /// `[x, y, width, height]` of both `fillRect` and `drawRect`.
    pub rect: [i32; 4],
    /// Each line with the position of its left end on the baseline.
    pub lines: Vec<(String, [i32; 2])>,
}
impl StatusBox {
    pub const FILL: u32 = 0;
    pub const OUTLINE: u32 = 0xff_ffff;
    pub const TEXT: u32 = 0xff_ffff;
    /// `padding` is the font's `[paragraphTopPadding, paragraphBottomPadding]`.
    pub fn layout(metrics: &font::Metrics, padding: [i32; 2], text: &str) -> Self {
        let split = metrics.split(text, Some(&[250]));
        // getMaxLineWidth(text, 250) and getParagraphLineCount(text, 250) * 13
        let max_width = split
            .iter()
            .map(|line| metrics.width(line))
            .max()
            .unwrap_or(0);
        let text_height = split.len() as i32 * 13;
        // drawInterfaceText(text, 10, 10, maxWidth, textHeight, 0xffffff, -1, 1, 1, 0)
        let lines = metrics
            .paragraph(text, [10, 10, max_width, text_height], [1, 1], 0, padding)
            .into_iter()
            .map(|line| (line.text, line.position))
            .collect();
        Self {
            rect: [6, 6, max_width + 8, text_height + 8],
            lines,
        }
    }
}

/// The six values `client.resetGameSessionState` draws from `Math.random()`
/// at each login, in the order it draws them. They perturb the camera and
/// the minimap so that no two sessions look alike to a bot.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Jitter {
    /// `Camera.cameraOffsetX`: -50..=49.
    pub camera_offset_x: i32,
    /// `Camera.yawTarget`: -10..=9, masked to 0..=2047.
    pub yaw_target: i32,
    /// `Camera.cameraOffsetY`: -55..=54.
    pub camera_offset_y: i32,
    /// `MiniMap.zoomOffset`: -20..=9.
    pub zoom_offset: i32,
    /// `MiniMap.compassAngleOffset`: -60..=59.
    pub compass_angle_offset: i32,
    /// `Camera.yawDrift`: -40..=39.
    pub yaw_drift: i32,
}
impl Jitter {
    /// `random` is `Math.random()`: uniform in `[0, 1)`. The six draws are
    /// [`crate::minimap::Drift::reset`], the port of these statements that
    /// is compared with the original on a JVM.
    pub fn draw(random: impl FnMut() -> f64) -> Self {
        let mut drift = crate::minimap::Drift::default();
        let yaw_target = drift.reset(random);
        Self {
            camera_offset_x: drift.camera_offset[0],
            yaw_target,
            camera_offset_y: drift.camera_offset[1],
            zoom_offset: drift.zoom_offset,
            compass_angle_offset: drift.angle_offset,
            yaw_drift: drift.yaw_drift,
        }
    }
}

/// The client state of a game session that this library defines. The
/// application adds what only it has (see the module documentation).
pub struct Session<'a> {
    pub vm: &'a mut Vm,
    /// None when the interface assets are unavailable.
    pub ui: Option<&'a mut State>,
    /// `PlayerList.players`, `PlayerList.appearanceCache` and
    /// `PlayerList.self`.
    pub players: &'a mut players::State,
    /// `NpcList.npcs`.
    pub npcs: &'a mut npc_updates::State,
    /// `SceneGraph.objStacks`.
    pub ground: &'a mut ground::State,
    /// `SceneGraph.spotanims` and `SceneGraph.projectiles`.
    pub world_spots: &'a mut crate::world_spot::State,
    /// The scripted camera (`Camera.cameraType`, `locked*`, shake).
    pub camera: &'a mut camera::Script,
    /// `LoginManager.mapFlagX` and `mapFlagY`.
    pub map_flag: &'a mut DestinationFlag,
    /// The local player's movement (`PlayerList.self`).
    pub motion: &'a mut Movement,
    /// The other players' movement.
    pub actors: &'a mut BTreeMap<usize, Movement>,
    /// Overhead chat, hit splats and health bars, `(is_npc, index)`.
    pub overheads: &'a mut BTreeMap<(bool, usize), overheads::State>,
    /// `Player.usernameInput`.
    pub username: &'a str,
}

/// `client.resetGameSessionState()` (client.java:631-755), run when a login
/// succeeds and before its rebuild packet is read.
///
/// Statement by statement, in the original's order:
///
/// | original | here |
/// |---|---|
/// | `Protocol.mouseIdleSamples`, `prevFocus`, `Mouse.prevClickTime`, `MouseRecorder.instance.samples`, `focus`, `ReflectionCheck.clear()`, `Mouse.setIdleLoops(0)`, `Protocol.cameraPositionChanged` | no state: this client does not report input, focus or the camera yet |
/// | `Protocol.opcode`, `opcode2`..`opcode4` | [`Lifecycle`] |
/// | `Protocol.logoutOnDisconnectTimer`, `Player.rebootTimer` | `Vm` |
/// | `Protocol.outboundBuffer.offset = 0` | `Vm::outgoing`; the application's own queue is its to clear |
/// | `Protocol.inboundBuffer.offset`, `LoginManager.ticksSinceLastPacket` | the new connection starts with both at 0 |
/// | `MiniMap.hintMapMarkers`, `MiniMap.state` | `Vm::minimap` |
/// | `MiniMenu.size`, `Cs1ScriptRunner.isMenuOpen` | the application's context menu |
/// | `Chat.messages`, `Chat.size` | `Vm::messages`, `Vm::chat_lines` |
/// | `MiniMenu.itemTargetMode`, `isTargeting` | `State::selected_item`, `State::targeting` |
/// | `Camera.cameraOffsetX/Y`, `yawTarget`, `yawDrift`, `MiniMap.zoomOffset`, `compassAngleOffset` | returned as [`Jitter`], for [`crate::session_drift::reset`] |
/// | `LoginManager.mapFlagX`, `mapFlagY` | `map_flag` |
/// | `LightingManager.minimapRenderedPlane = -1` | the application's minimap cache |
/// | `PlayerList.size`, `players`, `appearanceCache`, `self` | `players`, `actors`, `motion`; the application's model caches |
/// | `SoundPlayer.size` | `Vm::effects`; the application's sound queue |
/// | `NpcList.size`, `npcs` | `npcs` |
/// | `SceneGraph.projectiles`, `spotanims` | `world_spots` (the zone origin is kept, as the original keeps `currentChunkX/Y`) |
/// | `SceneGraph.objStacks` | `ground` |
/// | `ChangeLocRequest.queue` | the application's scene |
/// | `FriendsList.state`, `size`; `ClanChat.size`, `members`, `name`; `StockMarketManager.offers` | `Vm::social` |
/// | `VarpDomain.reset()`, `VarcDomain.varcs` | `Vm` |
/// | `DelayedStateChange.clear()` | `State::delayed` |
/// | `Camera.locked*` (ten fields) | `camera`, keeping its mode and shakes as the original does |
/// | close every interface, `InterfaceList.reset()`, `pleaseWaitComponent` | `State` |
/// | `PlayerAppearance.DEFAULT.set(..)` | no state: no default appearance is kept |
/// | `Player.options`, `secondaryOptions`, `cursors` | `Vm::player_options` |
/// | `Inv.clear()` | `State::containers` |
/// | `ScriptRunner.loadingScene = true` | no state: it waits for cache downloads, and this client reads a complete cache |
/// | `InterfaceList.rectangleDirty` | no state: every frame is drawn whole |
/// | `PlayerSkillXpTable` arrays | `Vm` |
/// | `FogManager.setInstantFade()` | the application's atmosphere |
/// | `Protocol.verifyId = 0` | `State::verify_id` |
/// | `MiniMenu.walkText`, `ScriptRunner.neverRemoveRoofs`, `InterfaceList.useStyledMenu` | `Vm` |
/// | the four recolour palettes | no state |
/// | `LoginManager.clearLoginScreenSprites()` | no state: there is no title screen here |
/// | `ClientProt.sendWindowDetails()` | [`Effect::WindowStatus`] |
///
/// `random` is `Math.random()`.
pub fn reset_session(session: Session<'_>, random: impl FnMut() -> f64) -> Jitter {
    let Session {
        vm,
        mut ui,
        players,
        npcs,
        ground,
        world_spots,
        camera,
        map_flag,
        motion,
        actors,
        overheads,
        username,
    } = session;
    vm.reset_session(ui.as_deref(), username);
    if let Some(ui) = ui.as_deref_mut() {
        ui.selected_item = None;
        ui.targeting.active = false;
    }
    let jitter = Jitter::draw(random);
    map_flag.tile = [0, 0];
    actors.clear();
    overheads.clear();
    *npcs = npc_updates::State::default();
    // PlayerList.self = PlayerList.players[2047] = new Player()
    *players = players::State::new(0, 0);
    *motion = Movement::new([0, 0]);
    world_spots.projectiles.clear();
    world_spots.effects.clear();
    *ground = ground::State::default();
    reset_locked_camera(camera);
    if let Some(ui) = ui {
        // DelayedStateChange.clear() comes before the interfaces close;
        // pleaseWaitComponent, Inv.clear() and verifyId = 0 come after.
        ui.delayed = crate::ui::delayed::Queue::default();
        ui.close_everything(|ui| vm.drain(ui));
        ui.reset_session_statics();
    }
    jitter
}

/// The ten `Camera.locked*` assignments of `client.resetGameSessionState`:
/// the locked camera's targets, speeds and steps become 0. The camera mode
/// (`cameraType`) and the shakes are left as they are.
fn reset_locked_camera(camera: &mut camera::Script) {
    let mut fresh = camera::Script::default();
    fresh.spline = camera.spline.take();
    fresh.locked = camera.locked;
    fresh.render = camera.render;
    fresh.pitch = camera.pitch;
    fresh.yaw = camera.yaw;
    fresh.shakes = camera.shakes;
    *camera = fresh;
}

/// `LoginManager.processLogout()` (LoginManager.java:649-695) after the
/// connection is closed, with the `InterfaceList.resetToLoginScreen(true)`
/// it ends in.
///
/// | original | here |
/// |---|---|
/// | `client.unload()`, `System.gc()` | no state: definition caches stay valid |
/// | `SceneGraph.clear()`, `PathFinder.collisionMaps[i].clear()`, `WorldMap.clear(false)` | the application's scene, navigation and world map |
/// | `MidiPlayer.playFadeOut()`, `MidiPlayer.jingle`, `MusicPlayer.groupId` | the application's music player (`Music::reset`) |
/// | `AreaSoundManager.clear(true)` | the application's ambient sounds |
/// | `SceneGraph.dynamicMapRegion`, `Camera.originX/Y`, `SceneGraph.centralZoneX/Y` | [`Lifecycle`] keeps the central zone; the application its scene base |
/// | `MiniMap.hintMapMarkers` | `Vm::minimap` |
/// | `PlayerList`, `NpcList`, `SceneGraph.objStacks` | `players`, `actors`, `npcs`, `ground` |
/// | `Camera.resetCameraEffects()` | `camera` |
/// | `Protocol.verifyId = 0`, `VarpDomain.reset()` | `State`, `Vm` |
/// | close every interface, `InterfaceList.reset()` | `State` |
/// | `topLevelInterface = loginScreenId` and its load scripts | the application's: it opens the title interface after this |
/// | `MiniMenu.defaultCursorId = -1`, `setCursor(scriptCursorId)` | `State::targeting` |
/// | `PlayerList.self = new Player()` at fine position 3000, 3000 | `players`, `motion` |
/// | `Flames.load`, `client.setGameState(10)` | [`Lifecycle`] |
///
/// Unlike a login, a logout keeps the chat lines, friends, clan, skills,
/// inventories and client variables in memory; the next login clears them.
pub fn process_logout(session: Session<'_>) {
    let Session {
        vm,
        ui,
        players,
        npcs,
        ground,
        world_spots: _,
        camera,
        map_flag: _,
        motion,
        actors,
        overheads,
        username,
    } = session;
    actors.clear();
    overheads.clear();
    *npcs = npc_updates::State::default();
    *players = players::State::new(0, 0);
    *ground = ground::State::default();
    // Camera.resetCameraEffects(): shakes off, both steps 0, camera type 1.
    let _ = camera.apply(&ServerEvent::CameraReset { counter: 0 }, |_| 0);
    vm.process_logout(ui.as_deref(), username);
    *motion = Movement::new([0, 0]);
    motion.fine = [3000, 3000];
    if let Some(ui) = ui {
        ui.verify_id = 0;
        ui.close_everything(|ui| vm.drain(ui));
        ui.targeting.default_cursor = -1;
    }
}

/// `LoginManager.reconnect()` (LoginManager.java:830-862) after reply 15:
/// the scene, the players and the interfaces stay.
///
/// | original | here |
/// |---|---|
/// | `Protocol.outboundBuffer.offset = 0` | `Vm::outgoing`; the application's own queue |
/// | `Protocol.opcode`..`opcode4`, `Protocol.length`, `inboundBuffer.offset`, `ticksSinceLastPacket` | [`Lifecycle`] and the new connection |
/// | `Cs1ScriptRunner.isMenuOpen`, `MiniMenu.size` | the application's context menu |
/// | `mapFlagX = 0` | `map_flag` (`mapFlagY` is kept) |
/// | `MiniMap.state`, `Player.rebootTimer` | `Vm` |
/// | `faceEntity = -1` for every player and NPC | `motion`, `actors`, `npcs` |
/// | `Inv.clear()` | `State::containers` |
/// | `Camera.cameraType = 1` | `camera` |
/// | `client.setGameState(30)` | [`Lifecycle`] |
/// | `InterfaceList.rectangleDirty` | no state |
/// | `ClientProt.sendWindowDetails()` | [`Effect::WindowStatus`] |
pub fn reconnect(session: Session<'_>) {
    let Session {
        vm,
        ui,
        npcs,
        camera,
        map_flag,
        motion,
        actors,
        ..
    } = session;
    vm.reconnect();
    map_flag.tile[0] = 0;
    motion.face_entity = -1;
    for actor in actors.values_mut() {
        actor.face_entity = -1;
    }
    for npc in npcs.actors.values_mut() {
        npc.motion.face_entity = -1;
    }
    if let Some(ui) = ui {
        ui.containers.clear();
    }
    camera.locked = false;
    camera.spline = None;
}

/// One connection attempt of `LoginManager.loop`, steps 2 to 10.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Attempt {
    /// Nothing decisive has arrived.
    Pending,
    /// Reply 2 with the player details and the first packet.
    Entered(WorldLogin),
    /// Reply 15.
    Reconnected,
    /// Any other reply code; the connection is closed.
    Reply(i32),
    /// The original's `IOException`: the connection could not be opened, or
    /// failed or ended before a reply.
    Io,
}

/// The connection behind [`LoginLoop`]: `SignLink.openSocket` and the reads
/// and writes of `LoginManager.loop`.
pub trait Dial {
    /// `LoginManager.loop` step 1: open a connection and begin the login
    /// exchange on it. An error is the original's `IOException`.
    fn open(&mut self, reconnect: bool) -> Result<(), String>;
    /// One 20 ms cycle of the open attempt.
    fn poll(&mut self) -> Attempt;
    /// `Protocol.socket.close()` on an attempt that is being abandoned.
    fn close(&mut self);
}

/// `LoginManager.loop`'s rule around one attempt (LoginManager.java:272-312,
/// 440-463, 545-562): 2000 cycles per attempt, one retry after a time-out,
/// a failed connection or reply 23, then reply -5 or -4.
///
/// The original alternates between its default and alternate port for the
/// retry, unless a JSON configuration fixes the address, in which case both
/// attempts go to the same one. A [`Dial`] has one address, as in that case.
pub struct LoginLoop<D: Dial> {
    pub dial: D,
    /// 0 idle, 1 open a connection, 2 exchange in progress.
    step: i32,
    loops: i32,
    errors: i32,
    reconnect: bool,
}
impl<D: Dial> LoginLoop<D> {
    pub fn new(dial: D) -> Self {
        Self {
            dial,
            step: 0,
            loops: 0,
            errors: 0,
            reconnect: false,
        }
    }
    /// An attempt is running.
    pub fn active(&self) -> bool {
        self.step != 0
    }
    /// The catch block: retry once, then reply -4.
    fn io_error(&mut self) -> LoginPoll {
        self.dial.close();
        if self.errors >= 1 {
            self.step = 0;
            LoginPoll::Failed(REPLY_IO_ERROR)
        } else {
            self.step = 1;
            self.loops = 0;
            self.errors += 1;
            LoginPoll::Pending
        }
    }
}
impl<D: Dial> LoginSeam for LoginLoop<D> {
    fn start(&mut self, reconnect: bool) {
        self.dial.close();
        self.reconnect = reconnect;
        self.loops = 0;
        self.step = 1;
        self.errors = 0;
    }
    fn cycle(&mut self) -> LoginPoll {
        if self.step == 0 {
            return LoginPoll::Pending;
        }
        self.loops += 1;
        if self.loops > 2000 {
            self.dial.close();
            if self.errors >= 1 {
                self.step = 0;
                return LoginPoll::Failed(REPLY_TIMEOUT);
            }
            self.loops = 0;
            self.step = 1;
            self.errors += 1;
        }
        if self.step == 1 {
            if self.dial.open(self.reconnect).is_err() {
                return self.io_error();
            }
            self.step = 2;
        }
        match self.dial.poll() {
            Attempt::Pending => LoginPoll::Pending,
            Attempt::Entered(world) => {
                self.step = 0;
                LoginPoll::Entered(world)
            }
            Attempt::Reconnected => {
                self.step = 0;
                LoginPoll::Reconnected
            }
            // `reply == 23 && errors < 1`: the server asks for another try.
            Attempt::Reply(23) if self.errors < 1 => {
                self.dial.close();
                self.step = 1;
                self.errors += 1;
                self.loops = 0;
                LoginPoll::Pending
            }
            Attempt::Reply(reply) => {
                self.dial.close();
                self.step = 0;
                LoginPoll::Failed(reply)
            }
            Attempt::Io => self.io_error(),
        }
    }
}

/// `Player.usernameInput`, `Player.password` and the address, kept for the
/// whole run as the original keeps them: game state 40 logs in again with
/// them. There is deliberately no `Debug`; the password is zeroed on drop.
pub struct Credentials {
    pub endpoint: String,
    pub username: Vec<u8>,
    password: Vec<u8>,
}
impl Credentials {
    pub fn new(endpoint: String, username: Vec<u8>, password: Vec<u8>) -> Self {
        Self {
            endpoint,
            username,
            password,
        }
    }
}
impl Drop for Credentials {
    fn drop(&mut self) {
        self.password.fill(0);
    }
}

/// [`Dial`] over the existing connection code: [`Connection`] with
/// [`crate::session::Session`] and [`crate::transport::Transport`].
pub struct LiveDial {
    credentials: Credentials,
    /// `GameShell.canvasWidth` and `canvasHeight` for the login block.
    pub size: [i32; 2],
    /// `Protocol.verifyId` for the login block.
    pub verify_id: i32,
    /// The CRC of each archive index for the login block.
    pub crc_sums: [i32; 29],
    /// `(int) (Math.random() * 9.9999999E7)`: the client half of the cipher
    /// seed, drawn twice per attempt.
    seed: Box<dyn FnMut() -> i32>,
    attempt: Option<Connection>,
    /// The connection of a finished login, for the caller to take.
    established: Option<Connection>,
}
impl LiveDial {
    pub fn new(credentials: Credentials, seed: Box<dyn FnMut() -> i32>) -> Self {
        Self {
            credentials,
            size: [765, 503],
            verify_id: 0,
            crc_sums: [0; 29],
            seed,
            attempt: None,
            established: None,
        }
    }
    pub fn username(&self) -> &[u8] {
        &self.credentials.username
    }
    /// What the attempt is doing, for a status line.
    pub fn status(&self) -> Option<&str> {
        self.attempt.as_ref().map(|c| c.status.as_str())
    }
    /// The game connection after [`Attempt::Entered`] or
    /// [`Attempt::Reconnected`]. Frames that arrived with the reply are
    /// returned by its next `poll`.
    pub fn take_connection(&mut self) -> Option<Connection> {
        self.established.take()
    }
}
impl Dial for LiveDial {
    fn open(&mut self, reconnect: bool) -> Result<(), String> {
        // LoginManager.java:351-352 draws key[1] before key[0].
        let second = (self.seed)();
        let first = (self.seed)();
        let mut config = Config::new(
            self.credentials.username.clone(),
            self.credentials.password.clone(),
            [first, second],
        );
        config.width = self.size[0];
        config.height = self.size[1];
        config.reconnect = reconnect;
        config.verify_id = self.verify_id;
        config.crc_sums = self.crc_sums;
        self.attempt = Some(Connection::connect(&self.credentials.endpoint, config)?);
        Ok(())
    }
    fn poll(&mut self) -> Attempt {
        let Some(connection) = &mut self.attempt else {
            return Attempt::Io;
        };
        match connection.poll() {
            Ok(frames) => connection.unread(frames),
            Err(_) => return Attempt::Io,
        }
        if connection.tick().is_err() {
            return Attempt::Io;
        }
        if let Some(world) = connection.entered.take() {
            self.established = self.attempt.take();
            return Attempt::Entered(world);
        }
        if connection.reconnected {
            self.established = self.attempt.take();
            return Attempt::Reconnected;
        }
        if let Some((code, _)) = connection.rejected {
            self.attempt = None;
            return Attempt::Reply(i32::from(code));
        }
        if connection.closed() {
            self.attempt = None;
            return Attempt::Io;
        }
        Attempt::Pending
    }
    fn close(&mut self) {
        if let Some(mut connection) = self.attempt.take() {
            connection.stop("Login attempt abandoned");
        }
    }
}
