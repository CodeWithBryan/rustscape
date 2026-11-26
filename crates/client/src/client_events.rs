//! The packets the original client's main loop sends by itself, and the
//! timers they hang on.
//!
//! One 20 ms cycle of `client.mainLoop` (client.java:1693-1745) is, in order:
//!
//! 1. `Keyboard.loop` and `Mouse.loop`, in every game state: each idle
//!    counter grows by one (any event of its device has zeroed it) and the
//!    latest click is latched for this cycle only.
//! 2. In state 25, `LoginManager.rebuildMap`. When the scene is built it
//!    writes opcodes 20 and 110 and returns to state 30.
//! 3. In state 30, `Protocol.loop` (Protocol.java:2398-3020): read at most
//!    100 server packets; then, if the state is still 30, write the mouse
//!    recorder (123), the click (75), the camera (21), the focus (22) and
//!    the preferences (98); run the world and the interfaces, whose packets
//!    follow; then the idle logout (245), the keep-alive (93) when nothing
//!    was written for more than 50 cycles, the verification id (177); and
//!    last one socket write of everything.
//!
//! Step 1 is [`crate::input::Input::cycle`], the ports of `Keyboard.loop` and
//! `Mouse.loop`. This module is steps 2 and 3 without the socket, and it
//! reads the same statics the original reads, from that port ([`Devices`]):
//! `Mouse.clickButton`, `clickX`, `clickY`, `clickTime` and `prevClickTime`
//! for the click report, `Keyboard.pressedKeys[96..=99]` or the key queue for
//! the camera report, `GameShell.focus`, `Mouse.lastMouseX/Y` for the mouse
//! recorder and the two idle counters for the idle logout. Nothing here
//! collects input of its own.
//!
//! [`ClientEvents::cycle_begin`] is the part of a cycle before the world and
//! the interfaces run, [`ClientEvents::cycle_end`] the part after them;
//! [`ClientEvents::cycle`] is both, for a cycle with nothing in between. The
//! frames of one cycle are a [`Cycle`], whose [`Cycle::written`] puts the
//! caller's interface and menu packets where the original writes them.
//! [`crate::session::Session::cycle`] adds the keep-alive and writes.
//!
//! The encoders and their conditions are in
//! [`rs09_protocol::game::outgoing`]; this module is when they run and in
//! what order. Line numbers refer to the pinned client sources.
use crate::{
    input::{
        Input as Devices,
        keyboard::{KEY_DOWN, KEY_LEFT, KEY_RIGHT, KEY_UP},
    },
    ui::vm::Preferences,
};
use rs09_protocol::game::{
    Frame,
    outgoing::{self, CameraReport, MouseRecorder, PreferenceFlags},
};

/// The server packets whose handler calls `Protocol.setVerifyId` with their
/// tracking number, ascending (Protocol.java:974-2282, resolved through
/// `ServerProt`). All 29 call it unconditionally, so reading any of them
/// makes the client send its verification id at the end of that cycle.
pub const TRACKED_SERVER_OPCODES: [u8; 29] = [
    2, 9, 21, 24, 27, 36, 48, 50, 65, 66, 69, 73, 111, 115, 119, 123, 125, 130, 132, 145, 149, 154,
    155, 165, 171, 176, 187, 207, 220,
];

/// `Protocol.loop` reads at most this many server packets per cycle
/// (Protocol.java:2412).
pub const PACKETS_PER_CYCLE: usize = 100;

/// `MouseRecorder.run` sleeps this long after each sample.
pub const MOUSE_SAMPLE_MILLIS: i64 = 50;

/// `client.mainRedraw` reports a changed window size this long after the
/// last change (client.java:840).
pub const WINDOW_REPORT_DELAY_MILLIS: i64 = 500;

/// What `ClientProt.sendWindowDetails` reads (ClientProt.java:190-196).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Window {
    /// `DisplayMode.getWindowMode()`.
    pub mode: i32,
    /// `GameShell.frameWidth`/`frameHeight`: the window's client area.
    pub frame: [i32; 2],
    /// `Preferences.antiAliasingMode`.
    pub anti_aliasing: i32,
}

impl Window {
    /// `GameShell.canvasWidth`/`canvasHeight` as `DisplayMode.setWindowMode`
    /// and `GameShell.updateCanvasSize` leave them: the whole client area in
    /// the resizable and full-screen modes, 765 by 503 in the fixed ones.
    /// (The reference checkout's own `resizableSD` flag, which makes mode 0
    /// resizable, is not modelled.)
    pub fn canvas(&self) -> [i32; 2] {
        if self.mode >= 2 {
            self.frame
        } else {
            [765, 503]
        }
    }

    /// WINDOW_STATUS for this window.
    pub fn status(&self) -> Frame {
        let [width, height] = self.canvas();
        outgoing::window_status(self.mode, width, height, self.anti_aliasing)
    }
}

/// `Preferences.toInt` over this client's preferences, the payload of
/// opcode 98.
pub fn packed_preferences(p: &Preferences) -> i32 {
    PreferenceFlags {
        brightness: p.brightness,
        all_levels_visible: p.all_levels_visible,
        remove_roofs_selectively: p.remove_roofs_selectively,
        show_ground_decorations: p.show_ground_decorations,
        high_detail_textures: p.high_detail_textures,
        many_idle_animations: p.many_idle_animations,
        flickering_effects: p.flickering_effects,
        many_ground_textures: p.many_ground_textures,
        character_shadows: p.character_shadows,
        scenery_shadows: p.scenery_shadows,
        high_detail_lighting: p.high_detail_lighting,
        high_water_detail: p.high_water_detail,
        fog: p.fog,
        stereo: p.stereo,
        sound_volume: p.sound_volume,
        music_volume: p.music_volume,
        ambient_volume: p.ambient_volume,
        particles: p.particles,
    }
    .to_int()
}

/// What one cycle reads besides the input devices: the state of the rest of
/// the client at this cycle.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Client {
    /// `Preferences.keyboardCameraEnabled`.
    pub keyboard_camera: bool,
    /// `(int) Camera.pitchTarget` and `(int) Camera.yawTarget`.
    pub camera: [i32; 2],
    /// `Preferences.toInt()`; see [`packed_preferences`].
    pub preferences: i32,
    /// `LoginManager.rebuildMap` would reach its end in this cycle: the
    /// scene the last rebuild asked for is built, which ends state 25.
    pub scene_ready: bool,
}

/// An arrow key code (`Keyboard.KEY_LEFT`, `KEY_RIGHT`, `KEY_UP`,
/// `KEY_DOWN`: 96 to 99), as Protocol.java:2536 and 2541 test for.
fn arrow(code: i32) -> bool {
    [KEY_LEFT, KEY_RIGHT, KEY_UP, KEY_DOWN]
        .iter()
        .any(|&key| key as i32 == code)
}

/// State owned by other parts of the client that a cycle reads and writes.
pub struct Shared<'a> {
    /// `Protocol.verifyId`, which the packet handlers keep.
    pub verify_id: i32,
    /// `Preferences.sentToServer`.
    pub preferences_sent: &'a mut bool,
    /// `Protocol.logoutOnDisconnectTimer`. Its countdown at the top of
    /// `Protocol.loop` belongs to the caller.
    pub logout_timer: &'a mut i32,
}

/// `ClientProt.ping` (ClientProt.java:199-219) on the game connection: the
/// call counts as a cycle since the last write; unless `force` is set it
/// does nothing while fewer than 50 have passed; otherwise `waiting` and a
/// keep-alive (opcode 93) behind it go out as one write. Returns whether it
/// wrote.
pub trait Ping {
    fn ping(&mut self, force: bool, waiting: &[Frame]) -> bool;
}

/// The frames of one cycle, in the order the original writes them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Cycle {
    /// Written before the interfaces run: what was already waiting in the
    /// outgoing buffer (a window status), the two frames of a finished scene
    /// rebuild, then opcodes 123, 75, 21, 22 and 98.
    pub before: Vec<Frame>,
    /// Written while the interfaces run, after the caller's own packets: a
    /// window status caused by a script changing the window mode.
    pub interface: Vec<Frame>,
    /// Written after the interfaces: the idle logout, opcode 245.
    pub after: Vec<Frame>,
    /// The verification id, opcode 177: after the keep-alive, last before
    /// the socket write.
    pub verify_id: Option<Frame>,
    /// `Protocol.loop` ran to its end (the game state was 30).
    pub playing: bool,
}

impl Cycle {
    /// Everything up to the keep-alive, with the caller's interface and menu
    /// packets of this cycle in their place, and the verification id that
    /// follows the keep-alive.
    pub fn written(
        self,
        interfaces: impl IntoIterator<Item = Frame>,
    ) -> (Vec<Frame>, Option<Frame>) {
        let mut frames = self.before;
        frames.extend(interfaces);
        frames.extend(self.interface);
        frames.extend(self.after);
        (frames, self.verify_id)
    }
}

/// The rebuild a packet asks for: its central zone and plane.
fn rebuild_of(frame: &Frame) -> Option<([i32; 2], i32)> {
    match frame.opcode {
        162 => rs09_protocol::game::rebuild::decode_normal(frame)
            .ok()
            .map(|r| ([r.zone_x, r.zone_y], r.plane)),
        214 => match rs09_protocol::game::events::ServerEvent::decode(frame) {
            Ok(Some(rs09_protocol::game::events::ServerEvent::DynamicRebuild(r))) => {
                Some(([r.zone_x, r.zone_y], r.plane))
            }
            _ => None,
        },
        _ => None,
    }
}

/// The server packets received on the game connection and not read yet.
#[derive(Clone, Debug, Default)]
pub struct Backlog {
    frames: std::collections::VecDeque<Frame>,
    /// The next packet of the connection is the rebuild packet the login
    /// itself reads (`LoginManager.loop` step 9), outside `Protocol.loop`.
    login_packet: bool,
}

impl Backlog {
    /// The connection is gone: what it had sent is dropped.
    pub fn clear(&mut self) {
        self.frames.clear();
        self.login_packet = false;
    }

    /// A session has just begun ([`ClientEvents::session_start`]): its
    /// first packet is the login's.
    pub fn expect_login_packet(&mut self) {
        self.login_packet = true;
    }

    /// The packets one cycle reads, of those just received (`incoming`) and
    /// those still waiting: at most [`PACKETS_PER_CYCLE`]
    /// (Protocol.java:2412). The original's loop does not look at the game
    /// state, so the hundred that hold a packet starting a rebuild are read
    /// to their end; the cycles of game state 25 that follow run
    /// `LoginManager.rebuildMap` instead of `Protocol.loop` and read none.
    ///
    /// The login's rebuild packet is returned alone, whatever the state.
    /// Each packet returned is noted for the verification id and, if it is
    /// a rebuild, for the scene rebuild.
    pub fn read(&mut self, events: &mut ClientEvents, incoming: Vec<Frame>) -> Vec<Frame> {
        self.frames.extend(incoming);
        let mut frames = Vec::new();
        if self.login_packet {
            if let Some(frame) = self.frames.pop_front() {
                self.login_packet = false;
                if let Some((zone, plane)) = rebuild_of(&frame) {
                    events.rebuild_packet(zone, plane, true);
                }
                frames.push(frame);
            }
            return frames;
        }
        if events.rebuilding() {
            return frames;
        }
        while frames.len() < PACKETS_PER_CYCLE {
            let Some(frame) = self.frames.pop_front() else {
                break;
            };
            events.server_frame(frame.opcode);
            // `SceneGraph.allLevelsAreVisible()` is true with the hardware
            // renderer, which is the only one this client has.
            if let Some((zone, plane)) = rebuild_of(&frame) {
                events.rebuild_packet(zone, plane, true);
            }
            frames.push(frame);
        }
        frames
    }
}

/// The per-session state behind the self-sent packets.
#[derive(Clone, Debug)]
pub struct ClientEvents {
    /// A game session is open: the socket exists and the state is 30 or 25.
    in_session: bool,
    /// The game state is 25: a rebuild packet was read and the scene is not
    /// built yet.
    rebuilding: bool,
    /// A cycle has passed since that packet was read. `rebuildMap` first
    /// runs in the cycle after it.
    rebuild_cycle_passed: bool,
    /// `SceneGraph.centralZoneX/Y` and `centralPlane`; `None` is the -1 the
    /// login puts there.
    central: Option<([i32; 2], i32)>,
    /// `Protocol.prevFocus`.
    prev_focus: bool,
    /// `Protocol.cameraPositionChanged` and `cameraSendCooldown`.
    camera: CameraReport,
    /// `Protocol.verifyIdChanged`.
    verify_id_changed: bool,
    recorder: MouseRecorder,
    last_mouse_sample: Option<i64>,
    /// Whether `InterfaceList.keyCodes`, filled late in the previous
    /// `Protocol.loop`, holds an arrow key.
    key_queue_arrows: bool,
    /// Frames written between cycles, sent at the head of the next write.
    outbound: Vec<Frame>,
    /// `DisplayMode.setWindowMode` ran during this cycle's interfaces.
    window_mode_set: bool,
    /// `GameShell.frameWidth`/`frameHeight` as last seen by the redraw.
    frame_size: Option<[i32; 2]>,
    /// `DisplayMode.canvasReplaceTime`; 0 is unset.
    canvas_replace_time: i64,
}

impl Default for ClientEvents {
    /// The static initial values: `prevFocus` true, the camera marked
    /// changed, everything else zero.
    fn default() -> Self {
        Self {
            in_session: false,
            rebuilding: false,
            rebuild_cycle_passed: false,
            central: None,
            prev_focus: true,
            camera: CameraReport::default(),
            verify_id_changed: false,
            recorder: MouseRecorder::default(),
            last_mouse_sample: None,
            key_queue_arrows: false,
            outbound: Vec::new(),
            window_mode_set: false,
            frame_size: None,
            canvas_replace_time: 0,
        }
    }
}

impl ClientEvents {
    pub fn in_session(&self) -> bool {
        self.in_session
    }

    /// The game state is 25.
    pub fn rebuilding(&self) -> bool {
        self.in_session && self.rebuilding
    }

    /// The cycles, of `cycles` about to run, that will run `Protocol.loop`
    /// and so read server packets: all of them in state 30; in state 25
    /// none until `LoginManager.rebuildMap` ends, which it does no earlier
    /// than the cycle after the rebuild packet was read and only once the
    /// scene is built (`scene_ready`). `Protocol.loop` then runs in that
    /// same cycle (client.java:1725-1734).
    pub fn reading_cycles(&self, cycles: u32, scene_ready: bool) -> u32 {
        if !self.rebuilding() {
            cycles
        } else if !scene_ready {
            0
        } else if self.rebuild_cycle_passed {
            cycles
        } else {
            cycles.saturating_sub(1)
        }
    }

    /// The login reply was read (`LoginManager.loop` steps 8 and 9,
    /// LoginManager.java:496-543): byte 6 of the reply sets the mouse
    /// recorder, then `client.resetGameSessionState` (client.java:631-755)
    /// runs and the first rebuild packet is read, which enters state 25.
    ///
    /// The reset zeroes the recorder's waiting samples and idle count, the
    /// previous click time, the mouse idle counter and the logout timer;
    /// sets `prevFocus` and the camera-changed flag; empties the outgoing
    /// buffer; and ends by writing the window status, which is therefore
    /// the first packet of the session. It leaves alone the keyboard idle
    /// counter, the camera cooldown, the verification-id flag, the
    /// recorder's previous position and the window-size timer.
    pub fn session_start(
        &mut self,
        mouse_recording: bool,
        window: &Window,
        logout_timer: &mut i32,
        devices: &mut Devices,
    ) {
        self.recorder.enabled = mouse_recording;
        self.recorder.reset_session();
        self.prev_focus = true;
        // Mouse.prevClickTime = 0L (client.java:634)
        devices.mouse.prev_click_time = 0;
        *logout_timer = 0;
        self.outbound.clear();
        self.window_mode_set = false;
        // Mouse.setIdleLoops(0) (client.java:653)
        devices.mouse.set_idle_loops(0);
        self.camera.changed = true;
        self.in_session = true;
        // `SceneGraph.centralZoneX = -1`, then `readRebuildPacket`.
        self.central = None;
        self.rebuilding = true;
        self.rebuild_cycle_passed = false;
        self.outbound.push(window.status());
    }

    /// The session is over (`LoginManager.processLogout`, or the socket was
    /// lost): nothing is written until the next [`Self::session_start`].
    /// The idle counters are the devices' and keep running, as they do on
    /// the login screen.
    pub fn session_end(&mut self) {
        self.in_session = false;
        self.rebuilding = false;
        self.outbound.clear();
        self.window_mode_set = false;
    }

    /// `LoginManager.reconnect` (LoginManager.java:830-862) after a lost
    /// connection was re-established: the outgoing buffer is emptied, the
    /// state returns to 30 and the window status is written again. Nothing
    /// else of this module's state is touched.
    pub fn reconnected(&mut self, window: &Window) {
        self.outbound.clear();
        self.window_mode_set = false;
        self.in_session = true;
        self.rebuilding = false;
        self.outbound.push(window.status());
    }

    /// A server packet was read. Marks the verification id as changed for
    /// the packets that carry a tracking number; the id itself is kept by
    /// their handlers.
    pub fn server_frame(&mut self, opcode: u8) {
        if TRACKED_SERVER_OPCODES.contains(&opcode) {
            self.verify_id_changed = true;
        }
    }

    /// A rebuild packet (REBUILD_NORMAL 162 or REBUILD_REGION 214) was read:
    /// `LoginManager.loadRegion` (LoginManager.java:698-712). Returns whether
    /// it starts a rebuild, which it does unless the central zone is
    /// unchanged and either the plane is too or all levels are visible.
    ///
    /// `all_levels_visible` is `SceneGraph.allLevelsAreVisible()`: always
    /// true with the hardware renderer, otherwise the preference.
    pub fn rebuild_packet(&mut self, zone: [i32; 2], plane: i32, all_levels_visible: bool) -> bool {
        if let Some((central_zone, central_plane)) = self.central
            && central_zone == zone
            && (central_plane == plane || all_levels_visible)
        {
            return false;
        }
        self.central = Some((zone, if all_levels_visible { 0 } else { plane }));
        self.rebuilding = true;
        self.rebuild_cycle_passed = false;
        true
    }

    /// `client.setGameState(25)` without a rebuild packet: `WorldMap.reset`
    /// (WorldMap.java:977-981), which the top-level interface opened with
    /// reset type 2 runs when the world map closes (Protocol.java:1284), and
    /// the `::rebuild` command. The scene is built again from the maps it
    /// already has; `rebuildMap` ends as after a packet, with the marker and
    /// MAP_REBUILD_COMPLETE.
    pub fn rebuild_requested(&mut self) {
        if self.in_session && !self.rebuilding {
            self.rebuilding = true;
            self.rebuild_cycle_passed = false;
        }
    }

    /// A cycle of game state 25 ends without `rebuildMap` having finished,
    /// or the cycle that read the rebuild packet ends (Protocol.java:2414):
    /// from the next cycle on [`Self::rebuild_end`] may come.
    pub fn rebuild_waited(&mut self) {
        self.rebuild_cycle_passed = true;
    }

    /// The end of `LoginManager.rebuildMap` in game state 25
    /// (LoginManager.java:1106-1130): the rebuild marker, written when the
    /// client runs in its own window, which this one always does; game state
    /// 30; MAP_REBUILD_COMPLETE. Both wait in the outgoing buffer for the
    /// write of the `Protocol.loop` that follows in the same cycle.
    pub fn rebuild_end(&mut self) {
        if !self.rebuilding() {
            return;
        }
        self.rebuilding = false;
        self.outbound.push(outgoing::map_rebuild_marker());
        self.outbound.push(outgoing::map_rebuild_complete());
    }

    /// One cycle of game state 25: `LoginManager.rebuildMap`
    /// (LoginManager.java:885-1135) as far as the connection sees it.
    ///
    /// Every call begins with `ping(false)` (886). While the scene is not
    /// built (`scene_ready` false; the original is waiting for map files
    /// and returns at 945 or 978) that is all. The call that builds the
    /// scene follows each of its blocking stages with `ping(true)`,
    /// `keep_alives` of them (1025-1085; see
    /// `underwater::rebuild_keep_alives`), and ends with
    /// [`Self::rebuild_end`]. Each ping that writes takes the whole outgoing
    /// buffer with it: what waits here and the caller's `pending` packets.
    ///
    /// Returns whether the rebuild ended. The caller then resets its timer
    /// (1134) and runs `Protocol.loop` in the same cycle, whose write
    /// carries the marker and MAP_REBUILD_COMPLETE.
    pub fn rebuild_cycle(
        &mut self,
        mut wire: Option<&mut dyn Ping>,
        pending: &mut std::collections::VecDeque<Frame>,
        scene_ready: bool,
        keep_alives: u32,
    ) -> bool {
        let mut ping =
            |events: &mut Self, pending: &mut std::collections::VecDeque<Frame>, force| {
                if let Some(wire) = wire.as_deref_mut() {
                    let waiting: Vec<Frame> = events
                        .outbound
                        .iter()
                        .chain(pending.iter())
                        .cloned()
                        .collect();
                    if wire.ping(force, &waiting) {
                        events.outbound.clear();
                        pending.clear();
                    }
                }
            };
        ping(self, pending, false);
        if !scene_ready {
            self.rebuild_waited();
            return false;
        }
        for _ in 0..keep_alives {
            ping(self, pending, true);
        }
        self.rebuild_end();
        true
    }

    /// `DisplayMode.setWindowMode` was called while the interfaces ran (a
    /// script or a command changed the window mode). With the socket open
    /// and the state 30 or 25 it ends by writing the window status
    /// (DisplayMode.java:273-275), whether or not the mode changed. It
    /// begins by clearing the window-size timer (DisplayMode.java:55), so a
    /// resize still waiting to be reported is covered by this report.
    pub fn window_mode_set(&mut self) {
        self.canvas_replace_time = 0;
        if self.in_session {
            self.window_mode_set = true;
        }
    }

    /// `DisplayMode.setWindowMode` measured the container
    /// (DisplayMode.java:113-120): `GameShell.frameWidth`/`frameHeight` are
    /// this size now, so the next redraw does not take it for a change. That
    /// matters when the container itself changed, on entering and leaving
    /// full screen.
    pub fn frame_measured(&mut self, frame: [i32; 2]) {
        self.frame_size = Some(frame);
    }

    /// The window-size watch of `client.mainRedraw` (client.java:817-842),
    /// once per drawn frame, between cycles.
    ///
    /// A change of the client area starts a 500 ms timer, restarted by every
    /// further change. A redraw in state 30 or 10 that finds the timer run
    /// out calls `DisplayMode.setWindowMode` with the current mode, which
    /// clears the timer and, in a session, writes the window status. In
    /// state 25 the timer waits. With a full-screen frame (mode 3) the size
    /// is not looked at (client.java:822).
    pub fn redraw(&mut self, now_ms: i64, window: &Window) {
        let state_30_or_10 = !self.rebuilding();
        if state_30_or_10 && self.canvas_replace_time != 0 && self.canvas_replace_time < now_ms {
            self.canvas_replace_time = 0;
            if self.in_session {
                self.outbound.push(window.status());
            }
        }
        if window.mode == 3 {
            return;
        }
        match self.frame_size {
            // `GameShell.frameWidth`/`frameHeight` start as the window's
            // size, so the first look is not a change.
            None => self.frame_size = Some(window.frame),
            Some(size) if size != window.frame => {
                self.frame_size = Some(window.frame);
                self.canvas_replace_time = now_ms + WINDOW_REPORT_DELAY_MILLIS;
            }
            Some(_) => {}
        }
    }

    /// `MouseRecorder.run`: called at any rate, records `Mouse.lastMouseX/Y`
    /// (the pointer as the last `Mouse.loop` saw it; -1, -1 outside the
    /// window) once at least 50 ms have passed since the last sample. Like
    /// the original's thread, which sleeps after each sample, it does not
    /// make up missed samples.
    pub fn sample_mouse(&mut self, now_ms: i64, devices: &Devices) {
        if self
            .last_mouse_sample
            .is_some_and(|last| now_ms.wrapping_sub(last) < MOUSE_SAMPLE_MILLIS)
        {
            return;
        }
        self.last_mouse_sample = Some(now_ms);
        self.recorder
            .sample(devices.mouse.last_mouse_x, devices.mouse.last_mouse_y);
    }

    /// One 20 ms cycle from the end of `Keyboard.loop` and `Mouse.loop`
    /// (which the caller has just run as [`Devices::cycle`]) to the point
    /// where `Protocol.loop` turns to the world and the interfaces
    /// (Protocol.java:2566): the end of a scene rebuild in state 25, then
    /// what the loop writes by itself from the input devices. See the module
    /// documentation for the order.
    ///
    /// `Protocol.loop` reads its server packets before this
    /// (Protocol.java:2412); see [`Self::reading_cycles`].
    pub fn cycle_begin(
        &mut self,
        devices: &mut Devices,
        client: &Client,
        shared: &mut Shared<'_>,
    ) -> Cycle {
        let mut cycle = Cycle::default();
        if !self.in_session {
            return cycle;
        }

        if self.rebuilding {
            // The cycle that read the rebuild packet leaves `Protocol.loop`
            // at once (Protocol.java:2414). `rebuildMap` runs from the next
            // cycle on and returns early until the scene is built. The
            // click latched for such a cycle is never looked at.
            if !(self.rebuild_cycle_passed && client.scene_ready) {
                self.rebuild_cycle_passed = true;
                cycle.before = std::mem::take(&mut self.outbound);
                return cycle;
            }
            self.rebuild_end();
        }
        cycle.before = std::mem::take(&mut self.outbound);

        // Protocol.loop in state 30, Protocol.java:2417-2565.
        cycle.playing = true;
        let mouse = &mut devices.mouse;
        cycle
            .before
            .extend(self.recorder.flush(mouse.click_button != 0));
        cycle.before.extend(outgoing::mouse_click_report(
            mouse.click_button,
            mouse.click_x,
            mouse.click_y,
            mouse.click_time,
            &mut mouse.prev_click_time,
        ));
        // The key queue read here was filled late in the previous cycle.
        let camera_keys = if client.keyboard_camera {
            self.key_queue_arrows
        } else {
            [KEY_LEFT, KEY_RIGHT, KEY_UP, KEY_DOWN]
                .iter()
                .any(|&key| devices.keyboard.pressed_keys[key])
        };
        if self.camera.step(camera_keys) {
            cycle.before.push(outgoing::camera_position(
                client.camera[0],
                client.camera[1],
            ));
        }
        // `GameShell.focus`, which `mainLoopWrapper` copies from `focusIn`
        // at the start of each cycle.
        cycle
            .before
            .extend(outgoing::focus_report(&mut self.prev_focus, devices.focus));
        if !*shared.preferences_sent {
            cycle.before.push(outgoing::preferences(client.preferences));
            *shared.preferences_sent = true;
        }
        // Protocol.java:2782-2791: this cycle's typed keys replace the
        // queue (`Devices::keys`).
        self.key_queue_arrows = devices.keys.iter().any(|key| arrow(key[0]));
        cycle
    }

    /// The end of the cycle [`Self::cycle_begin`] began, after the world and
    /// the interfaces have run (Protocol.java:2905-2982): a window status a
    /// script caused, the idle logout and the verification id.
    pub fn cycle_end(
        &mut self,
        cycle: &mut Cycle,
        devices: &mut Devices,
        window: &Window,
        shared: &mut Shared<'_>,
    ) {
        if !self.in_session {
            return;
        }
        if std::mem::take(&mut self.window_mode_set) {
            cycle.interface.push(window.status());
        }
        if !cycle.playing {
            return;
        }
        // Protocol.java:2905-2911: Mouse.getIdleLoops(),
        // Keyboard.getIdleLoops() and Mouse.setIdleLoops(14500).
        let mut mouse_idle = devices.mouse.idle_loops();
        if let Some(frame) = outgoing::idle_logout_report(
            &mut mouse_idle,
            devices.keyboard.idle_loops(),
            shared.logout_timer,
        ) {
            devices.mouse.set_idle_loops(mouse_idle);
            cycle.after.push(frame);
        }
        // Protocol.java:2979-2982.
        if std::mem::take(&mut self.verify_id_changed) {
            cycle.verify_id = Some(outgoing::verify_id(shared.verify_id));
        }
    }

    /// One whole cycle with nothing between its two halves.
    pub fn cycle(
        &mut self,
        devices: &mut Devices,
        client: &Client,
        window: &Window,
        shared: &mut Shared<'_>,
    ) -> Cycle {
        let mut cycle = self.cycle_begin(devices, client, shared);
        self.cycle_end(&mut cycle, devices, window, shared);
        cycle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Transcription tests. Every expectation is traced by hand from the
    // pinned Java named in the comments (`crates/client/tests/server_events.rs`
    // pins the files); none was produced by running the original classes.
    // The bytes of each frame are verified in `rs09_protocol::game::outgoing`
    // and are not tested again here: frames are compared by opcode, or built
    // with the same encoder.

    const WINDOW: Window = Window {
        mode: 2,
        frame: [1280, 720],
        anti_aliasing: 1,
    };

    /// A mouse press as `Mouse.loop` latches it (`clickButton`, `clickX`,
    /// `clickY`, `clickTime`).
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct Click {
        button: i32,
        x: i32,
        y: i32,
        time_ms: i64,
    }

    /// What one test cycle finds in the statics it reads. `mouse_event` and
    /// `key_event` are listener calls made before the cycle's `Mouse.loop`
    /// and `Keyboard.loop`; the other device fields are put into the port's
    /// latched state after them, so that a test can state each fact by
    /// itself. The tests at the end of this module go through the listeners
    /// for all of it.
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    struct Input {
        /// `GameShell.focus`.
        focused: bool,
        /// A mouse event that zeroes `Mouse.idleLoops` arrived.
        mouse_event: bool,
        /// A key was pressed or released.
        key_event: bool,
        click: Option<Click>,
        /// `Keyboard.pressedKeys[96..=99]`.
        arrow_held: bool,
        /// What `Keyboard.nextKey` hands to `InterfaceList.keyCodes`.
        arrow_typed: bool,
        keyboard_camera: bool,
        camera: [i32; 2],
        preferences: i32,
        scene_ready: bool,
    }

    impl Input {
        /// The same state with no new events: what a further cycle sees when
        /// several run without input arriving in between.
        fn held(&self) -> Self {
            Self {
                mouse_event: false,
                key_event: false,
                click: None,
                arrow_typed: false,
                ..*self
            }
        }
    }

    /// The mutable neighbours of one test session.
    struct World {
        events: ClientEvents,
        devices: Devices,
        verify_id: i32,
        preferences_sent: bool,
        logout_timer: i32,
    }

    impl World {
        fn new() -> Self {
            Self {
                events: ClientEvents::default(),
                devices: Devices::default(),
                verify_id: 0,
                preferences_sent: true,
                logout_timer: 0,
            }
        }
        fn start(&mut self, mouse_recording: bool) {
            self.events.session_start(
                mouse_recording,
                &WINDOW,
                &mut self.logout_timer,
                &mut self.devices,
            );
        }
        /// Logged in, the first scene built and the first cycle's frames
        /// (window status, rebuild frames, camera) already sent.
        fn playing() -> Self {
            let mut world = Self::new();
            world.start(false);
            world.cycle(&quiet());
            let first = world.cycle(&quiet());
            assert!(first.playing);
            world
        }
        /// `[Mouse.idleLoops, Keyboard.idleLoops]`.
        fn idle_loops(&self) -> [i32; 2] {
            self.devices.idle_loops()
        }
        /// Puts both idle counters where a test wants them. The keyboard's
        /// has no setter in the original either: an unmapped key's release
        /// zeroes it and `Keyboard.loop` counts it up.
        fn idle(&mut self, mouse: i32, keyboard: i32) {
            self.devices.keyboard.key_released(0);
            for _ in 0..keyboard {
                self.devices.keyboard.cycle();
            }
            self.devices.mouse.set_idle_loops(mouse);
        }
        /// A mouse recorder sample with `Mouse.lastMouseX/Y` at `position`.
        fn sample(&mut self, now_ms: i64, position: [i32; 2]) {
            [
                self.devices.mouse.last_mouse_x,
                self.devices.mouse.last_mouse_y,
            ] = position;
            self.events.sample_mouse(now_ms, &self.devices);
        }
        fn cycle(&mut self, input: &Input) -> Cycle {
            let devices = &mut self.devices;
            if input.mouse_event {
                let [x, y] = [devices.mouse.current_mouse_x, devices.mouse.current_mouse_y];
                devices.mouse.mouse_moved(x, y);
            }
            if input.key_event {
                devices.keyboard.key_released(0);
            }
            devices.focus = input.focused;
            devices.cycle();
            match input.click {
                Some(click) => {
                    devices.mouse.click_button = click.button;
                    devices.mouse.click_x = click.x;
                    devices.mouse.click_y = click.y;
                    devices.mouse.click_time = click.time_ms;
                }
                None => devices.mouse.click_button = 0,
            }
            devices.keyboard.pressed_keys[KEY_LEFT] = input.arrow_held;
            devices.keys.clear();
            if input.arrow_typed {
                devices.keys.push([KEY_UP as i32, -1]);
            }
            self.events.cycle(
                devices,
                &Client {
                    keyboard_camera: input.keyboard_camera,
                    camera: input.camera,
                    preferences: input.preferences,
                    scene_ready: input.scene_ready,
                },
                &WINDOW,
                &mut Shared {
                    verify_id: self.verify_id,
                    preferences_sent: &mut self.preferences_sent,
                    logout_timer: &mut self.logout_timer,
                },
            )
        }
        /// `cycles` cycles run back to back, as after a late frame: the
        /// first sees `input`, the others [`Input::held`].
        fn cycles(&mut self, cycles: u32, input: &Input) -> Vec<Cycle> {
            (0..cycles)
                .map(|index| {
                    if index == 0 {
                        self.cycle(input)
                    } else {
                        self.cycle(&input.held())
                    }
                })
                .collect()
        }
        fn opcodes(&mut self, input: &Input) -> Vec<u8> {
            opcodes(self.cycle(input))
        }
    }

    /// A focused window with nothing happening and the scene built.
    fn quiet() -> Input {
        Input {
            focused: true,
            scene_ready: true,
            camera: [128, 0],
            ..Input::default()
        }
    }

    /// A cycle's opcodes as written, with no interface packets.
    fn opcodes(cycle: Cycle) -> Vec<u8> {
        let (frames, verify) = cycle.written([]);
        frames.iter().chain(&verify).map(|f| f.opcode).collect()
    }

    fn click(button: i32, time_ms: i64) -> Option<Click> {
        Some(Click {
            button,
            x: 400,
            y: 300,
            time_ms,
        })
    }

    #[test]
    fn tracked_opcodes_are_the_handlers_that_set_the_verify_id() {
        assert!(TRACKED_SERVER_OPCODES.windows(2).all(|w| w[0] < w[1]));
        // The rebuild packets, the player and NPC updates and the varp
        // packets carry no tracking number.
        for opcode in [162, 214, 225, 32, 60, 226] {
            assert!(!TRACKED_SERVER_OPCODES.contains(&opcode));
        }
        // ServerProt: IF_OPENSUB 145, IF_CLOSESUB 149, IF_OPENTOP 155,
        // RUN_CS2 115, CAM_RESET 24.
        for opcode in [145, 149, 155, 115, 24] {
            assert!(TRACKED_SERVER_OPCODES.contains(&opcode));
        }
    }

    #[test]
    fn a_session_starts_with_the_window_status_then_the_rebuild_frames() {
        // client.resetGameSessionState ends with sendWindowDetails
        // (client.java:754); readRebuildPacket then enters state 25
        // (LoginManager.java:539-541).
        let mut world = World::new();
        world.logout_timer = 99;
        world.start(false);
        assert_eq!(world.logout_timer, 0);
        assert!(world.events.rebuilding());

        // The cycle after the login: `rebuildMap` has not run yet. The
        // window status is the only thing in the buffer.
        let first = world.cycle(&quiet());
        assert!(!first.playing);
        assert_eq!(first.before, [WINDOW.status()]);
        assert_eq!(opcodes(first), [243]);

        // The scene is not built: nothing is written, however long it takes.
        let waiting = Input {
            scene_ready: false,
            ..quiet()
        };
        for _ in 0..200 {
            assert_eq!(world.cycle(&waiting), Cycle::default());
        }
        assert!(world.events.rebuilding());

        // Built: the marker and the completion (LoginManager.java:1106-1130)
        // lead the write, and `Protocol.loop` runs in the same cycle. The
        // session's first camera report follows (cameraPositionChanged was
        // set by the reset, client.java:747).
        let done = world.cycle(&quiet());
        assert!(done.playing);
        assert_eq!(
            done.before,
            [
                outgoing::map_rebuild_marker(),
                outgoing::map_rebuild_complete(),
                outgoing::camera_position(128, 0),
            ]
        );
        assert_eq!(opcodes(done), [20, 110, 21]);
        assert!(!world.events.rebuilding());
        assert_eq!(world.opcodes(&quiet()), [] as [u8; 0]);
    }

    #[test]
    fn nothing_is_written_outside_a_session_but_the_idle_counters_run() {
        let mut world = World::new();
        let busy = Input {
            click: click(1, 5000),
            arrow_held: true,
            focused: false,
            ..quiet()
        };
        for _ in 0..10 {
            assert_eq!(world.cycle(&busy), Cycle::default());
        }
        assert_eq!(world.idle_loops(), [10, 10]);
        world.events.server_frame(145);
        world.events.window_mode_set();
        world.events.redraw(0, &WINDOW);
        assert_eq!(world.cycle(&busy), Cycle::default());
    }

    #[test]
    fn one_cycle_writes_its_frames_in_the_order_of_protocol_loop() {
        let mut world = World::playing();
        world.events.recorder.enabled = true;
        world.sample(0, [10, 20]);
        world.events.server_frame(145);
        world.events.window_mode_set();
        world.verify_id = 77;
        world.preferences_sent = false;
        // Both idle counters one cycle short of the limit.
        world.idle(15000, 15000);
        // The session's first camera report started the cooldown; let it
        // run out.
        for _ in 0..20 {
            world.events.camera.step(false);
        }
        let cycle = world.cycle(&Input {
            // A click does not reach the idle counters by itself here: the
            // press that latched it is a `mouse_event`. Left out, so that
            // the idle logout shares the cycle.
            click: click(2, 123_456),
            arrow_held: true,
            focused: false,
            preferences: 24754171,
            camera: [300, 1024],
            ..quiet()
        });
        assert!(cycle.playing);
        // Protocol.java:2430 (123), 2526 (75), 2547 (21), 2558 (22),
        // 2562 (98).
        assert_eq!(
            cycle.before.iter().map(|f| f.opcode).collect::<Vec<_>>(),
            [123, 75, 21, 22, 98]
        );
        assert_eq!(cycle.before[2], outgoing::camera_position(300, 1024));
        assert_eq!(cycle.before[3], outgoing::focus(false));
        assert_eq!(cycle.before[4], outgoing::preferences(24754171));
        assert_eq!(cycle.interface, [WINDOW.status()]);
        // Protocol.java:2910.
        assert_eq!(cycle.after, [outgoing::idle_logout()]);
        // Protocol.java:2979-2982, after the keep-alive test at 2976.
        assert_eq!(cycle.verify_id, Some(outgoing::verify_id(77)));
        assert!(world.preferences_sent);
        assert_eq!(world.logout_timer, 250);

        // The caller's interface and menu packets go between the two
        // groups; a script's window status follows them.
        let menu = Frame {
            opcode: 155,
            payload: vec![1, 2, 3, 4, 5, 6],
        };
        let (frames, verify) = cycle.written([menu]);
        assert_eq!(
            frames.iter().map(|f| f.opcode).collect::<Vec<_>>(),
            [123, 75, 21, 22, 98, 155, 243, 245]
        );
        assert_eq!(verify.map(|f| f.opcode), Some(177));

        // Each of those was a one-off.
        assert_eq!(
            world.opcodes(&Input {
                focused: false,
                ..quiet()
            }),
            [] as [u8; 0]
        );
    }

    #[test]
    fn the_camera_is_reported_at_most_every_twenty_cycles() {
        // Protocol.java:2530-2550, default preferences: an arrow key held.
        let mut world = World::playing();
        let held = Input {
            arrow_held: true,
            ..quiet()
        };
        // `World::playing` ended with the session's own report, which set
        // the cooldown to 20: under a held key the next report is the 20th
        // cycle from here, then every 20th.
        let sent: Vec<usize> = (1..=100).filter(|_| world.opcodes(&held) == [21]).collect();
        assert_eq!(sent, [20, 40, 60, 80, 100]);
        // Released: the report that was due is the last one.
        let after: Vec<usize> = (1..=60)
            .filter(|_| world.opcodes(&quiet()) == [21])
            .collect();
        assert_eq!(after, [] as [usize; 0]);
        // A single cycle with a key down, cooldown run out: reported at once.
        assert_eq!(world.opcodes(&held), [21]);
        // During the cooldown the change is remembered and sent when the
        // cooldown ends, 20 cycles after the previous report.
        assert_eq!(world.opcodes(&held), [] as [u8; 0]);
        let later: Vec<usize> = (2..=40)
            .filter(|_| world.opcodes(&quiet()) == [21])
            .collect();
        assert_eq!(later, [20]);
    }

    #[test]
    fn the_keyboard_camera_preference_reads_the_previous_cycles_key_queue() {
        // Protocol.java:2533-2540 reads InterfaceList.keyCodes, which the
        // same loop refills later (2782-2791): an arrow key typed before
        // cycle n is seen by the camera test of cycle n + 1.
        let mut world = World::playing();
        for _ in 0..30 {
            world.cycle(&quiet());
        }
        let preference = Input {
            keyboard_camera: true,
            ..quiet()
        };
        // Held keys mean nothing with the preference set.
        let held = Input {
            arrow_held: true,
            ..preference
        };
        assert_eq!(world.opcodes(&held), [] as [u8; 0]);
        let typed = Input {
            arrow_typed: true,
            ..preference
        };
        assert_eq!(world.opcodes(&typed), [] as [u8; 0]);
        assert_eq!(world.opcodes(&preference), [21]);
        assert_eq!(world.opcodes(&preference), [] as [u8; 0]);
    }

    #[test]
    fn idle_logout_comes_after_15001_idle_cycles_and_again_501_later() {
        // Mouse.loop and Keyboard.loop count every cycle; Protocol.java:2907
        // tests `> 15000`. Both counters start from 0 here; in the original
        // only the mouse counter is zeroed by resetGameSessionState
        // (client.java:653), the keyboard counter has run since start-up.
        let mut world = World::new();
        world.start(false);
        let mut sent = vec![];
        for cycle in 1..=16_100 {
            let frames = world.cycle(&quiet());
            if !frames.after.is_empty() {
                assert_eq!(frames.after, [outgoing::idle_logout()]);
                assert_eq!(world.logout_timer, 250);
                // Mouse.setIdleLoops(14500), Protocol.java:2909.
                assert_eq!(world.idle_loops()[0], 14500);
                world.logout_timer = 0;
                sent.push(cycle);
            }
        }
        assert_eq!(sent, [15_001, 15_502, 16_003]);
        // The keyboard counter was never put back.
        assert_eq!(world.idle_loops()[1], 16_100);
    }

    #[test]
    fn any_event_of_a_device_restarts_its_idle_count() {
        let mut world = World::playing();
        world.idle(15_000, 20_000);
        // A mouse event arrived before this cycle: the counter is 1 at the
        // test (zeroed by the event, counted by Mouse.loop).
        let moved = Input {
            mouse_event: true,
            ..quiet()
        };
        assert_eq!(world.cycle(&moved).after, []);
        assert_eq!(world.idle_loops(), [1, 20_001]);
        // Only the keyboard idle: both must exceed the limit.
        world.idle(15_000, 14_999);
        assert_eq!(world.cycle(&quiet()).after, []);
        assert_eq!(world.idle_loops(), [15_001, 15_000]);
        assert_eq!(world.cycle(&quiet()).after, [outgoing::idle_logout()]);
        // A key event keeps the logout away whatever the mouse does.
        world.devices.mouse.set_idle_loops(15_000);
        let key = Input {
            key_event: true,
            ..quiet()
        };
        assert_eq!(world.cycle(&key).after, []);
        assert_eq!(world.idle_loops(), [15_001, 1]);
    }

    #[test]
    fn the_verify_id_is_sent_once_per_cycle_and_only_when_marked() {
        // Protocol.setVerifyId marks (Protocol.java:3289-3293); the end of
        // the loop sends once and clears (2979-2982).
        let mut world = World::playing();
        assert_eq!(world.cycle(&quiet()).verify_id, None);
        // An untracked packet does not mark.
        world.events.server_frame(225);
        assert_eq!(world.cycle(&quiet()).verify_id, None);
        // Three tracked packets read in one cycle: one frame, with the id
        // the last of them left.
        for opcode in [145, 171, 115] {
            world.events.server_frame(opcode);
        }
        world.verify_id = 4321;
        assert_eq!(
            world.cycle(&quiet()).verify_id,
            Some(outgoing::verify_id(4321))
        );
        assert_eq!(world.cycle(&quiet()).verify_id, None);
        // Marked again with the same id: sent again. The flag, not a
        // change of the value, decides.
        world.events.server_frame(21);
        assert_eq!(
            world.cycle(&quiet()).verify_id,
            Some(outgoing::verify_id(4321))
        );
    }

    #[test]
    fn a_rebuild_holds_the_loop_and_ends_with_its_two_frames() {
        let mut world = World::playing();
        // First scene: zone 400,400 plane 0, hardware renderer.
        world.events.central = Some(([400, 400], 0));
        // LoginManager.loadRegion: the same zone starts nothing, whatever
        // the plane, when all levels are visible.
        assert!(!world.events.rebuild_packet([400, 400], 1, true));
        assert!(!world.events.rebuilding());
        // The software renderer rebuilds for another plane.
        assert!(world.events.rebuild_packet([400, 400], 1, false));
        assert!(world.events.rebuilding());
        assert!(!world.events.rebuild_packet([400, 400], 1, false));
        let mut world = World::playing();
        assert!(world.events.rebuild_packet([408, 400], 0, true));

        // The cycle that read the packet: `Protocol.loop` returns before
        // writing anything (Protocol.java:2414), even with the scene at
        // hand. Its click is lost; a tracked packet read in it stays marked.
        world.events.server_frame(145);
        world.verify_id = 9;
        let read = world.cycle(&Input {
            click: click(1, 1000),
            arrow_held: true,
            ..quiet()
        });
        assert_eq!(read, Cycle::default());
        // The next cycle: `rebuildMap` finishes, then `Protocol.loop` runs.
        // The arrow key held during the skipped cycle was never seen.
        let done = world.cycle(&quiet());
        assert!(done.playing);
        assert_eq!(
            done.before,
            [
                outgoing::map_rebuild_marker(),
                outgoing::map_rebuild_complete()
            ]
        );
        assert_eq!(done.verify_id, Some(outgoing::verify_id(9)));
        // The camera cooldown (20 after `World::playing`) did not run in
        // the skipped cycle: it was 19 after `done`, so a key held from
        // here is reported in the 19th cycle. Had it run, the 18th.
        let held = Input {
            arrow_held: true,
            ..quiet()
        };
        let camera: Vec<usize> = (1..=30).filter(|_| world.opcodes(&held) == [21]).collect();
        assert_eq!(camera, [19]);
    }

    #[test]
    fn the_idle_counters_run_during_a_rebuild_but_nothing_is_reported() {
        let mut world = World::playing();
        world.idle(14_990, 14_990);
        assert!(world.events.rebuild_packet([1, 1], 0, true));
        let waiting = Input {
            scene_ready: false,
            ..quiet()
        };
        for _ in 0..100 {
            assert_eq!(world.cycle(&waiting), Cycle::default());
        }
        assert_eq!(world.idle_loops(), [15_090, 15_090]);
        // The first cycle back in state 30 reports it.
        assert_eq!(world.opcodes(&quiet()), [20, 110, 245]);
    }

    #[test]
    fn clicks_report_the_time_since_the_previous_click_of_the_session() {
        // Protocol.java:2503-2529; Mouse.prevClickTime is 0 after the reset.
        let mut world = World::playing();
        let first = world.cycle(&Input {
            click: click(1, 1_000_000),
            ..quiet()
        });
        let mut prev = 0;
        assert_eq!(
            first.before,
            [outgoing::mouse_click_report(1, 400, 300, 1_000_000, &mut prev).unwrap()]
        );
        let second = world.cycle(&Input {
            click: click(2, 1_000_250),
            ..quiet()
        });
        assert_eq!(
            second.before,
            [outgoing::mouse_click_report(2, 400, 300, 1_000_250, &mut prev).unwrap()]
        );
        // No click, no frame; several cycles in one frame see it once.
        let cycles = world.cycles(
            3,
            &Input {
                click: click(1, 1_000_500),
                mouse_event: true,
                ..quiet()
            },
        );
        let per_cycle: Vec<Vec<u8>> = cycles.into_iter().map(opcodes).collect();
        assert_eq!(per_cycle, [vec![75], vec![], vec![]]);
        // The event zeroed the idle count once; three cycles counted.
        assert_eq!(world.idle_loops()[0], 3);
    }

    #[test]
    fn focus_is_reported_on_each_change_and_assumed_at_session_start() {
        // Protocol.java:2551-2560; prevFocus is true after the reset.
        let mut world = World::playing();
        let away = Input {
            focused: false,
            ..quiet()
        };
        assert_eq!(world.cycle(&away).before, [outgoing::focus(false)]);
        assert_eq!(world.cycle(&away).before, []);
        assert_eq!(world.cycle(&quiet()).before, [outgoing::focus(true)]);
        // A new session while unfocused reports the loss again.
        world.cycle(&away);
        world.start(false);
        assert_eq!(world.opcodes(&away), [243]);
        // The reset marks the camera as changed but leaves the cooldown,
        // which only `Protocol.loop` counts down, where the last session
        // left it: 16 here (20 at the first report, four cycles since).
        // The new session's camera report waits for it.
        assert_eq!(world.opcodes(&away), [20, 110, 22]);
        let camera: Vec<usize> = (1..=20).filter(|_| world.opcodes(&away) == [21]).collect();
        assert_eq!(camera, [15]);
    }

    #[test]
    fn the_mouse_recorder_follows_the_login_flag() {
        // Off (the pinned server sends 0): samples are dropped unsent.
        let mut world = World::playing();
        for sample in 0..60 {
            world.sample(sample * 50, [5, 5]);
        }
        assert_eq!(world.events.recorder.pending(), 60);
        assert_eq!(world.opcodes(&quiet()), [] as [u8; 0]);
        assert_eq!(world.events.recorder.pending(), 0);

        // On: sent with 40 samples waiting or on a click, before the click.
        let mut world = World::new();
        world.start(true);
        world.cycle(&quiet());
        world.cycle(&quiet());
        for sample in 0..39 {
            world.sample(sample * 50, [5, 5]);
        }
        assert_eq!(world.opcodes(&quiet()), [] as [u8; 0]);
        // Too soon after the last sample: not recorded.
        world.sample(38 * 50 + 49, [6, 6]);
        assert_eq!(world.events.recorder.pending(), 39);
        world.sample(39 * 50, [6, 6]);
        assert_eq!(world.opcodes(&quiet()), [123]);
        world.sample(40 * 50, [7, 7]);
        assert_eq!(
            world.opcodes(&Input {
                click: click(1, 9000),
                ..quiet()
            }),
            [123, 75]
        );
        // A late call records one sample, not the ones it missed.
        world.sample(100 * 50, [8, 8]);
        assert_eq!(world.events.recorder.pending(), 1);
        // A new session drops what is waiting.
        world.start(true);
        assert_eq!(world.events.recorder.pending(), 0);
    }

    #[test]
    fn a_resized_window_is_reported_half_a_second_after_the_last_change() {
        // client.java:817-842.
        let mut world = World::playing();
        let mut window = WINDOW;
        world.events.redraw(1000, &window);
        // First look: no change. Nothing pending.
        world.events.redraw(2000, &window);
        assert_eq!(world.opcodes(&quiet()), [] as [u8; 0]);
        window.frame = [1000, 700];
        world.events.redraw(3000, &window);
        // Still being dragged at 3400: the timer restarts.
        window.frame = [900, 650];
        world.events.redraw(3400, &window);
        world.events.redraw(3600, &window);
        assert_eq!(world.opcodes(&quiet()), [] as [u8; 0]);
        // `canvasReplaceTime < now` is strict.
        world.events.redraw(3900, &window);
        assert_eq!(world.opcodes(&quiet()), [] as [u8; 0]);
        world.events.redraw(3901, &window);
        let cycle = world.cycle(&quiet());
        assert_eq!(cycle.before, [window.status()]);
        // Once.
        world.events.redraw(5000, &window);
        assert_eq!(world.opcodes(&quiet()), [] as [u8; 0]);

        // During a rebuild (state 25) the redraw leaves the timer alone;
        // the report follows the rebuild.
        window.frame = [800, 600];
        world.events.redraw(6000, &window);
        assert!(world.events.rebuild_packet([3, 3], 0, true));
        world.events.redraw(7000, &window);
        assert_eq!(world.cycle(&quiet()), Cycle::default());
        assert_eq!(world.opcodes(&quiet()), [20, 110]);
        world.events.redraw(7040, &window);
        assert_eq!(world.cycle(&quiet()).before, [window.status()]);
    }

    #[test]
    fn setting_the_window_mode_covers_a_resize_still_waiting() {
        // DisplayMode.java:55 zeroes canvasReplaceTime before it reports
        // (273-275), so the resize is reported once, by the mode change.
        let mut world = World::playing();
        let mut window = WINDOW;
        world.events.redraw(1000, &window);
        window.frame = [1000, 700];
        world.events.redraw(2000, &window);
        world.events.window_mode_set();
        // (The harness writes its own window's status.)
        assert_eq!(world.opcodes(&quiet()), [243]);
        world.events.redraw(9000, &window);
        assert_eq!(world.opcodes(&quiet()), [] as [u8; 0]);
    }

    #[test]
    fn full_screen_is_not_watched_and_its_frame_is_not_a_resize() {
        // client.java:822: no size check with a full-screen frame.
        // DisplayMode.java:113-114: setWindowMode itself stores the size of
        // the container it ends with, entering and leaving.
        let mut world = World::playing();
        let windowed = WINDOW;
        let full = Window {
            mode: 3,
            frame: [1920, 1080],
            ..WINDOW
        };
        world.events.redraw(1000, &windowed);
        // CS2 5300: one report, from setWindowMode.
        world.events.window_mode_set();
        world.events.frame_measured(full.frame);
        assert_eq!(world.opcodes(&quiet()), [243]);
        world.events.redraw(2000, &full);
        world.events.redraw(9000, &full);
        assert_eq!(world.opcodes(&quiet()), [] as [u8; 0]);
        // CS2 5301 back to the window as it was.
        world.events.window_mode_set();
        world.events.frame_measured(windowed.frame);
        assert_eq!(world.opcodes(&quiet()), [243]);
        world.events.redraw(10000, &windowed);
        world.events.redraw(19000, &windowed);
        assert_eq!(world.opcodes(&quiet()), [] as [u8; 0]);
    }

    #[test]
    fn window_status_reports_the_canvas_of_the_mode() {
        // DisplayMode.java:121-131, GameShell.java:201-211.
        let window = |mode| Window {
            mode,
            frame: [1920, 1017],
            anti_aliasing: 2,
        };
        assert_eq!(window(3).canvas(), [1920, 1017]);
        assert_eq!(window(2).canvas(), [1920, 1017]);
        assert_eq!(window(1).canvas(), [765, 503]);
        assert_eq!(window(0).canvas(), [765, 503]);
        assert_eq!(
            window(2).status(),
            outgoing::window_status(2, 1920, 1017, 2)
        );
        assert_eq!(window(1).status(), outgoing::window_status(1, 765, 503, 2));
    }

    #[test]
    fn a_window_mode_set_by_a_script_is_reported_in_either_game_state() {
        // DisplayMode.java:273-275: socket open and state 30 or 25.
        let mut world = World::playing();
        world.events.window_mode_set();
        world.events.window_mode_set();
        let cycle = world.cycle(&quiet());
        assert_eq!(cycle.interface, [WINDOW.status()]);
        assert_eq!(world.cycle(&quiet()).interface, []);
        assert!(world.events.rebuild_packet([9, 9], 0, true));
        world.events.window_mode_set();
        let held = world.cycle(&quiet());
        assert!(!held.playing);
        assert_eq!(opcodes(held), [243]);
    }

    #[test]
    fn a_reconnect_empties_the_buffer_and_reports_the_window() {
        // LoginManager.java:830-862.
        let mut world = World::playing();
        world.events.window_mode_set();
        world.events.server_frame(145);
        world.events.session_end();
        assert_eq!(world.cycle(&quiet()), Cycle::default());
        world.events.reconnected(&WINDOW);
        let cycle = world.cycle(&quiet());
        assert!(cycle.playing);
        assert_eq!(cycle.before, [WINDOW.status()]);
        assert_eq!(cycle.interface, []);
        // verifyIdChanged is not part of what a reconnect resets.
        assert!(cycle.verify_id.is_some());
    }

    #[test]
    fn preferences_pack_like_the_original_defaults() {
        // The default preferences of both clients; the value is the one
        // `outgoing` verified against Preferences.toInt on the JVM.
        assert_eq!(packed_preferences(&Preferences::default()), 24754171);
        let quiet = Preferences {
            sound_volume: 0,
            music_volume: 0,
            ambient_volume: 0,
            ..Preferences::default()
        };
        assert_eq!(
            packed_preferences(&quiet),
            24754171 - (1 << 20) - (1 << 21) - (1 << 22)
        );
    }

    #[test]
    fn a_rebuild_without_a_packet_ends_with_the_same_two_frames() {
        // WorldMap.reset (WorldMap.java:977-981) sets state 25 from inside
        // the packet loop; the cycle returns at Protocol.java:2414 and the
        // next one runs rebuildMap to its end (LoginManager.java:1106-1130).
        let mut world = World::playing();
        world.events.central = Some(([400, 400], 0));
        world.events.rebuild_requested();
        assert!(world.events.rebuilding());
        assert_eq!(world.cycle(&quiet()), Cycle::default());
        assert_eq!(world.opcodes(&quiet()), [20, 110]);
        assert!(!world.events.rebuilding());
        // loadRegion was not involved: the central zone is the same.
        assert!(!world.events.rebuild_packet([400, 400], 0, true));
        // Without a session there is no state 25 to enter.
        let mut out = World::new();
        out.events.rebuild_requested();
        assert!(!out.events.rebuilding());
    }

    #[test]
    fn each_cycle_reads_its_own_hundred_packets_and_none_in_state_25() {
        // Protocol.java:2412: `for (i = 0; i < 100 && readPacket(); i++)`,
        // once per cycle; a frame that owes three cycles reads 100, 100 and
        // the rest, each batch in its own cycle.
        let packet = |opcode: u8| Frame {
            opcode,
            payload: vec![],
        };
        // REBUILD_NORMAL for a zone, with no region keys.
        let rebuild = |zone_x: u8| Frame {
            opcode: 162,
            payload: vec![0, 0, 0, 1, zone_x, 0, 0, 0, 0],
        };
        let mut world = World::playing();
        let mut backlog = Backlog::default();
        let incoming: Vec<Frame> = (0..250).map(|_| packet(86)).collect();
        assert_eq!(backlog.read(&mut world.events, incoming).len(), 100);
        assert_eq!(backlog.read(&mut world.events, vec![]).len(), 100);
        assert_eq!(backlog.read(&mut world.events, vec![]).len(), 50);
        assert!(backlog.read(&mut world.events, vec![]).is_empty());

        // The hundred that hold a rebuild packet are read to their end: the
        // packet loop does not look at the game state (2412-2414).
        let mut incoming: Vec<Frame> = (0..130).map(|_| packet(86)).collect();
        incoming[30] = rebuild(144);
        let read = backlog.read(&mut world.events, incoming);
        assert_eq!(read.len(), 100);
        assert!(world.events.rebuilding());
        // Game state 25 runs `rebuildMap`, not `Protocol.loop`: the thirty
        // left wait, however many cycles pass.
        assert!(backlog.read(&mut world.events, vec![packet(86)]).is_empty());
        world.events.rebuild_end();
        assert_eq!(backlog.read(&mut world.events, vec![]).len(), 31);

        // A session's first packet is the login's own and comes alone.
        let mut world = World::new();
        world.start(false);
        let mut backlog = Backlog::default();
        backlog.expect_login_packet();
        let read = backlog.read(
            &mut world.events,
            vec![rebuild(145), packet(86), packet(86)],
        );
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].opcode, 162);
        assert!(world.events.rebuilding());
        assert!(backlog.read(&mut world.events, vec![]).is_empty());
        world.events.rebuild_end();
        assert_eq!(backlog.read(&mut world.events, vec![]).len(), 2);
    }

    #[test]
    fn packets_are_read_only_by_cycles_that_run_protocol_loop() {
        // client.java:1725-1734: in state 25 a cycle runs rebuildMap and
        // then, only if that reached state 30, Protocol.loop with its
        // packet loop (Protocol.java:2412).
        let mut world = World::new();
        assert_eq!(world.events.reading_cycles(3, false), 3);
        world.start(false);
        // The cycle after the rebuild packet is the first to run rebuildMap.
        assert_eq!(world.events.reading_cycles(1, true), 0);
        assert_eq!(world.events.reading_cycles(3, true), 2);
        assert_eq!(world.events.reading_cycles(3, false), 0);
        let waiting = Input {
            scene_ready: false,
            ..quiet()
        };
        world.cycle(&waiting);
        assert_eq!(world.events.reading_cycles(1, false), 0);
        assert_eq!(world.events.reading_cycles(1, true), 1);
        assert!(world.cycle(&quiet()).playing);
        assert_eq!(world.events.reading_cycles(2, false), 2);
    }

    // The joined path. Window events go through `input::platform::Translator`
    // (the calls AWT made on the original's listeners) into the ports of
    // `Mouse` and `Keyboard`; each cycle is `Devices::cycle` (`Keyboard.loop`,
    // `Mouse.loop`) and then this module, as in `client.mainLoop`. The ports
    // are compared with the unchanged classes on a JVM by `tests/input.rs`;
    // what these tests add, by transcription, is that `Protocol.loop`'s
    // reads of their statics are the ones made here.

    use crate::input::platform::{Host, Translator};
    use macroquad::miniquad::{KeyCode, MouseButton};

    const READY: Client = Client {
        keyboard_camera: false,
        camera: [128, 0],
        preferences: 0,
        scene_ready: true,
    };

    impl World {
        /// One cycle of `client.mainLoop` over the ports as they are.
        fn real(&mut self, client: &Client) -> Cycle {
            self.devices.cycle();
            self.events.cycle(
                &mut self.devices,
                client,
                &WINDOW,
                &mut Shared {
                    verify_id: self.verify_id,
                    preferences_sent: &mut self.preferences_sent,
                    logout_timer: &mut self.logout_timer,
                },
            )
        }
        fn real_opcodes(&mut self, client: &Client) -> Vec<u8> {
            opcodes(self.real(client))
        }
    }

    #[test]
    fn a_press_reaches_the_click_report_through_the_mouse_port() {
        // Mouse.mousePressed fills the pending click, Mouse.loop latches it
        // for one cycle, Protocol.java:2503-2529 reports it.
        let mut world = World::playing();
        let mut window = Translator::new(Host::Native);
        window.mouse_motion(&mut world.devices, 400, 300);
        window.mouse_down(&mut world.devices, MouseButton::Left, 400, 300, 1_000_000);
        let first = world.real(&READY);
        let mut prev = 0;
        assert_eq!(
            first.before,
            [outgoing::mouse_click_report(1, 400, 300, 1_000_000, &mut prev).unwrap()]
        );
        assert_eq!(world.idle_loops()[0], 1);
        // Latched for that cycle only.
        assert_eq!(world.real_opcodes(&READY), [] as [u8; 0]);
        // The right button is button 2; the elapsed time is counted from
        // Mouse.prevClickTime, which Protocol.loop left at the first click.
        window.mouse_up(&mut world.devices, MouseButton::Left);
        window.mouse_down(&mut world.devices, MouseButton::Right, 10, 20, 1_000_250);
        let second = world.real(&READY);
        assert_eq!(
            second.before,
            [outgoing::mouse_click_report(2, 10, 20, 1_000_250, &mut prev).unwrap()]
        );
        assert_eq!(world.devices.mouse.prev_click_time, 1_000_250);
        // Two presses between two cycles: the pending click is the later.
        window.mouse_up(&mut world.devices, MouseButton::Right);
        window.mouse_down(&mut world.devices, MouseButton::Left, 1, 1, 1_000_300);
        window.mouse_up(&mut world.devices, MouseButton::Left);
        window.mouse_down(&mut world.devices, MouseButton::Left, 2, 2, 1_000_400);
        let third = world.real(&READY);
        assert_eq!(
            third.before,
            [outgoing::mouse_click_report(1, 2, 2, 1_000_400, &mut prev).unwrap()]
        );
        // The middle button is no click.
        window.mouse_up(&mut world.devices, MouseButton::Left);
        window.mouse_down(&mut world.devices, MouseButton::Middle, 5, 5, 1_000_500);
        assert_eq!(world.real_opcodes(&READY), [] as [u8; 0]);
    }

    #[test]
    fn arrow_keys_and_focus_reach_their_reports_through_the_keyboard_port() {
        let mut world = World::playing();
        let mut window = Translator::new(Host::Native);
        // The session's first camera report started the cooldown.
        for _ in 0..30 {
            assert_eq!(world.real_opcodes(&READY), [] as [u8; 0]);
        }
        // Keyboard.keyPressed queues the press, Keyboard.loop sets
        // pressedKeys[96], Protocol.java:2541 reads it.
        window.key_down(&mut world.devices, KeyCode::Left);
        assert_eq!(world.real_opcodes(&READY), [21]);
        let held: Vec<usize> = (1..=40)
            .filter(|_| world.real_opcodes(&READY) == [21])
            .collect();
        assert_eq!(held, [20, 40]);
        window.key_up(&mut world.devices, KeyCode::Left);
        let released: Vec<usize> = (1..=40)
            .filter(|_| world.real_opcodes(&READY) == [21])
            .collect();
        assert_eq!(released, [] as [usize; 0]);
        // Any other key changes nothing.
        window.key_down(&mut world.devices, KeyCode::A);
        assert_eq!(world.real_opcodes(&READY), [] as [u8; 0]);
        window.key_up(&mut world.devices, KeyCode::A);
        // Keyboard.focusLost: the next Keyboard.loop releases every key, so
        // the camera stops being marked; GameShell.focus is reported
        // (Protocol.java:2551-2560).
        window.key_down(&mut world.devices, KeyCode::Right);
        assert_eq!(world.real_opcodes(&READY), [21]);
        window.focus(&mut world.devices, false);
        assert_eq!(world.real(&READY).before, [outgoing::focus(false)]);
        for _ in 0..40 {
            assert_eq!(world.real_opcodes(&READY), [] as [u8; 0]);
        }
        window.focus(&mut world.devices, true);
        assert_eq!(world.real(&READY).before, [outgoing::focus(true)]);
        // With the keyboard-camera preference the test is the key queue
        // (`Keyboard.nextKey`), read a cycle after it is filled
        // (Protocol.java:2533-2540, 2787-2791); held keys count for nothing.
        let preference = Client {
            keyboard_camera: true,
            ..READY
        };
        window.key_down(&mut world.devices, KeyCode::Up);
        assert_eq!(world.real_opcodes(&preference), [] as [u8; 0]);
        assert_eq!(world.devices.keys, [[KEY_UP as i32, -1]]);
        assert_eq!(world.real_opcodes(&preference), [21]);
        for _ in 0..30 {
            assert_eq!(world.real_opcodes(&preference), [] as [u8; 0]);
        }
        assert!(world.devices.keyboard.pressed_keys[KEY_UP]);
    }

    #[test]
    fn the_idle_logout_follows_the_ports_idle_counters_and_their_exceptions() {
        // Protocol.java:2905-2911 over Mouse.getIdleLoops and
        // Keyboard.getIdleLoops. What does not zero them is the ports'
        // (Mouse.java, Keyboard.java): a typed character, a press or a drag
        // with the middle button. The wheel has no listener in `Mouse`.
        let mut world = World::playing();
        let mut window = Translator::new(Host::Native);
        world.idle(15_000, 15_000);
        window.character(&mut world.devices, 'a');
        window.mouse_down(&mut world.devices, MouseButton::Middle, 3, 3, 0);
        assert_eq!(world.real(&READY).after, [outgoing::idle_logout()]);
        assert_eq!(world.logout_timer, 250);
        // Mouse.setIdleLoops(14500) went to the port.
        assert_eq!(world.idle_loops(), [14_500, 15_001]);
        world.devices.mouse.set_idle_loops(15_000);
        window.mouse_motion(&mut world.devices, 9, 9);
        assert_eq!(world.real(&READY).after, [outgoing::idle_logout()]);
        // Releasing it is a mouseReleased like any other.
        world.devices.mouse.set_idle_loops(15_000);
        window.mouse_up(&mut world.devices, MouseButton::Middle);
        assert_eq!(world.real(&READY).after, []);
        assert_eq!(world.idle_loops(), [1, 15_003]);
        // A key press keeps the logout away whatever the mouse does.
        world.devices.mouse.set_idle_loops(15_000);
        window.key_down(&mut world.devices, KeyCode::A);
        assert_eq!(world.real(&READY).after, []);
        assert_eq!(world.idle_loops(), [15_001, 1]);
    }

    #[test]
    fn the_mouse_recorder_samples_the_pointer_of_the_last_mouse_loop() {
        // MouseRecorder.run stores Mouse.lastMouseX/Y, which Mouse.loop
        // copies from the listener's pointer; Mouse.mouseExited puts the
        // pointer at -1, -1.
        let mut world = World::new();
        world.start(true);
        world.cycle(&quiet());
        world.cycle(&quiet());
        let mut window = Translator::new(Host::Native);
        // Nothing has entered the window yet.
        world.events.sample_mouse(0, &world.devices);
        window.mouse_motion(&mut world.devices, 10, 20);
        // Not looped yet: the recorder still sees the old pointer.
        world.events.sample_mouse(50, &world.devices);
        world.real(&READY);
        world.events.sample_mouse(100, &world.devices);
        window.exit(&mut world.devices);
        world.events.sample_mouse(150, &world.devices);
        world.real(&READY);
        world.events.sample_mouse(200, &world.devices);
        let mut reference = MouseRecorder::default();
        reference.enabled = true;
        for [x, y] in [[-1, -1], [-1, -1], [10, 20], [10, 20], [-1, -1]] {
            reference.sample(x, y);
        }
        // A click sends what is waiting, before its own report.
        window.mouse_down(&mut world.devices, MouseButton::Left, 7, 7, 9000);
        let cycle = world.real(&READY);
        assert_eq!(
            cycle.before.iter().map(|f| f.opcode).collect::<Vec<_>>(),
            [123, 75]
        );
        assert_eq!(cycle.before[0], reference.flush(true).unwrap());
        // The press did not move the pointer back inside.
        assert_eq!(
            [
                world.devices.mouse.last_mouse_x,
                world.devices.mouse.last_mouse_y
            ],
            [-1, -1]
        );
    }

    #[test]
    fn a_new_session_zeroes_the_mouse_statics_and_leaves_the_keyboards() {
        // client.java:634 (Mouse.prevClickTime) and 653 (Mouse.setIdleLoops);
        // nothing in resetGameSessionState touches Keyboard.idleLoops.
        let mut world = World::playing();
        world.cycle(&Input {
            click: click(1, 5000),
            ..quiet()
        });
        assert_eq!(world.devices.mouse.prev_click_time, 5000);
        world.idle(700, 900);
        world.start(false);
        assert_eq!(world.idle_loops(), [0, 900]);
        assert_eq!(world.devices.mouse.prev_click_time, 0);
    }
}
