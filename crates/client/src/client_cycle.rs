//! Drives `rs09_client::client_events` and the one logic clock from the
//! main loop: counts the 20 ms cycles of each drawn frame, hands each cycle
//! the server packets it reads, runs the two halves of `Protocol.loop`'s
//! own packets around the cycle's world and interfaces, and ends the cycle
//! with its one write to the connection. In game state 25 it writes what
//! `LoginManager.rebuildMap` writes: keep-alives.
//!
//! Input is not collected here. The main loop pumps the window's events
//! into `rs09_client::input::Input` (the ports of `Mouse` and `Keyboard`)
//! once per drawn frame and runs `Input::cycle` at the start of each 20 ms
//! cycle; [`Driver::cycle_begin`] and [`Driver::cycle_end`] read that state
//! as `Protocol.loop` reads the original's statics.
use macroquad::{miniquad, prelude::*};
use rs09_client::{
    client_events::{
        Backlog, Client, ClientEvents, Cycle, Ping, Shared, Window, packed_preferences,
    },
    clock::{self, Clock},
    input::{Input, platform::Events},
    live::Connection,
    ui::vm::Preferences,
};
use rs09_protocol::game::Frame;
use std::collections::VecDeque;

/// The values the two halves of one cycle read and write.
pub struct CycleState<'a> {
    /// `(int) Camera.pitchTarget`, `(int) Camera.yawTarget`.
    pub camera: [i32; 2],
    pub preferences: &'a mut Preferences,
    /// `Protocol.verifyId`.
    pub verify_id: i32,
    /// `Protocol.logoutOnDisconnectTimer`.
    pub logout_timer: &'a mut i32,
    /// `DisplayMode.getWindowMode()`.
    pub window_mode: i32,
    /// `LoginManager.rebuildMap` has nothing left to wait for: the scene of
    /// the last rebuild is built (the session lifecycle is back in state 30).
    pub scene_ready: bool,
}

pub struct Driver {
    clock: Clock,
    pub events: ClientEvents,
    /// Server packets received but not read yet.
    backlog: Backlog,
    /// `GameShell.resetTimer()` ran during this frame's cycles: `logicCycles`
    /// is 0, and the cycle that called it is the frame's last.
    timer_reset: bool,

    /// Game state 40: the connection is lost and a reconnect is being
    /// tried. `Protocol.loop` does not run until it has succeeded.
    suspended: bool,
    /// The cycle [`Self::cycle_begin`] began.
    open: Option<Cycle>,
}

fn now_ms() -> i64 {
    (miniquad::date::now() * 1000.0) as i64
}

/// `GameShell.frameWidth`/`frameHeight` as `rs09_client::display::Display`
/// last measured them; the main loop notes them each frame.
fn window(mode: i32, anti_aliasing: i32) -> Window {
    Window {
        mode,
        frame: crate::display_host::frame(),
        anti_aliasing,
    }
}

impl Driver {
    pub fn new() -> Self {
        Self {
            clock: Clock::new(clock::nanos(get_time())),
            events: ClientEvents::default(),
            backlog: Backlog::default(),
            timer_reset: false,
            suspended: false,
            open: None,
        }
    }

    fn forget(&mut self) {
        self.backlog.clear();
        self.suspended = false;
        self.open = None;
    }

    /// `Protocol.handleConnectionLost` chose game state 40: the session is
    /// kept, but nothing of `Protocol.loop` runs until
    /// [`Self::reconnected`] or the end of the session.
    pub fn connection_lost(&mut self) {
        self.forget();
        self.suspended = true;
    }

    /// The game connection is gone or replaced by a new login: the next
    /// connection read starts a session of its own.
    pub fn session_end(&mut self) {
        self.events.session_end();
        self.forget();
    }

    /// `LoginManager.reconnect`: the same session goes on over a new
    /// connection. What was received over the old one is dropped.
    pub fn reconnected(&mut self, window_mode: i32, anti_aliasing: i32) {
        self.forget();
        self.events.reconnected(&window(window_mode, anti_aliasing));
    }

    /// `GameShell.resetTimer()`: the time a slow step took is not caught up
    /// with extra cycles, and the cycles still owed are dropped
    /// (`logicCycles = 0`, GameShell.java:174). The original calls it after
    /// each load step of the world map and at the end of every scene
    /// rebuild.
    pub fn reset_timer(&mut self) {
        self.clock.reset(clock::nanos(get_time()));
        self.timer_reset = true;
    }

    /// Whether [`Self::reset_timer`] ran since the frame began: the cycle
    /// that did it is the frame's last.
    pub fn timer_was_reset(&self) -> bool {
        self.timer_reset
    }

    /// The start of a drawn frame: the 20 ms cycles it owes, at most ten
    /// (`GameShell.run` and `NanoTimer.count`).
    pub fn begin_frame(&mut self) -> u32 {
        self.timer_reset = false;
        self.clock.poll(clock::nanos(get_time()))
    }

    /// A drawn frame in which the main loop's body does not run (game state
    /// 40 with its own screen). `client.mainLoop` still runs `Keyboard.loop`
    /// and `Mouse.loop` in every state: the frame's events are taken and its
    /// cycles run on the devices, so the idle counters go on and a click
    /// made meanwhile is not kept for the game.
    pub fn skip_frame(&mut self, events: &mut Events, input: &mut Input) {
        events.pump(input, now_ms());
        for _ in 0..self.begin_frame() {
            input.cycle();
        }
    }

    /// The server packets one cycle reads, of those just received and those
    /// still waiting: at most 100 (Protocol.java:2412). The original's loop
    /// does not look at the game state, so the hundred that hold a packet
    /// starting a rebuild are read to their end.
    ///
    /// A session that has just reached the game is started first
    /// (`client.resetGameSessionState`); its first packet is the rebuild
    /// the login reads by itself (`LoginManager.loop` step 9) and is
    /// returned alone. Each packet returned is noted for the verification
    /// id and, if it is a rebuild, for the scene rebuild.
    pub fn read(
        &mut self,
        incoming: Vec<Frame>,
        connection: &Connection,
        window_mode: i32,
        preferences: &Preferences,
        logout_timer: &mut i32,
        input: &mut Input,
    ) -> Vec<Frame> {
        let in_game = connection.in_game();
        if in_game && !self.events.in_session() {
            let recording = connection
                .world
                .as_ref()
                .is_some_and(|world| world.mouse_recording);
            self.events.session_start(
                recording,
                &window(window_mode, preferences.anti_aliasing),
                logout_timer,
                input,
            );
            self.backlog.expect_login_packet();
        } else if !in_game && self.events.in_session() {
            self.session_end();
        }
        if self.suspended {
            return Vec::new();
        }
        self.backlog.read(&mut self.events, incoming)
    }
    /// One cycle of game state 25, `LoginManager.rebuildMap`: its
    /// keep-alives and, once the scene is built, its end
    /// (`ClientEvents::rebuild_cycle`), which resets the timer
    /// (LoginManager.java:1134). Returns whether the rebuild ended: the
    /// caller then runs `Protocol.loop` in the same cycle.
    pub fn rebuild(
        &mut self,
        connection: Option<&mut Connection>,
        pending: &mut VecDeque<Frame>,
        scene_ready: bool,
        keep_alives: u32,
    ) -> bool {
        let wire = connection
            .filter(|connection| connection.in_game())
            .map(|connection| connection as &mut dyn Ping);
        let ended = self
            .events
            .rebuild_cycle(wire, pending, scene_ready, keep_alives);
        if ended {
            self.reset_timer();
        }
        ended
    }
    /// The cycle that read a rebuild packet leaves `Protocol.loop` at once
    /// (Protocol.java:2414): nothing is written, and what waits in the
    /// outgoing buffer stays there.
    pub fn cycle_left(&mut self) {
        self.open = None;
        self.events.rebuild_waited();
    }

    fn client(&self, state: &CycleState<'_>) -> Client {
        Client {
            keyboard_camera: state.preferences.keyboard_camera,
            camera: state.camera,
            preferences: packed_preferences(state.preferences),
            scene_ready: state.scene_ready,
        }
    }

    /// `Protocol.loop` after its packets, up to where it turns to the world
    /// and the interfaces (Protocol.java:2417-2565): what it writes by
    /// itself from the devices.
    pub fn cycle_begin(&mut self, input: &mut Input, state: CycleState<'_>) {
        self.open = None;
        if self.suspended {
            return;
        }
        let client = self.client(&state);
        let cycle = self.events.cycle_begin(
            input,
            &client,
            &mut Shared {
                verify_id: state.verify_id,
                preferences_sent: &mut state.preferences.sent_to_server,
                logout_timer: state.logout_timer,
            },
        );
        self.open = Some(cycle);
    }

    /// The end of the cycle [`Self::cycle_begin`] began, after its world and
    /// interfaces ran (Protocol.java:2905-2991): the idle logout, the
    /// keep-alive, the verification id and the cycle's one write. `pending`
    /// holds the interface and menu packets the cycle made, in the order
    /// they were made; they go between the loop's own first and last
    /// packets. Without a game session they are dropped.
    pub fn cycle_end(
        &mut self,
        input: &mut Input,
        state: CycleState<'_>,
        connection: Option<&mut Connection>,
        pending: &mut VecDeque<Frame>,
    ) {
        let cycle = self.open.take().map(|mut cycle| {
            self.events.cycle_end(
                &mut cycle,
                input,
                &window(state.window_mode, state.preferences.anti_aliasing),
                &mut Shared {
                    verify_id: state.verify_id,
                    preferences_sent: &mut state.preferences.sent_to_server,
                    logout_timer: state.logout_timer,
                },
            );
            cycle
        });
        match (connection, cycle) {
            (Some(connection), Some(cycle)) if connection.in_game() && cycle.playing => {
                let (written, verify_id) = cycle.written(pending.drain(..));
                if let Err(error) = connection.cycle(&written, verify_id.as_ref()) {
                    connection.stop(error);
                }
            }
            _ => pending.clear(),
        }
    }

    /// The end of a drawn frame, `client.mainRedraw`'s look at the window
    /// (client.java:817-842), and the mouse recorder's sample.
    pub fn end_frame(&mut self, input: &Input, window_mode: i32, anti_aliasing: i32) {
        let now = now_ms();
        self.events.sample_mouse(now, input);
        self.events.redraw(now, &window(window_mode, anti_aliasing));
    }
}
