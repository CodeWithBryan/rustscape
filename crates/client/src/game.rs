//! The state of the running client and one drawn frame of it.
//!
//! `main` sets everything up and then calls [`Game::frame`] until it returns
//! false. The fields are the variables the main loop used to keep on its
//! stack; the functions in `game/` are the parts of its body. Each of those
//! begins by binding the fields it names to locals of the same name, so the
//! statements inside read as they did in the loop.
use super::*;
use rs09_client::{
    game_cycle::{Arrived, Host, State as CycleState, Step},
    input::{Input, platform::Events},
    interaction::{Menu, MenuEntry},
    ui::{
        state::{DrawNode, Rect, State},
        vm::Vm,
    },
};
use rs09_protocol::game::players::RawAppearance;

mod cycle;
mod frame;
#[cfg(not(target_arch = "wasm32"))]
mod inject;
mod packets;
#[cfg(not(target_arch = "wasm32"))]
mod pose_dump;
mod redraw;

/// The window mode, canvas, left margin and frame last reported on stderr.
type Shown = (i32, [i32; 2], i32, [i32; 2]);

pub(super) struct Game {
    // Launch switches, fixed for the run.
    pub(super) live_mode: bool,
    pub(super) capture_realtime: bool,
    pub(super) capture_npcs: bool,
    pub(super) capture_animation: bool,
    pub(super) capture_models: bool,
    pub(super) capture: bool,
    pub(super) capture_frame: Option<i32>,
    pub(super) portrait: bool,
    pub(super) capture_route: bool,
    pub(super) capture_route_run: bool,
    pub(super) frustum_culling: bool,
    /// The launch switches over the lighting and texture detail.
    pub(super) detail: scene_detail::Detail,
    pub(super) capture_route_target: [i32; 2],
    pub(super) npc_packet_mode: bool,
    pub(super) streaming_map: bool,
    pub(super) workbench_keys: bool,
    pub(super) fog_allowed: bool,
    pub(super) flicker_allowed: bool,
    pub(super) loc_varbit: Option<(i32, i32)>,
    pub(super) animation_origin: i32,
    pub(super) preferences_loaded: bool,

    // What the loop keeps from frame to frame.
    pub(super) capture_route_issued: bool,
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) capture_travel: Option<world_cache::TravelCapture>,
    pub(super) workbench: Workbench,
    pub(super) npc_replay: Option<NpcReplay>,
    pub(super) replay: VecDeque<rs09_protocol::game::Frame>,
    pub(super) ui: Option<State>,
    pub(super) ui_renderer: ui_render::Renderer,
    pub(super) ui_models: ui_model_render::Renderer,
    pub(super) ground_renderer: ground_render::Renderer,
    pub(super) world_picker: world_pick::Picker,
    pub(super) minimap: Option<minimap_render::Renderer>,
    pub(super) drift_random: rs09_game_core::random::JavaRandom,
    pub(super) drift: rs09_client::minimap::Drift,
    pub(super) ui_audio: ui_audio::Effects,
    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
    pub(super) ambient_audio: ambient_audio::Locations,
    pub(super) ui_vm: Vm,
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) preferences_store: Option<rs09_client::preferences_file::Store>,
    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
    pub(super) music: Option<rs09_client::music::Music>,
    pub(super) preferences_applied: Option<u32>,
    pub(super) preferences_shown: Option<(i32, bool, bool)>,
    pub(super) show_ui: bool,
    pub(super) sounds: Vec<(u32, macroquad::audio::Sound)>,
    pub(super) sound_index: usize,
    pub(super) muted: bool,
    pub(super) audio_played: bool,
    pub(super) material: Material,
    pub(super) static_renderer: Option<static_render::Renderer>,
    pub(super) character_shadow_material: Material,
    pub(super) world_map: Option<world_cache::WorldMap>,
    pub(super) title_world: title_world::Title,
    pub(super) position: Vec2,
    pub(super) npc_showcase: bool,
    pub(super) motion: Movement,
    pub(super) route_steps: VecDeque<[i32; 2]>,
    pub(super) destination_flag: walking::DestinationFlag,
    pub(super) click_cross: Option<ClickCross>,
    pub(super) movement_tick: u64,
    pub(super) route_run: bool,
    pub(super) route_status: String,
    pub(super) atmosphere: rs09_client::atmosphere::Atmosphere,
    pub(super) atmosphere_tick: u64,
    pub(super) light_direction: [i32; 3],
    pub(super) lit_scene: u64,
    pub(super) light_meshes: scene_lights::LightMeshes,
    pub(super) follow: rs09_client::follow_camera::Follow,
    pub(super) scene_occlusion: scene_camera::SceneOcclusion,
    pub(super) scene_view: rs09_client::projection::Bounds,
    pub(super) camera_keys: Vec<i32>,
    pub(super) tween: bool,
    pub(super) roofs: bool,
    pub(super) shadows_enabled: bool,
    pub(super) character_shadows: bool,
    pub(super) brightness: i32,
    pub(super) corrected_lights: bool,
    pub(super) paused: bool,
    pub(super) ticks: f64,
    pub(super) world_ticks: f64,
    pub(super) manual: u32,
    pub(super) frame: i32,
    pub(super) context_menu: client_ui::ContextMenu,
    pub(super) menu_trims_seen: u32,
    pub(super) cursor_view: cursor_view::View,
    pub(super) tooltip_seed: rs09_client::ui::action_tooltip::SeedClock,
    pub(super) outbox: ActionOutbox,
    pub(super) session_life: Option<Box<lifecycle_host::Host>>,
    pub(super) connection: Option<rs09_client::live::Connection>,
    pub(super) player_state: rs09_protocol::game::players::State,
    pub(super) live_base: Option<[i32; 2]>,
    pub(super) live_actors: BTreeMap<usize, Movement>,
    pub(super) live_npcs: rs09_client::npc_updates::State,
    pub(super) live_appearances: BTreeMap<usize, Model>,
    pub(super) live_bas: BTreeMap<usize, PlayerBas>,
    pub(super) live_decoded: BTreeMap<usize, RawAppearance>,
    pub(super) motion_appearances: BTreeMap<usize, RawAppearance>,
    pub(super) appearance_cache: BTreeMap<usize, RawAppearance>,
    pub(super) live_forms: BTreeMap<usize, rs09_client::player_form::Form>,
    pub(super) player_picks: Vec<PlayerPick>,
    pub(super) body_overrides: BTreeMap<usize, [i32; 2]>,
    pub(super) actor_states: BTreeMap<usize, rs09_client::actor_animation::State>,
    pub(super) camera_script: rs09_client::camera::Script,
    pub(super) spline_finished: rs09_client::camera::SplineFinished,
    pub(super) actor_sounds: Vec<(usize, rs09_client::actor_animation::Sound, i32)>,
    pub(super) world_sounds: Vec<WorldSound>,
    pub(super) live_diagnostics: Vec<String>,
    pub(super) missing_scene_regions: i32,
    pub(super) cycle_driver: client_cycle::Driver,
    pub(super) render_profile_started: Option<f64>,
    pub(super) static_submission_seconds: f64,
    pub(super) last_render_metrics: serde_json::Value,
    pub(super) ui_time: f64,
    pub(super) ui_cycle: i32,
    pub(super) ui_scene_delta: i32,
    pub(super) input: Input,
    pub(super) input_events: Events,
    pub(super) published_inventory_press_active: bool,
    pub(super) model_cycle: i32,
    pub(super) world_spot_tick: u64,
    pub(super) ui_drag: rs09_client::ui::drag::Controller,
    pub(super) ui_debug: bool,
    pub(super) display_shown: Option<Shown>,

    // What one part of a frame leaves for the parts after it.
    /// The frame's length in seconds, at most 0.1.
    pub(super) dt: f32,
    /// The 20 ms cycles this frame owes.
    pub(super) cycles: u32,
    /// `roofs` or the never-remove-roofs preference.
    pub(super) roofs_frame: bool,
    pub(super) wheel_rotation: i32,
    /// The workbench's W, A, S and D keys.
    pub(super) direction: [i32; 2],
    /// `Keyboard.KEY_CTRL`.
    pub(super) running: bool,
    pub(super) self_size: i32,
    pub(super) view_distance: [i32; 2],
    pub(super) input_camera: Option<SceneCamera>,
    pub(super) plane: usize,
    pub(super) ui_nodes: Vec<DrawNode>,
    pub(super) hover_menu: Menu,
    pub(super) at_title: bool,
    pub(super) viewport: Option<(Rect, Rect)>,
    pub(super) captured_journey_finished: bool,

    // What ties a click to the menu a redraw built.
    /// `MiniMenu.clickProcessingState` and where the menu was built.
    pub(super) menu_click: rs09_client::game_cycle::MenuClick,
    /// The tile a walk or a spell on the world waits for.
    pub(super) walk: rs09_client::game_cycle::Walk,
    /// The spell that waits with it: `MiniMenu.targetInterfaceId` and the
    /// like as they were when it was cast.
    pub(super) walk_spell: Option<rs09_client::ui::targeting::Targeting>,
    /// This cycle moved the world: its camera step follows.
    pub(super) world_ran: bool,
    /// This cycle ran the interfaces: they are laid out again after it.
    pub(super) interfaces_ran: bool,
    /// `--frame-sleep MS`.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) frame_sleep: Option<std::time::Duration>,
}

/// What a walk was asked by.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WalkKind {
    /// `Walk here` in the scene: the yellow cross, the plain walk packet.
    World,
    /// `Walk here` on the minimap: the packet that carries the click.
    Minimap,
    /// A capture run's route.
    Route,
}

/// What `DisplayMode` did since the last look is carried out, and the
/// canvas is placed where it left it. `setWindowMode` clears the size
/// timer, measures the container and reports the window.
fn display_effects(
    ui_vm: &mut Vm,
    cycle_driver: &mut client_cycle::Driver,
    input_events: &mut Events,
    capture: bool,
    display_shown: &mut Option<Shown>,
) {
    if display_host::carry_out(&mut ui_vm.display) {
        cycle_driver.events.window_mode_set();
        cycle_driver.events.frame_measured(ui_vm.display.frame);
    }
    ui_vm.window_mode = ui_vm.display.window_mode();
    display_host::place(&ui_vm.display);
    input_events.canvas.rect = display_host::canvas_rect();
    display_host::camera();
    // Outside capture runs the client says where its canvas is, once
    // per change.
    let shown = (
        ui_vm.window_mode,
        ui_vm.display.canvas,
        ui_vm.display.left_margin,
        ui_vm.display.frame,
    );
    if !capture && *display_shown != Some(shown) {
        *display_shown = Some(shown);
        let (mode, [width, height], left, [frame_width, frame_height]) = shown;
        eprintln!(
            "Window mode {mode}: canvas {width}x{height} at {left},0 in {frame_width}x{frame_height}"
        );
    }
}

impl Game {
    /// One drawn frame of `GameShell.run`: the 20 ms cycles it owes, each a
    /// whole `client.mainLoop` (`rs09_client::game_cycle::cycle`), then
    /// `mainInputLoop` and the redraw. False ends the run.
    pub(super) async fn frame(&mut self) -> bool {
        // A development switch: `--frame-sleep MS` holds every frame back, so
        // that the loop can be watched owing several cycles a frame.
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(pause) = self.frame_sleep {
            std::thread::sleep(pause);
        }
        self.frame_start();
        // The login runs behind the lifecycle, on its own count of cycles.
        self.life_cycles();
        self.apply_session_effects();
        if self.life_frame() {
            next_frame().await;
            return true;
        }
        self.begin_cycles();
        // What hangs on the frame counter or is taken once per frame: the
        // offline replays, the window's events and the workbench's keys.
        self.replays();
        if !self.frame_input().await {
            return false;
        }
        self.frame_prepare();
        if let Some(route) = self.capture_harness() {
            self.walk_to(route, WalkKind::Route);
        }
        for _ in 0..self.cycles {
            rs09_client::game_cycle::cycle(self);
            // CS2 5411 (the title's Quit) ended the application.
            if self.ui_vm.exit_requested {
                return false;
            }
            self.cycle_finish(self.interfaces_ran);
            self.interfaces_ran = false;
            // MouseWheel.wheelRotation is read once.
            self.wheel_rotation = 0;
            // GameShell.resetTimer() set `logicCycles` to 0: the cycle that
            // ended a rebuild is the last of its burst.
            if self.cycle_driver.timer_was_reset() {
                break;
            }
        }
        self.network_status();
        // GameShell.mainInputLoop, then client.mainRedraw.
        self.main_input_loop();
        self.ambient_npcs();
        self.world_upkeep();
        self.after_interfaces();
        self.build_menu();
        self.end_frame();
        if !self.redraw().await {
            return false;
        }
        self.frame += 1;
        next_frame().await;
        true
    }

    /// `Protocol.loop`'s packets (Protocol.java:2412): at most a hundred,
    /// each handled as it is read and stamped with this cycle.
    fn cycle_packets(&mut self) {
        for server_frame in self.cycle_read() {
            if !self.handle_packet(server_frame) {
                break;
            }
        }
        self.apply_session_effects();
    }

    /// A menu entry runs (`MiniMenu.doAction`). A walk on the minimap
    /// starts at once, or teleports staff holding Ctrl and Shift
    /// (MiniMenu.java:613-640).
    fn act(&mut self, chosen: Option<MenuEntry>) {
        if let Some(tile) = self.do_action(chosen) {
            if self.staff_keys() {
                self.staff_teleport(&[[tile[0], tile[1], self.plane as i32]]);
            } else {
                self.walk_to(tile, WalkKind::Minimap);
            }
        }
    }

    /// The click a drag stood for, when the button is let go without one
    /// (Protocol.java:2727-2777).
    fn cycle_release(&mut self, inventory: bool) {
        if let Some(entry) = self.cycle_drag(inventory) {
            if !self.release_opens_menu(&entry) {
                self.act(Some(entry));
            }
        }
    }

    /// Protocol.java:2818-2867: the staff teleports, and the tile the last
    /// scene draw found for a walk or a spell that was waiting for one.
    fn cycle_walk(&mut self) {
        let staff = self.staff_keys();
        if staff {
            if let Some(tile) = self.wheel_plane() {
                self.staff_teleport(&[tile]);
            }
        }
        match self.walk.cycle(staff) {
            Arrived::Teleport(tile) => {
                self.staff_teleport(&[[tile[0], tile[1], self.plane as i32]]);
            }
            Arrived::Spell(tile) => self.cast_on_tile(tile),
            Arrived::Walk(tile) => self.walk_to(tile, WalkKind::World),
            // Only `client.game == 1` has the entry that leads here.
            Arrived::Face(_) | Arrived::Nothing => {}
        }
    }
}

impl Host for Game {
    fn state(&self) -> CycleState {
        use rs09_client::lifecycle::GameState;
        match &self.session_life {
            // The offline workbench is always in game.
            None => CycleState::InGame,
            Some(life) => match life.life.state() {
                GameState::Title if self.title_world.building() => CycleState::TitleLoading,
                GameState::Title => CycleState::Title,
                GameState::Loading => CycleState::Loading,
                // The lifecycle has seen the scene built; `rebuildMap` ends
                // in the next cycle's rebuild step.
                GameState::InGame if self.cycle_driver.events.rebuilding() => CycleState::Loading,
                GameState::InGame => CycleState::InGame,
                GameState::ConnectionLost => CycleState::ConnectionLost,
            },
        }
    }

    fn step(&mut self, step: Step) {
        match step {
            Step::Devices => {
                // The title scene's build spans cycles here, one blocking
                // cycle in the original: the devices wait for it.
                let hold = !rs09_client::game_cycle::reads_devices(self.state());
                self.cycle_devices(hold);
                // SOUND_SONGEND is SoundPlayer.loop's (Protocol.java:2572),
                // which only game state 30 runs.
                if self.state() != CycleState::InGame {
                    self.ui_vm.outgoing.retain(|frame| frame.opcode != 137);
                }
            }
            Step::Rebuild => {
                // The title scene (game state 28) is built between frames.
                if self.state() == CycleState::Loading {
                    self.apply_session_effects();
                    self.cycle_rebuild();
                }
            }
            // LoginManager.loop step 9 reads the rebuild packet that follows
            // the login reply itself; the rest of the login runs behind the
            // lifecycle (`life_cycles`).
            Step::Login => {
                if self.connection.is_some() {
                    self.cycle_packets();
                }
            }
            Step::Timers => self.cycle_timers(),
            Step::Packets => {
                self.cycle_packets();
                if self.state() != CycleState::InGame {
                    self.cycle_driver.cycle_left();
                }
            }
            Step::OwnPackets => self.cycle_own_packets(),
            Step::World => {
                self.advance_ticks();
                // The paused workbench and a capture run's first frame move
                // nothing.
                self.world_ran = self.movement_tick < self.world_ticks as u64;
                if self.world_ran {
                    self.prepare_sequences();
                    self.cycle_world();
                }
            }
            Step::InventoryDrag => self.cycle_release(true),
            Step::Interfaces => {
                self.cycle_interfaces();
                self.interfaces_ran = true;
            }
            Step::Hooks => self.cycle_hooks(),
            Step::Drag => {
                if self.state() == CycleState::Title {
                    self.begin_walk_drag();
                }
                self.cycle_release(false)
            }
            Step::Walk => self.cycle_walk(),
            Step::MenuClick => {
                let chosen = self.cycle_menu_click();
                self.act(chosen);
            }
            Step::Camera => {
                if self.world_ran {
                    self.cycle_camera();
                    self.movement_tick += 1;
                }
            }
            Step::Write => self.cycle_write(),
        }
    }
}
