//! Client-to-server game packets of the revision 530 client that no other
//! part of this tree encodes, with the client-side packing they need.
//!
//! The original writes every outgoing game packet as
//! `Protocol.outboundBuffer.p1isaac(OPCODE)` followed by `Buffer` writes. It
//! does so for exactly the 100 opcodes in [`ORIGINAL_CLIENT_OPCODES`]. Most of
//! them are already encoded elsewhere, mostly in `crates/client` (menu
//! actions, buttons, walking, chat, social lists, dialogue input). This
//! module holds the remaining ones:
//!
//! | opcode | function | pinned server |
//! |---|---|---|
//! | 17 | [`continue_delayed_login`] | size 0, no decoder |
//! | 20 | [`map_rebuild_marker`] | decoded, never processed |
//! | 21 | [`camera_position`] | decoded, ignored |
//! | 22 | [`focus`] | decoded, ignored |
//! | 75 | [`mouse_click`] | decoded, ignored |
//! | 98 | [`preferences`] | decoded, ignored |
//! | 109 | [`ground_item_option2`] | size 6, no decoder |
//! | 110 | [`map_rebuild_complete`] | decoded, never processed |
//! | 123 | [`MouseRecorder::flush`] | variable byte, no decoder |
//! | 163 | [`reflection_reply`] | no declared size |
//! | 177 | [`verify_id`] | adjusts the interface packet counter |
//! | 179 | [`face_tile`] | size 4, no decoder |
//! | 243 | [`window_status`] | switches the window mode |
//! | 245 | [`idle_logout`] | logs the player out |
//!
//! Field order, widths and byte transforms follow the Java write sequences
//! named on each function. A [`Frame`] holds the payload only: the transport
//! adds the opcode (with ISAAC when enabled) and, for variable packets, the
//! size byte the original writes with `p1(0)` ... `psize1`.
//!
//! The small state helpers ([`MouseRecorder`], [`CameraReport`],
//! [`mouse_click_report`], [`focus_report`], [`idle_logout_report`]) are the
//! conditions around those writes in `Protocol.loop`, so a client loop can
//! call them once per 20 ms cycle in the original order: reflection replies,
//! mouse movement, mouse click, camera, focus, preferences, then (at the end
//! of the cycle) the idle logout, the keep-alive and the verification id.
use crate::{Error, Result, buffer::Writer, checksum::crc32, game::Frame};

/// Every opcode the original client passes to `p1isaac`, ascending. Found by
/// searching the whole `rt4` package; `ClientProt` names are resolved.
///
/// The pinned server's `GameReadEvent.PACKET_SIZES` declares a size for each
/// of them except 163, and for no other opcode.
pub const ORIGINAL_CLIENT_OPCODES: [u8; 100] = [
    3, 4, 6, 9, 10, 17, 20, 21, 22, 23, 27, 30, 33, 34, 39, 44, 48, 53, 55, 57, 64, 65, 66, 68, 71,
    72, 73, 75, 77, 78, 79, 81, 82, 84, 85, 92, 93, 94, 98, 99, 101, 104, 106, 109, 110, 111, 114,
    115, 117, 120, 123, 124, 131, 132, 133, 134, 135, 137, 148, 153, 154, 155, 156, 157, 161, 162,
    163, 166, 167, 168, 170, 175, 177, 178, 179, 180, 184, 188, 194, 195, 196, 199, 201, 206, 213,
    215, 218, 228, 231, 233, 234, 237, 239, 243, 244, 245, 247, 248, 253, 254,
];

fn frame(opcode: u8, w: Writer) -> Frame {
    Frame {
        opcode,
        payload: w.into_vec(),
    }
}

// ---------------------------------------------------------------------------
// Input and idle reporting
// ---------------------------------------------------------------------------

/// EVENT_MOUSE_CLICK, opcode 75 (`Protocol.loop`, Protocol.java:2526-2528).
///
/// Layout, 6 bytes: `ip2add(button << 15 | elapsed)`, `mp4(x | y << 16)`.
/// `right_button` is the original's `Mouse.clickButton == 2`; `elapsed` is the
/// time since the previous click in 50 ms units, at most 32767; `x` and `y`
/// are the click position clamped to `0..=65535`. Use [`mouse_click_report`]
/// to derive these from raw click state as the original does.
///
/// The server's decoder labels the high half of the position `x` and the low
/// half `y`, the reverse of what the client packs. It ignores both.
pub fn mouse_click(right_button: bool, elapsed: i32, x: i32, y: i32) -> Frame {
    let mut w = Writer::new();
    w.ip2add((i32::from(right_button) << 15) | elapsed);
    w.mp4(x | (y << 16));
    frame(75, w)
}

/// The click report of `Protocol.loop` (Protocol.java:2503-2529).
///
/// `click_button` is `Mouse.clickButton` for this cycle (0 none, 1 left,
/// 2 right), `click_x`/`click_y` the click position, `click_time` its
/// timestamp in milliseconds and `prev_click_time` the original's
/// `Mouse.prevClickTime`, which starts at 0 for each game session and is
/// updated here. Nothing is sent for button 0.
pub fn mouse_click_report(
    click_button: i32,
    click_x: i32,
    click_y: i32,
    click_time: i64,
    prev_click_time: &mut i64,
) -> Option<Frame> {
    if click_button == 0 {
        return None;
    }
    let mut loops = click_time.wrapping_sub(*prev_click_time) / 50;
    let y = click_y.clamp(0, 65535);
    if loops > 32767 {
        loops = 32767;
    }
    *prev_click_time = click_time;
    let x = click_x.clamp(0, 65535);
    Some(mouse_click(click_button == 2, loops as i32, x, y))
}

/// The mouse recorder and its EVENT_MOUSE_MOVE packet, opcode 123
/// (`MouseRecorder.run` and `Protocol.loop`, Protocol.java:2426-2502).
///
/// A separate thread samples the mouse position every 50 ms into a buffer of
/// 500 positions ([`MouseRecorder::sample`]). Each game cycle the client sends
/// what was recorded when a click happened or 40 samples are waiting
/// ([`MouseRecorder::flush`]).
///
/// The pinned server sends 0 for the recorder flag in its login reply, so a
/// client that honours the flag never sends this packet to it.
#[derive(Clone, Debug, Default)]
pub struct MouseRecorder {
    /// `MouseRecorder.enabled`: byte 6 of the 14-byte login reply, `== 1`.
    pub enabled: bool,
    samples: Vec<[i32; 2]>,
    /// `MouseRecorder.mouseRecorderPrevX/Y`: the last position reported, or
    /// `[-1, -1]` once the mouse was reported outside the window. Starts at
    /// `[0, 0]`; the original never resets it, not even between sessions.
    pub prev: [i32; 2],
    /// `Protocol.mouseIdleSamples`: samples since the last reported movement,
    /// at most 2047.
    pub idle_samples: i32,
}

impl MouseRecorder {
    /// The original sample arrays hold 500 positions; further samples are lost.
    pub const CAPACITY: usize = 500;

    /// `MouseRecorder.run`, once every 50 ms: record the current mouse
    /// position (`Mouse.lastMouseX/Y`; `-1, -1` while the mouse is outside
    /// the window). The original samples whether or not recording is enabled.
    pub fn sample(&mut self, x: i32, y: i32) {
        if self.samples.len() < Self::CAPACITY {
            self.samples.push([x, y]);
        }
    }

    /// Samples waiting to be sent.
    pub fn pending(&self) -> usize {
        self.samples.len()
    }

    /// `client.resetGameSessionState`: a new game session clears the waiting
    /// samples and the idle count. `prev` and `enabled` are left alone (the
    /// login reply sets `enabled` just before this runs).
    pub fn reset_session(&mut self) {
        self.idle_samples = 0;
        self.samples.clear();
    }

    /// One game cycle of `Protocol.loop`. `click_pending` is
    /// `Mouse.clickButton != 0` for this cycle.
    ///
    /// When disabled, waiting samples are discarded. Otherwise, on a click or
    /// with at least 40 samples waiting, samples are packed in order until
    /// all are consumed or the payload has reached 240 bytes; the rest stay
    /// for the next cycle. The payload can be empty (every sample idle).
    ///
    /// Per sample, with `dx`/`dy` the change from the previous report and
    /// `idle` the idle samples before it:
    /// - unchanged position: nothing written, `idle` grows (up to 2047);
    /// - `idle < 8`, both deltas in `-32..=31`: `p2((dy + 32) + (idle << 12) + ((dx + 32) << 6))`;
    /// - `idle < 32`, both deltas in `-128..=127`: `p1(idle + 128)`, `p2(((dx + 128) << 8) + (dy + 128))`;
    /// - `idle < 32` otherwise: `p1(idle + 192)`, `p4(position)`;
    /// - otherwise: `p2(idle + 57344)`, `p4(position)`;
    ///
    /// where `position` is `x | y << 16` with both clamped to `0..=65534`, or
    /// `i32::MIN` for the outside-window sample `(-1, -1)`.
    pub fn flush(&mut self, click_pending: bool) -> Option<Frame> {
        if !self.enabled {
            self.samples.clear();
            return None;
        }
        if !click_pending && self.samples.len() < 40 {
            return None;
        }
        let mut w = Writer::new();
        let mut consumed = 0;
        while consumed < self.samples.len() && w.as_slice().len() < 240 {
            let [sample_x, sample_y] = self.samples[consumed];
            consumed += 1;
            let mut y = sample_y.clamp(0, 65534);
            let mut x = sample_x.clamp(0, 65534);
            let outside_window = sample_y == -1 && sample_x == -1;
            if outside_window {
                y = -1;
                x = -1;
            }
            if self.prev[0] != x || y != self.prev[1] {
                let dx = x.wrapping_sub(self.prev[0]);
                self.prev[0] = x;
                let dy = y.wrapping_sub(self.prev[1]);
                self.prev[1] = y;
                let position = if outside_window {
                    i32::MIN
                } else {
                    x | (y << 16)
                };
                let idle = self.idle_samples;
                if idle < 8 && (-32..=31).contains(&dx) && (-32..=31).contains(&dy) {
                    w.p2((dy + 32) + (idle << 12) + ((dx + 32) << 6));
                } else if idle < 32 && (-128..=127).contains(&dx) && (-128..=127).contains(&dy) {
                    w.p1(idle + 128);
                    w.p2(((dx + 128) << 8) + (dy + 128));
                } else if idle < 32 {
                    w.p1(idle + 192);
                    w.p4(position);
                } else {
                    w.p2(idle + 57344);
                    w.p4(position);
                }
                self.idle_samples = 0;
            } else if self.idle_samples < 2047 {
                self.idle_samples += 1;
            }
        }
        self.samples.drain(..consumed);
        Some(frame(123, w))
    }
}

/// EVENT_CAMERA_POSITION, opcode 21 (`Protocol.loop`, Protocol.java:2547-2549).
///
/// Layout, 4 bytes: `p2add(pitch)`, `ip2(yaw)`, from `(int) Camera.pitchTarget`
/// and `(int) Camera.yawTarget` (Java's float-to-int cast, which Rust's `as
/// i32` matches). See [`CameraReport`] for when it is sent.
pub fn camera_position(pitch: i32, yaw: i32) -> Frame {
    let mut w = Writer::new();
    w.p2add(pitch);
    w.ip2(yaw);
    frame(21, w)
}

/// When `Protocol.loop` reports the camera (Protocol.java:2530-2550).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CameraReport {
    /// `Protocol.cameraPositionChanged`: true at start-up and again at the
    /// start of every game session (`client.resetGameSessionState`).
    pub changed: bool,
    /// `Protocol.cameraSendCooldown`, in cycles.
    pub cooldown: i32,
}

impl Default for CameraReport {
    fn default() -> Self {
        Self {
            changed: true,
            cooldown: 0,
        }
    }
}

impl CameraReport {
    /// One game cycle; true when [`camera_position`] should be sent now with
    /// the current pitch and yaw targets.
    ///
    /// `camera_keys` is the original's change test for this cycle: with
    /// `Preferences.keyboardCameraEnabled` set, whether any key event queued
    /// this cycle has key code 96, 97, 98 or 99 (the arrow keys); with it
    /// clear, whether an arrow key is currently held. No other camera
    /// movement marks a change. At most one report is sent per 20 cycles.
    pub fn step(&mut self, camera_keys: bool) -> bool {
        if self.cooldown > 0 {
            self.cooldown -= 1;
        }
        if camera_keys {
            self.changed = true;
        }
        if self.changed && self.cooldown <= 0 {
            self.cooldown = 20;
            self.changed = false;
            return true;
        }
        false
    }
}

/// EVENT_APPLET_FOCUS, opcode 22 (`Protocol.loop`, Protocol.java:2553-2559).
///
/// Layout, 1 byte: `p1(1)` when the window gained focus, `p1(0)` when it lost
/// it.
pub fn focus(focused: bool) -> Frame {
    let mut w = Writer::new();
    w.p1(i32::from(focused));
    frame(22, w)
}

/// The focus test of `Protocol.loop`: a packet on each change of
/// `GameShell.focus`. `prev_focus` is `Protocol.prevFocus`, true at the start
/// of every game session.
pub fn focus_report(prev_focus: &mut bool, focused: bool) -> Option<Frame> {
    if focused == *prev_focus {
        return None;
    }
    *prev_focus = focused;
    Some(focus(focused))
}

/// Opcode 245 (`Protocol.loop`, Protocol.java:2910): the player has been idle
/// too long. No payload. The pinned server logs out every player below
/// administrator rights when it receives this.
pub fn idle_logout() -> Frame {
    frame(245, Writer::new())
}

/// The idle test of `Protocol.loop` (Protocol.java:2905-2911), late in each
/// game cycle: when both `Mouse.getIdleLoops()` and `Keyboard.getIdleLoops()`
/// exceed 15000 cycles (five minutes), the original sets
/// `logoutOnDisconnectTimer` to 250, puts the mouse idle count back to 14500
/// and sends [`idle_logout`]. Each idle count grows by one per cycle and is
/// zeroed by any event of its device.
pub fn idle_logout_report(
    mouse_idle_loops: &mut i32,
    keyboard_idle_loops: i32,
    logout_on_disconnect_timer: &mut i32,
) -> Option<Frame> {
    if *mouse_idle_loops > 15000 && keyboard_idle_loops > 15000 {
        *logout_on_disconnect_timer = 250;
        *mouse_idle_loops = 14500;
        return Some(idle_logout());
    }
    None
}

/// Opcode 98 (`Protocol.loop`, Protocol.java:2562-2563): the packed graphics
/// and sound preferences, `p4(Preferences.toInt())`. Sent in the first cycle
/// in which `Preferences.sentToServer` is false, which is then set. The login
/// block carries the same value and sets the flag too. See
/// [`PreferenceFlags::to_int`].
pub fn preferences(packed: i32) -> Frame {
    let mut w = Writer::new();
    w.p4(packed);
    frame(98, w)
}

/// The inputs of `Preferences.toInt` (Preferences.java:326).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PreferenceFlags {
    pub brightness: i32,
    pub all_levels_visible: bool,
    pub remove_roofs_selectively: bool,
    pub show_ground_decorations: bool,
    pub high_detail_textures: bool,
    pub many_idle_animations: bool,
    pub flickering_effects: bool,
    pub many_ground_textures: bool,
    pub character_shadows: bool,
    pub scenery_shadows: i32,
    pub high_detail_lighting: bool,
    pub high_water_detail: bool,
    pub fog: bool,
    pub stereo: bool,
    pub sound_volume: i32,
    pub music_volume: i32,
    pub ambient_volume: i32,
    pub particles: i32,
}

impl PreferenceFlags {
    /// `Preferences.toInt`. Bits: 0-2 brightness, 3 all levels visible, 4
    /// selective roof removal, 5 ground decorations, 6 high-detail textures,
    /// 7 idle animations, 8 flickering effects, 9 ground textures, 10
    /// character shadows, 11-12 scenery shadows, 13 high-detail lighting, 15
    /// high-detail water, 16 fog, 19 stereo, 20 sound effects on, 21 music
    /// on, 22 ambient sounds on, 23 and up the particle setting.
    ///
    /// The original is one sum whose negations cancel. Like it, this adds the
    /// particle setting unmasked, so a setting outside `0..=255` wraps.
    pub fn to_int(self) -> i32 {
        let on = |volume: i32| i32::from(volume != 0);
        [
            i32::from(self.stereo) << 19,
            i32::from(self.fog) << 16,
            i32::from(self.high_water_detail) << 15,
            i32::from(self.high_detail_lighting) << 13,
            i32::from(self.character_shadows) << 10,
            i32::from(self.many_ground_textures) << 9,
            i32::from(self.many_idle_animations) << 7,
            i32::from(self.high_detail_textures) << 6,
            i32::from(self.show_ground_decorations) << 5,
            i32::from(self.all_levels_visible) << 3,
            self.brightness & 7,
            i32::from(self.remove_roofs_selectively) << 4,
            i32::from(self.flickering_effects) << 8,
            (self.scenery_shadows & 3) << 11,
            on(self.sound_volume) << 20,
            on(self.music_volume) << 21,
            on(self.ambient_volume) << 22,
            self.particles << 23,
        ]
        .into_iter()
        .fold(0, i32::wrapping_add)
    }
}

/// WINDOW_STATUS, opcode 243 (`ClientProt.sendWindowDetails`,
/// ClientProt.java:191-195).
///
/// Layout, 6 bytes: `p1(window_mode)`, `p2(canvas_width)`,
/// `p2(canvas_height)`, `p1(anti_aliasing)`, from
/// `DisplayMode.getWindowMode()` (see [`window_mode`]), `GameShell.canvasWidth`,
/// `GameShell.canvasHeight` and `Preferences.antiAliasingMode`.
///
/// Sent at the end of `client.resetGameSessionState` (every login), at the
/// end of `LoginManager.reconnect`, and by `DisplayMode.setWindowMode` when
/// the socket is open and the game state is 30 or 25. The pinned server
/// stores the size and calls `switchWindowMode(window_mode)`.
pub fn window_status(
    window_mode: i32,
    canvas_width: i32,
    canvas_height: i32,
    anti_aliasing: i32,
) -> Frame {
    let mut w = Writer::new();
    w.p1(window_mode);
    w.p2(canvas_width);
    w.p2(canvas_height);
    w.p1(anti_aliasing);
    frame(243, w)
}

/// `DisplayMode.getWindowMode` (DisplayMode.java:68-78): 3 full screen, 2
/// resizable (hardware-rendered and resizable, or the software resizable
/// mode), 1 hardware-rendered fixed size, 0 software fixed size.
pub fn window_mode(
    full_screen: bool,
    gl_enabled: bool,
    resizable: bool,
    resizable_sd: bool,
) -> i32 {
    if full_screen {
        3
    } else if (gl_enabled && resizable) || resizable_sd {
        2
    } else if gl_enabled && !resizable {
        1
    } else {
        0
    }
}

/// TRANSMITVAR_VERIFYID, opcode 177 (`Protocol.transmitVerifyId`,
/// Protocol.java:3297-3298). Layout, 2 bytes: `p2(Protocol.verifyId)`.
///
/// Many server packets carry a tracking number; reading one calls
/// `setVerifyId`, which stores [`next_verify_id`] and marks it changed. At the
/// end of a cycle in which it was marked (after the keep-alive test, before
/// the socket write) the client sends it once. The pinned server adds the
/// difference to its own interface packet counter.
pub fn verify_id(id: i32) -> Frame {
    let mut w = Writer::new();
    w.p2(id);
    frame(177, w)
}

/// `Protocol.setVerifyId`: the id to report after a server packet with
/// tracking number `tracknum`. A new session starts from 0.
pub fn next_verify_id(tracknum: i32) -> i32 {
    tracknum.wrapping_add(1) & 0xffff
}

// ---------------------------------------------------------------------------
// Scene rebuild notifications
// ---------------------------------------------------------------------------

/// The constant payload of [`map_rebuild_marker`].
pub const MAP_REBUILD_MARKER: i32 = 1057001181;

/// Opcode 20 (`LoginManager.rebuildMap`, LoginManager.java:1106-1109).
/// Layout, 4 bytes: `p4(1057001181)`.
///
/// Written once the scene has been rebuilt, immediately before
/// [`map_rebuild_complete`], only when the client runs in its own window
/// (`GameShell.frame != null`, i.e. started as an application rather than an
/// applet), the socket is open and the game state is 25. The pinned server
/// names it MAP_REBUILD_STARTED and drops it after decoding.
pub fn map_rebuild_marker() -> Frame {
    let mut w = Writer::new();
    w.p4(MAP_REBUILD_MARKER);
    frame(20, w)
}

/// Opcode 110 (`LoginManager.rebuildMap`, LoginManager.java:1127-1130). No
/// payload.
///
/// Written when a scene rebuild finishes and the game state goes from 25
/// back to 30 (not for the login-screen rebuild, state 28), if the socket is
/// open. The pinned server drops it after decoding.
pub fn map_rebuild_complete() -> Frame {
    frame(110, Writer::new())
}

// ---------------------------------------------------------------------------
// Menu actions without an encoder elsewhere
// ---------------------------------------------------------------------------

/// Opcode 109: the second option of an item on the ground (menu action 34,
/// MiniMenu.java:821-838). Layout, 6 bytes: `ip2(y)`, `p2(x)`, `ip2add(id)`,
/// with absolute tile coordinates and the item id.
///
/// `MiniMenu.addEntries` gives action codes 21, 34, 18, 20 and 24 to ground
/// options 1 to 5 of an item; the deobfuscated name `PLAYER_ACTION_BLOCK` for
/// 34 is a mislabel. Before writing, the original walks to the item exactly
/// as for the other ground options and shows the red click cross. The pinned
/// server declares the size but has no decoder for it.
pub fn ground_item_option2(x: i32, y: i32, id: i32) -> Frame {
    let mut w = Writer::new();
    w.ip2(y);
    w.p2(x);
    w.ip2add(id);
    frame(109, w)
}

/// Opcode 179: "Face here" on a tile (menu action 36, MiniMenu.java:1098-1100
/// and Protocol.java:2849-2851). Layout, 4 bytes: `p2(y)`, `p2(x)`, absolute
/// tile coordinates.
///
/// The menu entry exists only when `client.game == 1`; the default game is 0.
/// From the minimap (entry key 1) it is written at once. From the viewport
/// (key 0) the action sets `viewportWalkState = 1` and requests the clicked
/// tile; when the scene pick resolves it (`clickTileX != -1`, state 2) the
/// packet is written with a yellow click cross. A moderator holding Ctrl and
/// Shift teleports instead. The pinned server has no decoder for it.
pub fn face_tile(x: i32, y: i32) -> Frame {
    let mut w = Writer::new();
    w.p2(y);
    w.p2(x);
    frame(179, w)
}

/// `Cheat.teleport` (Cheat.java:104-108): the command line a moderator's
/// Ctrl+Shift click or mouse wheel sends, `::tele plane,x>>6,y>>6,x&63,y&63`.
/// It goes out as an ordinary CLIENT_CHEAT command (opcode 44, encoded by the
/// client's chat module), after the client-side command checks.
pub fn teleport_command(x: i32, y: i32, plane: i32) -> String {
    format!(
        "::tele {plane},{},{},{},{}",
        x >> 6,
        y >> 6,
        x & 0x3f,
        y & 0x3f
    )
}

// ---------------------------------------------------------------------------
// Reflection check reply
// ---------------------------------------------------------------------------

/// One entry of a reflection check reply, in request order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReflectionResult {
    /// `p1(0)`, `p4(value)`: a static int field's value (request type 0), a
    /// field's modifiers (type 2) or a method's modifiers (type 4).
    Int(i32),
    /// `p1(0)`: a static int field was set (type 1), or an invoked method
    /// (type 3) returned null.
    Done,
    /// `p1(1)`, `p8(value)`: an invoked method returned a `Number`.
    Number(i64),
    /// `p1(2)`, `pjstr(text)`: an invoked method returned a `JagString`.
    Text(Vec<u8>),
    /// `p1(4)`: an invoked method returned any other object.
    Object,
    /// `p1(code)`: the entry failed. Codes -1 to -5 come from resolving the
    /// request (class not found, security, null, other exception, other
    /// throwable), -5 and -6 from a failed field or method lookup, and -10 to
    /// -21 from carrying it out (`ReflectionCheck.loop`'s catch order).
    Error(i8),
}

/// Opcode 163: the reply to a REFLECTION_CHEAT_CHECK (server opcode 114),
/// written by `ReflectionCheck.loop` (ReflectionCheck.java:73-155) at the
/// start of a game cycle once every lookup of the oldest queued check has
/// finished.
///
/// Layout, variable (byte size): `p4(id)`, one [`ReflectionResult`] per
/// requested entry, then `p4(crc)` where `crc` is the CRC-32 of everything
/// before it in the payload (`Buffer.addcrc` from just after the size byte).
///
/// The pinned server never sends opcode 114 and declares no size for 163, so
/// [`Frame::write`] refuses this frame with the pinned size table. A payload
/// over 255 bytes, which the original would mis-size, is rejected here.
pub fn reflection_reply(id: i32, results: &[ReflectionResult]) -> Result<Frame> {
    let mut w = Writer::new();
    w.p4(id);
    for result in results {
        match result {
            ReflectionResult::Int(value) => {
                w.p1(0);
                w.p4(*value);
            }
            ReflectionResult::Done => w.p1(0),
            ReflectionResult::Number(value) => {
                w.p1(1);
                w.p8(*value);
            }
            ReflectionResult::Text(text) => {
                w.p1(2);
                w.pjstr(text)?;
            }
            ReflectionResult::Object => w.p1(4),
            ReflectionResult::Error(code) => w.p1(i32::from(*code)),
        }
    }
    let crc = crc32(w.as_slice());
    w.p4(crc as i32);
    if w.as_slice().len() > 255 {
        return Err(Error::new(
            w.as_slice().len(),
            "reflection reply exceeds its byte length",
        ));
    }
    Ok(frame(163, w))
}

// ---------------------------------------------------------------------------
// Login flow
// ---------------------------------------------------------------------------

/// Opcode 17 (`LoginManager.loop` step 6, LoginManager.java:468-470). No
/// payload.
///
/// This belongs to the login state machine, not to game play: after login
/// reply 1 the client waits (step 5) until `continueDelayedLogin` moves it to
/// step 6, where it empties the outgoing buffer, writes this opcode through
/// the already keyed ISAAC cipher, sends it at once and waits for the next
/// login reply. The pinned server declares size 0 and has no decoder.
pub fn continue_delayed_login() -> Frame {
    frame(17, Writer::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        buffer::Reader,
        game::{CLIENT_PACKET_SIZES, ClientAction, Direction},
    };

    // Expected bytes marked "JVM" were printed by running the original
    // `rt4.Buffer` (and `rt4.JagString`) class on JDK 25 with the write
    // sequence copied from the client source lines named on each encoder.

    /// `GameReadEvent.PACKET_SIZES` of the pinned server
    /// (reference/server/Server/src/main/core/net/event/GameReadEvent.java),
    /// extracted mechanically: -1 is a byte-sized packet, -3 is undeclared.
    const SERVER_PACKET_SIZES: [i16; 256] = [
        -3, -3, -3, 2, 2, -3, 8, -3, -3, 6, 4, -3, -3, -3, -3, -3, //
        -3, 0, -3, -3, 4, 4, 1, 4, -3, -3, -3, 16, -3, -3, 2, -3, //
        -3, 6, 8, -3, -3, -3, -3, -1, -3, -3, -3, -3, -1, -3, -3, -3, //
        6, -3, -3, -3, -3, 6, -3, 8, -3, 8, -3, -3, -3, -3, -3, -3, //
        6, -1, 6, -3, 2, -3, -3, 2, 2, 12, -3, 6, -3, -1, 2, 12, //
        -3, 8, 12, -3, 6, 8, -3, -3, -3, -3, -3, -3, 2, 0, 2, -3, //
        -3, -3, 4, 10, -3, 14, -3, -3, 8, -3, 2, -3, -3, 6, 0, 2, //
        -3, -3, 2, 10, -3, -1, -3, -3, 8, -3, -3, -1, 6, -3, -3, -3, //
        -3, -3, -3, 10, 6, 2, 14, 8, -3, 4, -3, -3, -3, -3, -3, -3, //
        -3, -3, -3, -3, 2, -3, -3, -3, -3, 8, 8, 6, 8, 3, -3, -3, //
        -3, 8, 8, -3, -3, -3, 6, -1, 6, -3, 6, -3, -3, -3, -3, 2, //
        -3, 2, -1, 4, 2, -3, -3, -3, 0, -3, -3, -3, 9, -3, -3, -3, //
        -3, -3, 6, 8, 6, -3, -3, 6, -3, -1, -3, -3, -3, -3, 8, -3, //
        -3, -3, -3, -3, -3, 8, -3, -1, -3, -3, 2, -3, -3, -3, -3, -3, //
        -3, -3, -3, -3, 6, -3, -3, 9, -3, 12, 6, -3, -3, -1, -3, 8, //
        -3, -3, -3, 6, 8, 0, -3, 6, 10, -3, -3, -3, -3, 14, 6, -3, //
    ];

    /// The opcodes with a decoder in the pinned server's `Decoders530`
    /// (reference/server/Server/src/main/core/net/packet/in/Decoders530.kt),
    /// extracted mechanically. Any other opcode is logged as unhandled.
    const SERVER_DECODED: [u8; 83] = [
        3, 9, 10, 20, 21, 22, 23, 27, 30, 33, 34, 39, 44, 53, 55, 57, 64, 65, 66, 68, 71, 72, 73,
        75, 77, 78, 79, 81, 84, 92, 93, 94, 98, 99, 101, 104, 110, 111, 114, 115, 120, 124, 132,
        134, 135, 137, 148, 153, 155, 156, 157, 161, 162, 166, 167, 168, 170, 175, 177, 180, 184,
        188, 194, 195, 196, 199, 201, 206, 213, 215, 218, 231, 233, 234, 237, 239, 243, 244, 245,
        247, 248, 253, 254,
    ];

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// The server-side reading of a frame; every decoded field must be consumed.
    fn decode(frame: &Frame) -> Option<ClientAction> {
        let mut reader = Reader::new(&frame.payload);
        let action = ClientAction::decode(frame.opcode, &mut reader).unwrap();
        // The server reads nothing of the rebuild marker's four bytes.
        let unread = if frame.opcode == 20 { 4 } else { 0 };
        assert_eq!(reader.remaining(), unread, "opcode {}", frame.opcode);
        action
    }

    /// One frame from every encoder in this module.
    fn samples() -> Vec<Frame> {
        let mut recorder = MouseRecorder {
            enabled: true,
            ..MouseRecorder::default()
        };
        recorder.sample(10, 20);
        vec![
            continue_delayed_login(),
            map_rebuild_marker(),
            camera_position(383, 1536),
            focus(true),
            mouse_click(true, 5, 5, 65535),
            preferences(24754171),
            ground_item_option2(3222, 3240, 995),
            map_rebuild_complete(),
            recorder.flush(true).unwrap(),
            reflection_reply(7, &[ReflectionResult::Done]).unwrap(),
            verify_id(0x1235),
            face_tile(3222, 3240),
            window_status(2, 1920, 1017, 1),
            idle_logout(),
        ]
    }

    #[test]
    fn the_client_writes_exactly_the_opcodes_the_server_sizes_plus_the_reflection_reply() {
        assert_eq!(SERVER_PACKET_SIZES, CLIENT_PACKET_SIZES);
        assert!(ORIGINAL_CLIENT_OPCODES.windows(2).all(|w| w[0] < w[1]));
        for opcode in 0..=255u8 {
            let declared = SERVER_PACKET_SIZES[usize::from(opcode)] != -3;
            let written = ORIGINAL_CLIENT_OPCODES.contains(&opcode);
            assert_eq!(written, declared || opcode == 163, "opcode {opcode}");
        }
        assert_eq!(SERVER_PACKET_SIZES[163], -3);
        // Every decoder is for a sized opcode the client writes; 16 sized
        // opcodes have no decoder.
        assert!(
            SERVER_DECODED
                .iter()
                .all(|op| SERVER_PACKET_SIZES[usize::from(*op)] != -3)
        );
        let undecoded: Vec<u8> = ORIGINAL_CLIENT_OPCODES
            .into_iter()
            .filter(|op| *op != 163 && !SERVER_DECODED.contains(op))
            .collect();
        assert_eq!(
            undecoded,
            [
                4, 6, 17, 48, 82, 85, 106, 109, 117, 123, 131, 133, 154, 178, 179, 228
            ]
        );
    }

    #[test]
    fn payload_lengths_match_the_server_size_table() {
        let frames = samples();
        let opcodes: Vec<u8> = frames.iter().map(|f| f.opcode).collect();
        assert_eq!(
            opcodes,
            [
                17, 20, 21, 22, 75, 98, 109, 110, 123, 163, 177, 179, 243, 245
            ]
        );
        for frame in &frames {
            let declared = SERVER_PACKET_SIZES[usize::from(frame.opcode)];
            let mut wire = Vec::new();
            let written = frame.write(Direction::ClientToServer, None, &mut wire);
            match declared {
                -3 => {
                    // Only the reflection reply: the pinned table cannot frame it.
                    assert_eq!(frame.opcode, 163);
                    assert!(written.is_err());
                }
                -1 => {
                    assert_eq!(frame.opcode, 123);
                    written.unwrap();
                    assert_eq!(wire[0], 123);
                    assert_eq!(usize::from(wire[1]), frame.payload.len());
                    assert_eq!(&wire[2..], frame.payload);
                }
                size => {
                    assert_eq!(
                        frame.payload.len(),
                        size as usize,
                        "opcode {}",
                        frame.opcode
                    );
                    written.unwrap();
                    assert_eq!(wire[0], frame.opcode);
                    assert_eq!(&wire[1..], frame.payload);
                }
            }
        }
    }

    #[test]
    fn the_pinned_server_decoder_reads_back_what_it_decodes() {
        // `ClientAction::decode` is this crate's port of Decoders530.
        assert_eq!(
            decode(&camera_position(383, 1536)),
            Some(ClientAction::TrackingCameraPos { x: 383, y: 1536 })
        );
        assert_eq!(
            decode(&focus(true)),
            Some(ClientAction::TrackingFocus { is_focused: true })
        );
        assert_eq!(
            decode(&focus(false)),
            Some(ClientAction::TrackingFocus { is_focused: false })
        );
        // The decoder calls the high half x and the low half y; it keeps 11
        // bits of the elapsed time.
        assert_eq!(
            decode(&mouse_click(true, 1234, 400, 300)),
            Some(ClientAction::TrackingMouseClick {
                x: 300,
                y: 400,
                is_right_click: true,
                delay: 1234,
            })
        );
        assert_eq!(
            decode(&mouse_click(false, 32767, 764, 502)),
            Some(ClientAction::TrackingMouseClick {
                x: 502,
                y: 764,
                is_right_click: false,
                delay: 2047,
            })
        );
        assert_eq!(
            decode(&window_status(2, 1920, 1017, 1)),
            Some(ClientAction::TrackingDisplayUpdate {
                window_mode: 2,
                screen_width: 1920,
                screen_height: 1017,
                display_mode: 1,
            })
        );
        assert_eq!(
            decode(&idle_logout()),
            Some(ClientAction::TrackingAfkTimeout)
        );
        assert_eq!(
            decode(&verify_id(0x1235)),
            Some(ClientAction::PacketCountUpdate { count: 0x1235 })
        );
        assert_eq!(
            decode(&preferences(24754171)),
            Some(ClientAction::PlayerPrefsUpdate { prefs: 24754171 })
        );
        assert_eq!(decode(&map_rebuild_marker()), Some(ClientAction::NoProcess));
        assert_eq!(
            decode(&map_rebuild_complete()),
            Some(ClientAction::NoProcess)
        );
        for frame in samples() {
            let decoded = SERVER_DECODED.contains(&frame.opcode);
            assert_eq!(
                decoded,
                [20, 21, 22, 75, 98, 110, 177, 243, 245].contains(&frame.opcode)
            );
            if !decoded {
                let mut reader = Reader::new(&frame.payload);
                assert_eq!(ClientAction::decode(frame.opcode, &mut reader), Ok(None));
            }
        }
    }

    #[test]
    fn fixed_layouts_match_the_original_buffer() {
        // JVM, Protocol.java:2547-2549.
        assert_eq!(hex(&camera_position(383, 1536).payload), "01ff0006");
        assert_eq!(hex(&camera_position(128, 2047).payload), "0000ff07");
        // JVM, Protocol.java:2553-2559.
        assert_eq!(hex(&focus(true).payload), "01");
        assert_eq!(hex(&focus(false).payload), "00");
        // JVM, ClientProt.java:191-195.
        assert_eq!(
            hex(&window_status(2, 1920, 1017, 1).payload),
            "02078003f901"
        );
        assert_eq!(hex(&window_status(0, 765, 503, 0).payload), "0002fd01f700");
        // JVM, Protocol.java:3289-3298.
        assert_eq!(hex(&verify_id(next_verify_id(0x1234)).payload), "1235");
        assert_eq!(hex(&verify_id(next_verify_id(65535)).payload), "0000");
        // JVM, LoginManager.java:1107-1108.
        assert_eq!(hex(&map_rebuild_marker().payload), "3f008edd");
        // JVM, MiniMenu.java:834-837.
        assert_eq!(
            hex(&ground_item_option2(3222, 3240, 995).payload),
            "a80c0c966303"
        );
        assert_eq!(
            hex(&ground_item_option2(0x1234, 0xabcd, 0x5678).payload),
            "cdab1234f856"
        );
        // JVM, MiniMenu.java:1098-1100.
        assert_eq!(hex(&face_tile(3222, 3240).payload), "0ca80c96");
        // No payload in the original.
        for frame in [
            map_rebuild_complete(),
            idle_logout(),
            continue_delayed_login(),
        ] {
            assert!(frame.payload.is_empty());
        }
        assert_eq!(map_rebuild_complete().opcode, 110);
        assert_eq!(idle_logout().opcode, 245);
        assert_eq!(continue_delayed_login().opcode, 17);
    }

    #[test]
    fn click_reports_match_the_original_sequence() {
        // JVM, Protocol.java:2503-2529, run in this order with prevClickTime
        // starting at 0: (button, x, y, time) -> payload.
        let cases: [(i32, i32, i32, i64, &str); 6] = [
            (1, 400, 300, 1_000_000, "a04e2c019001"),
            (2, 5, 65535, 1_000_250, "8580ffff0500"),
            (1, -7, 70000, 1_000_299, "8000ffff0000"),
            (2, 65535, 0, 1_000_299 + 32767 * 50 + 49, "7fff0000ffff"),
            (
                2,
                764,
                502,
                1_000_299 + 32767 * 50 + 49 + 32768 * 50,
                "7ffff601fc02",
            ),
            (0, 1, 1, 0, ""),
        ];
        let mut prev = 0;
        for (button, x, y, time, expected) in cases {
            let before = prev;
            match mouse_click_report(button, x, y, time, &mut prev) {
                Some(frame) => {
                    assert_eq!(frame.opcode, 75);
                    assert_eq!(hex(&frame.payload), expected);
                    assert_eq!(prev, time);
                }
                None => {
                    assert_eq!((button, expected), (0, ""));
                    assert_eq!(prev, before);
                }
            }
        }
    }

    /// The output of the original mouse-recorder block (Protocol.java:2426-2502,
    /// copied verbatim around the original `rt4.Buffer`) for the script in
    /// `mouse_recorder_matches_the_original_block`. `len`, `crc` and `hex`
    /// describe the bytes after the size byte.
    const MOUSE_RECORDER_JVM: &str = "\
move_39_no_click - left=39 prev=0,0 idle=0
move_39_click len=81 crc=305790ce left=0 prev=138,200 idle=0
move_mixed len=50 crc=c54a0025 hex=2896003f0fc080a1808000ff80ff00c000bc012cc080000000c1fffe0000c00000fffe897c84e0280004fffbe02180000000 left=0 prev=-1,-1 idle=0
move_idle_round0 len=0 crc=00000000 hex= left=0 prev=-1,-1 idle=500
move_idle_round1 len=0 crc=00000000 hex= left=0 prev=-1,-1 idle=1000
move_idle_round2 len=0 crc=00000000 hex= left=0 prev=-1,-1 idle=1500
move_idle_round3 len=0 crc=00000000 hex= left=0 prev=-1,-1 idle=2000
move_idle_round4 len=0 crc=00000000 hex= left=0 prev=-1,-1 idle=2047
move_after_saturation len=6 crc=9da521dd hex=e7ff000a000a left=0 prev=10,10 idle=0
move_cut_round0 len=240 crc=6b0772da left=452 prev=20000,40000 idle=0
move_cut_round1 len=240 crc=6b0772da left=404 prev=20000,40000 idle=0
move_cut_round2 len=240 crc=6b0772da left=356 prev=20000,40000 idle=0
move_cut_round3 len=240 crc=6b0772da left=308 prev=20000,40000 idle=0
move_cut_round4 len=240 crc=6b0772da left=260 prev=20000,40000 idle=0
move_cut_round5 len=240 crc=6b0772da left=212 prev=20000,40000 idle=0
move_cut_round6 len=240 crc=6b0772da left=164 prev=20000,40000 idle=0
move_cut_round7 len=240 crc=6b0772da left=116 prev=20000,40000 idle=0
move_cut_round8 len=240 crc=6b0772da left=68 prev=20000,40000 idle=0
move_cut_round9 len=240 crc=6b0772da left=20 prev=20000,40000 idle=0
move_cut_tail_click len=100 crc=42b7795d left=0 prev=20000,40000 idle=0
move_random_round0 len=240 crc=c0b11817 left=396 prev=314,866 idle=0
move_random_round1 len=244 crc=7cf18a0b left=282 prev=1198,1118 idle=0
move_random_round2 len=244 crc=214dbde6 left=162 prev=941,858 idle=0
move_random_round3 len=241 crc=f0ca8b8b left=35 prev=750,557 idle=0
move_random_tail_click len=56 crc=1fe79067 hex=814c220c19809eda2243079e030e8182e40b6280c03214d903cf056513290532806b50844561039c0cea092f80aa928193ec062c09660332 left=0 prev=668,633 idle=0
move_disabled - left=0 prev=668,633 idle=0
";

    fn flush_line(recorder: &mut MouseRecorder, name: &str, click: bool) -> String {
        let body = match recorder.flush(click) {
            None => "-".to_owned(),
            Some(frame) => {
                assert_eq!(frame.opcode, 123);
                let mut body = format!(
                    "len={} crc={:08x}",
                    frame.payload.len(),
                    crc32(&frame.payload)
                );
                if frame.payload.len() <= 64 {
                    body += &format!(" hex={}", hex(&frame.payload));
                }
                body
            }
        };
        format!(
            "{name} {body} left={} prev={},{} idle={}\n",
            recorder.pending(),
            recorder.prev[0],
            recorder.prev[1],
            recorder.idle_samples
        )
    }

    #[test]
    fn mouse_recorder_matches_the_original_block() {
        let mut r = MouseRecorder {
            enabled: true,
            ..MouseRecorder::default()
        };
        let mut out = String::new();
        // Below the 40-sample threshold and no click: nothing is sent.
        for i in 0..39 {
            r.sample(100 + i, 200);
        }
        out += &flush_line(&mut r, "move_39_no_click", false);
        // A click flushes them. The first change is from (0,0): 5 bytes.
        out += &flush_line(&mut r, "move_39_click", true);
        // Small and medium deltas at their limits, idle runs, far jumps,
        // clamping and leaving the window.
        r.sample(138, 200);
        r.sample(138, 200);
        r.sample(140, 190);
        r.sample(108, 221);
        r.sample(139, 189);
        r.sample(172, 189);
        r.sample(44, 316);
        r.sample(171, 188);
        r.sample(300, 188);
        r.sample(-1, -1);
        r.sample(-1, -1);
        r.sample(-5, 70000);
        r.sample(65534, 0);
        r.sample(70000, -3);
        for _ in 0..8 {
            r.sample(65534, 0);
        }
        r.sample(65530, 4);
        for _ in 0..40 {
            r.sample(65530, 4);
        }
        r.sample(65531, 4);
        for _ in 0..33 {
            r.sample(65531, 4);
        }
        r.sample(-1, -1);
        out += &flush_line(&mut r, "move_mixed", false);
        // The idle count saturates at 2047; the packets are empty.
        for round in 0..5 {
            for _ in 0..500 {
                r.sample(-1, -1);
            }
            out += &flush_line(&mut r, &format!("move_idle_round{round}"), false);
        }
        r.sample(10, 10);
        out += &flush_line(&mut r, "move_after_saturation", true);
        // The 240-byte cut: far jumps take 5 bytes, so 48 samples a packet.
        for i in 0..500 {
            if i & 1 == 0 {
                r.sample(1000, 50);
            } else {
                r.sample(20000, 40000);
            }
        }
        let mut round = 0;
        while r.pending() >= 40 {
            out += &flush_line(&mut r, &format!("move_cut_round{round}"), false);
            round += 1;
        }
        out += &flush_line(&mut r, "move_cut_tail_click", true);
        // A pseudo-random walk from the same generator as the Java harness.
        let mut seed = 530i32;
        let mut next = move || {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            ((seed as u32) >> 16) as i32 & 0x7fff
        };
        let (mut px, mut py) = (300, 300);
        for _ in 0..500 {
            // 0-2 stay, 3-6 a short step, 7-8 a longer one, 9 a jump.
            let kind = next() % 10;
            if (3..7).contains(&kind) {
                px += next() % 41 - 20;
                py += next() % 41 - 20;
            } else if (7..9).contains(&kind) {
                px += next() % 201 - 100;
                py += next() % 201 - 100;
            } else if kind == 9 {
                px = next() % 1920;
                py = next() % 1080;
            }
            if next() % 50 == 0 {
                r.sample(-1, -1);
            } else {
                r.sample(px, py);
            }
        }
        let mut round = 0;
        while r.pending() >= 40 {
            out += &flush_line(&mut r, &format!("move_random_round{round}"), false);
            round += 1;
        }
        out += &flush_line(&mut r, "move_random_tail_click", true);
        // A disabled recorder discards what it holds.
        r.sample(1, 2);
        r.enabled = false;
        out += &flush_line(&mut r, "move_disabled", true);

        let expected: Vec<&str> = MOUSE_RECORDER_JVM.lines().collect();
        let actual: Vec<&str> = out.lines().collect();
        for (actual, expected) in actual.iter().zip(&expected) {
            assert_eq!(actual, expected);
        }
        assert_eq!(actual.len(), expected.len());
    }

    #[test]
    fn mouse_recorder_capacity_and_session_reset() {
        let mut r = MouseRecorder::default();
        for i in 0..600 {
            r.sample(i, i);
        }
        assert_eq!(r.pending(), MouseRecorder::CAPACITY);
        // Disabled: the samples are dropped without a packet.
        assert_eq!(r.flush(true), None);
        assert_eq!(r.pending(), 0);
        r.enabled = true;
        r.sample(3, 4);
        r.sample(3, 4);
        // Hand-traced: (4 + 32) + ((3 + 32) << 6) = 0x08e4, then one idle sample.
        let frame = r.flush(true).unwrap();
        assert_eq!(hex(&frame.payload), "08e4");
        assert_eq!((r.prev, r.idle_samples), ([3, 4], 1));
        r.sample(9, 9);
        r.reset_session();
        assert_eq!((r.pending(), r.prev, r.idle_samples), (0, [3, 4], 0));
        assert!(r.enabled);
    }

    #[test]
    fn preference_bits_match_the_original_expression() {
        // JVM, Preferences.java:326 with these inputs, then Protocol.java:2563.
        let defaults = PreferenceFlags {
            brightness: 3,
            all_levels_visible: true,
            remove_roofs_selectively: true,
            show_ground_decorations: true,
            high_detail_textures: true,
            many_idle_animations: true,
            flickering_effects: true,
            many_ground_textures: true,
            character_shadows: true,
            scenery_shadows: 2,
            high_detail_lighting: true,
            high_water_detail: true,
            fog: true,
            stereo: true,
            sound_volume: 127,
            music_volume: 255,
            ambient_volume: 127,
            particles: 2,
        };
        assert_eq!(defaults.to_int(), 24754171);
        assert_eq!(hex(&preferences(defaults.to_int()).payload), "0179b7fb");
        assert_eq!(PreferenceFlags::default().to_int(), 0);
        let mixed = PreferenceFlags {
            brightness: 4,
            all_levels_visible: true,
            high_detail_textures: true,
            flickering_effects: true,
            many_ground_textures: true,
            scenery_shadows: 1,
            high_detail_lighting: true,
            fog: true,
            music_volume: 3,
            particles: 1,
            ..PreferenceFlags::default()
        };
        assert_eq!(mixed.to_int(), 10562380);
        assert_eq!(hex(&preferences(mixed.to_int()).payload), "00a12b4c");
        // Brightness and shadow type are masked to 3 and 2 bits.
        let masks = PreferenceFlags {
            brightness: 0x7d,
            remove_roofs_selectively: true,
            show_ground_decorations: true,
            many_idle_animations: true,
            character_shadows: true,
            scenery_shadows: 0x7e,
            high_water_detail: true,
            stereo: true,
            sound_volume: 1,
            ambient_volume: 9,
            ..PreferenceFlags::default()
        };
        assert_eq!(masks.to_int(), 5805237);
        // The particle setting is not masked.
        let particles = |particles| PreferenceFlags {
            brightness: 1,
            particles,
            ..PreferenceFlags::default()
        };
        assert_eq!(particles(255).to_int(), 2139095041);
        assert_eq!(particles(300).to_int(), -1778384895);
        assert_eq!(
            hex(&preferences(particles(300).to_int()).payload),
            "96000001"
        );
        let negative = PreferenceFlags {
            brightness: 2,
            sound_volume: 1,
            music_volume: 1,
            ambient_volume: 1,
            particles: -1,
            ..defaults
        };
        assert_eq!(negative.to_int(), -411654);
        // The server's own reading of the default value (Decoders530
        // PLAYER_PREFS_UPDATE comments).
        let prefs = defaults.to_int();
        assert_eq!(prefs >> 23, 2);
        assert_eq!(prefs & 7, 3);
        assert_eq!((prefs >> 11) & 3, 2);
        for bit in [22, 21, 20, 19, 16, 15, 13, 10, 9, 8, 7, 6, 5, 4, 3] {
            assert_eq!((prefs >> bit) & 1, 1, "bit {bit}");
        }
    }

    #[test]
    fn reflection_reply_matches_the_original_buffer() {
        // JVM, ReflectionCheck.java:73-155 with one of every reply shape; the
        // first JVM byte (0x2c) is the size byte.
        let frame = reflection_reply(
            0x01020304,
            &[
                ReflectionResult::Int(-559038737),
                ReflectionResult::Done,
                ReflectionResult::Int(25),
                ReflectionResult::Done,
                ReflectionResult::Number(-2),
                ReflectionResult::Text(b"ok".to_vec()),
                ReflectionResult::Object,
                ReflectionResult::Int(9),
                ReflectionResult::Error(-1),
                ReflectionResult::Error(-5),
                ReflectionResult::Error(-6),
                ReflectionResult::Error(-10),
                ReflectionResult::Error(-21),
            ],
        )
        .unwrap();
        assert_eq!(frame.opcode, 163);
        assert_eq!(frame.payload.len(), 0x2c);
        assert_eq!(
            hex(&frame.payload),
            "0102030400deadbeef0000000000190001fffffffffffffffe026f6b00040000000009fffbfaf6eb9bad2129"
        );
        let empty = reflection_reply(7, &[]).unwrap();
        assert_eq!(empty.payload.len(), 8);
        assert_eq!(hex(&empty.payload), "00000007bf204abf");
        // Bounds the original does not have.
        assert!(reflection_reply(1, &[ReflectionResult::Text(vec![b'a', 0])]).is_err());
        let long = vec![ReflectionResult::Number(1); 28];
        assert!(reflection_reply(1, &long).is_err());
        assert_eq!(
            reflection_reply(1, &long[..27]).unwrap().payload.len(),
            4 + 27 * 9 + 4
        );
    }

    #[test]
    fn teleport_command_matches_the_original_text() {
        // JVM, Cheat.java:105 with JagString.concatenate/parseInt.
        assert_eq!(teleport_command(3222, 3218, 0), "::tele 0,50,50,22,18");
        assert_eq!(teleport_command(2000, 9999, 3), "::tele 3,31,156,16,15");
        assert_eq!(teleport_command(63, 64, 1), "::tele 1,0,1,63,0");
    }

    // The remaining tests trace the Java conditions by hand; they were not
    // run against the JVM.

    #[test]
    fn camera_report_follows_the_change_flag_and_cooldown() {
        let mut report = CameraReport::default();
        // A new session reports at once, then waits for a change.
        assert!(report.step(false));
        assert_eq!(report.cooldown, 20);
        assert!((0..100).all(|_| !report.step(false)));
        assert_eq!(report.cooldown, 0);
        // A change with no cooldown is reported in the same cycle.
        assert!(report.step(true));
        // Held keys: the next report comes when the cooldown runs out, on
        // the 20th cycle after the previous one.
        let cycles: Vec<bool> = (0..41).map(|_| report.step(true)).collect();
        let sent: Vec<usize> = (0..41).filter(|i| cycles[*i]).collect();
        assert_eq!(sent, [19, 39]);
        // A change during the cooldown is remembered until it runs out.
        assert!(!report.step(true));
        assert_eq!(report.cooldown, 18);
        assert!((0..17).all(|_| !report.step(false)));
        assert!(report.step(false));
        assert!(!report.changed);
    }

    #[test]
    fn focus_and_idle_reports_follow_the_original_conditions() {
        let mut prev = true;
        assert_eq!(focus_report(&mut prev, true), None);
        assert_eq!(focus_report(&mut prev, false), Some(focus(false)));
        assert!(!prev);
        assert_eq!(focus_report(&mut prev, false), None);
        assert_eq!(focus_report(&mut prev, true), Some(focus(true)));
        assert!(prev);

        let (mut mouse, mut timer) = (15000, 0);
        assert_eq!(idle_logout_report(&mut mouse, 15001, &mut timer), None);
        mouse = 15001;
        assert_eq!(idle_logout_report(&mut mouse, 15000, &mut timer), None);
        assert_eq!((mouse, timer), (15001, 0));
        assert_eq!(
            idle_logout_report(&mut mouse, 15001, &mut timer),
            Some(idle_logout())
        );
        assert_eq!((mouse, timer), (14500, 250));
        assert_eq!(idle_logout_report(&mut mouse, 20000, &mut timer), None);
    }

    #[test]
    fn window_mode_follows_the_original_order() {
        assert_eq!(window_mode(true, true, true, true), 3);
        assert_eq!(window_mode(false, true, true, false), 2);
        assert_eq!(window_mode(false, false, false, true), 2);
        assert_eq!(window_mode(false, true, false, true), 2);
        assert_eq!(window_mode(false, true, false, false), 1);
        assert_eq!(window_mode(false, false, true, false), 0);
        assert_eq!(window_mode(false, false, false, false), 0);
    }
}
