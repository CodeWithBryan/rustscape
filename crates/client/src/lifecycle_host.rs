//! The application side of `rs09_client::lifecycle`: it runs the state
//! machine once per 20 ms cycle, carries out its effects on the state the
//! main loop owns, and draws what is not the game or an interface: the
//! loading bar that ends the start-up load, the frozen frame of game state
//! 40 and the status box.
//!
//! The title screen is the `loginscreen` interface. The main loop runs it
//! like any other top-level interface; its scripts reach the login state
//! machine ([`LoginFlow`]) through `Vm::login_host`, and this host runs that
//! machine behind the lifecycle ([`FlowSeam`]). A launch file's credentials
//! are handed to the same machine as CS2 5600 would hand them, once the
//! title screen is up.
//!
//! Differences from the original:
//! - Of `client.mainLoad` only state 135 (the world list) runs here, behind
//!   the original's loading bar; the cache is complete by then (see
//!   `cache_update_screen`).
//! - The title screen's 3D scene (game state 28) is `title_world.rs`'s; it
//!   needs map streaming, and without it the interface's scene component
//!   shows the title background instead (see `title_screen.rs`).
//! - Game state 25 keeps the last frame under its box only for the scene
//!   that follows a login; a rebuild in game keeps drawing the old scene.
use super::{
    ActionOutbox, Model, Movement, Pack, PlayerBas, PlayerPick, Workbench, WorldSound, client_ui,
    ground_render, minimap_render,
    screen_frame::{self, Frame},
    ui_audio, ui_render,
};
use macroquad::prelude::*;
use rs09_client::{
    lifecycle::{self, Effect, GameState, Lifecycle, LogoutReason},
    live::Connection,
    login_flow::{DEFAULT_HOSTNAME, Endpoints, JsonConfig, LoginFlow, TransportDialer},
    login_seam::{self, FlowSeam},
    title::{Assets, WORLD_LIST_LOADING, WorldListStage},
    ui::{state::State, vm::Vm},
};
use rs09_protocol::game::players::RawAppearance;
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    rc::Rc,
};

/// `GlobalConfig`/`client.worldListId`'s default: the world a launch without
/// `--world` is taken to be.
const DEFAULT_WORLD: i32 = 1;

/// Where the start-up load is. The original's `client.mainLoad` ends with
/// the world list (state 135) and `InterfaceList.resetToLoginScreen(true)`
/// (state 160).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Boot {
    /// State 135: the world list is being fetched.
    WorldList,
    /// State 160: the title interface is to be opened.
    Open,
    /// The load is over: the game state decides what runs.
    Done,
    /// Game state 1000: the world list could not be had. The original stops
    /// for good with the loading bar as it last drew it.
    Fatal,
}

pub struct Host {
    pub life: Lifecycle,
    seam: FlowSeam<TransportDialer>,
    /// `LoginManager`, `CreateManager` and `WorldList`; the interface
    /// scripts hold the same object.
    flow: Rc<RefCell<LoginFlow<TransportDialer>>>,
    boot: Boot,
    /// `client.mainLoadPercentage` and `mainLoadSecondaryText`.
    bar: (i32, &'static str),
    /// The launch file's username and password, until they are handed to
    /// the title screen's login.
    auto_login: Option<(Vec<u8>, Vec<u8>)>,
    assets: Option<Assets>,
    /// The CRC of each archive index for the login block.
    checksums: [i32; 28],
    /// `Player.usernameInput` of the last login, for CS2 5015.
    username: String,
    /// The title song is to be started (`client.setGameState` entering the
    /// title group).
    title_song: bool,
    /// Wall-clock time up to which 20 ms cycles have been run.
    clock: f64,
    /// Cycles run by the last [`Self::cycles`], for the music player.
    pub elapsed_cycles: u32,
    /// Capture runs keep the main loop running in every state so that their
    /// frame counters advance.
    passive: bool,
    /// The last game frame, shown unchanged while game state 40 lasts (the
    /// original stops redrawing the game then).
    snapshot: Option<Frame>,
    snapshot_wanted: bool,
    /// `Protocol.previousSocket`.
    previous: Option<Connection>,
    /// The last frame drawn in a game state that redraws (5, 10, 30), kept
    /// on the graphics card. Entering game state 25, 28 or 40 the original
    /// copies the frame on screen back (`GlRenderer.draw()` in
    /// `client.setGameState`) and from then on only draws its box over it.
    kept: Option<Frame>,
    /// A login has succeeded and its first scene is not built yet (the
    /// original is in game state 25 from the login's own rebuild packet
    /// on): the frame from before the login stays up until it is.
    entering: bool,
    /// Game state 28 as of the last [`Self::apply`].
    title_loading: bool,
    /// The status box has been drawn since game state 25 began.
    loading_drawn: bool,
    box_error_reported: bool,
    bar_error_reported: bool,
    /// The login reply last printed, so that a refusal is printed once.
    reply_reported: i32,
    /// The status last handed to the page while there was no connection.
    published: Option<String>,
    /// Frames written by [`Self::dump_frame`].
    #[cfg(not(target_arch = "wasm32"))]
    dumped: u32,
}
/// The main loop's state a lifecycle effect touches.
pub struct Refs<'a> {
    pub connection: &'a mut Option<Connection>,
    pub workbench: &'a mut Workbench,
    pub ui: &'a mut Option<State>,
    pub vm: &'a mut Vm,
    pub players: &'a mut rs09_protocol::game::players::State,
    pub live_base: &'a mut Option<[i32; 2]>,
    pub actors: &'a mut BTreeMap<usize, Movement>,
    pub npcs: &'a mut rs09_client::npc_updates::State,
    pub appearances: &'a mut BTreeMap<usize, Model>,
    pub bas: &'a mut BTreeMap<usize, PlayerBas>,
    pub decoded: &'a mut BTreeMap<usize, RawAppearance>,
    pub motion_appearances: &'a mut BTreeMap<usize, RawAppearance>,
    pub appearance_cache: &'a mut BTreeMap<usize, RawAppearance>,
    pub forms: &'a mut BTreeMap<usize, rs09_client::player_form::Form>,
    pub picks: &'a mut Vec<PlayerPick>,
    pub body_overrides: &'a mut BTreeMap<usize, [i32; 2]>,
    pub actor_states: &'a mut BTreeMap<usize, rs09_client::actor_animation::State>,
    pub actor_sounds: &'a mut Vec<(usize, rs09_client::actor_animation::Sound, i32)>,
    pub world_sounds: &'a mut Vec<WorldSound>,
    pub camera: &'a mut rs09_client::camera::Script,
    pub motion: &'a mut Movement,
    pub map_flag: &'a mut rs09_client::walking::DestinationFlag,
    pub route_steps: &'a mut VecDeque<[i32; 2]>,
    pub context_menu: &'a mut client_ui::ContextMenu,
    pub outbox: &'a mut ActionOutbox,
    pub ground_renderer: &'a mut ground_render::Renderer,
    pub minimap: &'a mut Option<minimap_render::Renderer>,
    pub atmosphere: &'a mut rs09_client::atmosphere::Atmosphere,
    pub ui_audio: &'a mut ui_audio::Effects,
    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
    pub ambient: &'a mut super::ambient_audio::Locations,
    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
    pub music: &'a mut Option<rs09_client::music::Music>,
    /// The main loop's cycle clock and the packets it sends by itself.
    pub driver: &'a mut super::client_cycle::Driver,
    /// The scene the last rebuild packet asked for can be drawn.
    pub scene_ready: bool,
    /// The session's random offsets and their drift (`MiniMap.zoomOffset`
    /// and the like), and the follow camera that shares them.
    pub drift: &'a mut rs09_client::minimap::Drift,
    pub follow: &'a mut rs09_client::follow_camera::Follow,
    /// The title screen's 3D scene.
    pub title: &'a mut super::title_world::Title,
    /// `Math.random()` for those offsets: the main loop's generator, which
    /// also feeds their per-cycle drift.
    pub random: &'a mut rs09_game_core::random::JavaRandom,
}

impl Refs<'_> {
    fn session<'s>(&'s mut self, username: &'s str) -> lifecycle::Session<'s> {
        lifecycle::Session {
            vm: &mut *self.vm,
            ui: self.ui.as_mut(),
            players: &mut *self.players,
            npcs: &mut *self.npcs,
            ground: &mut self.workbench.ground,
            world_spots: &mut self.workbench.world_spots,
            camera: &mut *self.camera,
            map_flag: &mut *self.map_flag,
            motion: &mut *self.motion,
            actors: &mut *self.actors,
            overheads: &mut self.workbench.overheads,
            username,
        }
    }

    /// The players' models, forms and animation state: what this client
    /// derives from `PlayerList.players` and `PlayerList.appearanceCache`.
    fn clear_players(&mut self) {
        self.appearances.clear();
        self.bas.clear();
        self.decoded.clear();
        self.motion_appearances.clear();
        self.appearance_cache.clear();
        self.forms.clear();
        self.picks.clear();
        self.body_overrides.clear();
        self.actor_states.clear();
        self.workbench.avatar.clear();
        self.workbench.npcs.clear();
        self.route_steps.clear();
    }

    /// `SceneGraph.objStacks[..] = null` for what is drawn of them.
    fn clear_ground(&mut self) {
        self.workbench.ground_supports.clear();
        self.ground_renderer.clear();
    }

    /// `MiniMenu.size = 0` and `Cs1ScriptRunner.isMenuOpen = false`.
    fn close_menu(&mut self) {
        self.context_menu.menu = rs09_client::interaction::Menu::default();
    }

    /// The application's half of `client.resetGameSessionState`; the rest is
    /// `lifecycle::reset_session`.
    fn reset_session(&mut self) {
        // Protocol.outboundBuffer.offset = 0
        self.outbox.pending.clear();
        self.close_menu();
        if let Some(map) = self.minimap.as_mut() {
            map.invalidate();
        }
        self.clear_players();
        // SoundPlayer.size = 0
        self.actor_sounds.clear();
        self.world_sounds.clear();
        self.ui_audio.reset_queue();
        self.clear_ground();
        // ChangeLocRequest.queue = new LinkedList(). The original then builds
        // the scene from the map files again, which undoes every temporary
        // change; this scene is kept, so each change is reverted first.
        for change in &mut self.workbench.world.changes {
            change.reset_loops = 0;
        }
        if let Err(error) = self.workbench.advance_world() {
            eprintln!("Scenery reset at login: {error}");
        }
        self.workbench.world.changes.clear();
        // FogManager.setInstantFade()
        self.atmosphere.instant = true;
        // The scene base (Camera.originX/Y) is taken from the rebuild packet
        // that follows, as for the first scene of the run.
        *self.live_base = None;
    }

    /// The application's half of `LoginManager.processLogout`; the rest is
    /// `lifecycle::process_logout`.
    fn process_logout(&mut self) {
        // Protocol.socket.close(); Protocol.socket = null
        if let Some(mut connection) = self.connection.take() {
            connection.stop("Logged out");
        }
        // MidiPlayer.playFadeOut(), MidiPlayer.jingle, MusicPlayer.groupId
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        if let Some(music) = self.music.as_mut() {
            music.reset();
        }
        // AreaSoundManager.clear(true)
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        if let Some(ui) = self.ui.as_ref() {
            self.ambient.suspend(self.ui_audio, &ui.pack);
        }
        self.clear_players();
        self.clear_ground();
        self.close_menu();
        // Camera.originX/Y = 0, SceneGraph.centralZoneX/Y = 0
        *self.live_base = None;
    }
}

/// `client.main` with a `config.json`: one address for the world list and
/// for every world, which is what a launch endpoint is. The original then
/// logs in to `server_port + world` on that host whatever world the list
/// names, so `server_port` is the endpoint's port less the world.
#[cfg(not(target_arch = "wasm32"))]
fn endpoints(endpoint: &str, world: i32) -> (Endpoints, TransportDialer) {
    use std::net::ToSocketAddrs;
    let (host, port) = endpoint
        .rsplit_once(':')
        .and_then(|(host, port)| Some((host, port.parse::<u16>().ok()?)))
        .unwrap_or((endpoint, rs09_client::login_flow::DEFAULT_PORT as u16));
    let host = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_owned();
    let json = JsonConfig {
        ip_management: host.clone(),
        ip_address: host,
        world,
        server_port: i32::from(port) - world,
        wl_port: i32::from(port),
    };
    // `Transport::connect` takes an address; `SignLink.openSocket` resolves
    // names, here on the calling thread.
    let dialer = TransportDialer {
        endpoint: Box::new(|host, port| {
            u16::try_from(port)
                .ok()
                .and_then(|port| (host, port).to_socket_addrs().ok()?.next())
                .map_or_else(|| format!("{host}:{port}"), |address| address.to_string())
        }),
    };
    (Endpoints::standalone(DEFAULT_HOSTNAME, Some(json)), dialer)
}
/// The browser reaches the one WebSocket address it was launched with: the
/// world list and every login go there, whatever host and port the client
/// asks for.
#[cfg(target_arch = "wasm32")]
fn endpoints(endpoint: &str, world: i32) -> (Endpoints, TransportDialer) {
    let json = JsonConfig {
        ip_management: endpoint.to_owned(),
        ip_address: endpoint.to_owned(),
        world,
        server_port: 0,
        wl_port: 0,
    };
    let address = endpoint.to_owned();
    let dialer = TransportDialer {
        endpoint: Box::new(move |_, _| address.clone()),
    };
    (Endpoints::standalone(DEFAULT_HOSTNAME, Some(json)), dialer)
}

/// `MonotonicClock.currentTimeMillis()`: milliseconds since 1970, the clock
/// the scripts' world-list command (CS2 6500, `Vm::now_ms`) reads too.
fn now_ms() -> i64 {
    (miniquad::date::now() * 1000.0) as i64
}

impl Host {
    pub fn new(
        launch: rs09_client::launch::Launch,
        world: Option<i32>,
        capture: bool,
        crc_sums: [i32; 29],
    ) -> Self {
        let (endpoints, dialer) = endpoints(&launch.endpoint, world.unwrap_or(DEFAULT_WORLD));
        // Math.random(), which seeds the opcode cipher among other things.
        let mut random = rs09_game_core::random::JavaRandom::new(
            (miniquad::date::now() * 1000.0) as i64 ^ 0x2545_F491_4F6C_DD1D,
        );
        let flow = Rc::new(RefCell::new(LoginFlow::new(
            dialer,
            endpoints,
            Box::new(move || random.next_double()),
        )));
        let mut checksums = [0; 28];
        checksums.copy_from_slice(&crc_sums[..28]);
        let rs09_client::launch::Launch {
            username, password, ..
        } = launch;
        Self {
            life: Lifecycle::new(),
            seam: FlowSeam::new(Rc::clone(&flow)),
            flow,
            boot: Boot::WorldList,
            bar: (95, WORLD_LIST_LOADING),
            auto_login: (!username.is_empty()).then_some((username, password)),
            assets: None,
            checksums,
            username: String::new(),
            title_song: true,
            clock: get_time(),
            elapsed_cycles: 0,
            passive: capture,
            snapshot: None,
            snapshot_wanted: false,
            previous: None,
            kept: None,
            entering: false,
            title_loading: false,
            loading_drawn: false,
            box_error_reported: false,
            bar_error_reported: false,
            reply_reported: lifecycle::REPLY_NONE,
            published: None,
            #[cfg(not(target_arch = "wasm32"))]
            dumped: 0,
        }
    }

    /// The 20 ms cycles that are due: the end of the start-up load, then
    /// `CreateManager.loop`, `LoginManager.loopAuto` and `LoginManager.loop`
    /// in game states 10 and 40 (`client.mainLoop`).
    pub fn cycles(&mut self) {
        self.elapsed_cycles = 0;
        let now = get_time();
        // A long stall (a dragged window) is not replayed cycle by cycle.
        if now - self.clock > 1.0 {
            self.clock = now - 1.0;
        }
        while now - self.clock >= 0.02 {
            self.clock += 0.02;
            self.elapsed_cycles += 1;
            match self.boot {
                Boot::WorldList => {
                    let result = self.flow.borrow_mut().startup_world_list(now_ms());
                    match WorldListStage::of(result) {
                        WorldListStage::Bar {
                            percent,
                            text,
                            done,
                        } => {
                            self.bar = (percent, text);
                            if done {
                                self.boot = Boot::Open;
                            }
                        }
                        WorldListStage::Fatal(error) => {
                            // GameShell.error: the standalone client has no
                            // page to send the player to.
                            eprintln!("Error: {error}");
                            self.boot = Boot::Fatal;
                        }
                    }
                }
                Boot::Open | Boot::Fatal => {}
                // Game state 28 runs LoginManager.rebuildMap only.
                Boot::Done if self.title_loading && self.life.state() == GameState::Title => {}
                Boot::Done => self.seam.cycle_game_state(&mut self.life),
            }
        }
    }

    /// Logged out: the title screen (or the load before it) is what shows,
    /// and nothing of the world is to be clicked.
    pub fn at_title(&self) -> bool {
        self.life.state() == GameState::Title
    }

    /// `LoginManager.step != 0 || CreateManager.step != 0`: a login or an
    /// account creation is under way, during which the title screen's camera
    /// stands still (client.java:1202).
    pub fn login_busy(&self) -> bool {
        let flow = self.flow.borrow();
        flow.login.step != 0 || flow.create.step != 0
    }

    /// A login's first scene is being built behind the frame from before
    /// the login (game state 25 as the original shows it).
    pub fn frame_held(&self) -> bool {
        !self.passive
            && self.entering
            && matches!(self.life.state(), GameState::Title | GameState::Loading)
    }

    /// A line for the main loop's status text while it has no connection.
    pub fn status(&self) -> String {
        match self.boot {
            Boot::WorldList | Boot::Open => self.bar.1.to_owned(),
            Boot::Fatal => "World list unavailable".to_owned(),
            Boot::Done if self.life.logging_in() => "Logging in".to_owned(),
            Boot::Done => String::new(),
        }
    }

    /// The browser page's view of the client while there is no connection
    /// (the main loop publishes the session's state while there is one):
    /// [`Self::status`], handed over when it changes, so that the page does
    /// not go on showing the last session after a logout or a lost
    /// connection. Does nothing on the desktop.
    pub fn publish_status(&mut self, connected: bool) {
        if connected {
            self.published = None;
            return;
        }
        let status = self.status();
        if self.published.as_deref() != Some(status.as_str()) {
            rs09_client::launch::publish(&serde_json::json!({
                "status": status,
                "in_game": false,
            }));
            self.published = Some(status);
        }
    }

    /// A packet handler failed: the original's "T2" report and logout.
    pub fn handler_error(&mut self, error: String, payload: &[u8], self_tile: [i32; 2]) {
        println!("Error: {error}");
        self.life.handler_error(payload, self_tile);
    }

    /// The main loop should not run this frame: [`Self::frame`] draws it.
    /// That is game state 40, in which the original runs nothing but the
    /// reconnect login. `connected` is false until a login has handed over
    /// its connection, whose first packet (the scene rebuild) the main loop
    /// must then read.
    pub fn takes_frame(&self, connected: bool) -> bool {
        !self.passive && !connected && self.life.state() == GameState::ConnectionLost
    }

    /// Draws a frame of game state 40: the last game frame with the status
    /// box.
    pub fn frame(&mut self, renderer: &mut ui_render::Renderer, ui: Option<&State>, visual: &Pack) {
        clear_background(BLACK);
        if let Some(snapshot) = &self.snapshot {
            // The grab is of the whole window, letterbox included.
            snapshot.draw();
            crate::display_host::camera();
        }
        self.status_box(renderer, ui, visual, false);
        crate::display_host::letterbox();
        self.dump_frame();
    }

    /// Keeps the frame just drawn, without reading it back. The grab is of
    /// the whole window, letterbox included.
    fn keep_frame(&mut self) {
        let size = screen_frame::surface();
        if size.contains(&0) {
            return;
        }
        // Nothing queued draws the kept frame here: it is drawn only in the
        // frames that do not keep one.
        // SAFETY: the macroquad context exists whenever a frame is drawn.
        unsafe { get_internal_gl().flush() };
        if self.kept.as_ref().is_none_or(|kept| kept.size() != size) {
            self.kept = Some(Frame::new(size));
        }
        if let Some(kept) = &self.kept {
            kept.grab();
        }
    }

    /// The kept frame over whatever this frame drew.
    fn draw_kept(&self) {
        let Some(kept) = &self.kept else {
            clear_background(BLACK);
            return;
        };
        kept.draw();
        crate::display_host::camera();
    }

    fn assets(&mut self, ui: &State) -> Assets {
        *self.assets.get_or_insert_with(|| {
            Assets::read(&ui.pack).unwrap_or_else(|error| {
                eprintln!("Title assets: {error}");
                Assets::PINNED
            })
        })
    }

    /// The status box of game states 25, 28 and 40 over whatever is drawn.
    /// `title_loading` is game state 28, which the lifecycle counts as 10.
    fn status_box(
        &mut self,
        renderer: &mut ui_render::Renderer,
        ui: Option<&State>,
        visual: &Pack,
        title_loading: bool,
    ) {
        let state = if title_loading {
            GameState::Loading
        } else {
            self.life.state()
        };
        let Some(text) = lifecycle::status_text(state, None) else {
            return;
        };
        let drawn = match ui.map(|ui| renderer.status_box(&ui.pack, visual, &text)) {
            Some(Ok(())) => true,
            Some(Err(error)) => {
                if !self.box_error_reported {
                    self.box_error_reported = true;
                    eprintln!("Status box drawn without the game font: {error}");
                }
                false
            }
            None => false,
        };
        if !drawn {
            // Without the interface fonts: the same words, plainly.
            let lines: Vec<&str> = text.split("<br>").collect();
            draw_rectangle(6.0, 6.0, 380.0, lines.len() as f32 * 18.0 + 8.0, BLACK);
            for (i, line) in lines.iter().enumerate() {
                draw_text(line, 10.0, 24.0 + i as f32 * 18.0, 18.0, WHITE);
            }
        }
        if self.life.state() == GameState::Loading {
            self.loading_drawn = true;
        }
    }

    /// `LoadingBar.render` while the start-up load lasts (game state 5), in
    /// place of whatever the main loop drew.
    fn loading_bar(
        &mut self,
        renderer: &mut ui_render::Renderer,
        ui: Option<&State>,
        visual: &Pack,
    ) {
        if self.boot == Boot::Done {
            return;
        }
        let (percent, text) = self.bar;
        let drawn = match ui {
            Some(ui) => {
                let assets = self.assets(ui);
                renderer.loading_bar(&ui.pack, visual, &assets, percent, text)
            }
            None => Err("no interface assets".to_owned()),
        };
        if let Err(error) = drawn {
            if !self.bar_error_reported {
                self.bar_error_reported = true;
                eprintln!("Loading bar drawn without the title assets: {error}");
            }
            clear_background(BLACK);
            draw_text(
                format!("{} ({percent}%)", rs09_client::title::LOADING_TITLE),
                40.0,
                60.0,
                24.0,
                WHITE,
            );
            draw_text(text, 40.0, 90.0, 20.0, WHITE);
        }
    }

    /// The end of a frame the main loop drew: keep it if game state 40 has
    /// just begun, then draw the loading bar or the status box over it. In
    /// game state 28, and in the 25 that follows a login, the frame is not
    /// shown: the last one before that state stays up under the box.
    pub fn end_frame(
        &mut self,
        renderer: &mut ui_render::Renderer,
        ui: Option<&State>,
        visual: &Pack,
        title: &super::title_world::Title,
    ) {
        if self.snapshot_wanted {
            self.snapshot_wanted = false;
            self.snapshot = Some(Frame::snapshot());
        }
        self.loading_bar(renderer, ui, visual);
        let state = self.life.state();
        let title_loading = state == GameState::Title && self.boot == Boot::Done && title.loading();
        if !self.passive && title_loading || self.frame_held() {
            self.draw_kept();
            self.status_box(renderer, ui, visual, true);
        } else {
            self.status_box(renderer, ui, visual, false);
            if !self.passive && matches!(state, GameState::Title | GameState::InGame) {
                self.keep_frame();
            }
        }
        // After everything drawn on the canvas, before the frame is looked
        // at: the title background is wider than the fixed canvas.
        crate::display_host::letterbox();
        self.dump_frame();
    }

    /// A development aid for looking at a running client: when the folder
    /// named by `RS09_FRAME_DUMP` holds a file called `take` (or `take-25`,
    /// which waits for game state 25), the frame just drawn is written there
    /// as `frame-N.png` and the file is removed.
    fn dump_frame(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(folder) = std::env::var_os("RS09_FRAME_DUMP") {
            let folder = std::path::PathBuf::from(folder);
            // "take-25" waits for a frame of game state 25.
            let loading = folder.join("take-25");
            let request = if self.life.state() == GameState::Loading && loading.exists() {
                loading
            } else {
                folder.join("take")
            };
            if request.exists() {
                self.dumped += 1;
                let path = folder.join(format!("frame-{}.png", self.dumped));
                get_screen_data().export_png(&path.to_string_lossy());
                let _ = std::fs::remove_file(request);
            }
        }
    }

    /// `InterfaceList.resetToLoginScreen(true)` (InterfaceList.java:1087-
    /// 1122) for the interfaces: everything is closed and the title
    /// interface becomes the top level, with its load scripts run. The
    /// cursor and the local player are `lifecycle::process_logout`'s; the
    /// title scene the original then builds is not built here.
    fn open_title(&mut self, r: &mut Refs<'_>) {
        let Some(ui) = r.ui.as_mut() else {
            return;
        };
        let assets = self.assets(ui);
        let vm = &mut *r.vm;
        ui.close_everything(|ui| vm.drain(ui));
        if let Err(error) = ui.open_root(assets.interface) {
            eprintln!("Title interface {}: {error}", assets.interface);
        }
        vm.drain(ui);
        ui.targeting.default_cursor = -1;
        r.title.reset(r.workbench);
    }

    /// What the login block and the title scripts read from the rest of the
    /// client, kept current.
    fn refresh(&mut self, r: &mut Refs<'_>) {
        if r.vm.login_host.is_none() {
            r.vm.login_host = Some(Rc::clone(&self.flow) as _);
        }
        // Player.worldId, which a world hop (CS2 6503) changes.
        r.vm.world = self.flow.borrow().endpoints.world_id;
        let env = &mut self.seam.env;
        env.window_mode = r.vm.display.window_mode();
        env.canvas = r.vm.display.canvas;
        env.anti_aliasing = r.vm.preferences.anti_aliasing;
        env.affiliate = r.vm.affiliate;
        env.preferences = r.vm.preferences.to_int();
        env.verify_id = r.ui.as_ref().map_or(0, |ui| ui.verify_id);
        env.checksums = self.checksums;
        if std::mem::take(&mut self.seam.preferences_sent) {
            r.vm.preferences.sent_to_server = true;
        }
    }

    /// Carries out what the lifecycle decided since the last call. Called
    /// after the title/state-40 cycles and after the main loop has read the
    /// game connection.
    pub fn apply(&mut self, mut r: Refs<'_>) {
        self.refresh(&mut r);
        if std::mem::take(&mut self.title_song) {
            // client.setGameState entering 5, 10 or 28 from elsewhere.
            #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
            if let (Some(music), Some(ui)) = (r.music.as_mut(), r.ui.as_ref()) {
                let song = self.assets(ui).song;
                eprintln!("Title song {song}");
                music.title(song);
            }
        }
        if self.boot == Boot::Open {
            // mainLoad state 150 (client.java:1682): the window mode last
            // remembered, which the title screen then opens in.
            let remembered = r.vm.preferences.remembered_mode;
            r.vm.display
                .set_window_mode(false, remembered, -1, -1, &mut r.vm.preferences);
            r.vm.window_mode = r.vm.display.window_mode();
            // mainLoad state 160.
            self.open_title(&mut r);
            self.seam.mirror(GameState::Title);
            self.boot = Boot::Done;
            if let Some((username, mut password)) = self.auto_login.take() {
                // The launch file stands in for the player at the login
                // form: CS2 5600 with what would have been typed.
                if let Err(error) = self.flow.borrow_mut().script_login(&username, &password, 0) {
                    eprintln!("Launch login: {error}");
                }
                password.fill(0);
            }
        }
        // Any end of the game connection that is not a logout is
        // Protocol.handleConnectionLost: a failed read or write, or 750
        // cycles without a packet.
        if matches!(self.life.state(), GameState::InGame | GameState::Loading)
            && !self.life.stopping()
            && r.connection.as_ref().is_none_or(Connection::closed)
        {
            self.life
                .connection_lost(r.vm.logout_on_disconnect_timer, &mut self.seam);
            self.clock = get_time();
        }
        self.world_map_requests(&mut r);
        self.effects(&mut r);
        // The end of LoginManager.rebuildMap, once the box has been shown.
        if self.life.state() == GameState::Loading
            && (self.loading_drawn || self.passive)
            && r.scene_ready
        {
            self.life.rebuild_finished();
            self.effects(&mut r);
        }
        // The title screen shows a refusal in its login form; a launch
        // file's login has no form open, so the reply is printed as well.
        if self.life.logging_in() {
            self.reply_reported = lifecycle::REPLY_IN_PROGRESS;
        } else if self.life.state() == GameState::Title && self.life.reply != self.reply_reported {
            self.reply_reported = self.life.reply;
            if !matches!(
                self.life.reply,
                lifecycle::REPLY_NONE | lifecycle::REPLY_IN_PROGRESS | 2
            ) {
                eprintln!("Login reply {}", self.life.reply);
            }
        }
        self.title_loading = r.title.loading();
        r.vm.login_reply = match self.life.reply {
            lifecycle::REPLY_NONE => None,
            reply => Some(reply),
        };
    }

    /// What the world map asked of the scene since the last call: see
    /// `lifecycle::world_map_requests`.
    fn world_map_requests(&mut self, r: &mut Refs<'_>) {
        let Some(ui) = r.ui.as_mut() else {
            return;
        };
        let requests = ui.world_map.take_scene_requests();
        if lifecycle::world_map_requests(requests, &mut self.life, &mut r.driver.events) {
            r.driver.reset_timer();
        }
    }

    fn effects(&mut self, r: &mut Refs<'_>) {
        for effect in self.life.take_effects() {
            match effect {
                Effect::State { from, to } => {
                    eprintln!("Game state {} -> {}", from.code(), to.code());
                    // client.setGameState(25): WorldMap.clear(true).
                    if to == GameState::Loading {
                        super::world_map_render::scene_loading(r.ui.as_mut());
                    }
                    // client.setGameState: LoginManager.clear() entering 40,
                    // and Protocol.previousSocket closed on any other change.
                    self.seam.mirror(to);
                    if to != GameState::ConnectionLost {
                        self.previous = None;
                    }
                    if to != GameState::Loading {
                        self.entering = false;
                    }
                    match to {
                        GameState::ConnectionLost => self.snapshot_wanted = true,
                        GameState::Loading => self.loading_drawn = false,
                        _ => {}
                    }
                    if to != GameState::ConnectionLost {
                        self.snapshot = None;
                    }
                    // Leaving the title screen fades its music out; entering
                    // it starts the title song.
                    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
                    if from == GameState::Title {
                        if let Some(music) = r.music.as_mut() {
                            music.fade_out();
                        }
                    }
                    if to == GameState::Title {
                        self.title_song = true;
                    }
                }
                Effect::ResetSession(world) => {
                    let connection = match self.seam.take_entered() {
                        Some(entered) => {
                            login_seam::game_connection(&mut self.flow.borrow_mut(), &entered)
                        }
                        None => Err("the login left no first packet".to_owned()),
                    };
                    // Without it the next pass finds the connection gone and
                    // takes the connection-lost path.
                    *r.connection = connection
                        .map_err(|error| eprintln!("Game connection: {error}"))
                        .ok();
                    // The driver starts its session, window status first,
                    // when it reads this connection.
                    r.driver.session_end();
                    r.title.leave();
                    self.entering = true;
                    r.reset_session();
                    let (username, world_id) = {
                        let flow = self.flow.borrow();
                        (
                            rs09_client::ui::latin1(flow.username()),
                            flow.endpoints.world_id,
                        )
                    };
                    self.username = username;
                    // The six Math.random() draws of the reset, in its
                    // order, from the generator that also drives the drift.
                    let mut draws = [(); 6].map(|()| r.random.next_double()).into_iter();
                    let jitter = lifecycle::reset_session(r.session(&self.username), || {
                        draws.next().unwrap_or(0.0)
                    });
                    rs09_client::session_drift::reset(&jitter, r.drift, Some(&mut *r.follow));
                    r.vm.apply_login(&world, world_id, &self.username);
                    r.workbench.ground.self_id = i32::from(world.player_index);
                }
                // ClientProt.sendWindowDetails(): written by the driver, at
                // the start of a session and on `reconnected`.
                Effect::WindowStatus => {}
                Effect::ProcessLogout(reason) => {
                    match &reason {
                        LogoutReason::Server => eprintln!("Logged out by the server"),
                        LogoutReason::LostWhileLoggingOut => {
                            eprintln!("Connection closed while logging out")
                        }
                        LogoutReason::ReconnectFailed(reply) => {
                            eprintln!("Connection lost; reconnecting failed (reply {reply})")
                        }
                        LogoutReason::UnknownOpcode(opcode) => {
                            eprintln!("Unknown server packet {opcode}")
                        }
                        // The handler's message is already printed.
                        LogoutReason::HandlerError => {}
                    }
                    self.entering = false;
                    r.process_logout();
                    // WorldMap.clear(false), LoginManager.java:660.
                    super::world_map_render::logged_out(r.ui.as_mut());
                    r.driver.session_end();
                    lifecycle::process_logout(r.session(&self.username));
                    self.open_title(r);
                }
                Effect::SetAsideConnection => {
                    if let Some(connection) = r.connection.as_mut() {
                        connection.stop("Connection lost");
                    }
                    self.previous = r.connection.take();
                    r.driver.connection_lost();
                }
                Effect::Reconnect => {
                    let mut connection = self.seam.take_reconnected().and_then(|reconnected| {
                        login_seam::resumed_connection(&mut self.flow.borrow_mut(), &reconnected)
                            .map_err(|error| eprintln!("Game connection: {error}"))
                            .ok()
                    });
                    // Reply 15 carries no player details: they are the ones
                    // of the login this session began with.
                    if let (Some(new), Some(old)) = (&mut connection, &self.previous) {
                        new.world = old.world.clone();
                    }
                    *r.connection = connection;
                    r.driver
                        .reconnected(r.vm.window_mode, r.vm.preferences.anti_aliasing);
                    r.outbox.pending.clear();
                    r.close_menu();
                    lifecycle::reconnect(r.session(&self.username));
                }
                // The rebuild marker and MAP_REBUILD_COMPLETE: the driver
                // writes both, in its first cycle that finds the lifecycle
                // back in game state 30. `WorldMap.restorePreviousMap()`
                // follows the second (LoginManager.java:1127-1132).
                Effect::Send(frame) => {
                    if frame.opcode == 110 {
                        super::world_map_render::scene_rebuilt(r.ui.as_mut());
                    }
                }
                Effect::Report(text) => println!("Error: {text}"),
            }
        }
    }
}
