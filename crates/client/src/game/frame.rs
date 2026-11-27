// The browser build has no capture files or travel captures: some of the
// fields bound below are only read by the desktop build.
#![cfg_attr(target_arch = "wasm32", allow(unused_variables, unused_mut))]
// The statements below were written when these fields were locals of the
// main loop and are kept as they were: a field bound to a local of its own
// name is a reference, so `&mut name` borrows where the compiler would not
// need it to.
#![allow(clippy::needless_borrow)]
use super::*;

impl Game {
    pub(super) fn frame_start(&mut self) {
        let ui_vm = &mut self.ui_vm;
        let session_life = &mut self.session_life;
        let cycle_driver = &mut self.cycle_driver;
        let input = &mut self.input;
        let input_events = &mut self.input_events;
        let capture = self.capture;
        let mut display_shown = self.display_shown;
        macro_rules! display_effects {
            () => {
                display_effects(
                    ui_vm,
                    cycle_driver,
                    input_events,
                    capture,
                    &mut display_shown,
                )
            };
        }
        // The update connection and the cache's storage, where the cache is
        // in memory.
        #[cfg(target_arch = "wasm32")]
        cache_host::cycle();
        // client.mainRedraw's look at the window (client.java:817-851). In
        // game state 30 or 10 a canvas asked to be replaced is (CS2 6016),
        // and a full-screen frame that lost the focus returns to the
        // remembered mode; in any state a new size moves the canvas.
        let state_30_or_10 = session_life.as_ref().is_none_or(|life| {
            use rs09_client::lifecycle::GameState;
            matches!(life.life.state(), GameState::InGame | GameState::Title)
        });
        if state_30_or_10 {
            ui_vm.display.redraw(&mut ui_vm.preferences);
        }
        if let Some(window) = display_host::windowed(&ui_vm.display) {
            ui_vm.display.watch(window);
        }
        // A browser can also end full screen by itself (display_host).
        let full_screen_lost = display_host::full_screen_lost();
        if ui_vm.display.full_screen() && (full_screen_lost || !input.focus && state_30_or_10) {
            ui_vm.display.leave_full_screen(&mut ui_vm.preferences);
        }
        display_effects!();
        // A development aid beside the frame dump: the text of a file named
        // `command` in the folder `RS09_FRAME_DUMP` names is run as
        // `Cheat.execute` runs a typed `::` command (nothing is sent to the
        // server), and the file is removed.
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(folder) = std::env::var_os("RS09_FRAME_DUMP") {
            let request = std::path::Path::new(&folder).join("command");
            if let Ok(text) = std::fs::read_to_string(&request) {
                let _ = std::fs::remove_file(&request);
                let hd = ui_vm.window_mode != 0;
                if let Err(failure) = ui_vm.cheat.run(text.trim(), hd, &mut ui_vm.preferences) {
                    eprintln!("{}: {failure}", text.trim());
                }
            }
        }
        self.display_shown = display_shown;
    }

    pub(super) fn life_cycles(&mut self) {
        let mut session_life = &mut self.session_life;
        if let Some(life) = &mut session_life {
            life.cycles();
        }
    }

    pub(super) fn apply_session_effects(&mut self) {
        let mut workbench = &mut self.workbench;
        let mut ui = &mut self.ui;
        let mut ground_renderer = &mut self.ground_renderer;
        let mut minimap = &mut self.minimap;
        let mut drift_random = &mut self.drift_random;
        let mut drift = &mut self.drift;
        let mut ui_audio = &mut self.ui_audio;
        let mut ui_vm = &mut self.ui_vm;
        let mut title_world = &mut self.title_world;
        let mut motion = &mut self.motion;
        let mut destination_flag = &mut self.destination_flag;
        let mut atmosphere = &mut self.atmosphere;
        let mut follow = &mut self.follow;
        let mut context_menu = &mut self.context_menu;
        let mut outbox = &mut self.outbox;
        let mut session_life = &mut self.session_life;
        let mut connection = &mut self.connection;
        let mut player_state = &mut self.player_state;
        let mut live_actors = &mut self.live_actors;
        let mut live_npcs = &mut self.live_npcs;
        let mut live_appearances = &mut self.live_appearances;
        let mut live_bas = &mut self.live_bas;
        let mut live_decoded = &mut self.live_decoded;
        let mut motion_appearances = &mut self.motion_appearances;
        let mut appearance_cache = &mut self.appearance_cache;
        let mut live_forms = &mut self.live_forms;
        let mut player_picks = &mut self.player_picks;
        let mut body_overrides = &mut self.body_overrides;
        let mut actor_states = &mut self.actor_states;
        let mut camera_script = &mut self.camera_script;
        let mut actor_sounds = &mut self.actor_sounds;
        let mut world_sounds = &mut self.world_sounds;
        let mut cycle_driver = &mut self.cycle_driver;
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        let mut ambient_audio = &mut self.ambient_audio;
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        let mut music = &mut self.music;
        let streaming_map = self.streaming_map;
        let mut live_base = self.live_base;
        let mut route_steps = std::mem::take(&mut self.route_steps);
        // Carries out what the session lifecycle decided, on this loop's state.
        macro_rules! apply_session_effects {
            ($life:expr) => {{
                let scene_ready = !streaming_map
                    || workbench
                        .regions
                        .iter()
                        .any(|r| [r.x, r.z] == motion.head.map(|tile| tile >> 6));
                $life.apply(lifecycle_host::Refs {
                    connection: &mut connection,
                    workbench: &mut workbench,
                    ui: &mut ui,
                    vm: &mut ui_vm,
                    players: &mut player_state,
                    live_base: &mut live_base,
                    actors: &mut live_actors,
                    npcs: &mut live_npcs,
                    appearances: &mut live_appearances,
                    bas: &mut live_bas,
                    decoded: &mut live_decoded,
                    motion_appearances: &mut motion_appearances,
                    appearance_cache: &mut appearance_cache,
                    forms: &mut live_forms,
                    picks: &mut player_picks,
                    body_overrides: &mut body_overrides,
                    actor_states: &mut actor_states,
                    actor_sounds: &mut actor_sounds,
                    world_sounds: &mut world_sounds,
                    camera: &mut camera_script,
                    motion: &mut motion,
                    map_flag: &mut destination_flag,
                    route_steps: &mut route_steps,
                    context_menu: &mut context_menu,
                    outbox: &mut outbox,
                    ground_renderer: &mut ground_renderer,
                    minimap: &mut minimap,
                    atmosphere: &mut atmosphere,
                    ui_audio: &mut ui_audio,
                    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
                    ambient: &mut ambient_audio,
                    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
                    music: &mut music,
                    driver: &mut cycle_driver,
                    scene_ready,
                    drift: &mut drift,
                    follow: &mut follow,
                    title: &mut title_world,
                    random: &mut drift_random,
                })
            }};
        }
        if let Some(life) = &mut session_life {
            apply_session_effects!(life);
        }
        self.live_base = live_base;
        self.route_steps = route_steps;
    }

    pub(super) fn life_frame(&mut self) -> bool {
        let workbench = &mut self.workbench;
        let ui = &mut self.ui;
        let mut ui_renderer = &mut self.ui_renderer;
        let ui_vm = &mut self.ui_vm;
        let mut session_life = &mut self.session_life;
        let connection = &mut self.connection;
        let cycle_driver = &mut self.cycle_driver;
        let mut input = &mut self.input;
        let mut input_events = &mut self.input_events;
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        let mut music = &mut self.music;
        let capture = self.capture;
        let mut display_shown = self.display_shown;
        let mut route_status = std::mem::take(&mut self.route_status);
        macro_rules! display_effects {
            () => {
                display_effects(
                    ui_vm,
                    cycle_driver,
                    input_events,
                    capture,
                    &mut display_shown,
                )
            };
        }
        let mut taken = false;
        if let Some(life) = &mut session_life {
            // The title opens in the remembered window mode.
            display_effects!();
            life.publish_status(connection.is_some());
            if connection.is_none() {
                route_status = life.status();
            }
            if life.takes_frame(connection.is_some()) {
                // The loop's own clock keeps up, so the frames it skips are
                // not replayed when the game returns, and Keyboard.loop and
                // Mouse.loop run as in every game state.
                cycle_driver.skip_frame(&mut input_events, &mut input);
                life.frame(&mut ui_renderer, ui.as_ref(), &workbench.pack);
                #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
                if let Some(music) = &mut music {
                    for _ in 0..life.elapsed_cycles {
                        let _ = music.cycle();
                    }
                }
                taken = true;
            }
        }
        self.display_shown = display_shown;
        self.route_status = route_status;
        taken
    }

    pub(super) fn begin_cycles(&mut self) {
        let cycle_driver = &mut self.cycle_driver;
        let live_mode = self.live_mode;
        let capture_realtime = self.capture_realtime;
        let capture = self.capture;
        let mut ui_time = self.ui_time;
        let ui_cycle = self.ui_cycle;
        let dt = get_frame_time().min(0.1);
        // Whole cycles, at most ten a frame: the network, the interfaces and
        // the world all advance by this count.
        let mut cycles = cycle_driver.begin_frame();
        // A capture run is driven by its frames: one cycle each.
        if capture && !live_mode && !capture_realtime {
            cycles = 1;
        }
        // Until the frame's cycles run, the number of its last one: what the
        // replays that hang on frame counters stamp their packets with.
        if cycles > 0 {
            ui_time = f64::from(ui_cycle.wrapping_add(cycles as i32 - 1));
        }
        self.ui_time = ui_time;
        self.dt = dt;
        self.cycles = cycles;
    }

    pub(super) fn network_status(&mut self) {
        let workbench = &mut self.workbench;
        let ui = &mut self.ui;
        let ground_renderer = &mut self.ground_renderer;
        let ui_audio = &mut self.ui_audio;
        let ui_vm = &mut self.ui_vm;
        let world_map = &mut self.world_map;
        let outbox = &mut self.outbox;
        let mut connection = &mut self.connection;
        let player_state = &mut self.player_state;
        let live_npcs = &mut self.live_npcs;
        let live_forms = &mut self.live_forms;
        let live_diagnostics = &mut self.live_diagnostics;
        let capture = self.capture;
        let frame = self.frame;
        let live_base = self.live_base;
        let missing_scene_regions = self.missing_scene_regions;
        let mut route_status = std::mem::take(&mut self.route_status);
        let last_render_metrics = std::mem::take(&mut self.last_render_metrics);
        if let Some(connection) = &mut connection {
            route_status = if connection.in_game() && missing_scene_regions != 0 {
                format!("Map pack limit: {missing_scene_regions} missing region(s)")
            } else {
                connection.status.clone()
            };
            if frame % 25 == 0 {
                let map_stream_metrics = world_map.as_ref().map(world_cache::WorldMap::metrics);
                let state = serde_json::json!({"status":connection.status,"in_game":connection.in_game(),"frames":connection.frames,"player_index":connection.world.as_ref().map(|w|w.player_index),"base":live_base,"self_tile":player_state.self_player().tile(),"local_players":player_state.local_ids,"npcs":npc_snapshot(&live_npcs),"npc_views":workbench.npc_views(),"slot_poses":*workbench.slot_views.borrow(),"spots":workbench.spot_views,"targeting":ui.as_ref().map(|u|&u.targeting),"inventory":ui.as_ref().map(inventory_snapshot), "latest_chat":ui_vm.messages.first(),"legacy_buttons":ui.as_ref().map(|u|ui_vm.legacy_button_snapshot(u)),"weight":ui_vm.weight,"audio":ui_audio.status(),"appearance":player_state.self_player().appearance.as_ref().map(|a|serde_json::json!({"slots":a.slots,"bas":a.bas,"name37":a.name37})),"rendered_body_parts":workbench.avatar.len(),"player_forms":live_forms,"ground_piles":ground_renderer.snapshot(),"last_action":outbox.last().map(|f|serde_json::json!({"opcode":f.opcode,"payload":f.payload})),"renderer":&last_render_metrics,"map_stream":map_stream_metrics,"diagnostics":live_diagnostics});
                rs09_client::launch::publish(&state);
                #[cfg(not(target_arch = "wasm32"))]
                if capture {
                    std::fs::write("runs/visual/live-state.json", state.to_string())
                        .expect("live capture metadata");
                }
            }
        }
        self.route_status = route_status;
        self.last_render_metrics = last_render_metrics;
    }

    pub(super) fn replays(&mut self) {
        let workbench = &mut self.workbench;
        let mut npc_replay = &mut self.npc_replay;
        let replay = &mut self.replay;
        let mut ui = &mut self.ui;
        let ground_renderer = &mut self.ground_renderer;
        let mut ui_audio = &mut self.ui_audio;
        let ui_vm = &mut self.ui_vm;
        let motion = &mut self.motion;
        let outbox = &mut self.outbox;
        let mut live_npcs = &mut self.live_npcs;
        let actor_states = &mut self.actor_states;
        let actor_sounds = &mut self.actor_sounds;
        let mut world_sounds = &mut self.world_sounds;
        let live_diagnostics = &mut self.live_diagnostics;
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        let ambient_audio = &mut self.ambient_audio;
        let capture_realtime = self.capture_realtime;
        let capture = self.capture;
        let animation_origin = self.animation_origin;
        let world_ticks = self.world_ticks;
        let frame = self.frame;
        let ui_time = self.ui_time;
        let mut route_status = std::mem::take(&mut self.route_status);
        let last_render_metrics = std::mem::take(&mut self.last_render_metrics);
        if let Some(input) = &mut npc_replay {
            let tick = if capture && !capture_realtime {
                frame as u64
            } else {
                world_ticks as u64
            };
            while input.frames.front().is_some_and(|f| f.tick <= tick) {
                let message = input.frames.pop_front().unwrap();
                let packet = rs09_protocol::game::Frame {
                    opcode: message.opcode,
                    payload: message.payload,
                };
                if packet.opcode == 32 {
                    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
                    if let Some(ui) = &mut ui {
                        for error in ambient_audio.npc_update_multi(
                            workbench.audio_services(ui, &ui_vm, &live_npcs),
                            &mut ui_audio,
                        ) {
                            ui.diagnostic(error);
                        }
                    }
                    match workbench.receive_npcs(
                        &mut live_npcs,
                        &packet,
                        NpcInput {
                            cycle: ui_time as i32,
                            self_tile: motion.head,
                            origin: workbench.navigation.origin,
                            animation_origin,
                        },
                        &mut world_sounds,
                    ) {
                        Ok(update) => {
                            let messages = update.diagnostics;
                            #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
                            let messages = if let Some(ui) = &ui {
                                messages
                                    .into_iter()
                                    .chain(ambient_audio.npc_events(
                                        &update.ambient,
                                        workbench.audio_services(ui, &ui_vm, &live_npcs),
                                        &mut ui_audio,
                                    ))
                                    .collect::<Vec<_>>()
                            } else {
                                messages
                            };
                            for message in messages {
                                if !live_diagnostics.contains(&message)
                                    && live_diagnostics.len() < 32
                                {
                                    live_diagnostics.push(message);
                                }
                            }
                        }
                        Err(error) => panic!("NPC fixture decode: {error}"),
                    }
                } else {
                    let handled = if let Some(ui) = &mut ui {
                        ui_vm.apply_frame(ui, &packet).expect("NPC fixture update")
                    } else {
                        false
                    };
                    if !handled {
                        workbench
                            .server_frame(&packet, ui.as_mut(), ui_time as i32)
                            .expect("Offline world effect update");
                    }
                }
            }
            while input
                .player_spots
                .front()
                .is_some_and(|request| request.tick <= tick)
            {
                let request = input.player_spots.pop_front().unwrap();
                match actor_states
                    .entry(rs09_protocol::game::players::SELF)
                    .or_default()
                    .request_spot(
                        request.id,
                        request.packed,
                        ui_time as i32,
                        &workbench.spot_sequences,
                        &workbench.sequences,
                    ) {
                    Ok(events) => {
                        actor_sounds.extend(events.into_iter().map(|sound| {
                            (rs09_protocol::game::players::SELF, sound, ui_time as i32)
                        }))
                    }
                    Err(error) => {
                        let message = format!("Offline player spot{}: {error}", request.id);
                        if !live_diagnostics.contains(&message) && live_diagnostics.len() < 32 {
                            live_diagnostics.push(message);
                        }
                    }
                }
            }
            while input
                .player_slots
                .front()
                .is_some_and(|request| request.tick <= tick)
            {
                let request = input.player_slots.pop_front().unwrap();
                match actor_states
                    .entry(rs09_protocol::game::players::SELF)
                    .or_default()
                    .request_slots(&request.requests, &workbench.sequences)
                {
                    Ok(events) => {
                        actor_sounds.extend(events.into_iter().map(|sound| {
                            (rs09_protocol::game::players::SELF, sound, ui_time as i32)
                        }))
                    }
                    Err(error) => {
                        let message = format!("Offline player slots: {error}");
                        if live_diagnostics.len() < 32 && !live_diagnostics.contains(&message) {
                            live_diagnostics.push(message);
                        }
                    }
                }
            }
            route_status = "Offline NPC update replay".into();
            if frame % 25 == 0 {
                rs09_client::launch::publish(
                    &serde_json::json!({"status":route_status,"npcs":npc_snapshot(&live_npcs),"npc_views":workbench.npc_views(),"slot_poses":*workbench.slot_views.borrow(),"spots":workbench.spot_views,"targeting":ui.as_ref().map(|u|&u.targeting),"inventory":ui.as_ref().map(inventory_snapshot), "latest_chat":ui_vm.messages.first(),"legacy_buttons":ui.as_ref().map(|u|ui_vm.legacy_button_snapshot(u)),"weight":ui_vm.weight,"audio":ui_audio.status(),"ground_piles":ground_renderer.snapshot(),"last_action":outbox.last().map(|f|serde_json::json!({"opcode":f.opcode,"payload":f.payload})),"renderer":&last_render_metrics,"diagnostics":live_diagnostics}),
                );
            }
        }
        if frame > 0 && frame % 50 == 0 {
            while let Some(server_frame) = replay.pop_front() {
                let zone_origin = matches!(server_frame.opcode, 26 | 112);
                let ui_handled = if let Some(ui) = &mut ui {
                    ui_vm.apply_frame(ui, &server_frame).unwrap_or_else(|e| {
                        ui.diagnostic(e);
                        false
                    })
                } else {
                    false
                };
                if !ui_handled
                    && let Err(error) =
                        workbench.server_frame(&server_frame, ui.as_mut(), ui_time as i32)
                {
                    route_status = error;
                }
                if !zone_origin {
                    break;
                }
            }
        }
        self.route_status = route_status;
        self.last_render_metrics = last_render_metrics;
    }

    pub(super) async fn frame_input(&mut self) -> bool {
        let detail = &self.detail;
        let mut world_map = &mut self.world_map;
        let title_world = &self.title_world;
        let live_base = self.live_base;
        let material = &self.material;
        let character_shadow_material = &self.character_shadow_material;
        let ground_renderer = &mut self.ground_renderer;
        let workbench = &mut self.workbench;
        let mut ui = &mut self.ui;
        let ui_renderer = &mut self.ui_renderer;
        let ui_vm = &mut self.ui_vm;
        let sounds = &mut self.sounds;
        let session_life = &mut self.session_life;
        let mut connection = &mut self.connection;
        let cycle_driver = &mut self.cycle_driver;
        let mut input = &mut self.input;
        let input_events = &mut self.input_events;
        let preferences_store = &mut self.preferences_store;
        let live_mode = self.live_mode;
        let capture = self.capture;
        let npc_packet_mode = self.npc_packet_mode;
        let workbench_keys = self.workbench_keys;
        let preferences_loaded = self.preferences_loaded;
        let mut preferences_applied = self.preferences_applied;
        let mut preferences_shown = self.preferences_shown;
        let mut show_ui = self.show_ui;
        let mut sound_index = self.sound_index;
        let mut muted = self.muted;
        let mut audio_played = self.audio_played;
        let mut npc_showcase = self.npc_showcase;
        let mut tween = self.tween;
        let mut roofs = self.roofs;
        let mut shadows_enabled = self.shadows_enabled;
        let mut character_shadows = self.character_shadows;
        let mut brightness = self.brightness;
        let frame = self.frame;
        let mut ui_debug = self.ui_debug;
        let mut display_shown = self.display_shown;
        macro_rules! display_effects {
            () => {
                display_effects(
                    ui_vm,
                    cycle_driver,
                    input_events,
                    capture,
                    &mut display_shown,
                )
            };
        }
        // This frame's key, mouse and focus events, for the `Keyboard` and
        // `Mouse` state the logic cycles below read.
        input_events.pump(&mut input, (miniquad::date::now() * 1000.0) as i64);
        if workbench_keys && is_key_pressed(KeyCode::Escape) && !show_ui {
            return false;
        }
        if workbench_keys && is_key_pressed(KeyCode::F9) {
            show_ui = !show_ui;
        }
        if workbench_keys && is_key_pressed(KeyCode::F10) {
            ui_debug = !ui_debug;
        }
        if let Some(ui) = ui.as_mut().filter(|_| !live_mode) {
            for (key, group) in [
                (KeyCode::F5, -1),
                (KeyCode::F6, 762),
                (KeyCode::F7, 620),
                (KeyCode::F8, 387),
            ] {
                if is_key_pressed(key) {
                    show_ui = true;
                    if group == 762 || group == 620 || group == -1 {
                        // Replay a fresh open: the bank's onOpen mask precedes
                        // IF_OPENTOP and would be removed by closing an old pane.
                        ui.close_sub(746 << 16 | 6, true);
                        ui.close_sub(746 << 16 | 76, true);
                        ui_vm.drain(ui);
                        let path = if cfg!(target_arch = "wasm32") {
                            if group == -1 {
                                "assets/visual/ui-gameframe.bin"
                            } else if group == 762 {
                                "assets/visual/ui-bank.bin"
                            } else {
                                "assets/visual/ui-shop.bin"
                            }
                        } else if group == -1 {
                            "runs/visual/ui-gameframe.bin"
                        } else if group == 762 {
                            "runs/visual/ui-preview.bin"
                        } else {
                            "runs/visual/ui-shop.bin"
                        };
                        match load_file(path).await {
                            Ok(bytes) => {
                                let mut decoder = rs09_protocol::game::Decoder::new(
                                    rs09_protocol::game::Direction::ServerToClient,
                                    1_000_000,
                                    65535,
                                );
                                decoder.push(&bytes).expect("offline fixture");
                                while let Some(frame) =
                                    decoder.next_frame().expect("offline fixture framing")
                                {
                                    if let Err(e) = ui_vm.apply_frame(ui, &frame) {
                                        ui.diagnostic(e);
                                    }
                                }
                            }
                            Err(e) => {
                                ui.diagnostic(format!("Offline preview fixture unavailable: {e}"))
                            }
                        }
                    } else {
                        ui.close_sub(746 << 16 | 6, true);
                        ui_vm.drain(ui);
                        ui.open_sub(746 << 16 | 6, group, 0)
                            .expect("cached interface");
                    }
                }
            }
        }
        if !show_ui && is_key_pressed(KeyCode::G) && !sounds.is_empty() {
            sound_index = (sound_index + 1) % sounds.len();
        }
        if !show_ui && is_key_pressed(KeyCode::M) {
            muted = !muted;
        }
        if (!live_mode && !show_ui && is_key_pressed(KeyCode::F)
            || (capture && !npc_packet_mode && frame == 20))
            && !muted
            && !sounds.is_empty()
        {
            macroquad::audio::play_sound(
                &sounds[sound_index].1,
                macroquad::audio::PlaySoundParams {
                    looped: false,
                    volume: 0.45,
                },
            );
            audio_played = true;
        }
        if !show_ui && is_key_pressed(KeyCode::T) {
            tween = !tween;
        }
        if !npc_packet_mode && !show_ui && is_key_pressed(KeyCode::N) {
            npc_showcase = !npc_showcase;
            workbench.spawn_npcs(npc_showcase);
        }
        if !show_ui && is_key_pressed(KeyCode::R) {
            roofs = !roofs;
        }
        if !show_ui && is_key_pressed(KeyCode::H) {
            // H is Preferences.sceneryShadowsType (off, or 2) and Shift+H is
            // Preferences.characterShadowsOn: CS2 6011 and 6010 set them apart.
            let preferences = &mut ui_vm.preferences;
            if is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift) {
                character_shadows = !character_shadows;
                preferences.character_shadows = character_shadows;
            } else {
                shadows_enabled = !shadows_enabled;
                preferences.scenery_shadows = if shadows_enabled { 2 } else { 0 };
            }
            preferences.writes = preferences.writes.wrapping_add(1);
        }
        if !show_ui && is_key_pressed(KeyCode::B) {
            brightness = brightness % 4 + 1;
            ui_vm.preferences.brightness = brightness;
            ui_vm.preferences.writes = ui_vm.preferences.writes.wrapping_add(1);
        }
        // `::` commands run by scripts since the last frame (Cheat.execute).
        // Their switches and the particle preference are already set; the
        // rest have no counterpart here (see `rs09_client::cheat`).
        for effect in std::mem::take(&mut ui_vm.cheat.pending) {
            use rs09_client::cheat::Effect;
            match effect {
                Effect::Message(text) => {
                    if let Some(ui) = &mut ui {
                        let line = rs09_client::ui::vm::ChatLine {
                            text,
                            phrase: -1,
                            ..Default::default()
                        };
                        ui_vm.add_message(ui, line);
                    }
                }
                Effect::Print(text) => println!("{text}"),
                // Protocol.handleConnectionLost: the connection is dropped.
                // Reconnecting (game state 40) belongs to the login flow.
                Effect::ConnectionLost => {
                    if let Some(connection) = &mut connection {
                        connection.stop("Connection lost (::clientdrop)");
                    }
                }
                // Game state 25 builds the scene again from the loaded maps.
                // Here the collision maps (which ::noclip clears) and the
                // ground items are rebuilt; static geometry is built from
                // the same data once and does not change.
                Effect::RebuildScene => {
                    // client.setGameState(25) (Cheat.java:169): rebuildMap
                    // ends with the marker and MAP_REBUILD_COMPLETE.
                    if session_life
                        .as_mut()
                        .is_some_and(|life| life.life.rebuild_requested())
                    {
                        cycle_driver.events.rebuild_requested();
                    }
                    workbench.navigation = Navigation::new(
                        &workbench.regions,
                        &workbench.placements,
                        workbench.navigation.origin,
                    );
                    if let Err(error) = workbench.refresh_ground(ui.as_mut()) {
                        eprintln!("::rebuild: {error}");
                    }
                }
                // ::wm0 to ::wm3 (Cheat.java:177-188). ::wm3 asks for 1024
                // by 768, which ends in the remembered mode unless the
                // screen is that size (DisplayMode.java:101-103).
                Effect::WindowMode {
                    mode,
                    width,
                    height,
                } => {
                    ui_vm.display.set_window_mode(
                        false,
                        mode,
                        width,
                        height,
                        &mut ui_vm.preferences,
                    );
                    display_effects!();
                }
                Effect::ClearCollision => {
                    rs09_client::cheat::clear_collision(&mut workbench.navigation.maps)
                }
                Effect::Tweening(on) => tween = on,
                // ::replacecanvas (Cheat.java:166) sets GameShell.replaceCanvas:
                // the next redraw replaces the canvas, as CS2 6016 has it do.
                Effect::NoCounterpart(rs09_client::cheat::Missing::ReplaceCanvas) => {
                    ui_vm.display.replace_canvas = true;
                }
                _ => {}
            }
        }
        ui_vm.cheat.tweening = tween;
        ui_renderer.display_fps = ui_vm.cheat.display_fps.then(get_fps);
        // client.mainRedraw: the first frame with an interface ends safe mode.
        if ui.is_some() && show_ui {
            rs09_client::preferences_file::title_shown(&mut ui_vm.preferences);
        }
        // Preferences changed by a script, a key, a command or the saved
        // file take effect here. Command-line choices stand until a
        // preference actually changes.
        if preferences_applied != Some(ui_vm.preferences.writes) {
            let starting = preferences_applied.is_none();
            preferences_applied = Some(ui_vm.preferences.writes);
            let preferences = &ui_vm.preferences;
            rs09_client::materials::set_high_water_detail(preferences.high_water_detail);
            let shown = (
                preferences.brightness,
                preferences.scenery_shadows > 0,
                preferences.character_shadows,
            );
            // A write that changes none of these (safe mode, say) is not a
            // reason to drop a command-line choice.
            let changed = !starting && preferences_shown != Some(shown);
            preferences_shown = Some(shown);
            if changed || starting && preferences_loaded {
                (brightness, shadows_enabled, character_shadows) = shown;
            }
            // Wherever the original calls Preferences.write, mainLoad included.
            if let Some(store) = &preferences_store
                && let Err(error) = store.save(&ui_vm.preferences)
            {
                eprintln!(
                    "Preferences not saved to {}: {error}",
                    store.binary.display()
                );
            }
        }
        // High detail lighting, the brightness baked without it, and high
        // detail textures (scene_detail). A change has the textures generated
        // again (CS2 6006) or the scene rebuilt (CS2 6012, and 6001 without
        // high detail lighting): by the map streaming where there is one,
        // else from the prepared pack.
        let detail_change = detail.apply(&ui_vm.preferences, brightness, Some(frame));
        if detail_change.textures
            && let Err(error) = workbench.textures.regenerate(&workbench.pack)
        {
            eprintln!("Textures not generated again: {error}");
        }
        if detail_change.scene {
            if !capture {
                eprintln!(
                    "Lighting detail or ground decoration changed (high detail lighting {}): the scene is rebuilt",
                    !rs09_client::sd_light::enabled()
                );
            }
            ground_renderer.clear();
            // The stream follows a session's player or the title scene; the
            // offline workbench keeps the scene its pack was loaded with.
            let streamed =
                world_map.is_some() && (title_world.stream_tile().is_some() || live_base.is_some());
            if let Some(map) = &mut world_map {
                map.refresh();
            }
            if !streamed && let Err(error) = workbench.rebuild_static(ui.as_mut()) {
                eprintln!("Scene not rebuilt for the lighting preference: {error}");
            }
            // The light region says whether the shader lights anything, and
            // a rebuilt scene has a shadow map of its own.
            bind_scene_materials(&workbench, [&material, &character_shadow_material]);
        }
        self.preferences_applied = preferences_applied;
        self.preferences_shown = preferences_shown;
        self.show_ui = show_ui;
        self.sound_index = sound_index;
        self.muted = muted;
        self.audio_played = audio_played;
        self.npc_showcase = npc_showcase;
        self.tween = tween;
        self.roofs = roofs;
        self.shadows_enabled = shadows_enabled;
        self.character_shadows = character_shadows;
        self.brightness = brightness;
        self.ui_debug = ui_debug;
        self.display_shown = display_shown;
        true
    }

    pub(super) fn frame_prepare(&mut self) {
        let workbench = &mut self.workbench;
        let mut ui = &mut self.ui;
        let mut ui_models = &mut self.ui_models;
        let ground_renderer = &mut self.ground_renderer;
        let ui_vm = &mut self.ui_vm;
        let motion = &mut self.motion;
        let follow = &mut self.follow;
        let live_forms = &mut self.live_forms;
        let actor_states = &mut self.actor_states;
        let actor_sounds = &mut self.actor_sounds;
        let cycle_driver = &mut self.cycle_driver;
        let input_events = &mut self.input_events;
        let live_mode = self.live_mode;
        let capture = self.capture;
        let show_ui = self.show_ui;
        let position = self.position;
        let mut lit_scene = self.lit_scene;
        let roofs = self.roofs;
        let mut corrected_lights = self.corrected_lights;
        let mut paused = self.paused;
        let mut manual = self.manual;
        let ui_time = self.ui_time;
        let ui_cycle = self.ui_cycle;
        let mut display_shown = self.display_shown;
        macro_rules! display_effects {
            () => {
                display_effects(
                    ui_vm,
                    cycle_driver,
                    input_events,
                    capture,
                    &mut display_shown,
                )
            };
        }
        // ScriptRunner.neverRemoveRoofs (CS2 6021): roof mode 0, all levels drawn.
        let roofs = roofs || ui_vm.preferences.never_remove_roofs;
        if !show_ui && is_key_pressed(KeyCode::P) {
            corrected_lights = !corrected_lights;
            workbench
                .point_lights
                .place(&workbench.regions, corrected_lights);
            lit_scene = workbench.point_lights.revision;
        }
        if !live_mode && !show_ui && is_key_pressed(KeyCode::Space) {
            paused = !paused;
        }
        for (key, id) in [
            (KeyCode::Key1, 808),
            (KeyCode::Key2, 819),
            (KeyCode::Key3, 824),
            (KeyCode::Key4, 863),
            (KeyCode::Key5, 866),
        ] {
            if !live_mode && !show_ui && is_key_pressed(key) {
                manual = 808;
                if let Ok(events) = actor_states
                    .entry(rs09_protocol::game::players::SELF)
                    .or_default()
                    .request(
                        if id == 808 { -1 } else { id },
                        0,
                        motion.queue.len() as i32,
                        &workbench.sequences,
                    )
                {
                    actor_sounds.extend(
                        events
                            .into_iter()
                            .map(|s| (rs09_protocol::game::players::SELF, s, ui_time as i32)),
                    );
                }
            }
        }
        rs09_client::wheel::sample_native(mouse_wheel().1);
        // Keep rotations queued when a render frame has no logical UI tick.
        let wheel_rotation = if !show_ui || ui_cycle <= ui_time as i32 {
            rs09_client::wheel::poll()
        } else {
            0
        };
        // Nothing in the original client reads the wheel for the camera (its
        // `mouseWheelZoom` setting is for plugins); the workbench with its
        // interface hidden steps `Camera.ZOOM` as `API.UpdateCameraZoom` does.
        if !show_ui && wheel_rotation != 0 {
            let step = if wheel_rotation >= 0 { 50 } else { -50 };
            follow.zoom = (follow.zoom + step).clamp(1, 2000);
        }
        let mut direction = [0, 0];
        if !live_mode && !show_ui && is_key_down(KeyCode::W) {
            direction[1] += 1;
        }
        if !live_mode && !show_ui && is_key_down(KeyCode::S) {
            direction[1] -= 1;
        }
        if !live_mode && !show_ui && is_key_down(KeyCode::A) {
            direction[0] -= 1;
        }
        if !live_mode && !show_ui && is_key_down(KeyCode::D) {
            direction[0] += 1;
        }
        let self_size = live_forms
            .get(&rs09_protocol::game::players::SELF)
            .map_or(1, |form| form.size);
        let view_distance = [
            ui_vm.viewport.min_view_distance,
            ui_vm.viewport.max_view_distance,
        ];
        let plane = workbench.ground.plane.clamp(0, 3) as usize;

        if let Some(ui) = &mut ui {
            for error in ground_renderer.refresh(
                ground_render::piles(&workbench),
                ui,
                &mut ui_models,
                &workbench,
            ) {
                ui.diagnostic(error);
            }
            workbench.ground.take_dirty();
        }
        if let Some(ui) = &mut ui {
            ui_vm.location = [position.x as i32, position.y as i32, workbench.ground.plane];
            ui_vm.drain(ui);
        }
        if !ui_vm.display.effects.is_empty() {
            display_effects!();
        }
        let ui_nodes = ui
            .as_mut()
            .map(|ui| {
                ui.layout(screen_width() as i32, screen_height() as i32)
                    .unwrap_or_else(|e| {
                        ui.diagnostic(e);
                        Vec::new()
                    })
            })
            .unwrap_or_default();
        self.roofs_frame = roofs;
        self.lit_scene = lit_scene;
        self.corrected_lights = corrected_lights;
        self.paused = paused;
        self.manual = manual;
        self.display_shown = display_shown;
        self.wheel_rotation = wheel_rotation;
        self.direction = direction;
        self.self_size = self_size;
        self.view_distance = view_distance;
        self.plane = plane;
        self.ui_nodes = ui_nodes;
    }

    pub(super) fn capture_harness(&mut self) -> Option<[i32; 2]> {
        let workbench = &mut self.workbench;
        let motion = &mut self.motion;
        let mut outbox = &mut self.outbox;
        let connection = &mut self.connection;
        let player_state = &mut self.player_state;
        #[cfg(not(target_arch = "wasm32"))]
        let mut capture_travel = &mut self.capture_travel;
        let live_mode = self.live_mode;
        let capture_route = self.capture_route;
        let capture_route_target = self.capture_route_target;
        let streaming_map = self.streaming_map;
        let mut capture_route_issued = self.capture_route_issued;
        let frame = self.frame;
        let live_base = self.live_base;
        let capture_ready = if live_mode {
            connection.as_ref().is_some_and(|c| c.in_game())
                && live_base.is_some()
                && player_state.self_player().appearance.is_some()
                && !workbench.avatar.is_empty()
                && (!streaming_map
                    || workbench
                        .regions
                        .iter()
                        .any(|r| [r.x, r.z] == motion.head.map(|tile| tile >> 6)))
        } else {
            frame == 1
        };
        #[cfg(not(target_arch = "wasm32"))]
        let captured_journey_finished = if let (Some(travel), Some(base), Some(connection)) =
            (&mut capture_travel, live_base, &connection)
        {
            let [x, y] = player_state.self_player().tile();
            if capture_ready
                && let Some(packet) = travel
                    .next(
                        [x + base[0], y + base[1]],
                        connection.frames.get(&225).copied().unwrap_or_default() as u64,
                    )
                    .expect("travel capture waypoint")
            {
                queue_packet(&mut outbox, &packet);
            }
            travel.finished()
        } else {
            false
        };
        #[cfg(target_arch = "wasm32")]
        let captured_journey_finished = false;
        // The route a capture run walks, issued once, as a click on the
        // ground would be.
        let mut route = None;
        if capture_route && !capture_route_issued && capture_ready {
            capture_route_issued = true;
            route = Some(capture_route_target);
        }
        self.capture_route_issued = capture_route_issued;
        self.captured_journey_finished = captured_journey_finished;
        route
    }

    pub(super) fn main_input_loop(&mut self) {
        let workbench = &mut self.workbench;
        let follow = &mut self.follow;
        let scene_occlusion = &mut self.scene_occlusion;
        let input = &mut self.input;
        let show_ui = self.show_ui;
        let dt = self.dt;
        // GameShell.mainInputLoop: held arrow keys turn the follow camera's
        // targets every rendered frame, whatever is on screen. They are
        // Keyboard.pressedKeys, the keys as of the last logic cycle, which
        // is also what the camera report (opcode 21) looks at.
        scene_occlusion.refresh(&workbench);
        follow.keys(
            (f64::from(dt) * 1_000_000_000.0) as i64,
            {
                use rs09_client::input::keyboard::{KEY_DOWN, KEY_LEFT, KEY_RIGHT, KEY_UP};
                let pressed = &input.keyboard.pressed_keys;
                rs09_client::follow_camera::Held {
                    up: pressed[KEY_UP],
                    down: pressed[KEY_DOWN],
                    left: pressed[KEY_LEFT],
                    right: pressed[KEY_RIGHT],
                }
            },
            workbench.ground.plane,
            &scene_occlusion.scene(&workbench),
        );
        // The original client ignores the middle button; the workbench with
        // its interface hidden keeps the drag, in the original's angle units.
        if !show_ui && is_mouse_button_down(MouseButton::Middle) {
            let delta = mouse_delta_position();
            let units = f64::from(3.0 * 2048.0 / std::f32::consts::TAU);
            follow.set_angles(
                follow.pitch_target + f64::from(delta.y) * units,
                follow.yaw_target - f64::from(delta.x) * units,
                workbench.ground.plane,
                &scene_occlusion.scene(&workbench),
            );
        }
    }

    pub(super) fn ambient_npcs(&mut self) {
        let workbench = &mut self.workbench;
        let mut ui = &mut self.ui;
        let mut ui_audio = &mut self.ui_audio;
        let ui_vm = &mut self.ui_vm;
        let live_npcs = &mut self.live_npcs;
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        let ambient_audio = &mut self.ambient_audio;
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        if let Some(ui) = &mut ui {
            for error in ambient_audio.npc_update_multi(
                workbench.audio_services(ui, &ui_vm, &live_npcs),
                &mut ui_audio,
            ) {
                ui.diagnostic(error);
            }
        }
    }

    pub(super) fn world_upkeep(&mut self) {
        let mut workbench = &mut self.workbench;
        let mut ui = &mut self.ui;
        let ui_audio = &mut self.ui_audio;
        let ui_vm = &mut self.ui_vm;
        let material = &mut self.material;
        let static_renderer = &mut self.static_renderer;
        let character_shadow_material = &mut self.character_shadow_material;
        let mut world_map = &mut self.world_map;
        let title_world = &mut self.title_world;
        let mut motion = &mut self.motion;
        let mut atmosphere = &mut self.atmosphere;
        let camera_keys = &mut self.camera_keys;
        let session_life = &mut self.session_life;
        let live_actors = &mut self.live_actors;
        let mut live_npcs = &mut self.live_npcs;
        let mut camera_script = &mut self.camera_script;
        let actor_sounds = &mut self.actor_sounds;
        let mut live_diagnostics = &mut self.live_diagnostics;
        let npc_packet_mode = self.npc_packet_mode;
        let mut position;
        let corrected_lights = self.corrected_lights;
        let live_base = self.live_base;
        let ui_time = self.ui_time;
        if npc_packet_mode {
            let mut previous = std::mem::take(&mut workbench.npcs)
                .into_iter()
                .map(|npc| (npc.index, npc))
                .collect::<BTreeMap<_, _>>();
            workbench.npcs = live_npcs
                .local_ids
                .iter()
                .filter_map(|&index| {
                    let npc = live_npcs.actors.get(&index)?;
                    // A type not to be had stays in the update state only.
                    workbench.npc_definitions.get(&(npc.type_id as u32))?;
                    workbench
                        .npc_bases
                        .contains_key(&npc.metadata.bas)
                        .then(|| {
                            NpcActor::live(
                                index,
                                npc.type_id as u32,
                                npc.motion.clone(),
                                npc.animation.clone(),
                                previous.remove(&index),
                            )
                        })
                })
                .collect();
        }
        for error in workbench.update_npc_views(&ui_vm, ui.as_ref()) {
            if let Some(ui) = &mut ui {
                ui.diagnostic(format!("NPC view: {error}"));
            }
            if live_diagnostics.len() < 32 && !live_diagnostics.contains(&error) {
                live_diagnostics.push(error);
            }
        }
        if let Some(map) = &mut world_map {
            // In game the scene follows the player; the title screen's is
            // built around its own middle.
            if let Some(stream_tile) = title_world.stream_tile().or(live_base.map(|_| motion.head))
                && let Err(error) = map.step(
                    &mut workbench,
                    stream_tile,
                    f64::from(get_frame_time()),
                    static_renderer.as_ref(),
                    [&material, &character_shadow_material],
                    corrected_lights,
                    ui.as_mut(),
                    &mut live_diagnostics,
                )
            {
                let error = format!("World map: {error}");
                if live_diagnostics.len() < 32 && !live_diagnostics.contains(&error) {
                    live_diagnostics.push(error);
                }
            }
        }
        // Game state 28, and the 25 that follows a login, do nothing but
        // build their scene (the last frame stays up meanwhile), so the rest
        // of this frame's time goes to it.
        let frame_held = session_life.as_ref().is_some_and(|l| l.frame_held());
        if let Some(map) = world_map
            .as_mut()
            .filter(|_| title_world.loading() || frame_held)
        {
            let (started, region) = (get_time(), title_world.stream_region());
            while let Some(stream_tile) = title_world
                .stream_tile()
                .or(live_base.map(|_| motion.head))
                .filter(|_| get_time() - started < 0.012)
                .filter(|_| !(title_world.loading() && map.title_installed(region)))
            {
                let _ = map.step(
                    &mut workbench,
                    stream_tile,
                    0.0,
                    static_renderer.as_ref(),
                    [&material, &character_shadow_material],
                    corrected_lights,
                    ui.as_mut(),
                    &mut live_diagnostics,
                );
            }
        }
        // LoginManager.rebuildMap in game state 28: the title scene asked for
        // is built, and its NPCs are placed.
        title_world.installed(
            title_world::Scene {
                world: &mut workbench,
                map: world_map.as_mut(),
                camera: &mut camera_script,
                npcs: &mut live_npcs,
                motion: &mut motion,
                atmosphere: &mut atmosphere,
            },
            ui_time as i32,
        );
        position = vec2(motion.fine[0] as f32 / 128.0, motion.fine[1] as f32 / 128.0);
        // The title scene is seen from its camera, with no player in it.
        if let Some(eye) = title_world.eye(&camera_script) {
            position = eye;
        }
        let origin = workbench.navigation.origin;
        camera_keys.clear();
        for (index, event, cycle) in actor_sounds.drain(..) {
            if let Some(seq) = workbench.sequences.get(&(event.sequence as u32)) {
                let fine = if index == rs09_protocol::game::players::SELF {
                    motion.fine
                } else if let Some(actor) = live_actors.get(&index) {
                    actor.fine
                } else {
                    continue;
                };
                ui_audio.sequence(
                    seq,
                    event.frame,
                    [fine[0] - origin[0] * 128, fine[1] - origin[1] * 128],
                    index == rs09_protocol::game::players::SELF,
                    cycle,
                );
            }
        }
        self.position = position;
    }

    pub(super) fn after_interfaces(&mut self) {
        let mut workbench = &mut self.workbench;
        let ui = &mut self.ui;
        let mut minimap = &mut self.minimap;
        let drift = &mut self.drift;
        let ui_vm = &mut self.ui_vm;
        let world_map = &mut self.world_map;
        let title_world = &mut self.title_world;
        let mut motion = &mut self.motion;
        let mut atmosphere = &mut self.atmosphere;
        let follow = &mut self.follow;
        let scene_view = &mut self.scene_view;
        let mut live_npcs = &mut self.live_npcs;
        let mut camera_script = &mut self.camera_script;
        let show_ui = self.show_ui;
        let ui_nodes = std::mem::take(&mut self.ui_nodes);
        // InterfaceList.resetToLoginScreen: once the title interface's load
        // scripts have placed the camera, the title scene is set up there.
        title_world.after_interfaces(
            !ui_vm.camera_requests.is_empty(),
            title_world::Scene {
                world: &mut workbench,
                map: world_map.as_mut(),
                camera: &mut camera_script,
                npcs: &mut live_npcs,
                motion: &mut motion,
                atmosphere: &mut atmosphere,
            },
        );
        // Where the scene is drawn from now on: the viewport component of the
        // interface just laid out, or the whole window without one.
        *scene_view = scene_camera::view(
            ui.as_ref()
                .filter(|_| show_ui)
                .map(|ui| (ui, &ui_nodes[..])),
            &ui_vm.viewport,
        );
        if let Some(map) = &mut minimap {
            map.fine = motion.fine;
            map.view = rs09_client::minimap::View {
                // MiniMap turns by `(int) Camera.yawTarget`.
                yaw: follow.yaw_target as i32,
                angle_offset: drift.angle_offset,
                zoom_offset: drift.zoom_offset,
            };
            map.state = ui_vm.minimap.state;
        }
        self.ui_nodes = ui_nodes;
    }

    pub(super) fn build_menu(&mut self) {
        let workbench = &mut self.workbench;
        let mut ui = &mut self.ui;
        let ui_models = &mut self.ui_models;
        let ground_renderer = &mut self.ground_renderer;
        let world_picker = &mut self.world_picker;
        let mut minimap = &mut self.minimap;
        let ui_vm = &mut self.ui_vm;
        let motion = &mut self.motion;
        let follow = &mut self.follow;
        let scene_occlusion = &mut self.scene_occlusion;
        let scene_view = &mut self.scene_view;
        let context_menu = &mut self.context_menu;
        let session_life = &mut self.session_life;
        let connection = &mut self.connection;
        let player_state = &mut self.player_state;
        let live_actors = &mut self.live_actors;
        let live_decoded = &mut self.live_decoded;
        let live_forms = &mut self.live_forms;
        let player_picks = &mut self.player_picks;
        let mut camera_script = &mut self.camera_script;
        let input = &mut self.input;
        let show_ui = self.show_ui;
        let view_distance = self.view_distance;
        let plane = self.plane;
        let ui_nodes = std::mem::take(&mut self.ui_nodes);
        let walk = &mut self.walk;
        let menu_click = &mut self.menu_click;
        // The view rectangle is the one laid out for the previous frame.
        let (input_camera, _) = scene_camera::Viewer {
            follow: &follow,
            view: &scene_view,
            view_distance,
            world: &workbench,
            occlusion: &scene_occlusion,
            player: motion.fine,
        }
        .camera(&mut camera_script, false);
        let origin = workbench.navigation.origin;
        let mut hover_menu = rs09_client::interaction::Menu::default();
        hover_menu.add(rs09_client::interaction::MenuEntry {
            option: "Cancel".into(),
            target: String::new(),
            action: 1005,
            key: 0,
            args: [0; 2],
            cursor: -1,
        });
        // LoginManager.processInterface: the point the menu is built for.
        // While the menu is open the original leaves both alone.
        if !context_menu.menu.open {
            menu_click.redraw(&input.mouse);
        }
        let mouse = (
            menu_click.interface_mouse[0] as f32,
            menu_click.interface_mouse[1] as f32,
        );
        let mut minimap_errors = Vec::new();
        // The scene viewport (client code 1337): world entries exist only
        // while the mouse is inside it, and join the menu at its place in
        // the drawing order. Without one the whole window is the view.
        // Logged out there is no world to point at: the mini menu belongs to
        // the game loop of state 30.
        let at_title = session_life.as_ref().is_some_and(|life| life.at_title());
        let viewport = ui
            .as_ref()
            .filter(|_| show_ui)
            .and_then(|ui| rs09_client::ui::viewport::find(ui, &ui_nodes));
        let world_steps = {
            let rect =
                viewport.map_or([0.0, 0.0, screen_width(), screen_height()], |(whole, _)| {
                    [
                        whole.x as f32,
                        whole.y as f32,
                        whole.w as f32,
                        whole.h as f32,
                    ]
                });
            let inside = viewport.is_none_or(|(whole, visible)| {
                visible.w > 0 && visible.h > 0 && whole.contains([mouse.0 as i32, mouse.1 as i32])
            }) && !at_title
                && mouse.0 >= rect[0]
                && mouse.1 >= rect[1]
                && mouse.0 < rect[0] + rect[2]
                && mouse.1 < rect[1] + rect[3];
            world_picker.pick(
                &world_pick::Sources {
                    world: &workbench,
                    players: &player_picks,
                    piles: &ground_renderer,
                },
                &input_camera,
                &world_pick::Pointer {
                    rect,
                    mouse: inside.then(|| input_camera.ray(vec2(mouse.0, mouse.1))),
                },
                plane,
            )
        };
        let spell_on_tile = ui.as_ref().is_some_and(|u| {
            u.selected_item.is_none() && u.targeting.active && u.targeting.mask & 64 != 0
        });
        let (spell_verb, spell_cursor) = ui.as_ref().map_or((String::new(), -1), |u| {
            (u.targeting.verb.clone(), u.targeting.cursor)
        });
        // MiniMenu.addEntries line 1195, Cs1ScriptRunner line 431: hovering
        // the world or the minimap with a tile spell whose source component
        // has gone stops the targeting and adds nothing.
        let spell_source_gone = spell_on_tile
            && ui
                .as_ref()
                .is_some_and(|u| !u.components.contains_key(&u.targeting.source));
        let mut cancel_spell = false;
        let ui_hit = show_ui
            && ui.as_mut().is_some_and(|ui| {
                ui.menu_at_with(
                    &ui_nodes,
                    [mouse.0 as i32, mouse.1 as i32],
                    &mut hover_menu,
                    |c, node, menu| {
                        if c.client_code == rs09_client::ui::viewport::CLIENT_CODE {
                            // Filled in below, where the interface state is free.
                            menu.add(rs09_client::interaction::MenuEntry {
                                option: String::new(),
                                target: String::new(),
                                action: world_pick::ENTRIES,
                                key: 0,
                                args: [0; 2],
                                cursor: -1,
                            });
                            return Some(false);
                        }
                        if c.client_code != 1338 {
                            return None;
                        }
                        let Some(map) = &mut minimap else {
                            return Some(true);
                        };
                        let point = [mouse.0 as i32, mouse.1 as i32];
                        // InterfaceList.java:744-746 takes the click's offset in
                        // the walk; the click acts where this menu was built.
                        map.click_offset = [point[0] - node.rect.x, point[1] - node.rect.y];
                        match map.hover(&workbench.pack, c, node, point) {
                            Ok(Some(_)) if spell_source_gone => {
                                cancel_spell = true;
                                Some(true)
                            }
                            Ok(Some(tile)) => {
                                // Cs1ScriptRunner line 430: a spell that can
                                // target tiles replaces the walk entry.
                                menu.add(rs09_client::interaction::MenuEntry {
                                    option: if spell_on_tile {
                                        spell_verb.clone()
                                    } else {
                                        ui_vm
                                            .walk_text
                                            .clone()
                                            .unwrap_or_else(|| "Walk here".into())
                                    },
                                    target: if spell_on_tile {
                                        " ->".into()
                                    } else {
                                        String::new()
                                    },
                                    action: if spell_on_tile { 11 } else { 60 },
                                    key: 1,
                                    args: [tile[0] - origin[0], tile[1] - origin[1]],
                                    cursor: if spell_on_tile { spell_cursor } else { -1 },
                                });
                                Some(true)
                            }
                            Ok(None) => Some(true),
                            Err(e) => {
                                minimap_errors.push(format!("Minimap: {e}"));
                                Some(true)
                            }
                        }
                    },
                )
            });
        if let Some(ui) = &mut ui {
            for error in minimap_errors {
                ui.diagnostic(error);
            }
        }
        // MiniMenu.addEntries, run where the viewport component is drawn: the
        // entries below go into a menu of their own that takes the place the
        // viewport marked. A layer drawn later that does not let clicks
        // through has already removed the mark, and the world with it.
        let world_entries = hover_menu
            .entries
            .iter()
            .position(|e| e.action == world_pick::ENTRIES);
        let mut ui_menu = std::mem::take(&mut hover_menu);
        if world_entries.is_some() || (viewport.is_none() && !ui_hit && !at_title) {
            for step in &world_steps {
                // No walk entry while an item is selected. The entry carries
                // the point it was built for (MiniMenu.addEntries); the tile
                // under it is found by the scene draw that follows the click.
                if step
                    .walk()
                    .filter(|_| ui.as_ref().is_none_or(|u| u.selected_item.is_none()))
                    .is_some()
                {
                    if spell_source_gone {
                        cancel_spell = true;
                    } else {
                        hover_menu.add(rs09_client::interaction::MenuEntry {
                            option: if spell_on_tile {
                                spell_verb.clone()
                            } else {
                                ui_vm
                                    .walk_text
                                    .clone()
                                    .unwrap_or_else(|| "Walk here".into())
                            },
                            target: if spell_on_tile {
                                " ->".into()
                            } else {
                                String::new()
                            },
                            action: if spell_on_tile { 11 } else { 60 },
                            key: 0,
                            args: [mouse.0 as i32, mouse.1 as i32],
                            cursor: if spell_on_tile { spell_cursor } else { -1 },
                        });
                    }
                }
                // A location shows the definition its varbit or varp selects
                // (LocType.getMultiLoc) and is skipped when that is nothing:
                // the child `sync_multilocs` last resolved, which is drawn.
                if let Some((target, form, kind)) = step.location().and_then(|target| {
                    workbench
                        .menu_location(&target)
                        .map(|(form, kind)| (target, form, kind))
                }) {
                    if let Some(selected) = ui.as_ref().and_then(|u| u.selected_item.as_ref()) {
                        hover_menu.add(rs09_client::interaction::MenuEntry {
                            option: "Use".into(),
                            target: format!("{} -> <col=00ffff>{}", selected.text, kind.name),
                            action: 14,
                            key: target.key(origin),
                            args: [target.tile[0] - origin[0], target.tile[1] - origin[1]],
                            cursor: ui_vm.minimap_cursors[0].0,
                        });
                    } else if ui.as_ref().is_some_and(|u| u.targeting.active) {
                        let d = kind;
                        let ui = ui.as_mut().unwrap();
                        let entry = rs09_client::interaction::MenuEntry {
                            option: String::new(),
                            target: format!("<col=00ffff>{}", d.name),
                            action: 38,
                            key: target.key(origin),
                            args: [target.tile[0] - origin[0], target.tile[1] - origin[1]],
                            cursor: -1,
                        };
                        if let Err(error) =
                            ui.target_entry(&mut hover_menu, 4, Some(&d.params), entry)
                        {
                            ui.diagnostic(error);
                        }
                    } else {
                        hover_menu.location(kind, target, origin);
                        // Examine names the shown definition; the options
                        // keep the placed one in their key.
                        if let Some(examine) =
                            hover_menu.entries.last_mut().filter(|e| e.action == 1004)
                        {
                            examine.key = u64::from(form);
                        }
                    }
                }
                use rs09_client::entity_menu::{self, Actor as MenuActor, Kind as MenuKind};
                let menu_npcs = workbench
                    .npcs
                    .iter()
                    .filter_map(|npc| {
                        workbench.npc_definitions.get(&npc.id).map(|d| MenuActor {
                            index: npc.index,
                            fine: npc.motion.fine,
                            size: d.size,
                        })
                    })
                    .collect::<Vec<_>>();
                let menu_players = player_state
                    .local_ids
                    .iter()
                    .filter_map(|index| {
                        live_actors.get(index).map(|actor| MenuActor {
                            index: *index,
                            fine: actor.fine,
                            size: live_forms.get(index).map_or(1, |form| form.size),
                        })
                    })
                    .collect::<Vec<_>>();
                let actor_keys = step.actor_keys();
                let own = MenuActor {
                    index: rs09_protocol::game::players::SELF,
                    fine: motion.fine,
                    size: live_forms
                        .get(&rs09_protocol::game::players::SELF)
                        .map_or(1, |form| form.size),
                };
                for target in
                    entity_menu::expand_with_own(&actor_keys, &menu_npcs, &menu_players, Some(&own))
                {
                    if target.kind == MenuKind::Npc {
                        let Some(npc) = workbench.npcs.iter().find(|npc| npc.index == target.index)
                        else {
                            continue;
                        };
                        let Some(render_id) = npc.render_id else {
                            continue;
                        };
                        let d = &workbench.npc_types[&render_id].definition;
                        if let Some(ui) = ui
                            .as_mut()
                            .filter(|u| u.targeting.active && u.selected_item.is_none())
                        {
                            if hover_menu.entries.len() < 400 && d.interactive {
                                let entry = rs09_client::interaction::MenuEntry {
                                    option: String::new(),
                                    target: format!(
                                        "<col=ffff00>{}",
                                        rs09_client::npc_interaction::display_name(
                                            d,
                                            ui_vm.combat_level
                                        )
                                    ),
                                    action: 45,
                                    key: npc.index as u64,
                                    args: target.tile,
                                    cursor: -1,
                                };
                                if let Err(error) =
                                    ui.target_entry(&mut hover_menu, 2, Some(&d.params), entry)
                                {
                                    ui.diagnostic(error);
                                }
                            }
                            continue;
                        }
                        let first = hover_menu.entries.len();
                        rs09_client::npc_interaction::entries(
                            &mut hover_menu,
                            d,
                            npc.index as i32,
                            ui_vm.combat_level,
                            ui.as_ref().and_then(|u| u.selected_item.as_ref()),
                        );
                        for entry in &mut hover_menu.entries[first..] {
                            entry.args = target.tile;
                        }
                    } else if let (Some(ui), Some(local)) = (
                        &mut ui,
                        live_decoded.get(&rs09_protocol::game::players::SELF),
                    ) {
                        let result: Result<(), String> = (|| {
                            let local_team = ui_models.player_team(ui, local)?;
                            let index = target.index;
                            if !player_state.local_ids.contains(&index) {
                                return Ok(());
                            }
                            let Some(raw) = live_decoded.get(&index) else {
                                return Ok(());
                            };
                            if player_state.players[index].is_none() {
                                return Ok(());
                            };
                            let name = ui_vm.player_options.name(raw)?;
                            let team = ui_models.player_team(ui, raw)?;
                            rs09_client::player_interaction::entries(
                                &mut hover_menu,
                                &ui_vm.player_options,
                                rs09_client::player_interaction::Context {
                                    game: 0,
                                    local: rs09_client::player_interaction::Player {
                                        name: "",
                                        combat: local.combat,
                                        summoning: local.combat_with_summoning,
                                        range: local.combat_range,
                                        skill: local.skill,
                                        team: local_team,
                                    },
                                    selected: ui.selected_item.as_ref(),
                                    targeting: &ui.targeting,
                                    use_cursor: ui_vm.minimap_cursors[0].0,
                                },
                                &rs09_client::player_interaction::Player {
                                    name: &name,
                                    combat: raw.combat,
                                    summoning: raw.combat_with_summoning,
                                    range: raw.combat_range,
                                    skill: raw.skill,
                                    team,
                                },
                                index as i32,
                                target.tile,
                                connection
                                    .as_ref()
                                    .and_then(|c| c.world.as_ref())
                                    .is_some_and(|w| usize::from(w.player_index) == index),
                            );
                            Ok(())
                        })();
                        if let Err(error) = result {
                            ui.diagnostic(error);
                        }
                    }
                }
                if let Some(tile) = step.pile() {
                    let local = [tile[0] - origin[0], tile[1] - origin[1]];
                    let slot = rs09_protocol::game::ground::Tile {
                        plane: plane as i32,
                        x: local[0],
                        y: local[1],
                    };
                    if let (Some(ui), Ok(Some(stack))) = (&mut ui, workbench.ground.stack(slot)) {
                        for item in stack.items().iter().rev() {
                            let value = item.get();
                            match ui.items.get(&ui.pack, value.id) {
                                Ok(item) => {
                                    if ui.targeting.active && ui.selected_item.is_none() {
                                        let entry = rs09_client::interaction::MenuEntry {
                                            option: String::new(),
                                            target: format!("<col=ff9040>{}", item.name),
                                            action: 39,
                                            key: item.id as u64,
                                            args: local,
                                            cursor: -1,
                                        };
                                        if let Err(error) = ui.target_entry(
                                            &mut hover_menu,
                                            1,
                                            item.params.as_ref(),
                                            entry,
                                        ) {
                                            ui.diagnostic(error);
                                        }
                                    } else {
                                        rs09_client::ground_interaction::entries(
                                            &mut hover_menu,
                                            &item,
                                            local,
                                            ui.selected_item.as_ref(),
                                        );
                                    }
                                }
                                Err(error) => ui.diagnostic(error),
                            }
                        }
                    }
                }
            }
        }
        match world_entries {
            Some(at) => {
                ui_menu.entries.splice(at..=at, hover_menu.entries);
            }
            None => ui_menu.entries.append(&mut hover_menu.entries),
        }
        let mut hover_menu = ui_menu;
        if cancel_spell && let Some(ui) = &mut ui {
            ui_vm.cancel_targeting(ui);
        }
        hover_menu.sort();
        // CS2 5427's cursors for the Use and Examine entries built above.
        rs09_client::cursor::apply_script_cursors(&mut hover_menu, ui_vm.minimap_cursors);
        // The scene draw finds the tile under a walk or a spell that waits
        // for one (SceneGraph.java:3923-3950), and the walk states move on
        // (LoginManager.java:1468-1473).
        let walk_tile =
            |steps: &[world_pick::Step]| steps.iter().find_map(|step| step.walk()).flatten();
        let here = walk_tile(&world_steps);
        walk.drawn(|point| {
            if point == menu_click.interface_mouse {
                return here;
            }
            let rect =
                viewport.map_or([0.0, 0.0, screen_width(), screen_height()], |(whole, _)| {
                    [
                        whole.x as f32,
                        whole.y as f32,
                        whole.w as f32,
                        whole.h as f32,
                    ]
                });
            let steps = world_picker.pick(
                &world_pick::Sources {
                    world: &workbench,
                    players: &player_picks,
                    piles: &ground_renderer,
                },
                &input_camera,
                &world_pick::Pointer {
                    rect,
                    mouse: Some(input_camera.ray(vec2(point[0] as f32, point[1] as f32))),
                },
                plane,
            );
            walk_tile(&steps)
        });
        walk.redrawn();
        self.at_title = at_title;
        self.viewport = viewport;
        self.hover_menu = hover_menu;
        self.ui_nodes = ui_nodes;
        self.input_camera = Some(input_camera);
    }

    pub(super) fn end_frame(&mut self) {
        let ui_vm = &mut self.ui_vm;
        let cycle_driver = &mut self.cycle_driver;
        let input = &mut self.input;
        // client.mainRedraw follows the frame's cycles.
        cycle_driver.end_frame(&input, ui_vm.window_mode, ui_vm.preferences.anti_aliasing);
    }
}
